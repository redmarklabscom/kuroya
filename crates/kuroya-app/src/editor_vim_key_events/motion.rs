use eframe::egui::{Key, Modifiers};
use kuroya_core::{TextBuffer, TextEdit};
use std::ops::Range;

use super::{
    EditorVimCaseConversion, EditorVimCharFindMotion, EditorVimNamedRegister, EditorVimRegister,
    EditorVimRegisterKind, VIM_DEFAULT_CTRL_SCROLL_LINES, VIM_DEFAULT_PAGE_SCROLL_LINES,
    VIM_MAX_COUNT, vim_delete_range_into_register, vim_line_range_for_count,
};
pub(super) fn vim_line_first_non_whitespace_char(buffer: &TextBuffer, line: usize) -> usize {
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

pub(super) fn vim_move_to_matching_bracket(buffer: &mut TextBuffer) -> bool {
    let Some((_, target)) = vim_matching_bracket_pair(buffer) else {
        return false;
    };
    buffer.set_single_cursor(target);
    true
}

pub(super) fn vim_move_to_line_column(buffer: &mut TextBuffer, count: usize) {
    let target = vim_line_column_motion_char(buffer, count);
    buffer.set_single_cursor(target);
}

pub(super) fn vim_line_column_motion_char(buffer: &TextBuffer, count: usize) -> usize {
    let line = buffer.cursor_position().line;
    let column = count.clamp(1, VIM_MAX_COUNT).saturating_sub(1);
    buffer.line_column_to_char(line, column)
}

pub(super) fn vim_matching_bracket_range(buffer: &mut TextBuffer) -> Option<Range<usize>> {
    let (anchor, target) = vim_matching_bracket_pair(buffer)?;
    let start = anchor.min(target);
    let end = anchor.max(target).saturating_add(1).min(buffer.len_chars());
    (start < end).then_some(start..end)
}

fn vim_matching_bracket_pair(buffer: &mut TextBuffer) -> Option<(usize, usize)> {
    let cursor = buffer.cursor();
    if vim_char_at(buffer, cursor).is_some_and(vim_is_bracket_char) {
        let probe = cursor.saturating_add(1).min(buffer.len_chars());
        buffer.set_single_cursor(probe);
        let pair = buffer.matching_bracket();
        buffer.set_single_cursor(cursor);
        pair
    } else {
        buffer.matching_bracket()
    }
}

pub(super) fn vim_char_at(buffer: &TextBuffer, char_idx: usize) -> Option<char> {
    buffer.char_at(char_idx)
}

fn vim_is_bracket_char(ch: char) -> bool {
    matches!(ch, '(' | ')' | '[' | ']' | '{' | '}')
}

pub(super) fn vim_move_to_line_end(buffer: &mut TextBuffer, count: usize) {
    let count = count.clamp(1, VIM_MAX_COUNT);
    let line = buffer
        .cursor_position()
        .line
        .saturating_add(count.saturating_sub(1))
        .min(buffer.len_lines().saturating_sub(1));
    buffer.set_single_cursor(buffer.line_content_end_char(line));
}

pub(super) fn vim_move_space_forward(buffer: &mut TextBuffer, count: usize) {
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

pub(super) fn vim_move_space_backward(buffer: &mut TextBuffer, count: usize) {
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

pub(super) fn vim_move_next_line_first_non_whitespace(buffer: &mut TextBuffer, count: usize) {
    let line = buffer
        .cursor_position()
        .line
        .saturating_add(count.clamp(1, VIM_MAX_COUNT))
        .min(buffer.len_lines().saturating_sub(1));
    buffer.set_single_cursor(vim_line_first_non_whitespace_char(buffer, line));
}

pub(super) fn vim_move_previous_line_first_non_whitespace(buffer: &mut TextBuffer, count: usize) {
    let line = buffer
        .cursor_position()
        .line
        .saturating_sub(count.clamp(1, VIM_MAX_COUNT));
    buffer.set_single_cursor(vim_line_first_non_whitespace_char(buffer, line));
}

pub(super) fn vim_move_counted_line_first_non_whitespace(buffer: &mut TextBuffer, count: usize) {
    let line = buffer
        .cursor_position()
        .line
        .saturating_add(count.clamp(1, VIM_MAX_COUNT).saturating_sub(1))
        .min(buffer.len_lines().saturating_sub(1));
    buffer.set_single_cursor(vim_line_first_non_whitespace_char(buffer, line));
}

pub(super) fn vim_move_next_paragraph(buffer: &mut TextBuffer, count: usize) {
    for _ in 0..count.clamp(1, VIM_MAX_COUNT) {
        let target = vim_next_paragraph_line(buffer);
        buffer.set_single_cursor(buffer.line_column_to_char(target, 0));
    }
}

pub(super) fn vim_move_previous_paragraph(buffer: &mut TextBuffer, count: usize) {
    for _ in 0..count.clamp(1, VIM_MAX_COUNT) {
        let target = vim_previous_paragraph_line(buffer);
        buffer.set_single_cursor(buffer.line_column_to_char(target, 0));
    }
}

pub(super) fn vim_move_previous_big_word_end(buffer: &mut TextBuffer) {
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

pub(super) fn vim_next_paragraph_line(buffer: &TextBuffer) -> usize {
    let line_count = buffer.len_lines();
    if line_count == 0 {
        return 0;
    }

    let current = buffer
        .cursor_position()
        .line
        .min(line_count.saturating_sub(1));
    let mut line = current.saturating_add(1);
    if buffer.line_is_blank(current) {
        while line < line_count && buffer.line_is_blank(line) {
            line += 1;
        }
        return line.min(line_count.saturating_sub(1));
    }

    while line < line_count && !buffer.line_is_blank(line) {
        line += 1;
    }
    while line < line_count && buffer.line_is_blank(line) {
        line += 1;
    }
    line.min(line_count.saturating_sub(1))
}

pub(super) fn vim_previous_paragraph_line(buffer: &TextBuffer) -> usize {
    let line_count = buffer.len_lines();
    if line_count == 0 {
        return 0;
    }

    let current = buffer
        .cursor_position()
        .line
        .min(line_count.saturating_sub(1));
    if current == 0 {
        return 0;
    }

    let mut line = current.saturating_sub(1);
    while line > 0 && buffer.line_is_blank(line) {
        line -= 1;
    }
    while line > 0 && !buffer.line_is_blank(line - 1) {
        line -= 1;
    }
    line
}

pub(super) fn vim_open_line_below(buffer: &mut TextBuffer) {
    let line = buffer.cursor_position().line;
    let indent = vim_line_indent(buffer, line);
    buffer.move_line_end();
    buffer.insert_at_cursors(&vim_open_line_below_text(&indent));
}

pub(super) fn vim_open_line_above(buffer: &mut TextBuffer) {
    let line = buffer.cursor_position().line;
    let line_start = buffer.line_column_to_char(line, 0);
    let indent = vim_line_indent(buffer, line);
    let cursor = line_start.saturating_add(indent.chars().count());
    buffer.set_single_cursor(line_start);
    buffer.insert_at_cursors(&vim_open_line_above_text(&indent));
    buffer.set_single_cursor(cursor);
}

pub(super) fn vim_open_line_below_text(indent: &str) -> String {
    let mut text = String::with_capacity(1 + indent.len());
    text.push('\n');
    text.push_str(indent);
    text
}

pub(super) fn vim_open_line_above_text(indent: &str) -> String {
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

pub(super) fn vim_indent_lines(buffer: &mut TextBuffer, count: usize, indent_unit: &str) -> bool {
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

pub(super) fn vim_outdent_lines(buffer: &mut TextBuffer, count: usize, indent_unit: &str) -> bool {
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

pub(super) fn vim_line_outdent_len(buffer: &TextBuffer, line: usize, indent_unit: &str) -> usize {
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

pub(super) fn vim_apply_char_find(
    buffer: &mut TextBuffer,
    count: usize,
    motion: EditorVimCharFindMotion,
    target: char,
) -> bool {
    match motion {
        EditorVimCharFindMotion::FindBackward => vim_find_char_backward(buffer, count, target),
        EditorVimCharFindMotion::FindForward => vim_find_char_forward(buffer, count, target),
        EditorVimCharFindMotion::TillBackward => vim_till_char_backward(buffer, count, target),
        EditorVimCharFindMotion::TillForward => vim_till_char_forward(buffer, count, target),
    }
}

fn vim_find_char_forward(buffer: &mut TextBuffer, count: usize, target: char) -> bool {
    let count = count.clamp(1, VIM_MAX_COUNT);
    let cursor = buffer.cursor();
    let line_end = buffer.line_content_end_char(buffer.cursor_position().line);
    let mut remaining = count;
    for idx in cursor.saturating_add(1)..line_end {
        if buffer.char_at(idx) == Some(target) {
            remaining -= 1;
            if remaining == 0 {
                buffer.set_single_cursor(idx);
                return true;
            }
        }
    }
    false
}

fn vim_find_char_backward(buffer: &mut TextBuffer, count: usize, target: char) -> bool {
    let count = count.clamp(1, VIM_MAX_COUNT);
    let cursor = buffer.cursor();
    let line_start = buffer.line_column_to_char(buffer.cursor_position().line, 0);
    if cursor <= line_start {
        return false;
    }
    let mut remaining = count;
    for idx in (line_start..cursor).rev() {
        if buffer.char_at(idx) == Some(target) {
            remaining -= 1;
            if remaining == 0 {
                buffer.set_single_cursor(idx);
                return true;
            }
        }
    }
    false
}

fn vim_till_char_forward(buffer: &mut TextBuffer, count: usize, target: char) -> bool {
    let count = count.clamp(1, VIM_MAX_COUNT);
    let cursor = buffer.cursor();
    let line_end = buffer.line_content_end_char(buffer.cursor_position().line);
    let mut remaining = count;
    for idx in cursor.saturating_add(1)..line_end {
        if buffer.char_at(idx) == Some(target) {
            remaining -= 1;
            if remaining == 0 {
                buffer.set_single_cursor(idx.saturating_sub(1));
                return true;
            }
        }
    }
    false
}

fn vim_till_char_backward(buffer: &mut TextBuffer, count: usize, target: char) -> bool {
    let count = count.clamp(1, VIM_MAX_COUNT);
    let cursor = buffer.cursor();
    let line_start = buffer.line_column_to_char(buffer.cursor_position().line, 0);
    if cursor <= line_start {
        return false;
    }
    let mut remaining = count;
    for idx in (line_start..cursor).rev() {
        if buffer.char_at(idx) == Some(target) {
            remaining -= 1;
            if remaining == 0 {
                buffer.set_single_cursor(idx + 1);
                return true;
            }
        }
    }
    false
}

pub(super) fn vim_delete_forward_chars(buffer: &mut TextBuffer, count: usize) -> bool {
    let count = count.clamp(1, VIM_MAX_COUNT);
    let mut changed = false;
    for _ in 0..count {
        changed |= buffer.delete_forward();
    }
    changed
}

pub(super) fn vim_delete_backward_chars(buffer: &mut TextBuffer, count: usize) -> bool {
    let count = count.clamp(1, VIM_MAX_COUNT);
    let mut changed = false;
    for _ in 0..count {
        changed |= buffer.delete_backward_with_auto_pair_delete(false);
    }
    changed
}

pub(super) fn vim_delete_forward_chars_into_named_register(
    buffer: &mut TextBuffer,
    count: usize,
    unnamed_register: &mut Option<EditorVimRegister>,
    named_register: EditorVimNamedRegister,
) -> bool {
    let Some(range) = vim_delete_forward_chars_range(buffer, count) else {
        return false;
    };
    vim_delete_range_into_register(
        buffer,
        range,
        EditorVimRegisterKind::Characterwise,
        unnamed_register,
        Some(named_register),
    )
}

pub(super) fn vim_delete_backward_chars_into_named_register(
    buffer: &mut TextBuffer,
    count: usize,
    unnamed_register: &mut Option<EditorVimRegister>,
    named_register: EditorVimNamedRegister,
) -> bool {
    let Some(range) = vim_delete_backward_chars_range(buffer, count) else {
        return false;
    };
    vim_delete_range_into_register(
        buffer,
        range,
        EditorVimRegisterKind::Characterwise,
        unnamed_register,
        Some(named_register),
    )
}

fn vim_delete_forward_chars_range(buffer: &TextBuffer, count: usize) -> Option<Range<usize>> {
    let start = buffer.cursor();
    let end = start
        .saturating_add(count.clamp(1, VIM_MAX_COUNT))
        .min(buffer.len_chars());
    (start < end).then_some(start..end)
}

fn vim_delete_backward_chars_range(buffer: &TextBuffer, count: usize) -> Option<Range<usize>> {
    let end = buffer.cursor();
    let start = end.saturating_sub(count.clamp(1, VIM_MAX_COUNT));
    (start < end).then_some(start..end)
}

pub(super) fn vim_delete_line_backward(buffer: &mut TextBuffer) -> bool {
    let cursor = buffer.cursor();
    let line_start = buffer.line_column_to_char(buffer.cursor_position().line, 0);
    if cursor <= line_start {
        return false;
    }
    let edit = TextEdit {
        range: line_start..cursor,
        inserted: String::new(),
    };
    buffer.apply_edits_with_inserted_selection(vec![edit.clone()], &edit, 0..0)
}

pub(super) fn vim_replace_forward_chars(
    buffer: &mut TextBuffer,
    count: usize,
    replacement: char,
) -> bool {
    let count = count.clamp(1, VIM_MAX_COUNT);
    let start = buffer.cursor();
    let end = start.saturating_add(count).min(buffer.len_chars());
    if end <= start {
        return false;
    }

    let replaced_len = end - start;
    let inserted = std::iter::repeat_n(replacement, replaced_len).collect::<String>();
    let edit = TextEdit {
        range: start..end,
        inserted,
    };
    let cursor = replaced_len.saturating_sub(1);
    buffer.apply_edits_with_inserted_selection(vec![edit.clone()], &edit, cursor..cursor)
}

pub(super) fn vim_toggle_case_forward_chars(buffer: &mut TextBuffer, count: usize) -> bool {
    let start = buffer.cursor();
    let line_end = buffer.line_content_end_char(buffer.cursor_position().line);
    if start >= line_end {
        return false;
    }

    let end = start
        .saturating_add(count.clamp(1, VIM_MAX_COUNT))
        .min(line_end);
    vim_toggle_case_range(buffer, start..end, end)
}

pub(super) fn vim_convert_case_forward_chars(
    buffer: &mut TextBuffer,
    count: usize,
    conversion: EditorVimCaseConversion,
) -> bool {
    let start = buffer.cursor();
    let end = start
        .saturating_add(count.clamp(1, VIM_MAX_COUNT))
        .min(buffer.len_chars());
    if start >= end {
        return false;
    }

    vim_convert_case_range(buffer, start..end, start, conversion)
}

pub(super) fn vim_convert_case_lines(
    buffer: &mut TextBuffer,
    count: usize,
    conversion: EditorVimCaseConversion,
) -> bool {
    let Some(range) = vim_line_range_for_count(buffer, count) else {
        return false;
    };
    let cursor = buffer.cursor();
    vim_convert_case_range(buffer, range, cursor, conversion)
}

pub(super) fn vim_case_conversion_repeated_operator_key(
    conversion: EditorVimCaseConversion,
    key: Key,
    modifiers: Modifiers,
) -> bool {
    matches!(
        (conversion, key, modifiers.shift),
        (EditorVimCaseConversion::Lower, Key::U, false)
            | (EditorVimCaseConversion::Upper, Key::U, true)
    ) && !modifiers.command
        && !modifiers.alt
        && !modifiers.ctrl
}

pub(super) fn vim_toggle_case_range(
    buffer: &mut TextBuffer,
    range: Range<usize>,
    cursor: usize,
) -> bool {
    vim_convert_case_range(buffer, range, cursor, EditorVimCaseConversion::Toggle)
}

pub(super) fn vim_convert_case_range(
    buffer: &mut TextBuffer,
    range: Range<usize>,
    cursor: usize,
    conversion: EditorVimCaseConversion,
) -> bool {
    let mut edits = Vec::new();
    for idx in range.clone() {
        let Some(ch) = buffer.char_at(idx) else {
            continue;
        };
        let converted = match conversion {
            EditorVimCaseConversion::Lower if ch.is_ascii_uppercase() => ch.to_ascii_lowercase(),
            EditorVimCaseConversion::Upper if ch.is_ascii_lowercase() => ch.to_ascii_uppercase(),
            EditorVimCaseConversion::Toggle if ch.is_ascii_lowercase() => ch.to_ascii_uppercase(),
            EditorVimCaseConversion::Toggle if ch.is_ascii_uppercase() => ch.to_ascii_lowercase(),
            _ => continue,
        };
        edits.push(TextEdit {
            range: idx..idx + 1,
            inserted: converted.to_string(),
        });
    }

    let changed = !edits.is_empty() && buffer.apply_edits(edits);
    buffer.set_single_cursor(cursor.min(buffer.len_chars()));
    changed
}

pub(super) fn vim_join_lines(buffer: &mut TextBuffer, count: usize) -> bool {
    let count = count.clamp(1, VIM_MAX_COUNT);
    let mut changed = false;
    for _ in 0..count {
        changed |= buffer.join_lines();
    }
    changed
}

pub(super) fn vim_join_lines_without_whitespace(buffer: &mut TextBuffer, count: usize) -> bool {
    let count = count.clamp(1, VIM_MAX_COUNT);
    let mut changed = false;
    for _ in 0..count {
        changed |= vim_join_next_line_without_whitespace(buffer);
    }
    changed
}

fn vim_join_next_line_without_whitespace(buffer: &mut TextBuffer) -> bool {
    let line = buffer.cursor_position().line;
    if line + 1 >= buffer.len_lines() {
        return false;
    }
    if buffer.is_final_newline_line(line + 1) {
        return false;
    }

    let start = buffer.line_content_end_char(line);
    let next_line_start = buffer.line_column_to_char(line + 1, 0);
    let next_indent = buffer
        .line(line + 1)
        .unwrap_or_default()
        .chars()
        .take_while(|ch| matches!(ch, ' ' | '\t'))
        .count();
    let end = next_line_start
        .saturating_add(next_indent)
        .min(buffer.len_chars());
    if start >= end {
        return false;
    }

    let original_cursor = buffer.cursor();
    let changed = buffer.apply_edits(vec![TextEdit {
        range: start..end,
        inserted: String::new(),
    }]);
    if changed {
        buffer.set_single_cursor(original_cursor.min(buffer.len_chars()));
    }
    changed
}

pub(super) fn vim_ctrl_scroll_lines(count: Option<usize>) -> usize {
    count
        .unwrap_or(VIM_DEFAULT_CTRL_SCROLL_LINES)
        .clamp(1, VIM_MAX_COUNT)
}

pub(super) fn vim_line_scroll_lines(count: Option<usize>) -> usize {
    count.unwrap_or(1).clamp(1, VIM_MAX_COUNT)
}

pub(super) fn vim_page_scroll_lines(count: Option<usize>) -> usize {
    count
        .unwrap_or(1)
        .max(1)
        .saturating_mul(VIM_DEFAULT_PAGE_SCROLL_LINES)
        .min(VIM_MAX_COUNT)
}

pub(super) fn vim_move_down_lines(buffer: &mut TextBuffer, count: usize) {
    for _ in 0..count {
        buffer.move_down();
    }
}

pub(super) fn vim_move_up_lines(buffer: &mut TextBuffer, count: usize) {
    for _ in 0..count {
        buffer.move_up();
    }
}
