use super::LspTraceEntry;
use crate::path_display::sanitized_display_label_cow;
use std::borrow::Cow;

pub(crate) const MAX_LSP_TRACE_METHOD_CHARS: usize = 120;
pub(crate) const MAX_LSP_TRACE_DETAIL_CHARS: usize = 512;
pub(super) const MAX_LSP_TRACE_FIELD_CHARS: usize = 160;
pub(super) const MAX_LSP_TRACE_LANGUAGE_CHARS: usize = 80;
pub(super) const MAX_LSP_TRACE_TOKEN_CHARS: usize = 120;

#[cfg(test)]
pub(super) fn bounded_lsp_trace_text(value: &str, max_chars: usize) -> String {
    bounded_lsp_trace_text_cow(value, max_chars).into_owned()
}

pub(super) fn bounded_lsp_trace_text_or(value: &str, max_chars: usize, fallback: &str) -> String {
    bounded_lsp_trace_text_or_cow(value, max_chars, fallback).into_owned()
}

pub(super) fn normalize_lsp_trace_text(value: &mut String, max_chars: usize) {
    normalize_lsp_trace_text_or(value, max_chars, "");
}

pub(super) fn normalize_lsp_trace_text_or(value: &mut String, max_chars: usize, fallback: &str) {
    let normalized = match bounded_lsp_trace_text_or_cow(value, max_chars, fallback) {
        Cow::Borrowed(label) if label == value.as_str() => None,
        Cow::Borrowed(label) => Some(label.to_owned()),
        Cow::Owned(label) if label == value.as_str() => None,
        Cow::Owned(label) => Some(label),
    };
    if let Some(normalized) = normalized {
        *value = normalized;
    }
}

pub(super) fn bounded_lsp_trace_text_cow(value: &str, max_chars: usize) -> Cow<'_, str> {
    bounded_lsp_trace_text_or_cow(value, max_chars, "")
}

fn bounded_lsp_trace_text_or_cow<'a>(
    value: &'a str,
    max_chars: usize,
    fallback: &str,
) -> Cow<'a, str> {
    if max_chars == 0 {
        return Cow::Borrowed("");
    }

    match lsp_trace_text_without_format_controls(value) {
        Cow::Borrowed(value) => {
            if value.is_empty() && fallback.is_empty() {
                Cow::Borrowed(value)
            } else {
                sanitized_display_label_cow(value, max_chars, fallback)
            }
        }
        Cow::Owned(value) => sanitized_owned_lsp_trace_text(value, max_chars, fallback),
    }
}

fn sanitized_owned_lsp_trace_text<'a>(
    value: String,
    max_chars: usize,
    fallback: &str,
) -> Cow<'a, str> {
    if value.is_empty() && fallback.is_empty() {
        return Cow::Owned(value);
    }

    let sanitized = sanitized_display_label_cow(&value, max_chars, fallback);
    let borrowed_original = matches!(&sanitized, Cow::Borrowed(label) if *label == value.as_str());
    if borrowed_original {
        drop(sanitized);
        return Cow::Owned(value);
    }
    Cow::Owned(sanitized.into_owned())
}

fn lsp_trace_text_without_format_controls(value: &str) -> Cow<'_, str> {
    if !value.chars().any(is_lsp_trace_format_control) {
        return Cow::Borrowed(value);
    }

    Cow::Owned(
        value
            .chars()
            .filter(|ch| !is_lsp_trace_format_control(*ch))
            .collect(),
    )
}

pub(super) fn lsp_trace_detail_label(detail: String) -> String {
    let mut detail = detail;
    normalize_lsp_trace_text(&mut detail, MAX_LSP_TRACE_DETAIL_CHARS);
    detail
}

pub(super) fn lsp_trace_field_label(value: &str, max_chars: usize, fallback: &str) -> String {
    bounded_lsp_trace_text_or(value, max_chars, fallback)
}

pub(super) struct LspTraceDisplayLabels<'a> {
    pub(super) method: Cow<'a, str>,
    pub(super) detail: Cow<'a, str>,
}

pub(super) fn lsp_trace_entry_display_labels(entry: &LspTraceEntry) -> LspTraceDisplayLabels<'_> {
    LspTraceDisplayLabels {
        method: lsp_trace_display_label(&entry.method, MAX_LSP_TRACE_METHOD_CHARS),
        detail: lsp_trace_display_label(&entry.detail, MAX_LSP_TRACE_DETAIL_CHARS),
    }
}

pub(super) fn lsp_trace_display_label(value: &str, max_chars: usize) -> Cow<'_, str> {
    bounded_lsp_trace_text_cow(value, max_chars)
}

pub(super) fn is_lsp_trace_format_control(ch: char) -> bool {
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
