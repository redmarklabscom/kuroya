use crate::{KuroyaApp, ui_text::count_label};
use kuroya_core::BufferId;

impl KuroyaApp {
    pub(crate) fn close_other_buffers(&mut self, keep: BufferId) {
        let mut closed = 0;
        let mut pending = Vec::new();
        let has_pending_close_buffers = !self.pending_close_buffers.is_empty();
        let mut index = 0;
        while index < self.buffers.len() {
            let buffer = &self.buffers[index];
            let id = buffer.id();
            if id == keep {
                index += 1;
                continue;
            }

            if buffer.is_dirty() {
                if self.dirty_close_buffer != Some(id)
                    && (!has_pending_close_buffers || !self.pending_close_buffers.contains(&id))
                {
                    pending.push(id);
                }
                index += 1;
            } else {
                self.force_close_buffer_at_position(index);
                closed += 1;
            }
        }
        self.pending_close_buffers.extend(pending);
        if self.buffer(keep).is_some() {
            self.set_active_buffer(keep);
        }
        if !self.pending_close_buffers.is_empty() {
            self.begin_next_pending_close();
        } else {
            self.status = close_other_buffers_status(closed);
        }
    }

    pub(crate) fn begin_next_pending_close(&mut self) {
        if self.dirty_close_buffer.is_some() {
            return;
        }
        let pending = std::mem::take(&mut self.pending_close_buffers);
        let mut pending = pending.into_iter();
        while let Some(id) = pending.next() {
            let Some(position) = self.buffers.iter().position(|buffer| buffer.id() == id) else {
                continue;
            };
            if self.buffers[position].is_dirty() {
                let label = super::buffer_close_status_label(
                    &self.buffer_label_for(&self.buffers[position]),
                );
                self.pending_close_buffers.extend(pending);
                self.set_active_buffer(id);
                self.dirty_close_buffer = Some(id);
                self.status = format!("Unsaved changes in {label}");
                return;
            }
            self.force_close_buffer_at_position(position);
        }
    }
}

fn close_other_buffers_status(closed: usize) -> String {
    format!(
        "Closed {}",
        count_label(closed, "other buffer", "other buffers")
    )
}

#[cfg(test)]
mod tests {
    use super::close_other_buffers_status;
    use crate::{KuroyaApp, app_startup_context::AppStartupContext, terminal::TerminalPane};
    use kuroya_core::{EditorSettings, TextBuffer, Workspace};
    use std::{path::PathBuf, time::Instant};
    use tokio::runtime::Runtime;

    #[test]
    fn close_other_buffers_status_uses_count_labels() {
        assert_eq!(close_other_buffers_status(0), "Closed 0 other buffers");
        assert_eq!(close_other_buffers_status(1), "Closed 1 other buffer");
        assert_eq!(close_other_buffers_status(2), "Closed 2 other buffers");
    }

    #[test]
    fn begin_next_pending_close_does_not_replace_active_dirty_guard() {
        let root = PathBuf::from("workspace");
        let pending_path = root.join("src/pending.rs");
        let active_path = root.join("src/active.rs");
        let mut app = app_for_test(root);
        let mut pending = TextBuffer::from_text(7, Some(pending_path), "pending".to_owned());
        pending.mark_dirty();
        let mut active = TextBuffer::from_text(9, Some(active_path), "active".to_owned());
        active.mark_dirty();
        app.buffers.push(pending);
        app.buffers.push(active);
        app.dirty_close_buffer = Some(9);
        app.pending_close_buffers.extend([7, 8]);

        app.begin_next_pending_close();

        assert_eq!(app.dirty_close_buffer, Some(9));
        assert_eq!(app.pending_close_buffers, vec![7, 8]);
    }

    #[test]
    fn close_other_buffers_does_not_duplicate_already_pending_dirty_buffers() {
        let root = PathBuf::from("workspace");
        let mut app = app_for_test(root.clone());
        app.buffers.push(TextBuffer::from_text(
            1,
            Some(root.join("keep.rs")),
            "keep".to_owned(),
        ));
        let mut pending =
            TextBuffer::from_text(7, Some(root.join("pending.rs")), "pending".to_owned());
        pending.mark_dirty();
        app.buffers.push(pending);
        app.pending_close_buffers.push(7);

        app.close_other_buffers(1);

        assert_eq!(app.dirty_close_buffer, Some(7));
        assert!(app.pending_close_buffers.is_empty());
    }

    #[test]
    fn close_other_buffers_skips_dirty_buffer_already_under_close_guard() {
        let root = PathBuf::from("workspace");
        let mut app = app_for_test(root.clone());
        app.buffers.push(TextBuffer::from_text(
            1,
            Some(root.join("keep.rs")),
            "keep".to_owned(),
        ));
        let mut guarded =
            TextBuffer::from_text(7, Some(root.join("guarded.rs")), "guarded".to_owned());
        guarded.mark_dirty();
        app.buffers.push(guarded);
        app.dirty_close_buffer = Some(7);

        app.close_other_buffers(1);

        assert_eq!(app.dirty_close_buffer, Some(7));
        assert!(app.pending_close_buffers.is_empty());
    }

    fn app_for_test(root: PathBuf) -> KuroyaApp {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let settings = EditorSettings::default();
        KuroyaApp::from_startup_context(AppStartupContext {
            runtime: Runtime::new().expect("test runtime"),
            tx,
            rx,
            workspace: Workspace::new(root.clone()),
            settings: settings.clone(),
            settings_panel_draft: settings,
            settings_editor_font_path: String::new(),
            settings_ui_font_path: String::new(),
            theme_picker_selected: 0,
            saved_session: None,
            terminal: TerminalPane::new(root.clone(), 100, 12.0, 1.2),
            watcher: None,
            recent_projects: Vec::new(),
            trusted_workspaces: vec![root],
            now: Instant::now(),
            startup_timings: Vec::new(),
        })
    }
}
