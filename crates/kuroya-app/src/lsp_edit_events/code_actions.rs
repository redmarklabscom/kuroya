use crate::{
    KuroyaApp,
    lsp_code_actions::{count_auto_import_code_actions, sort_code_actions_for_display},
    path_display::{display_error_label_cow, display_path_label_cow},
    workspace_state::{
        active_buffer_lsp_position_matches, buffer_id_path_version_matches,
        lsp_event_path_is_current,
    },
};
use kuroya_core::{BufferId, LspCodeAction};
use std::path::{Path, PathBuf};

impl KuroyaApp {
    pub(super) fn handle_lsp_code_actions_result(
        &mut self,
        id: BufferId,
        path: PathBuf,
        version: u64,
        line: usize,
        column: usize,
        actions: Option<Vec<LspCodeAction>>,
        error: Option<String>,
    ) {
        if !lsp_event_path_is_current(&self.workspace.root, &path)
            || self.active != Some(id)
            || !buffer_id_path_version_matches(&self.buffers, id, &path, version)
            || !active_buffer_lsp_position_matches(
                self.active_buffer(),
                &path,
                version,
                line,
                column,
            )
        {
            return;
        }
        if let Some(error) = error {
            self.code_actions_open = false;
            self.code_actions.clear();
            self.code_actions_buffer_id = None;
            self.code_actions_path = None;
            self.code_actions_version = None;
            self.status = code_actions_failed_status(&error);
        } else if let Some(mut actions) = actions {
            sort_code_actions_for_display(&mut actions);
            let count = actions.len();
            let auto_import_count = count_auto_import_code_actions(&actions);
            self.code_actions_open = true;
            self.code_actions = actions;
            self.code_actions_buffer_id = Some(id);
            self.code_actions_path = Some(path);
            self.code_actions_version = Some(version);
            self.code_actions_line = line + 1;
            self.code_actions_column = column;
            self.code_actions_selected = 0;
            self.status = if count == 0 {
                format!("No code actions at {}:{}", line + 1, column)
            } else if auto_import_count > 0 {
                format!(
                    "{count} code actions ({auto_import_count} auto-import) at {}:{}",
                    line + 1,
                    column
                )
            } else {
                format!("{count} code actions at {}:{}", line + 1, column)
            };
        } else {
            self.code_actions_open = false;
            self.code_actions.clear();
            self.code_actions_buffer_id = None;
            self.code_actions_path = None;
            self.code_actions_version = None;
            self.status = code_actions_load_failed_status(&path);
        }
    }

    pub(super) fn handle_lsp_code_action_resolve_result(
        &mut self,
        id: BufferId,
        path: PathBuf,
        version: u64,
        line: usize,
        column: usize,
        action: Option<LspCodeAction>,
        error: Option<String>,
    ) {
        if !lsp_event_path_is_current(&self.workspace.root, &path)
            || self.active != Some(id)
            || !buffer_id_path_version_matches(&self.buffers, id, &path, version)
            || !active_buffer_lsp_position_matches(
                self.active_buffer(),
                &path,
                version,
                line,
                column,
            )
        {
            return;
        }

        if let Some(error) = error {
            self.status = code_action_resolve_failed_status(&error);
        } else if let Some(action) = action {
            if !self.apply_code_action_workspace_edit(&action, &action.title) {
                self.status = code_action_resolve_no_edit_status(&path);
            }
        } else {
            self.status = code_action_resolve_no_edit_status(&path);
        }
    }
}

fn code_actions_failed_status(error: &str) -> String {
    format!("Code actions failed: {}", display_error_label_cow(error))
}

fn code_actions_load_failed_status(path: &Path) -> String {
    format!(
        "Could not load code actions for {}",
        display_path_label_cow(path)
    )
}

fn code_action_resolve_failed_status(error: &str) -> String {
    format!(
        "Code action resolve failed: {}",
        display_error_label_cow(error)
    )
}

fn code_action_resolve_no_edit_status(path: &Path) -> String {
    format!(
        "Code action resolve returned no editable changes for {}",
        display_path_label_cow(path)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app_startup_context::AppStartupContext,
        lsp_ui_events::LspUiEvent,
        path_display::{DISPLAY_ERROR_LABEL_MAX_CHARS, DISPLAY_PATH_LABEL_MAX_CHARS},
        terminal::TerminalPane,
    };
    use kuroya_core::{
        EditorSettings, LspTextEdit, LspWorkspaceDocumentChange, LspWorkspaceResourceOperation,
        TextBuffer, Workspace,
    };
    use std::{
        fs,
        path::{Path, PathBuf},
        thread,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };
    use tokio::runtime::Runtime;

    #[test]
    fn code_action_resolve_result_applies_resolved_edits() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "fn main() {}\n".to_owned());
        buffer.set_single_cursor(buffer.line_column_to_char(0, 0));
        let version = buffer.version();
        app.active = Some(7);
        app.buffers.push(buffer);

        app.handle_lsp_edit_event(LspUiEvent::CodeActionResolveResult {
            id: 7,
            path: path.clone(),
            version,
            line: 0,
            column: 1,
            action: Some(resolved_action(&path)),
            error: None,
        });

        assert_eq!(
            app.buffer(7).expect("buffer").text(),
            "use std::collections::HashMap;\nfn main() {}\n"
        );
        assert_eq!(
            app.status,
            "Applied code action `Import HashMap`: changed 1 open buffers"
        );
    }

    #[test]
    fn stale_code_action_resolve_result_is_ignored() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "fn main() {}\n".to_owned());
        buffer.set_single_cursor(buffer.line_column_to_char(0, 0));
        let version = buffer.version();
        app.active = Some(7);
        app.buffers.push(buffer);
        app.status = "before".to_owned();

        app.handle_lsp_edit_event(LspUiEvent::CodeActionResolveResult {
            id: 7,
            path: path.clone(),
            version: version + 1,
            line: 0,
            column: 1,
            action: Some(resolved_action(&path)),
            error: None,
        });

        assert_eq!(app.buffer(7).expect("buffer").text(), "fn main() {}\n");
        assert_eq!(app.status, "before");
    }

    #[test]
    fn code_action_resolve_result_applies_resource_document_changes() {
        let root = temp_workspace("code-action-resource");
        fs::create_dir_all(root.join("src")).unwrap();
        let active_path = root.join("src/main.rs");
        let created_path = root.join("src/generated.rs");
        let mut app = app_for_test(root.clone());
        let mut buffer =
            TextBuffer::from_text(7, Some(active_path.clone()), "fn main() {}\n".to_owned());
        buffer.set_single_cursor(buffer.line_column_to_char(0, 0));
        let version = buffer.version();
        app.active = Some(7);
        app.buffers.push(buffer);

        app.handle_lsp_edit_event(LspUiEvent::CodeActionResolveResult {
            id: 7,
            path: active_path,
            version,
            line: 0,
            column: 1,
            action: Some(resource_action(&created_path)),
            error: None,
        });

        drain_until_lsp_workspace_event(&mut app);

        assert_eq!(fs::read_to_string(&created_path).unwrap(), "generated\n");
        drop(app);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn code_action_resource_preflight_failure_does_not_mutate_open_buffer() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "fn main() {}\n".to_owned());
        buffer.set_single_cursor(buffer.line_column_to_char(0, 0));
        let version = buffer.version();
        app.active = Some(7);
        app.buffers.push(buffer);

        app.handle_lsp_edit_event(LspUiEvent::CodeActionResolveResult {
            id: 7,
            path: path.clone(),
            version,
            line: 0,
            column: 1,
            action: Some(inconsistent_resource_action(
                &path,
                &root.join("src/missing.rs"),
            )),
            error: None,
        });

        assert_eq!(app.buffer(7).expect("buffer").text(), "fn main() {}\n");
        assert!(
            app.status
                .contains("Applied code action `Mixed resource` rejected"),
            "{}",
            app.status
        );
    }

    #[test]
    fn code_action_resolve_result_for_inactive_duplicate_buffer_is_ignored() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        let mut stale = TextBuffer::from_text(7, Some(path.clone()), "fn main() {}\n".to_owned());
        stale.set_single_cursor(0);
        let version = stale.version();
        let mut active = TextBuffer::from_text(8, Some(path.clone()), "fn main() {}\n".to_owned());
        active.set_single_cursor(0);
        app.active = Some(8);
        app.buffers.push(stale);
        app.buffers.push(active);
        app.status = "before".to_owned();

        app.handle_lsp_edit_event(LspUiEvent::CodeActionResolveResult {
            id: 7,
            path: path.clone(),
            version,
            line: 0,
            column: 1,
            action: Some(resolved_action(&path)),
            error: None,
        });

        assert_eq!(app.buffer(7).expect("buffer").text(), "fn main() {}\n");
        assert_eq!(app.buffer(8).expect("buffer").text(), "fn main() {}\n");
        assert_eq!(app.status, "before");
    }

    #[test]
    fn code_action_error_statuses_sanitize_and_bound_lsp_error_text() {
        let error = unsafe_error_text();

        for (prefix, status) in [
            ("Code actions failed: ", code_actions_failed_status(&error)),
            (
                "Code action resolve failed: ",
                code_action_resolve_failed_status(&error),
            ),
        ] {
            assert_safe_status_text(&status);
            assert!(status.starts_with(prefix));
            assert!(status.contains("..."));
            assert!(
                status.chars().count() <= prefix.chars().count() + DISPLAY_ERROR_LABEL_MAX_CHARS
            );
        }
    }

    #[test]
    fn code_action_path_statuses_sanitize_and_bound_file_labels() {
        let path = unsafe_path();

        for (prefix, status) in [
            (
                "Could not load code actions for ",
                code_actions_load_failed_status(&path),
            ),
            (
                "Code action resolve returned no editable changes for ",
                code_action_resolve_no_edit_status(&path),
            ),
        ] {
            assert_safe_status_text(&status);
            assert!(status.starts_with(prefix));
            assert!(status.contains("..."));
            assert!(
                status.chars().count() <= prefix.chars().count() + DISPLAY_PATH_LABEL_MAX_CHARS
            );
        }
    }

    #[test]
    fn code_action_result_preserves_raw_actions_while_status_uses_counts() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "fn main() {}\n".to_owned());
        buffer.set_single_cursor(0);
        let version = buffer.version();
        app.active = Some(7);
        app.buffers.push(buffer);
        let raw_title = unsafe_action_text("Fix\nImport");
        let raw_kind = unsafe_action_text("quickfix\nkind");
        let action = LspCodeAction {
            title: raw_title.clone(),
            kind: Some(raw_kind.clone()),
            edits: Vec::new(),
            document_changes: Vec::new(),
            resolve_payload: None,
        };

        app.handle_lsp_edit_event(LspUiEvent::CodeActionsResult {
            id: 7,
            path,
            version,
            line: 0,
            column: 1,
            actions: Some(vec![action]),
            error: None,
        });

        assert_eq!(app.status, "1 code actions at 1:1");
        assert_eq!(app.code_actions[0].title, raw_title);
        assert_eq!(app.code_actions[0].kind.as_deref(), Some(raw_kind.as_str()));
    }

    fn resolved_action(path: &std::path::Path) -> LspCodeAction {
        LspCodeAction {
            title: "Import HashMap".to_owned(),
            kind: Some("quickfix".to_owned()),
            edits: vec![LspTextEdit {
                path: path.to_path_buf(),
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 1,
                new_text: "use std::collections::HashMap;\n".to_owned(),
            }],
            document_changes: Vec::new(),
            resolve_payload: None,
        }
    }

    fn resource_action(path: &Path) -> LspCodeAction {
        LspCodeAction {
            title: "Create generated module".to_owned(),
            kind: Some("quickfix".to_owned()),
            edits: vec![edit(path, "generated\n")],
            document_changes: vec![
                LspWorkspaceDocumentChange::Resource(LspWorkspaceResourceOperation::CreateFile {
                    path: path.to_path_buf(),
                    overwrite: false,
                    ignore_if_exists: false,
                }),
                LspWorkspaceDocumentChange::TextEdit {
                    path: path.to_path_buf(),
                    version: None,
                    edits: vec![edit(path, "generated\n")],
                },
            ],
            resolve_payload: None,
        }
    }

    fn inconsistent_resource_action(open_path: &Path, missing_path: &Path) -> LspCodeAction {
        LspCodeAction {
            title: "Mixed resource".to_owned(),
            kind: Some("quickfix".to_owned()),
            edits: vec![edit(open_path, "use std::fs;\n")],
            document_changes: vec![LspWorkspaceDocumentChange::Resource(
                LspWorkspaceResourceOperation::DeleteFile {
                    path: missing_path.to_path_buf(),
                    recursive: false,
                    ignore_if_not_exists: false,
                },
            )],
            resolve_payload: None,
        }
    }

    fn edit(path: &Path, new_text: &str) -> LspTextEdit {
        LspTextEdit {
            path: path.to_path_buf(),
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 1,
            new_text: new_text.to_owned(),
        }
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

    fn drain_until_lsp_workspace_event(app: &mut KuroyaApp) {
        for _ in 0..50 {
            if app.handle_events() > 0 {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("timed out waiting for LSP workspace edit event");
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("kuroya-lsp-{name}-{}-{nanos}", std::process::id()))
    }

    fn unsafe_path() -> PathBuf {
        PathBuf::from("workspace/src").join(format!(
            "bad\n{}\u{202e}tail.rs",
            "very-long-path-".repeat(DISPLAY_PATH_LABEL_MAX_CHARS)
        ))
    }

    fn unsafe_error_text() -> String {
        format!(
            "first\nsecond\u{202e}{}tail",
            "very-long-error-".repeat(DISPLAY_ERROR_LABEL_MAX_CHARS)
        )
    }

    fn unsafe_action_text(prefix: &str) -> String {
        format!(
            "{prefix}\u{202e}{}tail",
            "very-long-action-".repeat(DISPLAY_PATH_LABEL_MAX_CHARS)
        )
    }

    fn assert_safe_status_text(status: &str) {
        assert!(
            !status.chars().any(is_unsafe_status_char),
            "status contains unsafe display characters: {status:?}"
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
}
