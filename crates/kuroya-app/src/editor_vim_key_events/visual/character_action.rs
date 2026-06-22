use eframe::egui::{Key, Modifiers};
use kuroya_core::TextBuffer;

use super::super::{
    EditorVimLastChange, EditorVimMode, EditorVimPendingKey, EditorVimRegister,
    EditorVimRepeatAction, VimKeyResult, vim_repeatable_change_result,
};
use super::{
    vim_convert_case_visual_character, vim_delete_visual_character,
    vim_indent_visual_character_lines, vim_join_visual_character_lines,
    vim_outdent_visual_character_lines, vim_visual_character_case_conversion,
    vim_visual_character_change_key, vim_visual_character_delete_key,
    vim_visual_character_indent_key, vim_visual_character_join_key,
    vim_visual_character_join_repeat_count, vim_visual_character_line_repeat_count,
    vim_visual_character_outdent_key, vim_visual_character_repeat_count,
    vim_visual_character_replace_key, vim_visual_character_yank_key, vim_yank_visual_character,
};

pub(in crate::editor_vim_key_events) fn handle_vim_visual_character_action_key_event(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    mode: &mut EditorVimMode,
    pending: &mut Option<EditorVimPendingKey>,
    unnamed_register: &mut Option<EditorVimRegister>,
    last_change: &mut Option<EditorVimLastChange>,
    anchor: usize,
    cursor: usize,
    indent_unit: &str,
    suppress_text: Option<char>,
) -> Option<VimKeyResult> {
    if vim_visual_character_yank_key(key, modifiers) {
        vim_yank_visual_character(buffer, anchor, cursor, unnamed_register);
        *pending = None;
        return Some(VimKeyResult::handled(suppress_text));
    }
    if vim_visual_character_join_key(key, modifiers) {
        let repeat_count = vim_visual_character_join_repeat_count(buffer, anchor, cursor);
        let changed = vim_join_visual_character_lines(buffer, anchor, cursor);
        *pending = None;
        return Some(vim_repeatable_change_result(
            changed,
            last_change,
            EditorVimRepeatAction::JoinLines,
            repeat_count,
            suppress_text,
        ));
    }
    if vim_visual_character_indent_key(key, modifiers) {
        let repeat_count = vim_visual_character_line_repeat_count(buffer, anchor, cursor);
        let changed = vim_indent_visual_character_lines(buffer, anchor, cursor, indent_unit);
        *pending = None;
        return Some(vim_repeatable_change_result(
            changed,
            last_change,
            EditorVimRepeatAction::IndentLines,
            repeat_count,
            suppress_text,
        ));
    }
    if vim_visual_character_outdent_key(key, modifiers) {
        let repeat_count = vim_visual_character_line_repeat_count(buffer, anchor, cursor);
        let changed = vim_outdent_visual_character_lines(buffer, anchor, cursor, indent_unit);
        *pending = None;
        return Some(vim_repeatable_change_result(
            changed,
            last_change,
            EditorVimRepeatAction::OutdentLines,
            repeat_count,
            suppress_text,
        ));
    }
    if let Some(conversion) = vim_visual_character_case_conversion(key, modifiers) {
        let repeat_count = vim_visual_character_repeat_count(buffer, anchor, cursor);
        let changed = vim_convert_case_visual_character(buffer, anchor, cursor, conversion);
        *pending = None;
        return Some(vim_repeatable_change_result(
            changed,
            last_change,
            EditorVimRepeatAction::ConvertCaseForwardChars(conversion),
            repeat_count,
            suppress_text,
        ));
    }
    if vim_visual_character_delete_key(key, modifiers) {
        let changed = vim_delete_visual_character(buffer, anchor, cursor, unnamed_register);
        *pending = None;
        return Some(if changed {
            VimKeyResult::changed(suppress_text)
        } else {
            VimKeyResult::handled(suppress_text)
        });
    }
    if vim_visual_character_change_key(key, modifiers) {
        let repeat_count = vim_visual_character_repeat_count(buffer, anchor, cursor);
        let changed = vim_delete_visual_character(buffer, anchor, cursor, unnamed_register);
        *pending = None;
        return Some(if changed {
            *mode = EditorVimMode::Insert;
            vim_repeatable_change_result(
                changed,
                last_change,
                EditorVimRepeatAction::SubstituteForwardChars,
                repeat_count,
                suppress_text,
            )
        } else {
            VimKeyResult::handled(suppress_text)
        });
    }
    if vim_visual_character_replace_key(key, modifiers) {
        *pending = Some(EditorVimPendingKey::VisualCharacterReplace { anchor, cursor });
        return Some(VimKeyResult::handled(suppress_text));
    }

    None
}
