use std::{borrow::Cow, collections::VecDeque};

pub(super) fn vim_suppress_printable_key_text_if(
    printable_key_char: Option<char>,
    suppressed_text: &mut VecDeque<char>,
    enabled: bool,
) {
    if enabled {
        vim_suppress_printable_key_text(printable_key_char, suppressed_text);
    }
}

fn vim_suppress_printable_key_text(
    printable_key_char: Option<char>,
    suppressed_text: &mut VecDeque<char>,
) {
    if let Some(ch) = printable_key_char {
        suppressed_text.push_back(ch);
    }
}

pub(crate) fn vim_text_after_suppression<'a>(
    text: &'a str,
    suppressed_text: &mut VecDeque<char>,
) -> Option<Cow<'a, str>> {
    let Some(expected) = suppressed_text.pop_front() else {
        return (!text.is_empty()).then_some(Cow::Borrowed(text));
    };
    let mut chars = text.char_indices();
    let (_, first) = chars.next()?;
    if first != expected {
        suppressed_text.push_front(expected);
        return Some(Cow::Borrowed(text));
    }
    match chars.next() {
        Some((byte_idx, _)) => Some(Cow::Borrowed(&text[byte_idx..])),
        None => None,
    }
}
