use crate::{
    KuroyaApp,
    command_ui_runtime::find_query_seed_from_selection,
    editor_input::EditorContextAction,
    path_clipboard::{PathCopyKind, copy_path_to_clipboard},
};
use eframe::egui::Context;
use kuroya_core::{BufferId, EditorFindSeedSearchStringFromSelection, TextBuffer};

pub(crate) fn editor_clipboard_text(
    buffer: &TextBuffer,
    empty_selection_clipboard: bool,
) -> Option<String> {
    if empty_selection_clipboard {
        buffer.selected_text_or_lines()
    } else {
        buffer.selected_text()
    }
}

struct EditorClipboardPayload {
    text: String,
    has_selection: bool,
}

fn editor_clipboard_payload(
    buffer: &TextBuffer,
    empty_selection_clipboard: bool,
) -> Option<EditorClipboardPayload> {
    if empty_selection_clipboard {
        let has_selection = buffer.has_selection();
        let text = if has_selection {
            buffer.selected_text()
        } else {
            buffer.selected_text_or_lines()
        };
        text.map(|text| EditorClipboardPayload {
            text,
            has_selection,
        })
    } else {
        buffer.selected_text().map(|text| EditorClipboardPayload {
            text,
            has_selection: true,
        })
    }
}

#[cfg(test)]
pub(crate) fn delete_editor_cut_target(
    buffer: &mut TextBuffer,
    empty_selection_clipboard: bool,
) -> bool {
    if buffer.has_selection() || empty_selection_clipboard {
        buffer.delete_selection_or_lines()
    } else {
        false
    }
}

impl KuroyaApp {
    pub(crate) fn run_editor_clipboard_context_action(
        &mut self,
        ctx: &Context,
        buffer_id: BufferId,
        action: EditorContextAction,
    ) -> bool {
        match action {
            EditorContextAction::Copy => {
                if let Some(payload) = self.buffer(buffer_id).and_then(|buffer| {
                    editor_clipboard_payload(buffer, self.settings.empty_selection_clipboard)
                }) {
                    ctx.copy_text(payload.text);
                    self.status =
                        clipboard_status("Copied selection", "Copied line", payload.has_selection);
                } else {
                    self.status = "No text to copy".to_owned();
                }
            }
            EditorContextAction::Cut => {
                let mut copied = false;
                let mut changed = false;
                let mut cut_selection = false;
                let empty_selection_clipboard = self.settings.empty_selection_clipboard;
                if let Some(buffer) = self.buffer_mut(buffer_id)
                    && let Some(payload) =
                        editor_clipboard_payload(buffer, empty_selection_clipboard)
                {
                    ctx.copy_text(payload.text);
                    copied = true;
                    cut_selection = payload.has_selection;
                    changed = buffer.delete_selection_or_lines();
                }
                if changed {
                    self.mark_buffer_changed(buffer_id);
                    self.status = clipboard_status("Cut selection", "Cut line", cut_selection);
                } else if copied {
                    self.status =
                        clipboard_status("Copied selection", "Copied line", cut_selection);
                } else {
                    self.status = "No text to cut".to_owned();
                }
            }
            EditorContextAction::FindSelection => {
                if let Some(selected) = find_query_seed_from_selection(
                    self.buffer(buffer_id),
                    EditorFindSeedSearchStringFromSelection::Selection,
                ) {
                    self.buffer_find_query = selected;
                    self.buffer_find_query_history_cursor = None;
                    self.buffer_find_query_history_draft = None;
                }
                self.buffer_find_open = true;
                self.buffer_find_match = 0;
                self.buffer_find_scope = None;
                self.select_find_match();
            }
            EditorContextAction::CopyDiffPatch => {
                self.copy_diff_buffer_patch(ctx, buffer_id);
            }
            EditorContextAction::CopyDiffHunkPatch => {
                self.copy_diff_buffer_hunk_patch(ctx, buffer_id);
            }
            EditorContextAction::CopyActivePath => {
                if let Some(path) = self.buffer_file_or_diff_source_path(buffer_id) {
                    self.status = copy_path_to_clipboard(
                        ctx,
                        &self.workspace.root,
                        &path,
                        PathCopyKind::Absolute,
                    );
                } else {
                    self.status = "No file-backed buffer to copy path".to_owned();
                }
            }
            EditorContextAction::CopyActiveRelativePath => {
                if let Some(path) = self.buffer_file_or_diff_source_path(buffer_id) {
                    self.status = copy_path_to_clipboard(
                        ctx,
                        &self.workspace.root,
                        &path,
                        PathCopyKind::Relative,
                    );
                } else {
                    self.status = "No file-backed buffer to copy relative path".to_owned();
                }
            }
            _ => return false,
        }
        true
    }
}

fn clipboard_status(
    selection_status: &'static str,
    line_status: &'static str,
    has_selection: bool,
) -> String {
    if has_selection {
        selection_status
    } else {
        line_status
    }
    .to_owned()
}

#[cfg(test)]
mod tests {
    use super::{delete_editor_cut_target, editor_clipboard_text};
    use kuroya_core::TextBuffer;

    #[test]
    fn editor_clipboard_text_can_require_an_explicit_selection() {
        let mut buffer = TextBuffer::from_text(1, None, "one\ntwo\n".to_owned());
        buffer.set_single_cursor(buffer.line_column_to_char(1, 0));

        assert_eq!(
            editor_clipboard_text(&buffer, true).as_deref(),
            Some("two\n")
        );
        assert_eq!(editor_clipboard_text(&buffer, false), None);
    }

    #[test]
    fn editor_cut_target_respects_empty_selection_clipboard() {
        let mut disabled = TextBuffer::from_text(1, None, "one\ntwo\n".to_owned());
        disabled.set_single_cursor(disabled.line_column_to_char(1, 0));
        assert!(!delete_editor_cut_target(&mut disabled, false));
        assert_eq!(disabled.text(), "one\ntwo\n");

        let mut enabled = TextBuffer::from_text(1, None, "one\ntwo\n".to_owned());
        enabled.set_single_cursor(enabled.line_column_to_char(1, 0));
        assert!(delete_editor_cut_target(&mut enabled, true));
        assert_eq!(enabled.text(), "one\n");
    }
}
