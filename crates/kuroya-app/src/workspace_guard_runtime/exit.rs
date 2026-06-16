use crate::{
    KuroyaApp,
    save_lifecycle::{dirty_buffer_ids, dirty_buffer_save_block_reason, has_active_save_work},
    transient_state::PendingExit,
    workspace_guard_runtime::workspace_guard_status_message,
};
use eframe::egui::{self, Context};
use kuroya_core::BufferId;

impl KuroyaApp {
    pub(crate) fn handle_close_request(&mut self, ctx: &Context) {
        if !ctx.input(|input| input.viewport().close_requested()) || self.exit_confirmed {
            return;
        }

        let dirty_count = self
            .buffers
            .iter()
            .filter(|buffer| buffer.is_dirty())
            .count();
        let terminal_count = self.terminal_exit_confirmation_count();
        if dirty_count == 0 && terminal_count == 0 {
            self.clear_pending_workspace_switch_for_exit();
            self.exit_confirmed = true;
            self.pending_exit = None;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        self.clear_pending_workspace_switch_for_exit();
        if self.pending_exit.is_none() {
            self.pending_exit = Some(PendingExit::Confirm);
            self.status = exit_confirmation_status(dirty_count, terminal_count);
        }
    }

    pub(crate) fn start_pending_exit_save(&mut self) {
        self.clear_pending_workspace_switch_for_exit();

        let dirty = dirty_buffer_ids(&self.buffers);
        if dirty.is_empty() {
            let terminal_count = self.terminal_exit_confirmation_count();
            if terminal_count == 0 {
                self.exit_confirmed = true;
                self.pending_exit = None;
            } else {
                self.pending_exit = Some(PendingExit::Confirm);
                self.status = exit_confirmation_status(0, terminal_count);
            }
            return;
        }

        let changed_on_disk = self.observed_external_change_buffer_ids();
        if let Some(reason) = dirty_buffer_save_block_reason(
            &dirty,
            &self.buffers,
            &changed_on_disk,
            &self.lossy_decoded_buffers,
            &self.binary_preview_buffers,
            "exiting",
        ) {
            self.status = workspace_guard_status_message(&reason);
            self.pending_exit = Some(PendingExit::Confirm);
            return;
        }

        for &id in &dirty {
            self.spawn_save(id);
        }
        self.pending_exit = Some(PendingExit::Saving { ids: dirty });
        self.advance_pending_exit_after_save();
    }

    pub(crate) fn advance_pending_exit_after_save(&mut self) {
        if self.pending_exit.is_none() && !self.exit_confirmed {
            return;
        }
        self.clear_pending_workspace_switch_for_exit();

        let Some(mut pending) = self.pending_exit.take() else {
            return;
        };
        pending.prune_invalid_buffer_ids(|id| self.buffer(id).is_some());
        let ids = match pending {
            PendingExit::Saving { ids } => ids,
            pending => {
                self.pending_exit = Some(pending);
                return;
            }
        };
        if ids.iter().any(|id| {
            has_active_save_work(
                *id,
                &self.in_flight_saves,
                &self.queued_save_paths,
                &self.pending_format_on_save,
            )
        }) {
            self.pending_exit = Some(PendingExit::Saving { ids });
            return;
        }

        let still_dirty = super::pending_guard_dirty_count(&ids, &self.buffers);
        if still_dirty == 0 {
            let terminal_count = self.terminal_exit_confirmation_count();
            if terminal_count == 0 {
                self.exit_confirmed = true;
                self.pending_exit = None;
            } else {
                self.pending_exit = Some(PendingExit::Confirm);
                self.status = exit_confirmation_status(0, terminal_count);
            }
        } else {
            self.pending_exit = Some(PendingExit::Confirm);
            self.status = exit_paused_status(still_dirty);
        }
    }

    pub(crate) fn pause_pending_exit_after_save_failure(&mut self, id: BufferId) {
        if matches!(
            self.pending_exit.as_ref(),
            Some(PendingExit::Saving { ids }) if ids.contains(&id)
        ) {
            self.clear_pending_workspace_switch_for_exit();
            self.pending_exit = Some(PendingExit::Confirm);
        }
    }

    pub(crate) fn terminal_exit_confirmation_count(&self) -> usize {
        self.terminal
            .exit_confirmation_session_count(self.settings.terminal_confirm_on_exit)
    }
}

fn exit_confirmation_status(dirty_count: usize, terminal_count: usize) -> String {
    match (dirty_count, terminal_count) {
        (0, 0) => "Ready to exit".to_owned(),
        (0, terminals) => {
            let noun = if terminals == 1 {
                "active terminal session"
            } else {
                "active terminal sessions"
            };
            format!("{terminals} {noun} before exit")
        }
        (dirty, 0) => {
            let noun = if dirty == 1 {
                "unsaved file"
            } else {
                "unsaved files"
            };
            format!("{dirty} {noun} before exit")
        }
        (dirty, terminals) => {
            let dirty_noun = if dirty == 1 {
                "unsaved file"
            } else {
                "unsaved files"
            };
            let terminal_noun = if terminals == 1 {
                "active terminal session"
            } else {
                "active terminal sessions"
            };
            format!("{dirty} {dirty_noun} and {terminals} {terminal_noun} before exit")
        }
    }
}

fn exit_paused_status(still_dirty: usize) -> String {
    let noun = if still_dirty == 1 { "file" } else { "files" };
    let verb = if still_dirty == 1 { "has" } else { "have" };
    format!("Exit paused; {still_dirty} {noun} still {verb} unsaved changes")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app_state::{PendingFileReload, PendingFormatOnSave, QueuedFileReload},
        source_control_runtime::source_control_app_for_test,
        transient_state::PendingExit,
    };
    use kuroya_core::{BufferId, TextBuffer};
    use std::path::PathBuf;

    #[test]
    fn pending_exit_waits_for_pending_format_on_save() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = source_control_app_for_test(root, true);
        insert_dirty_pending_format_on_save(&mut app, 7, path);
        app.pending_exit = Some(PendingExit::Saving { ids: vec![7] });

        app.advance_pending_exit_after_save();

        assert!(matches!(
            app.pending_exit,
            Some(PendingExit::Saving { ids }) if ids == vec![7]
        ));
        assert!(!app.exit_confirmed);
    }

    #[test]
    fn pending_exit_save_blocks_pending_clean_reload() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = source_control_app_for_test(root, true);
        insert_dirty_pending_clean_reload(&mut app, 7, path);

        app.start_pending_exit_save();

        assert!(matches!(app.pending_exit, Some(PendingExit::Confirm)));
        assert!(!app.exit_confirmed);
        assert!(app.in_flight_saves.is_empty());
        assert!(app.status.contains("changed on disk"));
        assert!(app.status.contains("before exiting"));
    }

    #[test]
    fn pending_exit_save_blocks_queued_clean_reload() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = source_control_app_for_test(root, true);
        insert_dirty_queued_clean_reload(&mut app, 7, path);

        app.start_pending_exit_save();

        assert!(matches!(app.pending_exit, Some(PendingExit::Confirm)));
        assert!(!app.exit_confirmed);
        assert!(app.in_flight_saves.is_empty());
        assert!(app.status.contains("changed on disk"));
        assert!(app.status.contains("before exiting"));
    }

    #[test]
    fn start_pending_exit_save_clears_pending_workspace_switch_route() {
        let root = PathBuf::from("workspace");
        let mut app = source_control_app_for_test(root, true);
        app.pending_workspace_switch =
            Some(crate::transient_state::PendingWorkspaceSwitch::Confirm {
                target: PathBuf::from("next-workspace"),
            });

        app.start_pending_exit_save();

        assert!(app.pending_workspace_switch.is_none());
        assert!(app.exit_confirmed);
        assert!(app.pending_exit.is_none());
    }

    #[test]
    fn advance_pending_exit_after_save_clears_pending_workspace_switch_route() {
        let root = PathBuf::from("workspace");
        let mut app = source_control_app_for_test(root, true);
        app.pending_exit = Some(PendingExit::Saving { ids: Vec::new() });
        app.pending_workspace_switch =
            Some(crate::transient_state::PendingWorkspaceSwitch::Confirm {
                target: PathBuf::from("next-workspace"),
            });

        app.advance_pending_exit_after_save();

        assert!(app.pending_workspace_switch.is_none());
        assert!(app.exit_confirmed);
        assert!(app.pending_exit.is_none());
    }

    #[test]
    fn advance_pending_exit_after_save_preserves_workspace_switch_without_exit() {
        let root = PathBuf::from("workspace");
        let mut app = source_control_app_for_test(root, true);
        let target = PathBuf::from("next-workspace");
        app.pending_workspace_switch =
            Some(crate::transient_state::PendingWorkspaceSwitch::Confirm {
                target: target.clone(),
            });

        app.advance_pending_exit_after_save();

        assert!(matches!(
            app.pending_workspace_switch,
            Some(crate::transient_state::PendingWorkspaceSwitch::Confirm { target: ref actual })
                if actual == &target
        ));
        assert!(!app.exit_confirmed);
        assert!(app.pending_exit.is_none());
    }

    #[test]
    fn pause_pending_exit_after_save_failure_clears_pending_workspace_switch_route() {
        let root = PathBuf::from("workspace");
        let mut app = source_control_app_for_test(root, true);
        app.pending_exit = Some(PendingExit::Saving { ids: vec![7] });
        app.pending_workspace_switch =
            Some(crate::transient_state::PendingWorkspaceSwitch::Confirm {
                target: PathBuf::from("next-workspace"),
            });

        app.pause_pending_exit_after_save_failure(7);

        assert!(app.pending_workspace_switch.is_none());
        assert!(matches!(app.pending_exit, Some(PendingExit::Confirm)));
        assert!(!app.exit_confirmed);
    }

    #[test]
    fn exit_confirmation_status_uses_file_and_terminal_count_labels() {
        assert_eq!(exit_confirmation_status(0, 0), "Ready to exit");
        assert_eq!(
            exit_confirmation_status(0, 1),
            "1 active terminal session before exit"
        );
        assert_eq!(
            exit_confirmation_status(0, 2),
            "2 active terminal sessions before exit"
        );
        assert_eq!(exit_confirmation_status(1, 0), "1 unsaved file before exit");
        assert_eq!(
            exit_confirmation_status(2, 0),
            "2 unsaved files before exit"
        );
        assert_eq!(
            exit_confirmation_status(1, 1),
            "1 unsaved file and 1 active terminal session before exit"
        );
        assert_eq!(
            exit_confirmation_status(2, 2),
            "2 unsaved files and 2 active terminal sessions before exit"
        );
    }

    #[test]
    fn exit_paused_status_uses_file_count_labels() {
        assert_eq!(
            exit_paused_status(1),
            "Exit paused; 1 file still has unsaved changes"
        );
        assert_eq!(
            exit_paused_status(2),
            "Exit paused; 2 files still have unsaved changes"
        );
    }

    fn insert_dirty_pending_format_on_save(app: &mut KuroyaApp, id: BufferId, path: PathBuf) {
        let mut buffer = TextBuffer::from_text(id, Some(path.clone()), "fn main() {}\n".to_owned());
        let version = buffer.version();
        buffer.mark_dirty();
        app.buffers.push(buffer);
        app.pending_format_on_save.insert(
            id,
            PendingFormatOnSave {
                save_path: path.clone(),
                format_path: path,
                version,
                request_id: 1,
            },
        );
    }

    fn insert_dirty_pending_clean_reload(app: &mut KuroyaApp, id: BufferId, path: PathBuf) {
        let mut buffer = TextBuffer::from_text(id, Some(path.clone()), "fn main() {}\n".to_owned());
        let version = buffer.version();
        buffer.mark_dirty();
        app.buffers.push(buffer);
        app.in_flight_reloads.insert(
            id,
            PendingFileReload {
                request_id: 1,
                path,
                version,
                force_dirty: false,
            },
        );
    }

    fn insert_dirty_queued_clean_reload(app: &mut KuroyaApp, id: BufferId, path: PathBuf) {
        let mut buffer = TextBuffer::from_text(id, Some(path.clone()), "fn main() {}\n".to_owned());
        buffer.mark_dirty();
        app.buffers.push(buffer);
        app.queued_file_reloads.insert(
            id,
            QueuedFileReload {
                path,
                force_dirty: false,
            },
        );
    }
}
