use std::borrow::Cow;

pub(super) const TERMINAL_SEARCH_QUERY_MAX_CHARS: usize = 256;
pub(super) const TERMINAL_SEARCH_QUERY_SCAN_MAX_CHARS: usize = 4096;

pub(super) fn normalize_terminal_search_query(query: &str) -> Option<Cow<'_, str>> {
    let mut chars = 0usize;
    let mut start_byte = None;
    let mut last_non_space_end = 0usize;
    let mut normalized = None::<String>;
    let mut requires_owned = false;
    let mut pending_space = false;
    let mut suppress_ascii_spaces = false;
    let mut hit_normalization_bound = false;

    for (scanned, (byte, ch)) in query.char_indices().enumerate() {
        if chars >= TERMINAL_SEARCH_QUERY_MAX_CHARS
            || scanned >= TERMINAL_SEARCH_QUERY_SCAN_MAX_CHARS
        {
            hit_normalization_bound = true;
            break;
        }

        if is_terminal_search_bidi_format_control(ch) {
            terminal_search_query_require_owned(
                &mut normalized,
                &mut requires_owned,
                query,
                start_byte,
                byte,
            );
            continue;
        }

        if is_terminal_search_unsafe_query_space(ch) {
            terminal_search_query_require_owned(
                &mut normalized,
                &mut requires_owned,
                query,
                start_byte,
                byte,
            );
            if chars > 0 {
                pending_space = true;
            }
            suppress_ascii_spaces = true;
            continue;
        }

        if ch == ' ' {
            if chars == 0 {
                continue;
            }
            if let Some(normalized) = normalized.as_mut() {
                if pending_space || suppress_ascii_spaces {
                    pending_space = true;
                    continue;
                }
                normalized.push(' ');
            }
            start_byte.get_or_insert(byte);
            chars += 1;
            continue;
        }

        if let Some(normalized) = normalized.as_mut() {
            if pending_space && !normalized.ends_with(' ') {
                if chars + 1 >= TERMINAL_SEARCH_QUERY_MAX_CHARS {
                    hit_normalization_bound = true;
                    break;
                }
                normalized.push(' ');
                chars += 1;
            }
            normalized.push(ch);
        } else if requires_owned {
            normalized
                .get_or_insert_with(|| {
                    String::with_capacity(query.len().min(TERMINAL_SEARCH_QUERY_MAX_CHARS))
                })
                .push(ch);
        } else {
            start_byte.get_or_insert(byte);
            last_non_space_end = byte + ch.len_utf8();
        }
        pending_space = false;
        suppress_ascii_spaces = false;
        chars += 1;
    }

    if hit_normalization_bound {
        return None;
    }

    if let Some(mut normalized) = normalized {
        while normalized.ends_with(' ') {
            normalized.pop();
        }
        return (!normalized.is_empty()).then_some(Cow::Owned(normalized));
    }

    let start_byte = start_byte?;
    (last_non_space_end > start_byte)
        .then_some(Cow::Borrowed(&query[start_byte..last_non_space_end]))
}

fn terminal_search_query_require_owned(
    normalized: &mut Option<String>,
    requires_owned: &mut bool,
    query: &str,
    start_byte: Option<usize>,
    end_byte: usize,
) {
    if normalized.is_some() || start_byte.is_some() {
        terminal_search_query_ensure_owned(normalized, query, start_byte, end_byte);
    } else {
        *requires_owned = true;
    }
}

fn terminal_search_query_ensure_owned(
    normalized: &mut Option<String>,
    query: &str,
    start_byte: Option<usize>,
    end_byte: usize,
) {
    if normalized.is_some() {
        return;
    }

    let mut owned = String::with_capacity(query.len().min(TERMINAL_SEARCH_QUERY_MAX_CHARS));
    if let Some(start_byte) = start_byte {
        owned.push_str(&query[start_byte..end_byte]);
    }
    *normalized = Some(owned);
}

fn is_terminal_search_unsafe_query_space(ch: char) -> bool {
    ch.is_control() || (ch.is_whitespace() && ch != ' ')
}

fn is_terminal_search_bidi_format_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{061c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
    )
}
