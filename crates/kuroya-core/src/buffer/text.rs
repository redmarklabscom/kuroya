use ropey::RopeSlice;
use std::{borrow::Cow, ops::Range};

pub(super) fn chars_match(left: char, right: char, case_sensitive: bool) -> bool {
    if case_sensitive {
        left == right
    } else {
        left.eq_ignore_ascii_case(&right)
    }
}

pub(super) fn rope_slice_text<'a>(slice: &'a RopeSlice<'a>) -> Cow<'a, str> {
    let mut chunks = slice.chunks();
    let Some(first) = chunks.next() else {
        return Cow::Borrowed("");
    };
    if chunks.next().is_none() {
        Cow::Borrowed(first)
    } else {
        Cow::Owned(slice.to_string())
    }
}

pub(super) fn push_rope_slice_text(text: &mut String, slice: RopeSlice<'_>) {
    for chunk in slice.chunks() {
        text.push_str(chunk);
    }
}

pub(super) fn rope_slice_to_string(slice: RopeSlice<'_>) -> String {
    let mut text = String::with_capacity(slice.len_bytes());
    push_rope_slice_text(&mut text, slice);
    text
}

pub(super) fn byte_range_to_char_range(text: &str, range: Range<usize>) -> Option<Range<usize>> {
    if range.start > range.end || range.end > text.len() {
        return None;
    }

    let mut start = None;
    let mut end = None;
    let mut char_idx = 0;
    for (byte_idx, _) in text.char_indices() {
        if byte_idx == range.start {
            start = Some(char_idx);
        }
        if byte_idx == range.end {
            end = Some(char_idx);
            break;
        }
        char_idx += 1;
    }
    if range.start == text.len() {
        start = Some(char_idx);
    }
    if range.end == text.len() {
        end = Some(char_idx);
    }
    Some(start?..end?)
}

pub(super) fn char_range_to_byte_range(text: &str, range: &Range<usize>) -> Option<Range<usize>> {
    if range.start > range.end {
        return None;
    }
    let mut start = None;
    let mut end = None;
    let mut char_idx = 0;
    let mut reached_end = false;
    for (byte_idx, _) in text.char_indices() {
        if char_idx == range.start {
            start = Some(byte_idx);
        }
        if char_idx == range.end {
            end = Some(byte_idx);
            reached_end = true;
            break;
        }
        char_idx += 1;
    }
    if !reached_end && range.start == char_idx {
        start = Some(text.len());
    }
    if !reached_end && range.end == char_idx {
        end = Some(text.len());
    }
    Some(start?..end?)
}

pub(super) fn normalize_char_range(range: &Range<usize>, len_chars: usize) -> Range<usize> {
    let start = range.start.min(len_chars);
    let end = range.end.min(len_chars).max(start);
    start..end
}
