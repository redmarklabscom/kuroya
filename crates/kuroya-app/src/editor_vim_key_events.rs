use eframe::egui::{Event, ImeEvent, Key, Modifiers};
use kuroya_core::TextBuffer;
use std::{collections::VecDeque, ops::Range};

mod commands;
mod direct;
mod motion;
mod operator;
mod parser;
mod pending;
mod search;
mod state;
mod visual;

use self::commands::{
    vim_delete_lines_into_named_register, vim_delete_lines_into_register, vim_put_register_after,
    vim_put_register_before, vim_yank_lines, vim_yank_lines_into_named_register,
};
use self::motion::{
    vim_apply_char_find, vim_case_conversion_repeated_operator_key, vim_char_at,
    vim_convert_case_lines, vim_convert_case_range, vim_ctrl_scroll_lines,
    vim_delete_backward_chars_into_named_register, vim_delete_forward_chars_into_named_register,
    vim_delete_line_backward, vim_indent_lines, vim_join_lines_without_whitespace,
    vim_line_column_motion_char, vim_line_first_non_whitespace_char, vim_line_outdent_len,
    vim_line_scroll_lines, vim_matching_bracket_range, vim_move_counted_line_first_non_whitespace,
    vim_move_down_lines, vim_move_next_line_first_non_whitespace, vim_move_next_paragraph,
    vim_move_previous_big_word_end, vim_move_previous_line_first_non_whitespace,
    vim_move_previous_paragraph, vim_move_space_backward, vim_move_space_forward,
    vim_move_to_line_column, vim_move_to_matching_bracket, vim_move_up_lines,
    vim_next_paragraph_line, vim_outdent_lines, vim_page_scroll_lines, vim_previous_paragraph_line,
    vim_replace_forward_chars, vim_toggle_case_range,
};
#[cfg(test)]
use self::motion::{vim_open_line_above_text, vim_open_line_below_text};
use self::operator::{
    vim_apply_operator_motion, vim_apply_operator_motion_into_named_register,
    vim_apply_text_object, vim_apply_text_object_into_named_register,
    vim_convert_case_operator_motion, vim_convert_case_text_object, vim_delete_range_into_register,
    vim_delete_to_line_end_into_named_register, vim_text_object_range,
    vim_toggle_case_operator_motion, vim_toggle_case_text_object, vim_yank_operator_motion,
    vim_yank_operator_motion_into_named_register, vim_yank_range_into_register,
    vim_yank_text_object, vim_yank_text_object_into_named_register,
};
use self::parser::{
    vim_combined_count, vim_count_digit, vim_normal_key_can_mutate, vim_normal_key_next_pending,
    vim_normal_key_next_pending_after_count, vim_operator_char_find_motion_for_key,
    vim_operator_go_motion_for_key, vim_operator_motion_for_key, vim_pending_key_accepts,
    vim_pending_key_next_char_find, vim_pending_key_next_named_register,
    vim_pending_key_next_operator_count, vim_pending_key_next_operator_go,
    vim_pending_key_next_text_object, vim_push_count_digit, vim_register_command_count,
    vim_register_command_next_count, vim_text_object_kind_for_key, vim_text_object_scope_for_key,
};
use self::pending::handle_vim_pending_or_direct_normal_key_event;
pub(crate) use self::search::vim_pending_search_status_label;
#[cfg(test)]
use self::search::{VIM_SEARCH_INPUT, VIM_SEARCHES, vim_search_word_target, vim_set_last_search};
use self::search::{
    vim_clear_search_input, vim_delete_search_input_word_backward,
    vim_finish_pending_literal_search, vim_operator_search_match_range,
    vim_operator_search_repeat_range, vim_operator_search_word_under_cursor_range,
    vim_pop_search_input, vim_push_search_input, vim_repeat_last_search, vim_search_match_range,
    vim_search_word_under_cursor,
};
#[cfg(test)]
use self::state::vim_clear_named_registers;
use self::state::{
    vim_jump_to_mark, vim_mark_name_for_key, vim_named_register, vim_named_register_for_key,
    vim_set_mark, vim_write_registers,
};

use self::visual::{
    handle_vim_visual_character_char_find_key_event, handle_vim_visual_character_go_key_event,
    handle_vim_visual_character_key_event, handle_vim_visual_character_register_command_key_event,
    handle_vim_visual_character_register_prefix_key_event,
    handle_vim_visual_character_replace_key_event,
    handle_vim_visual_character_text_object_key_event, vim_cancel_pending_visual_character,
    vim_set_visual_character_selection, vim_visual_character_case_conversion,
    vim_visual_character_change_key, vim_visual_character_delete_key,
    vim_visual_character_indent_key, vim_visual_character_join_key,
    vim_visual_character_outdent_key, vim_visual_pending_after_key,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditorVimMode {
    Normal,
    Insert,
}

impl EditorVimMode {
    pub(crate) fn accepts_text_input(self) -> bool {
        matches!(self, Self::Insert)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditorVimPendingKey {
    ChangeMotionCount {
        operator_count: usize,
        motion_count: usize,
    },
    ChangeMotionCountIntoRegister {
        operator_count: usize,
        motion_count: usize,
        register: EditorVimNamedRegister,
    },
    ChangeTextObject {
        operator_count: usize,
        motion_count: usize,
        scope: EditorVimTextObjectScope,
    },
    ChangeTextObjectIntoRegister {
        operator_count: usize,
        motion_count: usize,
        scope: EditorVimTextObjectScope,
        register: EditorVimNamedRegister,
    },
    ChangeCharFind {
        operator_count: usize,
        motion_count: usize,
        motion: EditorVimCharFindMotion,
    },
    ChangeCharFindIntoRegister {
        operator_count: usize,
        motion_count: usize,
        motion: EditorVimCharFindMotion,
        register: EditorVimNamedRegister,
    },
    Count(usize),
    // Kept in pending state so this one-file visual slice does not add an
    // EditorVimMode variant that external UI matches would need to handle.
    VisualCharacter {
        anchor: usize,
        cursor: usize,
    },
    VisualCharacterCount {
        anchor: usize,
        cursor: usize,
        count: usize,
    },
    VisualCharacterGo {
        anchor: usize,
        cursor: usize,
        count: Option<usize>,
    },
    VisualCharacterReplace {
        anchor: usize,
        cursor: usize,
    },
    VisualCharacterCharFind {
        anchor: usize,
        cursor: usize,
        count: Option<usize>,
        motion: EditorVimCharFindMotion,
    },
    VisualCharacterTextObject {
        anchor: usize,
        cursor: usize,
        count: Option<usize>,
        scope: EditorVimTextObjectScope,
    },
    VisualCharacterRegisterPrefix {
        anchor: usize,
        cursor: usize,
        count: Option<usize>,
    },
    VisualCharacterRegisterCommand {
        anchor: usize,
        cursor: usize,
        count: Option<usize>,
        register: EditorVimNamedRegister,
    },
    RegisterPrefix(usize),
    RegisterCommand {
        prefix_count: usize,
        command_count: Option<usize>,
        register: EditorVimNamedRegister,
    },
    DeleteMotionCount {
        operator_count: usize,
        motion_count: usize,
    },
    DeleteMotionCountIntoRegister {
        operator_count: usize,
        motion_count: usize,
        register: EditorVimNamedRegister,
    },
    DeleteTextObject {
        operator_count: usize,
        motion_count: usize,
        scope: EditorVimTextObjectScope,
    },
    DeleteTextObjectIntoRegister {
        operator_count: usize,
        motion_count: usize,
        scope: EditorVimTextObjectScope,
        register: EditorVimNamedRegister,
    },
    DeleteCharFind {
        operator_count: usize,
        motion_count: usize,
        motion: EditorVimCharFindMotion,
    },
    DeleteCharFindIntoRegister {
        operator_count: usize,
        motion_count: usize,
        motion: EditorVimCharFindMotion,
        register: EditorVimNamedRegister,
    },
    ChangeLine(usize),
    ChangeLineIntoRegister {
        operator_count: usize,
        register: EditorVimNamedRegister,
    },
    DeleteLine(usize),
    DeleteLineIntoRegister {
        operator_count: usize,
        register: EditorVimNamedRegister,
    },
    FindCharBackward(usize),
    FindCharForward(usize),
    Go(Option<usize>),
    IndentLine(usize),
    JumpMark {
        linewise: bool,
    },
    OutdentLine(usize),
    ReplaceChar(usize),
    SearchInput {
        count: usize,
        forward: bool,
    },
    SetMark,
    TillCharBackward(usize),
    TillCharForward(usize),
    OperatorGoMotion {
        operator_count: usize,
        motion_count: usize,
        operator: EditorVimOperatorGoKind,
    },
    ConvertCaseOperator {
        operator_count: usize,
        conversion: EditorVimCaseConversion,
    },
    ConvertCaseMotionCount {
        operator_count: usize,
        motion_count: usize,
        conversion: EditorVimCaseConversion,
    },
    ConvertCaseCharFind {
        operator_count: usize,
        motion_count: usize,
        motion: EditorVimCharFindMotion,
        conversion: EditorVimCaseConversion,
    },
    ConvertCaseTextObject {
        operator_count: usize,
        motion_count: usize,
        scope: EditorVimTextObjectScope,
        conversion: EditorVimCaseConversion,
    },
    ToggleCaseOperator(usize),
    ToggleCaseMotionCount {
        operator_count: usize,
        motion_count: usize,
    },
    ToggleCaseCharFind {
        operator_count: usize,
        motion_count: usize,
        motion: EditorVimCharFindMotion,
    },
    ToggleCaseTextObject {
        operator_count: usize,
        motion_count: usize,
        scope: EditorVimTextObjectScope,
    },
    YankLine(usize),
    YankLineIntoRegister {
        operator_count: usize,
        register: EditorVimNamedRegister,
    },
    YankMotionCount {
        operator_count: usize,
        motion_count: usize,
    },
    YankMotionCountIntoRegister {
        operator_count: usize,
        motion_count: usize,
        register: EditorVimNamedRegister,
    },
    YankTextObject {
        operator_count: usize,
        motion_count: usize,
        scope: EditorVimTextObjectScope,
    },
    YankTextObjectIntoRegister {
        operator_count: usize,
        motion_count: usize,
        scope: EditorVimTextObjectScope,
        register: EditorVimNamedRegister,
    },
    YankCharFind {
        operator_count: usize,
        motion_count: usize,
        motion: EditorVimCharFindMotion,
    },
    YankCharFindIntoRegister {
        operator_count: usize,
        motion_count: usize,
        motion: EditorVimCharFindMotion,
        register: EditorVimNamedRegister,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditorVimRegister {
    text: String,
    kind: EditorVimRegisterKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditorVimLastChange {
    action: EditorVimRepeatAction,
    count: usize,
    insert_replay: Vec<EditorVimInsertReplayStep>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditorVimRegisterKind {
    Characterwise,
    Linewise,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EditorVimNamedRegister {
    index: usize,
    append: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditorVimTextObjectScope {
    Inner,
    Outer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditorVimTextObjectKind {
    Word,
    BigWord,
    Block { open: char, close: char },
    Quote { quote: char },
    Paragraph,
    Sentence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditorVimRepeatAction {
    AppendAfterCursor,
    ChangeLines,
    ChangeLinesIntoRegister(EditorVimNamedRegister),
    ChangeOperatorMotion(EditorVimOperatorMotion),
    ChangeOperatorMotionIntoRegister {
        motion: EditorVimOperatorMotion,
        register: EditorVimNamedRegister,
    },
    ChangeTextObject {
        scope: EditorVimTextObjectScope,
        kind: EditorVimTextObjectKind,
    },
    ChangeTextObjectIntoRegister {
        scope: EditorVimTextObjectScope,
        kind: EditorVimTextObjectKind,
        register: EditorVimNamedRegister,
    },
    ChangeToLineEnd,
    ChangeToLineEndIntoRegister(EditorVimNamedRegister),
    DeleteBackwardChars,
    DeleteBackwardCharsIntoRegister(EditorVimNamedRegister),
    DeleteForwardChars,
    DeleteForwardCharsIntoRegister(EditorVimNamedRegister),
    DeleteLines,
    DeleteLinesIntoRegister(EditorVimNamedRegister),
    DeleteOperatorMotion(EditorVimOperatorMotion),
    DeleteOperatorMotionIntoRegister {
        motion: EditorVimOperatorMotion,
        register: EditorVimNamedRegister,
    },
    DeleteTextObject {
        scope: EditorVimTextObjectScope,
        kind: EditorVimTextObjectKind,
    },
    DeleteTextObjectIntoRegister {
        scope: EditorVimTextObjectScope,
        kind: EditorVimTextObjectKind,
        register: EditorVimNamedRegister,
    },
    DeleteToLineEnd,
    DeleteToLineEndIntoRegister(EditorVimNamedRegister),
    IndentLines,
    InsertAtCursor,
    InsertLineEnd,
    InsertLineFirstNonWhitespace,
    JoinLines,
    JoinLinesWithoutWhitespace,
    OpenLineAbove,
    OpenLineBelow,
    OutdentLines,
    PutAfter,
    PutAfterNamed(EditorVimNamedRegister),
    PutBefore,
    PutBeforeNamed(EditorVimNamedRegister),
    ReplaceForwardChars(char),
    SubstituteForwardChars,
    SubstituteForwardCharsIntoRegister(EditorVimNamedRegister),
    ConvertCaseForwardChars(EditorVimCaseConversion),
    ConvertCaseLines(EditorVimCaseConversion),
    ConvertCaseOperatorMotion {
        motion: EditorVimOperatorMotion,
        conversion: EditorVimCaseConversion,
    },
    ConvertCaseTextObject {
        scope: EditorVimTextObjectScope,
        kind: EditorVimTextObjectKind,
        conversion: EditorVimCaseConversion,
    },
    ToggleCaseForwardChars,
    ToggleCaseOperatorMotion(EditorVimOperatorMotion),
    ToggleCaseTextObject {
        scope: EditorVimTextObjectScope,
        kind: EditorVimTextObjectKind,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditorVimCaseConversion {
    Lower,
    Upper,
    Toggle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum EditorVimInsertReplayStep {
    Backspace,
    DeleteLineBackward,
    DeleteWordBackward,
    Enter,
    EnterAutoIndent,
    InsertText(String),
    Tab,
    ShiftTab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditorVimOperatorMotion {
    BigWordBackward,
    BigWordEnd,
    BigWordEndBackward,
    BigWordForward,
    CharFind {
        motion: EditorVimCharFindMotion,
        target: char,
    },
    CharacterBackward,
    CharacterForward,
    LineColumn,
    LineColumnStart,
    LineEnd,
    LineFirstNonWhitespace,
    MatchingBracket,
    ParagraphBackward,
    ParagraphForward,
    SearchRepeat {
        reverse: bool,
    },
    SearchMatch {
        reverse: bool,
    },
    SearchWordUnderCursor {
        forward: bool,
        whole_word: bool,
    },
    WordBackward,
    WordEnd,
    WordEndBackward,
    WordForward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditorVimOperatorGoKind {
    Change,
    ChangeIntoRegister(EditorVimNamedRegister),
    ConvertCase(EditorVimCaseConversion),
    Delete,
    DeleteIntoRegister(EditorVimNamedRegister),
    ToggleCase,
    Yank,
    YankIntoRegister(EditorVimNamedRegister),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EditorVimCharFind {
    motion: EditorVimCharFindMotion,
    target: char,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditorVimCharFindMotion {
    FindBackward,
    FindForward,
    TillBackward,
    TillForward,
}

impl EditorVimCharFindMotion {
    fn reversed(self) -> Self {
        match self {
            Self::FindBackward => Self::FindForward,
            Self::FindForward => Self::FindBackward,
            Self::TillBackward => Self::TillForward,
            Self::TillForward => Self::TillBackward,
        }
    }
}

const VIM_MAX_COUNT: usize = 999;
const VIM_DEFAULT_CTRL_SCROLL_LINES: usize = 10;
const VIM_DEFAULT_PAGE_SCROLL_LINES: usize = VIM_DEFAULT_CTRL_SCROLL_LINES * 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VimKeyResult {
    pub(crate) handled: bool,
    pub(crate) changed: bool,
    pub(crate) suppress_text: Option<char>,
}

impl VimKeyResult {
    fn ignored() -> Self {
        Self {
            handled: false,
            changed: false,
            suppress_text: None,
        }
    }

    fn handled(suppress_text: Option<char>) -> Self {
        Self {
            handled: true,
            changed: false,
            suppress_text,
        }
    }

    fn changed(suppress_text: Option<char>) -> Self {
        Self {
            handled: true,
            changed: true,
            suppress_text,
        }
    }
}

#[cfg(test)]
pub(crate) fn handle_vim_editor_key_event(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    mode: &mut EditorVimMode,
    pending: &mut Option<EditorVimPendingKey>,
) -> VimKeyResult {
    let mut last_char_find = None;
    let mut unnamed_register = None;
    handle_vim_editor_key_event_with_state(
        buffer,
        key,
        modifiers,
        mode,
        pending,
        &mut last_char_find,
        &mut unnamed_register,
    )
}

#[cfg(test)]
pub(crate) fn handle_vim_editor_key_event_with_state(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    mode: &mut EditorVimMode,
    pending: &mut Option<EditorVimPendingKey>,
    last_char_find: &mut Option<EditorVimCharFind>,
    unnamed_register: &mut Option<EditorVimRegister>,
) -> VimKeyResult {
    let mut last_change = None;
    handle_vim_editor_key_event_with_repeat_state(
        buffer,
        key,
        modifiers,
        mode,
        pending,
        last_char_find,
        unnamed_register,
        &mut last_change,
    )
}

#[cfg(test)]
pub(crate) fn handle_vim_editor_key_event_with_repeat_state(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    mode: &mut EditorVimMode,
    pending: &mut Option<EditorVimPendingKey>,
    last_char_find: &mut Option<EditorVimCharFind>,
    unnamed_register: &mut Option<EditorVimRegister>,
    last_change: &mut Option<EditorVimLastChange>,
) -> VimKeyResult {
    handle_vim_editor_key_event_with_state_and_indent(
        buffer,
        key,
        modifiers,
        mode,
        pending,
        last_char_find,
        unnamed_register,
        last_change,
        "    ",
    )
}

pub(crate) fn handle_vim_editor_key_event_with_state_and_indent(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    mode: &mut EditorVimMode,
    pending: &mut Option<EditorVimPendingKey>,
    last_char_find: &mut Option<EditorVimCharFind>,
    unnamed_register: &mut Option<EditorVimRegister>,
    last_change: &mut Option<EditorVimLastChange>,
    indent_unit: &str,
) -> VimKeyResult {
    match *mode {
        EditorVimMode::Insert => handle_vim_insert_key_event(
            buffer,
            key,
            modifiers,
            mode,
            pending,
            last_char_find,
            last_change,
        ),
        EditorVimMode::Normal => handle_vim_normal_key_event(
            buffer,
            key,
            modifiers,
            mode,
            pending,
            last_char_find,
            unnamed_register,
            last_change,
            indent_unit,
        ),
    }
}

pub(crate) fn vim_events_include_mutation(
    events: &[Event],
    initial_mode: EditorVimMode,
    initial_pending: Option<EditorVimPendingKey>,
) -> bool {
    let mut mode = initial_mode;
    let mut pending = initial_pending;
    let mut suppressed_text = VecDeque::new();
    for event in events {
        match event {
            Event::Cut => return true,
            Event::Paste(text) if mode.accepts_text_input() => {
                if !text.is_empty() {
                    return true;
                }
            }
            Event::Text(text) | Event::Ime(ImeEvent::Commit(text)) => {
                let text = vim_text_after_suppression(text, &mut suppressed_text);
                if mode.accepts_text_input() && text.is_some() {
                    return true;
                }
            }
            Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => match mode {
                EditorVimMode::Insert => {
                    if vim_escape_key(*key, *modifiers) {
                        mode = EditorVimMode::Normal;
                        pending = None;
                    } else if insert_mode_key_can_mutate(*key, *modifiers) {
                        return true;
                    }
                }
                EditorVimMode::Normal => {
                    let printable_key_char = vim_printable_key_char(*key, *modifiers);
                    if vim_normal_key_can_mutate(*key, *modifiers, pending, printable_key_char) {
                        return true;
                    }
                    if vim_escape_key(*key, *modifiers) {
                        pending = None;
                        continue;
                    }
                    if matches!(pending, Some(EditorVimPendingKey::SearchInput { .. })) {
                        if vim_search_input_accept_key(*key, *modifiers) {
                            pending = None;
                            continue;
                        }
                        if vim_search_input_cancel_key(*key, *modifiers) {
                            pending = None;
                            continue;
                        }
                        if vim_search_input_control_edit(*key, *modifiers).is_some() {
                            continue;
                        }
                        if printable_key_char.is_some() {
                            vim_suppress_printable_key_text(
                                printable_key_char,
                                &mut suppressed_text,
                            );
                        }
                        continue;
                    }
                    if let Some(next_pending) =
                        vim_visual_pending_after_key(pending, *key, *modifiers, printable_key_char)
                    {
                        vim_suppress_printable_key_text(printable_key_char, &mut suppressed_text);
                        pending = next_pending;
                    } else if let Some(next_pending) =
                        vim_pending_key_next_named_register(pending, *key, *modifiers)
                    {
                        vim_suppress_printable_key_text(printable_key_char, &mut suppressed_text);
                        pending = Some(next_pending);
                    } else if let Some(next_pending) =
                        vim_pending_key_next_operator_count(pending, *key, *modifiers)
                    {
                        vim_suppress_printable_key_text(printable_key_char, &mut suppressed_text);
                        pending = Some(next_pending);
                    } else if let Some(next_pending) =
                        vim_pending_key_next_operator_go(pending, *key, *modifiers)
                    {
                        vim_suppress_printable_key_text(printable_key_char, &mut suppressed_text);
                        pending = Some(next_pending);
                    } else if let Some(next_pending) =
                        vim_pending_key_next_text_object(pending, *key, *modifiers)
                    {
                        vim_suppress_printable_key_text(printable_key_char, &mut suppressed_text);
                        pending = Some(next_pending);
                    } else if let Some(next_pending) =
                        vim_pending_key_next_char_find(pending, *key, *modifiers)
                    {
                        vim_suppress_printable_key_text(printable_key_char, &mut suppressed_text);
                        pending = Some(next_pending);
                    } else if !matches!(pending, Some(EditorVimPendingKey::Count(_)))
                        && vim_pending_key_accepts(pending, *key, *modifiers, printable_key_char)
                    {
                        vim_suppress_printable_key_text(printable_key_char, &mut suppressed_text);
                        pending = None;
                    } else if let Some(EditorVimPendingKey::Count(count)) = pending
                        && let Some(next_pending) =
                            vim_normal_key_next_pending_after_count(count, *key, *modifiers)
                    {
                        vim_suppress_printable_key_text(printable_key_char, &mut suppressed_text);
                        pending = Some(next_pending);
                    } else if let Some(next_pending) = vim_normal_key_next_pending(*key, *modifiers)
                    {
                        vim_suppress_printable_key_text(printable_key_char, &mut suppressed_text);
                        pending = Some(next_pending);
                    } else if let Some(next_mode) = vim_normal_key_next_mode(*key, *modifiers) {
                        vim_suppress_printable_key_text(printable_key_char, &mut suppressed_text);
                        pending = None;
                        mode = next_mode;
                    } else if vim_normal_key_is_handled(*key, *modifiers) {
                        vim_suppress_printable_key_text(printable_key_char, &mut suppressed_text);
                        if !vim_pending_key_accepts(pending, *key, *modifiers, printable_key_char) {
                            pending = None;
                        }
                    } else {
                        pending = None;
                    }
                }
            },
            _ => {}
        }
    }
    false
}

fn vim_suppress_printable_key_text(
    printable_key_char: Option<char>,
    suppressed_text: &mut VecDeque<char>,
) {
    if let Some(ch) = printable_key_char {
        suppressed_text.push_back(ch);
    }
}

pub(crate) fn vim_text_after_suppression<'a>(
    text: &'a str,
    suppressed_text: &mut VecDeque<char>,
) -> Option<std::borrow::Cow<'a, str>> {
    let Some(expected) = suppressed_text.pop_front() else {
        return (!text.is_empty()).then_some(std::borrow::Cow::Borrowed(text));
    };
    let mut chars = text.char_indices();
    let (_, first) = chars.next()?;
    if first != expected {
        suppressed_text.push_front(expected);
        return Some(std::borrow::Cow::Borrowed(text));
    }
    match chars.next() {
        Some((byte_idx, _)) => Some(std::borrow::Cow::Borrowed(&text[byte_idx..])),
        None => None,
    }
}

fn handle_vim_insert_key_event(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    mode: &mut EditorVimMode,
    pending: &mut Option<EditorVimPendingKey>,
    _last_char_find: &mut Option<EditorVimCharFind>,
    last_change: &mut Option<EditorVimLastChange>,
) -> VimKeyResult {
    if vim_escape_key(key, modifiers) {
        *mode = EditorVimMode::Normal;
        *pending = None;
        VimKeyResult::handled(None)
    } else if vim_insert_delete_char_backward_key(key, modifiers) {
        let changed = buffer.delete_backward_with_auto_pair_delete(false);
        if changed {
            vim_record_insert_replay_step(last_change, EditorVimInsertReplayStep::Backspace);
            VimKeyResult::changed(None)
        } else {
            VimKeyResult::handled(None)
        }
    } else if vim_insert_delete_line_backward_key(key, modifiers) {
        let changed = vim_delete_line_backward(buffer);
        if changed {
            vim_record_insert_replay_step(
                last_change,
                EditorVimInsertReplayStep::DeleteLineBackward,
            );
            VimKeyResult::changed(None)
        } else {
            VimKeyResult::handled(None)
        }
    } else if vim_insert_delete_word_backward_key(key, modifiers) {
        let changed = buffer.delete_word_backward();
        if changed {
            vim_record_insert_replay_step(
                last_change,
                EditorVimInsertReplayStep::DeleteWordBackward,
            );
            VimKeyResult::changed(None)
        } else {
            VimKeyResult::handled(None)
        }
    } else {
        VimKeyResult::ignored()
    }
}

fn vim_repeatable_change_result(
    changed: bool,
    last_change: &mut Option<EditorVimLastChange>,
    action: EditorVimRepeatAction,
    count: usize,
    suppress_text: Option<char>,
) -> VimKeyResult {
    if changed {
        *last_change = Some(EditorVimLastChange {
            action,
            count: count.clamp(1, VIM_MAX_COUNT),
            insert_replay: Vec::new(),
        });
        VimKeyResult::changed(suppress_text)
    } else {
        VimKeyResult::handled(suppress_text)
    }
}

fn handle_vim_operator_go_motion_key_event(
    buffer: &mut TextBuffer,
    mode: &mut EditorVimMode,
    unnamed_register: &mut Option<EditorVimRegister>,
    last_change: &mut Option<EditorVimLastChange>,
    operator_count: usize,
    motion_count: usize,
    operator: EditorVimOperatorGoKind,
    motion: EditorVimOperatorMotion,
    suppress_text: Option<char>,
) -> VimKeyResult {
    let count = vim_combined_count(operator_count, motion_count);
    match operator {
        EditorVimOperatorGoKind::Change => {
            let changed = vim_apply_operator_motion(
                buffer,
                operator_count,
                motion_count,
                motion,
                unnamed_register,
            );
            *mode = EditorVimMode::Insert;
            vim_repeatable_change_result(
                changed,
                last_change,
                EditorVimRepeatAction::ChangeOperatorMotion(motion),
                count,
                suppress_text,
            )
        }
        EditorVimOperatorGoKind::ChangeIntoRegister(register) => {
            let changed = vim_apply_operator_motion_into_named_register(
                buffer,
                operator_count,
                motion_count,
                motion,
                unnamed_register,
                register,
            );
            *mode = EditorVimMode::Insert;
            vim_repeatable_change_result(
                changed,
                last_change,
                EditorVimRepeatAction::ChangeOperatorMotionIntoRegister { motion, register },
                count,
                suppress_text,
            )
        }
        EditorVimOperatorGoKind::ConvertCase(conversion) => vim_repeatable_change_result(
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
        ),
        EditorVimOperatorGoKind::Delete => vim_repeatable_change_result(
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
        ),
        EditorVimOperatorGoKind::DeleteIntoRegister(register) => vim_repeatable_change_result(
            vim_apply_operator_motion_into_named_register(
                buffer,
                operator_count,
                motion_count,
                motion,
                unnamed_register,
                register,
            ),
            last_change,
            EditorVimRepeatAction::DeleteOperatorMotionIntoRegister { motion, register },
            count,
            suppress_text,
        ),
        EditorVimOperatorGoKind::ToggleCase => vim_repeatable_change_result(
            vim_toggle_case_operator_motion(buffer, operator_count, motion_count, motion),
            last_change,
            EditorVimRepeatAction::ToggleCaseOperatorMotion(motion),
            count,
            suppress_text,
        ),
        EditorVimOperatorGoKind::Yank => {
            vim_yank_operator_motion(
                buffer,
                operator_count,
                motion_count,
                motion,
                unnamed_register,
            );
            VimKeyResult::handled(suppress_text)
        }
        EditorVimOperatorGoKind::YankIntoRegister(register) => {
            vim_yank_operator_motion_into_named_register(
                buffer,
                operator_count,
                motion_count,
                motion,
                unnamed_register,
                register,
            );
            VimKeyResult::handled(suppress_text)
        }
    }
}

fn handle_vim_search_input_key_event(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    pending: &mut Option<EditorVimPendingKey>,
    count: usize,
    forward: bool,
    suppress_text: Option<char>,
) -> VimKeyResult {
    if vim_search_input_accept_key(key, modifiers) {
        vim_finish_pending_literal_search(buffer, count, forward);
        return VimKeyResult::handled(None);
    }

    if let Some(edit) = vim_search_input_control_edit(key, modifiers) {
        match edit {
            EditorVimSearchInputEdit::DeleteCharBackward => vim_pop_search_input(),
            EditorVimSearchInputEdit::Clear => vim_clear_search_input(),
            EditorVimSearchInputEdit::DeleteWordBackward => vim_delete_search_input_word_backward(),
        }
        *pending = Some(EditorVimPendingKey::SearchInput { count, forward });
        return VimKeyResult::handled(None);
    }

    if let Some(ch) = vim_printable_key_char(key, modifiers) {
        vim_push_search_input(ch);
        *pending = Some(EditorVimPendingKey::SearchInput { count, forward });
        return VimKeyResult::handled(suppress_text);
    }

    *pending = Some(EditorVimPendingKey::SearchInput { count, forward });
    VimKeyResult::handled(None)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditorVimSearchInputEdit {
    DeleteCharBackward,
    Clear,
    DeleteWordBackward,
}

fn vim_search_input_control_edit(
    key: Key,
    modifiers: Modifiers,
) -> Option<EditorVimSearchInputEdit> {
    if (key == Key::Backspace && no_text_modifiers(modifiers))
        || vim_insert_delete_char_backward_key(key, modifiers)
    {
        Some(EditorVimSearchInputEdit::DeleteCharBackward)
    } else if vim_insert_delete_line_backward_key(key, modifiers) {
        Some(EditorVimSearchInputEdit::Clear)
    } else if vim_insert_delete_word_backward_key(key, modifiers) {
        Some(EditorVimSearchInputEdit::DeleteWordBackward)
    } else {
        None
    }
}

fn vim_search_input_accept_key(key: Key, modifiers: Modifiers) -> bool {
    (key == Key::Enter && no_text_modifiers(modifiers))
        || (matches!(key, Key::J | Key::M)
            && modifiers.ctrl
            && !modifiers.shift
            && !modifiers.alt
            && !modifiers.command)
}

fn vim_search_input_cancel_key(key: Key, modifiers: Modifiers) -> bool {
    key == Key::C && modifiers.ctrl && !modifiers.shift && !modifiers.alt && !modifiers.command
}

fn vim_record_insert_change(
    last_change: &mut Option<EditorVimLastChange>,
    action: EditorVimRepeatAction,
) {
    *last_change = Some(EditorVimLastChange {
        action,
        count: 1,
        insert_replay: Vec::new(),
    });
}

pub(crate) fn vim_record_inserted_text(last_change: &mut Option<EditorVimLastChange>, text: &str) {
    if text.is_empty() {
        return;
    }
    if let Some(change) = last_change.as_mut()
        && change.action.accepts_inserted_text()
    {
        match change.insert_replay.last_mut() {
            Some(EditorVimInsertReplayStep::InsertText(existing)) => existing.push_str(text),
            _ => change
                .insert_replay
                .push(EditorVimInsertReplayStep::InsertText(text.to_owned())),
        }
    }
}

pub(crate) fn vim_record_insert_replay_key_with_auto_indent(
    last_change: &mut Option<EditorVimLastChange>,
    key: Key,
    modifiers: Modifiers,
    auto_indent: bool,
) {
    let Some(change) = last_change.as_mut() else {
        return;
    };
    if !change.action.accepts_inserted_text() {
        return;
    }
    if modifiers.command || modifiers.alt {
        return;
    }
    if modifiers.ctrl {
        if vim_insert_delete_char_backward_key(key, modifiers) {
            change
                .insert_replay
                .push(EditorVimInsertReplayStep::Backspace);
        } else if vim_insert_delete_line_backward_key(key, modifiers) {
            change
                .insert_replay
                .push(EditorVimInsertReplayStep::DeleteLineBackward);
        } else if vim_insert_delete_word_backward_key(key, modifiers) {
            change
                .insert_replay
                .push(EditorVimInsertReplayStep::DeleteWordBackward);
        }
        return;
    }
    let Some(step) = (match key {
        Key::Backspace if !modifiers.shift => Some(EditorVimInsertReplayStep::Backspace),
        Key::Enter if !modifiers.shift && auto_indent => {
            Some(EditorVimInsertReplayStep::EnterAutoIndent)
        }
        Key::Enter if !modifiers.shift => Some(EditorVimInsertReplayStep::Enter),
        Key::Tab if modifiers.shift => Some(EditorVimInsertReplayStep::ShiftTab),
        Key::Tab => Some(EditorVimInsertReplayStep::Tab),
        _ => None,
    }) else {
        return;
    };
    change.insert_replay.push(step);
}

fn vim_record_insert_replay_step(
    last_change: &mut Option<EditorVimLastChange>,
    step: EditorVimInsertReplayStep,
) {
    let Some(change) = last_change.as_mut() else {
        return;
    };
    if change.action.accepts_inserted_text() {
        change.insert_replay.push(step);
    }
}

impl EditorVimRepeatAction {
    fn accepts_inserted_text(self) -> bool {
        matches!(
            self,
            Self::ChangeLines
                | Self::ChangeLinesIntoRegister(_)
                | Self::ChangeOperatorMotion(_)
                | Self::ChangeOperatorMotionIntoRegister { .. }
                | Self::ChangeTextObject { .. }
                | Self::ChangeTextObjectIntoRegister { .. }
                | Self::ChangeToLineEnd
                | Self::ChangeToLineEndIntoRegister(_)
                | Self::AppendAfterCursor
                | Self::InsertAtCursor
                | Self::InsertLineEnd
                | Self::InsertLineFirstNonWhitespace
                | Self::OpenLineAbove
                | Self::OpenLineBelow
                | Self::SubstituteForwardChars
                | Self::SubstituteForwardCharsIntoRegister(_)
        )
    }

    fn is_plain_insert(self) -> bool {
        matches!(
            self,
            Self::AppendAfterCursor
                | Self::InsertAtCursor
                | Self::InsertLineEnd
                | Self::InsertLineFirstNonWhitespace
        )
    }
}

fn handle_vim_normal_key_event(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    mode: &mut EditorVimMode,
    pending: &mut Option<EditorVimPendingKey>,
    last_char_find: &mut Option<EditorVimCharFind>,
    unnamed_register: &mut Option<EditorVimRegister>,
    last_change: &mut Option<EditorVimLastChange>,
    indent_unit: &str,
) -> VimKeyResult {
    if modifiers.command || modifiers.alt {
        if matches!(*pending, Some(EditorVimPendingKey::SearchInput { .. })) {
            vim_clear_search_input();
        }
        vim_cancel_pending_visual_character(buffer, *pending);
        *pending = None;
        return VimKeyResult::ignored();
    }

    let suppress_text = vim_printable_key_char(key, modifiers);
    let count = if let Some(EditorVimPendingKey::Count(count)) = *pending {
        if let Some(digit) = vim_count_digit(key, modifiers, true) {
            *pending = Some(EditorVimPendingKey::Count(vim_push_count_digit(
                count, digit,
            )));
            return VimKeyResult::handled(suppress_text);
        }
        *pending = None;
        Some(count)
    } else {
        None
    };
    let count_value = count.unwrap_or(1).clamp(1, VIM_MAX_COUNT);
    if vim_escape_key(key, modifiers) {
        if matches!(*pending, Some(EditorVimPendingKey::SearchInput { .. })) {
            vim_clear_search_input();
        }
        vim_cancel_pending_visual_character(buffer, *pending);
        *pending = None;
        return VimKeyResult::handled(None);
    }
    if let Some(EditorVimPendingKey::SearchInput { count, forward }) = *pending
        && vim_search_input_accept_key(key, modifiers)
    {
        *pending = None;
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
    if let Some(EditorVimPendingKey::SearchInput { count, forward }) = *pending
        && vim_search_input_control_edit(key, modifiers).is_some()
    {
        *pending = None;
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
    if matches!(*pending, Some(EditorVimPendingKey::SearchInput { .. }))
        && vim_search_input_cancel_key(key, modifiers)
    {
        vim_clear_search_input();
        *pending = None;
        return VimKeyResult::handled(None);
    }
    if modifiers.ctrl {
        if matches!(*pending, Some(EditorVimPendingKey::SearchInput { .. })) {
            vim_clear_search_input();
        }
        vim_cancel_pending_visual_character(buffer, *pending);
        *pending = None;
        if modifiers.shift {
            return VimKeyResult::ignored();
        }
        return match key {
            Key::R => {
                if buffer.redo() {
                    VimKeyResult::changed(None)
                } else {
                    VimKeyResult::handled(None)
                }
            }
            Key::D => {
                vim_move_down_lines(buffer, vim_ctrl_scroll_lines(count));
                VimKeyResult::handled(None)
            }
            Key::E => {
                vim_move_down_lines(buffer, vim_line_scroll_lines(count));
                VimKeyResult::handled(None)
            }
            Key::F => {
                vim_move_down_lines(buffer, vim_page_scroll_lines(count));
                VimKeyResult::handled(None)
            }
            Key::N => {
                vim_move_down_lines(buffer, vim_line_scroll_lines(count));
                VimKeyResult::handled(None)
            }
            Key::B => {
                vim_move_up_lines(buffer, vim_page_scroll_lines(count));
                VimKeyResult::handled(None)
            }
            Key::P => {
                vim_move_up_lines(buffer, vim_line_scroll_lines(count));
                VimKeyResult::handled(None)
            }
            Key::U => {
                vim_move_up_lines(buffer, vim_ctrl_scroll_lines(count));
                VimKeyResult::handled(None)
            }
            Key::Y => {
                vim_move_up_lines(buffer, vim_line_scroll_lines(count));
                VimKeyResult::handled(None)
            }
            _ => VimKeyResult::ignored(),
        };
    }
    handle_vim_pending_or_direct_normal_key_event(
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

fn vim_go_to_line(buffer: &mut TextBuffer, line_one_based: usize) {
    let line = line_one_based.saturating_sub(1);
    let cursor = buffer.line_column_to_char(line, 0);
    buffer.set_single_cursor(cursor);
}

fn vim_line_range_for_count(buffer: &TextBuffer, count: usize) -> Option<Range<usize>> {
    if buffer.len_lines() == 0 || buffer.len_chars() == 0 {
        return None;
    }

    let count = count.clamp(1, VIM_MAX_COUNT);
    let start_line = buffer.cursor_position().line;
    let end_line = start_line
        .saturating_add(count.saturating_sub(1))
        .min(buffer.len_lines().saturating_sub(1));
    let start = buffer.line_column_to_char(start_line, 0);
    let end = if end_line + 1 < buffer.len_lines() {
        buffer.line_column_to_char(end_line + 1, 0)
    } else {
        buffer.len_chars()
    };
    (start < end).then_some(start..end)
}

fn vim_normal_key_next_mode(key: Key, modifiers: Modifiers) -> Option<EditorVimMode> {
    if modifiers.command || modifiers.alt || modifiers.ctrl {
        return None;
    }
    matches!(key, Key::I | Key::A | Key::O).then_some(EditorVimMode::Insert)
}

fn vim_normal_key_is_handled(key: Key, modifiers: Modifiers) -> bool {
    if vim_escape_key(key, modifiers) {
        return true;
    }
    if modifiers.command || modifiers.alt {
        return false;
    }
    if modifiers.ctrl {
        return matches!(
            key,
            Key::R | Key::B | Key::D | Key::E | Key::F | Key::N | Key::P | Key::U | Key::Y
        ) && !modifiers.shift;
    }
    if vim_search_direction_for_key(key, modifiers).is_some() {
        return true;
    }
    if vim_line_column_motion_key(key, modifiers) {
        return true;
    }
    if matches!(key, Key::Comma | Key::Semicolon) && !modifiers.shift {
        return true;
    }
    if matches!(
        (key, modifiers.shift),
        (Key::Backtick, true)
            | (Key::CloseBracket, true)
            | (Key::Comma, true)
            | (Key::Equals, true)
            | (Key::Num3, true)
            | (Key::Num8, true)
            | (Key::OpenBracket, true)
            | (Key::Period, true)
            | (Key::Quote, true)
    ) {
        return true;
    }
    if key == Key::Period && !modifiers.shift {
        return true;
    }
    if matches!(
        (key, modifiers.shift),
        (Key::Backtick, false) | (Key::M, false) | (Key::Quote, false)
    ) {
        return true;
    }
    if matches!(
        key,
        Key::Backspace | Key::Enter | Key::Home | Key::End | Key::Space
    ) {
        return no_text_modifiers(modifiers);
    }
    matches!(
        key,
        Key::Escape
            | Key::H
            | Key::J
            | Key::K
            | Key::L
            | Key::Minus
            | Key::W
            | Key::E
            | Key::B
            | Key::C
            | Key::D
            | Key::Num0
            | Key::Num4
            | Key::Num5
            | Key::Num6
            | Key::N
            | Key::I
            | Key::A
            | Key::O
            | Key::P
            | Key::S
            | Key::X
            | Key::U
            | Key::Y
            | Key::G
    )
}

fn vim_search_direction_for_key(key: Key, modifiers: Modifiers) -> Option<bool> {
    if modifiers.command || modifiers.alt || modifiers.ctrl {
        return None;
    }
    match (key, modifiers.shift) {
        (Key::Slash, false) => Some(true),
        (Key::Slash, true) | (Key::Questionmark, _) => Some(false),
        _ => None,
    }
}

fn insert_mode_key_can_mutate(key: Key, modifiers: Modifiers) -> bool {
    if ((modifiers.ctrl || modifiers.alt) && matches!(key, Key::Backspace | Key::Delete))
        || vim_insert_delete_char_backward_key(key, modifiers)
        || vim_insert_delete_line_backward_key(key, modifiers)
        || vim_insert_delete_word_backward_key(key, modifiers)
    {
        true
    } else if modifiers.command || modifiers.ctrl {
        matches!(key, Key::Z | Key::Y)
    } else {
        matches!(key, Key::Backspace | Key::Delete | Key::Enter | Key::Tab)
    }
}

fn vim_escape_key(key: Key, modifiers: Modifiers) -> bool {
    (key == Key::Escape && no_text_modifiers(modifiers))
        || (key == Key::OpenBracket
            && modifiers.ctrl
            && !modifiers.shift
            && !modifiers.alt
            && !modifiers.command)
}

fn vim_line_column_motion_key(key: Key, modifiers: Modifiers) -> bool {
    if modifiers.command || modifiers.alt || modifiers.ctrl {
        return false;
    }
    matches!(
        (key, modifiers.shift),
        (Key::Backslash, true) | (Key::Pipe, _)
    )
}

fn vim_insert_delete_line_backward_key(key: Key, modifiers: Modifiers) -> bool {
    key == Key::U && modifiers.ctrl && !modifiers.shift && !modifiers.alt && !modifiers.command
}

fn vim_insert_delete_word_backward_key(key: Key, modifiers: Modifiers) -> bool {
    key == Key::W && modifiers.ctrl && !modifiers.shift && !modifiers.alt && !modifiers.command
}

fn vim_insert_delete_char_backward_key(key: Key, modifiers: Modifiers) -> bool {
    key == Key::H && modifiers.ctrl && !modifiers.shift && !modifiers.alt && !modifiers.command
}

fn vim_printable_key_char(key: Key, modifiers: Modifiers) -> Option<char> {
    if modifiers.command || modifiers.alt || modifiers.ctrl {
        return None;
    }
    let shifted = modifiers.shift;
    match key {
        Key::A => Some(if shifted { 'A' } else { 'a' }),
        Key::B => Some(if shifted { 'B' } else { 'b' }),
        Key::C => Some(if shifted { 'C' } else { 'c' }),
        Key::Colon => Some(':'),
        Key::Comma => Some(if shifted { '<' } else { ',' }),
        Key::D => Some(if shifted { 'D' } else { 'd' }),
        Key::E => Some(if shifted { 'E' } else { 'e' }),
        Key::Equals => Some(if shifted { '+' } else { '=' }),
        Key::Exclamationmark => Some('!'),
        Key::F => Some(if shifted { 'F' } else { 'f' }),
        Key::G => Some(if shifted { 'G' } else { 'g' }),
        Key::H => Some(if shifted { 'H' } else { 'h' }),
        Key::I => Some(if shifted { 'I' } else { 'i' }),
        Key::J => Some(if shifted { 'J' } else { 'j' }),
        Key::K => Some(if shifted { 'K' } else { 'k' }),
        Key::L => Some(if shifted { 'L' } else { 'l' }),
        Key::M => Some(if shifted { 'M' } else { 'm' }),
        Key::Minus => Some(if shifted { '_' } else { '-' }),
        Key::N => Some(if shifted { 'N' } else { 'n' }),
        Key::O => Some(if shifted { 'O' } else { 'o' }),
        Key::P => Some(if shifted { 'P' } else { 'p' }),
        Key::Q => Some(if shifted { 'Q' } else { 'q' }),
        Key::Period => Some(if shifted { '>' } else { '.' }),
        Key::OpenBracket => Some(if shifted { '{' } else { '[' }),
        Key::CloseBracket => Some(if shifted { '}' } else { ']' }),
        Key::OpenCurlyBracket => Some('{'),
        Key::CloseCurlyBracket => Some('}'),
        Key::Plus => Some('+'),
        Key::Questionmark => Some('?'),
        Key::R => Some(if shifted { 'R' } else { 'r' }),
        Key::S => Some(if shifted { 'S' } else { 's' }),
        Key::Semicolon => Some(if shifted { ':' } else { ';' }),
        Key::Slash => Some(if shifted { '?' } else { '/' }),
        Key::Space => Some(' '),
        Key::T => Some(if shifted { 'T' } else { 't' }),
        Key::U => Some(if shifted { 'U' } else { 'u' }),
        Key::V => Some(if shifted { 'V' } else { 'v' }),
        Key::W => Some(if shifted { 'W' } else { 'w' }),
        Key::X => Some(if shifted { 'X' } else { 'x' }),
        Key::Y => Some(if shifted { 'Y' } else { 'y' }),
        Key::Z => Some(if shifted { 'Z' } else { 'z' }),
        Key::Backslash => Some(if shifted { '|' } else { '\\' }),
        Key::Backtick => Some(if shifted { '~' } else { '`' }),
        Key::Pipe => Some('|'),
        Key::Quote => Some(if shifted { '"' } else { '\'' }),
        Key::Num0 if !shifted => Some('0'),
        Key::Num1 if shifted => Some('!'),
        Key::Num1 if !shifted => Some('1'),
        Key::Num2 if shifted => Some('@'),
        Key::Num2 if !shifted => Some('2'),
        Key::Num3 if !shifted => Some('3'),
        Key::Num3 if shifted => Some('#'),
        Key::Num4 if !shifted => Some('4'),
        Key::Num4 if shifted => Some('$'),
        Key::Num5 if !shifted => Some('5'),
        Key::Num5 if shifted => Some('%'),
        Key::Num6 if !shifted => Some('6'),
        Key::Num6 if shifted => Some('^'),
        Key::Num7 if shifted => Some('&'),
        Key::Num7 if !shifted => Some('7'),
        Key::Num8 if !shifted => Some('8'),
        Key::Num8 if shifted => Some('*'),
        Key::Num9 if !shifted => Some('9'),
        Key::Num9 if shifted => Some('('),
        Key::Num0 if shifted => Some(')'),
        _ => None,
    }
}

fn vim_replacement_key_char(key: Key, modifiers: Modifiers) -> Option<char> {
    if key == Key::Enter && no_text_modifiers(modifiers) {
        Some('\n')
    } else {
        vim_printable_key_char(key, modifiers)
    }
}

fn no_text_modifiers(modifiers: Modifiers) -> bool {
    !modifiers.ctrl && !modifiers.command && !modifiers.alt && !modifiers.shift
}

#[cfg(test)]
mod tests;
