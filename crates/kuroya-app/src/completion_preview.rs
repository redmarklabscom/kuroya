use kuroya_core::{EditorSuggestPreviewMode, LspCompletionItem};
use std::borrow::Cow;

const MAX_COMPLETION_PREVIEW_CHARS: usize = 160;
const MAX_INLINE_COMPLETION_PREVIEW_CHARS: usize = 80;
const MAX_COMPLETION_PREVIEW_MATCH_CHARS: usize = 512;
const MAX_COMPLETION_PREVIEW_PREFIX_CHARS: usize = 256;
const COMPLETION_PREVIEW_TEXT_SAMPLE_BYTES: usize = 4 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompletionInlinePreview {
    pub(crate) line_idx: usize,
    pub(crate) text: String,
}

pub(crate) fn completion_inline_preview_for_item(
    line_number: usize,
    item: Option<&LspCompletionItem>,
    prefix: &str,
    mode: EditorSuggestPreviewMode,
) -> Option<CompletionInlinePreview> {
    let line_idx = line_number.checked_sub(1)?;
    let text = completion_item_inline_preview_suffix(item?, prefix, mode)?;
    Some(CompletionInlinePreview { line_idx, text })
}

pub(crate) fn completion_item_preview_text(
    item: &LspCompletionItem,
    prefix: &str,
    mode: EditorSuggestPreviewMode,
) -> Option<String> {
    let prefix = completion_preview_normalized_prefix(prefix);
    completion_item_preview_text_for_normalized_prefix(item, prefix.as_ref(), mode)
}

fn completion_item_preview_text_for_normalized_prefix(
    item: &LspCompletionItem,
    normalized_prefix: &str,
    mode: EditorSuggestPreviewMode,
) -> Option<String> {
    let preview = sanitized_completion_preview_text(&item.insert_text)?;
    let match_text = item
        .filter_text
        .as_deref()
        .and_then(completion_preview_match_text)
        .or_else(|| completion_preview_match_text(&item.label))
        .unwrap_or("");

    let visible = match mode {
        EditorSuggestPreviewMode::Prefix => {
            completion_preview_prefix_matches(preview.as_ref(), normalized_prefix)
                || completion_preview_prefix_matches(match_text, normalized_prefix)
        }
        EditorSuggestPreviewMode::Subword => {
            completion_preview_subword_matches(preview.as_ref(), normalized_prefix)
                || completion_preview_subword_matches(match_text, normalized_prefix)
        }
        EditorSuggestPreviewMode::SubwordSmart => {
            completion_preview_prefix_matches(preview.as_ref(), normalized_prefix)
                || completion_preview_prefix_matches(match_text, normalized_prefix)
                || (normalized_prefix.chars().count() >= 2
                    && (completion_preview_subword_matches(preview.as_ref(), normalized_prefix)
                        || completion_preview_subword_matches(match_text, normalized_prefix)))
        }
    };

    visible.then(|| preview.into_owned())
}

fn completion_item_inline_preview_suffix(
    item: &LspCompletionItem,
    prefix: &str,
    mode: EditorSuggestPreviewMode,
) -> Option<String> {
    let prefix = completion_preview_normalized_prefix(prefix);
    let preview = completion_item_preview_text_for_normalized_prefix(item, prefix.as_ref(), mode)?;
    let suffix = completion_preview_prefix_suffix(&preview, prefix.as_ref())?;
    bounded_inline_completion_preview_suffix(suffix)
}

fn sanitized_completion_preview_text(text: &str) -> Option<Cow<'_, str>> {
    let (text, input_truncated) =
        completion_preview_text_sample(text, MAX_COMPLETION_PREVIEW_CHARS);
    if text.is_empty() {
        return None;
    }
    if !input_truncated && completion_preview_text_can_borrow(text, MAX_COMPLETION_PREVIEW_CHARS) {
        return Some(Cow::Borrowed(text));
    }

    let mut output = String::with_capacity(text.len().min(MAX_COMPLETION_PREVIEW_CHARS + 3));
    let mut previous_was_space = false;
    let mut truncated = input_truncated;
    let mut chars_written = 0usize;

    for ch in text.chars() {
        if chars_written >= MAX_COMPLETION_PREVIEW_CHARS {
            truncated = true;
            break;
        }

        if ch.is_control() || ch.is_whitespace() {
            if !previous_was_space && !output.is_empty() {
                output.push(' ');
                chars_written += 1;
            }
            previous_was_space = true;
        } else if is_completion_preview_format_control(ch) {
            continue;
        } else {
            output.push(ch);
            chars_written += 1;
            previous_was_space = false;
        }
    }

    if output.ends_with(' ') {
        output.pop();
    }
    if output.is_empty() {
        return None;
    }

    if truncated {
        output.push_str("...");
        Some(Cow::Owned(output))
    } else {
        Some(Cow::Owned(output))
    }
}

fn completion_preview_text_can_borrow(text: &str, max_chars: usize) -> bool {
    let mut previous_was_space = false;
    for (chars, ch) in text.chars().enumerate() {
        if chars >= max_chars
            || ch.is_control()
            || is_completion_preview_format_control(ch)
            || (ch.is_whitespace() && (ch != ' ' || previous_was_space))
        {
            return false;
        }
        previous_was_space = ch == ' ';
    }
    true
}

fn completion_preview_prefix_matches(text: &str, lowercase_prefix: &str) -> bool {
    completion_preview_lowercase_starts_with(text, lowercase_prefix)
}

fn completion_preview_subword_matches(text: &str, lowercase_prefix: &str) -> bool {
    if lowercase_prefix.is_empty() {
        return true;
    }

    completion_preview_lowercase_contains(text, lowercase_prefix)
        || completion_preview_is_subsequence(lowercase_prefix, text)
}

fn completion_preview_is_subsequence(lowercase_needle: &str, haystack: &str) -> bool {
    let mut haystack = haystack.chars().flat_map(char::to_lowercase);
    lowercase_needle
        .chars()
        .all(|needle_ch| haystack.by_ref().any(|candidate| candidate == needle_ch))
}

fn completion_preview_normalized_prefix(prefix: &str) -> Cow<'_, str> {
    let (prefix, _) = completion_preview_text_sample(prefix, MAX_COMPLETION_PREVIEW_PREFIX_CHARS);
    completion_preview_lowercase_cow(prefix)
}

fn completion_preview_match_text(text: &str) -> Option<&str> {
    let (text, _) = completion_preview_text_sample(text, MAX_COMPLETION_PREVIEW_MATCH_CHARS);
    (!text.is_empty()).then_some(text)
}

fn completion_preview_lowercase_cow(text: &str) -> Cow<'_, str> {
    if completion_preview_needs_lowercase(text) {
        Cow::Owned(text.to_lowercase())
    } else {
        Cow::Borrowed(text)
    }
}

fn completion_preview_lowercase_starts_with(text: &str, lowercase_prefix: &str) -> bool {
    if lowercase_prefix.is_empty() {
        return true;
    }

    if text.is_ascii() && lowercase_prefix.is_ascii() {
        let prefix_len = lowercase_prefix.len();
        return text
            .as_bytes()
            .get(..prefix_len)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(lowercase_prefix.as_bytes()));
    }

    let mut text_lower = text.chars().flat_map(char::to_lowercase);
    lowercase_prefix
        .chars()
        .all(|expected| text_lower.next().is_some_and(|ch| ch == expected))
}

fn completion_preview_lowercase_contains(text: &str, lowercase_needle: &str) -> bool {
    lowercase_needle.is_empty()
        || text.contains(lowercase_needle)
        || completion_preview_ascii_contains_ignore_case(text, lowercase_needle)
        || (completion_preview_needs_lowercase(text)
            && text.to_lowercase().contains(lowercase_needle))
}

fn completion_preview_ascii_contains_ignore_case(text: &str, lowercase_needle: &str) -> bool {
    if !text.is_ascii() || !lowercase_needle.is_ascii() {
        return false;
    }

    let needle = lowercase_needle.as_bytes();
    text.as_bytes()
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle))
}

fn completion_preview_needs_lowercase(text: &str) -> bool {
    if text.is_ascii() {
        return text.bytes().any(|byte| byte.is_ascii_uppercase());
    }

    text.chars().any(|ch| {
        let mut lower = ch.to_lowercase();
        lower.next() != Some(ch) || lower.next().is_some()
    })
}

fn is_completion_preview_format_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{061c}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{206f}'
            | '\u{feff}'
    )
}

fn completion_preview_text_sample(text: &str, max_chars: usize) -> (&str, bool) {
    if max_chars == 0 {
        return ("", !text.is_empty());
    }

    let mut end = text.len().min(
        COMPLETION_PREVIEW_TEXT_SAMPLE_BYTES
            .max(max_chars.saturating_mul(4))
            .saturating_add(4),
    );
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let sample = text[..end].trim();
    if sample.is_empty() {
        return ("", end < text.len());
    }
    if let Some((idx, _)) = sample.char_indices().nth(max_chars) {
        (&sample[..idx], true)
    } else {
        (sample, end < text.len())
    }
}

fn completion_preview_prefix_suffix<'a>(text: &'a str, lowercase_prefix: &str) -> Option<&'a str> {
    if lowercase_prefix.is_empty() {
        return Some(text);
    }

    let mut expected = lowercase_prefix.chars();
    let mut expected_ch = expected.next()?;
    for (idx, text_ch) in text.char_indices() {
        for candidate in text_ch.to_lowercase() {
            if candidate != expected_ch {
                return None;
            }
            if let Some(next) = expected.next() {
                expected_ch = next;
            } else {
                return Some(&text[idx + text_ch.len_utf8()..]);
            }
        }
    }

    None
}

fn bounded_inline_completion_preview_suffix(text: &str) -> Option<String> {
    let mut output = String::with_capacity(text.len().min(MAX_INLINE_COMPLETION_PREVIEW_CHARS + 3));
    let mut chars = text.chars();
    for _ in 0..MAX_INLINE_COMPLETION_PREVIEW_CHARS {
        let Some(ch) = chars.next() else {
            break;
        };
        output.push(ch);
    }

    if output.is_empty() {
        return None;
    }

    if chars.next().is_some() {
        output.push_str("...");
    }

    Some(output)
}

#[cfg(test)]
mod tests {
    use super::{
        COMPLETION_PREVIEW_TEXT_SAMPLE_BYTES, MAX_COMPLETION_PREVIEW_MATCH_CHARS,
        MAX_COMPLETION_PREVIEW_PREFIX_CHARS, MAX_INLINE_COMPLETION_PREVIEW_CHARS,
        completion_inline_preview_for_item, completion_item_inline_preview_suffix,
        completion_item_preview_text, completion_preview_normalized_prefix,
    };
    use kuroya_core::{EditorSuggestPreviewMode, LspCompletionItem};

    fn item() -> LspCompletionItem {
        LspCompletionItem {
            label: "println!".to_owned(),
            detail: Some("macro".to_owned()),
            documentation: None,
            kind: Some(3),
            deprecated: false,
            is_snippet: false,
            sort_text: None,
            filter_text: None,
            preselect: false,
            commit_characters: Vec::new(),
            insert_text: "println!".to_owned(),
            snippet_selection: None,
            snippet_tabstops: Vec::new(),
            snippet_tabstop_groups: Vec::new(),
            text_edit: None,
            insert_text_edit: None,
            additional_text_edits: Vec::new(),
            resolve_payload: None,
        }
    }

    #[test]
    fn completion_preview_respects_preview_mode() {
        let mut completion = item();
        completion.label = "println!".to_owned();
        completion.insert_text = "println!(value);".to_owned();

        assert_eq!(
            completion_item_preview_text(&completion, "pln", EditorSuggestPreviewMode::Prefix),
            None
        );
        assert_eq!(
            completion_item_preview_text(&completion, "pln", EditorSuggestPreviewMode::Subword),
            Some("println!(value);".to_owned())
        );
        assert_eq!(
            completion_item_preview_text(
                &completion,
                "pln",
                EditorSuggestPreviewMode::SubwordSmart
            ),
            Some("println!(value);".to_owned())
        );
        assert_eq!(
            completion_item_preview_text(&completion, "p", EditorSuggestPreviewMode::SubwordSmart),
            Some("println!(value);".to_owned())
        );
    }

    #[test]
    fn completion_preview_is_sanitized_and_bounded() {
        let mut completion = item();
        completion.insert_text = format!(" {}\n\t{}tail", "x".repeat(160), "y".repeat(16));

        let preview =
            completion_item_preview_text(&completion, "", EditorSuggestPreviewMode::Prefix)
                .expect("preview");

        assert!(preview.ends_with("..."));
        assert!(!preview.contains("tail"));
        assert!(!preview.contains('\n'));
        assert!(!preview.contains('\t'));
    }

    #[test]
    fn completion_preview_ignores_visible_tail_after_huge_hidden_prefix() {
        let mut completion = item();
        completion.insert_text = format!(
            "{}println!(value)",
            "\u{202e}".repeat(COMPLETION_PREVIEW_TEXT_SAMPLE_BYTES + 32)
        );

        assert_eq!(
            completion_item_preview_text(&completion, "", EditorSuggestPreviewMode::Prefix),
            None
        );
    }

    #[test]
    fn completion_preview_strips_invisible_format_controls() {
        let mut completion = item();
        completion.insert_text = "print\u{200d}ln\u{202e}!(value)\u{feff}".to_owned();

        let preview =
            completion_item_preview_text(&completion, "pri", EditorSuggestPreviewMode::Prefix)
                .expect("preview");

        assert_eq!(preview, "println!(value)");
        assert!(!preview.contains('\u{200d}'));
        assert!(!preview.contains('\u{202e}'));
        assert!(!preview.contains('\u{feff}'));
    }

    #[test]
    fn completion_preview_bounds_raw_match_text_without_mutating_item() {
        let mut completion = item();
        completion.label = format!(
            "{}needle",
            "x".repeat(MAX_COMPLETION_PREVIEW_MATCH_CHARS + 8)
        );
        completion.insert_text = "insert_text".to_owned();
        let raw_completion = completion.clone();

        assert_eq!(
            completion_item_preview_text(&completion, "needle", EditorSuggestPreviewMode::Subword),
            None
        );
        assert_eq!(completion, raw_completion);
    }

    #[test]
    fn completion_preview_bounds_normalized_prefix() {
        let raw_prefix = format!(
            "{}tail",
            "\u{03bb}".repeat(MAX_COMPLETION_PREVIEW_PREFIX_CHARS + 8)
        );
        let prefix = completion_preview_normalized_prefix(&raw_prefix);

        assert_eq!(prefix.chars().count(), MAX_COMPLETION_PREVIEW_PREFIX_CHARS);
        assert!(!prefix.contains("tail"));
    }

    #[test]
    fn inline_preview_uses_only_prefix_suffixes() {
        let mut completion = item();
        completion.insert_text = "println!(value);".to_owned();

        assert_eq!(
            completion_item_inline_preview_suffix(
                &completion,
                "pri",
                EditorSuggestPreviewMode::Prefix
            )
            .as_deref(),
            Some("ntln!(value);")
        );
        assert_eq!(
            completion_item_inline_preview_suffix(
                &completion,
                "",
                EditorSuggestPreviewMode::Prefix
            )
            .as_deref(),
            Some("println!(value);")
        );
        assert_eq!(
            completion_item_inline_preview_suffix(
                &completion,
                "pln",
                EditorSuggestPreviewMode::Subword
            ),
            None
        );
        assert_eq!(
            completion_item_inline_preview_suffix(
                &completion,
                "println!(value);",
                EditorSuggestPreviewMode::Prefix
            ),
            None
        );

        completion.insert_text = "Println".to_owned();
        assert_eq!(
            completion_item_inline_preview_suffix(
                &completion,
                "pri",
                EditorSuggestPreviewMode::Prefix
            )
            .as_deref(),
            Some("ntln")
        );
    }

    #[test]
    fn completion_preview_matches_uppercase_ascii_without_normalizing_result() {
        let mut completion = item();
        completion.insert_text = "HashMap::new".to_owned();

        assert_eq!(
            completion_item_preview_text(&completion, "map", EditorSuggestPreviewMode::Subword),
            Some("HashMap::new".to_owned())
        );
    }

    #[test]
    fn completion_preview_prefix_matches_ascii_without_lowercase_allocation() {
        let mut completion = item();
        completion.label = "HashMap::new".to_owned();
        completion.insert_text = "HashMap::new".to_owned();

        assert_eq!(
            completion_item_preview_text(&completion, "HASH", EditorSuggestPreviewMode::Prefix),
            Some("HashMap::new".to_owned())
        );
        assert_eq!(
            completion_item_inline_preview_suffix(
                &completion,
                "HASH",
                EditorSuggestPreviewMode::Prefix
            )
            .as_deref(),
            Some("Map::new")
        );
    }

    #[test]
    fn inline_preview_suffix_matches_unicode_case_folding() {
        let mut completion = item();
        completion.label = "r\u{00e9}sum\u{00e9}".to_owned();
        completion.insert_text = "r\u{00e9}sum\u{00e9}".to_owned();

        assert_eq!(
            completion_item_preview_text(
                &completion,
                "R\u{00c9}",
                EditorSuggestPreviewMode::Prefix
            ),
            Some("r\u{00e9}sum\u{00e9}".to_owned())
        );
        assert_eq!(
            completion_item_inline_preview_suffix(
                &completion,
                "R\u{00c9}",
                EditorSuggestPreviewMode::Prefix
            )
            .as_deref(),
            Some("sum\u{00e9}")
        );
    }

    #[test]
    fn inline_preview_tracks_completion_line() {
        let completion = item();

        assert_eq!(
            completion_inline_preview_for_item(
                4,
                Some(&completion),
                "pri",
                EditorSuggestPreviewMode::Prefix
            ),
            Some(super::CompletionInlinePreview {
                line_idx: 3,
                text: "ntln!".to_owned(),
            })
        );
        assert_eq!(
            completion_inline_preview_for_item(
                0,
                Some(&completion),
                "pri",
                EditorSuggestPreviewMode::Prefix
            ),
            None
        );
        assert_eq!(
            completion_inline_preview_for_item(4, None, "pri", EditorSuggestPreviewMode::Prefix),
            None
        );
    }

    #[test]
    fn inline_preview_suffix_is_bounded() {
        let mut completion = item();
        completion.insert_text =
            format!("p{}", "x".repeat(MAX_INLINE_COMPLETION_PREVIEW_CHARS + 8));

        let preview = completion_item_inline_preview_suffix(
            &completion,
            "p",
            EditorSuggestPreviewMode::Prefix,
        )
        .expect("preview");

        assert!(preview.ends_with("..."));
        assert_eq!(
            preview.chars().count(),
            MAX_INLINE_COMPLETION_PREVIEW_CHARS + 3
        );
    }
}
