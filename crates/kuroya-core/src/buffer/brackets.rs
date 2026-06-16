use crate::{buffer::AutoPairSettings, syntax::LanguageConfiguration};
use ropey::Rope;

pub(super) fn is_bracket(ch: char) -> bool {
    matches!(ch, '(' | ')' | '[' | ']' | '{' | '}')
}

pub(super) fn is_opening_bracket(ch: char) -> bool {
    matches!(ch, '(' | '[' | '{')
}

pub(super) fn is_closing_bracket(ch: char) -> bool {
    matches!(ch, ')' | ']' | '}')
}

pub(super) fn bracket_color_depth(
    stack: &[char],
    ch: char,
    opening: bool,
    independent_color_pool_per_bracket_type: bool,
) -> usize {
    if !independent_color_pool_per_bracket_type {
        return if opening {
            stack.len()
        } else {
            stack.len().saturating_sub(1)
        };
    }

    let open = if opening {
        ch
    } else {
        matching_pair(ch).unwrap_or(ch)
    };
    let same_type_depth = stack.iter().filter(|candidate| **candidate == open).count();
    if opening {
        same_type_depth
    } else {
        same_type_depth.saturating_sub(1)
    }
}

pub(super) fn auto_pair_close(ch: char, language_config: LanguageConfiguration) -> Option<char> {
    language_config
        .auto_closing_pairs()
        .iter()
        .find_map(|pair| (pair.open == ch).then_some(pair.close))
}

pub(super) fn is_auto_pair_close(ch: char, language_config: LanguageConfiguration) -> bool {
    language_config
        .auto_closing_pairs()
        .iter()
        .any(|pair| pair.close == ch)
}

fn is_quote_pair(ch: char) -> bool {
    matches!(ch, '"' | '\'' | '`')
}

pub(super) fn auto_pair_enabled_for(ch: char, settings: AutoPairSettings) -> bool {
    if is_quote_pair(ch) {
        settings.quotes
    } else {
        settings.brackets
    }
}

pub(super) fn auto_pair_close_enabled_for(
    close: char,
    settings: AutoPairSettings,
    language_config: LanguageConfiguration,
) -> bool {
    language_config
        .auto_closing_pairs()
        .iter()
        .any(|pair| pair.close == close && auto_pair_enabled_for(pair.open, settings))
}

pub(super) fn auto_pair_matches(
    open: char,
    close: char,
    language_config: LanguageConfiguration,
) -> bool {
    auto_pair_close(open, language_config) == Some(close)
}

pub(super) fn matching_pair(ch: char) -> Option<char> {
    match ch {
        '(' => Some(')'),
        ')' => Some('('),
        '[' => Some(']'),
        ']' => Some('['),
        '{' => Some('}'),
        '}' => Some('{'),
        _ => None,
    }
}

pub(super) fn brackets_match(open: char, close: char) -> bool {
    matches!((open, close), ('(', ')') | ('[', ']') | ('{', '}'))
}

pub(super) fn previous_non_whitespace_char(rope: &Rope, before: usize) -> Option<char> {
    let mut idx = before.min(rope.len_chars());
    while idx > 0 {
        idx -= 1;
        let ch = rope.char(idx);
        if !ch.is_whitespace() {
            return Some(ch);
        }
    }
    None
}

pub(super) fn next_non_whitespace_char(rope: &Rope, after: usize) -> Option<char> {
    let mut idx = after.min(rope.len_chars());
    while idx < rope.len_chars() {
        let ch = rope.char(idx);
        if !ch.is_whitespace() {
            return Some(ch);
        }
        idx += 1;
    }
    None
}
