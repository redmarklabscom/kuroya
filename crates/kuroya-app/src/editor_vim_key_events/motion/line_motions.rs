use kuroya_core::TextBuffer;

use super::super::VIM_MAX_COUNT;

pub(in crate::editor_vim_key_events) fn vim_line_first_non_whitespace_char(
    buffer: &TextBuffer,
    line: usize,
) -> usize {
    let line_start = buffer.line_column_to_char(line, 0);
    let Some(text) = buffer.line(line) else {
        return line_start;
    };
    let column = text
        .chars()
        .take_while(|ch| matches!(ch, ' ' | '\t'))
        .count();
    buffer.line_column_to_char(line, column)
}

pub(in crate::editor_vim_key_events) fn vim_move_to_line_column(
    buffer: &mut TextBuffer,
    count: usize,
) {
    let target = vim_line_column_motion_char(buffer, count);
    buffer.set_single_cursor(target);
}

pub(in crate::editor_vim_key_events) fn vim_line_column_motion_char(
    buffer: &TextBuffer,
    count: usize,
) -> usize {
    let line = buffer.cursor_position().line;
    let column = count.clamp(1, VIM_MAX_COUNT).saturating_sub(1);
    buffer.line_column_to_char(line, column)
}

pub(in crate::editor_vim_key_events) fn vim_move_to_line_end(
    buffer: &mut TextBuffer,
    count: usize,
) {
    let count = count.clamp(1, VIM_MAX_COUNT);
    let line = buffer
        .cursor_position()
        .line
        .saturating_add(count.saturating_sub(1))
        .min(buffer.len_lines().saturating_sub(1));
    buffer.set_single_cursor(buffer.line_content_end_char(line));
}

pub(in crate::editor_vim_key_events) fn vim_move_space_forward(
    buffer: &mut TextBuffer,
    count: usize,
) {
    for _ in 0..count.clamp(1, VIM_MAX_COUNT) {
        let position = buffer.cursor_position();
        let line_start = buffer.line_column_to_char(position.line, 0);
        let line_end = buffer.line_content_end_char(position.line);
        let last_content_char = line_end.saturating_sub(1).max(line_start);

        if buffer.cursor() >= last_content_char && position.line + 1 < buffer.len_lines() {
            buffer.set_single_cursor(buffer.line_column_to_char(position.line + 1, 0));
        } else if buffer.cursor() < line_end {
            buffer.move_right();
        }
    }
}

pub(in crate::editor_vim_key_events) fn vim_move_space_backward(
    buffer: &mut TextBuffer,
    count: usize,
) {
    for _ in 0..count.clamp(1, VIM_MAX_COUNT) {
        let position = buffer.cursor_position();
        let line_start = buffer.line_column_to_char(position.line, 0);

        if buffer.cursor() <= line_start && position.line > 0 {
            let previous_line = position.line - 1;
            let previous_start = buffer.line_column_to_char(previous_line, 0);
            let previous_end = buffer.line_content_end_char(previous_line);
            buffer.set_single_cursor(previous_end.saturating_sub(1).max(previous_start));
        } else if buffer.cursor() > line_start {
            buffer.move_left();
        }
    }
}

pub(in crate::editor_vim_key_events) fn vim_move_next_line_first_non_whitespace(
    buffer: &mut TextBuffer,
    count: usize,
) {
    let line = buffer
        .cursor_position()
        .line
        .saturating_add(count.clamp(1, VIM_MAX_COUNT))
        .min(buffer.len_lines().saturating_sub(1));
    buffer.set_single_cursor(vim_line_first_non_whitespace_char(buffer, line));
}

pub(in crate::editor_vim_key_events) fn vim_move_previous_line_first_non_whitespace(
    buffer: &mut TextBuffer,
    count: usize,
) {
    let line = buffer
        .cursor_position()
        .line
        .saturating_sub(count.clamp(1, VIM_MAX_COUNT));
    buffer.set_single_cursor(vim_line_first_non_whitespace_char(buffer, line));
}

pub(in crate::editor_vim_key_events) fn vim_move_counted_line_first_non_whitespace(
    buffer: &mut TextBuffer,
    count: usize,
) {
    let line = buffer
        .cursor_position()
        .line
        .saturating_add(count.clamp(1, VIM_MAX_COUNT).saturating_sub(1))
        .min(buffer.len_lines().saturating_sub(1));
    buffer.set_single_cursor(vim_line_first_non_whitespace_char(buffer, line));
}
