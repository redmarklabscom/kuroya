use crate::{
    KuroyaApp,
    editor_pane_actions::PendingCodeLensCommand,
    lsp_runtime::lsp_command_queue_failed_status,
    path_display::{display_path_label_cow, sanitized_display_label_cow},
};
use kuroya_core::{BufferId, LspCodeLens};
use std::{borrow::Cow, path::Path};

const CODE_LENS_REQUEST_TITLE_MAX_CHARS: usize = 120;
const CODE_LENS_REQUEST_COMMAND_MAX_CHARS: usize = 120;

impl KuroyaApp {
    pub(crate) fn execute_code_lens_command(&mut self, id: BufferId, lens: PendingCodeLensCommand) {
        let Some((_, path, version, _, _)) = self.lsp_position_for_buffer(id) else {
            self.status = "No LSP code lens target".to_owned();
            return;
        };
        let current_lenses = self.code_lenses.get(&path).map(Vec::as_slice);
        if !current_code_lens_command_matches(current_lenses, &lens) {
            self.status = stale_code_lens_command_status(&lens.title);
            return;
        }
        let Some(client) = self.ensure_lsp_for_buffer(id) else {
            self.status = "No LSP server configured for this buffer".to_owned();
            return;
        };

        let status = lsp_code_lens_command_status(&lens.title);
        let trace_detail = lsp_code_lens_command_trace_detail(&path, &lens.command);
        if !client.execute_command(
            id,
            path.clone(),
            version,
            lens.title,
            lens.command,
            lens.arguments,
        ) {
            self.status = lsp_command_queue_failed_status("workspace/executeCommand");
            return;
        }
        self.record_lsp_client_trace("workspace/executeCommand", trace_detail);
        self.status = status;
    }
}

fn current_code_lens_command_matches(
    lenses: Option<&[LspCodeLens]>,
    pending: &PendingCodeLensCommand,
) -> bool {
    lenses.is_some_and(|lenses| {
        lenses.iter().any(|lens| {
            lens.title == pending.title
                && lens.command.as_deref() == Some(pending.command.as_str())
                && lens.command_arguments.as_ref() == pending.arguments.as_ref()
        })
    })
}

fn stale_code_lens_command_status(title: &str) -> String {
    let title = code_lens_title_label_cow(title);
    format!("Code lens `{}` is no longer available", title.as_ref())
}

fn lsp_code_lens_command_status(title: &str) -> String {
    let title = code_lens_title_label_cow(title);
    format!("Running code lens `{}`", title.as_ref())
}

fn code_lens_title_label_cow(title: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(title, CODE_LENS_REQUEST_TITLE_MAX_CHARS, "code lens")
}

fn code_lens_command_label_cow(command: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(command, CODE_LENS_REQUEST_COMMAND_MAX_CHARS, "command")
}

fn lsp_code_lens_command_trace_detail(path: &Path, command: &str) -> String {
    let path = display_path_label_cow(path);
    let command = code_lens_command_label_cow(command);
    format!("{} `{}`", path.as_ref(), command.as_ref())
}

#[cfg(test)]
mod tests {
    use super::{
        CODE_LENS_REQUEST_COMMAND_MAX_CHARS, CODE_LENS_REQUEST_TITLE_MAX_CHARS,
        code_lens_command_label_cow, code_lens_title_label_cow, current_code_lens_command_matches,
        lsp_code_lens_command_status, lsp_code_lens_command_trace_detail,
        stale_code_lens_command_status,
    };
    use crate::{
        KuroyaApp, app_startup_context::AppStartupContext,
        editor_pane_actions::PendingCodeLensCommand, path_display::display_path_label_cow,
        terminal::TerminalPane,
    };
    use kuroya_core::{EditorSettings, LspCodeLens, TextBuffer, Workspace};
    use serde_json::json;
    use std::{
        borrow::Cow,
        path::{Path, PathBuf},
        sync::Arc,
        time::Instant,
    };
    use tokio::runtime::Runtime;

    #[test]
    fn code_lens_command_status_sanitizes_and_bounds_provider_title() {
        let title = format!("Run\n{}\u{202e}", "very-long-".repeat(32));

        let status = lsp_code_lens_command_status(&title);

        assert!(!status.contains('\n'));
        assert!(!status.contains('\u{202e}'));
        assert!(status.contains("..."));
        assert!(
            status.chars().count()
                <= "Running code lens ``".chars().count() + CODE_LENS_REQUEST_TITLE_MAX_CHARS
        );
    }

    #[test]
    fn code_lens_title_label_cow_borrows_clean_ascii_and_unicode_labels() {
        assert_eq!(
            code_lens_title_label_cow("Run Test"),
            Cow::Borrowed("Run Test")
        );

        let unicode = "Run \u{30c6}\u{30b9}\u{30c8}";
        match code_lens_title_label_cow(unicode) {
            Cow::Borrowed(label) => assert_eq!(label, unicode),
            Cow::Owned(label) => panic!("expected borrowed title label, got {label:?}"),
        }
    }

    #[test]
    fn code_lens_command_label_cow_borrows_clean_ascii_and_unicode_labels() {
        assert_eq!(
            code_lens_command_label_cow("rust-analyzer.runSingle"),
            Cow::Borrowed("rust-analyzer.runSingle")
        );

        let unicode = "rust-analyzer.\u{5b9f}\u{884c}";
        match code_lens_command_label_cow(unicode) {
            Cow::Borrowed(label) => assert_eq!(label, unicode),
            Cow::Owned(label) => panic!("expected borrowed command label, got {label:?}"),
        }
    }

    #[test]
    fn code_lens_title_label_cow_owns_dirty_truncated_and_fallback_labels() {
        let dirty = code_lens_title_label_cow("Run\n\u{202e}Target");
        assert_eq!(dirty.as_ref(), "Run Target");
        assert!(matches!(dirty, Cow::Owned(_)));

        let long = "title-".repeat(CODE_LENS_REQUEST_TITLE_MAX_CHARS);
        let truncated = code_lens_title_label_cow(&long);
        assert!(truncated.as_ref().contains("..."));
        assert!(truncated.as_ref().chars().count() <= CODE_LENS_REQUEST_TITLE_MAX_CHARS);
        assert!(matches!(truncated, Cow::Owned(_)));

        let fallback = code_lens_title_label_cow("\n\t\u{202e}");
        assert_eq!(fallback.as_ref(), "code lens");
        assert!(matches!(fallback, Cow::Owned(_)));
    }

    #[test]
    fn code_lens_command_label_cow_owns_dirty_truncated_and_fallback_labels() {
        let dirty = code_lens_command_label_cow("cmd\n\u{202e}run");
        assert_eq!(dirty.as_ref(), "cmd run");
        assert!(matches!(dirty, Cow::Owned(_)));

        let long = "command-".repeat(CODE_LENS_REQUEST_COMMAND_MAX_CHARS);
        let truncated = code_lens_command_label_cow(&long);
        assert!(truncated.as_ref().contains("..."));
        assert!(truncated.as_ref().chars().count() <= CODE_LENS_REQUEST_COMMAND_MAX_CHARS);
        assert!(matches!(truncated, Cow::Owned(_)));

        let fallback = code_lens_command_label_cow("\n\t\u{202e}");
        assert_eq!(fallback.as_ref(), "command");
        assert!(matches!(fallback, Cow::Owned(_)));
    }

    #[test]
    fn code_lens_trace_detail_sanitizes_path_and_command_without_rewriting_inputs() {
        let path = Path::new("workspace/src").join("lens\n\u{202e}.rs");
        let command = format!("cmd\n{}\u{202e}", "very-long-".repeat(32));

        let detail = lsp_code_lens_command_trace_detail(&path, &command);

        let path_label = display_path_label_cow(&path);
        assert!(detail.starts_with(path_label.as_ref()));
        assert!(!detail.contains('\n'));
        assert!(!detail.contains('\u{202e}'));
        assert!(detail.contains("..."));
        assert!(command.contains('\n'));
        assert!(command.contains('\u{202e}'));
        assert!(
            detail.chars().count()
                <= path_label.chars().count()
                    + " ``".chars().count()
                    + CODE_LENS_REQUEST_COMMAND_MAX_CHARS
        );
    }

    #[test]
    fn code_lens_status_sanitizes_title_without_rewriting_input() {
        let title = "Run\n\u{202e}Target".to_owned();

        let status = lsp_code_lens_command_status(&title);
        let stale = stale_code_lens_command_status(&title);

        assert_eq!(status, "Running code lens `Run Target`");
        assert_eq!(stale, "Code lens `Run Target` is no longer available");
        assert_eq!(title, "Run\n\u{202e}Target");
    }

    #[test]
    fn code_lens_command_identity_requires_live_raw_title_command_and_arguments() {
        let raw_title = "Run\n\u{202e}Target".to_owned();
        let raw_command = "rust-analyzer.run\n\u{202e}Single".to_owned();
        let arguments = Arc::new(json!({
            "label": raw_title.clone(),
            "command": raw_command.clone(),
        }));
        let pending = PendingCodeLensCommand {
            title: raw_title.clone(),
            command: raw_command.clone(),
            arguments: Some(arguments.clone()),
        };
        let lenses = vec![LspCodeLens {
            line: 1,
            column: 1,
            title: raw_title.clone(),
            command: Some(raw_command.clone()),
            command_arguments: Some(arguments.clone()),
            resolve_payload: Some(Arc::new(json!({ "raw": "payload\n\u{202e}" }))),
        }];

        assert!(current_code_lens_command_matches(Some(&lenses), &pending));
        assert!(!current_code_lens_command_matches(
            Some(&lenses),
            &PendingCodeLensCommand {
                title: "Run Target".to_owned(),
                ..pending.clone()
            }
        ));
        assert!(!current_code_lens_command_matches(
            Some(&lenses),
            &PendingCodeLensCommand {
                arguments: Some(Arc::new(json!({"label": "changed"}))),
                ..pending
            }
        ));
        assert!(raw_title.contains('\n'));
        assert!(raw_command.contains('\n'));
    }

    #[test]
    fn execute_code_lens_command_rejects_stale_pending_command_before_lsp_queue() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        app.buffers.push(TextBuffer::from_text(
            7,
            Some(path.clone()),
            "fn main() {}\n".to_owned(),
        ));
        app.code_lenses.insert(
            path,
            vec![LspCodeLens {
                line: 1,
                column: 1,
                title: "Run Other".to_owned(),
                command: Some("rust-analyzer.runOther".to_owned()),
                command_arguments: None,
                resolve_payload: None,
            }],
        );

        app.execute_code_lens_command(
            7,
            PendingCodeLensCommand {
                title: "Run\n\u{202e}Target".to_owned(),
                command: "rust-analyzer.runSingle".to_owned(),
                arguments: None,
            },
        );

        assert_eq!(app.status, "Code lens `Run Target` is no longer available");
    }

    #[test]
    fn stale_code_lens_command_status_sanitizes_and_bounds_provider_title() {
        let title = format!("Run\n{}\u{202e}", "very-long-".repeat(32));

        let status = stale_code_lens_command_status(&title);

        assert!(!status.contains('\n'));
        assert!(!status.contains('\u{202e}'));
        assert!(status.contains("..."));
        assert!(
            status.chars().count()
                <= "Code lens `` is no longer available".chars().count()
                    + CODE_LENS_REQUEST_TITLE_MAX_CHARS
        );
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
