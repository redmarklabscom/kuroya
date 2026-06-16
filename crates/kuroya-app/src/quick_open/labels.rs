#[cfg(test)]
use crate::history::NavigationLocation;
use std::{
    borrow::Cow,
    fmt::Write as _,
    path::{Component, Path},
};

use super::{
    QUICK_OPEN_RESULT_LABEL_MAX_CHARS, QuickOpenQuery, quick_open_normalized_line_column,
    quick_open_workspace_relative_path,
};

pub(crate) fn quick_open_result_label_with_navigation_line_column(
    rel: &str,
    query: &QuickOpenQuery,
    navigation_line_column: Option<(usize, usize)>,
) -> String {
    if query.line.is_some() {
        return quick_open_result_label(rel, query);
    }

    if let Some(line_column) = navigation_line_column {
        quick_open_result_label_from_parts(rel, Some(line_column))
    } else {
        quick_open_result_label_from_parts(rel, None)
    }
}

#[cfg(test)]
pub(crate) fn quick_open_result_label_with_navigation(
    rel: &str,
    query: &QuickOpenQuery,
    navigation_location: Option<&NavigationLocation>,
) -> String {
    quick_open_result_label_with_navigation_line_column(
        rel,
        query,
        navigation_location.map(|location| (location.line, location.column)),
    )
}

pub(crate) fn quick_open_result_label(rel: &str, query: &QuickOpenQuery) -> String {
    if let Some(line) = query.line {
        quick_open_result_label_from_parts(rel, Some((line, query.column)))
    } else {
        quick_open_result_label_from_parts(rel, None)
    }
}

pub(crate) fn quick_open_result_label_from_parts(
    rel: &str,
    line_column: Option<(usize, usize)>,
) -> String {
    let line_column =
        line_column.map(|(line, column)| quick_open_normalized_line_column(line, column));
    let rel_max_chars = QUICK_OPEN_RESULT_LABEL_MAX_CHARS
        .saturating_sub(quick_open_line_column_suffix_chars(line_column))
        .max(1);
    if let Some((line, column)) = line_column {
        let rel = sanitized_quick_open_result_label_text(rel, rel_max_chars, ".");
        let rel = rel.as_ref();
        let mut label = String::with_capacity(rel.len().saturating_add(24));
        label.push_str(rel);
        let _ = write!(label, ":{line}:{column}");
        label
    } else {
        sanitized_quick_open_result_label(rel, rel_max_chars, ".")
    }
}

pub(crate) fn quick_open_line_column_suffix_chars(line_column: Option<(usize, usize)>) -> usize {
    line_column
        .map(|(line, column)| {
            2 + quick_open_decimal_digits(line) + quick_open_decimal_digits(column)
        })
        .unwrap_or_default()
}

pub(crate) fn quick_open_decimal_digits(mut value: usize) -> usize {
    let mut digits = 1;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

pub(crate) fn sanitized_quick_open_result_label(
    value: &str,
    max_chars: usize,
    fallback: &str,
) -> String {
    sanitized_quick_open_result_label_text(value, max_chars, fallback).into_owned()
}

pub(crate) fn sanitized_quick_open_result_label_text<'a>(
    value: &'a str,
    max_chars: usize,
    fallback: &str,
) -> Cow<'a, str> {
    if max_chars == 0 {
        return Cow::Borrowed("");
    }

    if is_clean_quick_open_result_label(value, max_chars) {
        return Cow::Borrowed(value);
    }

    let mut output = String::with_capacity(value.len().min(max_chars.saturating_mul(4)));
    let mut chars = 0usize;
    let mut truncated = false;
    let mut pending_space = false;

    for ch in value.chars() {
        if is_quick_open_format_control(ch) {
            continue;
        }

        if ch.is_control() || matches!(ch, '\u{2028}' | '\u{2029}') {
            pending_space = chars > 0;
            continue;
        }

        if pending_space && !output.ends_with(' ') {
            if chars >= max_chars {
                truncated = true;
                break;
            }
            output.push(' ');
            chars += 1;
        }
        pending_space = false;

        if chars >= max_chars {
            truncated = true;
            break;
        }
        output.push(ch);
        chars += 1;
    }

    if truncated && max_chars > 3 {
        truncate_quick_open_result_label(&mut output, max_chars - 3);
        output.push_str("...");
    }

    let normalized = output.trim();
    if normalized.is_empty() {
        Cow::Owned(fallback.to_owned())
    } else if normalized.len() == output.len() {
        Cow::Owned(output)
    } else {
        Cow::Owned(normalized.to_owned())
    }
}

pub(crate) fn is_clean_quick_open_result_label(value: &str, max_chars: usize) -> bool {
    if value.is_empty() || value.trim().len() != value.len() {
        return false;
    }

    for (chars, ch) in value.chars().enumerate() {
        if chars >= max_chars
            || ch.is_control()
            || matches!(ch, '\u{2028}' | '\u{2029}')
            || is_quick_open_format_control(ch)
        {
            return false;
        }
    }

    true
}

pub(crate) fn truncate_quick_open_result_label(text: &mut String, max_chars: usize) {
    if let Some((byte_index, _)) = text.char_indices().nth(max_chars) {
        text.truncate(byte_index);
    }
}

pub(crate) fn is_quick_open_format_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{061c}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
            | '\u{feff}'
    )
}

pub(crate) fn quick_open_relative_label<'a>(workspace_root: &Path, path: &'a Path) -> Cow<'a, str> {
    if let Ok(relative) = path.strip_prefix(workspace_root)
        && quick_open_relative_label_path_is_clean(relative)
    {
        return quick_open_path_display_label(relative);
    }

    if let Some(relative) = quick_open_workspace_relative_path(workspace_root, path) {
        Cow::Owned(quick_open_path_display_label_owned(&relative))
    } else {
        quick_open_path_display_label(path)
    }
}

pub(crate) fn quick_open_path_display_label(path: &Path) -> Cow<'_, str> {
    let display = path.to_string_lossy();
    #[cfg(windows)]
    {
        Cow::Owned(display.replace('\\', "/"))
    }
    #[cfg(not(windows))]
    {
        display
    }
}

pub(crate) fn quick_open_path_display_label_owned(path: &Path) -> String {
    let display = path.to_string_lossy();
    #[cfg(windows)]
    {
        display.replace('\\', "/")
    }
    #[cfg(not(windows))]
    {
        display.into_owned()
    }
}

pub(crate) fn quick_open_relative_label_path_is_clean(path: &Path) -> bool {
    !path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::Prefix(_) | Component::RootDir
        )
    })
}
