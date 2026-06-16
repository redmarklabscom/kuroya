use eframe::egui::{Key, Modifiers};
use kuroya_core::TextBuffer;
use std::cell::RefCell;

use super::{
    EditorVimNamedRegister, EditorVimRegister, EditorVimRegisterKind,
    vim_line_first_non_whitespace_char, vim_printable_key_char,
};

const VIM_MARK_SLOT_COUNT: usize = 52;
const VIM_NAMED_REGISTER_COUNT: usize = 26;
const VIM_BLACK_HOLE_REGISTER_INDEX: usize = VIM_NAMED_REGISTER_COUNT;
const VIM_MARK_BUFFER_LIMIT: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EditorVimMark {
    line: usize,
    column: usize,
}

#[derive(Debug, Clone)]
struct EditorVimBufferMarks {
    buffer_key: usize,
    marks: [Option<EditorVimMark>; VIM_MARK_SLOT_COUNT],
}

impl EditorVimBufferMarks {
    fn new(buffer_key: usize) -> Self {
        Self {
            buffer_key,
            marks: [None; VIM_MARK_SLOT_COUNT],
        }
    }
}

thread_local! {
    static VIM_MARKS: RefCell<Vec<EditorVimBufferMarks>> = const { RefCell::new(Vec::new()) };
    static VIM_NAMED_REGISTERS: RefCell<[Option<EditorVimRegister>; VIM_NAMED_REGISTER_COUNT]> =
        RefCell::new(std::array::from_fn(|_| None));
}

pub(super) fn vim_mark_name_for_key(key: Key, modifiers: Modifiers) -> Option<char> {
    vim_printable_key_char(key, modifiers).filter(|ch| ch.is_ascii_alphabetic())
}

fn vim_mark_slot(mark: char) -> Option<usize> {
    if mark.is_ascii_lowercase() {
        Some((mark as u8 - b'a') as usize)
    } else if mark.is_ascii_uppercase() {
        Some(26 + (mark as u8 - b'A') as usize)
    } else {
        None
    }
}

pub(super) fn vim_named_register_for_key(
    key: Key,
    modifiers: Modifiers,
) -> Option<EditorVimNamedRegister> {
    let ch = vim_printable_key_char(key, modifiers)?;
    if ch == '_' {
        Some(EditorVimNamedRegister {
            index: VIM_BLACK_HOLE_REGISTER_INDEX,
            append: false,
        })
    } else {
        ch.is_ascii_alphabetic().then(|| EditorVimNamedRegister {
            index: (ch.to_ascii_lowercase() as u8 - b'a') as usize,
            append: ch.is_ascii_uppercase(),
        })
    }
}

fn vim_named_register_is_black_hole(register: EditorVimNamedRegister) -> bool {
    register.index == VIM_BLACK_HOLE_REGISTER_INDEX
}

pub(super) fn vim_named_register(register: EditorVimNamedRegister) -> Option<EditorVimRegister> {
    if vim_named_register_is_black_hole(register) || register.index >= VIM_NAMED_REGISTER_COUNT {
        return None;
    }
    VIM_NAMED_REGISTERS.with(|registers| registers.borrow()[register.index].clone())
}

fn vim_store_named_register(register: EditorVimNamedRegister, value: &EditorVimRegister) {
    if vim_named_register_is_black_hole(register) || register.index >= VIM_NAMED_REGISTER_COUNT {
        return;
    }
    VIM_NAMED_REGISTERS.with(|registers| {
        let mut registers = registers.borrow_mut();
        if register.append
            && let Some(existing) = &mut registers[register.index]
        {
            existing.text.push_str(&value.text);
            if value.kind == EditorVimRegisterKind::Linewise {
                existing.kind = EditorVimRegisterKind::Linewise;
            }
        } else {
            registers[register.index] = Some(value.clone());
        }
    });
}

pub(super) fn vim_write_registers(
    unnamed_register: &mut Option<EditorVimRegister>,
    named_register: Option<EditorVimNamedRegister>,
    value: EditorVimRegister,
) {
    if named_register.is_some_and(vim_named_register_is_black_hole) {
        return;
    }
    if let Some(register) = named_register {
        vim_store_named_register(register, &value);
    }
    *unnamed_register = Some(value);
}

#[cfg(test)]
pub(super) fn vim_clear_named_registers() {
    VIM_NAMED_REGISTERS.with(|registers| {
        registers.borrow_mut().fill(None);
    });
}

fn vim_buffer_mark_key(buffer: &TextBuffer) -> usize {
    buffer as *const TextBuffer as usize
}

pub(super) fn vim_set_mark(buffer: &TextBuffer, mark: char) -> bool {
    let Some(slot) = vim_mark_slot(mark) else {
        return false;
    };
    let position = buffer.cursor_position();
    let mark = EditorVimMark {
        line: position.line,
        column: position.column,
    };
    let buffer_key = vim_buffer_mark_key(buffer);
    VIM_MARKS.with(|marks| {
        let mut marks = marks.borrow_mut();
        if let Some(existing) = marks
            .iter_mut()
            .find(|entry| entry.buffer_key == buffer_key)
        {
            existing.marks[slot] = Some(mark);
            return;
        }

        if marks.len() >= VIM_MARK_BUFFER_LIMIT {
            marks.remove(0);
        }
        let mut entry = EditorVimBufferMarks::new(buffer_key);
        entry.marks[slot] = Some(mark);
        marks.push(entry);
    });
    true
}

fn vim_mark_for_buffer(buffer: &TextBuffer, mark: char) -> Option<EditorVimMark> {
    let slot = vim_mark_slot(mark)?;
    let buffer_key = vim_buffer_mark_key(buffer);
    VIM_MARKS.with(|marks| {
        marks
            .borrow()
            .iter()
            .find(|entry| entry.buffer_key == buffer_key)
            .and_then(|entry| entry.marks[slot])
    })
}

pub(super) fn vim_jump_to_mark(buffer: &mut TextBuffer, mark: char, linewise: bool) -> bool {
    let Some(mark) = vim_mark_for_buffer(buffer, mark) else {
        return false;
    };
    let line = mark.line.min(buffer.len_lines().saturating_sub(1));
    let cursor = if linewise {
        vim_line_first_non_whitespace_char(buffer, line)
    } else {
        buffer.line_column_to_char(line, mark.column)
    };
    buffer.set_single_cursor(cursor);
    true
}
