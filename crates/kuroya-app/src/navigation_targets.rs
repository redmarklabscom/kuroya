use crate::history::NavigationLocation;
#[cfg(test)]
use kuroya_core::Diagnostic;
use kuroya_core::{GitLineChangeKind, TextBuffer};
#[cfg(test)]
use std::collections::BTreeSet;
#[cfg(test)]
use std::path::PathBuf;
use std::{borrow::Cow, collections::BTreeMap, fmt::Write as _, path::Path};

pub(crate) const NAVIGATION_TARGET_LABEL_MAX_CHARS: usize = 160;
pub(crate) const NAVIGATION_STATUS_MAX_CHARS: usize = 240;
const DIFF_HUNK_HEADER_PREFIX_MAX_CHARS: usize = 4096;

#[cfg(test)]
pub(crate) trait DiagnosticNavigationItem {
    fn as_diagnostic(&self) -> &Diagnostic;
}

#[cfg(test)]
impl DiagnosticNavigationItem for Diagnostic {
    fn as_diagnostic(&self) -> &Diagnostic {
        self
    }
}

#[cfg(test)]
impl DiagnosticNavigationItem for &Diagnostic {
    fn as_diagnostic(&self) -> &Diagnostic {
        self
    }
}

pub(crate) fn navigation_location_label(location: &NavigationLocation) -> String {
    let suffix_chars =
        2 + decimal_digit_count(location.line) + decimal_digit_count(location.column);
    let path_max_chars = NAVIGATION_TARGET_LABEL_MAX_CHARS
        .saturating_sub(suffix_chars)
        .max(1);
    let path = navigation_path_label_with_limit_cow(&location.path, path_max_chars);
    let path = path.as_ref();
    let mut label = String::with_capacity(path.len().saturating_add(suffix_chars));
    label.push_str(path);
    let _ = write!(label, ":{}:{}", location.line, location.column);
    label
}

pub(crate) fn navigation_path_label(path: &Path) -> String {
    navigation_path_label_with_limit(path, NAVIGATION_TARGET_LABEL_MAX_CHARS)
}

pub(crate) fn navigation_target_label(label: &str) -> String {
    bounded_navigation_text(label, NAVIGATION_TARGET_LABEL_MAX_CHARS)
}

pub(crate) fn navigation_status_text(status: impl AsRef<str>) -> String {
    bounded_navigation_text(status.as_ref(), NAVIGATION_STATUS_MAX_CHARS)
}

fn navigation_path_label_with_limit(path: &Path, max_chars: usize) -> String {
    navigation_path_label_with_limit_cow(path, max_chars).into_owned()
}

fn navigation_path_label_with_limit_cow(path: &Path, max_chars: usize) -> Cow<'_, str> {
    match navigation_compact_path_text(path) {
        Cow::Borrowed(label) => bounded_navigation_text_cow(label, max_chars),
        Cow::Owned(label) => {
            Cow::Owned(bounded_navigation_text_cow(&label, max_chars).into_owned())
        }
    }
}

fn navigation_compact_path_text(path: &Path) -> Cow<'_, str> {
    if path.as_os_str().is_empty() {
        return Cow::Borrowed(".");
    }

    path.file_name()
        .and_then(|name| name.to_str())
        .map(Cow::Borrowed)
        .unwrap_or_else(|| Cow::Owned(path.display().to_string()))
}

fn decimal_digit_count(mut value: usize) -> usize {
    let mut digits = 1;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

fn bounded_navigation_text(text: &str, max_chars: usize) -> String {
    bounded_navigation_text_cow(text, max_chars).into_owned()
}

fn bounded_navigation_text_cow(text: &str, max_chars: usize) -> Cow<'_, str> {
    if max_chars == 0 {
        return Cow::Borrowed("");
    }

    if is_simple_navigation_text(text, max_chars) {
        return Cow::Borrowed(text);
    }

    let mut output = String::new();
    let mut chars = 0usize;
    let mut truncated = false;
    let mut last_was_control_space = false;

    for ch in text.chars() {
        if is_navigation_bidi_format_control(ch) {
            continue;
        }

        let ch = if is_navigation_line_break_or_control(ch) {
            if output.is_empty() || output.ends_with(' ') || last_was_control_space {
                continue;
            }
            last_was_control_space = true;
            ' '
        } else {
            last_was_control_space = false;
            ch
        };

        if chars >= max_chars {
            truncated = true;
            break;
        }

        output.push(ch);
        chars += 1;
    }

    if truncated && max_chars > 3 {
        truncate_to_chars(&mut output, max_chars - 3);
        output.push_str("...");
    }

    let normalized = output.trim();
    if normalized.is_empty() {
        Cow::Owned(".".to_owned())
    } else if normalized.len() == output.len() {
        Cow::Owned(output)
    } else {
        Cow::Owned(normalized.to_owned())
    }
}

fn is_simple_navigation_text(text: &str, max_chars: usize) -> bool {
    if text.is_ascii() {
        is_simple_ascii_navigation_text(text, max_chars)
    } else {
        is_simple_unicode_navigation_text(text, max_chars)
    }
}

fn is_simple_ascii_navigation_text(text: &str, max_chars: usize) -> bool {
    if text.is_empty() || text.len() > max_chars {
        return false;
    }

    let bytes = text.as_bytes();
    !matches!(bytes.first(), Some(b' '))
        && !matches!(bytes.last(), Some(b' '))
        && bytes.iter().all(|byte| (b' '..=b'~').contains(byte))
}

fn is_simple_unicode_navigation_text(text: &str, max_chars: usize) -> bool {
    if text.is_empty() || text.trim().len() != text.len() {
        return false;
    }

    for (chars, ch) in text.chars().enumerate() {
        if chars >= max_chars {
            return false;
        }

        if is_navigation_bidi_format_control(ch) || is_navigation_line_break_or_control(ch) {
            return false;
        }
    }

    true
}

fn truncate_to_chars(text: &mut String, max_chars: usize) {
    if let Some((byte_index, _)) = text.char_indices().nth(max_chars) {
        text.truncate(byte_index);
    }
}

fn is_navigation_line_break_or_control(ch: char) -> bool {
    ch.is_control() || matches!(ch, '\u{2028}' | '\u{2029}')
}

fn is_navigation_bidi_format_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{061c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
    )
}

#[cfg(test)]
pub(crate) fn next_diagnostic_index(
    diagnostics: &[impl DiagnosticNavigationItem],
    path: &PathBuf,
    line: usize,
    column: usize,
    direction: isize,
) -> usize {
    if diagnostics.is_empty() {
        return 0;
    }

    if direction < 0 {
        diagnostics
            .iter()
            .rposition(|diagnostic| {
                let diagnostic = diagnostic.as_diagnostic();
                diagnostic.path < *path
                    || (diagnostic.path == *path
                        && (diagnostic.line, diagnostic.column) < (line, column))
            })
            .unwrap_or(diagnostics.len() - 1)
    } else {
        diagnostics
            .iter()
            .position(|diagnostic| {
                let diagnostic = diagnostic.as_diagnostic();
                diagnostic.path > *path
                    || (diagnostic.path == *path
                        && (diagnostic.line, diagnostic.column) > (line, column))
            })
            .unwrap_or(0)
    }
}

#[cfg(test)]
pub(crate) fn normalize_changed_lines_for_buffer(
    changed_lines: BTreeSet<usize>,
    line_count: usize,
) -> BTreeSet<usize> {
    let line_count = line_count.max(1);
    changed_lines
        .into_iter()
        .filter(|line| *line > 0)
        .map(|line| line.min(line_count))
        .collect()
}

pub(crate) fn normalize_changed_line_kinds_for_buffer(
    changed_lines: BTreeMap<usize, GitLineChangeKind>,
    line_count: usize,
) -> BTreeMap<usize, GitLineChangeKind> {
    let line_count = line_count.max(1);
    changed_lines
        .into_iter()
        .filter(|(line, _)| *line > 0)
        .map(|(line, kind)| (line.min(line_count), kind))
        .collect()
}

#[cfg(test)]
pub(crate) fn next_changed_line(
    changed_lines: &BTreeSet<usize>,
    current_line: usize,
    direction: isize,
) -> Option<usize> {
    if changed_lines.is_empty() {
        return None;
    }

    let current_line = current_line.max(1);
    if direction < 0 {
        changed_lines
            .range(..current_line)
            .next_back()
            .copied()
            .or_else(|| changed_lines.iter().next_back().copied())
    } else {
        changed_lines
            .range(current_line.saturating_add(1)..)
            .next()
            .copied()
            .or_else(|| changed_lines.iter().next().copied())
    }
}

pub(crate) fn next_changed_line_kind(
    changed_lines: &BTreeMap<usize, GitLineChangeKind>,
    current_line: usize,
    direction: isize,
) -> Option<usize> {
    if changed_lines.is_empty() {
        return None;
    }

    let current_line = current_line.max(1);
    if direction < 0 {
        changed_lines
            .range(..current_line)
            .next_back()
            .map(|(line, _)| *line)
            .or_else(|| changed_lines.keys().next_back().copied())
    } else {
        changed_lines
            .range(current_line.saturating_add(1)..)
            .next()
            .map(|(line, _)| *line)
            .or_else(|| changed_lines.keys().next().copied())
    }
}

#[cfg(test)]
pub(crate) fn diff_hunk_header_lines(text: &str) -> BTreeSet<usize> {
    text.lines()
        .enumerate()
        .filter_map(|(index, line)| text_line_is_diff_hunk_header(line).then_some(index + 1))
        .collect()
}

#[cfg(test)]
pub(crate) fn diff_hunk_header_lines_for_buffer(buffer: &TextBuffer) -> BTreeSet<usize> {
    (0..buffer.len_lines())
        .filter_map(|index| buffer_line_is_diff_hunk_header(buffer, index).then_some(index + 1))
        .collect()
}

pub(crate) fn next_diff_hunk_header_line_for_buffer(
    buffer: &TextBuffer,
    current_line: usize,
    direction: isize,
) -> Option<usize> {
    let line_count = buffer.len_lines();
    if line_count == 0 {
        return None;
    }

    let current_line = current_line.max(1);
    if current_line > line_count {
        return if direction < 0 {
            find_diff_hunk_header_line(buffer, (0..line_count).rev())
        } else {
            find_diff_hunk_header_line(buffer, 0..line_count)
        };
    }

    if direction < 0 {
        find_diff_hunk_header_line(buffer, (0..current_line.saturating_sub(1)).rev()).or_else(
            || {
                find_diff_hunk_header_line(
                    buffer,
                    (current_line.saturating_sub(1)..line_count).rev(),
                )
            },
        )
    } else {
        find_diff_hunk_header_line(buffer, current_line..line_count)
            .or_else(|| find_diff_hunk_header_line(buffer, 0..current_line))
    }
}

#[cfg(test)]
pub(crate) fn diff_hunk_index_at_line(text: &str, line: usize) -> Option<usize> {
    let line = line.max(1);
    let mut hunk_index = None;
    for (index, text_line) in text.lines().enumerate() {
        let current_line = index + 1;
        if current_line > line {
            break;
        }
        if text_line_is_diff_hunk_header(text_line) {
            hunk_index = Some(hunk_index.map_or(0, |index| index + 1));
        } else if current_line == line && text_line.starts_with("diff --git ") {
            return None;
        }
    }
    hunk_index
}

pub(crate) fn diff_hunk_index_at_buffer_line(buffer: &TextBuffer, line: usize) -> Option<usize> {
    let line_count = buffer.len_lines();
    if line == 0 || line > line_count {
        return None;
    }

    let mut hunk_index = None;
    for index in 0..line {
        let current_line = index + 1;
        if buffer_line_is_diff_hunk_header(buffer, index) {
            hunk_index = Some(hunk_index.map_or(0, |index| index + 1));
        } else if current_line == line && buffer.line_starts_with(index, "diff --git ") {
            return None;
        }
    }
    hunk_index
}

fn find_diff_hunk_header_line(
    buffer: &TextBuffer,
    line_indices: impl Iterator<Item = usize>,
) -> Option<usize> {
    let line_count = buffer.len_lines();
    line_indices
        .filter(|line_idx| *line_idx < line_count)
        .find(|line_idx| buffer_line_is_diff_hunk_header(buffer, *line_idx))
        .map(|line_idx| line_idx + 1)
}

fn buffer_line_is_diff_hunk_header(buffer: &TextBuffer, line_idx: usize) -> bool {
    if !buffer.line_starts_with(line_idx, "@@") {
        return false;
    }

    buffer
        .line_content_prefix(line_idx, DIFF_HUNK_HEADER_PREFIX_MAX_CHARS)
        .is_some_and(|line| text_line_is_diff_hunk_header(&line))
}

fn text_line_is_diff_hunk_header(line: &str) -> bool {
    let line = line.trim_end_matches(['\r', '\n']);
    if !line.starts_with("@@") {
        return false;
    }

    let mut parts = line.split_whitespace();
    let marker = match parts.next() {
        Some(marker) if marker.len() >= 2 && marker.bytes().all(|byte| byte == b'@') => marker,
        _ => return false,
    };

    let mut old_range_count = 0usize;
    let mut new_range_count = 0usize;
    let mut has_non_empty_range = false;
    let mut closed = false;

    for part in parts.by_ref() {
        if part == marker {
            closed = true;
            break;
        }

        let Some((kind, count)) = parse_diff_hunk_range_token(part) else {
            return false;
        };
        has_non_empty_range |= count > 0;
        match kind {
            '-' => old_range_count = old_range_count.saturating_add(1),
            '+' => new_range_count = new_range_count.saturating_add(1),
            _ => return false,
        }
    }

    closed
        && old_range_count == marker.len().saturating_sub(1)
        && new_range_count == 1
        && has_non_empty_range
}

fn parse_diff_hunk_range_token(token: &str) -> Option<(char, usize)> {
    let mut chars = token.chars();
    let kind = chars.next()?;
    if kind != '-' && kind != '+' {
        return None;
    }

    let range = chars.as_str();
    let (start, count) = range.split_once(',').unwrap_or((range, ""));
    if start.is_empty() || (range.contains(',') && count.is_empty()) || count.contains(',') {
        return None;
    }

    let start = start.parse::<usize>().ok()?;
    let count = if count.is_empty() {
        1
    } else {
        count.parse::<usize>().ok()?
    };
    if start == 0 && count > 0 {
        return None;
    }

    Some((kind, count))
}

#[cfg(test)]
mod tests {
    use super::{
        NAVIGATION_STATUS_MAX_CHARS, NAVIGATION_TARGET_LABEL_MAX_CHARS, bounded_navigation_text,
        bounded_navigation_text_cow, diff_hunk_header_lines, diff_hunk_index_at_buffer_line,
        navigation_location_label, navigation_status_text, next_diff_hunk_header_line_for_buffer,
    };
    use crate::history::NavigationLocation;
    use kuroya_core::TextBuffer;
    use std::{borrow::Cow, collections::BTreeSet, path::PathBuf};

    #[test]
    fn navigation_status_text_cow_helper_borrows_clean_ascii_and_unicode() {
        let ascii = "src/main.rs";
        let unicode = "résumé/目标.rs";

        assert!(matches!(
            bounded_navigation_text_cow(ascii, NAVIGATION_STATUS_MAX_CHARS),
            Cow::Borrowed(label) if label == ascii
        ));
        assert!(matches!(
            bounded_navigation_text_cow(unicode, NAVIGATION_STATUS_MAX_CHARS),
            Cow::Borrowed(label) if label == unicode
        ));
    }

    #[test]
    fn navigation_status_text_cow_helper_owns_dirty_truncated_and_fallback_paths() {
        assert!(matches!(
            bounded_navigation_text_cow(" alpha\n\u{202e}beta ", NAVIGATION_STATUS_MAX_CHARS),
            Cow::Owned(label) if label == "alpha beta"
        ));
        assert!(matches!(
            bounded_navigation_text_cow("abcdef", 5),
            Cow::Owned(label) if label == "ab..."
        ));
        assert!(matches!(
            bounded_navigation_text_cow("", NAVIGATION_STATUS_MAX_CHARS),
            Cow::Owned(label) if label == "."
        ));
    }

    #[test]
    fn navigation_status_text_cow_helper_matches_string_wrapper() {
        let cases = [
            ("", NAVIGATION_STATUS_MAX_CHARS),
            ("main.rs", NAVIGATION_STATUS_MAX_CHARS),
            ("résumé.rs", NAVIGATION_STATUS_MAX_CHARS),
            ("  padded  ", NAVIGATION_STATUS_MAX_CHARS),
            ("a\n\u{202e}b", NAVIGATION_STATUS_MAX_CHARS),
            ("abcdef", 5),
            ("abcdef", 3),
            ("anything", 0),
        ];

        for (text, max_chars) in cases {
            assert_eq!(
                bounded_navigation_text_cow(text, max_chars).as_ref(),
                bounded_navigation_text(text, max_chars)
            );
        }
    }

    #[test]
    fn navigation_location_label_stays_bounded_with_suffix() {
        let path = PathBuf::from(format!(
            "workspace/src/{}.rs",
            "a".repeat(NAVIGATION_TARGET_LABEL_MAX_CHARS * 2)
        ));

        let label = navigation_location_label(&NavigationLocation::new(path, 12345, 678));

        assert!(label.chars().count() <= NAVIGATION_TARGET_LABEL_MAX_CHARS);
        assert!(label.contains("..."));
        assert!(label.ends_with(":12345:678"));
    }

    #[test]
    fn navigation_location_label_keeps_clean_unicode_path_suffix_output() {
        let path = PathBuf::from("workspace").join("src").join("überblick.rs");

        let label = navigation_location_label(&NavigationLocation::new(path, 7, 9));

        assert_eq!(label, "überblick.rs:7:9");
    }

    #[test]
    fn navigation_location_label_handles_maximum_line_and_column() {
        let path = PathBuf::from(format!(
            "workspace/src/{}.rs",
            "a".repeat(NAVIGATION_TARGET_LABEL_MAX_CHARS * 2)
        ));

        let label =
            navigation_location_label(&NavigationLocation::new(path, usize::MAX, usize::MAX));

        assert!(label.chars().count() <= NAVIGATION_TARGET_LABEL_MAX_CHARS);
        assert!(label.ends_with(&format!(":{}:{}", usize::MAX, usize::MAX)));
    }

    #[test]
    fn navigation_status_text_strips_controls_and_stays_bounded() {
        let status = navigation_status_text(format!(
            "Jumped\n{}\u{202e}",
            "x".repeat(NAVIGATION_STATUS_MAX_CHARS * 2)
        ));

        assert!(status.chars().count() <= NAVIGATION_STATUS_MAX_CHARS);
        assert!(!status.contains('\n'));
        assert!(!status.contains('\u{202e}'));
    }

    #[test]
    fn diff_hunk_navigation_wraps_from_anchor() {
        let buffer = TextBuffer::from_text(
            1,
            None,
            "diff --git a/main.rs b/main.rs\n@@ -1 +1 @@\n-old\n+new\n@@ -8 +8 @@\n-more\n+less\n"
                .to_owned(),
        );

        assert_eq!(
            next_diff_hunk_header_line_for_buffer(&buffer, 1, 1),
            Some(2)
        );
        assert_eq!(
            next_diff_hunk_header_line_for_buffer(&buffer, 2, 1),
            Some(5)
        );
        assert_eq!(
            next_diff_hunk_header_line_for_buffer(&buffer, 5, 1),
            Some(2)
        );
        assert_eq!(
            next_diff_hunk_header_line_for_buffer(&buffer, 5, -1),
            Some(2)
        );
        assert_eq!(
            next_diff_hunk_header_line_for_buffer(&buffer, 2, -1),
            Some(5)
        );
    }

    #[test]
    fn diff_hunk_navigation_returns_current_hunk_when_it_is_the_only_target() {
        let buffer = TextBuffer::from_text(
            1,
            None,
            "diff --git a/main.rs b/main.rs\n@@ -1 +1 @@\n-old\n+new\n".to_owned(),
        );

        assert_eq!(
            next_diff_hunk_header_line_for_buffer(&buffer, 2, 1),
            Some(2)
        );
        assert_eq!(
            next_diff_hunk_header_line_for_buffer(&buffer, 2, -1),
            Some(2)
        );
    }

    #[test]
    fn diff_hunk_navigation_wraps_from_out_of_range_anchor() {
        let buffer = TextBuffer::from_text(
            1,
            None,
            "@@ -1 +1 @@\n-old\n+new\ncontext\n@@ -8 +8 @@\n-more\n+less\n".to_owned(),
        );

        assert_eq!(
            next_diff_hunk_header_line_for_buffer(&buffer, usize::MAX, 1),
            Some(1)
        );
        assert_eq!(
            next_diff_hunk_header_line_for_buffer(&buffer, usize::MAX, -1),
            Some(5)
        );
    }

    #[test]
    fn diff_hunk_navigation_ignores_malformed_hunk_like_lines() {
        let diff = "\
diff --git a/main.rs b/main.rs
@@ -1 +1 @@
-old
+new
@@ not a hunk @@
still current hunk
@@ -8 +8 @@
-more
+less
";
        let buffer = TextBuffer::from_text(1, None, diff.to_owned());

        assert_eq!(diff_hunk_header_lines(diff), BTreeSet::from([2, 7]));
        assert_eq!(
            next_diff_hunk_header_line_for_buffer(&buffer, 4, 1),
            Some(7)
        );
        assert_eq!(
            next_diff_hunk_header_line_for_buffer(&buffer, 7, -1),
            Some(2)
        );
        assert_eq!(diff_hunk_index_at_buffer_line(&buffer, 6), Some(0));
        assert_eq!(diff_hunk_index_at_buffer_line(&buffer, 8), Some(1));
    }

    #[test]
    fn diff_hunk_navigation_rejects_overflowing_hunk_ranges() {
        let diff = "\
diff --git a/main.rs b/main.rs
@@ -1,999999999999999999999999 +1,1 @@
ignored
@@ -8 +8 @@
-more
+less
";
        let buffer = TextBuffer::from_text(1, None, diff.to_owned());

        assert_eq!(diff_hunk_header_lines(diff), BTreeSet::from([4]));
        assert_eq!(
            next_diff_hunk_header_line_for_buffer(&buffer, 1, 1),
            Some(4)
        );
        assert_eq!(diff_hunk_index_at_buffer_line(&buffer, 3), None);
        assert_eq!(diff_hunk_index_at_buffer_line(&buffer, 5), Some(0));
    }

    #[test]
    fn diff_hunk_index_rejects_invalid_lines_without_scanning_to_eof() {
        let buffer = TextBuffer::from_text(
            1,
            None,
            "diff --git a/main.rs b/main.rs\n@@ -1 +1 @@\n-old\n+new\n".to_owned(),
        );

        assert_eq!(diff_hunk_index_at_buffer_line(&buffer, 0), None);
        assert_eq!(diff_hunk_index_at_buffer_line(&buffer, usize::MAX), None);
        assert_eq!(diff_hunk_index_at_buffer_line(&buffer, 3), Some(0));
    }
}
