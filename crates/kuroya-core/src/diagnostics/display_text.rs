use std::borrow::Cow;

use super::{DIAGNOSTIC_DISPLAY_TEXT_MAX_CHARS, DIAGNOSTIC_DISPLAY_TRUNCATION_MARKER};

pub fn diagnostic_display_text(value: &str) -> String {
    sanitize_diagnostic_display_text_cow(value, DIAGNOSTIC_DISPLAY_TEXT_MAX_CHARS).into_owned()
}

pub(super) fn sanitize_diagnostic_display_text_cow(value: &str, max_chars: usize) -> Cow<'_, str> {
    if diagnostic_display_text_can_borrow(value, max_chars) {
        Cow::Borrowed(value)
    } else {
        Cow::Owned(sanitize_diagnostic_display_text_owned(value, max_chars))
    }
}

fn diagnostic_display_text_can_borrow(value: &str, max_chars: usize) -> bool {
    if max_chars == 0 || value.is_empty() || value.starts_with(' ') || value.ends_with(' ') {
        return false;
    }

    let mut chars = 0usize;
    let mut previous_was_space = false;
    for ch in value.chars() {
        chars += 1;
        if chars > max_chars {
            return false;
        }

        if ch == ' ' {
            if previous_was_space {
                return false;
            }
            previous_was_space = true;
            continue;
        }

        if ch.is_control() || ch.is_whitespace() || is_diagnostic_display_format_control(ch) {
            return false;
        }
        previous_was_space = false;
    }

    !previous_was_space
}

fn sanitize_diagnostic_display_text_owned(value: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }

    let mut sanitized = String::with_capacity(value.len().min(max_chars));
    let mut chars = 0usize;
    let mut pending_space = false;
    let mut truncated = false;

    for ch in value.trim().chars() {
        if ch.is_control() || ch.is_whitespace() || is_diagnostic_display_format_control(ch) {
            pending_space = chars > 0;
            continue;
        }

        if pending_space && !sanitized.ends_with(' ') {
            if chars >= max_chars {
                truncated = true;
                break;
            }
            sanitized.push(' ');
            chars += 1;
        }
        pending_space = false;

        if chars >= max_chars {
            truncated = true;
            break;
        }
        sanitized.push(ch);
        chars += 1;
    }

    if truncated {
        truncate_diagnostic_display_text(&sanitized, max_chars)
    } else {
        sanitized
    }
}

fn truncate_diagnostic_display_text(value: &str, max_chars: usize) -> String {
    let marker_chars = DIAGNOSTIC_DISPLAY_TRUNCATION_MARKER.chars().count();
    if max_chars <= marker_chars {
        return DIAGNOSTIC_DISPLAY_TRUNCATION_MARKER
            .chars()
            .take(max_chars)
            .collect();
    }

    let keep = max_chars - marker_chars;
    let mut truncated = value.chars().take(keep).collect::<String>();
    while truncated.chars().last().is_some_and(char::is_whitespace) {
        truncated.pop();
    }
    truncated.push_str(DIAGNOSTIC_DISPLAY_TRUNCATION_MARKER);
    truncated
}

pub(super) fn is_diagnostic_display_format_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{00ad}'
            | '\u{034f}'
            | '\u{061c}'
            | '\u{180e}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{206f}'
            | '\u{feff}'
    )
}
