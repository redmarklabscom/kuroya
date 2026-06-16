use kuroya_core::TextBuffer;
use std::ops::Range;

pub(crate) fn buffer_position_to_lsp_utf16_column(
    buffer: &TextBuffer,
    line: usize,
    char_column: usize,
) -> Option<usize> {
    if line >= buffer.len_lines() {
        return None;
    }

    let line_start = buffer.line_column_to_char(line, 0);
    let char_idx = buffer.line_column_to_char(line, char_column);
    let text = buffer.text_range(line_start..char_idx)?;
    char_offset_to_lsp_utf16_column(&text, text.chars().count())
}

pub(crate) fn lsp_line_content_utf16_len(buffer: &TextBuffer, line: usize) -> Option<usize> {
    if line >= buffer.len_lines() {
        return None;
    }

    let line_start = buffer.line_column_to_char(line, 0);
    let line_end = buffer.line_content_end_char(line);
    let text = buffer.text_range(line_start..line_end)?;
    Some(text.chars().map(char::len_utf16).sum())
}

pub(crate) fn lsp_one_based_utf16_position_to_buffer_char(
    buffer: &TextBuffer,
    line_one_based: usize,
    column_one_based: usize,
) -> Option<usize> {
    let line = line_one_based.checked_sub(1)?;
    let utf16_column = column_one_based.checked_sub(1)?;
    if line >= buffer.len_lines() {
        return None;
    }

    let line_start = buffer.line_column_to_char(line, 0);
    let line_end = buffer.line_content_end_char(line);
    let text = buffer.text_range(line_start..line_end)?;
    let column = lsp_utf16_column_to_char_offset(&text, utf16_column)?;
    Some(line_start + column)
}

pub(crate) fn lsp_one_based_utf16_column_to_char_column(
    buffer: &TextBuffer,
    line_one_based: usize,
    column_one_based: usize,
) -> Option<usize> {
    let line = line_one_based.checked_sub(1)?;
    let line_start = buffer.line_column_to_char(line, 0);
    let char_idx =
        lsp_one_based_utf16_position_to_buffer_char(buffer, line_one_based, column_one_based)?;
    Some(char_idx.saturating_sub(line_start))
}

pub(crate) fn lsp_one_based_utf16_span_to_buffer_char_range(
    buffer: &TextBuffer,
    line_one_based: usize,
    column_one_based: usize,
    length_utf16: usize,
) -> Option<Range<usize>> {
    if length_utf16 == 0 {
        return None;
    }

    let end_column = column_one_based.checked_add(length_utf16)?;
    lsp_one_based_utf16_range_to_buffer_char_range(
        buffer,
        line_one_based,
        column_one_based,
        end_column,
    )
}

pub(crate) fn lsp_one_based_utf16_range_to_buffer_char_range(
    buffer: &TextBuffer,
    line_one_based: usize,
    start_column_one_based: usize,
    end_column_one_based: usize,
) -> Option<Range<usize>> {
    let start = lsp_one_based_utf16_position_to_buffer_char(
        buffer,
        line_one_based,
        start_column_one_based,
    )?;
    let end =
        lsp_one_based_utf16_position_to_buffer_char(buffer, line_one_based, end_column_one_based)?;
    (start < end).then_some(start..end)
}

fn lsp_utf16_column_to_char_offset(text: &str, utf16_column: usize) -> Option<usize> {
    let mut remaining = utf16_column;
    let mut char_offset = 0usize;
    for ch in text.chars() {
        if remaining == 0 {
            return Some(char_offset);
        }

        let width = ch.len_utf16();
        if remaining < width {
            return None;
        }
        remaining -= width;
        char_offset += 1;
    }

    (remaining == 0).then_some(char_offset)
}

fn char_offset_to_lsp_utf16_column(text: &str, char_offset: usize) -> Option<usize> {
    let mut current_offset = 0usize;
    let mut utf16_column = 0usize;
    for ch in text.chars() {
        if current_offset == char_offset {
            return Some(utf16_column);
        }
        current_offset += 1;
        utf16_column += ch.len_utf16();
    }

    (current_offset == char_offset).then_some(utf16_column)
}

#[cfg(test)]
mod tests {
    use super::{
        buffer_position_to_lsp_utf16_column, lsp_line_content_utf16_len,
        lsp_one_based_utf16_position_to_buffer_char, lsp_one_based_utf16_span_to_buffer_char_range,
    };
    use kuroya_core::TextBuffer;

    #[test]
    fn buffer_positions_convert_char_columns_to_lsp_utf16_columns() {
        let buffer = TextBuffer::from_text(1, None, "😀alpha".to_owned());

        assert_eq!(buffer_position_to_lsp_utf16_column(&buffer, 0, 0), Some(0));
        assert_eq!(buffer_position_to_lsp_utf16_column(&buffer, 0, 1), Some(2));
        assert_eq!(buffer_position_to_lsp_utf16_column(&buffer, 0, 6), Some(7));
    }

    #[test]
    fn lsp_utf16_positions_convert_to_buffer_chars_and_reject_surrogate_splits() {
        let buffer = TextBuffer::from_text(1, None, "😀alpha".to_owned());

        assert_eq!(
            lsp_one_based_utf16_position_to_buffer_char(&buffer, 1, 1),
            Some(0)
        );
        assert_eq!(
            lsp_one_based_utf16_position_to_buffer_char(&buffer, 1, 3),
            Some(1)
        );
        assert_eq!(
            lsp_one_based_utf16_position_to_buffer_char(&buffer, 1, 8),
            Some(6)
        );
        assert_eq!(
            lsp_one_based_utf16_position_to_buffer_char(&buffer, 1, 2),
            None
        );
    }

    #[test]
    fn lsp_utf16_spans_convert_to_buffer_ranges() {
        let buffer = TextBuffer::from_text(1, None, "😀alpha".to_owned());

        assert_eq!(
            lsp_one_based_utf16_span_to_buffer_char_range(&buffer, 1, 1, 2),
            Some(0..1)
        );
        assert_eq!(
            lsp_one_based_utf16_span_to_buffer_char_range(&buffer, 1, 3, 5),
            Some(1..6)
        );
        assert_eq!(
            lsp_one_based_utf16_span_to_buffer_char_range(&buffer, 1, 1, 1),
            None
        );
    }

    #[test]
    fn line_content_utf16_len_counts_surrogate_pairs() {
        let buffer = TextBuffer::from_text(1, None, "😀x\nbeta".to_owned());

        assert_eq!(lsp_line_content_utf16_len(&buffer, 0), Some(3));
        assert_eq!(lsp_line_content_utf16_len(&buffer, 1), Some(4));
    }
}
