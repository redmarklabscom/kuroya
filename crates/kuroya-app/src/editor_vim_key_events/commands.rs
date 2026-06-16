use kuroya_core::{TextBuffer, TextEdit};

use super::motion::{
    vim_convert_case_forward_chars, vim_convert_case_lines, vim_delete_backward_chars,
    vim_delete_backward_chars_into_named_register, vim_delete_forward_chars,
    vim_delete_forward_chars_into_named_register, vim_delete_line_backward, vim_indent_lines,
    vim_join_lines, vim_join_lines_without_whitespace, vim_open_line_above, vim_open_line_below,
    vim_outdent_lines, vim_replace_forward_chars, vim_toggle_case_forward_chars,
};
use super::operator::{
    vim_apply_operator_motion, vim_apply_operator_motion_into_named_register,
    vim_apply_text_object, vim_apply_text_object_into_named_register,
    vim_convert_case_operator_motion, vim_convert_case_text_object, vim_delete_to_line_end,
    vim_delete_to_line_end_into_named_register, vim_toggle_case_operator_motion,
    vim_toggle_case_text_object,
};
use super::state::{vim_named_register, vim_write_registers};
use super::{
    EditorVimInsertReplayStep, EditorVimLastChange, EditorVimMode, EditorVimNamedRegister,
    EditorVimRegister, EditorVimRegisterKind, EditorVimRepeatAction, VIM_MAX_COUNT,
    vim_line_range_for_count,
};

pub(super) fn vim_yank_lines(
    buffer: &TextBuffer,
    count: usize,
    unnamed_register: &mut Option<EditorVimRegister>,
) -> bool {
    vim_yank_lines_into_registers(buffer, count, unnamed_register, None)
}

pub(super) fn vim_yank_lines_into_named_register(
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

pub(super) fn vim_delete_lines_into_register(
    buffer: &mut TextBuffer,
    count: usize,
    unnamed_register: &mut Option<EditorVimRegister>,
) -> bool {
    vim_delete_lines_into_registers(buffer, count, unnamed_register, None)
}

pub(super) fn vim_delete_lines_into_named_register(
    buffer: &mut TextBuffer,
    count: usize,
    unnamed_register: &mut Option<EditorVimRegister>,
    named_register: EditorVimNamedRegister,
) -> bool {
    vim_delete_lines_into_registers(buffer, count, unnamed_register, Some(named_register))
}

fn vim_delete_lines_into_registers(
    buffer: &mut TextBuffer,
    count: usize,
    unnamed_register: &mut Option<EditorVimRegister>,
    named_register: Option<EditorVimNamedRegister>,
) -> bool {
    vim_yank_lines_into_registers(buffer, count, unnamed_register, named_register);
    vim_delete_lines(buffer, count)
}

fn vim_delete_lines(buffer: &mut TextBuffer, count: usize) -> bool {
    let count = count.clamp(1, VIM_MAX_COUNT);
    let Some(range) = vim_line_range_for_count(buffer, count) else {
        return false;
    };
    buffer.set_selection(range.start, range.end);
    buffer.delete_lines()
}

pub(super) fn vim_put_register_after(
    buffer: &mut TextBuffer,
    register: Option<&EditorVimRegister>,
    count: usize,
) -> bool {
    vim_put_register(buffer, register, count, true)
}

pub(super) fn vim_put_register_before(
    buffer: &mut TextBuffer,
    register: Option<&EditorVimRegister>,
    count: usize,
) -> bool {
    vim_put_register(buffer, register, count, false)
}

fn vim_put_register(
    buffer: &mut TextBuffer,
    register: Option<&EditorVimRegister>,
    count: usize,
    after: bool,
) -> bool {
    let Some(register) = register else {
        return false;
    };
    match register.kind {
        EditorVimRegisterKind::Characterwise => {
            vim_put_characterwise_register(buffer, register, count, after)
        }
        EditorVimRegisterKind::Linewise => {
            vim_put_linewise_register(buffer, register, count, after)
        }
    }
}

fn vim_put_linewise_register(
    buffer: &mut TextBuffer,
    register: &EditorVimRegister,
    count: usize,
    after: bool,
) -> bool {
    if register.text.is_empty() {
        return false;
    }

    let count = count.clamp(1, VIM_MAX_COUNT);
    let current_line = buffer.cursor_position().line;
    let insert_at = if after && current_line + 1 < buffer.len_lines() {
        buffer.line_column_to_char(current_line + 1, 0)
    } else if after {
        buffer.len_chars()
    } else {
        buffer.line_column_to_char(current_line, 0)
    };

    let mut inserted = String::new();
    for _ in 0..count {
        inserted.push_str(&register.text);
    }

    let cursor_offset = if after
        && insert_at == buffer.len_chars()
        && buffer.len_chars() > 0
        && !vim_buffer_ends_with_line_break(buffer)
    {
        inserted.insert(0, '\n');
        1
    } else {
        0
    };

    let edit = TextEdit {
        range: insert_at..insert_at,
        inserted,
    };
    buffer.apply_edits_with_inserted_selection(
        vec![edit.clone()],
        &edit,
        cursor_offset..cursor_offset,
    )
}

fn vim_put_characterwise_register(
    buffer: &mut TextBuffer,
    register: &EditorVimRegister,
    count: usize,
    after: bool,
) -> bool {
    if register.text.is_empty() {
        return false;
    }
    let insert_at = if after {
        buffer.cursor().saturating_add(1).min(buffer.len_chars())
    } else {
        buffer.cursor()
    };
    let count = count.clamp(1, VIM_MAX_COUNT);
    let mut inserted = String::new();
    for _ in 0..count {
        inserted.push_str(&register.text);
    }
    let inserted_len = inserted.chars().count();
    let cursor_offset = inserted_len.saturating_sub(1);
    let edit = TextEdit {
        range: insert_at..insert_at,
        inserted,
    };
    buffer.apply_edits_with_inserted_selection(
        vec![edit.clone()],
        &edit,
        cursor_offset..cursor_offset,
    )
}

fn vim_buffer_ends_with_line_break(buffer: &TextBuffer) -> bool {
    let len = buffer.len_chars();
    len > 0 && buffer.char_at(len - 1) == Some('\n')
}

pub(super) fn vim_apply_last_change(
    buffer: &mut TextBuffer,
    change: EditorVimLastChange,
    repeat_count: Option<usize>,
    mode: &mut EditorVimMode,
    unnamed_register: &mut Option<EditorVimRegister>,
    indent_unit: &str,
) -> bool {
    let count = repeat_count.unwrap_or(change.count).clamp(1, VIM_MAX_COUNT);
    let mut enters_insert = false;
    let changed = match change.action {
        EditorVimRepeatAction::AppendAfterCursor => {
            buffer.move_right();
            false
        }
        EditorVimRepeatAction::ChangeLines => {
            enters_insert = true;
            vim_delete_lines_into_register(buffer, count, unnamed_register)
        }
        EditorVimRepeatAction::ChangeLinesIntoRegister(register) => {
            enters_insert = true;
            vim_delete_lines_into_named_register(buffer, count, unnamed_register, register)
        }
        EditorVimRepeatAction::ChangeOperatorMotion(motion) => {
            enters_insert = true;
            vim_apply_operator_motion(buffer, 1, count, motion, unnamed_register)
        }
        EditorVimRepeatAction::ChangeOperatorMotionIntoRegister { motion, register } => {
            enters_insert = true;
            vim_apply_operator_motion_into_named_register(
                buffer,
                1,
                count,
                motion,
                unnamed_register,
                register,
            )
        }
        EditorVimRepeatAction::ChangeTextObject { scope, kind } => {
            enters_insert = true;
            vim_apply_text_object(buffer, 1, count, scope, kind, unnamed_register)
        }
        EditorVimRepeatAction::ChangeTextObjectIntoRegister {
            scope,
            kind,
            register,
        } => {
            enters_insert = true;
            vim_apply_text_object_into_named_register(
                buffer,
                1,
                count,
                scope,
                kind,
                unnamed_register,
                register,
            )
        }
        EditorVimRepeatAction::ChangeToLineEnd => {
            enters_insert = true;
            vim_delete_to_line_end(buffer, count)
        }
        EditorVimRepeatAction::ChangeToLineEndIntoRegister(register) => {
            enters_insert = true;
            vim_delete_to_line_end_into_named_register(buffer, count, unnamed_register, register)
        }
        EditorVimRepeatAction::DeleteBackwardChars => vim_delete_backward_chars(buffer, count),
        EditorVimRepeatAction::DeleteBackwardCharsIntoRegister(register) => {
            vim_delete_backward_chars_into_named_register(buffer, count, unnamed_register, register)
        }
        EditorVimRepeatAction::DeleteForwardChars => vim_delete_forward_chars(buffer, count),
        EditorVimRepeatAction::DeleteForwardCharsIntoRegister(register) => {
            vim_delete_forward_chars_into_named_register(buffer, count, unnamed_register, register)
        }
        EditorVimRepeatAction::DeleteLines => {
            vim_delete_lines_into_register(buffer, count, unnamed_register)
        }
        EditorVimRepeatAction::DeleteLinesIntoRegister(register) => {
            vim_delete_lines_into_named_register(buffer, count, unnamed_register, register)
        }
        EditorVimRepeatAction::DeleteOperatorMotion(motion) => {
            vim_apply_operator_motion(buffer, 1, count, motion, unnamed_register)
        }
        EditorVimRepeatAction::DeleteOperatorMotionIntoRegister { motion, register } => {
            vim_apply_operator_motion_into_named_register(
                buffer,
                1,
                count,
                motion,
                unnamed_register,
                register,
            )
        }
        EditorVimRepeatAction::DeleteTextObject { scope, kind } => {
            vim_apply_text_object(buffer, 1, count, scope, kind, unnamed_register)
        }
        EditorVimRepeatAction::DeleteTextObjectIntoRegister {
            scope,
            kind,
            register,
        } => vim_apply_text_object_into_named_register(
            buffer,
            1,
            count,
            scope,
            kind,
            unnamed_register,
            register,
        ),
        EditorVimRepeatAction::DeleteToLineEnd => vim_delete_to_line_end(buffer, count),
        EditorVimRepeatAction::DeleteToLineEndIntoRegister(register) => {
            vim_delete_to_line_end_into_named_register(buffer, count, unnamed_register, register)
        }
        EditorVimRepeatAction::IndentLines => vim_indent_lines(buffer, count, indent_unit),
        EditorVimRepeatAction::InsertAtCursor => false,
        EditorVimRepeatAction::InsertLineEnd => {
            buffer.move_line_end();
            false
        }
        EditorVimRepeatAction::InsertLineFirstNonWhitespace => {
            buffer.move_line_first_non_whitespace();
            false
        }
        EditorVimRepeatAction::JoinLines => vim_join_lines(buffer, count),
        EditorVimRepeatAction::JoinLinesWithoutWhitespace => {
            vim_join_lines_without_whitespace(buffer, count)
        }
        EditorVimRepeatAction::OpenLineAbove => {
            enters_insert = true;
            for _ in 0..count {
                vim_open_line_above(buffer);
            }
            true
        }
        EditorVimRepeatAction::OpenLineBelow => {
            enters_insert = true;
            for _ in 0..count {
                vim_open_line_below(buffer);
            }
            true
        }
        EditorVimRepeatAction::OutdentLines => vim_outdent_lines(buffer, count, indent_unit),
        EditorVimRepeatAction::PutAfter => {
            vim_put_register_after(buffer, unnamed_register.as_ref(), count)
        }
        EditorVimRepeatAction::PutAfterNamed(register) => {
            let named_register = vim_named_register(register);
            vim_put_register_after(buffer, named_register.as_ref(), count)
        }
        EditorVimRepeatAction::PutBefore => {
            vim_put_register_before(buffer, unnamed_register.as_ref(), count)
        }
        EditorVimRepeatAction::PutBeforeNamed(register) => {
            let named_register = vim_named_register(register);
            vim_put_register_before(buffer, named_register.as_ref(), count)
        }
        EditorVimRepeatAction::ReplaceForwardChars(replacement) => {
            vim_replace_forward_chars(buffer, count, replacement)
        }
        EditorVimRepeatAction::SubstituteForwardChars => {
            enters_insert = true;
            vim_delete_forward_chars(buffer, count)
        }
        EditorVimRepeatAction::SubstituteForwardCharsIntoRegister(register) => {
            enters_insert = true;
            vim_delete_forward_chars_into_named_register(buffer, count, unnamed_register, register)
        }
        EditorVimRepeatAction::ConvertCaseForwardChars(conversion) => {
            vim_convert_case_forward_chars(buffer, count, conversion)
        }
        EditorVimRepeatAction::ConvertCaseLines(conversion) => {
            vim_convert_case_lines(buffer, count, conversion)
        }
        EditorVimRepeatAction::ConvertCaseOperatorMotion { motion, conversion } => {
            vim_convert_case_operator_motion(buffer, 1, count, motion, conversion)
        }
        EditorVimRepeatAction::ConvertCaseTextObject {
            scope,
            kind,
            conversion,
        } => vim_convert_case_text_object(buffer, 1, count, scope, kind, conversion),
        EditorVimRepeatAction::ToggleCaseForwardChars => {
            vim_toggle_case_forward_chars(buffer, count)
        }
        EditorVimRepeatAction::ToggleCaseOperatorMotion(motion) => {
            vim_toggle_case_operator_motion(buffer, 1, count, motion)
        }
        EditorVimRepeatAction::ToggleCaseTextObject { scope, kind } => {
            vim_toggle_case_text_object(buffer, 1, count, scope, kind)
        }
    };

    if change.action.accepts_inserted_text() && !change.insert_replay.is_empty() {
        let insert_count = if change.action.is_plain_insert() {
            count
        } else {
            1
        };
        let inserted =
            vim_replay_insert_steps(buffer, &change.insert_replay, insert_count, indent_unit);
        *mode = EditorVimMode::Normal;
        return changed || inserted;
    }

    if enters_insert || change.action.is_plain_insert() {
        *mode = EditorVimMode::Insert;
    }
    changed
}

fn vim_replay_insert_steps(
    buffer: &mut TextBuffer,
    steps: &[EditorVimInsertReplayStep],
    count: usize,
    indent_unit: &str,
) -> bool {
    if steps.is_empty() {
        return false;
    }
    let mut changed = false;
    for _ in 0..count.clamp(1, VIM_MAX_COUNT) {
        for step in steps {
            changed |= vim_replay_insert_step(buffer, step, indent_unit);
        }
    }
    changed
}

fn vim_replay_insert_step(
    buffer: &mut TextBuffer,
    step: &EditorVimInsertReplayStep,
    indent_unit: &str,
) -> bool {
    match step {
        EditorVimInsertReplayStep::Backspace => buffer.delete_backward_with_auto_pair_delete(false),
        EditorVimInsertReplayStep::DeleteLineBackward => vim_delete_line_backward(buffer),
        EditorVimInsertReplayStep::DeleteWordBackward => buffer.delete_word_backward(),
        EditorVimInsertReplayStep::Enter => {
            buffer.insert_at_cursors("\n");
            true
        }
        EditorVimInsertReplayStep::EnterAutoIndent => {
            buffer.insert_newline_with_indent_unit(indent_unit);
            true
        }
        EditorVimInsertReplayStep::InsertText(text) => {
            buffer.insert_at_cursors(text);
            !text.is_empty()
        }
        EditorVimInsertReplayStep::Tab => {
            if indent_unit.is_empty() {
                return false;
            }
            buffer.insert_at_cursors(indent_unit);
            true
        }
        EditorVimInsertReplayStep::ShiftTab => buffer.outdent_lines(indent_unit),
    }
}
