use kuroya_core::{TextBuffer, TextEdit};

use super::super::{
    EditorVimCaseConversion, EditorVimNamedRegister, EditorVimRegister, EditorVimRegisterKind,
    vim_convert_case_range, vim_delete_range_into_register, vim_line_outdent_len,
    vim_yank_range_into_register,
};
use super::selection::{vim_visual_character_line_span, vim_visual_character_range};
pub(in crate::editor_vim_key_events) fn vim_join_visual_character_lines(
    buffer: &mut TextBuffer,
    anchor: usize,
    cursor: usize,
) -> bool {
    let Some(range) = vim_visual_character_range(buffer, anchor, cursor) else {
        buffer.set_single_cursor(cursor.min(buffer.len_chars()));
        return false;
    };
    let cursor = range.start;
    buffer.set_selection(range.start, range.end);
    let changed = buffer.join_lines();
    buffer.set_single_cursor(cursor.min(buffer.len_chars()));
    changed
}

pub(in crate::editor_vim_key_events) fn vim_indent_visual_character_lines(
    buffer: &mut TextBuffer,
    anchor: usize,
    cursor: usize,
    indent_unit: &str,
) -> bool {
    let Some((range, selection_start, _)) = vim_visual_character_line_span(buffer, anchor, cursor)
    else {
        buffer.set_single_cursor(cursor.min(buffer.len_chars()));
        return false;
    };
    let position = buffer.char_position(selection_start);
    if indent_unit.is_empty() {
        buffer.set_single_cursor(selection_start.min(buffer.len_chars()));
        return false;
    }

    buffer.set_selection(range.start, range.end);
    let changed = buffer.indent_lines(indent_unit);
    if changed {
        let column = position.column.saturating_add(indent_unit.chars().count());
        buffer.set_single_cursor(buffer.line_column_to_char(position.line, column));
    } else {
        buffer.set_single_cursor(selection_start.min(buffer.len_chars()));
    }
    changed
}

pub(in crate::editor_vim_key_events) fn vim_outdent_visual_character_lines(
    buffer: &mut TextBuffer,
    anchor: usize,
    cursor: usize,
    indent_unit: &str,
) -> bool {
    let Some((range, selection_start, _)) = vim_visual_character_line_span(buffer, anchor, cursor)
    else {
        buffer.set_single_cursor(cursor.min(buffer.len_chars()));
        return false;
    };
    let position = buffer.char_position(selection_start);
    let remove_len = vim_line_outdent_len(buffer, position.line, indent_unit);

    buffer.set_selection(range.start, range.end);
    let changed = buffer.outdent_lines(indent_unit);
    if changed {
        let column = position.column.saturating_sub(remove_len);
        buffer.set_single_cursor(buffer.line_column_to_char(position.line, column));
    } else {
        buffer.set_single_cursor(selection_start.min(buffer.len_chars()));
    }
    changed
}

pub(in crate::editor_vim_key_events) fn vim_yank_visual_character(
    buffer: &mut TextBuffer,
    anchor: usize,
    cursor: usize,
    unnamed_register: &mut Option<EditorVimRegister>,
) -> bool {
    vim_yank_visual_character_into_registers(buffer, anchor, cursor, unnamed_register, None)
}

pub(in crate::editor_vim_key_events) fn vim_yank_visual_character_into_named_register(
    buffer: &mut TextBuffer,
    anchor: usize,
    cursor: usize,
    unnamed_register: &mut Option<EditorVimRegister>,
    named_register: EditorVimNamedRegister,
) -> bool {
    vim_yank_visual_character_into_registers(
        buffer,
        anchor,
        cursor,
        unnamed_register,
        Some(named_register),
    )
}

pub(in crate::editor_vim_key_events) fn vim_yank_visual_character_into_registers(
    buffer: &mut TextBuffer,
    anchor: usize,
    cursor: usize,
    unnamed_register: &mut Option<EditorVimRegister>,
    named_register: Option<EditorVimNamedRegister>,
) -> bool {
    let Some(range) = vim_visual_character_range(buffer, anchor, cursor) else {
        buffer.set_single_cursor(cursor.min(buffer.len_chars()));
        return false;
    };
    let yanked = vim_yank_range_into_register(
        buffer,
        range.clone(),
        EditorVimRegisterKind::Characterwise,
        unnamed_register,
        named_register,
    );
    buffer.set_single_cursor(range.start);
    yanked
}

pub(in crate::editor_vim_key_events) fn vim_delete_visual_character(
    buffer: &mut TextBuffer,
    anchor: usize,
    cursor: usize,
    unnamed_register: &mut Option<EditorVimRegister>,
) -> bool {
    vim_delete_visual_character_into_registers(buffer, anchor, cursor, unnamed_register, None)
}

pub(in crate::editor_vim_key_events) fn vim_delete_visual_character_into_named_register(
    buffer: &mut TextBuffer,
    anchor: usize,
    cursor: usize,
    unnamed_register: &mut Option<EditorVimRegister>,
    named_register: EditorVimNamedRegister,
) -> bool {
    vim_delete_visual_character_into_registers(
        buffer,
        anchor,
        cursor,
        unnamed_register,
        Some(named_register),
    )
}

pub(in crate::editor_vim_key_events) fn vim_delete_visual_character_into_registers(
    buffer: &mut TextBuffer,
    anchor: usize,
    cursor: usize,
    unnamed_register: &mut Option<EditorVimRegister>,
    named_register: Option<EditorVimNamedRegister>,
) -> bool {
    let Some(range) = vim_visual_character_range(buffer, anchor, cursor) else {
        buffer.set_single_cursor(cursor.min(buffer.len_chars()));
        return false;
    };
    vim_delete_range_into_register(
        buffer,
        range,
        EditorVimRegisterKind::Characterwise,
        unnamed_register,
        named_register,
    )
}

pub(in crate::editor_vim_key_events) fn vim_convert_case_visual_character(
    buffer: &mut TextBuffer,
    anchor: usize,
    cursor: usize,
    conversion: EditorVimCaseConversion,
) -> bool {
    let Some(range) = vim_visual_character_range(buffer, anchor, cursor) else {
        buffer.set_single_cursor(cursor.min(buffer.len_chars()));
        return false;
    };
    vim_convert_case_range(buffer, range.clone(), range.start, conversion)
}

pub(in crate::editor_vim_key_events) fn vim_replace_visual_character(
    buffer: &mut TextBuffer,
    anchor: usize,
    cursor: usize,
    replacement: char,
) -> bool {
    let Some(range) = vim_visual_character_range(buffer, anchor, cursor) else {
        buffer.set_single_cursor(cursor.min(buffer.len_chars()));
        return false;
    };
    let replaced_len = range.end.saturating_sub(range.start);
    if replaced_len == 0 {
        buffer.set_single_cursor(range.start);
        return false;
    }

    let inserted = std::iter::repeat_n(replacement, replaced_len).collect::<String>();
    let edit = TextEdit { range, inserted };
    buffer.apply_edits_with_inserted_selection(vec![edit.clone()], &edit, 0..0)
}
