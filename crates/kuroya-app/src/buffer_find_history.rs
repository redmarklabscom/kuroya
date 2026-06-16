use kuroya_core::EditorFindHistory;
use std::borrow::Cow;
use std::collections::VecDeque;

pub(crate) const MAX_BUFFER_FIND_HISTORY: usize = 50;
const MAX_BUFFER_FIND_HISTORY_VALUE_CHARS: usize = 1_024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BufferFindHistoryDirection {
    Older,
    Newer,
}

pub(crate) fn buffer_find_history_enabled(setting: EditorFindHistory) -> bool {
    matches!(setting, EditorFindHistory::Workspace)
}

pub(crate) fn record_buffer_find_query_history(
    history: &mut VecDeque<String>,
    query: &str,
    limit: usize,
) -> bool {
    record_normalized_history_value(history, query, limit, true)
}

pub(crate) fn record_buffer_find_replacement_history(
    history: &mut VecDeque<String>,
    replacement: &str,
    limit: usize,
) -> bool {
    record_normalized_history_value(history, replacement, limit, false)
}

pub(crate) fn normalize_buffer_find_query_history(
    values: impl IntoIterator<Item = String>,
    limit: usize,
) -> VecDeque<String> {
    normalize_buffer_find_history(values, limit, true)
}

pub(crate) fn normalize_buffer_find_replacement_history(
    values: impl IntoIterator<Item = String>,
    limit: usize,
) -> VecDeque<String> {
    normalize_buffer_find_history(values, limit, false)
}

fn normalize_buffer_find_history(
    values: impl IntoIterator<Item = String>,
    limit: usize,
    trim_edges: bool,
) -> VecDeque<String> {
    let limit = effective_buffer_find_history_limit(limit);
    if limit == 0 {
        return VecDeque::new();
    }
    let mut history = VecDeque::new();
    for value in values {
        let Some(value) = normalize_buffer_find_history_value(&value, trim_edges) else {
            continue;
        };
        if history.iter().any(|existing| existing == &value) {
            continue;
        }
        history.push_back(value);
        if history.len() >= limit {
            break;
        }
    }
    history
}

pub(crate) fn apply_buffer_find_history_navigation(
    value: &mut String,
    history: &VecDeque<String>,
    cursor: &mut Option<usize>,
    draft: &mut Option<String>,
    direction: BufferFindHistoryDirection,
) -> bool {
    if history.is_empty() {
        *cursor = None;
        *draft = None;
        return false;
    }

    if cursor.is_some_and(|index| index >= history.len()) {
        *cursor = None;
        *draft = None;
    }

    let next = match direction {
        BufferFindHistoryDirection::Older => match *cursor {
            Some(index) => Some(index.saturating_add(1).min(history.len() - 1)),
            None => {
                *draft = Some(value.clone());
                Some(0)
            }
        },
        BufferFindHistoryDirection::Newer => match *cursor {
            Some(0) => {
                *cursor = None;
                if let Some(restored) = draft.take() {
                    *value = restored;
                    return true;
                }
                return false;
            }
            Some(index) => Some(index - 1),
            None => None,
        },
    };

    let Some(next) = next else {
        return false;
    };
    let Some(history_value) = history.get(next) else {
        *cursor = None;
        return false;
    };

    *cursor = Some(next);
    if value == history_value {
        return false;
    }
    *value = history_value.clone();
    true
}

fn record_normalized_history_value(
    history: &mut VecDeque<String>,
    value: &str,
    limit: usize,
    trim_edges: bool,
) -> bool {
    let limit = effective_buffer_find_history_limit(limit);
    if limit == 0 {
        return false;
    }
    let Some(value) = normalize_buffer_find_history_value_borrowed(value, trim_edges) else {
        return false;
    };
    if history
        .front()
        .is_some_and(|item| item.as_str() == value.as_ref())
    {
        history.truncate(limit);
        return true;
    }
    if let Some(index) = history
        .iter()
        .position(|item| item.as_str() == value.as_ref())
    {
        let Some(value) = history.remove(index) else {
            return false;
        };
        history.push_front(value);
        history.truncate(limit);
        return true;
    }
    record_exact_history_value(history, value.into_owned(), limit)
}

fn record_exact_history_value(history: &mut VecDeque<String>, value: String, limit: usize) -> bool {
    let limit = effective_buffer_find_history_limit(limit);
    if limit == 0 {
        return false;
    }

    if history.front().is_some_and(|item| item == &value) {
        history.truncate(limit);
        return true;
    }

    history.retain(|item| item != &value);
    history.push_front(value);
    history.truncate(limit);
    true
}

fn normalize_buffer_find_history_value(value: &str, trim_edges: bool) -> Option<String> {
    normalize_buffer_find_history_value_borrowed(value, trim_edges).map(Cow::into_owned)
}

fn normalize_buffer_find_history_value_borrowed(
    value: &str,
    trim_edges: bool,
) -> Option<Cow<'_, str>> {
    let value = if trim_edges { value.trim() } else { value };
    if value.is_empty() {
        return None;
    }

    if value.len() <= MAX_BUFFER_FIND_HISTORY_VALUE_CHARS
        && value.is_ascii()
        && !value.as_bytes().iter().any(u8::is_ascii_control)
    {
        return Some(Cow::Borrowed(value));
    }

    let mut needs_normalization = false;
    for (char_count, ch) in value.chars().enumerate() {
        if char_count >= MAX_BUFFER_FIND_HISTORY_VALUE_CHARS
            || buffer_find_history_char_is_control(ch)
        {
            needs_normalization = true;
            break;
        }
    }
    if !needs_normalization {
        return Some(Cow::Borrowed(value));
    }

    let mut normalized = String::new();
    let mut chars = 0;
    let mut in_control_run = false;
    for ch in value.chars() {
        if chars >= MAX_BUFFER_FIND_HISTORY_VALUE_CHARS {
            break;
        }
        if buffer_find_history_char_is_control(ch) {
            if !in_control_run {
                normalized.push(' ');
                chars += 1;
                in_control_run = true;
            }
            continue;
        }
        normalized.push(ch);
        chars += 1;
        in_control_run = false;
    }

    if trim_edges {
        let trimmed = normalized.trim();
        if trimmed.is_empty() {
            None
        } else if trimmed.len() == normalized.len() {
            Some(Cow::Owned(normalized))
        } else {
            Some(Cow::Owned(trimmed.to_owned()))
        }
    } else if normalized.is_empty() {
        None
    } else {
        Some(Cow::Owned(normalized))
    }
}

fn effective_buffer_find_history_limit(limit: usize) -> usize {
    limit.min(MAX_BUFFER_FIND_HISTORY)
}

fn buffer_find_history_char_is_control(ch: char) -> bool {
    ch.is_control()
        || matches!(ch, '\u{2028}' | '\u{2029}')
        || buffer_find_history_hidden_format_control(ch)
}

fn buffer_find_history_hidden_format_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{061c}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
            | '\u{feff}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_history_records_trimmed_deduped_values() {
        let mut history = VecDeque::new();

        assert!(record_buffer_find_query_history(&mut history, " first ", 3));
        assert!(record_buffer_find_query_history(&mut history, "second", 3));
        assert!(record_buffer_find_query_history(&mut history, "first", 3));
        assert!(!record_buffer_find_query_history(&mut history, "   ", 3));

        assert_eq!(
            history,
            VecDeque::from(["first".to_owned(), "second".to_owned()])
        );
    }

    #[test]
    fn query_history_sanitizes_single_line_bounded_values() {
        let mut history = VecDeque::new();
        let oversized = format!(
            " {}{} ",
            "a".repeat(MAX_BUFFER_FIND_HISTORY_VALUE_CHARS + 8),
            "\n\tignored"
        );

        assert!(record_buffer_find_query_history(
            &mut history,
            " alpha\n\tbeta ",
            4
        ));
        assert!(record_buffer_find_query_history(
            &mut history,
            &oversized,
            4
        ));
        assert!(!record_buffer_find_query_history(&mut history, "\n\r\t", 4));

        assert_eq!(history[1], "alpha beta");
        assert_eq!(
            history[0].chars().count(),
            MAX_BUFFER_FIND_HISTORY_VALUE_CHARS
        );
        assert!(history[0].chars().all(|ch| !ch.is_control()));
    }

    #[test]
    fn query_history_sanitizes_hidden_format_controls() {
        let mut history = VecDeque::new();

        assert!(record_buffer_find_query_history(
            &mut history,
            " alpha\u{202e}\u{2066}\u{2029}beta ",
            4
        ));

        assert_eq!(history, VecDeque::from(["alpha beta".to_owned()]));
    }

    #[test]
    fn replacement_history_preserves_exact_non_empty_values() {
        let mut history = VecDeque::new();

        record_buffer_find_replacement_history(&mut history, " spaced ", 3);
        record_buffer_find_replacement_history(&mut history, "value", 3);
        record_buffer_find_replacement_history(&mut history, " spaced ", 3);
        record_buffer_find_replacement_history(&mut history, "", 3);

        assert_eq!(
            history,
            VecDeque::from([" spaced ".to_owned(), "value".to_owned()])
        );
    }

    #[test]
    fn replacement_history_preserves_spaces_but_sanitizes_controls_and_caps_length() {
        let mut history = VecDeque::new();
        let oversized = format!(
            "{}\n{}",
            "x".repeat(MAX_BUFFER_FIND_HISTORY_VALUE_CHARS),
            "tail"
        );

        record_buffer_find_replacement_history(&mut history, " keep  spaces ", 4);
        record_buffer_find_replacement_history(&mut history, "line\r\n\tbreak", 4);
        record_buffer_find_replacement_history(&mut history, &oversized, 4);
        record_buffer_find_replacement_history(&mut history, "", 4);

        assert_eq!(history[2], " keep  spaces ");
        assert_eq!(history[1], "line break");
        assert_eq!(
            history[0].chars().count(),
            MAX_BUFFER_FIND_HISTORY_VALUE_CHARS
        );
        assert!(history[0].chars().all(|ch| !ch.is_control()));
    }

    #[test]
    fn replacement_history_sanitizes_hidden_format_controls() {
        let mut history = VecDeque::new();

        record_buffer_find_replacement_history(
            &mut history,
            "keep\u{202e}\u{2066}\u{2028}value",
            4,
        );

        assert_eq!(history, VecDeque::from(["keep value".to_owned()]));
    }

    #[test]
    fn buffer_find_history_fast_path_borrows_clean_values_and_owns_sanitized_values() {
        let clean_query =
            normalize_buffer_find_history_value_borrowed(" clean query ", true).unwrap();
        assert!(matches!(clean_query, Cow::Borrowed("clean query")));

        let clean_replacement =
            normalize_buffer_find_history_value_borrowed(" keep  spaces ", false).unwrap();
        assert!(matches!(clean_replacement, Cow::Borrowed(" keep  spaces ")));

        let sanitized = normalize_buffer_find_history_value_borrowed("line\r\n\tbreak", true)
            .expect("sanitized history value");
        match sanitized {
            Cow::Owned(value) => assert_eq!(value, "line break"),
            Cow::Borrowed(value) => panic!("expected owned sanitized value, got {value:?}"),
        }
    }

    #[test]
    fn history_records_are_bounded() {
        let mut history = VecDeque::new();
        record_buffer_find_query_history(&mut history, "one", 2);
        record_buffer_find_query_history(&mut history, "two", 2);
        record_buffer_find_query_history(&mut history, "three", 2);

        assert_eq!(
            history,
            VecDeque::from(["three".to_owned(), "two".to_owned()])
        );
    }

    #[test]
    fn replacement_history_clamps_oversized_record_limit() {
        let mut history = VecDeque::new();

        for index in 0..MAX_BUFFER_FIND_HISTORY + 8 {
            record_buffer_find_replacement_history(
                &mut history,
                &format!("value-{index}"),
                usize::MAX,
            );
        }

        assert_eq!(history.len(), MAX_BUFFER_FIND_HISTORY);
        assert_eq!(history.front().map(String::as_str), Some("value-57"));
        assert_eq!(history.back().map(String::as_str), Some("value-8"));
    }

    #[test]
    fn history_navigation_steps_older_newer_and_restores_draft() {
        let history = VecDeque::from(["three".to_owned(), "two".to_owned(), "one".to_owned()]);
        let mut value = "draft".to_owned();
        let mut cursor = None;
        let mut draft = None;

        assert!(apply_buffer_find_history_navigation(
            &mut value,
            &history,
            &mut cursor,
            &mut draft,
            BufferFindHistoryDirection::Older
        ));
        assert_eq!(value, "three");
        assert_eq!(cursor, Some(0));
        assert_eq!(draft.as_deref(), Some("draft"));

        assert!(apply_buffer_find_history_navigation(
            &mut value,
            &history,
            &mut cursor,
            &mut draft,
            BufferFindHistoryDirection::Older
        ));
        assert_eq!(value, "two");
        assert_eq!(cursor, Some(1));

        assert!(apply_buffer_find_history_navigation(
            &mut value,
            &history,
            &mut cursor,
            &mut draft,
            BufferFindHistoryDirection::Newer
        ));
        assert_eq!(value, "three");
        assert_eq!(cursor, Some(0));

        assert!(apply_buffer_find_history_navigation(
            &mut value,
            &history,
            &mut cursor,
            &mut draft,
            BufferFindHistoryDirection::Newer
        ));
        assert_eq!(value, "draft");
        assert_eq!(cursor, None);
        assert_eq!(draft, None);
    }

    #[test]
    fn history_navigation_clears_stale_state_when_history_is_empty() {
        let history = VecDeque::new();
        let mut value = "draft".to_owned();
        let mut cursor = Some(8);
        let mut draft = Some("stale".to_owned());

        assert!(!apply_buffer_find_history_navigation(
            &mut value,
            &history,
            &mut cursor,
            &mut draft,
            BufferFindHistoryDirection::Older
        ));
        assert_eq!(value, "draft");
        assert_eq!(cursor, None);
        assert_eq!(draft, None);
    }

    #[test]
    fn history_navigation_recovers_when_cursor_outlives_history() {
        let history = VecDeque::from(["new".to_owned(), "old".to_owned()]);
        let mut value = "current".to_owned();
        let mut cursor = Some(99);
        let mut draft = Some("stale".to_owned());

        assert!(apply_buffer_find_history_navigation(
            &mut value,
            &history,
            &mut cursor,
            &mut draft,
            BufferFindHistoryDirection::Older
        ));
        assert_eq!(value, "new");
        assert_eq!(cursor, Some(0));
        assert_eq!(draft.as_deref(), Some("current"));
    }

    #[test]
    fn history_normalization_filters_old_session_values() {
        let history = normalize_buffer_find_query_history(
            [
                " alpha ".to_owned(),
                "".to_owned(),
                "beta".to_owned(),
                "line\n\tbreak".to_owned(),
            ]
            .to_vec(),
            8,
        );

        assert_eq!(
            history,
            VecDeque::from([
                "alpha".to_owned(),
                "beta".to_owned(),
                "line break".to_owned()
            ])
        );
    }

    #[test]
    fn replacement_history_normalization_filters_old_session_values() {
        let history = normalize_buffer_find_replacement_history(
            [
                " value ".to_owned(),
                "\n".to_owned(),
                "line\r\nbreak".to_owned(),
                " value ".to_owned(),
            ],
            8,
        );

        assert_eq!(
            history,
            VecDeque::from([
                " value ".to_owned(),
                " ".to_owned(),
                "line break".to_owned()
            ])
        );
    }
}
