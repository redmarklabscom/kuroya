use kuroya_core::TextBuffer;

use super::vim_char_at;

pub(in crate::editor_vim_key_events) fn vim_move_previous_big_word_end(buffer: &mut TextBuffer) {
    let target = vim_previous_big_word_end_char(buffer, buffer.cursor());
    buffer.set_single_cursor(target);
}

fn vim_previous_big_word_end_char(buffer: &TextBuffer, cursor: usize) -> usize {
    let len = buffer.len_chars();
    let idx = cursor.min(len);
    if idx == 0 {
        return 0;
    }

    let mut probe = idx - 1;
    if idx < len && vim_char_at(buffer, idx).is_some_and(|ch| !ch.is_whitespace()) {
        while probe > 0 && vim_char_at(buffer, probe).is_some_and(|ch| !ch.is_whitespace()) {
            probe -= 1;
        }
        if vim_char_at(buffer, probe).is_some_and(|ch| !ch.is_whitespace()) {
            return 0;
        }
    }

    while probe > 0 && vim_char_at(buffer, probe).is_some_and(char::is_whitespace) {
        probe -= 1;
    }
    if vim_char_at(buffer, probe).is_some_and(char::is_whitespace) {
        0
    } else {
        probe
    }
}
