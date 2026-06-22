use kuroya_core::TextBuffer;

use super::super::super::EditorVimRepeatAction;
use super::super::super::motion::{
    vim_convert_case_forward_chars, vim_convert_case_lines, vim_toggle_case_forward_chars,
};
use super::super::super::operator::{
    vim_convert_case_operator_motion, vim_convert_case_text_object,
    vim_toggle_case_operator_motion, vim_toggle_case_text_object,
};
use super::action::ApplyLastActionOutcome;

pub(super) fn vim_apply_case_repeat_action(
    buffer: &mut TextBuffer,
    action: EditorVimRepeatAction,
    count: usize,
) -> ApplyLastActionOutcome {
    let changed = match action {
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
        _ => unreachable!("non-case repeat action routed to case handler"),
    };
    ApplyLastActionOutcome::normal(changed)
}
