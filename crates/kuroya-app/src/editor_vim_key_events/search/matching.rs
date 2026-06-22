use crate::editor_vim_key_events::VIM_MAX_COUNT;
use kuroya_core::TextBuffer;
use std::{collections::VecDeque, ops::Range};

const VIM_SEARCH_HIGHLIGHT_MAX_MATCHES: usize = 5_000;

pub(super) fn vim_search_highlight_ranges_for_query(
    buffer: &TextBuffer,
    word: &str,
    whole_word: bool,
) -> Vec<Range<usize>> {
    let Some(needle) = vim_word_search_needle(word, buffer.len_chars()) else {
        return Vec::new();
    };
    let needle_len = needle.len();
    let mut ranges = Vec::new();
    let mut last_end = 0usize;
    vim_for_each_word_search_match(buffer, &needle, whole_word, |_, start| {
        if ranges.len() >= VIM_SEARCH_HIGHLIGHT_MAX_MATCHES || start < last_end {
            return;
        }
        let end = start.saturating_add(needle_len).min(buffer.len_chars());
        ranges.push(start..end);
        last_end = end;
    });
    ranges
}

pub(super) fn vim_search_match_range_for_query(
    buffer: &TextBuffer,
    word: &str,
    origin: usize,
    count: usize,
    forward: bool,
    whole_word: bool,
) -> Option<Range<usize>> {
    let needle = vim_word_search_needle(word, buffer.len_chars())?;
    let needle_len = needle.len();
    let mut matches = Vec::new();
    vim_for_each_word_search_match(buffer, &needle, whole_word, |_, start| {
        matches.push(start..start.saturating_add(needle_len).min(buffer.len_chars()));
    });

    let match_count = matches.len();
    if match_count == 0 {
        return None;
    }

    let count = count.clamp(1, VIM_MAX_COUNT);
    let current = matches
        .iter()
        .position(|range| range.start <= origin && origin < range.end);
    let target_index = if forward {
        let first = current
            .or_else(|| matches.iter().position(|range| range.start > origin))
            .unwrap_or(0);
        (first + count - 1) % match_count
    } else {
        let first = current
            .or_else(|| matches.iter().rposition(|range| range.start < origin))
            .unwrap_or(match_count - 1);
        (first + match_count - ((count - 1) % match_count)) % match_count
    };

    matches.get(target_index).cloned()
}

pub(in crate::editor_vim_key_events) fn vim_search_word_target(
    buffer: &TextBuffer,
    word: &str,
    origin: usize,
    count: usize,
    forward: bool,
    whole_word: bool,
) -> Option<usize> {
    let needle = vim_word_search_needle(word, buffer.len_chars())?;
    let count = count.clamp(1, VIM_MAX_COUNT);
    let mut total_matches = 0usize;
    let mut first_matches = Vec::with_capacity(count);
    let mut last_matches = VecDeque::with_capacity(count);
    let mut first_after_index = None;
    let mut after_count = 0usize;
    let mut forward_target_without_wrap = None;
    let mut last_before_index = None;
    let mut before_count = 0usize;
    let mut before_matches = VecDeque::with_capacity(count);

    vim_for_each_word_search_match(buffer, &needle, whole_word, |match_index, start| {
        total_matches = match_index.saturating_add(1);
        if first_matches.len() < count {
            first_matches.push(start);
        }
        if last_matches.len() == count {
            last_matches.pop_front();
        }
        last_matches.push_back(start);

        if start > origin {
            first_after_index.get_or_insert(match_index);
            after_count = after_count.saturating_add(1);
            if after_count == count {
                forward_target_without_wrap = Some(start);
            }
        }
        if start < origin {
            last_before_index = Some(match_index);
            before_count = before_count.saturating_add(1);
            if before_matches.len() == count {
                before_matches.pop_front();
            }
            before_matches.push_back(start);
        }
    });

    if total_matches == 0 {
        return None;
    }

    if forward {
        if let Some(target) = forward_target_without_wrap {
            return Some(target);
        }
        let first_after = first_after_index.unwrap_or(0);
        let target_index = (first_after + count - 1) % total_matches;
        return first_matches.get(target_index).copied();
    }

    if before_count >= count {
        return before_matches.get(before_matches.len() - count).copied();
    }
    let last_before = last_before_index.unwrap_or(total_matches - 1);
    let target_index =
        (last_before + total_matches - ((count - 1) % total_matches)) % total_matches;
    if let Some(target) = first_matches.get(target_index) {
        return Some(*target);
    }
    let first_last_index = total_matches.saturating_sub(last_matches.len());
    last_matches
        .get(target_index.checked_sub(first_last_index)?)
        .copied()
}

fn vim_word_search_needle(word: &str, buffer_len: usize) -> Option<Vec<char>> {
    let needle = word.chars().collect::<Vec<_>>();
    (!needle.is_empty() && needle.len() <= buffer_len).then_some(needle)
}

fn vim_for_each_word_search_match(
    buffer: &TextBuffer,
    needle: &[char],
    whole_word: bool,
    mut visit: impl FnMut(usize, usize),
) {
    let needle_len = needle.len();
    let Some(&first_needle_char) = needle.first() else {
        return;
    };
    let buffer_len = buffer.len_chars();
    if needle_len > buffer_len {
        return;
    }

    let mut match_index = 0usize;
    for start in 0..=buffer_len - needle_len {
        if buffer.char_at(start) != Some(first_needle_char) {
            continue;
        }
        let end = start + needle_len;
        if whole_word && !vim_is_whole_word_search_match(buffer, start, end, buffer_len) {
            continue;
        }
        if needle
            .iter()
            .enumerate()
            .skip(1)
            .all(|(offset, ch)| buffer.char_at(start + offset) == Some(*ch))
        {
            visit(match_index, start);
            match_index = match_index.saturating_add(1);
        }
    }
}

fn vim_is_whole_word_search_match(
    buffer: &TextBuffer,
    start: usize,
    end: usize,
    buffer_len: usize,
) -> bool {
    let before = start.checked_sub(1).and_then(|idx| buffer.char_at(idx));
    let after = (end < buffer_len).then(|| buffer.char_at(end)).flatten();
    !before.is_some_and(|ch| vim_is_buffer_word_char(buffer, ch))
        && !after.is_some_and(|ch| vim_is_buffer_word_char(buffer, ch))
}

pub(super) fn vim_is_buffer_word_char(buffer: &TextBuffer, ch: char) -> bool {
    !ch.is_whitespace() && !buffer.word_separators().contains(ch)
}
