use eframe::egui::{Key, Modifiers};

use super::{
    EditorVimCaseConversion, EditorVimCharFindMotion, EditorVimOperatorGoKind,
    EditorVimOperatorMotion, EditorVimPendingKey, EditorVimTextObjectKind,
    EditorVimTextObjectScope, VIM_MAX_COUNT, no_text_modifiers,
    vim_case_conversion_repeated_operator_key, vim_line_column_motion_key, vim_mark_name_for_key,
    vim_named_register_for_key, vim_printable_key_char, vim_replacement_key_char,
    vim_search_direction_for_key, vim_search_input_control_edit,
    vim_visual_character_case_conversion, vim_visual_character_change_key,
    vim_visual_character_delete_key, vim_visual_character_indent_key,
    vim_visual_character_join_key, vim_visual_character_outdent_key,
};

pub(super) fn vim_normal_key_can_mutate(
    key: Key,
    modifiers: Modifiers,
    pending: Option<EditorVimPendingKey>,
    printable_key_char: Option<char>,
) -> bool {
    if modifiers.command || modifiers.alt {
        return false;
    }
    if modifiers.ctrl {
        return key == Key::R && !modifiers.shift;
    }
    if matches!(
        pending,
        Some(
            EditorVimPendingKey::VisualCharacter { .. }
                | EditorVimPendingKey::VisualCharacterCount { .. }
                | EditorVimPendingKey::VisualCharacterGo { .. }
        )
    ) {
        let joins_visual_selection = vim_visual_character_join_key(key, modifiers)
            && !matches!(pending, Some(EditorVimPendingKey::VisualCharacterGo { .. }));
        let indents_visual_selection = vim_visual_character_indent_key(key, modifiers);
        let outdents_visual_selection = vim_visual_character_outdent_key(key, modifiers);
        return vim_visual_character_delete_key(key, modifiers)
            || vim_visual_character_change_key(key, modifiers)
            || joins_visual_selection
            || indents_visual_selection
            || outdents_visual_selection
            || vim_visual_character_case_conversion(key, modifiers).is_some();
    }
    if matches!(
        pending,
        Some(EditorVimPendingKey::VisualCharacterCharFind { .. })
    ) {
        return false;
    }
    if matches!(pending, Some(EditorVimPendingKey::SearchInput { .. })) {
        return false;
    }
    if matches!(
        pending,
        Some(EditorVimPendingKey::VisualCharacterRegisterCommand { .. })
    ) {
        return vim_visual_character_delete_key(key, modifiers)
            || vim_visual_character_change_key(key, modifiers);
    }
    if matches!(
        pending,
        Some(EditorVimPendingKey::VisualCharacterReplace { .. })
    ) {
        return printable_key_char.is_some();
    }
    if matches!(
        pending,
        Some(
            EditorVimPendingKey::VisualCharacterTextObject { .. }
                | EditorVimPendingKey::VisualCharacterRegisterPrefix { .. }
        )
    ) {
        return false;
    }
    if matches!(
        (pending, key, modifiers.shift),
        (Some(EditorVimPendingKey::Go(_)), Key::Backtick, true)
            | (Some(EditorVimPendingKey::Go(_)), Key::U, false)
            | (Some(EditorVimPendingKey::Go(_)), Key::U, true)
    ) {
        return false;
    }
    if matches!(pending, Some(EditorVimPendingKey::ReplaceChar(_))) {
        return vim_replacement_key_char(key, modifiers).is_some();
    }
    if matches!(pending, Some(EditorVimPendingKey::RegisterCommand { .. })) {
        return key == Key::P
            || key == Key::S
            || key == Key::X
            || matches!(key, Key::C | Key::D) && modifiers.shift;
    }
    if matches!(
        pending,
        Some(
            EditorVimPendingKey::FindCharForward(_)
                | EditorVimPendingKey::FindCharBackward(_)
                | EditorVimPendingKey::TillCharForward(_)
                | EditorVimPendingKey::TillCharBackward(_)
        )
    ) && printable_key_char.is_some()
    {
        return false;
    }
    if matches!(
        pending,
        Some(
            EditorVimPendingKey::ChangeCharFind { .. }
                | EditorVimPendingKey::ChangeCharFindIntoRegister { .. }
                | EditorVimPendingKey::DeleteCharFind { .. }
                | EditorVimPendingKey::DeleteCharFindIntoRegister { .. }
                | EditorVimPendingKey::ConvertCaseCharFind { .. }
                | EditorVimPendingKey::ToggleCaseCharFind { .. }
        )
    ) && printable_key_char.is_some()
    {
        return true;
    }
    if matches!(
        pending,
        Some(
            EditorVimPendingKey::YankCharFind { .. }
                | EditorVimPendingKey::YankCharFindIntoRegister { .. }
        )
    ) && printable_key_char.is_some()
    {
        return false;
    }
    if let Some(EditorVimPendingKey::OperatorGoMotion { operator, .. }) = pending {
        if vim_operator_go_motion_for_key(key, modifiers).is_some() {
            return vim_operator_go_kind_can_mutate(operator);
        }
    }
    if matches!(
        (pending, key, modifiers.shift),
        (Some(EditorVimPendingKey::ChangeLine(_)), Key::C, false)
            | (
                Some(EditorVimPendingKey::ChangeLineIntoRegister { .. }),
                Key::C,
                false
            )
            | (Some(EditorVimPendingKey::DeleteLine(_)), Key::D, false)
            | (
                Some(EditorVimPendingKey::DeleteLineIntoRegister { .. }),
                Key::D,
                false
            )
            | (Some(EditorVimPendingKey::Go(_)), Key::J, true)
    ) {
        return true;
    }
    if matches!(
        (pending, key, modifiers.shift),
        (Some(EditorVimPendingKey::YankLine(_)), Key::Y, false)
            | (
                Some(EditorVimPendingKey::YankLineIntoRegister { .. }),
                Key::Y,
                false
            )
    ) {
        return false;
    }
    if matches!(
        (pending, key, modifiers.shift),
        (Some(EditorVimPendingKey::IndentLine(_)), Key::Period, true)
            | (Some(EditorVimPendingKey::OutdentLine(_)), Key::Comma, true)
    ) {
        return true;
    }
    if let Some(
        EditorVimPendingKey::ConvertCaseOperator { conversion, .. }
        | EditorVimPendingKey::ConvertCaseMotionCount { conversion, .. },
    ) = pending
        && vim_case_conversion_repeated_operator_key(conversion, key, modifiers)
    {
        return true;
    }
    if matches!(
        pending,
        Some(
            EditorVimPendingKey::ConvertCaseOperator { .. }
                | EditorVimPendingKey::ConvertCaseMotionCount { .. }
        )
    ) && key == Key::U
    {
        return false;
    }
    if key == Key::Period && !modifiers.shift {
        return true;
    }
    if matches!(
        pending,
        Some(
            EditorVimPendingKey::ChangeTextObject { .. }
                | EditorVimPendingKey::ChangeTextObjectIntoRegister { .. }
                | EditorVimPendingKey::DeleteTextObject { .. }
                | EditorVimPendingKey::DeleteTextObjectIntoRegister { .. }
                | EditorVimPendingKey::ConvertCaseTextObject { .. }
                | EditorVimPendingKey::ToggleCaseTextObject { .. }
        )
    ) && vim_text_object_kind_for_key(key, modifiers).is_some()
    {
        return true;
    }
    if matches!(
        pending,
        Some(
            EditorVimPendingKey::ChangeLine(_)
                | EditorVimPendingKey::ChangeLineIntoRegister { .. }
                | EditorVimPendingKey::ChangeMotionCount { .. }
                | EditorVimPendingKey::ChangeMotionCountIntoRegister { .. }
                | EditorVimPendingKey::DeleteLine(_)
                | EditorVimPendingKey::DeleteMotionCount { .. }
                | EditorVimPendingKey::DeleteLineIntoRegister { .. }
                | EditorVimPendingKey::DeleteMotionCountIntoRegister { .. }
                | EditorVimPendingKey::ConvertCaseOperator { .. }
                | EditorVimPendingKey::ConvertCaseMotionCount { .. }
                | EditorVimPendingKey::ToggleCaseOperator(_)
                | EditorVimPendingKey::ToggleCaseMotionCount { .. }
        )
    ) && vim_operator_motion_for_key(key, modifiers).is_some()
    {
        return true;
    }
    matches!(key, Key::O | Key::P | Key::S | Key::X | Key::U)
        || key == Key::Backtick && modifiers.shift
        || key == Key::J && modifiers.shift
        || matches!(key, Key::C | Key::D) && modifiers.shift
}

pub(super) fn vim_normal_key_next_pending(
    key: Key,
    modifiers: Modifiers,
) -> Option<EditorVimPendingKey> {
    if modifiers.command || modifiers.alt || modifiers.ctrl {
        return None;
    }
    if key == Key::F {
        return Some(if modifiers.shift {
            EditorVimPendingKey::FindCharBackward(1)
        } else {
            EditorVimPendingKey::FindCharForward(1)
        });
    }
    if key == Key::T {
        return Some(if modifiers.shift {
            EditorVimPendingKey::TillCharBackward(1)
        } else {
            EditorVimPendingKey::TillCharForward(1)
        });
    }
    if key == Key::Period && modifiers.shift {
        return Some(EditorVimPendingKey::IndentLine(1));
    }
    if key == Key::Comma && modifiers.shift {
        return Some(EditorVimPendingKey::OutdentLine(1));
    }
    if let Some(forward) = vim_search_direction_for_key(key, modifiers) {
        return Some(EditorVimPendingKey::SearchInput { count: 1, forward });
    }
    if key == Key::Quote && modifiers.shift {
        return Some(EditorVimPendingKey::RegisterPrefix(1));
    }
    if modifiers.shift {
        return None;
    }
    match key {
        Key::C => Some(EditorVimPendingKey::ChangeLine(1)),
        Key::D => Some(EditorVimPendingKey::DeleteLine(1)),
        Key::G => Some(EditorVimPendingKey::Go(None)),
        Key::M => Some(EditorVimPendingKey::SetMark),
        Key::Quote => Some(EditorVimPendingKey::JumpMark { linewise: true }),
        Key::Backtick => Some(EditorVimPendingKey::JumpMark { linewise: false }),
        Key::R => Some(EditorVimPendingKey::ReplaceChar(1)),
        Key::V => Some(EditorVimPendingKey::VisualCharacter {
            anchor: 0,
            cursor: 0,
        }),
        Key::Y => Some(EditorVimPendingKey::YankLine(1)),
        key => vim_count_digit(key, modifiers, false).map(EditorVimPendingKey::Count),
    }
}

pub(super) fn vim_normal_key_next_pending_after_count(
    count: usize,
    key: Key,
    modifiers: Modifiers,
) -> Option<EditorVimPendingKey> {
    if modifiers.command || modifiers.alt || modifiers.ctrl {
        return None;
    }
    if key == Key::F {
        return Some(if modifiers.shift {
            EditorVimPendingKey::FindCharBackward(count)
        } else {
            EditorVimPendingKey::FindCharForward(count)
        });
    }
    if key == Key::T {
        return Some(if modifiers.shift {
            EditorVimPendingKey::TillCharBackward(count)
        } else {
            EditorVimPendingKey::TillCharForward(count)
        });
    }
    if key == Key::Period && modifiers.shift {
        return Some(EditorVimPendingKey::IndentLine(count));
    }
    if key == Key::Comma && modifiers.shift {
        return Some(EditorVimPendingKey::OutdentLine(count));
    }
    if let Some(forward) = vim_search_direction_for_key(key, modifiers) {
        return Some(EditorVimPendingKey::SearchInput { count, forward });
    }
    if key == Key::Quote && modifiers.shift {
        return Some(EditorVimPendingKey::RegisterPrefix(count));
    }
    if modifiers.shift {
        return None;
    }
    match key {
        Key::C => Some(EditorVimPendingKey::ChangeLine(count)),
        Key::D => Some(EditorVimPendingKey::DeleteLine(count)),
        Key::G => Some(EditorVimPendingKey::Go(Some(count))),
        Key::M => Some(EditorVimPendingKey::SetMark),
        Key::Quote => Some(EditorVimPendingKey::JumpMark { linewise: true }),
        Key::Backtick => Some(EditorVimPendingKey::JumpMark { linewise: false }),
        Key::R => Some(EditorVimPendingKey::ReplaceChar(count)),
        Key::V => Some(EditorVimPendingKey::VisualCharacter {
            anchor: 0,
            cursor: 0,
        }),
        Key::Y => Some(EditorVimPendingKey::YankLine(count)),
        key => vim_count_digit(key, modifiers, true)
            .map(|digit| EditorVimPendingKey::Count(vim_push_count_digit(count, digit))),
    }
}

pub(super) fn vim_count_digit(key: Key, modifiers: Modifiers, allow_zero: bool) -> Option<usize> {
    if modifiers.command || modifiers.alt || modifiers.ctrl || modifiers.shift {
        return None;
    }
    match key {
        Key::Num0 if allow_zero => Some(0),
        Key::Num1 => Some(1),
        Key::Num2 => Some(2),
        Key::Num3 => Some(3),
        Key::Num4 => Some(4),
        Key::Num5 => Some(5),
        Key::Num6 => Some(6),
        Key::Num7 => Some(7),
        Key::Num8 => Some(8),
        Key::Num9 => Some(9),
        _ => None,
    }
}

pub(super) fn vim_push_count_digit(count: usize, digit: usize) -> usize {
    count
        .saturating_mul(10)
        .saturating_add(digit)
        .clamp(1, VIM_MAX_COUNT)
}

pub(super) fn vim_register_command_count(
    prefix_count: usize,
    command_count: Option<usize>,
) -> usize {
    match command_count {
        Some(command_count) => vim_combined_count(prefix_count, command_count),
        None => prefix_count.clamp(1, VIM_MAX_COUNT),
    }
}

pub(super) fn vim_register_command_next_count(
    command_count: Option<usize>,
    key: Key,
    modifiers: Modifiers,
) -> Option<usize> {
    match command_count {
        Some(count) => {
            vim_count_digit(key, modifiers, true).map(|digit| vim_push_count_digit(count, digit))
        }
        None => vim_count_digit(key, modifiers, false),
    }
}

pub(super) fn vim_operator_motion_for_key(
    key: Key,
    modifiers: Modifiers,
) -> Option<EditorVimOperatorMotion> {
    if modifiers.command || modifiers.alt || modifiers.ctrl {
        return None;
    }
    if vim_line_column_motion_key(key, modifiers) {
        return Some(EditorVimOperatorMotion::LineColumn);
    }
    if key == Key::Home && no_text_modifiers(modifiers) {
        return Some(EditorVimOperatorMotion::LineColumnStart);
    }
    if key == Key::End && no_text_modifiers(modifiers) {
        return Some(EditorVimOperatorMotion::LineEnd);
    }
    match (key, modifiers.shift) {
        (Key::B, false) => Some(EditorVimOperatorMotion::WordBackward),
        (Key::B, true) => Some(EditorVimOperatorMotion::BigWordBackward),
        (Key::E, false) => Some(EditorVimOperatorMotion::WordEnd),
        (Key::E, true) => Some(EditorVimOperatorMotion::BigWordEnd),
        (Key::Backspace, false) => Some(EditorVimOperatorMotion::CharacterBackward),
        (Key::H, false) => Some(EditorVimOperatorMotion::CharacterBackward),
        (Key::L, false) => Some(EditorVimOperatorMotion::CharacterForward),
        (Key::Space, false) => Some(EditorVimOperatorMotion::CharacterForward),
        (Key::W, false) => Some(EditorVimOperatorMotion::WordForward),
        (Key::W, true) => Some(EditorVimOperatorMotion::BigWordForward),
        (Key::Num0, false) => Some(EditorVimOperatorMotion::LineColumnStart),
        (Key::Num4, true) => Some(EditorVimOperatorMotion::LineEnd),
        (Key::Num5, true) => Some(EditorVimOperatorMotion::MatchingBracket),
        (Key::Num6, true) => Some(EditorVimOperatorMotion::LineFirstNonWhitespace),
        (Key::CloseBracket, true) => Some(EditorVimOperatorMotion::ParagraphForward),
        (Key::OpenBracket, true) => Some(EditorVimOperatorMotion::ParagraphBackward),
        (Key::Num3, true) => Some(EditorVimOperatorMotion::SearchWordUnderCursor {
            forward: false,
            whole_word: true,
        }),
        (Key::Num8, true) => Some(EditorVimOperatorMotion::SearchWordUnderCursor {
            forward: true,
            whole_word: true,
        }),
        (Key::N, false) => Some(EditorVimOperatorMotion::SearchRepeat { reverse: false }),
        (Key::N, true) => Some(EditorVimOperatorMotion::SearchRepeat { reverse: true }),
        _ => None,
    }
}

pub(super) fn vim_operator_go_motion_for_key(
    key: Key,
    modifiers: Modifiers,
) -> Option<EditorVimOperatorMotion> {
    if modifiers.command || modifiers.alt || modifiers.ctrl {
        return None;
    }
    match (key, modifiers.shift) {
        (Key::E, false) => Some(EditorVimOperatorMotion::WordEndBackward),
        (Key::E, true) => Some(EditorVimOperatorMotion::BigWordEndBackward),
        (Key::N, false) => Some(EditorVimOperatorMotion::SearchMatch { reverse: false }),
        (Key::N, true) => Some(EditorVimOperatorMotion::SearchMatch { reverse: true }),
        (Key::Num3, true) => Some(EditorVimOperatorMotion::SearchWordUnderCursor {
            forward: false,
            whole_word: false,
        }),
        (Key::Num8, true) => Some(EditorVimOperatorMotion::SearchWordUnderCursor {
            forward: true,
            whole_word: false,
        }),
        _ => None,
    }
}

fn vim_operator_go_kind_can_mutate(operator: EditorVimOperatorGoKind) -> bool {
    !matches!(
        operator,
        EditorVimOperatorGoKind::Yank | EditorVimOperatorGoKind::YankIntoRegister(_)
    )
}

pub(super) fn vim_operator_char_find_motion_for_key(
    key: Key,
    modifiers: Modifiers,
) -> Option<EditorVimCharFindMotion> {
    if modifiers.command || modifiers.alt || modifiers.ctrl {
        return None;
    }
    match (key, modifiers.shift) {
        (Key::F, false) => Some(EditorVimCharFindMotion::FindForward),
        (Key::F, true) => Some(EditorVimCharFindMotion::FindBackward),
        (Key::T, false) => Some(EditorVimCharFindMotion::TillForward),
        (Key::T, true) => Some(EditorVimCharFindMotion::TillBackward),
        _ => None,
    }
}

pub(super) fn vim_combined_count(operator_count: usize, motion_count: usize) -> usize {
    operator_count
        .max(1)
        .saturating_mul(motion_count.max(1))
        .clamp(1, VIM_MAX_COUNT)
}

pub(super) fn vim_text_object_scope_for_key(
    key: Key,
    modifiers: Modifiers,
) -> Option<EditorVimTextObjectScope> {
    if !no_text_modifiers(modifiers) {
        return None;
    }
    match key {
        Key::I => Some(EditorVimTextObjectScope::Inner),
        Key::A => Some(EditorVimTextObjectScope::Outer),
        _ => None,
    }
}

pub(super) fn vim_text_object_kind_for_key(
    key: Key,
    modifiers: Modifiers,
) -> Option<EditorVimTextObjectKind> {
    if modifiers.command || modifiers.alt || modifiers.ctrl {
        return None;
    }
    match vim_printable_key_char(key, modifiers)? {
        'W' => Some(EditorVimTextObjectKind::BigWord),
        'w' => Some(EditorVimTextObjectKind::Word),
        'p' => Some(EditorVimTextObjectKind::Paragraph),
        's' => Some(EditorVimTextObjectKind::Sentence),
        '(' | ')' => Some(EditorVimTextObjectKind::Block {
            open: '(',
            close: ')',
        }),
        '[' | ']' => Some(EditorVimTextObjectKind::Block {
            open: '[',
            close: ']',
        }),
        '<' | '>' => Some(EditorVimTextObjectKind::Block {
            open: '<',
            close: '>',
        }),
        '{' | '}' => Some(EditorVimTextObjectKind::Block {
            open: '{',
            close: '}',
        }),
        ch @ ('"' | '\'' | '`') => Some(EditorVimTextObjectKind::Quote { quote: ch }),
        _ => None,
    }
}

pub(super) fn vim_pending_key_next_named_register(
    pending: Option<EditorVimPendingKey>,
    key: Key,
    modifiers: Modifiers,
) -> Option<EditorVimPendingKey> {
    match pending {
        Some(EditorVimPendingKey::RegisterPrefix(count)) => {
            vim_named_register_for_key(key, modifiers).map(|register| {
                EditorVimPendingKey::RegisterCommand {
                    prefix_count: count,
                    command_count: None,
                    register,
                }
            })
        }
        Some(EditorVimPendingKey::RegisterCommand {
            prefix_count,
            command_count,
            register,
        }) => {
            if let Some(command_count) =
                vim_register_command_next_count(command_count, key, modifiers)
            {
                return Some(EditorVimPendingKey::RegisterCommand {
                    prefix_count,
                    command_count: Some(command_count),
                    register,
                });
            }
            let count = vim_register_command_count(prefix_count, command_count);
            if modifiers.command || modifiers.alt || modifiers.ctrl || modifiers.shift {
                return None;
            }
            match key {
                Key::C => Some(EditorVimPendingKey::ChangeLineIntoRegister {
                    operator_count: count,
                    register,
                }),
                Key::D => Some(EditorVimPendingKey::DeleteLineIntoRegister {
                    operator_count: count,
                    register,
                }),
                Key::Y => Some(EditorVimPendingKey::YankLineIntoRegister {
                    operator_count: count,
                    register,
                }),
                _ => None,
            }
        }
        _ => None,
    }
}

pub(super) fn vim_pending_key_next_operator_count(
    pending: Option<EditorVimPendingKey>,
    key: Key,
    modifiers: Modifiers,
) -> Option<EditorVimPendingKey> {
    match pending {
        Some(EditorVimPendingKey::Go(operator_count))
            if key == Key::Backtick
                && modifiers.shift
                && !modifiers.command
                && !modifiers.alt
                && !modifiers.ctrl =>
        {
            Some(EditorVimPendingKey::ToggleCaseOperator(
                operator_count.unwrap_or(1),
            ))
        }
        Some(EditorVimPendingKey::Go(operator_count))
            if key == Key::U && !modifiers.command && !modifiers.alt && !modifiers.ctrl =>
        {
            Some(EditorVimPendingKey::ConvertCaseOperator {
                operator_count: operator_count.unwrap_or(1),
                conversion: if modifiers.shift {
                    EditorVimCaseConversion::Upper
                } else {
                    EditorVimCaseConversion::Lower
                },
            })
        }
        Some(EditorVimPendingKey::ChangeLine(operator_count)) => {
            vim_count_digit(key, modifiers, false).map(|motion_count| {
                EditorVimPendingKey::ChangeMotionCount {
                    operator_count,
                    motion_count,
                }
            })
        }
        Some(EditorVimPendingKey::ChangeLineIntoRegister {
            operator_count,
            register,
        }) => vim_count_digit(key, modifiers, false).map(|motion_count| {
            EditorVimPendingKey::ChangeMotionCountIntoRegister {
                operator_count,
                motion_count,
                register,
            }
        }),
        Some(EditorVimPendingKey::DeleteLine(operator_count)) => {
            vim_count_digit(key, modifiers, false).map(|motion_count| {
                EditorVimPendingKey::DeleteMotionCount {
                    operator_count,
                    motion_count,
                }
            })
        }
        Some(EditorVimPendingKey::DeleteLineIntoRegister {
            operator_count,
            register,
        }) => vim_count_digit(key, modifiers, false).map(|motion_count| {
            EditorVimPendingKey::DeleteMotionCountIntoRegister {
                operator_count,
                motion_count,
                register,
            }
        }),
        Some(EditorVimPendingKey::YankLine(operator_count)) => {
            vim_count_digit(key, modifiers, false).map(|motion_count| {
                EditorVimPendingKey::YankMotionCount {
                    operator_count,
                    motion_count,
                }
            })
        }
        Some(EditorVimPendingKey::YankLineIntoRegister {
            operator_count,
            register,
        }) => vim_count_digit(key, modifiers, false).map(|motion_count| {
            EditorVimPendingKey::YankMotionCountIntoRegister {
                operator_count,
                motion_count,
                register,
            }
        }),
        Some(EditorVimPendingKey::ToggleCaseOperator(operator_count)) => {
            vim_count_digit(key, modifiers, false).map(|motion_count| {
                EditorVimPendingKey::ToggleCaseMotionCount {
                    operator_count,
                    motion_count,
                }
            })
        }
        Some(EditorVimPendingKey::ConvertCaseOperator {
            operator_count,
            conversion,
        }) => vim_count_digit(key, modifiers, false).map(|motion_count| {
            EditorVimPendingKey::ConvertCaseMotionCount {
                operator_count,
                motion_count,
                conversion,
            }
        }),
        Some(EditorVimPendingKey::ChangeMotionCount {
            operator_count,
            motion_count,
        }) => vim_count_digit(key, modifiers, true).map(|digit| {
            EditorVimPendingKey::ChangeMotionCount {
                operator_count,
                motion_count: vim_push_count_digit(motion_count, digit),
            }
        }),
        Some(EditorVimPendingKey::ChangeMotionCountIntoRegister {
            operator_count,
            motion_count,
            register,
        }) => vim_count_digit(key, modifiers, true).map(|digit| {
            EditorVimPendingKey::ChangeMotionCountIntoRegister {
                operator_count,
                motion_count: vim_push_count_digit(motion_count, digit),
                register,
            }
        }),
        Some(EditorVimPendingKey::DeleteMotionCount {
            operator_count,
            motion_count,
        }) => vim_count_digit(key, modifiers, true).map(|digit| {
            EditorVimPendingKey::DeleteMotionCount {
                operator_count,
                motion_count: vim_push_count_digit(motion_count, digit),
            }
        }),
        Some(EditorVimPendingKey::DeleteMotionCountIntoRegister {
            operator_count,
            motion_count,
            register,
        }) => vim_count_digit(key, modifiers, true).map(|digit| {
            EditorVimPendingKey::DeleteMotionCountIntoRegister {
                operator_count,
                motion_count: vim_push_count_digit(motion_count, digit),
                register,
            }
        }),
        Some(EditorVimPendingKey::YankMotionCount {
            operator_count,
            motion_count,
        }) => vim_count_digit(key, modifiers, true).map(|digit| {
            EditorVimPendingKey::YankMotionCount {
                operator_count,
                motion_count: vim_push_count_digit(motion_count, digit),
            }
        }),
        Some(EditorVimPendingKey::YankMotionCountIntoRegister {
            operator_count,
            motion_count,
            register,
        }) => vim_count_digit(key, modifiers, true).map(|digit| {
            EditorVimPendingKey::YankMotionCountIntoRegister {
                operator_count,
                motion_count: vim_push_count_digit(motion_count, digit),
                register,
            }
        }),
        Some(EditorVimPendingKey::ToggleCaseMotionCount {
            operator_count,
            motion_count,
        }) => vim_count_digit(key, modifiers, true).map(|digit| {
            EditorVimPendingKey::ToggleCaseMotionCount {
                operator_count,
                motion_count: vim_push_count_digit(motion_count, digit),
            }
        }),
        Some(EditorVimPendingKey::ConvertCaseMotionCount {
            operator_count,
            motion_count,
            conversion,
        }) => vim_count_digit(key, modifiers, true).map(|digit| {
            EditorVimPendingKey::ConvertCaseMotionCount {
                operator_count,
                motion_count: vim_push_count_digit(motion_count, digit),
                conversion,
            }
        }),
        _ => None,
    }
}

pub(super) fn vim_pending_key_next_operator_go(
    pending: Option<EditorVimPendingKey>,
    key: Key,
    modifiers: Modifiers,
) -> Option<EditorVimPendingKey> {
    if key != Key::G || modifiers.shift || modifiers.command || modifiers.alt || modifiers.ctrl {
        return None;
    }
    let (operator_count, motion_count, operator) = match pending? {
        EditorVimPendingKey::ChangeLine(operator_count) => {
            (operator_count, 1, EditorVimOperatorGoKind::Change)
        }
        EditorVimPendingKey::ChangeLineIntoRegister {
            operator_count,
            register,
        } => (
            operator_count,
            1,
            EditorVimOperatorGoKind::ChangeIntoRegister(register),
        ),
        EditorVimPendingKey::ChangeMotionCount {
            operator_count,
            motion_count,
        } => (
            operator_count,
            motion_count,
            EditorVimOperatorGoKind::Change,
        ),
        EditorVimPendingKey::ChangeMotionCountIntoRegister {
            operator_count,
            motion_count,
            register,
        } => (
            operator_count,
            motion_count,
            EditorVimOperatorGoKind::ChangeIntoRegister(register),
        ),
        EditorVimPendingKey::ConvertCaseOperator {
            operator_count,
            conversion,
        } => (
            operator_count,
            1,
            EditorVimOperatorGoKind::ConvertCase(conversion),
        ),
        EditorVimPendingKey::ConvertCaseMotionCount {
            operator_count,
            motion_count,
            conversion,
        } => (
            operator_count,
            motion_count,
            EditorVimOperatorGoKind::ConvertCase(conversion),
        ),
        EditorVimPendingKey::DeleteLine(operator_count) => {
            (operator_count, 1, EditorVimOperatorGoKind::Delete)
        }
        EditorVimPendingKey::DeleteLineIntoRegister {
            operator_count,
            register,
        } => (
            operator_count,
            1,
            EditorVimOperatorGoKind::DeleteIntoRegister(register),
        ),
        EditorVimPendingKey::DeleteMotionCount {
            operator_count,
            motion_count,
        } => (
            operator_count,
            motion_count,
            EditorVimOperatorGoKind::Delete,
        ),
        EditorVimPendingKey::DeleteMotionCountIntoRegister {
            operator_count,
            motion_count,
            register,
        } => (
            operator_count,
            motion_count,
            EditorVimOperatorGoKind::DeleteIntoRegister(register),
        ),
        EditorVimPendingKey::ToggleCaseOperator(operator_count) => {
            (operator_count, 1, EditorVimOperatorGoKind::ToggleCase)
        }
        EditorVimPendingKey::ToggleCaseMotionCount {
            operator_count,
            motion_count,
        } => (
            operator_count,
            motion_count,
            EditorVimOperatorGoKind::ToggleCase,
        ),
        EditorVimPendingKey::YankLine(operator_count) => {
            (operator_count, 1, EditorVimOperatorGoKind::Yank)
        }
        EditorVimPendingKey::YankLineIntoRegister {
            operator_count,
            register,
        } => (
            operator_count,
            1,
            EditorVimOperatorGoKind::YankIntoRegister(register),
        ),
        EditorVimPendingKey::YankMotionCount {
            operator_count,
            motion_count,
        } => (operator_count, motion_count, EditorVimOperatorGoKind::Yank),
        EditorVimPendingKey::YankMotionCountIntoRegister {
            operator_count,
            motion_count,
            register,
        } => (
            operator_count,
            motion_count,
            EditorVimOperatorGoKind::YankIntoRegister(register),
        ),
        _ => return None,
    };

    Some(EditorVimPendingKey::OperatorGoMotion {
        operator_count,
        motion_count,
        operator,
    })
}

pub(super) fn vim_pending_key_next_text_object(
    pending: Option<EditorVimPendingKey>,
    key: Key,
    modifiers: Modifiers,
) -> Option<EditorVimPendingKey> {
    let scope = vim_text_object_scope_for_key(key, modifiers)?;
    match pending {
        Some(EditorVimPendingKey::ChangeLine(operator_count)) => {
            Some(EditorVimPendingKey::ChangeTextObject {
                operator_count,
                motion_count: 1,
                scope,
            })
        }
        Some(EditorVimPendingKey::ChangeLineIntoRegister {
            operator_count,
            register,
        }) => Some(EditorVimPendingKey::ChangeTextObjectIntoRegister {
            operator_count,
            motion_count: 1,
            scope,
            register,
        }),
        Some(EditorVimPendingKey::DeleteLine(operator_count)) => {
            Some(EditorVimPendingKey::DeleteTextObject {
                operator_count,
                motion_count: 1,
                scope,
            })
        }
        Some(EditorVimPendingKey::DeleteLineIntoRegister {
            operator_count,
            register,
        }) => Some(EditorVimPendingKey::DeleteTextObjectIntoRegister {
            operator_count,
            motion_count: 1,
            scope,
            register,
        }),
        Some(EditorVimPendingKey::YankLine(operator_count)) => {
            Some(EditorVimPendingKey::YankTextObject {
                operator_count,
                motion_count: 1,
                scope,
            })
        }
        Some(EditorVimPendingKey::YankLineIntoRegister {
            operator_count,
            register,
        }) => Some(EditorVimPendingKey::YankTextObjectIntoRegister {
            operator_count,
            motion_count: 1,
            scope,
            register,
        }),
        Some(EditorVimPendingKey::ChangeMotionCount {
            operator_count,
            motion_count,
        }) => Some(EditorVimPendingKey::ChangeTextObject {
            operator_count,
            motion_count,
            scope,
        }),
        Some(EditorVimPendingKey::ChangeMotionCountIntoRegister {
            operator_count,
            motion_count,
            register,
        }) => Some(EditorVimPendingKey::ChangeTextObjectIntoRegister {
            operator_count,
            motion_count,
            scope,
            register,
        }),
        Some(EditorVimPendingKey::DeleteMotionCount {
            operator_count,
            motion_count,
        }) => Some(EditorVimPendingKey::DeleteTextObject {
            operator_count,
            motion_count,
            scope,
        }),
        Some(EditorVimPendingKey::DeleteMotionCountIntoRegister {
            operator_count,
            motion_count,
            register,
        }) => Some(EditorVimPendingKey::DeleteTextObjectIntoRegister {
            operator_count,
            motion_count,
            scope,
            register,
        }),
        Some(EditorVimPendingKey::YankMotionCount {
            operator_count,
            motion_count,
        }) => Some(EditorVimPendingKey::YankTextObject {
            operator_count,
            motion_count,
            scope,
        }),
        Some(EditorVimPendingKey::YankMotionCountIntoRegister {
            operator_count,
            motion_count,
            register,
        }) => Some(EditorVimPendingKey::YankTextObjectIntoRegister {
            operator_count,
            motion_count,
            scope,
            register,
        }),
        Some(EditorVimPendingKey::ConvertCaseOperator {
            operator_count,
            conversion,
        }) => Some(EditorVimPendingKey::ConvertCaseTextObject {
            operator_count,
            motion_count: 1,
            scope,
            conversion,
        }),
        Some(EditorVimPendingKey::ConvertCaseMotionCount {
            operator_count,
            motion_count,
            conversion,
        }) => Some(EditorVimPendingKey::ConvertCaseTextObject {
            operator_count,
            motion_count,
            scope,
            conversion,
        }),
        Some(EditorVimPendingKey::ToggleCaseOperator(operator_count)) => {
            Some(EditorVimPendingKey::ToggleCaseTextObject {
                operator_count,
                motion_count: 1,
                scope,
            })
        }
        Some(EditorVimPendingKey::ToggleCaseMotionCount {
            operator_count,
            motion_count,
        }) => Some(EditorVimPendingKey::ToggleCaseTextObject {
            operator_count,
            motion_count,
            scope,
        }),
        _ => None,
    }
}

pub(super) fn vim_pending_key_next_char_find(
    pending: Option<EditorVimPendingKey>,
    key: Key,
    modifiers: Modifiers,
) -> Option<EditorVimPendingKey> {
    let motion = vim_operator_char_find_motion_for_key(key, modifiers)?;
    match pending {
        Some(EditorVimPendingKey::ChangeLine(operator_count)) => {
            Some(EditorVimPendingKey::ChangeCharFind {
                operator_count,
                motion_count: 1,
                motion,
            })
        }
        Some(EditorVimPendingKey::ChangeLineIntoRegister {
            operator_count,
            register,
        }) => Some(EditorVimPendingKey::ChangeCharFindIntoRegister {
            operator_count,
            motion_count: 1,
            motion,
            register,
        }),
        Some(EditorVimPendingKey::DeleteLine(operator_count)) => {
            Some(EditorVimPendingKey::DeleteCharFind {
                operator_count,
                motion_count: 1,
                motion,
            })
        }
        Some(EditorVimPendingKey::DeleteLineIntoRegister {
            operator_count,
            register,
        }) => Some(EditorVimPendingKey::DeleteCharFindIntoRegister {
            operator_count,
            motion_count: 1,
            motion,
            register,
        }),
        Some(EditorVimPendingKey::YankLine(operator_count)) => {
            Some(EditorVimPendingKey::YankCharFind {
                operator_count,
                motion_count: 1,
                motion,
            })
        }
        Some(EditorVimPendingKey::YankLineIntoRegister {
            operator_count,
            register,
        }) => Some(EditorVimPendingKey::YankCharFindIntoRegister {
            operator_count,
            motion_count: 1,
            motion,
            register,
        }),
        Some(EditorVimPendingKey::ToggleCaseOperator(operator_count)) => {
            Some(EditorVimPendingKey::ToggleCaseCharFind {
                operator_count,
                motion_count: 1,
                motion,
            })
        }
        Some(EditorVimPendingKey::ConvertCaseOperator {
            operator_count,
            conversion,
        }) => Some(EditorVimPendingKey::ConvertCaseCharFind {
            operator_count,
            motion_count: 1,
            motion,
            conversion,
        }),
        Some(EditorVimPendingKey::ChangeMotionCount {
            operator_count,
            motion_count,
        }) => Some(EditorVimPendingKey::ChangeCharFind {
            operator_count,
            motion_count,
            motion,
        }),
        Some(EditorVimPendingKey::ChangeMotionCountIntoRegister {
            operator_count,
            motion_count,
            register,
        }) => Some(EditorVimPendingKey::ChangeCharFindIntoRegister {
            operator_count,
            motion_count,
            motion,
            register,
        }),
        Some(EditorVimPendingKey::DeleteMotionCount {
            operator_count,
            motion_count,
        }) => Some(EditorVimPendingKey::DeleteCharFind {
            operator_count,
            motion_count,
            motion,
        }),
        Some(EditorVimPendingKey::DeleteMotionCountIntoRegister {
            operator_count,
            motion_count,
            register,
        }) => Some(EditorVimPendingKey::DeleteCharFindIntoRegister {
            operator_count,
            motion_count,
            motion,
            register,
        }),
        Some(EditorVimPendingKey::YankMotionCount {
            operator_count,
            motion_count,
        }) => Some(EditorVimPendingKey::YankCharFind {
            operator_count,
            motion_count,
            motion,
        }),
        Some(EditorVimPendingKey::YankMotionCountIntoRegister {
            operator_count,
            motion_count,
            register,
        }) => Some(EditorVimPendingKey::YankCharFindIntoRegister {
            operator_count,
            motion_count,
            motion,
            register,
        }),
        Some(EditorVimPendingKey::ToggleCaseMotionCount {
            operator_count,
            motion_count,
        }) => Some(EditorVimPendingKey::ToggleCaseCharFind {
            operator_count,
            motion_count,
            motion,
        }),
        Some(EditorVimPendingKey::ConvertCaseMotionCount {
            operator_count,
            motion_count,
            conversion,
        }) => Some(EditorVimPendingKey::ConvertCaseCharFind {
            operator_count,
            motion_count,
            motion,
            conversion,
        }),
        _ => None,
    }
}

pub(super) fn vim_pending_key_accepts(
    pending: Option<EditorVimPendingKey>,
    key: Key,
    modifiers: Modifiers,
    printable_key_char: Option<char>,
) -> bool {
    if matches!(pending, Some(EditorVimPendingKey::ReplaceChar(_))) {
        return vim_replacement_key_char(key, modifiers).is_some();
    }
    if matches!(pending, Some(EditorVimPendingKey::SearchInput { .. })) {
        return (key == Key::Enter || key == Key::Backspace) && no_text_modifiers(modifiers)
            || vim_search_input_control_edit(key, modifiers).is_some()
            || printable_key_char.is_some();
    }
    if matches!(pending, Some(EditorVimPendingKey::RegisterPrefix(_))) {
        return vim_named_register_for_key(key, modifiers).is_some();
    }
    if matches!(pending, Some(EditorVimPendingKey::RegisterCommand { .. })) {
        return matches!(
            (key, modifiers.shift),
            (Key::C, true)
                | (Key::D, true)
                | (Key::P, false)
                | (Key::P, true)
                | (Key::S, false)
                | (Key::S, true)
                | (Key::X, false)
                | (Key::X, true)
                | (Key::Y, true)
        );
    }
    if matches!(
        pending,
        Some(
            EditorVimPendingKey::FindCharForward(_)
                | EditorVimPendingKey::FindCharBackward(_)
                | EditorVimPendingKey::TillCharForward(_)
                | EditorVimPendingKey::TillCharBackward(_)
        )
    ) {
        return printable_key_char.is_some();
    }
    if matches!(
        pending,
        Some(
            EditorVimPendingKey::ChangeCharFind { .. }
                | EditorVimPendingKey::ChangeCharFindIntoRegister { .. }
                | EditorVimPendingKey::DeleteCharFind { .. }
                | EditorVimPendingKey::ToggleCaseCharFind { .. }
                | EditorVimPendingKey::ConvertCaseCharFind { .. }
                | EditorVimPendingKey::YankCharFind { .. }
                | EditorVimPendingKey::DeleteCharFindIntoRegister { .. }
                | EditorVimPendingKey::YankCharFindIntoRegister { .. }
        )
    ) {
        return printable_key_char.is_some();
    }
    if matches!(pending, Some(EditorVimPendingKey::OperatorGoMotion { .. })) {
        return vim_operator_go_motion_for_key(key, modifiers).is_some();
    }
    if matches!(
        pending,
        Some(EditorVimPendingKey::JumpMark { .. } | EditorVimPendingKey::SetMark)
    ) {
        return vim_mark_name_for_key(key, modifiers).is_some();
    }
    if let Some(
        EditorVimPendingKey::ConvertCaseOperator { conversion, .. }
        | EditorVimPendingKey::ConvertCaseMotionCount { conversion, .. },
    ) = pending
        && vim_case_conversion_repeated_operator_key(conversion, key, modifiers)
    {
        return true;
    }
    matches!(
        (pending, key, modifiers.shift),
        (Some(EditorVimPendingKey::ChangeLine(_)), Key::C, false)
            | (
                Some(EditorVimPendingKey::ChangeLineIntoRegister { .. }),
                Key::C,
                false
            )
            | (Some(EditorVimPendingKey::DeleteLine(_)), Key::D, false)
            | (
                Some(EditorVimPendingKey::DeleteLineIntoRegister { .. }),
                Key::D,
                false
            )
            | (Some(EditorVimPendingKey::Go(_)), Key::Num3, true)
            | (Some(EditorVimPendingKey::Go(_)), Key::Num8, true)
            | (Some(EditorVimPendingKey::Go(_)), Key::G, false)
            | (Some(EditorVimPendingKey::Go(_)), Key::E, false)
            | (Some(EditorVimPendingKey::Go(_)), Key::E, true)
            | (Some(EditorVimPendingKey::Go(_)), Key::J, true)
            | (Some(EditorVimPendingKey::Go(_)), Key::U, false)
            | (Some(EditorVimPendingKey::Go(_)), Key::U, true)
            | (Some(EditorVimPendingKey::IndentLine(_)), Key::Period, true)
            | (Some(EditorVimPendingKey::OutdentLine(_)), Key::Comma, true)
            | (Some(EditorVimPendingKey::YankLine(_)), Key::Y, false)
            | (
                Some(EditorVimPendingKey::YankLineIntoRegister { .. }),
                Key::Y,
                false
            )
    ) || matches!(
        pending,
        Some(
            EditorVimPendingKey::ChangeLine(_)
                | EditorVimPendingKey::ChangeLineIntoRegister { .. }
                | EditorVimPendingKey::ChangeMotionCount { .. }
                | EditorVimPendingKey::ChangeMotionCountIntoRegister { .. }
                | EditorVimPendingKey::DeleteLine(_)
                | EditorVimPendingKey::DeleteMotionCount { .. }
                | EditorVimPendingKey::DeleteLineIntoRegister { .. }
                | EditorVimPendingKey::DeleteMotionCountIntoRegister { .. }
                | EditorVimPendingKey::YankLine(_)
                | EditorVimPendingKey::YankMotionCount { .. }
                | EditorVimPendingKey::YankLineIntoRegister { .. }
                | EditorVimPendingKey::YankMotionCountIntoRegister { .. }
                | EditorVimPendingKey::ConvertCaseOperator { .. }
                | EditorVimPendingKey::ConvertCaseMotionCount { .. }
                | EditorVimPendingKey::ToggleCaseOperator(_)
                | EditorVimPendingKey::ToggleCaseMotionCount { .. }
        )
    ) && vim_operator_motion_for_key(key, modifiers).is_some()
        || matches!(
            pending,
            Some(
                EditorVimPendingKey::ChangeTextObject { .. }
                    | EditorVimPendingKey::ChangeTextObjectIntoRegister { .. }
                    | EditorVimPendingKey::DeleteTextObject { .. }
                    | EditorVimPendingKey::YankTextObject { .. }
                    | EditorVimPendingKey::DeleteTextObjectIntoRegister { .. }
                    | EditorVimPendingKey::YankTextObjectIntoRegister { .. }
                    | EditorVimPendingKey::ConvertCaseTextObject { .. }
                    | EditorVimPendingKey::ToggleCaseTextObject { .. }
            )
        ) && vim_text_object_kind_for_key(key, modifiers).is_some()
}
