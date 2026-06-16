use crate::{KuroyaApp, editor_input::normalized_editor_paste_text};
use kuroya_core::{BufferId, TextBuffer};

impl KuroyaApp {
    pub(crate) fn refresh_editor_selection_clipboard_from_buffer(&mut self, buffer_index: usize) {
        if !self.settings.selection_clipboard {
            self.editor_selection_clipboard = None;
            return;
        }

        let Some(buffer) = self.buffers.get(buffer_index) else {
            self.editor_selection_clipboard = None;
            return;
        };

        if let Some(text) = editor_selection_clipboard_text(buffer) {
            self.editor_selection_clipboard = Some(text);
        } else if buffer.has_selection() {
            self.editor_selection_clipboard = None;
        }
    }

    pub(crate) fn paste_editor_selection_clipboard_at(
        &mut self,
        buffer_id: BufferId,
        char_idx: usize,
    ) -> bool {
        if !self.settings.selection_clipboard {
            return false;
        }

        let Some(text) = self
            .editor_selection_clipboard
            .as_deref()
            .filter(|text| !text.is_empty())
        else {
            self.status = "Selection clipboard empty".to_owned();
            return false;
        };

        let changed = self
            .buffers
            .iter_mut()
            .find(|buffer| buffer.id() == buffer_id)
            .is_some_and(|buffer| paste_selection_clipboard_text_at(buffer, char_idx, text));
        if changed {
            self.mark_buffer_changed(buffer_id);
            self.status = "Pasted selection clipboard".to_owned();
        }
        changed
    }
}

pub(crate) fn editor_selection_clipboard_text(buffer: &TextBuffer) -> Option<String> {
    buffer
        .selected_text()
        .and_then(|text| normalized_editor_paste_text(&text).map(|text| text.into_owned()))
}

pub(crate) fn paste_selection_clipboard_text_at(
    buffer: &mut TextBuffer,
    char_idx: usize,
    text: &str,
) -> bool {
    let Some(text) = normalized_editor_paste_text(text) else {
        return false;
    };

    buffer.set_single_cursor(char_idx.min(buffer.len_chars()));
    buffer.insert_at_cursors(text.as_ref());
    true
}

#[cfg(test)]
mod tests {
    use super::{editor_selection_clipboard_text, paste_selection_clipboard_text_at};
    use kuroya_core::TextBuffer;

    #[test]
    fn selection_clipboard_text_requires_explicit_selection() {
        let mut buffer = TextBuffer::from_text(1, None, "alpha beta".to_owned());
        assert_eq!(editor_selection_clipboard_text(&buffer), None);

        buffer.set_selection(0, 5);

        assert_eq!(
            editor_selection_clipboard_text(&buffer).as_deref(),
            Some("alpha")
        );
    }

    #[test]
    fn selection_clipboard_text_sanitizes_selected_controls() {
        let mut buffer = TextBuffer::from_text(1, None, "a\u{0}b\tc\u{1b}".to_owned());
        buffer.set_selection(0, buffer.len_chars());

        assert_eq!(
            editor_selection_clipboard_text(&buffer).as_deref(),
            Some("ab\tc")
        );
    }

    #[test]
    fn selection_clipboard_text_rejects_control_only_selection() {
        let mut buffer = TextBuffer::from_text(1, None, "\u{0}\u{1b}\u{7f}".to_owned());
        buffer.set_selection(0, buffer.len_chars());

        assert_eq!(editor_selection_clipboard_text(&buffer), None);
    }

    #[test]
    fn paste_selection_clipboard_text_inserts_at_requested_position() {
        let mut buffer = TextBuffer::from_text(1, None, "alpha beta".to_owned());

        assert!(paste_selection_clipboard_text_at(&mut buffer, 6, "X "));

        assert_eq!(buffer.text(), "alpha X beta");
        assert_eq!(buffer.cursor(), 8);
    }

    #[test]
    fn paste_selection_clipboard_text_sanitizes_inserted_controls() {
        let mut buffer = TextBuffer::from_text(1, None, "alpha".to_owned());

        assert!(paste_selection_clipboard_text_at(
            &mut buffer,
            2,
            "X\u{0}Y\u{1b}"
        ));

        assert_eq!(buffer.text(), "alXYpha");
        assert_eq!(buffer.cursor(), 4);
    }

    #[test]
    fn paste_selection_clipboard_text_rejects_empty_text() {
        let mut buffer = TextBuffer::from_text(1, None, "alpha".to_owned());

        assert!(!paste_selection_clipboard_text_at(&mut buffer, 2, ""));

        assert_eq!(buffer.text(), "alpha");
    }

    #[test]
    fn paste_selection_clipboard_text_rejects_control_only_text() {
        let mut buffer = TextBuffer::from_text(1, None, "alpha".to_owned());

        assert!(!paste_selection_clipboard_text_at(
            &mut buffer,
            2,
            "\u{0}\u{1b}\u{7f}"
        ));

        assert_eq!(buffer.text(), "alpha");
    }
}
