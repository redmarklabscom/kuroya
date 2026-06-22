use crate::path_display::sanitized_display_label_cow;
use kuroya_core::text_match::ascii_case_insensitive_contains;
use std::borrow::Cow;

pub(in crate::keybindings_panel) const KEYBINDINGS_QUERY_MAX_CHARS: usize = 160;
pub(in crate::keybindings_panel) const KEYBINDING_INLINE_QUERY_TERMS: usize = 8;

pub(in crate::keybindings_panel) fn sanitize_keybindings_query(query: &mut String) -> bool {
    if query.is_empty() {
        return false;
    }

    let Cow::Owned(sanitized) = sanitize_keybindings_query_cow(query.as_str()) else {
        return false;
    };
    if sanitized == *query {
        return false;
    }
    *query = sanitized;
    true
}

pub(in crate::keybindings_panel) fn sanitize_keybindings_query_cow(query: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(query, KEYBINDINGS_QUERY_MAX_CHARS, "")
}

#[derive(Debug, PartialEq, Eq)]
pub(in crate::keybindings_panel) enum KeybindingQueryTerms<'a> {
    Inline {
        terms: [&'a str; KEYBINDING_INLINE_QUERY_TERMS],
        len: usize,
    },
    Heap(Vec<&'a str>),
}

impl<'a> KeybindingQueryTerms<'a> {
    pub(in crate::keybindings_panel) fn as_slice(&self) -> &[&'a str] {
        match self {
            Self::Inline { terms, len } => &terms[..*len],
            Self::Heap(terms) => terms,
        }
    }

    pub(in crate::keybindings_panel) fn is_empty(&self) -> bool {
        self.as_slice().is_empty()
    }
}

pub(in crate::keybindings_panel) fn keybinding_query_terms(
    query: &str,
) -> KeybindingQueryTerms<'_> {
    let mut terms = [""; KEYBINDING_INLINE_QUERY_TERMS];
    let mut len = 0usize;
    let mut split = query.split_whitespace();
    while let Some(term) = split.next() {
        if len < KEYBINDING_INLINE_QUERY_TERMS {
            terms[len] = term;
            len += 1;
        } else {
            let (remaining, _) = split.size_hint();
            let mut heap = Vec::with_capacity(KEYBINDING_INLINE_QUERY_TERMS + 1 + remaining);
            heap.extend_from_slice(&terms);
            heap.push(term);
            heap.extend(split);
            return KeybindingQueryTerms::Heap(heap);
        }
    }

    KeybindingQueryTerms::Inline { terms, len }
}

pub(in crate::keybindings_panel) fn keybinding_search_text_matches_terms(
    search_text: &str,
    terms: &[&str],
) -> bool {
    terms
        .iter()
        .all(|term| ascii_case_insensitive_contains(search_text, term))
}
