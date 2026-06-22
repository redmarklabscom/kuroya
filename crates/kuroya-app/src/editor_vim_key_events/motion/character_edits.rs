use kuroya_core::{TextBuffer, TextEdit};
use std::ops::Range;

use super::super::{
    EditorVimNamedRegister, EditorVimRegister, EditorVimRegisterKind, VIM_MAX_COUNT,
    vim_delete_range_into_register,
};

pub(in crate::editor_vim_key_events) fn vim_delete_forward_chars(
    buffer: &mut TextBuffer,
    count: usize,
) -> bool {
    let count = count.clamp(1, VIM_MAX_COUNT);
    let mut changed = false;
    for _ in 0..count {
        changed |= buffer.delete_forward();
    }
    changed
}

pub(in crate::editor_vim_key_events) fn vim_delete_backward_chars(
    buffer: &mut TextBuffer,
    count: usize,
) -> bool {
    let count = count.clamp(1, VIM_MAX_COUNT);
    let mut changed = false;
    for _ in 0..count {
        changed |= buffer.delete_backward_with_auto_pair_delete(false);
    }
    changed
}

pub(in crate::editor_vim_key_events) fn vim_delete_forward_chars_into_named_register(
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

pub(in crate::editor_vim_key_events) fn vim_delete_backward_chars_into_named_register(
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

pub(in crate::editor_vim_key_events) fn vim_delete_line_backward(buffer: &mut TextBuffer) -> bool {
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

pub(in crate::editor_vim_key_events) fn vim_replace_forward_chars(
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
