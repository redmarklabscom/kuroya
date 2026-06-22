use kuroya_core::TextBuffer;

use super::super::{EditorVimCharFindMotion, VIM_MAX_COUNT};

pub(in crate::editor_vim_key_events) fn vim_apply_char_find(
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
