use crate::lsp_text_positions::lsp_one_based_utf16_position_to_buffer_char;
use crate::lsp_text_positions::lsp_one_based_utf16_span_to_buffer_char_range;
use kuroya_core::TextBuffer;

pub(super) fn lsp_position_within_buffer(buffer: &TextBuffer, line: usize, column: usize) -> bool {
    lsp_one_based_utf16_position_to_buffer_char(buffer, line, column).is_some()
}

pub(super) fn lsp_span_within_buffer(
    buffer: &TextBuffer,
    line: usize,
    column: usize,
    length: usize,
) -> bool {
    lsp_one_based_utf16_span_to_buffer_char_range(buffer, line, column, length).is_some()
}

#[cfg(test)]
mod tests {
    use super::{lsp_position_within_buffer, lsp_span_within_buffer};
    use kuroya_core::TextBuffer;

    #[test]
    fn lsp_positions_allow_line_end_but_reject_missing_lines() {
        let buffer = TextBuffer::from_text(1, None, "alpha\nbeta".to_owned());

        assert!(lsp_position_within_buffer(&buffer, 1, 1));
        assert!(lsp_position_within_buffer(&buffer, 1, 6));
        assert!(lsp_position_within_buffer(&buffer, 2, 5));
        assert!(!lsp_position_within_buffer(&buffer, 0, 1));
        assert!(!lsp_position_within_buffer(&buffer, 1, 0));
        assert!(!lsp_position_within_buffer(&buffer, 1, 7));
        assert!(!lsp_position_within_buffer(&buffer, 3, 1));
    }

    #[test]
    fn lsp_positions_use_utf16_columns() {
        let buffer = TextBuffer::from_text(1, None, "😀x".to_owned());

        assert!(lsp_position_within_buffer(&buffer, 1, 1));
        assert!(lsp_position_within_buffer(&buffer, 1, 3));
        assert!(lsp_position_within_buffer(&buffer, 1, 4));
        assert!(!lsp_position_within_buffer(&buffer, 1, 2));
        assert!(!lsp_position_within_buffer(&buffer, 1, 5));
    }

    #[test]
    fn lsp_spans_must_fit_inside_one_line() {
        let buffer = TextBuffer::from_text(1, None, "alpha\nbeta".to_owned());

        assert!(lsp_span_within_buffer(&buffer, 1, 1, 5));
        assert!(lsp_span_within_buffer(&buffer, 2, 2, 3));
        assert!(!lsp_span_within_buffer(&buffer, 1, 1, 6));
        assert!(!lsp_span_within_buffer(&buffer, 1, 6, 1));
        assert!(!lsp_span_within_buffer(&buffer, 2, 2, 0));
    }

    #[test]
    fn lsp_spans_use_utf16_lengths() {
        let buffer = TextBuffer::from_text(1, None, "😀x".to_owned());

        assert!(lsp_span_within_buffer(&buffer, 1, 1, 2));
        assert!(lsp_span_within_buffer(&buffer, 1, 3, 1));
        assert!(!lsp_span_within_buffer(&buffer, 1, 1, 1));
        assert!(!lsp_span_within_buffer(&buffer, 1, 2, 1));
    }
}
