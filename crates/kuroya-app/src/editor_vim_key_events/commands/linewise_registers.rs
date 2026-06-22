use kuroya_core::{TextBuffer, TextEdit};
use std::ops::Range;

use super::super::state::vim_write_registers;
use super::super::{
    EditorVimNamedRegister, EditorVimRegister, EditorVimRegisterKind, VIM_MAX_COUNT,
    vim_line_range_for_count,
};

pub(in crate::editor_vim_key_events) fn vim_yank_lines(
    buffer: &TextBuffer,
    count: usize,
    unnamed_register: &mut Option<EditorVimRegister>,
) -> bool {
    vim_yank_lines_into_registers(buffer, count, unnamed_register, None)
}

pub(in crate::editor_vim_key_events) fn vim_yank_lines_into_named_register(
    buffer: &TextBuffer,
    count: usize,
    unnamed_register: &mut Option<EditorVimRegister>,
    named_register: EditorVimNamedRegister,
) -> bool {
    vim_yank_lines_into_registers(buffer, count, unnamed_register, Some(named_register))
}

fn vim_yank_lines_into_registers(
    buffer: &TextBuffer,
    count: usize,
    unnamed_register: &mut Option<EditorVimRegister>,
    named_register: Option<EditorVimNamedRegister>,
) -> bool {
    let Some(range) = vim_line_range_for_count(buffer, count) else {
        return false;
    };
    let Some(mut text) = buffer.text_range(range) else {
        return false;
    };
    if !text.ends_with('\n') {
        text.push('\n');
    }
    vim_write_registers(
        unnamed_register,
        named_register,
        EditorVimRegister {
            text,
            kind: EditorVimRegisterKind::Linewise,
        },
    );
    true
}

pub(in crate::editor_vim_key_events) fn vim_delete_lines_into_register(
    buffer: &mut TextBuffer,
    count: usize,
    unnamed_register: &mut Option<EditorVimRegister>,
) -> bool {
    vim_delete_lines_into_registers(
        buffer,
        count,
        unnamed_register,
        None,
        VimLineDeleteMode::Delete,
    )
}

pub(in crate::editor_vim_key_events) fn vim_delete_lines_into_named_register(
    buffer: &mut TextBuffer,
    count: usize,
    unnamed_register: &mut Option<EditorVimRegister>,
    named_register: EditorVimNamedRegister,
) -> bool {
    vim_delete_lines_into_registers(
        buffer,
        count,
        unnamed_register,
        Some(named_register),
        VimLineDeleteMode::Delete,
    )
}

pub(in crate::editor_vim_key_events) fn vim_change_lines_into_register(
    buffer: &mut TextBuffer,
    count: usize,
    unnamed_register: &mut Option<EditorVimRegister>,
) -> bool {
    vim_delete_lines_into_registers(
        buffer,
        count,
        unnamed_register,
        None,
        VimLineDeleteMode::Change,
    )
}

pub(in crate::editor_vim_key_events) fn vim_change_lines_into_named_register(
    buffer: &mut TextBuffer,
    count: usize,
    unnamed_register: &mut Option<EditorVimRegister>,
    named_register: EditorVimNamedRegister,
) -> bool {
    vim_delete_lines_into_registers(
        buffer,
        count,
        unnamed_register,
        Some(named_register),
        VimLineDeleteMode::Change,
    )
}

fn vim_delete_lines_into_registers(
    buffer: &mut TextBuffer,
    count: usize,
    unnamed_register: &mut Option<EditorVimRegister>,
    named_register: Option<EditorVimNamedRegister>,
    mode: VimLineDeleteMode,
) -> bool {
    vim_yank_lines_into_registers(buffer, count, unnamed_register, named_register);
    vim_delete_lines(buffer, count, mode)
}

fn vim_delete_lines(buffer: &mut TextBuffer, count: usize, mode: VimLineDeleteMode) -> bool {
    let count = count.clamp(1, VIM_MAX_COUNT);
    let Some(mut range) = vim_line_range_for_count(buffer, count) else {
        return false;
    };
    let inserted = match mode {
        VimLineDeleteMode::Change => vim_line_change_replacement(buffer, &range),
        VimLineDeleteMode::Delete => {
            range = vim_line_delete_range_without_final_blank_row(buffer, range);
            String::new()
        }
    };
    buffer.apply_edits(vec![TextEdit { range, inserted }])
}

#[derive(Clone, Copy)]
enum VimLineDeleteMode {
    Change,
    Delete,
}

fn vim_line_delete_range_without_final_blank_row(
    buffer: &TextBuffer,
    range: Range<usize>,
) -> Range<usize> {
    if range.start == 0 || range.end < buffer.len_chars() {
        return range;
    }
    let Some(previous) = buffer.text_range(range.start - 1..range.start) else {
        return range;
    };
    if previous != "\n" {
        return range;
    }

    let mut start = range.start - 1;
    if start > 0 && buffer.text_range(start - 1..start).as_deref() == Some("\r") {
        start -= 1;
    }
    start..range.end
}

fn vim_line_change_replacement(buffer: &TextBuffer, range: &Range<usize>) -> String {
    let Some(text) = buffer.text_range(range.clone()) else {
        return String::new();
    };
    if text.ends_with("\r\n") {
        "\r\n".to_owned()
    } else if text.ends_with('\n') {
        "\n".to_owned()
    } else {
        String::new()
    }
}
