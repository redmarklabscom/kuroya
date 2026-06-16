mod simple;

use self::simple::handle_vim_simple_pending_key_event;
use super::direct::handle_vim_direct_normal_key_event;
use super::*;

pub(super) fn handle_vim_pending_or_direct_normal_key_event(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    mode: &mut EditorVimMode,
    pending: &mut Option<EditorVimPendingKey>,
    last_char_find: &mut Option<EditorVimCharFind>,
    unnamed_register: &mut Option<EditorVimRegister>,
    last_change: &mut Option<EditorVimLastChange>,
    indent_unit: &str,
    count: Option<usize>,
    count_value: usize,
    suppress_text: Option<char>,
) -> VimKeyResult {
    if let Some(pending_key) = pending.take() {
        if let Some(next_pending) =
            vim_pending_key_next_operator_go(Some(pending_key), key, modifiers)
        {
            *pending = Some(next_pending);
            return VimKeyResult::handled(suppress_text);
        }
        if let Some(result) = handle_vim_simple_pending_key_event(
            buffer,
            key,
            modifiers,
            pending,
            last_char_find,
            last_change,
            indent_unit,
            pending_key,
            suppress_text,
        ) {
            return result;
        }
        match (pending_key, key) {
            (EditorVimPendingKey::SearchInput { count, forward }, key) => {
                return handle_vim_search_input_key_event(
                    buffer,
                    key,
                    modifiers,
                    pending,
                    count,
                    forward,
                    suppress_text,
                );
            }
            (
                EditorVimPendingKey::OperatorGoMotion {
                    operator_count,
                    motion_count,
                    operator,
                },
                key,
            ) => {
                if let Some(motion) = vim_operator_go_motion_for_key(key, modifiers) {
                    return handle_vim_operator_go_motion_key_event(
                        buffer,
                        mode,
                        unnamed_register,
                        last_change,
                        operator_count,
                        motion_count,
                        operator,
                        motion,
                        suppress_text,
                    );
                }
            }
            (EditorVimPendingKey::VisualCharacter { anchor, cursor }, key) => {
                return handle_vim_visual_character_key_event(
                    buffer,
                    key,
                    modifiers,
                    mode,
                    pending,
                    last_char_find,
                    unnamed_register,
                    last_change,
                    anchor,
                    cursor,
                    None,
                    indent_unit,
                    suppress_text,
                );
            }
            (
                EditorVimPendingKey::VisualCharacterCount {
                    anchor,
                    cursor,
                    count,
                },
                key,
            ) => {
                return handle_vim_visual_character_key_event(
                    buffer,
                    key,
                    modifiers,
                    mode,
                    pending,
                    last_char_find,
                    unnamed_register,
                    last_change,
                    anchor,
                    cursor,
                    Some(count),
                    indent_unit,
                    suppress_text,
                );
            }
            (
                EditorVimPendingKey::VisualCharacterGo {
                    anchor,
                    cursor,
                    count,
                },
                key,
            ) => {
                return handle_vim_visual_character_go_key_event(
                    buffer,
                    key,
                    modifiers,
                    mode,
                    pending,
                    last_char_find,
                    unnamed_register,
                    last_change,
                    anchor,
                    cursor,
                    count,
                    indent_unit,
                    suppress_text,
                );
            }
            (EditorVimPendingKey::VisualCharacterReplace { anchor, cursor }, key) => {
                return handle_vim_visual_character_replace_key_event(
                    buffer,
                    key,
                    modifiers,
                    pending,
                    last_change,
                    anchor,
                    cursor,
                    suppress_text,
                );
            }
            (
                EditorVimPendingKey::VisualCharacterCharFind {
                    anchor,
                    cursor,
                    count,
                    motion,
                },
                key,
            ) => {
                return handle_vim_visual_character_char_find_key_event(
                    buffer,
                    key,
                    modifiers,
                    pending,
                    last_char_find,
                    anchor,
                    cursor,
                    count,
                    motion,
                    suppress_text,
                );
            }
            (
                EditorVimPendingKey::VisualCharacterRegisterPrefix {
                    anchor,
                    cursor,
                    count,
                },
                key,
            ) => {
                return handle_vim_visual_character_register_prefix_key_event(
                    buffer,
                    key,
                    modifiers,
                    pending,
                    anchor,
                    cursor,
                    count,
                    suppress_text,
                );
            }
            (
                EditorVimPendingKey::VisualCharacterRegisterCommand {
                    anchor,
                    cursor,
                    count,
                    register,
                },
                key,
            ) => {
                return handle_vim_visual_character_register_command_key_event(
                    buffer,
                    key,
                    modifiers,
                    mode,
                    pending,
                    unnamed_register,
                    last_change,
                    anchor,
                    cursor,
                    count,
                    register,
                    suppress_text,
                );
            }
            (
                EditorVimPendingKey::VisualCharacterTextObject {
                    anchor,
                    cursor,
                    count,
                    scope,
                },
                key,
            ) => {
                return handle_vim_visual_character_text_object_key_event(
                    buffer,
                    key,
                    modifiers,
                    pending,
                    anchor,
                    cursor,
                    count,
                    scope,
                    suppress_text,
                );
            }
            (EditorVimPendingKey::RegisterPrefix(count), _) => {
                if let Some(register) = vim_named_register_for_key(key, modifiers) {
                    *pending = Some(EditorVimPendingKey::RegisterCommand {
                        prefix_count: count,
                        command_count: None,
                        register,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
            }
            (
                EditorVimPendingKey::RegisterCommand {
                    prefix_count,
                    command_count,
                    register,
                },
                key,
            ) => {
                if let Some(command_count) =
                    vim_register_command_next_count(command_count, key, modifiers)
                {
                    *pending = Some(EditorVimPendingKey::RegisterCommand {
                        prefix_count,
                        command_count: Some(command_count),
                        register,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                let count = vim_register_command_count(prefix_count, command_count);
                match (key, modifiers.shift) {
                    (Key::C, false) => {
                        *pending = Some(EditorVimPendingKey::ChangeLineIntoRegister {
                            operator_count: count,
                            register,
                        });
                        return VimKeyResult::handled(suppress_text);
                    }
                    (Key::C, true) => {
                        let changed = vim_delete_to_line_end_into_named_register(
                            buffer,
                            count,
                            unnamed_register,
                            register,
                        );
                        *pending = None;
                        *mode = EditorVimMode::Insert;
                        return vim_repeatable_change_result(
                            changed,
                            last_change,
                            EditorVimRepeatAction::ChangeToLineEndIntoRegister(register),
                            count,
                            suppress_text,
                        );
                    }
                    (Key::D, false) => {
                        *pending = Some(EditorVimPendingKey::DeleteLineIntoRegister {
                            operator_count: count,
                            register,
                        });
                        return VimKeyResult::handled(suppress_text);
                    }
                    (Key::D, true) => {
                        return vim_repeatable_change_result(
                            vim_delete_to_line_end_into_named_register(
                                buffer,
                                count,
                                unnamed_register,
                                register,
                            ),
                            last_change,
                            EditorVimRepeatAction::DeleteToLineEndIntoRegister(register),
                            count,
                            suppress_text,
                        );
                    }
                    (Key::Y, false) => {
                        *pending = Some(EditorVimPendingKey::YankLineIntoRegister {
                            operator_count: count,
                            register,
                        });
                        return VimKeyResult::handled(suppress_text);
                    }
                    (Key::Y, true) => {
                        vim_yank_lines_into_named_register(
                            buffer,
                            count,
                            unnamed_register,
                            register,
                        );
                        return VimKeyResult::handled(suppress_text);
                    }
                    (Key::P, false) => {
                        let named_register = vim_named_register(register);
                        return vim_repeatable_change_result(
                            vim_put_register_after(buffer, named_register.as_ref(), count),
                            last_change,
                            EditorVimRepeatAction::PutAfterNamed(register),
                            count,
                            suppress_text,
                        );
                    }
                    (Key::P, true) => {
                        let named_register = vim_named_register(register);
                        return vim_repeatable_change_result(
                            vim_put_register_before(buffer, named_register.as_ref(), count),
                            last_change,
                            EditorVimRepeatAction::PutBeforeNamed(register),
                            count,
                            suppress_text,
                        );
                    }
                    (Key::S, false) => {
                        let changed = vim_delete_forward_chars_into_named_register(
                            buffer,
                            count,
                            unnamed_register,
                            register,
                        );
                        *pending = None;
                        *mode = EditorVimMode::Insert;
                        return vim_repeatable_change_result(
                            changed,
                            last_change,
                            EditorVimRepeatAction::SubstituteForwardCharsIntoRegister(register),
                            count,
                            suppress_text,
                        );
                    }
                    (Key::S, true) => {
                        let changed = vim_delete_lines_into_named_register(
                            buffer,
                            count,
                            unnamed_register,
                            register,
                        );
                        *pending = None;
                        *mode = EditorVimMode::Insert;
                        return vim_repeatable_change_result(
                            changed,
                            last_change,
                            EditorVimRepeatAction::ChangeLinesIntoRegister(register),
                            count,
                            suppress_text,
                        );
                    }
                    (Key::X, false) => {
                        return vim_repeatable_change_result(
                            vim_delete_forward_chars_into_named_register(
                                buffer,
                                count,
                                unnamed_register,
                                register,
                            ),
                            last_change,
                            EditorVimRepeatAction::DeleteForwardCharsIntoRegister(register),
                            count,
                            suppress_text,
                        );
                    }
                    (Key::X, true) => {
                        return vim_repeatable_change_result(
                            vim_delete_backward_chars_into_named_register(
                                buffer,
                                count,
                                unnamed_register,
                                register,
                            ),
                            last_change,
                            EditorVimRepeatAction::DeleteBackwardCharsIntoRegister(register),
                            count,
                            suppress_text,
                        );
                    }
                    _ => {}
                }
            }
            (EditorVimPendingKey::ChangeLine(count), Key::C) if !modifiers.shift => {
                let changed = vim_delete_lines_into_register(buffer, count, unnamed_register);
                *mode = EditorVimMode::Insert;
                return vim_repeatable_change_result(
                    changed,
                    last_change,
                    EditorVimRepeatAction::ChangeLines,
                    count,
                    suppress_text,
                );
            }
            (
                EditorVimPendingKey::ChangeLineIntoRegister {
                    operator_count,
                    register,
                },
                Key::C,
            ) if !modifiers.shift => {
                let changed = vim_delete_lines_into_named_register(
                    buffer,
                    operator_count,
                    unnamed_register,
                    register,
                );
                *mode = EditorVimMode::Insert;
                return vim_repeatable_change_result(
                    changed,
                    last_change,
                    EditorVimRepeatAction::ChangeLinesIntoRegister(register),
                    operator_count,
                    suppress_text,
                );
            }
            (
                EditorVimPendingKey::ChangeLineIntoRegister {
                    operator_count,
                    register,
                },
                key,
            ) => {
                if let Some(digit) = vim_count_digit(key, modifiers, false) {
                    *pending = Some(EditorVimPendingKey::ChangeMotionCountIntoRegister {
                        operator_count,
                        motion_count: digit,
                        register,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(scope) = vim_text_object_scope_for_key(key, modifiers) {
                    *pending = Some(EditorVimPendingKey::ChangeTextObjectIntoRegister {
                        operator_count,
                        motion_count: 1,
                        scope,
                        register,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(motion) = vim_operator_char_find_motion_for_key(key, modifiers) {
                    *pending = Some(EditorVimPendingKey::ChangeCharFindIntoRegister {
                        operator_count,
                        motion_count: 1,
                        motion,
                        register,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(motion) = vim_operator_motion_for_key(key, modifiers) {
                    let count = vim_combined_count(operator_count, 1);
                    let changed = vim_apply_operator_motion_into_named_register(
                        buffer,
                        operator_count,
                        1,
                        motion,
                        unnamed_register,
                        register,
                    );
                    *mode = EditorVimMode::Insert;
                    return vim_repeatable_change_result(
                        changed,
                        last_change,
                        EditorVimRepeatAction::ChangeOperatorMotionIntoRegister {
                            motion,
                            register,
                        },
                        count,
                        suppress_text,
                    );
                }
            }
            (EditorVimPendingKey::DeleteLine(count), Key::D) if !modifiers.shift => {
                return vim_repeatable_change_result(
                    vim_delete_lines_into_register(buffer, count, unnamed_register),
                    last_change,
                    EditorVimRepeatAction::DeleteLines,
                    count,
                    suppress_text,
                );
            }
            (
                EditorVimPendingKey::DeleteLineIntoRegister {
                    operator_count,
                    register,
                },
                Key::D,
            ) if !modifiers.shift => {
                return vim_repeatable_change_result(
                    vim_delete_lines_into_named_register(
                        buffer,
                        operator_count,
                        unnamed_register,
                        register,
                    ),
                    last_change,
                    EditorVimRepeatAction::DeleteLinesIntoRegister(register),
                    operator_count,
                    suppress_text,
                );
            }
            (EditorVimPendingKey::ChangeLine(operator_count), key) => {
                if let Some(digit) = vim_count_digit(key, modifiers, false) {
                    *pending = Some(EditorVimPendingKey::ChangeMotionCount {
                        operator_count,
                        motion_count: digit,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(scope) = vim_text_object_scope_for_key(key, modifiers) {
                    *pending = Some(EditorVimPendingKey::ChangeTextObject {
                        operator_count,
                        motion_count: 1,
                        scope,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(motion) = vim_operator_char_find_motion_for_key(key, modifiers) {
                    *pending = Some(EditorVimPendingKey::ChangeCharFind {
                        operator_count,
                        motion_count: 1,
                        motion,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(motion) = vim_operator_motion_for_key(key, modifiers) {
                    let count = vim_combined_count(operator_count, 1);
                    let changed = vim_apply_operator_motion(
                        buffer,
                        operator_count,
                        1,
                        motion,
                        unnamed_register,
                    );
                    *mode = EditorVimMode::Insert;
                    return vim_repeatable_change_result(
                        changed,
                        last_change,
                        EditorVimRepeatAction::ChangeOperatorMotion(motion),
                        count,
                        suppress_text,
                    );
                }
            }
            (EditorVimPendingKey::DeleteLine(operator_count), key) => {
                if let Some(digit) = vim_count_digit(key, modifiers, false) {
                    *pending = Some(EditorVimPendingKey::DeleteMotionCount {
                        operator_count,
                        motion_count: digit,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(scope) = vim_text_object_scope_for_key(key, modifiers) {
                    *pending = Some(EditorVimPendingKey::DeleteTextObject {
                        operator_count,
                        motion_count: 1,
                        scope,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(motion) = vim_operator_char_find_motion_for_key(key, modifiers) {
                    *pending = Some(EditorVimPendingKey::DeleteCharFind {
                        operator_count,
                        motion_count: 1,
                        motion,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(motion) = vim_operator_motion_for_key(key, modifiers) {
                    let count = vim_combined_count(operator_count, 1);
                    return vim_repeatable_change_result(
                        vim_apply_operator_motion(
                            buffer,
                            operator_count,
                            1,
                            motion,
                            unnamed_register,
                        ),
                        last_change,
                        EditorVimRepeatAction::DeleteOperatorMotion(motion),
                        count,
                        suppress_text,
                    );
                }
            }
            (
                EditorVimPendingKey::DeleteLineIntoRegister {
                    operator_count,
                    register,
                },
                key,
            ) => {
                if let Some(digit) = vim_count_digit(key, modifiers, false) {
                    *pending = Some(EditorVimPendingKey::DeleteMotionCountIntoRegister {
                        operator_count,
                        motion_count: digit,
                        register,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(scope) = vim_text_object_scope_for_key(key, modifiers) {
                    *pending = Some(EditorVimPendingKey::DeleteTextObjectIntoRegister {
                        operator_count,
                        motion_count: 1,
                        scope,
                        register,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(motion) = vim_operator_char_find_motion_for_key(key, modifiers) {
                    *pending = Some(EditorVimPendingKey::DeleteCharFindIntoRegister {
                        operator_count,
                        motion_count: 1,
                        motion,
                        register,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(motion) = vim_operator_motion_for_key(key, modifiers) {
                    let count = vim_combined_count(operator_count, 1);
                    return vim_repeatable_change_result(
                        vim_apply_operator_motion_into_named_register(
                            buffer,
                            operator_count,
                            1,
                            motion,
                            unnamed_register,
                            register,
                        ),
                        last_change,
                        EditorVimRepeatAction::DeleteOperatorMotionIntoRegister {
                            motion,
                            register,
                        },
                        count,
                        suppress_text,
                    );
                }
            }
            (
                EditorVimPendingKey::ChangeMotionCount {
                    operator_count,
                    motion_count,
                },
                key,
            ) => {
                if let Some(digit) = vim_count_digit(key, modifiers, true) {
                    *pending = Some(EditorVimPendingKey::ChangeMotionCount {
                        operator_count,
                        motion_count: vim_push_count_digit(motion_count, digit),
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(scope) = vim_text_object_scope_for_key(key, modifiers) {
                    *pending = Some(EditorVimPendingKey::ChangeTextObject {
                        operator_count,
                        motion_count,
                        scope,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(motion) = vim_operator_char_find_motion_for_key(key, modifiers) {
                    *pending = Some(EditorVimPendingKey::ChangeCharFind {
                        operator_count,
                        motion_count,
                        motion,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(motion) = vim_operator_motion_for_key(key, modifiers) {
                    let count = vim_combined_count(operator_count, motion_count);
                    let changed = vim_apply_operator_motion(
                        buffer,
                        operator_count,
                        motion_count,
                        motion,
                        unnamed_register,
                    );
                    *mode = EditorVimMode::Insert;
                    return vim_repeatable_change_result(
                        changed,
                        last_change,
                        EditorVimRepeatAction::ChangeOperatorMotion(motion),
                        count,
                        suppress_text,
                    );
                }
            }
            (
                EditorVimPendingKey::ChangeMotionCountIntoRegister {
                    operator_count,
                    motion_count,
                    register,
                },
                key,
            ) => {
                if let Some(digit) = vim_count_digit(key, modifiers, true) {
                    *pending = Some(EditorVimPendingKey::ChangeMotionCountIntoRegister {
                        operator_count,
                        motion_count: vim_push_count_digit(motion_count, digit),
                        register,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(scope) = vim_text_object_scope_for_key(key, modifiers) {
                    *pending = Some(EditorVimPendingKey::ChangeTextObjectIntoRegister {
                        operator_count,
                        motion_count,
                        scope,
                        register,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(motion) = vim_operator_char_find_motion_for_key(key, modifiers) {
                    *pending = Some(EditorVimPendingKey::ChangeCharFindIntoRegister {
                        operator_count,
                        motion_count,
                        motion,
                        register,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(motion) = vim_operator_motion_for_key(key, modifiers) {
                    let count = vim_combined_count(operator_count, motion_count);
                    let changed = vim_apply_operator_motion_into_named_register(
                        buffer,
                        operator_count,
                        motion_count,
                        motion,
                        unnamed_register,
                        register,
                    );
                    *mode = EditorVimMode::Insert;
                    return vim_repeatable_change_result(
                        changed,
                        last_change,
                        EditorVimRepeatAction::ChangeOperatorMotionIntoRegister {
                            motion,
                            register,
                        },
                        count,
                        suppress_text,
                    );
                }
            }
            (
                EditorVimPendingKey::DeleteMotionCount {
                    operator_count,
                    motion_count,
                },
                key,
            ) => {
                if let Some(digit) = vim_count_digit(key, modifiers, true) {
                    *pending = Some(EditorVimPendingKey::DeleteMotionCount {
                        operator_count,
                        motion_count: vim_push_count_digit(motion_count, digit),
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(scope) = vim_text_object_scope_for_key(key, modifiers) {
                    *pending = Some(EditorVimPendingKey::DeleteTextObject {
                        operator_count,
                        motion_count,
                        scope,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(motion) = vim_operator_char_find_motion_for_key(key, modifiers) {
                    *pending = Some(EditorVimPendingKey::DeleteCharFind {
                        operator_count,
                        motion_count,
                        motion,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(motion) = vim_operator_motion_for_key(key, modifiers) {
                    let count = vim_combined_count(operator_count, motion_count);
                    return vim_repeatable_change_result(
                        vim_apply_operator_motion(
                            buffer,
                            operator_count,
                            motion_count,
                            motion,
                            unnamed_register,
                        ),
                        last_change,
                        EditorVimRepeatAction::DeleteOperatorMotion(motion),
                        count,
                        suppress_text,
                    );
                }
            }
            (
                EditorVimPendingKey::DeleteMotionCountIntoRegister {
                    operator_count,
                    motion_count,
                    register,
                },
                key,
            ) => {
                if let Some(digit) = vim_count_digit(key, modifiers, true) {
                    *pending = Some(EditorVimPendingKey::DeleteMotionCountIntoRegister {
                        operator_count,
                        motion_count: vim_push_count_digit(motion_count, digit),
                        register,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(scope) = vim_text_object_scope_for_key(key, modifiers) {
                    *pending = Some(EditorVimPendingKey::DeleteTextObjectIntoRegister {
                        operator_count,
                        motion_count,
                        scope,
                        register,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(motion) = vim_operator_char_find_motion_for_key(key, modifiers) {
                    *pending = Some(EditorVimPendingKey::DeleteCharFindIntoRegister {
                        operator_count,
                        motion_count,
                        motion,
                        register,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(motion) = vim_operator_motion_for_key(key, modifiers) {
                    let count = vim_combined_count(operator_count, motion_count);
                    return vim_repeatable_change_result(
                        vim_apply_operator_motion_into_named_register(
                            buffer,
                            operator_count,
                            motion_count,
                            motion,
                            unnamed_register,
                            register,
                        ),
                        last_change,
                        EditorVimRepeatAction::DeleteOperatorMotionIntoRegister {
                            motion,
                            register,
                        },
                        count,
                        suppress_text,
                    );
                }
            }
            (
                EditorVimPendingKey::ChangeTextObject {
                    operator_count,
                    motion_count,
                    scope,
                },
                key,
            ) => {
                if let Some(kind) = vim_text_object_kind_for_key(key, modifiers) {
                    let count = vim_combined_count(operator_count, motion_count);
                    let changed = vim_apply_text_object(
                        buffer,
                        operator_count,
                        motion_count,
                        scope,
                        kind,
                        unnamed_register,
                    );
                    *mode = EditorVimMode::Insert;
                    return vim_repeatable_change_result(
                        changed,
                        last_change,
                        EditorVimRepeatAction::ChangeTextObject { scope, kind },
                        count,
                        suppress_text,
                    );
                }
            }
            (
                EditorVimPendingKey::ChangeTextObjectIntoRegister {
                    operator_count,
                    motion_count,
                    scope,
                    register,
                },
                key,
            ) => {
                if let Some(kind) = vim_text_object_kind_for_key(key, modifiers) {
                    let count = vim_combined_count(operator_count, motion_count);
                    let changed = vim_apply_text_object_into_named_register(
                        buffer,
                        operator_count,
                        motion_count,
                        scope,
                        kind,
                        unnamed_register,
                        register,
                    );
                    *mode = EditorVimMode::Insert;
                    return vim_repeatable_change_result(
                        changed,
                        last_change,
                        EditorVimRepeatAction::ChangeTextObjectIntoRegister {
                            scope,
                            kind,
                            register,
                        },
                        count,
                        suppress_text,
                    );
                }
            }
            (
                EditorVimPendingKey::DeleteTextObject {
                    operator_count,
                    motion_count,
                    scope,
                },
                key,
            ) => {
                if let Some(kind) = vim_text_object_kind_for_key(key, modifiers) {
                    let count = vim_combined_count(operator_count, motion_count);
                    return vim_repeatable_change_result(
                        vim_apply_text_object(
                            buffer,
                            operator_count,
                            motion_count,
                            scope,
                            kind,
                            unnamed_register,
                        ),
                        last_change,
                        EditorVimRepeatAction::DeleteTextObject { scope, kind },
                        count,
                        suppress_text,
                    );
                }
            }
            (
                EditorVimPendingKey::DeleteTextObjectIntoRegister {
                    operator_count,
                    motion_count,
                    scope,
                    register,
                },
                key,
            ) => {
                if let Some(kind) = vim_text_object_kind_for_key(key, modifiers) {
                    let count = vim_combined_count(operator_count, motion_count);
                    return vim_repeatable_change_result(
                        vim_apply_text_object_into_named_register(
                            buffer,
                            operator_count,
                            motion_count,
                            scope,
                            kind,
                            unnamed_register,
                            register,
                        ),
                        last_change,
                        EditorVimRepeatAction::DeleteTextObjectIntoRegister {
                            scope,
                            kind,
                            register,
                        },
                        count,
                        suppress_text,
                    );
                }
            }
            (
                EditorVimPendingKey::ChangeCharFind {
                    operator_count,
                    motion_count,
                    motion,
                },
                _,
            ) => {
                if let Some(target) = vim_printable_key_char(key, modifiers) {
                    let count = vim_combined_count(operator_count, motion_count);
                    let operator_motion = EditorVimOperatorMotion::CharFind { motion, target };
                    *last_char_find = Some(EditorVimCharFind { motion, target });
                    let changed = vim_apply_operator_motion(
                        buffer,
                        operator_count,
                        motion_count,
                        operator_motion,
                        unnamed_register,
                    );
                    *mode = EditorVimMode::Insert;
                    return vim_repeatable_change_result(
                        changed,
                        last_change,
                        EditorVimRepeatAction::ChangeOperatorMotion(operator_motion),
                        count,
                        suppress_text,
                    );
                }
            }
            (
                EditorVimPendingKey::ChangeCharFindIntoRegister {
                    operator_count,
                    motion_count,
                    motion,
                    register,
                },
                _,
            ) => {
                if let Some(target) = vim_printable_key_char(key, modifiers) {
                    let count = vim_combined_count(operator_count, motion_count);
                    let operator_motion = EditorVimOperatorMotion::CharFind { motion, target };
                    *last_char_find = Some(EditorVimCharFind { motion, target });
                    let changed = vim_apply_operator_motion_into_named_register(
                        buffer,
                        operator_count,
                        motion_count,
                        operator_motion,
                        unnamed_register,
                        register,
                    );
                    *mode = EditorVimMode::Insert;
                    return vim_repeatable_change_result(
                        changed,
                        last_change,
                        EditorVimRepeatAction::ChangeOperatorMotionIntoRegister {
                            motion: operator_motion,
                            register,
                        },
                        count,
                        suppress_text,
                    );
                }
            }
            (
                EditorVimPendingKey::DeleteCharFind {
                    operator_count,
                    motion_count,
                    motion,
                },
                _,
            ) => {
                if let Some(target) = vim_printable_key_char(key, modifiers) {
                    let count = vim_combined_count(operator_count, motion_count);
                    let operator_motion = EditorVimOperatorMotion::CharFind { motion, target };
                    *last_char_find = Some(EditorVimCharFind { motion, target });
                    return vim_repeatable_change_result(
                        vim_apply_operator_motion(
                            buffer,
                            operator_count,
                            motion_count,
                            operator_motion,
                            unnamed_register,
                        ),
                        last_change,
                        EditorVimRepeatAction::DeleteOperatorMotion(operator_motion),
                        count,
                        suppress_text,
                    );
                }
            }
            (
                EditorVimPendingKey::DeleteCharFindIntoRegister {
                    operator_count,
                    motion_count,
                    motion,
                    register,
                },
                _,
            ) => {
                if let Some(target) = vim_printable_key_char(key, modifiers) {
                    let count = vim_combined_count(operator_count, motion_count);
                    let operator_motion = EditorVimOperatorMotion::CharFind { motion, target };
                    *last_char_find = Some(EditorVimCharFind { motion, target });
                    return vim_repeatable_change_result(
                        vim_apply_operator_motion_into_named_register(
                            buffer,
                            operator_count,
                            motion_count,
                            operator_motion,
                            unnamed_register,
                            register,
                        ),
                        last_change,
                        EditorVimRepeatAction::DeleteOperatorMotionIntoRegister {
                            motion: operator_motion,
                            register,
                        },
                        count,
                        suppress_text,
                    );
                }
            }
            (
                EditorVimPendingKey::ConvertCaseOperator {
                    operator_count,
                    conversion,
                },
                key,
            ) => {
                if vim_case_conversion_repeated_operator_key(conversion, key, modifiers) {
                    let count = vim_combined_count(operator_count, 1);
                    return vim_repeatable_change_result(
                        vim_convert_case_lines(buffer, count, conversion),
                        last_change,
                        EditorVimRepeatAction::ConvertCaseLines(conversion),
                        count,
                        suppress_text,
                    );
                }
                if key == Key::U && !modifiers.command && !modifiers.alt && !modifiers.ctrl {
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(digit) = vim_count_digit(key, modifiers, false) {
                    *pending = Some(EditorVimPendingKey::ConvertCaseMotionCount {
                        operator_count,
                        motion_count: digit,
                        conversion,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(scope) = vim_text_object_scope_for_key(key, modifiers) {
                    *pending = Some(EditorVimPendingKey::ConvertCaseTextObject {
                        operator_count,
                        motion_count: 1,
                        scope,
                        conversion,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(motion) = vim_operator_char_find_motion_for_key(key, modifiers) {
                    *pending = Some(EditorVimPendingKey::ConvertCaseCharFind {
                        operator_count,
                        motion_count: 1,
                        motion,
                        conversion,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(motion) = vim_operator_motion_for_key(key, modifiers) {
                    let count = vim_combined_count(operator_count, 1);
                    return vim_repeatable_change_result(
                        vim_convert_case_operator_motion(
                            buffer,
                            operator_count,
                            1,
                            motion,
                            conversion,
                        ),
                        last_change,
                        EditorVimRepeatAction::ConvertCaseOperatorMotion { motion, conversion },
                        count,
                        suppress_text,
                    );
                }
            }
            (
                EditorVimPendingKey::ConvertCaseMotionCount {
                    operator_count,
                    motion_count,
                    conversion,
                },
                key,
            ) => {
                if vim_case_conversion_repeated_operator_key(conversion, key, modifiers) {
                    let count = vim_combined_count(operator_count, motion_count);
                    return vim_repeatable_change_result(
                        vim_convert_case_lines(buffer, count, conversion),
                        last_change,
                        EditorVimRepeatAction::ConvertCaseLines(conversion),
                        count,
                        suppress_text,
                    );
                }
                if key == Key::U && !modifiers.command && !modifiers.alt && !modifiers.ctrl {
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(digit) = vim_count_digit(key, modifiers, true) {
                    *pending = Some(EditorVimPendingKey::ConvertCaseMotionCount {
                        operator_count,
                        motion_count: vim_push_count_digit(motion_count, digit),
                        conversion,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(scope) = vim_text_object_scope_for_key(key, modifiers) {
                    *pending = Some(EditorVimPendingKey::ConvertCaseTextObject {
                        operator_count,
                        motion_count,
                        scope,
                        conversion,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(motion) = vim_operator_char_find_motion_for_key(key, modifiers) {
                    *pending = Some(EditorVimPendingKey::ConvertCaseCharFind {
                        operator_count,
                        motion_count,
                        motion,
                        conversion,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(motion) = vim_operator_motion_for_key(key, modifiers) {
                    let count = vim_combined_count(operator_count, motion_count);
                    return vim_repeatable_change_result(
                        vim_convert_case_operator_motion(
                            buffer,
                            operator_count,
                            motion_count,
                            motion,
                            conversion,
                        ),
                        last_change,
                        EditorVimRepeatAction::ConvertCaseOperatorMotion { motion, conversion },
                        count,
                        suppress_text,
                    );
                }
            }
            (
                EditorVimPendingKey::ConvertCaseCharFind {
                    operator_count,
                    motion_count,
                    motion,
                    conversion,
                },
                _,
            ) => {
                if let Some(target) = vim_printable_key_char(key, modifiers) {
                    let count = vim_combined_count(operator_count, motion_count);
                    let operator_motion = EditorVimOperatorMotion::CharFind { motion, target };
                    *last_char_find = Some(EditorVimCharFind { motion, target });
                    return vim_repeatable_change_result(
                        vim_convert_case_operator_motion(
                            buffer,
                            operator_count,
                            motion_count,
                            operator_motion,
                            conversion,
                        ),
                        last_change,
                        EditorVimRepeatAction::ConvertCaseOperatorMotion {
                            motion: operator_motion,
                            conversion,
                        },
                        count,
                        suppress_text,
                    );
                }
            }
            (
                EditorVimPendingKey::ConvertCaseTextObject {
                    operator_count,
                    motion_count,
                    scope,
                    conversion,
                },
                key,
            ) => {
                if let Some(kind) = vim_text_object_kind_for_key(key, modifiers) {
                    let count = vim_combined_count(operator_count, motion_count);
                    return vim_repeatable_change_result(
                        vim_convert_case_text_object(
                            buffer,
                            operator_count,
                            motion_count,
                            scope,
                            kind,
                            conversion,
                        ),
                        last_change,
                        EditorVimRepeatAction::ConvertCaseTextObject {
                            scope,
                            kind,
                            conversion,
                        },
                        count,
                        suppress_text,
                    );
                }
            }
            (EditorVimPendingKey::ToggleCaseOperator(operator_count), key) => {
                if let Some(digit) = vim_count_digit(key, modifiers, false) {
                    *pending = Some(EditorVimPendingKey::ToggleCaseMotionCount {
                        operator_count,
                        motion_count: digit,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(scope) = vim_text_object_scope_for_key(key, modifiers) {
                    *pending = Some(EditorVimPendingKey::ToggleCaseTextObject {
                        operator_count,
                        motion_count: 1,
                        scope,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(motion) = vim_operator_char_find_motion_for_key(key, modifiers) {
                    *pending = Some(EditorVimPendingKey::ToggleCaseCharFind {
                        operator_count,
                        motion_count: 1,
                        motion,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(motion) = vim_operator_motion_for_key(key, modifiers) {
                    let count = vim_combined_count(operator_count, 1);
                    return vim_repeatable_change_result(
                        vim_toggle_case_operator_motion(buffer, operator_count, 1, motion),
                        last_change,
                        EditorVimRepeatAction::ToggleCaseOperatorMotion(motion),
                        count,
                        suppress_text,
                    );
                }
            }
            (
                EditorVimPendingKey::ToggleCaseMotionCount {
                    operator_count,
                    motion_count,
                },
                key,
            ) => {
                if let Some(digit) = vim_count_digit(key, modifiers, true) {
                    *pending = Some(EditorVimPendingKey::ToggleCaseMotionCount {
                        operator_count,
                        motion_count: vim_push_count_digit(motion_count, digit),
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(scope) = vim_text_object_scope_for_key(key, modifiers) {
                    *pending = Some(EditorVimPendingKey::ToggleCaseTextObject {
                        operator_count,
                        motion_count,
                        scope,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(motion) = vim_operator_char_find_motion_for_key(key, modifiers) {
                    *pending = Some(EditorVimPendingKey::ToggleCaseCharFind {
                        operator_count,
                        motion_count,
                        motion,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(motion) = vim_operator_motion_for_key(key, modifiers) {
                    let count = vim_combined_count(operator_count, motion_count);
                    return vim_repeatable_change_result(
                        vim_toggle_case_operator_motion(
                            buffer,
                            operator_count,
                            motion_count,
                            motion,
                        ),
                        last_change,
                        EditorVimRepeatAction::ToggleCaseOperatorMotion(motion),
                        count,
                        suppress_text,
                    );
                }
            }
            (
                EditorVimPendingKey::ToggleCaseCharFind {
                    operator_count,
                    motion_count,
                    motion,
                },
                _,
            ) => {
                if let Some(target) = vim_printable_key_char(key, modifiers) {
                    let count = vim_combined_count(operator_count, motion_count);
                    let operator_motion = EditorVimOperatorMotion::CharFind { motion, target };
                    *last_char_find = Some(EditorVimCharFind { motion, target });
                    return vim_repeatable_change_result(
                        vim_toggle_case_operator_motion(
                            buffer,
                            operator_count,
                            motion_count,
                            operator_motion,
                        ),
                        last_change,
                        EditorVimRepeatAction::ToggleCaseOperatorMotion(operator_motion),
                        count,
                        suppress_text,
                    );
                }
            }
            (
                EditorVimPendingKey::ToggleCaseTextObject {
                    operator_count,
                    motion_count,
                    scope,
                },
                key,
            ) => {
                if let Some(kind) = vim_text_object_kind_for_key(key, modifiers) {
                    let count = vim_combined_count(operator_count, motion_count);
                    return vim_repeatable_change_result(
                        vim_toggle_case_text_object(
                            buffer,
                            operator_count,
                            motion_count,
                            scope,
                            kind,
                        ),
                        last_change,
                        EditorVimRepeatAction::ToggleCaseTextObject { scope, kind },
                        count,
                        suppress_text,
                    );
                }
            }
            (EditorVimPendingKey::YankLine(count), Key::Y) if !modifiers.shift => {
                vim_yank_lines(buffer, count, unnamed_register);
                return VimKeyResult::handled(suppress_text);
            }
            (
                EditorVimPendingKey::YankLineIntoRegister {
                    operator_count,
                    register,
                },
                Key::Y,
            ) if !modifiers.shift => {
                vim_yank_lines_into_named_register(
                    buffer,
                    operator_count,
                    unnamed_register,
                    register,
                );
                return VimKeyResult::handled(suppress_text);
            }
            (EditorVimPendingKey::YankLine(operator_count), key) => {
                if let Some(digit) = vim_count_digit(key, modifiers, false) {
                    *pending = Some(EditorVimPendingKey::YankMotionCount {
                        operator_count,
                        motion_count: digit,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(scope) = vim_text_object_scope_for_key(key, modifiers) {
                    *pending = Some(EditorVimPendingKey::YankTextObject {
                        operator_count,
                        motion_count: 1,
                        scope,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(motion) = vim_operator_char_find_motion_for_key(key, modifiers) {
                    *pending = Some(EditorVimPendingKey::YankCharFind {
                        operator_count,
                        motion_count: 1,
                        motion,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(motion) = vim_operator_motion_for_key(key, modifiers) {
                    vim_yank_operator_motion(buffer, operator_count, 1, motion, unnamed_register);
                    return VimKeyResult::handled(suppress_text);
                }
            }
            (
                EditorVimPendingKey::YankLineIntoRegister {
                    operator_count,
                    register,
                },
                key,
            ) => {
                if let Some(digit) = vim_count_digit(key, modifiers, false) {
                    *pending = Some(EditorVimPendingKey::YankMotionCountIntoRegister {
                        operator_count,
                        motion_count: digit,
                        register,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(scope) = vim_text_object_scope_for_key(key, modifiers) {
                    *pending = Some(EditorVimPendingKey::YankTextObjectIntoRegister {
                        operator_count,
                        motion_count: 1,
                        scope,
                        register,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(motion) = vim_operator_char_find_motion_for_key(key, modifiers) {
                    *pending = Some(EditorVimPendingKey::YankCharFindIntoRegister {
                        operator_count,
                        motion_count: 1,
                        motion,
                        register,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(motion) = vim_operator_motion_for_key(key, modifiers) {
                    vim_yank_operator_motion_into_named_register(
                        buffer,
                        operator_count,
                        1,
                        motion,
                        unnamed_register,
                        register,
                    );
                    return VimKeyResult::handled(suppress_text);
                }
            }
            (
                EditorVimPendingKey::YankMotionCount {
                    operator_count,
                    motion_count,
                },
                key,
            ) => {
                if let Some(digit) = vim_count_digit(key, modifiers, true) {
                    *pending = Some(EditorVimPendingKey::YankMotionCount {
                        operator_count,
                        motion_count: vim_push_count_digit(motion_count, digit),
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(scope) = vim_text_object_scope_for_key(key, modifiers) {
                    *pending = Some(EditorVimPendingKey::YankTextObject {
                        operator_count,
                        motion_count,
                        scope,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(motion) = vim_operator_char_find_motion_for_key(key, modifiers) {
                    *pending = Some(EditorVimPendingKey::YankCharFind {
                        operator_count,
                        motion_count,
                        motion,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(motion) = vim_operator_motion_for_key(key, modifiers) {
                    vim_yank_operator_motion(
                        buffer,
                        operator_count,
                        motion_count,
                        motion,
                        unnamed_register,
                    );
                    return VimKeyResult::handled(suppress_text);
                }
            }
            (
                EditorVimPendingKey::YankMotionCountIntoRegister {
                    operator_count,
                    motion_count,
                    register,
                },
                key,
            ) => {
                if let Some(digit) = vim_count_digit(key, modifiers, true) {
                    *pending = Some(EditorVimPendingKey::YankMotionCountIntoRegister {
                        operator_count,
                        motion_count: vim_push_count_digit(motion_count, digit),
                        register,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(scope) = vim_text_object_scope_for_key(key, modifiers) {
                    *pending = Some(EditorVimPendingKey::YankTextObjectIntoRegister {
                        operator_count,
                        motion_count,
                        scope,
                        register,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(motion) = vim_operator_char_find_motion_for_key(key, modifiers) {
                    *pending = Some(EditorVimPendingKey::YankCharFindIntoRegister {
                        operator_count,
                        motion_count,
                        motion,
                        register,
                    });
                    return VimKeyResult::handled(suppress_text);
                }
                if let Some(motion) = vim_operator_motion_for_key(key, modifiers) {
                    vim_yank_operator_motion_into_named_register(
                        buffer,
                        operator_count,
                        motion_count,
                        motion,
                        unnamed_register,
                        register,
                    );
                    return VimKeyResult::handled(suppress_text);
                }
            }
            (
                EditorVimPendingKey::YankTextObject {
                    operator_count,
                    motion_count,
                    scope,
                },
                key,
            ) => {
                if let Some(kind) = vim_text_object_kind_for_key(key, modifiers) {
                    vim_yank_text_object(
                        buffer,
                        operator_count,
                        motion_count,
                        scope,
                        kind,
                        unnamed_register,
                    );
                    return VimKeyResult::handled(suppress_text);
                }
            }
            (
                EditorVimPendingKey::YankTextObjectIntoRegister {
                    operator_count,
                    motion_count,
                    scope,
                    register,
                },
                key,
            ) => {
                if let Some(kind) = vim_text_object_kind_for_key(key, modifiers) {
                    vim_yank_text_object_into_named_register(
                        buffer,
                        operator_count,
                        motion_count,
                        scope,
                        kind,
                        unnamed_register,
                        register,
                    );
                    return VimKeyResult::handled(suppress_text);
                }
            }
            (
                EditorVimPendingKey::YankCharFind {
                    operator_count,
                    motion_count,
                    motion,
                },
                _,
            ) => {
                if let Some(target) = vim_printable_key_char(key, modifiers) {
                    let operator_motion = EditorVimOperatorMotion::CharFind { motion, target };
                    *last_char_find = Some(EditorVimCharFind { motion, target });
                    vim_yank_operator_motion(
                        buffer,
                        operator_count,
                        motion_count,
                        operator_motion,
                        unnamed_register,
                    );
                    return VimKeyResult::handled(suppress_text);
                }
            }
            (
                EditorVimPendingKey::YankCharFindIntoRegister {
                    operator_count,
                    motion_count,
                    motion,
                    register,
                },
                _,
            ) => {
                if let Some(target) = vim_printable_key_char(key, modifiers) {
                    let operator_motion = EditorVimOperatorMotion::CharFind { motion, target };
                    *last_char_find = Some(EditorVimCharFind { motion, target });
                    vim_yank_operator_motion_into_named_register(
                        buffer,
                        operator_count,
                        motion_count,
                        operator_motion,
                        unnamed_register,
                        register,
                    );
                    return VimKeyResult::handled(suppress_text);
                }
            }
            _ => {}
        }
    }

    handle_vim_direct_normal_key_event(
        buffer,
        key,
        modifiers,
        mode,
        pending,
        last_char_find,
        unnamed_register,
        last_change,
        indent_unit,
        count,
        count_value,
        suppress_text,
    )
}
