use kuroya_core::TextBuffer;
use std::ops::Range;

use super::super::VIM_MAX_COUNT;
pub(in crate::editor_vim_key_events) fn vim_visual_character_clamped_cursor(
    buffer: &TextBuffer,
    cursor: usize,
) -> usize {
    let len = buffer.len_chars();
    if len == 0 {
        return 0;
    }

    let cursor = cursor.min(len);
    if cursor == len {
        return len - 1;
    }

    let position = buffer.char_position(cursor);
    let line_start = buffer.line_column_to_char(position.line, 0);
    let line_content_end = buffer.line_content_end_char(position.line);
    if cursor == line_content_end && cursor > line_start {
        cursor - 1
    } else {
        cursor
    }
}

pub(in crate::editor_vim_key_events) fn vim_set_visual_character_selection(
    buffer: &mut TextBuffer,
    anchor: usize,
    cursor: usize,
) {
    let len = buffer.len_chars();
    if len == 0 {
        buffer.set_single_cursor(0);
        return;
    }

    let anchor = anchor.min(len);
    let cursor = cursor.min(len);
    if cursor >= anchor {
        buffer.set_selection(anchor, cursor.saturating_add(1).min(len));
    } else {
        buffer.set_selection(anchor.saturating_add(1).min(len), cursor);
    }
}

pub(in crate::editor_vim_key_events) fn vim_visual_character_range(
    buffer: &TextBuffer,
    anchor: usize,
    cursor: usize,
) -> Option<Range<usize>> {
    let len = buffer.len_chars();
    if len == 0 {
        return None;
    }
    let anchor = anchor.min(len);
    let cursor = cursor.min(len);
    let start = anchor.min(cursor);
    let end = anchor.max(cursor).saturating_add(1).min(len);
    (start < end).then_some(start..end)
}

pub(in crate::editor_vim_key_events) fn vim_visual_character_repeat_count(
    buffer: &TextBuffer,
    anchor: usize,
    cursor: usize,
) -> usize {
    vim_visual_character_range(buffer, anchor, cursor)
        .map(|range| {
            range
                .end
                .saturating_sub(range.start)
                .clamp(1, VIM_MAX_COUNT)
        })
        .unwrap_or(1)
}

pub(in crate::editor_vim_key_events) fn vim_visual_character_line_span(
    buffer: &TextBuffer,
    anchor: usize,
    cursor: usize,
) -> Option<(Range<usize>, usize, usize)> {
    let selection = vim_visual_character_range(buffer, anchor, cursor)?;
    let last_selected_char = selection.end.checked_sub(1)?;
    let start_line = buffer.char_position(selection.start).line;
    let end_line = buffer.char_position(last_selected_char).line;
    let start = buffer.line_column_to_char(start_line, 0);
    let end = if end_line + 1 < buffer.len_lines() {
        buffer.line_column_to_char(end_line + 1, 0)
    } else {
        buffer.len_chars()
    };
    let line_count = end_line.saturating_sub(start_line).saturating_add(1);
    (start < end).then_some((start..end, selection.start, line_count))
}

pub(in crate::editor_vim_key_events) fn vim_visual_character_line_repeat_count(
    buffer: &TextBuffer,
    anchor: usize,
    cursor: usize,
) -> usize {
    vim_visual_character_line_span(buffer, anchor, cursor)
        .map(|(_, _, line_count)| line_count.clamp(1, VIM_MAX_COUNT))
        .unwrap_or(1)
}

pub(in crate::editor_vim_key_events) fn vim_visual_character_join_repeat_count(
    buffer: &TextBuffer,
    anchor: usize,
    cursor: usize,
) -> usize {
    let Some(range) = vim_visual_character_range(buffer, anchor, cursor) else {
        return 1;
    };
    let Some(last_selected_char) = range.end.checked_sub(1) else {
        return 1;
    };

    let start_line = buffer.char_position(range.start).line;
    let end_line = buffer.char_position(last_selected_char).line;
    end_line
        .saturating_sub(start_line)
        .max(1)
        .clamp(1, VIM_MAX_COUNT)
}
