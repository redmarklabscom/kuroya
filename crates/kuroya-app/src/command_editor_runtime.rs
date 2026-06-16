use crate::KuroyaApp;
use kuroya_core::{BufferId, Command, TextBuffer};
use selection::run_selection_editor_command;

mod selection;

impl KuroyaApp {
    pub(crate) fn run_editor_command(&mut self, command: Command) {
        if run_selection_editor_command(self, &command) {
            return;
        }

        match command {
            Command::ToggleLineComment => {
                let Some(id) = self.active_editor_buffer_id() else {
                    return;
                };
                if self.block_protected_preview_edit(id) {
                    return;
                }
                self.toggle_line_comment_for_buffer(id);
            }
            Command::Undo => self.apply_active_buffer_edit(TextBuffer::undo),
            Command::Redo => self.apply_active_buffer_edit(TextBuffer::redo),
            Command::IndentLines => {
                let Some(id) = self.active_editor_buffer_id() else {
                    return;
                };
                if self.block_protected_preview_edit(id) {
                    return;
                }
                let tab = self.indent_options_for_buffer(id).unit;
                let changed = self
                    .buffer_mut(id)
                    .is_some_and(|buffer| buffer.indent_lines(&tab));
                if changed {
                    self.mark_buffer_changed(id);
                }
            }
            Command::OutdentLines => {
                let Some(id) = self.active_editor_buffer_id() else {
                    return;
                };
                if self.block_protected_preview_edit(id) {
                    return;
                }
                let tab = self.indent_options_for_buffer(id).unit;
                let changed = self
                    .buffer_mut(id)
                    .is_some_and(|buffer| buffer.outdent_lines(&tab));
                if changed {
                    self.mark_buffer_changed(id);
                }
            }
            Command::DeleteLines => self.apply_active_buffer_edit(TextBuffer::delete_lines),
            Command::JoinLines => self.apply_active_buffer_edit(TextBuffer::join_lines),
            Command::DuplicateLines => self.apply_active_buffer_edit(TextBuffer::duplicate_lines),
            Command::MoveLineUp => self.apply_active_buffer_edit(TextBuffer::move_lines_up),
            Command::MoveLineDown => self.apply_active_buffer_edit(TextBuffer::move_lines_down),
            _ => {}
        }
    }

    fn apply_active_buffer_edit(&mut self, edit: impl FnOnce(&mut TextBuffer) -> bool) {
        let Some(id) = self.active_editor_buffer_id() else {
            return;
        };
        if self.block_protected_preview_edit(id) {
            return;
        }
        if self.buffer_mut(id).is_some_and(edit) {
            self.mark_buffer_changed(id);
        }
    }

    fn active_editor_buffer_id(&mut self) -> Option<BufferId> {
        let Some(id) = self.active else {
            self.status = "No active file".to_owned();
            return None;
        };
        if self.buffer(id).is_none() {
            self.status = "No active file".to_owned();
            return None;
        }
        Some(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{app_startup_context::AppStartupContext, terminal::TerminalPane};
    use kuroya_core::{EditorSettings, Workspace};
    use std::{
        path::PathBuf,
        time::{Instant, SystemTime, UNIX_EPOCH},
    };
    use tokio::runtime::Runtime;

    #[test]
    fn editor_commands_report_no_active_file_for_missing_active_buffer() {
        let mut app = app_for_test(temp_root("editor-no-active-file"));

        app.run_editor_command(Command::Undo);
        assert_eq!(app.status, "No active file");

        app.active = Some(404);
        app.status = "previous".to_owned();
        app.run_editor_command(Command::IndentLines);
        assert_eq!(app.status, "No active file");
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

    fn temp_root(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        std::env::temp_dir().join(format!("kuroya-{name}-{}-{nanos}", std::process::id()))
    }
}
