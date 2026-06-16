use super::{TASK_DISPLAY_LABEL_MAX_CHARS, TASK_DISPLAY_TRUNCATION_MARKER};
use std::borrow::Cow;

pub(super) fn task_display_label(value: &str) -> String {
    let label = sanitize_task_display_text(value, TASK_DISPLAY_LABEL_MAX_CHARS, true);
    if label.is_empty() {
        "Task".to_owned()
    } else {
        label
    }
}

pub(super) fn sanitize_task_display_text(
    value: &str,
    max_chars: usize,
    trim_edges: bool,
) -> String {
    sanitize_task_display_text_cow(value, max_chars, trim_edges).into_owned()
}

pub(super) fn sanitize_task_display_text_cow(
    value: &str,
    max_chars: usize,
    trim_edges: bool,
) -> Cow<'_, str> {
    if max_chars == 0 {
        return Cow::Borrowed("");
    }

    let value = if trim_edges {
        trim_task_display_text(value)
    } else {
        value
    };
    if is_simple_task_display_text(value, max_chars, trim_edges) {
        return Cow::Borrowed(value);
    }

    let mut sanitized = String::with_capacity(value.len().min(max_chars));
    let mut count = 0;
    let mut truncated = false;

    for ch in value.chars() {
        if is_task_display_format_control(ch) {
            continue;
        }

        let ch = if ch.is_control() || ch.is_whitespace() {
            if sanitized.is_empty() || sanitized.ends_with(' ') {
                continue;
            }
            ' '
        } else {
            ch
        };

        if count >= max_chars {
            truncated = true;
            break;
        }

        sanitized.push(ch);
        count += 1;
    }

    if trim_edges {
        trim_task_display_text_in_place(&mut sanitized);
    }

    if truncated {
        Cow::Owned(add_display_truncation_marker(
            &sanitized, max_chars, trim_edges,
        ))
    } else {
        Cow::Owned(sanitized)
    }
}

pub(super) fn truncate_task_display_text(
    value: String,
    max_chars: usize,
    trim_edges: bool,
) -> String {
    if value.len() <= max_chars || (!value.is_ascii() && value.chars().count() <= max_chars) {
        value
    } else {
        add_display_truncation_marker(&value, max_chars, trim_edges)
    }
}

fn is_simple_task_display_text(value: &str, max_chars: usize, trim_edges: bool) -> bool {
    if !value.is_ascii() {
        return is_simple_unicode_task_display_text(value, max_chars, trim_edges);
    }

    if value.len() > max_chars {
        return false;
    }

    let mut previous_space = false;
    for (index, byte) in value.bytes().enumerate() {
        match byte {
            b'!'..=b'~' => previous_space = false,
            b' ' if index > 0 && !previous_space => previous_space = true,
            _ => return false,
        }
    }
    !trim_edges || !previous_space
}

fn is_simple_unicode_task_display_text(value: &str, max_chars: usize, trim_edges: bool) -> bool {
    let mut previous_space = false;

    for (index, ch) in value.chars().enumerate() {
        if index >= max_chars {
            return false;
        }

        match ch {
            ' ' if index > 0 && !previous_space => previous_space = true,
            ' ' => return false,
            _ if ch.is_control() || ch.is_whitespace() || is_task_display_format_control(ch) => {
                return false;
            }
            _ => previous_space = false,
        }
    }

    !trim_edges || !previous_space
}

pub(super) fn add_display_truncation_marker(
    value: &str,
    max_chars: usize,
    trim_edges: bool,
) -> String {
    let marker_len = TASK_DISPLAY_TRUNCATION_MARKER.chars().count();
    if max_chars <= marker_len {
        return TASK_DISPLAY_TRUNCATION_MARKER
            .chars()
            .take(max_chars)
            .collect();
    }

    let keep = max_chars - marker_len;
    let mut value = value.chars().take(keep).collect::<String>();
    if trim_edges {
        trim_task_display_text_end_in_place(&mut value);
    }
    value.push_str(TASK_DISPLAY_TRUNCATION_MARKER);
    value
}

fn trim_task_display_text(value: &str) -> &str {
    if !value.is_ascii() {
        return value.trim();
    }

    trim_task_display_text_ascii(value)
}

fn trim_task_display_text_in_place(value: &mut String) {
    let start = value.len() - value.trim_start().len();
    if start > 0 {
        value.drain(..start);
    }
    trim_task_display_text_end_in_place(value);
}

fn trim_task_display_text_end_in_place(value: &mut String) {
    let end = value.trim_end().len();
    value.truncate(end);
}

fn trim_task_display_text_ascii(value: &str) -> &str {
    let bytes = value.as_bytes();
    let mut start = 0;
    while start < bytes.len() && bytes[start].is_ascii_whitespace() {
        start += 1;
    }

    let mut end = bytes.len();
    while end > start && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }

    &value[start..end]
}

pub(super) fn strip_task_display_format_controls(value: &str) -> Cow<'_, str> {
    if value.is_ascii() || !value.chars().any(is_task_display_format_control) {
        return Cow::Borrowed(value);
    }

    Cow::Owned(
        value
            .chars()
            .filter(|ch| !is_task_display_format_control(*ch))
            .collect(),
    )
}

pub(super) fn is_task_display_format_control(ch: char) -> bool {
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
