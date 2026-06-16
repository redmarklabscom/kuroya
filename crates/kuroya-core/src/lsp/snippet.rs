use std::{collections::BTreeMap, ops::Range};

use super::{
    MAX_SNIPPET_EXPANSION_BYTES, MAX_SNIPPET_NESTING, MAX_SNIPPET_SOURCE_BYTES,
    MAX_SNIPPET_TABSTOPS,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SnippetExpansion {
    pub(super) text: String,
    pub(super) selection: Option<Range<usize>>,
    pub(super) tabstops: Vec<Range<usize>>,
    pub(super) tabstop_groups: Vec<Vec<Range<usize>>>,
}

#[derive(Default)]
struct SnippetExpansionState {
    text: String,
    char_len: usize,
    selection: Option<Range<usize>>,
    selected_tabstop: Option<usize>,
    tabstops: Vec<(usize, Range<usize>)>,
    tabstop_values: BTreeMap<usize, String>,
    final_cursor: Option<usize>,
    overflowed: bool,
}

impl SnippetExpansionState {
    fn push_char(&mut self, ch: char) {
        if self.overflowed {
            return;
        }
        if self.text.len().saturating_add(ch.len_utf8()) > MAX_SNIPPET_EXPANSION_BYTES {
            self.overflowed = true;
            return;
        }
        self.text.push(ch);
        self.char_len = self.char_len.saturating_add(1);
    }

    fn push_text(&mut self, text: &str) -> Range<usize> {
        let start = self.char_len;
        if self.overflowed {
            return start..start;
        }
        if self.text.len().saturating_add(text.len()) > MAX_SNIPPET_EXPANSION_BYTES {
            self.overflowed = true;
            return start..start;
        }
        self.text.push_str(text);
        self.char_len = self.char_len.saturating_add(text.chars().count());
        start..self.char_len
    }

    fn text_for_range(&self, range: Range<usize>) -> String {
        self.text
            .chars()
            .skip(range.start)
            .take(range.end.saturating_sub(range.start))
            .collect()
    }

    fn emit_tabstop_reference(&mut self, tabstop: usize) {
        if tabstop == 0 {
            let offset = self.char_len;
            self.record_tabstop(tabstop, offset..offset);
            return;
        }
        if let Some(value) = self.tabstop_values.get(&tabstop).cloned() {
            let range = self.push_text(&value);
            self.record_tabstop(tabstop, range);
        } else {
            let offset = self.char_len;
            self.record_tabstop(tabstop, offset..offset);
        }
    }

    fn record_placeholder_tabstop(&mut self, tabstop: usize, range: Range<usize>) {
        if tabstop != 0 && !self.tabstop_values.contains_key(&tabstop) {
            let value = self.text_for_range(range.clone());
            self.tabstop_values.insert(tabstop, value);
        }
        self.record_tabstop(tabstop, range);
    }

    fn record_tabstop(&mut self, tabstop: usize, range: Range<usize>) {
        if tabstop == 0 {
            self.final_cursor.get_or_insert(range.start);
            return;
        }
        if self.tabstops.len() >= MAX_SNIPPET_TABSTOPS {
            self.overflowed = true;
            return;
        }
        if self
            .selected_tabstop
            .is_none_or(|selected| tabstop < selected)
        {
            self.selected_tabstop = Some(tabstop);
            self.selection = Some(range.clone());
        }
        self.tabstops.push((tabstop, range));
    }
}

pub(super) fn expand_lsp_completion_snippet(snippet: &str) -> Option<SnippetExpansion> {
    if snippet.len() > MAX_SNIPPET_SOURCE_BYTES {
        return None;
    }

    let chars = snippet.chars().collect::<Vec<_>>();
    let mut state = SnippetExpansionState::default();
    let _ = expand_snippet_segment(&chars, 0, None, 0, &mut state);
    if state.overflowed {
        return None;
    }
    if state.selection.is_none()
        && let Some(cursor) = state.final_cursor
    {
        state.selection = Some(cursor..cursor);
    }
    let tabstop_groups = snippet_tabstop_groups(state.tabstops, state.final_cursor);
    let tabstops = snippet_tabstops(&tabstop_groups);
    Some(SnippetExpansion {
        text: state.text,
        selection: state.selection,
        tabstops,
        tabstop_groups,
    })
}

fn expand_snippet_segment(
    chars: &[char],
    mut idx: usize,
    terminator: Option<char>,
    depth: usize,
    state: &mut SnippetExpansionState,
) -> usize {
    while idx < chars.len() {
        if state.overflowed {
            return idx;
        }
        if terminator.is_some_and(|term| chars[idx] == term) {
            return idx + 1;
        }

        match chars[idx] {
            '\\' => {
                if let Some(next) = chars.get(idx + 1).copied() {
                    if is_snippet_escape(next, false) {
                        state.push_char(next);
                        idx += 2;
                    } else {
                        state.push_char('\\');
                        idx += 1;
                    }
                } else {
                    state.push_char('\\');
                    idx += 1;
                }
            }
            '$' if depth <= MAX_SNIPPET_NESTING => {
                if let Some(next_idx) = parse_snippet_dollar(chars, idx, depth, state) {
                    idx = next_idx;
                } else {
                    state.push_char('$');
                    idx += 1;
                }
            }
            ch => {
                state.push_char(ch);
                idx += 1;
            }
        }
    }
    idx
}

fn parse_snippet_dollar(
    chars: &[char],
    idx: usize,
    depth: usize,
    state: &mut SnippetExpansionState,
) -> Option<usize> {
    if chars.get(idx).copied()? != '$' {
        return None;
    }

    let next = chars.get(idx + 1).copied()?;
    if next.is_ascii_digit() {
        let end = consume_ascii_digits(chars, idx + 1);
        let tabstop = snippet_tabstop(chars, idx + 1..end)?;
        state.emit_tabstop_reference(tabstop);
        return Some(end);
    }
    if next == '{' {
        return parse_snippet_braced(chars, idx + 2, depth + 1, state);
    }
    if is_snippet_variable_start(next) {
        return Some(consume_snippet_variable(chars, idx + 1));
    }
    None
}

fn parse_snippet_braced(
    chars: &[char],
    start: usize,
    depth: usize,
    state: &mut SnippetExpansionState,
) -> Option<usize> {
    if depth > MAX_SNIPPET_NESTING {
        return None;
    }

    let mut idx = start;
    while idx < chars.len() && !matches!(chars[idx], ':' | '|' | '}') {
        idx += 1;
    }

    let tabstop = snippet_tabstop(chars, start..idx);
    match chars.get(idx).copied()? {
        '}' => {
            if let Some(tabstop) = tabstop {
                state.emit_tabstop_reference(tabstop);
            }
            Some(idx + 1)
        }
        ':' => {
            if let Some(tabstop) = tabstop
                && let Some(value) = state.tabstop_values.get(&tabstop).cloned()
            {
                let range = state.push_text(&value);
                state.record_tabstop(tabstop, range);
                let mut scratch = SnippetExpansionState::default();
                let next_idx =
                    expand_snippet_segment(chars, idx + 1, Some('}'), depth + 1, &mut scratch);
                if scratch.overflowed {
                    state.overflowed = true;
                }
                return Some(next_idx);
            }
            let start_offset = state.char_len;
            let next_idx = expand_snippet_segment(chars, idx + 1, Some('}'), depth + 1, state);
            if let Some(tabstop) = tabstop {
                state.record_placeholder_tabstop(tabstop, start_offset..state.char_len);
            }
            Some(next_idx)
        }
        '|' => parse_snippet_choice(chars, idx + 1, tabstop, state),
        _ => None,
    }
}

fn parse_snippet_choice(
    chars: &[char],
    mut idx: usize,
    tabstop: Option<usize>,
    state: &mut SnippetExpansionState,
) -> Option<usize> {
    let mut taking_first = true;
    let start_offset = state.char_len;
    while idx < chars.len() {
        match chars[idx] {
            '\\' => {
                if let Some(next) = chars.get(idx + 1).copied() {
                    if is_snippet_escape(next, true) {
                        if taking_first {
                            state.push_char(next);
                        }
                        idx += 2;
                    } else {
                        if taking_first {
                            state.push_char('\\');
                        }
                        idx += 1;
                    }
                } else {
                    if taking_first {
                        state.push_char('\\');
                    }
                    idx += 1;
                }
            }
            ',' if taking_first => {
                taking_first = false;
                idx += 1;
            }
            '|' if chars.get(idx + 1) == Some(&'}') => {
                if let Some(tabstop) = tabstop {
                    state.record_placeholder_tabstop(tabstop, start_offset..state.char_len);
                }
                return Some(idx + 2);
            }
            ch => {
                if taking_first {
                    state.push_char(ch);
                }
                idx += 1;
            }
        }
        if state.overflowed {
            return None;
        }
    }
    None
}

fn is_snippet_escape(ch: char, in_choice: bool) -> bool {
    matches!(ch, '$' | '}' | '\\') || (in_choice && matches!(ch, ',' | '|'))
}

fn snippet_tabstop(chars: &[char], range: Range<usize>) -> Option<usize> {
    if range.start == range.end {
        return None;
    }
    let mut value = 0usize;
    for idx in range {
        let digit = chars.get(idx)?.to_digit(10)? as usize;
        value = value.saturating_mul(10).saturating_add(digit);
    }
    Some(value)
}

fn snippet_tabstop_groups(
    mut tabstops: Vec<(usize, Range<usize>)>,
    final_cursor: Option<usize>,
) -> Vec<Vec<Range<usize>>> {
    tabstops.sort_by_key(|(tabstop, _)| *tabstop);
    let mut output: Vec<Vec<Range<usize>>> = Vec::new();
    let mut last_tabstop = None;
    for (tabstop, range) in tabstops {
        if last_tabstop == Some(tabstop) {
            if let Some(group) = output.last_mut() {
                group.push(range);
            }
        } else {
            output.push(vec![range]);
            last_tabstop = Some(tabstop);
        }
    }
    if let Some(cursor) = final_cursor {
        output.push(single_snippet_tabstop_group(cursor..cursor));
    }
    output
}

fn single_snippet_tabstop_group(range: Range<usize>) -> Vec<Range<usize>> {
    std::iter::once(range).collect()
}

fn snippet_tabstops(tabstop_groups: &[Vec<Range<usize>>]) -> Vec<Range<usize>> {
    tabstop_groups
        .iter()
        .filter_map(|group| group.first().cloned())
        .collect()
}

fn consume_ascii_digits(chars: &[char], mut idx: usize) -> usize {
    while chars.get(idx).is_some_and(|ch| ch.is_ascii_digit()) {
        idx += 1;
    }
    idx
}

fn consume_snippet_variable(chars: &[char], mut idx: usize) -> usize {
    while chars
        .get(idx)
        .is_some_and(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
    {
        idx += 1;
    }
    idx
}

fn is_snippet_variable_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}
