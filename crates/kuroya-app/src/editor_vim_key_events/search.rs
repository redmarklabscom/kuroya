use super::{EditorVimPendingKey, VIM_MAX_COUNT};
use kuroya_core::{BufferId, TextBuffer};
use std::{cell::RefCell, collections::VecDeque, ops::Range};

const VIM_SEARCH_BUFFER_LIMIT: usize = 128;
const VIM_SEARCH_STATUS_QUERY_MAX_CHARS: usize = 96;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct EditorVimSearch {
    pub(super) word: String,
    pub(super) forward: bool,
    pub(super) whole_word: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct EditorVimBufferSearch {
    pub(super) buffer_id: BufferId,
    pub(super) search: EditorVimSearch,
}

thread_local! {
    pub(super) static VIM_SEARCH_INPUT: RefCell<String> = const { RefCell::new(String::new()) };
    pub(super) static VIM_SEARCHES: RefCell<Vec<EditorVimBufferSearch>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn vim_pending_search_status_label(
    pending: Option<EditorVimPendingKey>,
) -> Option<String> {
    let EditorVimPendingKey::SearchInput { forward, .. } = pending? else {
        return None;
    };

    Some(VIM_SEARCH_INPUT.with(|input| {
        let query = input.borrow();
        let mut label = String::with_capacity(
            1 + query
                .len()
                .min(VIM_SEARCH_STATUS_QUERY_MAX_CHARS * 4 + "...".len()),
        );
        label.push(if forward { '/' } else { '?' });
        push_vim_search_status_query(&mut label, &query);
        label
    }))
}

fn push_vim_search_status_query(label: &mut String, query: &str) {
    let mut chars = query.chars();
    for _ in 0..VIM_SEARCH_STATUS_QUERY_MAX_CHARS {
        let Some(ch) = chars.next() else {
            return;
        };
        label.push(ch);
    }
    if chars.next().is_some() {
        label.push_str("...");
    }
}

pub(super) fn vim_operator_search_repeat_range(
    buffer: &mut TextBuffer,
    count: usize,
    reverse: bool,
) -> Option<Range<usize>> {
    let start = buffer.cursor();
    if !vim_repeat_last_search(buffer, count, reverse) {
        return None;
    }

    let target = buffer.cursor();
    buffer.set_single_cursor(start);
    (start != target).then_some(start.min(target)..start.max(target))
}

pub(super) fn vim_operator_search_match_range(
    buffer: &TextBuffer,
    count: usize,
    reverse: bool,
) -> Option<Range<usize>> {
    vim_search_match_range(buffer, count, reverse)
}

pub(super) fn vim_search_match_range(
    buffer: &TextBuffer,
    count: usize,
    reverse: bool,
) -> Option<Range<usize>> {
    let buffer_id = buffer.id();
    let origin = buffer.cursor();
    VIM_SEARCHES.with(|searches| {
        let mut searches = searches.borrow_mut();
        let search = searches
            .iter_mut()
            .find(|entry| entry.buffer_id == buffer_id)
            .map(|entry| &mut entry.search)?;
        let forward = if reverse {
            !search.forward
        } else {
            search.forward
        };
        let range = vim_search_match_range_for_query(
            buffer,
            &search.word,
            origin,
            count,
            forward,
            search.whole_word,
        )?;
        search.forward = forward;
        Some(range)
    })
}

fn vim_search_match_range_for_query(
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

pub(super) fn vim_operator_search_word_under_cursor_range(
    buffer: &mut TextBuffer,
    count: usize,
    forward: bool,
    whole_word: bool,
) -> Option<Range<usize>> {
    let start = buffer.cursor();
    let word_range = vim_search_word_range_at_or_after_cursor(buffer)?;
    let word = buffer.text_range(word_range.clone())?;
    if word.is_empty() {
        return None;
    }

    vim_set_last_search(buffer, &word, forward, whole_word);
    let target =
        vim_search_word_target(buffer, &word, word_range.start, count, forward, whole_word)?;
    (start != target).then_some(start.min(target)..start.max(target))
}

pub(super) fn vim_search_word_under_cursor(
    buffer: &mut TextBuffer,
    count: usize,
    forward: bool,
    whole_word: bool,
) -> bool {
    let Some(word_range) = vim_search_word_range_at_or_after_cursor(buffer) else {
        return false;
    };
    let Some(word) = buffer.text_range(word_range.clone()) else {
        return false;
    };
    if word.is_empty() {
        return false;
    }

    vim_set_last_search(buffer, &word, forward, whole_word);
    vim_search_word(buffer, &word, word_range.start, count, forward, whole_word)
}

pub(super) fn vim_clear_search_input() {
    VIM_SEARCH_INPUT.with(|input| input.borrow_mut().clear());
}

pub(super) fn vim_push_search_input(ch: char) {
    VIM_SEARCH_INPUT.with(|input| input.borrow_mut().push(ch));
}

pub(super) fn vim_pop_search_input() {
    VIM_SEARCH_INPUT.with(|input| {
        input.borrow_mut().pop();
    });
}

pub(super) fn vim_delete_search_input_word_backward() {
    VIM_SEARCH_INPUT.with(|input| {
        let mut query = input.borrow_mut();
        while query
            .chars()
            .next_back()
            .is_some_and(|ch| ch.is_whitespace())
        {
            query.pop();
        }
        while query
            .chars()
            .next_back()
            .is_some_and(|ch| !ch.is_whitespace())
        {
            query.pop();
        }
    });
}

pub(super) fn vim_finish_pending_literal_search(
    buffer: &mut TextBuffer,
    count: usize,
    forward: bool,
) -> bool {
    VIM_SEARCH_INPUT.with(|input| {
        let mut query = input.borrow_mut();
        let moved = vim_literal_search(buffer, &query, count, forward);
        query.clear();
        moved
    })
}

fn vim_literal_search(buffer: &mut TextBuffer, query: &str, count: usize, forward: bool) -> bool {
    if query.is_empty() {
        return vim_repeat_last_search_in_direction(buffer, count, forward);
    }
    vim_set_last_search(buffer, query, forward, false);
    vim_search_word(buffer, query, buffer.cursor(), count, forward, false)
}

fn vim_search_word_range_at_or_after_cursor(buffer: &TextBuffer) -> Option<Range<usize>> {
    if buffer
        .char_at(buffer.cursor())
        .is_some_and(|ch| vim_is_buffer_word_char(buffer, ch))
    {
        if let Some(range) = buffer.word_range_at_cursor() {
            return Some(range);
        }
    }

    if let Some(range) = vim_search_word_range_before_cursor_at_line_end(buffer) {
        return Some(range);
    }

    let len = buffer.len_chars();
    let mut idx = buffer.cursor().saturating_add(1);
    while idx < len {
        let ch = buffer.char_at(idx)?;
        if vim_is_buffer_word_char(buffer, ch) {
            return vim_word_range_at(buffer, idx);
        }
        idx += 1;
    }
    None
}

fn vim_search_word_range_before_cursor_at_line_end(buffer: &TextBuffer) -> Option<Range<usize>> {
    let cursor = buffer.cursor();
    if cursor == 0 {
        return None;
    }

    let line = buffer.cursor_position().line;
    if cursor != buffer.line_content_end_char(line) {
        return None;
    }

    let previous = cursor - 1;
    if !buffer
        .char_at(previous)
        .is_some_and(|ch| vim_is_buffer_word_char(buffer, ch))
    {
        return None;
    }

    vim_word_range_at(buffer, previous)
}

fn vim_word_range_at(buffer: &TextBuffer, cursor: usize) -> Option<Range<usize>> {
    let ch = buffer.char_at(cursor)?;
    if !vim_is_buffer_word_char(buffer, ch) {
        return None;
    }

    let mut start = cursor;
    while start > 0
        && buffer
            .char_at(start - 1)
            .is_some_and(|ch| vim_is_buffer_word_char(buffer, ch))
    {
        start -= 1;
    }

    let len = buffer.len_chars();
    let mut end = cursor + 1;
    while end < len
        && buffer
            .char_at(end)
            .is_some_and(|ch| vim_is_buffer_word_char(buffer, ch))
    {
        end += 1;
    }

    Some(start..end)
}

pub(super) fn vim_repeat_last_search(buffer: &mut TextBuffer, count: usize, reverse: bool) -> bool {
    let buffer_id = buffer.id();
    let origin = buffer.cursor();
    let target = VIM_SEARCHES.with(|searches| {
        let searches = searches.borrow();
        let search = searches
            .iter()
            .find(|entry| entry.buffer_id == buffer_id)
            .map(|entry| &entry.search)?;
        let forward = if reverse {
            !search.forward
        } else {
            search.forward
        };
        vim_search_word_target(
            buffer,
            &search.word,
            origin,
            count,
            forward,
            search.whole_word,
        )
    });

    let Some(target) = target else {
        return false;
    };
    buffer.set_single_cursor(target);
    true
}

fn vim_repeat_last_search_in_direction(
    buffer: &mut TextBuffer,
    count: usize,
    forward: bool,
) -> bool {
    let buffer_id = buffer.id();
    let origin = buffer.cursor();
    let target = VIM_SEARCHES.with(|searches| {
        let mut searches = searches.borrow_mut();
        let search = searches
            .iter_mut()
            .find(|entry| entry.buffer_id == buffer_id)
            .map(|entry| &mut entry.search)?;
        let target = vim_search_word_target(
            buffer,
            &search.word,
            origin,
            count,
            forward,
            search.whole_word,
        )?;
        search.forward = forward;
        Some(target)
    });

    let Some(target) = target else {
        return false;
    };
    buffer.set_single_cursor(target);
    true
}

pub(super) fn vim_set_last_search(
    buffer: &TextBuffer,
    word: &str,
    forward: bool,
    whole_word: bool,
) {
    let buffer_id = buffer.id();
    VIM_SEARCHES.with(|searches| {
        let mut searches = searches.borrow_mut();
        if let Some(existing) = searches
            .iter_mut()
            .find(|entry| entry.buffer_id == buffer_id)
        {
            if existing.search.word == word
                && existing.search.forward == forward
                && existing.search.whole_word == whole_word
            {
                return;
            }
            existing.search.word.clear();
            existing.search.word.push_str(word);
            existing.search.forward = forward;
            existing.search.whole_word = whole_word;
            return;
        }

        if searches.len() >= VIM_SEARCH_BUFFER_LIMIT {
            searches.remove(0);
        }
        let search = EditorVimSearch {
            word: word.to_owned(),
            forward,
            whole_word,
        };
        searches.push(EditorVimBufferSearch { buffer_id, search });
    });
}

fn vim_search_word(
    buffer: &mut TextBuffer,
    word: &str,
    origin: usize,
    count: usize,
    forward: bool,
    whole_word: bool,
) -> bool {
    let Some(target) = vim_search_word_target(buffer, word, origin, count, forward, whole_word)
    else {
        return false;
    };
    buffer.set_single_cursor(target);
    true
}

pub(super) fn vim_search_word_target(
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

fn vim_is_buffer_word_char(buffer: &TextBuffer, ch: char) -> bool {
    !ch.is_whitespace() && !buffer.word_separators().contains(ch)
}
