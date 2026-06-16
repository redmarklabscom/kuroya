use crate::{
    command_palette_items::MAX_COMMAND_PALETTE_QUERY_PATTERN_CHARS, ui_text::truncate_middle,
};
use std::borrow::Cow;

pub(super) const COMMAND_PALETTE_RESULT_LABEL_LIMIT: usize = 160;
pub(super) const COMMAND_PALETTE_RESULT_CHORD_LIMIT: usize = 80;
pub(super) const COMMAND_PALETTE_RESULT_TEXT_SCAN_CHARS: usize = 4096;
const COMMAND_PALETTE_EMPTY_QUERY_LIMIT: usize = 48;

pub(super) fn normalize_command_palette_result_text_owned(
    text: String,
    max_chars: usize,
) -> String {
    if command_palette_result_text_is_normalized(&text, max_chars) {
        return text;
    }

    normalize_command_palette_result_text(&text, max_chars)
}

fn command_palette_result_text_is_normalized(text: &str, max_chars: usize) -> bool {
    if text.is_empty() {
        return true;
    }

    if max_chars == 0 {
        return false;
    }

    let mut previous_space = false;
    for (chars, ch) in text.chars().enumerate() {
        if chars >= max_chars
            || ch.is_control()
            || is_command_palette_result_format_control(ch)
            || (ch.is_whitespace() && ch != ' ')
        {
            return false;
        }

        if ch == ' ' {
            if chars == 0 || previous_space {
                return false;
            }
            previous_space = true;
        } else {
            previous_space = false;
        }
    }

    !previous_space
}

pub(super) fn command_palette_match_query(query: &str) -> Cow<'_, str> {
    if command_palette_result_text_is_normalized(query, MAX_COMMAND_PALETTE_QUERY_PATTERN_CHARS) {
        Cow::Borrowed(query)
    } else {
        Cow::Owned(normalize_command_palette_result_text(
            query,
            MAX_COMMAND_PALETTE_QUERY_PATTERN_CHARS,
        ))
    }
}

pub(super) fn normalize_command_palette_result_text(text: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }

    let mut normalized = String::with_capacity(text.len().min(max_chars));
    let mut chars = 0usize;
    let mut pending_space = false;

    for (scanned_chars, ch) in text.chars().enumerate() {
        if scanned_chars >= COMMAND_PALETTE_RESULT_TEXT_SCAN_CHARS {
            break;
        }

        if chars >= max_chars {
            break;
        }

        if is_command_palette_result_format_control(ch) {
            continue;
        }

        if ch.is_control() || ch.is_whitespace() {
            pending_space = !normalized.is_empty();
            continue;
        }

        if pending_space {
            normalized.push(' ');
            chars += 1;
            pending_space = false;
            if chars >= max_chars {
                break;
            }
        }

        normalized.push(ch);
        chars += 1;
    }

    while normalized.ends_with(' ') {
        normalized.pop();
    }

    normalized
}

fn is_command_palette_result_format_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{00ad}'
            | '\u{061c}'
            | '\u{180e}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{206f}'
            | '\u{feff}'
    )
}

pub(super) fn command_palette_empty_state_label(query: &str) -> String {
    let mut label = String::new();
    command_palette_empty_state_label_into(&mut label, query);
    label
}

pub(super) fn command_palette_empty_state_label_into(label: &mut String, query: &str) {
    label.clear();
    let query = command_palette_match_query(query);
    let query = query.as_ref();
    if query.is_empty() {
        label.push_str("No commands available");
    } else {
        let _ = std::fmt::Write::write_fmt(
            label,
            format_args!(
                "No commands match \"{}\"",
                truncate_middle(query, COMMAND_PALETTE_EMPTY_QUERY_LIMIT)
            ),
        );
    }
}

pub(super) fn command_palette_result_summary(command_count: usize, query: &str) -> String {
    let mut summary = String::with_capacity(32);
    command_palette_result_summary_into(&mut summary, command_count, query);
    summary
}

pub(super) fn command_palette_result_summary_into(
    summary: &mut String,
    command_count: usize,
    query: &str,
) {
    let noun = if command_count == 1 {
        "command"
    } else {
        "commands"
    };
    summary.clear();
    if command_palette_query_is_blank(query) {
        summary.push_str("Showing ");
        let _ = std::fmt::Write::write_fmt(summary, format_args!("{command_count} {noun}"));
    } else {
        let _ = std::fmt::Write::write_fmt(summary, format_args!("{command_count} {noun} matched"));
    }
}

fn command_palette_query_is_blank(query: &str) -> bool {
    command_palette_match_query(query).is_empty()
}
