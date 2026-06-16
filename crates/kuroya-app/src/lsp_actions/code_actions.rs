use crate::{
    KuroyaApp,
    lsp_edit_events::workspace_files::workspace_document_changes_contain_resource_operations,
    lsp_runtime::lsp_command_queue_failed_status,
    path_display::{display_path_label_cow, sanitized_display_label_cow},
};
use kuroya_core::{BufferId, LspCodeAction};
use std::{
    borrow::Cow,
    fmt::Write as _,
    path::{Path, PathBuf},
};

const CODE_ACTION_STATUS_TITLE_MAX_CHARS: usize = 120;

impl KuroyaApp {
    pub(crate) fn apply_code_action(&mut self, action: LspCodeAction) {
        let display_title = code_action_status_title(&action.title).into_owned();
        self.code_actions_open = false;
        self.code_actions.clear();
        if !action.edits.is_empty() || !action.document_changes.is_empty() {
            if self
                .current_code_action_target(&display_title, "apply")
                .is_none()
            {
                self.clear_lsp_code_action_state();
                return;
            }
            self.clear_lsp_code_action_state();
            if !self.apply_owned_code_action_workspace_edit(action) {
                self.status = format!("Code action `{display_title}` returned no editable changes");
            }
            return;
        }

        if !action.needs_resolve() {
            self.clear_lsp_code_action_state();
            self.status = format!("Code action `{display_title}` returned no editable changes");
            return;
        }

        let Some((origin_id, origin_path, origin_version, line, character)) =
            self.current_code_action_target(&display_title, "resolve")
        else {
            self.clear_lsp_code_action_state();
            return;
        };
        let Some(client) = self.ensure_lsp_for_buffer(origin_id) else {
            self.clear_lsp_code_action_state();
            self.status = format!("Could not resolve code action `{display_title}`: no LSP server");
            return;
        };
        let trace_label = code_action_lsp_trace_label(&origin_path, line + 1, character + 1);
        if !client.resolve_code_action(
            origin_id,
            origin_path,
            origin_version,
            line,
            character,
            action,
        ) {
            self.clear_lsp_code_action_state();
            self.status = lsp_command_queue_failed_status("codeAction/resolve");
            return;
        }

        self.record_lsp_client_trace("codeAction/resolve", trace_label);
        self.clear_lsp_code_action_state();
        self.status = format!("Resolving code action `{display_title}`");
    }

    fn current_code_action_target(
        &mut self,
        display_title: &str,
        operation: &str,
    ) -> Option<(BufferId, PathBuf, u64, usize, usize)> {
        let Some(origin_id) = self.code_actions_buffer_id else {
            self.status = format!(
                "Could not {operation} code action `{display_title}`: missing action target"
            );
            return None;
        };
        let Some(origin_path) = self.code_actions_path.as_ref() else {
            self.status = format!(
                "Could not {operation} code action `{display_title}`: missing action target"
            );
            return None;
        };
        let Some(origin_version) = self.code_actions_version else {
            self.status = format!(
                "Could not {operation} code action `{display_title}`: missing action target"
            );
            return None;
        };
        let Some((id, path, version, line, character)) = self.active_lsp_position() else {
            self.status = format!(
                "Could not {operation} code action `{display_title}`: no active LSP target"
            );
            return None;
        };
        if id != origin_id
            || path.as_path() != origin_path
            || version != origin_version
            || self.code_actions_line != line + 1
            || self.code_actions_column != character + 1
        {
            self.status =
                format!("Could not {operation} code action `{display_title}`: target changed");
            return None;
        }

        Some((origin_id, path, origin_version, line, character))
    }

    fn apply_owned_code_action_workspace_edit(&mut self, action: LspCodeAction) -> bool {
        let LspCodeAction {
            title,
            edits,
            document_changes,
            ..
        } = action;
        let label = code_action_status_label(&title);
        if workspace_document_changes_contain_resource_operations(&document_changes) {
            self.apply_lsp_workspace_document_changes_for_action(&edits, document_changes, &label);
            return true;
        }
        if !edits.is_empty() {
            self.apply_lsp_workspace_edits(edits, &label);
            return true;
        }
        false
    }

    pub(crate) fn apply_code_action_workspace_edit(
        &mut self,
        action: &LspCodeAction,
        title: &str,
    ) -> bool {
        let label = code_action_status_label(title);
        if workspace_document_changes_contain_resource_operations(&action.document_changes) {
            self.apply_lsp_workspace_document_changes_for_action(
                &action.edits,
                action.document_changes.clone(),
                &label,
            );
            return true;
        }
        if !action.edits.is_empty() {
            self.apply_lsp_workspace_edits(action.edits.clone(), &label);
            return true;
        }
        false
    }
}

fn code_action_status_title(title: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(title, CODE_ACTION_STATUS_TITLE_MAX_CHARS, "code action")
}

fn code_action_status_label(title: &str) -> String {
    let title = code_action_status_title(title);
    let title = title.as_ref();
    let mut status = String::with_capacity("Applied code action ``".len() + title.len());
    status.push_str("Applied code action `");
    status.push_str(title);
    status.push('`');
    status
}

fn code_action_lsp_trace_label(path: &Path, line: usize, column: usize) -> String {
    let path = display_path_label_cow(path);
    let mut label = String::with_capacity(path.len() + 24);
    label.push_str(path.as_ref());
    let _ = write!(label, ":{line}:{column}");
    label
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app_startup_context::AppStartupContext, path_display::DISPLAY_PATH_LABEL_MAX_CHARS,
        terminal::TerminalPane,
    };
    use kuroya_core::{EditorSettings, LspTextEdit, TextBuffer, TextEdit, Workspace};
    use serde_json::json;
    use std::{path::PathBuf, sync::Arc, time::Instant};
    use tokio::runtime::Runtime;

    #[test]
    fn resolving_code_action_rejects_popup_origin_after_buffer_version_changes() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "fn main() {}\n".to_owned());
        buffer.set_single_cursor(0);
        let origin_version = buffer.version();
        let end = buffer.len_chars();
        buffer.apply_edit(TextEdit {
            range: end..end,
            inserted: "// changed\n".to_owned(),
        });
        app.active = Some(7);
        app.buffers.push(buffer);
        app.code_actions_open = true;
        app.code_actions_buffer_id = Some(7);
        app.code_actions_path = Some(path);
        app.code_actions_version = Some(origin_version);
        app.code_actions_line = 1;
        app.code_actions_column = 1;

        app.apply_code_action(resolvable_action());

        assert_eq!(
            app.status,
            "Could not resolve code action `Import HashMap`: target changed"
        );
        assert_code_action_state_cleared(&app);
    }

    #[test]
    fn applying_code_action_clears_consumed_popup_origin_after_success() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "fn main() {}\n".to_owned());
        buffer.set_single_cursor(0);
        let origin_version = buffer.version();
        app.active = Some(7);
        app.buffers.push(buffer);
        app.code_actions_open = true;
        app.code_actions = vec![resolvable_action()];
        app.code_actions_buffer_id = Some(7);
        app.code_actions_path = Some(path.clone());
        app.code_actions_version = Some(origin_version);
        app.code_actions_line = 1;
        app.code_actions_column = 1;
        app.code_actions_selected = 3;

        app.apply_code_action(action_with_title_and_edit(
            "Import HashMap".to_owned(),
            path,
        ));

        assert_eq!(
            app.buffer(7).expect("buffer").text(),
            "use std::collections::HashMap;\nfn main() {}\n"
        );
        assert_eq!(
            app.status,
            "Applied code action `Import HashMap`: changed 1 open buffers"
        );
        assert_code_action_state_cleared(&app);
    }

    #[test]
    fn current_code_action_target_sanitizes_unsafe_status_title() {
        let root = PathBuf::from("workspace");
        let mut app = app_for_test(root);
        let raw_title = unsafe_code_action_title();
        let display_title = code_action_status_title(&raw_title);

        assert!(
            app.current_code_action_target(&display_title, "apply")
                .is_none()
        );

        assert_safe_status_title(
            &app.status,
            "Could not apply code action `",
            "`: missing action target",
        );
    }

    #[test]
    fn applying_code_action_sanitizes_stale_target_status_title() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "fn main() {}\n".to_owned());
        buffer.set_single_cursor(0);
        let origin_version = buffer.version();
        let end = buffer.len_chars();
        buffer.apply_edit(TextEdit {
            range: end..end,
            inserted: "// changed\n".to_owned(),
        });
        app.active = Some(7);
        app.buffers.push(buffer);
        app.code_actions_open = true;
        app.code_actions_buffer_id = Some(7);
        app.code_actions_path = Some(path.clone());
        app.code_actions_version = Some(origin_version);
        app.code_actions_line = 1;
        app.code_actions_column = 1;

        app.apply_code_action(action_with_title_and_edit(unsafe_code_action_title(), path));

        assert_safe_status_title(
            &app.status,
            "Could not apply code action `",
            "`: target changed",
        );
    }

    #[test]
    fn workspace_edit_status_sanitizes_title_without_mutating_action() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let raw_title = unsafe_code_action_title();
        let mut app = app_for_test(root.clone());
        app.buffers.push(TextBuffer::from_text(
            7,
            Some(path.clone()),
            "fn main() {}\n".to_owned(),
        ));
        let mut action = action_with_title_and_edit(raw_title.clone(), path);
        let raw_payload = Arc::new(json!({
            "title": raw_title,
            "data": {
                "line": "first\nsecond\u{202e}",
                "id": 7,
            }
        }));
        action.resolve_payload = Some(raw_payload.clone());

        assert!(app.apply_code_action_workspace_edit(&action, &action.title));

        assert_eq!(action.title, raw_payload["title"].as_str().unwrap());
        assert_eq!(
            action.resolve_payload.as_deref(),
            Some(raw_payload.as_ref())
        );
        assert_eq!(
            app.buffer(7).expect("buffer").text(),
            "use std::collections::HashMap;\nfn main() {}\n"
        );
        assert_safe_status_title(
            &app.status,
            "Applied code action `",
            "`: changed 1 open buffers",
        );
    }

    #[test]
    fn code_action_status_title_borrows_clean_titles() {
        let title = "Import HashMap";

        match code_action_status_title(title) {
            Cow::Borrowed(label) => assert_eq!(label, title),
            Cow::Owned(label) => panic!("expected borrowed title, got {label:?}"),
        }
        assert_eq!(
            code_action_status_label(title),
            "Applied code action `Import HashMap`"
        );
    }

    #[test]
    fn code_action_status_title_owns_dirty_bounded_and_fallback_titles() {
        let dirty_title = unsafe_code_action_title();
        let dirty_label = code_action_status_title(&dirty_title);

        assert!(
            matches!(dirty_label, Cow::Owned(_)),
            "expected owned dirty title"
        );
        assert!(!dirty_label.contains('\n'));
        assert!(!dirty_label.contains('\u{202e}'));
        assert!(dirty_label.contains("..."));
        assert!(dirty_label.chars().count() <= CODE_ACTION_STATUS_TITLE_MAX_CHARS);

        let blank_label = code_action_status_title("\n\t\u{202e}");
        assert!(
            matches!(blank_label, Cow::Owned(_)),
            "expected owned fallback title"
        );
        assert_eq!(blank_label.as_ref(), "code action");
    }

    #[test]
    fn code_action_lsp_trace_label_sanitizes_and_bounds_path() {
        let path = PathBuf::from("workspace/src").join(format!(
            "bad\n{}\u{202e}action.rs",
            "very-long-path-".repeat(DISPLAY_PATH_LABEL_MAX_CHARS)
        ));

        let trace = code_action_lsp_trace_label(&path, 8, 3);

        assert_safe_status_text(&trace);
        assert!(trace.contains("..."), "{trace}");
        assert!(trace.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS + ":8:3".chars().count());
    }

    fn resolvable_action() -> LspCodeAction {
        LspCodeAction {
            title: "Import HashMap".to_owned(),
            kind: Some("quickfix".to_owned()),
            edits: Vec::new(),
            document_changes: Vec::new(),
            resolve_payload: Some(Arc::new(json!({
                "title": "Import HashMap",
                "kind": "quickfix",
                "data": { "id": 7 }
            }))),
        }
    }

    fn action_with_title_and_edit(title: String, path: PathBuf) -> LspCodeAction {
        LspCodeAction {
            title,
            kind: Some("quickfix".to_owned()),
            edits: vec![LspTextEdit {
                path,
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

    fn unsafe_code_action_title() -> String {
        format!(
            "Fix import\n{}\u{202e}tail",
            "very-long-action-title-".repeat(CODE_ACTION_STATUS_TITLE_MAX_CHARS)
        )
    }

    fn assert_safe_status_title(status: &str, prefix: &str, suffix: &str) {
        let title = status
            .strip_prefix(prefix)
            .and_then(|value| value.strip_suffix(suffix))
            .unwrap_or_else(|| panic!("unexpected status: {status}"));

        assert!(!title.contains('\n'), "{status}");
        assert!(!title.contains('\u{202e}'), "{status}");
        assert!(title.contains("..."), "{status}");
        assert!(title.chars().count() <= CODE_ACTION_STATUS_TITLE_MAX_CHARS);
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

    fn assert_code_action_state_cleared(app: &KuroyaApp) {
        assert!(!app.code_actions_open);
        assert!(app.code_actions.is_empty());
        assert_eq!(app.code_actions_buffer_id, None);
        assert_eq!(app.code_actions_path, None);
        assert_eq!(app.code_actions_version, None);
        assert_eq!(app.code_actions_line, 0);
        assert_eq!(app.code_actions_column, 0);
        assert_eq!(app.code_actions_selected, 0);
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
