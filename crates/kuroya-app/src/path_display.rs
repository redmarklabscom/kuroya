use std::{borrow::Cow, collections::VecDeque, path::Path};

use crate::ui_text::truncate_middle;

pub(crate) const DISPLAY_PATH_LABEL_MAX_CHARS: usize = 120;
pub(crate) const DISPLAY_ERROR_LABEL_MAX_CHARS: usize = 160;

pub(crate) fn compact_path(path: &Path) -> String {
    compact_path_text(path).into_owned()
}

fn compact_path_text(path: &Path) -> Cow<'_, str> {
    if path.as_os_str().is_empty() {
        return Cow::Borrowed(".");
    }

    path.file_name()
        .and_then(|name| name.to_str())
        .map(Cow::Borrowed)
        .unwrap_or_else(|| Cow::Owned(path.display().to_string()))
}

pub(crate) fn display_path_label(path: &Path) -> String {
    display_path_label_cow(path).into_owned()
}

pub(crate) fn display_path_label_cow(path: &Path) -> Cow<'_, str> {
    match compact_path_text(path) {
        Cow::Borrowed(label) => {
            sanitized_display_label_cow(label, DISPLAY_PATH_LABEL_MAX_CHARS, ".")
        }
        Cow::Owned(label) => Cow::Owned(sanitized_owned_display_label(
            label,
            DISPLAY_PATH_LABEL_MAX_CHARS,
            ".",
        )),
    }
}

#[cfg(test)]
pub(crate) fn display_error_label(error: &str) -> String {
    display_error_label_cow(error).into_owned()
}

#[cfg(test)]
pub(crate) fn sanitized_display_label(value: &str, max_chars: usize, fallback: &str) -> String {
    sanitized_display_label_cow(value, max_chars, fallback).into_owned()
}

pub(crate) fn display_error_label_cow(error: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(error, DISPLAY_ERROR_LABEL_MAX_CHARS, "unknown error")
}

pub(crate) fn sanitized_owned_display_label(
    value: String,
    max_chars: usize,
    fallback: &str,
) -> String {
    let label = {
        let raw = value.as_str();
        match sanitized_display_label_cow(raw, max_chars, fallback) {
            Cow::Borrowed(label) if label.as_ptr() == raw.as_ptr() && label.len() == raw.len() => {
                None
            }
            Cow::Borrowed(label) => Some(label.to_owned()),
            Cow::Owned(label) => Some(label),
        }
    };

    label.unwrap_or(value)
}

pub(crate) fn sanitized_display_label_cow<'a>(
    value: &'a str,
    max_chars: usize,
    fallback: &str,
) -> Cow<'a, str> {
    if max_chars == 0 {
        return Cow::Borrowed("");
    }

    if let Some(analysis) = analyze_ascii_display_label(value, max_chars) {
        if analysis.is_simple {
            return Cow::Borrowed(value);
        }
        if !analysis.needs_sanitization {
            let label = if analysis.needs_trim {
                analysis.trimmed(value)
            } else {
                value
            };
            return Cow::Owned(truncate_middle(
                if label.is_empty() { fallback } else { label },
                max_chars,
            ));
        }
        return Cow::Owned(sanitize_ascii_display_label(value, max_chars, fallback));
    } else if !display_label_needs_sanitization(value) {
        if !value.is_empty()
            && !display_label_needs_trim(value)
            && value.chars().count() <= max_chars
        {
            return Cow::Borrowed(value);
        }

        let label = if display_label_needs_trim(value) {
            display_label_trimmed(value)
        } else {
            value
        };
        return Cow::Owned(truncate_middle(
            if label.is_empty() { fallback } else { label },
            max_chars,
        ));
    }

    Cow::Owned(sanitize_unicode_display_label(value, max_chars, fallback))
}

fn sanitize_unicode_display_label(value: &str, max_chars: usize, fallback: &str) -> String {
    let mut sanitized = BoundedUnicodeDisplayLabel::new(max_chars);
    let mut pending_control_space = false;
    let mut pending_whitespace = Vec::new();
    for ch in value.chars() {
        if is_hidden_format_control(ch) {
            continue;
        }

        if ch.is_control() || matches!(ch, '\u{2028}' | '\u{2029}') {
            pending_control_space = sanitized.has_output() || !pending_whitespace.is_empty();
            continue;
        }

        if ch.is_whitespace() {
            if sanitized.has_output() || !pending_whitespace.is_empty() {
                if pending_control_space && ch != ' ' {
                    pending_whitespace.push(' ');
                }
                pending_whitespace.push(ch);
            }
            pending_control_space = false;
            continue;
        }

        if !pending_whitespace.is_empty() {
            for pending in pending_whitespace.drain(..) {
                sanitized.push_char(pending);
            }
            pending_control_space = false;
        } else if pending_control_space {
            sanitized.push_char(' ');
            pending_control_space = false;
        }
        sanitized.push_char(ch);
    }

    sanitized.finish(fallback)
}

fn sanitize_ascii_display_label(value: &str, max_chars: usize, fallback: &str) -> String {
    let mut sanitized = BoundedAsciiDisplayLabel::new(max_chars);
    let mut pending_control_space = false;
    let mut pending_literal_spaces = 0usize;
    for byte in value.bytes() {
        if byte < b' ' || byte == b'\x7f' {
            pending_control_space = sanitized.has_output() || pending_literal_spaces > 0;
            continue;
        }

        if byte == b' ' {
            if sanitized.has_output() || pending_literal_spaces > 0 {
                pending_literal_spaces += 1;
            }
            pending_control_space = false;
            continue;
        }

        if pending_literal_spaces > 0 {
            sanitized.push_repeated_spaces(pending_literal_spaces);
            pending_literal_spaces = 0;
            pending_control_space = false;
        } else if pending_control_space {
            sanitized.push_byte(b' ');
            pending_control_space = false;
        }
        sanitized.push_byte(byte);
    }

    sanitized.finish(fallback)
}

struct BoundedAsciiDisplayLabel {
    max_chars: usize,
    prefix: String,
    tail: VecDeque<u8>,
    tail_capacity: usize,
    total_chars: usize,
}

struct BoundedUnicodeDisplayLabel {
    max_chars: usize,
    prefix: String,
    tail: VecDeque<char>,
    tail_capacity: usize,
    total_chars: usize,
}

impl BoundedUnicodeDisplayLabel {
    fn new(max_chars: usize) -> Self {
        let tail_capacity = display_middle_truncate_tail_chars(max_chars);
        Self {
            max_chars,
            prefix: String::with_capacity(max_chars.min(256)),
            tail: VecDeque::with_capacity(tail_capacity),
            tail_capacity,
            total_chars: 0,
        }
    }

    fn has_output(&self) -> bool {
        self.total_chars > 0
    }

    fn push_char(&mut self, ch: char) {
        if self.total_chars < self.max_chars {
            self.prefix.push(ch);
        }
        if self.tail_capacity > 0 {
            if self.tail.len() == self.tail_capacity {
                self.tail.pop_front();
            }
            self.tail.push_back(ch);
        }
        self.total_chars += 1;
    }

    fn finish(self, fallback: &str) -> String {
        if self.total_chars == 0 {
            return truncate_middle(fallback, self.max_chars);
        }
        if self.total_chars <= self.max_chars {
            return self.prefix;
        }
        if self.max_chars <= 3 {
            return tiny_display_ellipsis(self.max_chars);
        }

        let head = display_middle_truncate_head_chars(self.max_chars);
        let mut label = String::with_capacity(self.max_chars);
        label.extend(self.prefix.chars().take(head));
        label.push_str("...");
        label.extend(self.tail);
        label
    }
}

impl BoundedAsciiDisplayLabel {
    fn new(max_chars: usize) -> Self {
        let tail_capacity = display_middle_truncate_tail_chars(max_chars);
        Self {
            max_chars,
            prefix: String::with_capacity(max_chars.min(256)),
            tail: VecDeque::with_capacity(tail_capacity),
            tail_capacity,
            total_chars: 0,
        }
    }

    fn has_output(&self) -> bool {
        self.total_chars > 0
    }

    fn push_repeated_spaces(&mut self, count: usize) {
        for _ in 0..count {
            self.push_byte(b' ');
        }
    }

    fn push_byte(&mut self, byte: u8) {
        if self.total_chars < self.max_chars {
            self.prefix.push(byte as char);
        }
        if self.tail_capacity > 0 {
            if self.tail.len() == self.tail_capacity {
                self.tail.pop_front();
            }
            self.tail.push_back(byte);
        }
        self.total_chars += 1;
    }

    fn finish(self, fallback: &str) -> String {
        if self.total_chars == 0 {
            return truncate_middle(fallback, self.max_chars);
        }
        if self.total_chars <= self.max_chars {
            return self.prefix;
        }
        if self.max_chars <= 3 {
            return tiny_display_ellipsis(self.max_chars);
        }

        let head = display_middle_truncate_head_chars(self.max_chars);
        let mut label = String::with_capacity(self.max_chars);
        label.push_str(&self.prefix[..head]);
        label.push_str("...");
        for byte in self.tail {
            label.push(byte as char);
        }
        label
    }
}

fn display_middle_truncate_head_chars(max_chars: usize) -> usize {
    let keep = max_chars.saturating_sub(3);
    keep / 2
}

fn display_middle_truncate_tail_chars(max_chars: usize) -> usize {
    let keep = max_chars.saturating_sub(3);
    keep.saturating_sub(keep / 2)
}

fn tiny_display_ellipsis(max_chars: usize) -> String {
    match max_chars {
        0 => String::new(),
        1 => ".".to_owned(),
        2 => "..".to_owned(),
        _ => "...".to_owned(),
    }
}

struct AsciiDisplayLabelAnalysis {
    is_simple: bool,
    needs_sanitization: bool,
    needs_trim: bool,
    trim_start: usize,
    trim_end: usize,
}

impl AsciiDisplayLabelAnalysis {
    fn trimmed<'a>(&self, value: &'a str) -> &'a str {
        &value[self.trim_start..self.trim_end]
    }
}

fn analyze_ascii_display_label(value: &str, max_chars: usize) -> Option<AsciiDisplayLabelAnalysis> {
    let bytes = value.as_bytes();
    let mut needs_sanitization = false;
    for byte in bytes {
        if !byte.is_ascii() {
            return None;
        }
        needs_sanitization |= *byte < b' ' || *byte == b'\x7f';
    }

    let mut trim_start = 0;
    while trim_start < bytes.len() && bytes[trim_start].is_ascii_whitespace() {
        trim_start += 1;
    }
    let mut trim_end = bytes.len();
    while trim_end > trim_start && bytes[trim_end - 1].is_ascii_whitespace() {
        trim_end -= 1;
    }

    let needs_trim = trim_start != 0 || trim_end != bytes.len();
    Some(AsciiDisplayLabelAnalysis {
        is_simple: !value.is_empty()
            && value.len() <= max_chars
            && !needs_sanitization
            && !needs_trim,
        needs_sanitization,
        needs_trim,
        trim_start,
        trim_end,
    })
}

fn display_label_needs_sanitization(value: &str) -> bool {
    for (index, byte) in value.bytes().enumerate() {
        if !byte.is_ascii() {
            return value[index..].chars().any(|ch| {
                is_hidden_format_control(ch)
                    || ch.is_control()
                    || matches!(ch, '\u{2028}' | '\u{2029}')
            });
        }
        if byte < b' ' || byte == b'\x7f' {
            return true;
        }
    }

    false
}

fn display_label_needs_trim(value: &str) -> bool {
    if value.is_ascii() {
        let bytes = value.as_bytes();
        return bytes.first().is_some_and(|byte| byte.is_ascii_whitespace())
            || bytes.last().is_some_and(|byte| byte.is_ascii_whitespace());
    }

    value.chars().next().is_some_and(char::is_whitespace)
        || value.chars().next_back().is_some_and(char::is_whitespace)
}

fn display_label_trimmed(value: &str) -> &str {
    if !value.is_ascii() {
        return value.trim();
    }

    display_label_trimmed_ascii(value)
}

fn display_label_trimmed_ascii(value: &str) -> &str {
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

fn is_hidden_format_control(ch: char) -> bool {
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

#[cfg(test)]
mod tests {
    use super::{
        DISPLAY_ERROR_LABEL_MAX_CHARS, DISPLAY_PATH_LABEL_MAX_CHARS, compact_path,
        display_error_label, display_error_label_cow, display_path_label, display_path_label_cow,
        sanitized_owned_display_label,
    };
    use std::{borrow::Cow, path::Path};

    #[test]
    fn compact_path_uses_file_name_for_nested_paths() {
        assert_eq!(compact_path(Path::new("workspace/src/main.rs")), "main.rs");
    }

    #[test]
    fn compact_path_keeps_roots_and_empty_paths_visible() {
        assert_eq!(compact_path(Path::new("")), ".");
        assert!(!compact_path(Path::new("/")).is_empty());
    }

    #[test]
    fn display_path_label_sanitizes_and_bounds_display_only_text() {
        let path = Path::new("workspace/src")
            .join(format!("bad\n{}\u{202e}tail.rs", "very-long-".repeat(24)));

        let label = display_path_label(&path);

        assert!(!label.contains('\n'));
        assert!(!label.contains('\u{202e}'));
        assert!(label.contains("..."));
        assert!(label.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);
    }

    #[test]
    fn display_path_label_cow_borrows_clean_file_names() {
        let path = Path::new("workspace/src/main.rs");

        assert!(matches!(
            display_path_label_cow(path),
            Cow::Borrowed("main.rs")
        ));
    }

    #[test]
    fn display_path_label_cow_owns_non_utf8_or_dirty_names() {
        let dirty = Path::new("workspace").join("bad\nname\u{202e}.rs");
        let label = display_path_label_cow(&dirty);

        assert_eq!(label.as_ref(), "bad name.rs");
        assert!(matches!(label, Cow::Owned(_)));

        assert!(matches!(
            display_path_label_cow(Path::new("")),
            Cow::Borrowed(".")
        ));
    }

    #[test]
    fn display_error_label_cow_borrows_clean_errors() {
        assert!(matches!(
            display_error_label_cow("permission denied"),
            Cow::Borrowed("permission denied")
        ));
    }

    #[test]
    fn sanitized_owned_display_label_reuses_clean_owned_strings() {
        let label = String::from("clean.rs");
        let ptr = label.as_ptr();
        let len = label.len();

        let sanitized = sanitized_owned_display_label(label, DISPLAY_PATH_LABEL_MAX_CHARS, ".");

        assert_eq!(sanitized, "clean.rs");
        assert_eq!(sanitized.as_ptr(), ptr);
        assert_eq!(sanitized.len(), len);
    }

    #[test]
    fn display_path_label_falls_back_for_blank_control_names() {
        assert_eq!(super::sanitized_display_label("\n\u{202e}", 12, "."), ".");
    }

    #[test]
    fn display_label_strips_hidden_format_controls_without_touching_visible_text() {
        let label = super::sanitized_display_label(
            "\u{00ad}\u{200b}alpha\u{200c}\u{200d}beta\u{180e}\u{feff}\u{2066}gamma\u{2069}",
            64,
            ".",
        );

        assert_eq!(label, "alphabetagamma");
        assert!(!label.contains('\u{00ad}'));
        assert!(!label.contains('\u{200b}'));
        assert!(!label.contains('\u{200c}'));
        assert!(!label.contains('\u{200d}'));
        assert!(!label.contains('\u{180e}'));
        assert!(!label.contains('\u{feff}'));
        assert!(!label.contains('\u{2066}'));
    }

    #[test]
    fn display_label_strips_full_invisible_format_control_block() {
        let label = super::sanitized_display_label(
            "\u{2060}alpha\u{206a}\u{206b}beta\u{206c}\u{206d}\u{206e}\u{206f}",
            64,
            ".",
        );

        assert_eq!(label, "alphabeta");
        assert!(!('\u{2060}'..='\u{206f}').any(|ch| label.contains(ch)));
    }

    #[test]
    fn display_label_hidden_format_only_uses_fallback() {
        assert_eq!(
            super::sanitized_display_label("\u{200b}\u{200c}\u{feff}\u{2066}", 12, "."),
            "."
        );
    }

    #[test]
    fn display_label_clean_input_still_trims_falls_back_and_bounds() {
        assert_eq!(
            super::sanitized_display_label("clean.rs", 32, "."),
            "clean.rs"
        );
        assert_eq!(
            super::sanitized_display_label("  clean.rs  ", 32, "."),
            "clean.rs"
        );
        assert_eq!(
            super::sanitized_display_label("\u{2003}clean.rs\u{2003}", 32, "."),
            "clean.rs"
        );
        assert_eq!(
            super::sanitized_display_label("clean-Î».rs", 32, "."),
            "clean-Î».rs"
        );
        assert_eq!(super::sanitized_display_label("   ", 32, "."), ".");
        assert_eq!(
            super::sanitized_display_label("abcdefghijklmnopqrstuvwxyz", 12, "."),
            "abcd...vwxyz"
        );
    }

    #[test]
    fn display_label_cow_borrows_clean_ascii_and_unicode() {
        assert!(matches!(
            super::sanitized_display_label_cow("clean.rs", 32, "."),
            Cow::Borrowed("clean.rs")
        ));

        let unicode = "clean-\u{03bb}.rs";
        match super::sanitized_display_label_cow(unicode, 32, ".") {
            Cow::Borrowed(label) => assert_eq!(label, unicode),
            Cow::Owned(label) => panic!("expected borrowed label, got {label:?}"),
        }
    }

    #[test]
    fn display_label_cow_owns_dirty_bounded_and_fallback_paths() {
        let cases = [
            ("  clean.rs  ", 32, "."),
            ("alpha\nbeta", 64, "."),
            ("\u{200b}alpha", 64, "."),
            ("abcdefghijklmnopqrstuvwxyz", 12, "."),
            ("   ", 32, "."),
        ];

        for (value, max_chars, fallback) in cases {
            let label = super::sanitized_display_label_cow(value, max_chars, fallback);

            assert_eq!(
                label.as_ref(),
                super::sanitized_display_label(value, max_chars, fallback)
            );
            assert!(
                matches!(label, Cow::Owned(_)),
                "expected owned label for {value:?}"
            );
        }
    }

    #[test]
    fn display_label_ascii_controls_are_collapsed_before_bounding() {
        let label = super::sanitized_display_label("alpha\x7fbeta\r\ngamma\tfinish", 18, ".");

        assert_eq!(label, "alpha b...a finish");
        assert!(!label.chars().any(char::is_control));
    }

    #[test]
    fn display_label_ascii_sanitizer_collapses_control_runs_without_extra_spaces() {
        assert_eq!(
            super::sanitized_display_label("alpha\r\n\n beta\x7f\tgamma", 64, "."),
            "alpha beta gamma"
        );
        assert_eq!(super::sanitized_display_label("\r\n\t\x7f", 64, "."), ".");
    }

    #[test]
    fn display_label_ascii_sanitizer_bounds_huge_unsafe_labels() {
        let label = super::sanitized_display_label(
            &format!("start\n{}\nfinish   ", "x".repeat(512)),
            16,
            ".",
        );

        assert_eq!(label, "start ... finish");
        assert_eq!(label.chars().count(), 16);
    }

    #[test]
    fn display_label_unicode_sanitizer_bounds_huge_unsafe_labels() {
        let label = super::sanitized_display_label(
            &format!("\u{03b1}\n{}\u{202e}\u{03c9}   ", "\u{4e2d}".repeat(512)),
            16,
            ".",
        );

        assert!(!label.contains('\n'));
        assert!(!label.contains('\u{202e}'));
        assert!(label.contains("..."));
        assert_eq!(label.chars().next(), Some('\u{03b1}'));
        assert_eq!(label.chars().last(), Some('\u{03c9}'));
        assert_eq!(label.chars().count(), 16);
    }

    #[test]
    fn display_error_label_sanitizes_and_bounds_error_text() {
        let error = display_error_label(&format!(
            "first line\nsecond line \u{202e}{}",
            "x".repeat(DISPLAY_ERROR_LABEL_MAX_CHARS * 2)
        ));

        assert!(!error.contains('\n'));
        assert!(!error.contains('\u{202e}'));
        assert!(error.contains("..."));
        assert!(error.chars().count() <= DISPLAY_ERROR_LABEL_MAX_CHARS);
    }
}
