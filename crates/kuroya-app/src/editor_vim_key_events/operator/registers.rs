use kuroya_core::TextBuffer;
use std::ops::Range;

use super::super::{
    EditorVimNamedRegister, EditorVimRegister, EditorVimRegisterKind, VIM_MAX_COUNT,
    vim_write_registers,
};

pub(in crate::editor_vim_key_events) fn vim_delete_range_into_register(
    buffer: &mut TextBuffer,
    range: Range<usize>,
    kind: EditorVimRegisterKind,
    unnamed_register: &mut Option<EditorVimRegister>,
    named_register: Option<EditorVimNamedRegister>,
) -> bool {
    if !vim_yank_range_into_register(
        buffer,
        range.clone(),
        kind,
        unnamed_register,
        named_register,
    ) {
        return false;
    }
    buffer.set_selection(range.start, range.end);
    buffer.delete_selection_ranges()
}

pub(in crate::editor_vim_key_events) fn vim_yank_range_into_register(
    buffer: &TextBuffer,
    range: Range<usize>,
    kind: EditorVimRegisterKind,
    unnamed_register: &mut Option<EditorVimRegister>,
    named_register: Option<EditorVimNamedRegister>,
) -> bool {
    let Some(text) = buffer.text_range(range) else {
        return false;
    };
    if text.is_empty() {
        return false;
    }
    vim_write_registers(
        unnamed_register,
        named_register,
        EditorVimRegister { text, kind },
    );
    true
}

pub(in crate::editor_vim_key_events) fn vim_delete_to_line_end(
    buffer: &mut TextBuffer,
    count: usize,
) -> bool {
    let Some(range) = vim_delete_to_line_end_range(buffer, count) else {
        return false;
    };
    buffer.set_selection(range.start, range.end);
    buffer.delete_selection_ranges()
}

pub(in crate::editor_vim_key_events) fn vim_delete_to_line_end_into_named_register(
    buffer: &mut TextBuffer,
    count: usize,
    unnamed_register: &mut Option<EditorVimRegister>,
    named_register: EditorVimNamedRegister,
) -> bool {
    let Some(range) = vim_delete_to_line_end_range(buffer, count) else {
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

fn vim_delete_to_line_end_range(buffer: &TextBuffer, count: usize) -> Option<Range<usize>> {
    let count = count.clamp(1, VIM_MAX_COUNT);
    let start = buffer.cursor();
    let start_line = buffer.cursor_position().line;
    let end_line = start_line
        .saturating_add(count.saturating_sub(1))
        .min(buffer.len_lines().saturating_sub(1));
    let end = buffer.line_content_end_char(end_line);
    (start < end).then_some(start..end)
}
