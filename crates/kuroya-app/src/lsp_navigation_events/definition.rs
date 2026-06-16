use crate::{
    KuroyaApp,
    path_display::{display_error_label_cow, display_path_label_cow},
};
use kuroya_core::{BufferId, LspDefinition};
use std::path::{Path, PathBuf};

use super::active_lsp_navigation_response_matches;

impl KuroyaApp {
    pub(super) fn handle_lsp_definition_result(
        &mut self,
        id: BufferId,
        origin_path: PathBuf,
        version: u64,
        origin_line: usize,
        origin_column: usize,
        definition: Option<LspDefinition>,
        error: Option<String>,
    ) {
        if !active_lsp_navigation_response_matches(
            self,
            id,
            &origin_path,
            version,
            origin_line,
            origin_column,
        ) {
            return;
        }
        if let Some(error) = error {
            self.status = definition_failed_status(&error);
            return;
        }
        if let Some(definition) = definition {
            self.lsp_hover = None;
            if self.open_lsp_file_at(definition.path.clone(), definition.line, definition.column) {
                self.status =
                    definition_found_status(&definition.path, definition.line, definition.column);
            }
        } else {
            self.status = no_definition_status(&origin_path, origin_line + 1, origin_column);
        }
    }
}

fn definition_failed_status(error: &str) -> String {
    format!("Definition failed: {}", display_error_label_cow(error))
}

fn definition_found_status(path: &Path, line: usize, column: usize) -> String {
    format!(
        "Definition: {}:{line}:{column}",
        display_path_label_cow(path)
    )
}

fn no_definition_status(path: &Path, line: usize, column: usize) -> String {
    format!(
        "No definition at {}:{line}:{column}",
        display_path_label_cow(path)
    )
}

#[cfg(test)]
mod tests {
    use super::{definition_failed_status, definition_found_status, no_definition_status};
    use crate::{
        KuroyaApp,
        app_startup_context::AppStartupContext,
        path_display::{
            DISPLAY_ERROR_LABEL_MAX_CHARS, DISPLAY_PATH_LABEL_MAX_CHARS, display_path_label_cow,
        },
        terminal::TerminalPane,
    };
    use kuroya_core::{EditorSettings, LspDefinition, TextBuffer, Workspace};
    use std::{
        path::{Path, PathBuf},
        time::Instant,
    };
    use tokio::runtime::Runtime;

    #[test]
    fn definition_statuses_sanitize_and_bound_path_labels() {
        let path = Path::new("workspace/src").join(format!(
            "bad\n{}\u{202e}definition.rs",
            "very-long-".repeat(32)
        ));
        let label = display_path_label_cow(&path);

        let found = definition_found_status(&path, 12, 4);
        let missing = no_definition_status(&path, 3, 7);

        assert_eq!(found, format!("Definition: {}:12:4", label.as_ref()));
        assert_eq!(missing, format!("No definition at {}:3:7", label.as_ref()));
        assert_safe_status_text(&found);
        assert_safe_status_text(&missing);
        assert!(label.contains("..."));
        assert!(label.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);
    }

    #[test]
    fn definition_failure_status_sanitizes_and_bounds_provider_error() {
        let error = format!(
            "first line\nsecond line \u{202e}{}",
            "very-long-error-".repeat(DISPLAY_ERROR_LABEL_MAX_CHARS)
        );

        let status = definition_failed_status(&error);

        assert_safe_status_text(&status);
        assert!(status.contains("..."));
        assert!(
            status.chars().count()
                <= "Definition failed: ".chars().count() + DISPLAY_ERROR_LABEL_MAX_CHARS
        );
    }

    #[test]
    fn definition_result_rejects_destination_outside_workspace() {
        let root = PathBuf::from("workspace");
        let origin = root.join("src/main.rs");
        let outside = PathBuf::from("outside/main.rs");
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text(
            7,
            Some(origin.clone()),
            "fn main() {\n    target();\n}\n".to_owned(),
        );
        buffer.set_single_cursor(buffer.line_column_to_char(1, 4));
        let version = buffer.version();
        app.buffers.push(buffer);
        app.set_active_buffer(7);

        app.handle_lsp_definition_result(
            7,
            origin,
            version,
            1,
            5,
            Some(LspDefinition {
                path: outside,
                line: 1,
                column: 1,
            }),
            None,
        );

        assert_eq!(app.active, Some(7));
        assert!(app.pending_open_paths.is_empty());
        assert!(app.pending_file_jump.is_none());
        assert_eq!(
            app.status,
            "Cannot open LSP location outside the workspace: main.rs"
        );
    }

    #[test]
    fn definition_result_ignores_stale_buffer_id() {
        let root = PathBuf::from("workspace");
        let origin = root.join("src/main.rs");
        let target = root.join("src/lib.rs");
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text(
            7,
            Some(origin.clone()),
            "fn main() {\n    target();\n}\n".to_owned(),
        );
        buffer.set_single_cursor(buffer.line_column_to_char(1, 4));
        let version = buffer.version();
        app.buffers.push(buffer);
        app.set_active_buffer(7);
        app.status = "unchanged".to_owned();

        app.handle_lsp_definition_result(
            8,
            origin,
            version,
            1,
            5,
            Some(LspDefinition {
                path: target,
                line: 1,
                column: 1,
            }),
            None,
        );

        assert_eq!(app.active, Some(7));
        assert!(app.pending_open_paths.is_empty());
        assert!(app.pending_file_jump.is_none());
        assert_eq!(app.status, "unchanged");
    }

    fn assert_safe_status_text(status: &str) {
        assert!(
            !status.chars().any(is_unsafe_status_char),
            "unsafe status: {status:?}"
        );
    }

    fn is_unsafe_status_char(ch: char) -> bool {
        ch.is_control()
            || matches!(
                ch,
                '\u{061c}'
                    | '\u{200e}'
                    | '\u{200f}'
                    | '\u{2028}'..='\u{202e}'
                    | '\u{2066}'..='\u{2069}'
            )
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
