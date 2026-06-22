use kuroya_core::TextBuffer;

pub(in crate::editor_vim_key_events) fn vim_char_at(
    buffer: &TextBuffer,
    char_idx: usize,
) -> Option<char> {
    buffer.char_at(char_idx)
}
