use std::{borrow::Cow, collections::BTreeMap};

use super::{
    SETTINGS_DISPLAY_TEXT_MAX_CHARS, SETTINGS_DISPLAY_TRUNCATION_MARKER,
    SETTINGS_MAP_KEY_MAX_CHARS, SETTINGS_MAP_MAX_ITEMS, SETTINGS_MAP_VALUE_MAX_CHARS,
    SETTINGS_STRING_MAX_CHARS, replace_if_changed,
};

pub(super) fn trim_string_in_place(value: &mut String) {
    let start = value.len() - value.trim_start().len();
    if start > 0 {
        value.drain(..start);
    }
    let end = value.trim_end().len();
    value.truncate(end);
}

pub(super) fn sanitize_settings_plain_string(value: &mut String) -> bool {
    let normalized =
        match normalize_settings_plain_string_cow(value, SETTINGS_STRING_MAX_CHARS, true) {
            Cow::Borrowed(_) => return false,
            Cow::Owned(normalized) => normalized,
        };
    replace_if_changed(value, normalized)
}

pub(super) fn sanitize_settings_plain_string_with_default(
    value: &mut String,
    fallback: &str,
) -> bool {
    let normalized =
        match normalize_settings_plain_string_cow(value, SETTINGS_STRING_MAX_CHARS, true) {
            Cow::Borrowed(_) if !value.is_empty() => return false,
            Cow::Borrowed(_) => fallback.to_owned(),
            Cow::Owned(normalized) if normalized.is_empty() => fallback.to_owned(),
            Cow::Owned(normalized) => normalized,
        };
    replace_if_changed(value, normalized)
}

pub(super) fn sanitize_settings_optional_string(value: &mut Option<String>) -> bool {
    if matches!(value.as_deref(), Some("")) {
        *value = None;
        return true;
    }

    let normalized = {
        let Some(current) = value.as_deref() else {
            return false;
        };
        match normalize_settings_plain_string_cow(current, SETTINGS_STRING_MAX_CHARS, true) {
            Cow::Borrowed(_) => return false,
            Cow::Owned(normalized) => normalized,
        }
    };
    if normalized.is_empty() {
        *value = None;
        true
    } else {
        *value = Some(normalized);
        true
    }
}

pub(super) fn sanitize_settings_display_string(value: &mut String, fallback: Option<&str>) -> bool {
    let normalized =
        match normalize_settings_display_text_cow(value, SETTINGS_DISPLAY_TEXT_MAX_CHARS, fallback)
        {
            Cow::Borrowed(_) => return false,
            Cow::Owned(normalized) => normalized,
        };
    replace_if_changed(value, normalized)
}

pub(super) fn sanitize_settings_optional_display_string(value: &mut Option<String>) -> bool {
    let normalized = {
        let Some(current) = value.as_deref() else {
            return false;
        };
        match normalize_settings_display_text_cow(current, SETTINGS_DISPLAY_TEXT_MAX_CHARS, None) {
            Cow::Borrowed(_) => return false,
            Cow::Owned(normalized) => normalized,
        }
    };
    if normalized.is_empty() {
        *value = None;
        true
    } else {
        let changed = value.as_ref() != Some(&normalized);
        *value = Some(normalized);
        changed
    }
}

pub(super) fn sanitize_settings_string_list(
    values: &mut Vec<String>,
    max_items: usize,
    max_chars: usize,
    dedupe: bool,
) -> bool {
    let original = std::mem::take(values);
    let mut normalized = Vec::with_capacity(original.len().min(max_items));
    let mut seen = Vec::new();
    let mut changed = original.len() > max_items;

    for value in original {
        if normalized.len() >= max_items {
            changed = true;
            continue;
        }

        let item = normalize_settings_plain_string(&value, max_chars, true);
        if item.is_empty() {
            changed = true;
            continue;
        }
        if dedupe && seen.iter().any(|seen_item: &String| seen_item == &item) {
            changed = true;
            continue;
        }

        changed |= item != value;
        if dedupe {
            seen.push(item.clone());
        }
        normalized.push(item);
    }

    *values = normalized;
    changed
}

pub(super) fn sanitize_settings_enum_list<T: Clone + PartialEq>(
    values: &mut Vec<T>,
    max_items: usize,
) -> bool {
    let original = std::mem::take(values);
    let mut normalized = Vec::with_capacity(original.len().min(max_items));
    let mut changed = original.len() > max_items;

    for value in original {
        if normalized.len() >= max_items {
            changed = true;
            continue;
        }
        if normalized.contains(&value) {
            changed = true;
            continue;
        }
        normalized.push(value);
    }

    *values = normalized;
    changed
}

pub(super) fn sanitize_settings_bool_map(values: &mut BTreeMap<String, bool>) -> bool {
    let original = std::mem::take(values);
    let mut normalized = BTreeMap::new();
    let mut changed = original.len() > SETTINGS_MAP_MAX_ITEMS;

    for (key, value) in original {
        if normalized.len() >= SETTINGS_MAP_MAX_ITEMS {
            changed = true;
            continue;
        }

        let normalized_key =
            normalize_settings_plain_string(&key, SETTINGS_MAP_KEY_MAX_CHARS, true);
        if normalized_key.is_empty() {
            changed = true;
            continue;
        }
        changed |= normalized_key != key;
        changed |= normalized.insert(normalized_key, value).is_some();
    }

    *values = normalized;
    changed
}

pub(super) fn sanitize_settings_string_map(values: &mut BTreeMap<String, String>) -> bool {
    let original = std::mem::take(values);
    let mut normalized = BTreeMap::new();
    let mut changed = original.len() > SETTINGS_MAP_MAX_ITEMS;

    for (key, value) in original {
        if normalized.len() >= SETTINGS_MAP_MAX_ITEMS {
            changed = true;
            continue;
        }

        let normalized_key =
            normalize_settings_plain_string(&key, SETTINGS_MAP_KEY_MAX_CHARS, true);
        let normalized_value =
            normalize_settings_plain_string(&value, SETTINGS_MAP_VALUE_MAX_CHARS, true);
        if normalized_key.is_empty() {
            changed = true;
            continue;
        }
        changed |= normalized_key != key || normalized_value != value;
        changed |= normalized
            .insert(normalized_key, normalized_value)
            .is_some();
    }

    *values = normalized;
    changed
}

pub(super) fn normalize_settings_plain_string(
    value: &str,
    max_chars: usize,
    trim_edges: bool,
) -> String {
    normalize_settings_plain_string_cow(value, max_chars, trim_edges).into_owned()
}

pub(super) fn normalize_settings_plain_string_cow(
    value: &str,
    max_chars: usize,
    trim_edges: bool,
) -> Cow<'_, str> {
    let normalized_input = if trim_edges { value.trim() } else { value };
    let mut changed = normalized_input.len() != value.len();
    let mut chars = 0usize;

    for ch in normalized_input.chars() {
        if chars >= max_chars {
            changed = true;
            break;
        }
        if ch.is_control() || is_settings_format_control(ch) {
            changed = true;
            continue;
        }
        chars += 1;
    }

    if changed {
        Cow::Owned(normalize_settings_plain_string_owned(
            value, max_chars, trim_edges,
        ))
    } else {
        Cow::Borrowed(value)
    }
}

pub(super) fn normalize_settings_plain_string_owned(
    value: &str,
    max_chars: usize,
    trim_edges: bool,
) -> String {
    let value = if trim_edges { value.trim() } else { value };
    let mut normalized = String::with_capacity(value.len().min(max_chars));

    for ch in value.chars() {
        if normalized.chars().count() >= max_chars {
            break;
        }
        if ch.is_control() || is_settings_format_control(ch) {
            continue;
        }
        normalized.push(ch);
    }

    if trim_edges {
        trim_string_in_place(&mut normalized);
    }
    normalized
}

#[cfg(test)]
pub(super) fn normalize_settings_display_text(
    value: &str,
    max_chars: usize,
    fallback: Option<&str>,
) -> String {
    normalize_settings_display_text_cow(value, max_chars, fallback).into_owned()
}

pub(super) fn normalize_settings_display_text_cow<'a>(
    value: &'a str,
    max_chars: usize,
    fallback: Option<&str>,
) -> Cow<'a, str> {
    if is_normalized_settings_display_text(value, max_chars) {
        Cow::Borrowed(value)
    } else {
        Cow::Owned(normalize_settings_display_text_owned(
            value, max_chars, fallback,
        ))
    }
}

pub(super) fn is_normalized_settings_display_text(value: &str, max_chars: usize) -> bool {
    if max_chars == 0 || value.is_empty() {
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
            if chars == 1 || previous_was_space {
                return false;
            }
            previous_was_space = true;
            continue;
        }

        if ch.is_control() || ch.is_whitespace() || is_settings_format_control(ch) {
            return false;
        }
        previous_was_space = false;
    }

    !previous_was_space
}

pub(super) fn normalize_settings_display_text_owned(
    value: &str,
    max_chars: usize,
    fallback: Option<&str>,
) -> String {
    if max_chars == 0 {
        return String::new();
    }

    let mut normalized = String::with_capacity(value.len().min(max_chars));
    let mut chars = 0usize;
    let mut pending_space = false;
    let mut truncated = false;

    for ch in value.trim().chars() {
        if ch.is_control() || ch.is_whitespace() || is_settings_format_control(ch) {
            pending_space = chars > 0;
            continue;
        }

        if pending_space && !normalized.ends_with(' ') {
            if chars >= max_chars {
                truncated = true;
                break;
            }
            normalized.push(' ');
            chars += 1;
        }
        pending_space = false;

        if chars >= max_chars {
            truncated = true;
            break;
        }
        normalized.push(ch);
        chars += 1;
    }

    if truncated {
        normalized = truncate_settings_display_text(&normalized, max_chars);
    }

    trim_string_in_place(&mut normalized);
    if normalized.is_empty() {
        fallback.unwrap_or_default().to_owned()
    } else {
        normalized
    }
}

pub(super) fn truncate_settings_display_text(value: &str, max_chars: usize) -> String {
    let marker_chars = SETTINGS_DISPLAY_TRUNCATION_MARKER.chars().count();
    if max_chars <= marker_chars {
        return SETTINGS_DISPLAY_TRUNCATION_MARKER
            .chars()
            .take(max_chars)
            .collect();
    }

    let keep = max_chars - marker_chars;
    let mut truncated = value.chars().take(keep).collect::<String>();
    trim_string_in_place(&mut truncated);
    truncated.push_str(SETTINGS_DISPLAY_TRUNCATION_MARKER);
    truncated
}

pub(super) fn is_settings_format_control(ch: char) -> bool {
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
