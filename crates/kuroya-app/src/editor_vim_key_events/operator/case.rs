use kuroya_core::TextBuffer;

use super::super::{
    EditorVimCaseConversion, EditorVimOperatorMotion, EditorVimTextObjectKind,
    EditorVimTextObjectScope, vim_combined_count, vim_convert_case_range, vim_toggle_case_range,
};
use super::motions::vim_operator_motion_range;
use super::text_objects::vim_text_object_range;

pub(in crate::editor_vim_key_events) fn vim_convert_case_operator_motion(
    buffer: &mut TextBuffer,
    operator_count: usize,
    motion_count: usize,
    motion: EditorVimOperatorMotion,
    conversion: EditorVimCaseConversion,
) -> bool {
    let count = vim_combined_count(operator_count, motion_count);
    let Some(range) = vim_operator_motion_range(buffer, count, motion) else {
        return false;
    };
    let cursor = range.start;
    vim_convert_case_range(buffer, range, cursor, conversion)
}

pub(in crate::editor_vim_key_events) fn vim_convert_case_text_object(
    buffer: &mut TextBuffer,
    operator_count: usize,
    motion_count: usize,
    scope: EditorVimTextObjectScope,
    kind: EditorVimTextObjectKind,
    conversion: EditorVimCaseConversion,
) -> bool {
    let count = vim_combined_count(operator_count, motion_count);
    let Some(range) = vim_text_object_range(buffer, count, scope, kind) else {
        return false;
    };
    let cursor = range.start;
    vim_convert_case_range(buffer, range, cursor, conversion)
}

pub(in crate::editor_vim_key_events) fn vim_toggle_case_operator_motion(
    buffer: &mut TextBuffer,
    operator_count: usize,
    motion_count: usize,
    motion: EditorVimOperatorMotion,
) -> bool {
    let count = vim_combined_count(operator_count, motion_count);
    let Some(range) = vim_operator_motion_range(buffer, count, motion) else {
        return false;
    };
    let cursor = range.start;
    vim_toggle_case_range(buffer, range, cursor)
}

pub(in crate::editor_vim_key_events) fn vim_toggle_case_text_object(
    buffer: &mut TextBuffer,
    operator_count: usize,
    motion_count: usize,
    scope: EditorVimTextObjectScope,
    kind: EditorVimTextObjectKind,
) -> bool {
    let count = vim_combined_count(operator_count, motion_count);
    let Some(range) = vim_text_object_range(buffer, count, scope, kind) else {
        return false;
    };
    let cursor = range.start;
    vim_toggle_case_range(buffer, range, cursor)
}
