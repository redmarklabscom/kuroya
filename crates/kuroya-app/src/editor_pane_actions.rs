use crate::{
    KuroyaApp, editor_input::EditorContextAction, path_display::sanitized_display_label_cow,
    workspace_state::PaneId,
};
use eframe::egui::{Context, OpenUrl};
use kuroya_core::{BufferId, clamp_editor_multi_cursor_limit};
use serde_json::Value;
use std::{borrow::Cow, ops::Range, sync::Arc};

const EDITOR_OPEN_URL_LABEL_MAX_CHARS: usize = 160;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PendingCodeLensCommand {
    pub(crate) title: String,
    pub(crate) command: String,
    pub(crate) arguments: Option<Arc<Value>>,
}

#[derive(Default)]
pub(crate) struct PendingEditorPaneActions {
    pub(crate) focus_editor: bool,
    pub(crate) minimap_jump: Option<usize>,
    pub(crate) fold_toggle_line: Option<usize>,
    pub(crate) select_line: Option<usize>,
    pub(crate) select_range: Option<Range<usize>>,
    pub(crate) context_action: Option<EditorContextAction>,
    pub(crate) code_lens_command: Option<PendingCodeLensCommand>,
    pub(crate) cursor: Option<(usize, bool, bool)>,
    pub(crate) open_url: Option<String>,
    pub(crate) hover_char_idx: Option<usize>,
    pub(crate) drag_start_char_idx: Option<usize>,
    pub(crate) drag_drop_char_idx: Option<usize>,
    pub(crate) selection_clipboard_paste_char_idx: Option<usize>,
}

impl KuroyaApp {
    pub(crate) fn apply_editor_pane_actions(
        &mut self,
        ctx: &Context,
        pane_id: PaneId,
        active_id: BufferId,
        actions: PendingEditorPaneActions,
    ) {
        let target_available = self.editor_pane_action_target_available(pane_id, active_id);
        self.update_editor_lsp_hover_target(
            ctx,
            pane_id,
            active_id,
            target_available.then_some(actions.hover_char_idx).flatten(),
        );
        if !target_available {
            return;
        }

        if actions.focus_editor {
            self.focus_editor_pane(pane_id, active_id);
        }

        if let Some(line) = actions.minimap_jump {
            if self.focus_editor_pane(pane_id, active_id) {
                self.pending_scroll_lines.insert(active_id, line);
            }
        }

        if let Some(line) = actions.fold_toggle_line {
            if self.focus_editor_pane(pane_id, active_id) {
                self.toggle_fold_at_line(active_id, line);
            }
        }

        if let Some(line_idx) = actions.select_line {
            if self.focus_editor_pane(pane_id, active_id)
                && let Some(buffer) = self.buffer_mut(active_id)
            {
                let cursor = buffer.line_column_to_char(line_idx, 0);
                buffer.set_single_cursor(cursor);
                if buffer.select_lines() {
                    self.status = "Selected line".to_owned();
                }
            }
        }

        if let Some(range) = actions.select_range {
            if self.focus_editor_pane(pane_id, active_id)
                && let Some(buffer) = self.buffer_mut(active_id)
            {
                buffer.set_selection(range.start, range.end);
                self.status = "Selected bracket block".to_owned();
            }
        }

        if let Some(command) = actions.code_lens_command {
            if self.focus_editor_pane(pane_id, active_id) {
                self.execute_code_lens_command(active_id, command);
            }
        }

        if let Some((cursor, add_cursor, extend_selection)) = actions.cursor {
            let multi_cursor_limit =
                clamp_editor_multi_cursor_limit(self.settings.multi_cursor_limit).max(1);
            let mut selection_limit_reached = false;
            if self.focus_editor_pane(pane_id, active_id)
                && let Some(buffer) = self.buffer_mut(active_id)
            {
                if add_cursor {
                    if !buffer.add_cursor_with_limit(cursor, multi_cursor_limit)
                        && buffer.selections().len() >= multi_cursor_limit
                    {
                        selection_limit_reached = true;
                    }
                } else if extend_selection {
                    buffer.set_selection(buffer.cursor(), cursor);
                } else {
                    buffer.set_single_cursor(cursor);
                }
            }
            if selection_limit_reached {
                self.status = format!("Selection limit reached ({multi_cursor_limit})");
            }
        }

        if let Some(url) = actions.open_url {
            ctx.open_url(OpenUrl::new_tab(url.clone()));
            self.status = editor_open_url_status(&url);
        }

        if let Some(char_idx) = actions.drag_start_char_idx {
            if self.editor_pane_action_target_available(pane_id, active_id) {
                self.start_editor_selection_drag(pane_id, active_id, char_idx);
            }
        }

        if let Some(char_idx) = actions.drag_drop_char_idx {
            if self.editor_pane_action_target_available(pane_id, active_id) {
                self.finish_editor_selection_drag(pane_id, active_id, char_idx);
            }
        }

        if let Some(char_idx) = actions.selection_clipboard_paste_char_idx {
            if self.focus_editor_pane(pane_id, active_id) {
                self.paste_editor_selection_clipboard_at(active_id, char_idx);
            }
        }

        if let Some(action) = actions.context_action {
            if self.focus_editor_pane(pane_id, active_id) {
                self.run_editor_context_action(ctx, active_id, action);
            }
        }
    }

    fn focus_editor_pane(&mut self, pane_id: PaneId, active_id: BufferId) -> bool {
        if !self.editor_pane_action_target_available(pane_id, active_id) {
            return false;
        }
        self.focused_pane = Some(pane_id);
        self.active_pane = pane_id;
        self.set_active_buffer(active_id);
        true
    }

    fn editor_pane_action_target_available(&self, pane_id: PaneId, active_id: BufferId) -> bool {
        self.buffer(active_id).is_some()
            && self
                .panes
                .iter()
                .any(|pane| pane.id == pane_id && pane.active == Some(active_id))
    }
}

fn editor_open_url_status(url: &str) -> String {
    format!("Opening {}", editor_open_url_label_cow(url))
}

fn editor_open_url_label_cow(url: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(url, EDITOR_OPEN_URL_LABEL_MAX_CHARS, "link")
}

#[cfg(test)]
mod tests {
    use super::{
        EDITOR_OPEN_URL_LABEL_MAX_CHARS, PendingEditorPaneActions, editor_open_url_label_cow,
        editor_open_url_status,
    };
    use crate::{KuroyaApp, app_startup_context::AppStartupContext, terminal::TerminalPane};
    use eframe::egui::Context;
    use kuroya_core::{EditorSettings, TextBuffer, Workspace};
    use std::{
        borrow::Cow,
        path::PathBuf,
        time::{Duration, Instant},
    };
    use tokio::runtime::Runtime;

    #[test]
    fn editor_open_url_label_cow_borrows_clean_ascii_and_unicode() {
        assert!(matches!(
            editor_open_url_label_cow("https://example.test/docs?q=rust"),
            Cow::Borrowed("https://example.test/docs?q=rust")
        ));

        let unicode = "https://example.test/\u{03bb}/\u{6771}\u{4eac}";
        match editor_open_url_label_cow(unicode) {
            Cow::Borrowed(label) => assert_eq!(label, unicode),
            Cow::Owned(label) => panic!("expected borrowed label, got {label:?}"),
        }
    }

    #[test]
    fn editor_open_url_label_cow_owns_dirty_truncated_and_fallback_output() {
        match editor_open_url_label_cow("https://example.test/a\nb\u{202e}c") {
            Cow::Owned(label) => {
                assert_eq!(label, "https://example.test/a bc");
                assert!(!label.contains('\n'));
                assert!(!label.contains('\u{202e}'));
            }
            Cow::Borrowed(label) => panic!("expected owned dirty label, got {label:?}"),
        }

        match editor_open_url_label_cow(&format!(
            "https://example.test/{}",
            "very-long-url-segment-".repeat(16)
        )) {
            Cow::Owned(label) => {
                assert!(label.starts_with("https://example.test/very-long-url-segment-"));
                assert!(label.contains("..."));
                assert!(label.chars().count() <= EDITOR_OPEN_URL_LABEL_MAX_CHARS);
            }
            Cow::Borrowed(label) => panic!("expected owned truncated label, got {label:?}"),
        }

        match editor_open_url_label_cow("\n\u{202e}\u{0000}") {
            Cow::Owned(label) => assert_eq!(label, "link"),
            Cow::Borrowed(label) => panic!("expected owned fallback label, got {label:?}"),
        }
    }

    #[test]
    fn editor_open_url_status_formats_from_label_cow() {
        let url = format!(
            "https://example.test/a\n{}\u{202e}",
            "very-long-url-segment-".repeat(16)
        );
        let label = editor_open_url_label_cow(&url);
        let status = editor_open_url_status(&url);

        assert_eq!(status, format!("Opening {}", label));
        assert!(status.starts_with("Opening https://example.test/a "));
        assert!(!status.contains('\n'));
        assert!(!status.contains('\u{202e}'));
        assert!(status.contains("..."));
        assert!(status.chars().count() <= "Opening ".len() + EDITOR_OPEN_URL_LABEL_MAX_CHARS);
    }

    #[test]
    fn editor_open_url_status_preserves_fallback_wording() {
        assert_eq!(editor_open_url_status("\n\u{202e}\u{0000}"), "Opening link");
    }

    #[test]
    fn pointer_added_cursor_respects_multi_cursor_limit() {
        let root = PathBuf::from("workspace");
        let mut app = app_for_test(root);
        app.settings.multi_cursor_limit = 2;
        let mut buffer = TextBuffer::from_text(1, None, "abcd".to_owned());
        buffer.set_cursors([0, 2]);
        app.buffers.push(buffer);
        app.panes[0].active = Some(1);

        app.apply_editor_pane_actions(
            &Context::default(),
            1,
            1,
            PendingEditorPaneActions {
                cursor: Some((3, true, false)),
                ..PendingEditorPaneActions::default()
            },
        );

        assert_eq!(
            app.buffer(1)
                .unwrap()
                .cursor_positions()
                .into_iter()
                .map(|pos| pos.char_idx)
                .collect::<Vec<_>>(),
            vec![0, 2]
        );
        assert_eq!(app.status, "Selection limit reached (2)");
    }

    #[test]
    fn editor_pane_actions_ignore_stale_buffer_target() {
        let root = PathBuf::from("workspace");
        let mut app = app_for_test(root);
        app.buffers
            .push(TextBuffer::from_text(1, None, "one".to_owned()));
        app.buffers
            .push(TextBuffer::from_text(2, None, "two".to_owned()));
        app.panes[0].active = Some(1);
        app.active = Some(1);
        app.active_pane = 1;
        app.focused_pane = Some(1);
        app.status = "ready".to_owned();

        app.apply_editor_pane_actions(
            &Context::default(),
            1,
            2,
            PendingEditorPaneActions {
                focus_editor: true,
                minimap_jump: Some(3),
                open_url: Some("https://example.test/stale".to_owned()),
                ..PendingEditorPaneActions::default()
            },
        );

        assert_eq!(app.active, Some(1));
        assert_eq!(app.active_pane, 1);
        assert_eq!(app.focused_pane, Some(1));
        assert_eq!(app.panes[0].active, Some(1));
        assert!(!app.pending_scroll_lines.contains_key(&2));
        assert_eq!(app.status, "ready");
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
            now: Instant::now() - Duration::from_secs(1),
            startup_timings: Vec::new(),
        })
    }
}
