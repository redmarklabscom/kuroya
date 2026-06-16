use crate::{
    KuroyaApp,
    lsp_completion_ranking::{
        completion_prefix_at, filter_completion_items_by_settings, rank_completion_items,
        rank_completion_items_for_buffer, selected_completion_index,
    },
    lsp_completion_resolve::{CompletionResolveIntent, completion_resolve_preserves_apply_payload},
    path_display::{display_error_label_cow, display_path_label_cow},
    transient_state::LspSignatureHelpPopup,
    workspace_state::{
        active_buffer_lsp_position_matches, buffer_id_path_version_matches,
        lsp_event_path_is_current,
    },
};
use kuroya_core::{BufferId, LspCompletionItem, LspSignatureHelp};
use std::path::{Path, PathBuf};

impl KuroyaApp {
    pub(super) fn handle_lsp_completion_result(
        &mut self,
        id: BufferId,
        path: PathBuf,
        version: u64,
        line: usize,
        column: usize,
        items: Option<Vec<LspCompletionItem>>,
        error: Option<String>,
    ) {
        if !lsp_event_path_is_current(&self.workspace.root, &path)
            || self.completion_buffer_id != Some(id)
            || self.completion_path.as_ref() != Some(&path)
            || self.completion_version != Some(version)
            || self.completion_line != line + 1
            || self.completion_column != column
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
            self.completion_open = false;
            self.completion_items.clear();
            self.completion_buffer_id = None;
            self.completion_path = None;
            self.completion_version = None;
            self.completion_prefix.clear();
            self.completion_preview_resolve_in_flight.clear();
            self.completion_preview_resolve_recent_attempts.clear();
            self.status = completion_failed_status(&error);
        } else if let Some(mut items) = items {
            let total_count = items.len();
            let prefix = self
                .active_buffer()
                .map(|buffer| completion_prefix_at(buffer, line, column))
                .unwrap_or_default();
            filter_completion_items_by_settings(&mut items, &self.settings, &prefix);
            if let Some(buffer) = self.active_buffer() {
                rank_completion_items_for_buffer(&mut items, &prefix, &self.settings, buffer, line);
            } else {
                rank_completion_items(&mut items, &prefix, &self.settings);
            }
            let count = items.len();
            let selected = selected_completion_index(
                &items,
                &prefix,
                &self.settings,
                &self.completion_recent_labels,
                &self.completion_recent_prefix_labels,
            );
            self.completion_open = true;
            self.completion_items = items;
            self.completion_buffer_id = Some(id);
            self.completion_path = Some(path);
            self.completion_version = Some(version);
            self.completion_line = line + 1;
            self.completion_column = column;
            self.completion_prefix = prefix;
            self.completion_selected = selected;
            self.completion_preview_resolve_in_flight.clear();
            self.completion_preview_resolve_recent_attempts.clear();
            self.status = if count == 0 {
                if total_count == 0 {
                    format!("No completions at {}:{}", line + 1, column)
                } else {
                    format!(
                        "No visible completions at {}:{} ({total_count} filtered)",
                        line + 1,
                        column
                    )
                }
            } else if count == total_count {
                format!("{count} completions at {}:{}", line + 1, column)
            } else {
                format!(
                    "{count} completions at {}:{} ({} filtered)",
                    line + 1,
                    column,
                    total_count.saturating_sub(count)
                )
            };
        } else {
            self.completion_open = false;
            self.completion_items.clear();
            self.completion_buffer_id = None;
            self.completion_path = None;
            self.completion_version = None;
            self.completion_prefix.clear();
            self.completion_preview_resolve_in_flight.clear();
            self.completion_preview_resolve_recent_attempts.clear();
            self.status = completion_load_failed_status(&path);
        }
    }

    pub(super) fn handle_lsp_completion_item_resolve_result(
        &mut self,
        id: BufferId,
        path: PathBuf,
        version: u64,
        line: usize,
        column: usize,
        item: Option<LspCompletionItem>,
        fallback_item: LspCompletionItem,
        intent: CompletionResolveIntent,
        error: Option<String>,
    ) {
        if let CompletionResolveIntent::Preview { selected } = intent {
            self.handle_completion_preview_resolve_result(
                id,
                path,
                version,
                line,
                column,
                selected,
                item,
                fallback_item,
                error,
            );
            return;
        }
        let CompletionResolveIntent::Apply { commit_text } = intent else {
            return;
        };
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

        if let Some(item) = item {
            let item = if completion_resolve_preserves_apply_payload(&item, &fallback_item) {
                item
            } else {
                fallback_item
            };
            self.apply_resolved_completion_item_to_active_buffer(item, commit_text);
        } else {
            if let Some(error) = error {
                self.status = completion_resolve_failed_status(&error);
            }
            self.apply_resolved_completion_item_to_active_buffer(fallback_item, commit_text);
        }
    }

    pub(super) fn handle_lsp_signature_help_result(
        &mut self,
        id: BufferId,
        path: PathBuf,
        version: u64,
        line: usize,
        column: usize,
        help: Option<LspSignatureHelp>,
        error: Option<String>,
    ) {
        if !lsp_event_path_is_current(&self.workspace.root, &path)
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
            self.signature_help = None;
            self.status = signature_help_failed_status(&error);
        } else if let Some(help) = help {
            let count = help.signatures.len();
            if count == 0 {
                self.signature_help = None;
                self.status = format!("No signature help at {}:{}", line + 1, column);
                return;
            }
            self.signature_help = Some(LspSignatureHelpPopup {
                id,
                path,
                line: line + 1,
                column,
                help,
            });
            self.status = format!("{count} signatures at {}:{}", line + 1, column);
        } else {
            self.signature_help = None;
            self.status = format!("No signature help at {}:{}", line + 1, column);
        }
    }
}

fn completion_failed_status(error: &str) -> String {
    format!("Completion failed: {}", display_error_label_cow(error))
}

fn completion_resolve_failed_status(error: &str) -> String {
    format!(
        "Completion resolve failed: {}",
        display_error_label_cow(error)
    )
}

fn completion_load_failed_status(path: &Path) -> String {
    format!(
        "Could not load completions for {}",
        display_path_label_cow(path)
    )
}

fn signature_help_failed_status(error: &str) -> String {
    format!("Signature help failed: {}", display_error_label_cow(error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path_display::{DISPLAY_ERROR_LABEL_MAX_CHARS, DISPLAY_PATH_LABEL_MAX_CHARS};
    use crate::{
        app_startup_context::AppStartupContext, lsp_ui_events::LspUiEvent, terminal::TerminalPane,
        transient_state::LspSignatureHelpPopup,
    };
    use kuroya_core::{
        EditorSettings, LspParameterInformation, LspSignatureInformation, LspTextEdit, TextBuffer,
        Workspace,
    };
    use std::{path::PathBuf, sync::Arc, time::Instant};
    use tokio::runtime::Runtime;

    #[test]
    fn completion_failed_status_sanitizes_and_bounds_lsp_error_text() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        let version = push_active_buffer(&mut app, 7, path.clone(), "fn main() {}\n", 0, 0);
        set_completion_request_origin(&mut app, 7, path.clone(), version, 0, 1);

        app.handle_lsp_edit_event(LspUiEvent::CompletionResult {
            id: 7,
            path,
            version,
            line: 0,
            column: 1,
            items: None,
            error: Some(unsafe_error_text()),
        });

        assert_sanitized_error_status(&app.status, "Completion failed: ");
    }

    #[test]
    fn completion_resolve_failed_status_sanitizes_and_bounds_lsp_error_text() {
        let status = completion_resolve_failed_status(&unsafe_error_text());

        assert_sanitized_error_status(&status, "Completion resolve failed: ");
    }

    #[test]
    fn signature_help_failed_status_sanitizes_and_bounds_lsp_error_text() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        let version = push_active_buffer(&mut app, 7, path.clone(), "fn main() {}\n", 0, 0);

        app.handle_lsp_edit_event(LspUiEvent::SignatureHelpResult {
            id: 7,
            path,
            version,
            line: 0,
            column: 1,
            help: None,
            error: Some(unsafe_error_text()),
        });

        assert_sanitized_error_status(&app.status, "Signature help failed: ");
    }

    #[test]
    fn completion_load_failed_status_sanitizes_and_bounds_path_text() {
        let root = PathBuf::from("workspace");
        let path = root.join(format!(
            "bad\n{}\u{202e}tail.rs",
            "very-long-path-".repeat(DISPLAY_PATH_LABEL_MAX_CHARS)
        ));
        let mut app = app_for_test(root.clone());
        let version = push_active_buffer(&mut app, 7, path.clone(), "fn main() {}\n", 0, 0);
        set_completion_request_origin(&mut app, 7, path.clone(), version, 0, 1);

        app.handle_lsp_edit_event(LspUiEvent::CompletionResult {
            id: 7,
            path,
            version,
            line: 0,
            column: 1,
            items: None,
            error: None,
        });

        assert_safe_status_text(&app.status);
        assert!(app.status.starts_with("Could not load completions for "));
        assert!(!app.status.contains('\n'));
        assert!(!app.status.contains('\u{202e}'));
        assert!(app.status.contains("bad"));
        assert!(app.status.contains("..."));
        assert!(
            app.status.chars().count()
                <= "Could not load completions for ".chars().count() + DISPLAY_PATH_LABEL_MAX_CHARS
        );
    }

    #[test]
    fn completion_result_preserves_raw_lsp_payloads_while_status_stays_display_only() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        let version = push_active_buffer(&mut app, 7, path.clone(), "fn main() {}\n", 0, 0);
        set_completion_request_origin(&mut app, 7, path.clone(), version, 0, 1);
        let raw_label = unsafe_label_text("completion");
        let raw_detail = unsafe_label_text("detail");
        let raw_doc = unsafe_label_text("documentation");
        let raw_payload = Arc::new(serde_json::json!({
            "label": raw_label,
            "documentation": raw_doc,
        }));
        let item = LspCompletionItem {
            label: raw_label.clone(),
            detail: Some(raw_detail.clone()),
            documentation: Some(raw_doc.clone()),
            resolve_payload: Some(raw_payload.clone()),
            ..fallback_completion(&path)
        };

        app.handle_lsp_edit_event(LspUiEvent::CompletionResult {
            id: 7,
            path,
            version,
            line: 0,
            column: 1,
            items: Some(vec![item]),
            error: None,
        });

        assert_eq!(app.status, "1 completions at 1:1");
        let stored = app.completion_items.first().expect("completion item");
        assert_eq!(stored.label, raw_label);
        assert_eq!(stored.detail.as_deref(), Some(raw_detail.as_str()));
        assert_eq!(stored.documentation.as_deref(), Some(raw_doc.as_str()));
        assert_eq!(stored.resolve_payload.as_ref(), Some(&raw_payload));
    }

    #[test]
    fn completion_result_after_popup_close_does_not_reopen_completions() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        let version = push_active_buffer(&mut app, 7, path.clone(), "fn main() {}\n", 0, 0);
        set_completion_request_origin(&mut app, 7, path.clone(), version, 0, 1);
        app.clear_completion_popup_state();
        app.status = "Closed completions".to_owned();

        app.handle_lsp_edit_event(LspUiEvent::CompletionResult {
            id: 7,
            path: path.clone(),
            version,
            line: 0,
            column: 1,
            items: Some(vec![fallback_completion(&path)]),
            error: None,
        });

        assert!(!app.completion_open);
        assert!(app.completion_items.is_empty());
        assert_eq!(app.status, "Closed completions");
    }

    #[test]
    fn signature_help_result_preserves_raw_lsp_payloads_while_status_stays_display_only() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        let version = push_active_buffer(&mut app, 7, path.clone(), "fn main() {}\n", 0, 0);
        let help = LspSignatureHelp {
            signatures: vec![LspSignatureInformation {
                label: unsafe_label_text("signature"),
                documentation: Some(unsafe_label_text("signature-doc")),
                parameters: vec![LspParameterInformation {
                    label: unsafe_label_text("parameter"),
                    documentation: Some(unsafe_label_text("parameter-doc")),
                }],
            }],
            active_signature: 0,
            active_parameter: Some(0),
        };

        app.handle_lsp_edit_event(LspUiEvent::SignatureHelpResult {
            id: 7,
            path,
            version,
            line: 0,
            column: 1,
            help: Some(help.clone()),
            error: None,
        });

        assert_eq!(app.status, "1 signatures at 1:1");
        let popup = app.signature_help.as_ref().expect("signature popup");
        assert_eq!(&popup.help, &help);
    }

    #[test]
    fn empty_signature_help_result_closes_stale_popup() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        let version = push_active_buffer(&mut app, 7, path.clone(), "fn main() {}\n", 0, 0);
        app.signature_help = Some(LspSignatureHelpPopup {
            id: 7,
            path: path.clone(),
            line: 1,
            column: 1,
            help: LspSignatureHelp {
                signatures: vec![LspSignatureInformation {
                    label: "stale(value)".to_owned(),
                    documentation: None,
                    parameters: vec![LspParameterInformation {
                        label: "value".to_owned(),
                        documentation: None,
                    }],
                }],
                active_signature: 0,
                active_parameter: Some(0),
            },
        });

        app.handle_lsp_edit_event(LspUiEvent::SignatureHelpResult {
            id: 7,
            path,
            version,
            line: 0,
            column: 1,
            help: Some(LspSignatureHelp {
                signatures: Vec::new(),
                active_signature: 0,
                active_parameter: None,
            }),
            error: None,
        });

        assert!(app.signature_help.is_none());
        assert_eq!(app.status, "No signature help at 1:1");
    }

    #[test]
    fn completion_item_resolve_result_applies_resolved_edits_and_commit_text() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        let mut buffer = TextBuffer::from_text(
            7,
            Some(path.clone()),
            "fn main() {\n    Hash\n}\n".to_owned(),
        );
        buffer.set_single_cursor(buffer.line_column_to_char(1, 8));
        let version = buffer.version();
        app.active = Some(7);
        app.buffers.push(buffer);

        app.handle_lsp_edit_event(LspUiEvent::CompletionItemResolveResult {
            id: 7,
            path: path.clone(),
            version,
            line: 1,
            column: 9,
            item: Some(Box::new(resolved_completion(&path))),
            fallback_item: Box::new(fallback_completion(&path)),
            intent: CompletionResolveIntent::Apply {
                commit_text: Some(".".to_owned()),
            },
            error: None,
        });

        assert_eq!(
            app.buffer(7).expect("buffer").text(),
            "use std::collections::HashMap;\nfn main() {\n    HashMap.\n}\n"
        );
        assert_eq!(app.status, "Inserted completion `HashMap` and `.`");
    }

    #[test]
    fn completion_item_resolve_result_uses_fallback_when_apply_payload_changes() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        let mut buffer = TextBuffer::from_text(
            7,
            Some(path.clone()),
            "fn main() {\n    Hash\n}\n".to_owned(),
        );
        buffer.set_single_cursor(buffer.line_column_to_char(1, 8));
        let version = buffer.version();
        app.active = Some(7);
        app.buffers.push(buffer);
        let mut changed_resolved = resolved_completion(&path);
        changed_resolved.insert_text = "HashSet".to_owned();

        app.handle_lsp_edit_event(LspUiEvent::CompletionItemResolveResult {
            id: 7,
            path: path.clone(),
            version,
            line: 1,
            column: 9,
            item: Some(Box::new(changed_resolved)),
            fallback_item: Box::new(fallback_completion(&path)),
            intent: CompletionResolveIntent::Apply { commit_text: None },
            error: None,
        });

        assert_eq!(
            app.buffer(7).expect("buffer").text(),
            "fn main() {\n    HashMap\n}\n"
        );
        assert_eq!(app.status, "Inserted completion `HashMap`");
    }

    #[test]
    fn stale_completion_item_resolve_result_is_ignored() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        let mut buffer = TextBuffer::from_text(
            7,
            Some(path.clone()),
            "fn main() {\n    Hash\n}\n".to_owned(),
        );
        buffer.set_single_cursor(buffer.line_column_to_char(1, 8));
        let version = buffer.version();
        app.active = Some(7);
        app.buffers.push(buffer);
        app.status = "before".to_owned();

        app.handle_lsp_edit_event(LspUiEvent::CompletionItemResolveResult {
            id: 7,
            path: path.clone(),
            version: version + 1,
            line: 1,
            column: 9,
            item: Some(Box::new(resolved_completion(&path))),
            fallback_item: Box::new(fallback_completion(&path)),
            intent: CompletionResolveIntent::Apply { commit_text: None },
            error: None,
        });

        assert_eq!(
            app.buffer(7).expect("buffer").text(),
            "fn main() {\n    Hash\n}\n"
        );
        assert_eq!(app.status, "before");
    }

    fn push_active_buffer(
        app: &mut KuroyaApp,
        id: BufferId,
        path: PathBuf,
        text: &str,
        line: usize,
        column: usize,
    ) -> u64 {
        let mut buffer = TextBuffer::from_text(id, Some(path), text.to_owned());
        buffer.set_single_cursor(buffer.line_column_to_char(line, column));
        let version = buffer.version();
        app.active = Some(id);
        app.buffers.push(buffer);
        version
    }

    fn set_completion_request_origin(
        app: &mut KuroyaApp,
        id: BufferId,
        path: PathBuf,
        version: u64,
        line: usize,
        column: usize,
    ) {
        app.completion_open = true;
        app.completion_buffer_id = Some(id);
        app.completion_path = Some(path);
        app.completion_version = Some(version);
        app.completion_line = line + 1;
        app.completion_column = column;
    }

    fn unsafe_error_text() -> String {
        format!(
            "first\nsecond\u{202e}{}tail",
            "very-long-error-".repeat(DISPLAY_ERROR_LABEL_MAX_CHARS)
        )
    }

    fn unsafe_label_text(prefix: &str) -> String {
        format!(
            "{prefix}\nvalue\u{202e}{}tail",
            "very-long-label-".repeat(DISPLAY_PATH_LABEL_MAX_CHARS)
        )
    }

    fn assert_sanitized_error_status(status: &str, prefix: &str) {
        assert_safe_status_text(status);
        assert!(status.starts_with(prefix));
        assert!(!status.contains('\n'));
        assert!(!status.contains('\u{202e}'));
        assert!(status.contains("first second"));
        assert!(status.contains("..."));
        assert!(status.chars().count() <= prefix.chars().count() + DISPLAY_ERROR_LABEL_MAX_CHARS);
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

    fn resolved_completion(path: &std::path::Path) -> LspCompletionItem {
        let mut item = fallback_completion(path);
        item.additional_text_edits = vec![LspTextEdit {
            path: path.to_path_buf(),
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 1,
            new_text: "use std::collections::HashMap;\n".to_owned(),
        }];
        item
    }

    fn fallback_completion(path: &std::path::Path) -> LspCompletionItem {
        LspCompletionItem {
            label: "HashMap".to_owned(),
            detail: None,
            documentation: None,
            kind: Some(7),
            deprecated: false,
            is_snippet: false,
            sort_text: None,
            filter_text: None,
            preselect: false,
            commit_characters: Vec::new(),
            insert_text: "HashMap".to_owned(),
            snippet_selection: None,
            snippet_tabstops: Vec::new(),
            snippet_tabstop_groups: Vec::new(),
            text_edit: Some(LspTextEdit {
                path: path.to_path_buf(),
                start_line: 2,
                start_column: 5,
                end_line: 2,
                end_column: 9,
                new_text: "HashMap".to_owned(),
            }),
            insert_text_edit: None,
            additional_text_edits: Vec::new(),
            resolve_payload: None,
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
}
