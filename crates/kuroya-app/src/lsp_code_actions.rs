use crate::{path_display::sanitized_display_label_cow, ui_text::truncate_middle};
use kuroya_core::{Diagnostic, DiagnosticSet, LspCodeAction};
use std::{borrow::Cow, path::Path};

pub(crate) const MAX_CODE_ACTION_CONTEXT_DIAGNOSTICS: usize = 20;
const CODE_ACTION_KIND_LABEL_MAX_CHARS: usize = 48;
const CODE_ACTION_TITLE_LABEL_MAX_CHARS: usize = 180;
const CODE_ACTION_FRAGMENT_SAMPLE_BYTES: usize = 4096;

pub(crate) fn code_action_diagnostics_for_line(
    diagnostics: &DiagnosticSet,
    path: &Path,
    line: usize,
) -> Vec<Diagnostic> {
    diagnostics
        .iter_for_line(path, line)
        .filter(|diagnostic| diagnostic.source != "kuroya-static")
        .take(MAX_CODE_ACTION_CONTEXT_DIAGNOSTICS)
        .cloned()
        .collect()
}

pub(crate) fn sort_code_actions_for_display(actions: &mut [LspCodeAction]) {
    actions.sort_by_cached_key(code_action_sort_key);
}

pub(crate) fn count_auto_import_code_actions(actions: &[LspCodeAction]) -> usize {
    actions
        .iter()
        .filter(|action| is_auto_import_code_action(action))
        .count()
}

pub(crate) fn code_action_display_label(action: &LspCodeAction) -> String {
    let kind = if is_auto_import_code_action(action) {
        "auto-import"
    } else {
        action.kind.as_deref().unwrap_or("quickfix")
    };
    let kind = code_action_display_fragment(kind, CODE_ACTION_KIND_LABEL_MAX_CHARS, "quickfix");
    let title = code_action_display_fragment(
        &action.title,
        CODE_ACTION_TITLE_LABEL_MAX_CHARS,
        "code action",
    );
    let mut label = String::with_capacity(kind.len() + 2 + title.len());
    label.push_str(&kind);
    label.push_str("  ");
    label.push_str(&title);
    label
}

fn code_action_display_fragment<'a>(
    value: &'a str,
    max_chars: usize,
    fallback: &str,
) -> Cow<'a, str> {
    if value.len() > CODE_ACTION_FRAGMENT_SAMPLE_BYTES {
        return Cow::Owned(sanitize_sampled_code_action_display_fragment(
            sample_code_action_display_fragment(value, max_chars),
            max_chars,
            fallback,
        ));
    }

    sanitized_display_label_cow(value, max_chars, fallback)
}

fn sanitize_sampled_code_action_display_fragment(
    sample: String,
    max_chars: usize,
    fallback: &str,
) -> String {
    let label = {
        let raw = sample.as_str();
        match sanitized_display_label_cow(raw, max_chars, fallback) {
            Cow::Borrowed(label) if label.as_ptr() == raw.as_ptr() && label.len() == raw.len() => {
                None
            }
            Cow::Borrowed(label) => Some(label.to_owned()),
            Cow::Owned(label) => Some(label),
        }
    };

    label.unwrap_or(sample)
}

fn sample_code_action_display_fragment(value: &str, max_chars: usize) -> String {
    if max_chars <= 3 {
        return truncate_middle(value, max_chars);
    }

    let keep = max_chars.saturating_sub(3).saturating_mul(2).max(32);
    let head_chars = keep / 2;
    let tail_chars = keep.saturating_sub(head_chars);

    if value.is_ascii() {
        let head_end = head_chars.min(value.len());
        let tail_start = value.len().saturating_sub(tail_chars.min(value.len()));
        let mut sample = String::with_capacity(head_end + value.len() - tail_start);
        sample.push_str(&value[..head_end]);
        sample.push_str(&value[tail_start..]);
        return sample;
    }

    let mut sample = String::new();
    sample.extend(value.chars().take(head_chars));
    let mut tail = value.chars().rev().take(tail_chars).collect::<Vec<_>>();
    tail.reverse();
    sample.extend(tail);
    sample
}

pub(crate) fn is_auto_import_code_action(action: &LspCodeAction) -> bool {
    if action
        .kind
        .as_deref()
        .is_some_and(|kind| code_action_kind_matches(kind, "source.addMissingImports"))
    {
        return true;
    }

    let quickfix_or_unspecified = match action.kind.as_deref() {
        Some(kind) => code_action_kind_matches(kind, "quickfix"),
        None => true,
    };
    quickfix_or_unspecified && title_looks_like_auto_import(&action.title)
}

fn code_action_sort_key(action: &LspCodeAction) -> CodeActionSortKey {
    let title = action.title.to_ascii_lowercase();
    CodeActionSortKey {
        rank: code_action_rank(action, &title),
        kind: normalized_kind(action).to_owned(),
        title,
    }
}

fn code_action_rank(action: &LspCodeAction, lower_title: &str) -> u8 {
    if is_auto_import_code_action_with_lower_title(action, lower_title) {
        return 0;
    }

    let Some(kind) = action.kind.as_deref() else {
        return 1;
    };
    if code_action_kind_matches(kind, "quickfix") {
        1
    } else if code_action_kind_matches(kind, "source.fixAll") {
        2
    } else if code_action_kind_matches(kind, "source.organizeImports") {
        3
    } else if code_action_kind_matches(kind, "source") {
        4
    } else if code_action_kind_matches(kind, "refactor") {
        5
    } else {
        6
    }
}

fn normalized_kind(action: &LspCodeAction) -> &str {
    action.kind.as_deref().unwrap_or("quickfix")
}

fn code_action_kind_matches(kind: &str, expected: &str) -> bool {
    kind == expected
        || kind
            .strip_prefix(expected)
            .is_some_and(|suffix| suffix.starts_with('.'))
}

fn is_auto_import_code_action_with_lower_title(action: &LspCodeAction, lower_title: &str) -> bool {
    if action
        .kind
        .as_deref()
        .is_some_and(|kind| code_action_kind_matches(kind, "source.addMissingImports"))
    {
        return true;
    }

    let quickfix_or_unspecified = match action.kind.as_deref() {
        Some(kind) => code_action_kind_matches(kind, "quickfix"),
        None => true,
    };
    quickfix_or_unspecified && title_looks_like_auto_import_lowercase(lower_title)
}

fn title_looks_like_auto_import_lowercase(title: &str) -> bool {
    let title = title.trim();
    title.starts_with("import ")
        || title.starts_with("add import")
        || title.starts_with("add all missing import")
        || title.starts_with("add missing import")
}

fn title_looks_like_auto_import(title: &str) -> bool {
    let title = title.trim();
    starts_with_ascii_case_insensitive(title, "import ")
        || starts_with_ascii_case_insensitive(title, "add import")
        || starts_with_ascii_case_insensitive(title, "add all missing import")
        || starts_with_ascii_case_insensitive(title, "add missing import")
}

fn starts_with_ascii_case_insensitive(text: &str, prefix: &str) -> bool {
    text.get(..prefix.len())
        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(prefix))
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct CodeActionSortKey {
    rank: u8,
    kind: String,
    title: String,
}

#[cfg(test)]
mod tests {
    use super::{
        CODE_ACTION_FRAGMENT_SAMPLE_BYTES, CODE_ACTION_KIND_LABEL_MAX_CHARS,
        CODE_ACTION_TITLE_LABEL_MAX_CHARS, code_action_display_fragment, code_action_display_label,
        is_auto_import_code_action, sample_code_action_display_fragment,
        sort_code_actions_for_display,
    };
    use crate::path_display::sanitized_display_label;
    use kuroya_core::LspCodeAction;
    use std::borrow::Cow;

    fn action(title: &str, kind: Option<&str>) -> LspCodeAction {
        LspCodeAction {
            title: title.to_owned(),
            kind: kind.map(str::to_owned),
            edits: Vec::new(),
            document_changes: Vec::new(),
            resolve_payload: None,
        }
    }

    #[test]
    fn code_action_display_label_sanitizes_and_bounds_server_text() {
        let long_title = format!(
            "Fix import\n{}\u{202e}tail",
            "unsafe-title-".repeat(CODE_ACTION_TITLE_LABEL_MAX_CHARS)
        );
        let long_kind = format!(
            "quickfix\n{}\u{2066}kind",
            "unsafe-kind-".repeat(CODE_ACTION_KIND_LABEL_MAX_CHARS)
        );
        let label = code_action_display_label(&action(&long_title, Some(&long_kind)));

        assert!(!label.contains('\n'), "{label:?}");
        assert!(!label.contains('\u{202e}'), "{label:?}");
        assert!(!label.contains('\u{2066}'), "{label:?}");
        assert!(label.contains("..."), "{label:?}");
        assert!(
            label.chars().count()
                <= CODE_ACTION_KIND_LABEL_MAX_CHARS + 2 + CODE_ACTION_TITLE_LABEL_MAX_CHARS,
            "{label:?}"
        );
    }

    #[test]
    fn code_action_display_label_falls_back_for_blank_title_and_kind() {
        assert_eq!(
            code_action_display_label(&action("\n\u{202e}", Some("\n\u{2066}"))),
            "quickfix  code action"
        );
    }

    #[test]
    fn code_action_display_fragment_borrows_clean_ascii_fragments() {
        assert!(matches!(
            code_action_display_fragment("quickfix", CODE_ACTION_KIND_LABEL_MAX_CHARS, "fallback"),
            Cow::Borrowed(value) if value == "quickfix"
        ));
        assert!(matches!(
            code_action_display_fragment(
                "Extract function",
                CODE_ACTION_TITLE_LABEL_MAX_CHARS,
                "fallback"
            ),
            Cow::Borrowed(value) if value == "Extract function"
        ));
    }

    #[test]
    fn code_action_display_fragment_borrows_clean_unicode_fragments() {
        let value = "Extract méthode λ";

        match code_action_display_fragment(value, CODE_ACTION_TITLE_LABEL_MAX_CHARS, "fallback") {
            Cow::Borrowed(label) => assert_eq!(label, value),
            Cow::Owned(label) => panic!("expected borrowed label, got {label:?}"),
        }
    }

    #[test]
    fn code_action_display_fragment_owns_dirty_truncated_and_fallback_fragments() {
        let cases = [
            (
                " Extract function ",
                CODE_ACTION_TITLE_LABEL_MAX_CHARS,
                "fallback",
            ),
            ("alpha\nbeta", CODE_ACTION_TITLE_LABEL_MAX_CHARS, "fallback"),
            (
                "\u{200b}alpha",
                CODE_ACTION_TITLE_LABEL_MAX_CHARS,
                "fallback",
            ),
            (
                "abcdefghijklmnopqrstuvwxyz",
                CODE_ACTION_KIND_LABEL_MAX_CHARS.min(12),
                "fallback",
            ),
            ("\n\u{202e}", CODE_ACTION_TITLE_LABEL_MAX_CHARS, "fallback"),
        ];

        for (value, max_chars, fallback) in cases {
            let label = code_action_display_fragment(value, max_chars, fallback);

            assert_eq!(
                label.as_ref(),
                sanitized_display_label(value, max_chars, fallback)
            );
            assert!(
                matches!(label, Cow::Owned(_)),
                "expected owned label for {value:?}"
            );
        }
    }

    #[test]
    fn code_action_display_fragment_samples_huge_inputs_as_owned_safe_fragments() {
        let value = format!(
            "Fix\n{}{}\u{2066}tail",
            "very-long-title-".repeat(CODE_ACTION_FRAGMENT_SAMPLE_BYTES / 4),
            "λ".repeat(128)
        );

        let label =
            code_action_display_fragment(&value, CODE_ACTION_TITLE_LABEL_MAX_CHARS, "fallback");
        let expected = sanitized_display_label(
            &sample_code_action_display_fragment(&value, CODE_ACTION_TITLE_LABEL_MAX_CHARS),
            CODE_ACTION_TITLE_LABEL_MAX_CHARS,
            "fallback",
        );

        assert_eq!(label.as_ref(), expected);
        assert!(matches!(label, Cow::Owned(_)));
        assert!(!label.contains('\n'), "{label:?}");
        assert!(!label.contains('\u{2066}'), "{label:?}");
        assert!(label.chars().count() <= CODE_ACTION_TITLE_LABEL_MAX_CHARS);
    }

    #[test]
    fn code_action_display_label_samples_huge_unsafe_server_text() {
        let title = format!("Fix\n{}{}tail", "very-long-title-".repeat(512), "\u{202e}");
        let kind = format!("quickfix\t{}kind", "very-long-kind-".repeat(256));
        let label = code_action_display_label(&action(&title, Some(&kind)));

        assert!(!label.contains('\n'), "{label:?}");
        assert!(!label.contains('\t'), "{label:?}");
        assert!(!label.contains('\u{202e}'), "{label:?}");
        assert!(label.contains("..."), "{label:?}");
        assert!(
            label.chars().count()
                <= CODE_ACTION_KIND_LABEL_MAX_CHARS + 2 + CODE_ACTION_TITLE_LABEL_MAX_CHARS,
            "{label:?}"
        );
    }

    #[test]
    fn code_action_display_label_samples_huge_blank_server_text_to_fallbacks() {
        let title = " ".repeat(8192);
        let kind = "\n\u{202e}".repeat(4096);

        assert_eq!(
            code_action_display_label(&action(&title, Some(&kind))),
            "quickfix  code action"
        );
    }

    #[test]
    fn code_action_auto_import_detection_includes_add_all_missing_imports_titles() {
        let mut actions = vec![
            action("Extract function", Some("refactor.extract")),
            action("Add semicolon", Some("quickfix")),
            action("Add all missing imports", Some("quickfix")),
        ];

        assert!(is_auto_import_code_action(&actions[2]));

        sort_code_actions_for_display(&mut actions);
        assert_eq!(
            actions
                .iter()
                .map(|action| action.title.as_str())
                .collect::<Vec<_>>(),
            vec![
                "Add all missing imports",
                "Add semicolon",
                "Extract function"
            ]
        );
    }
}
