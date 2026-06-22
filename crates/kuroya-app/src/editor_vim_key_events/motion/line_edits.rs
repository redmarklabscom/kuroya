use kuroya_core::TextBuffer;

use super::super::vim_line_range_for_count;

pub(in crate::editor_vim_key_events) fn vim_open_line_below(buffer: &mut TextBuffer) {
    let line = buffer.cursor_position().line;
    let indent = vim_line_indent(buffer, line);
    buffer.move_line_end();
    buffer.insert_at_cursors(&vim_open_line_below_text(&indent));
}

pub(in crate::editor_vim_key_events) fn vim_open_line_above(buffer: &mut TextBuffer) {
    let line = buffer.cursor_position().line;
    let line_start = buffer.line_column_to_char(line, 0);
    let indent = vim_line_indent(buffer, line);
    let cursor = line_start.saturating_add(indent.chars().count());
    buffer.set_single_cursor(line_start);
    buffer.insert_at_cursors(&vim_open_line_above_text(&indent));
    buffer.set_single_cursor(cursor);
}

pub(in crate::editor_vim_key_events) fn vim_open_line_below_text(indent: &str) -> String {
    let mut text = String::with_capacity(1 + indent.len());
    text.push('\n');
    text.push_str(indent);
    text
}

pub(in crate::editor_vim_key_events) fn vim_open_line_above_text(indent: &str) -> String {
    let mut text = String::with_capacity(indent.len() + 1);
    text.push_str(indent);
    text.push('\n');
    text
}

fn vim_line_indent(buffer: &TextBuffer, line: usize) -> String {
    buffer
        .line(line)
        .unwrap_or_default()
        .chars()
        .take_while(|ch| matches!(ch, ' ' | '\t'))
        .collect()
}

pub(in crate::editor_vim_key_events) fn vim_indent_lines(
    buffer: &mut TextBuffer,
    count: usize,
    indent_unit: &str,
) -> bool {
    if indent_unit.is_empty() {
        return false;
    }
    let position = buffer.cursor_position();
    let Some(range) = vim_line_range_for_count(buffer, count) else {
        return false;
    };

    buffer.set_selection(range.start, range.end);
    let changed = buffer.indent_lines(indent_unit);
    if changed {
        let column = position.column.saturating_add(indent_unit.chars().count());
        buffer.set_single_cursor(buffer.line_column_to_char(position.line, column));
    }
    changed
}

pub(in crate::editor_vim_key_events) fn vim_outdent_lines(
    buffer: &mut TextBuffer,
    count: usize,
    indent_unit: &str,
) -> bool {
    let position = buffer.cursor_position();
    let remove_len = vim_line_outdent_len(buffer, position.line, indent_unit);
    let Some(range) = vim_line_range_for_count(buffer, count) else {
        return false;
    };

    buffer.set_selection(range.start, range.end);
    let changed = buffer.outdent_lines(indent_unit);
    if changed {
        let column = position.column.saturating_sub(remove_len);
        buffer.set_single_cursor(buffer.line_column_to_char(position.line, column));
    }
    changed
}

pub(in crate::editor_vim_key_events) fn vim_line_outdent_len(
    buffer: &TextBuffer,
    line: usize,
    indent_unit: &str,
) -> usize {
    let indent_width = indent_unit.chars().count().max(1);
    let Some(text) = buffer.line(line) else {
        return 0;
    };
    let mut chars = text.chars();
    if matches!(chars.next(), Some('\t')) {
        return 1;
    }

    text.chars()
        .take_while(|ch| *ch == ' ')
        .take(indent_width)
        .count()
}
