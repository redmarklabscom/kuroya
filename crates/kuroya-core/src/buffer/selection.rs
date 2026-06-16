use super::edits::{consider_expansion_candidate, normalize_selections};
use super::text::push_rope_slice_text;
use super::{CursorPosition, Selection, TextBuffer};
use std::ops::Range;

impl TextBuffer {
    pub fn word_at_cursor(&self) -> Option<String> {
        let range = self.word_range_at_cursor()?;
        Some(self.rope.slice(range).to_string())
    }

    pub fn word_range_at_cursor(&self) -> Option<Range<usize>> {
        let cursor = self.selections.last().copied()?.cursor;
        self.word_range_at(cursor)
    }

    pub fn completion_prefix_range(&self) -> Option<Range<usize>> {
        let cursor = self
            .selections
            .last()
            .copied()?
            .cursor
            .min(self.len_chars());
        let mut start = cursor;
        while start > 0 && self.is_word_char(self.rope.char(start - 1)) {
            start -= 1;
        }
        Some(start..cursor)
    }

    pub fn select_all(&mut self) {
        self.selections = vec![Selection {
            anchor: 0,
            cursor: self.len_chars(),
        }];
    }

    pub fn selected_text(&self) -> Option<String> {
        let mut selected = String::new();
        let mut has_selected_text = false;
        for selection in &self.selections {
            let range = selection.range();
            if range.start == range.end {
                continue;
            }
            if has_selected_text {
                selected.push('\n');
            }
            push_rope_slice_text(&mut selected, self.rope.slice(range));
            has_selected_text = true;
        }

        has_selected_text.then_some(selected)
    }

    pub fn selected_text_or_lines(&self) -> Option<String> {
        if self.has_selection() {
            return self.selected_text();
        }

        let mut selected = String::new();
        for block in self.selected_line_blocks() {
            let range = self.line_block_char_range(block);
            if range.start != range.end {
                push_rope_slice_text(&mut selected, self.rope.slice(range));
            }
        }

        (!selected.is_empty()).then_some(selected)
    }

    pub fn select_lines(&mut self) -> bool {
        let blocks = self.selected_line_blocks();
        let line_selections = self.selections_for_line_blocks(blocks.clone());
        let selections = if line_selections == self.selections {
            self.selections_for_line_blocks(
                blocks
                    .into_iter()
                    .map(|block| block.start..block.end.saturating_add(1).min(self.len_lines()))
                    .collect(),
            )
        } else {
            line_selections
        };

        if selections == self.selections {
            return false;
        }

        self.selections = selections;
        true
    }

    fn selections_for_line_blocks(&self, blocks: Vec<Range<usize>>) -> Vec<Selection> {
        normalize_selections(
            blocks
                .into_iter()
                .map(|block| {
                    let range = self.line_block_char_range(block);
                    Selection {
                        anchor: range.start,
                        cursor: range.end,
                    }
                })
                .collect(),
            self.len_chars(),
        )
    }

    pub fn cursor(&self) -> usize {
        self.selections
            .last()
            .map(|selection| selection.cursor)
            .unwrap_or_default()
            .min(self.len_chars())
    }

    pub fn set_single_cursor(&mut self, char_idx: usize) {
        let char_idx = char_idx.min(self.len_chars());
        self.selections.clear();
        self.selections.push(Selection::caret(char_idx));
    }

    pub fn set_selection(&mut self, anchor: usize, cursor: usize) {
        let len_chars = self.len_chars();
        self.selections = normalize_selections(
            vec![Selection {
                anchor: anchor.min(len_chars),
                cursor: cursor.min(len_chars),
            }],
            len_chars,
        );
    }

    pub fn set_selections<I>(&mut self, selections: I)
    where
        I: IntoIterator<Item = Selection>,
    {
        self.selections = normalize_selections(selections.into_iter().collect(), self.len_chars());
    }

    pub fn set_cursors<I>(&mut self, cursors: I)
    where
        I: IntoIterator<Item = usize>,
    {
        self.selections = normalize_selections(
            cursors
                .into_iter()
                .map(|cursor| Selection::caret(cursor.min(self.len_chars())))
                .collect(),
            self.len_chars(),
        );
    }

    pub fn add_cursor(&mut self, char_idx: usize) {
        let _ = self.add_cursor_with_limit(char_idx, usize::MAX);
    }

    pub fn add_cursor_with_limit(&mut self, char_idx: usize, max_selections: usize) -> bool {
        self.add_cursors_with_limit(std::iter::once(char_idx), max_selections)
    }

    pub fn select_next_occurrence(&mut self) -> bool {
        let Some(primary) = self.selections.last().copied() else {
            return false;
        };

        if primary.is_caret() {
            let Some(range) = self.word_range_at(primary.cursor) else {
                return false;
            };
            self.set_selection(range.start, range.end);
            return true;
        }

        let query_range = primary.range();
        let query_end = query_range.end;
        let query = self.rope.slice(query_range).to_string();
        if query.is_empty() {
            return false;
        }

        let Some(next) = self.next_occurrence_after(&query, query_end, &self.selections) else {
            return false;
        };

        self.selections.push(Selection {
            anchor: next.start,
            cursor: next.end,
        });
        self.selections =
            normalize_selections(std::mem::take(&mut self.selections), self.len_chars());
        true
    }

    pub fn select_all_occurrences(&mut self, max_matches: usize) -> usize {
        if max_matches == 0 {
            return 0;
        }

        let Some(primary) = self.selections.last().copied() else {
            return 0;
        };
        let query_range = if primary.is_caret() {
            let Some(range) = self.word_range_at(primary.cursor) else {
                return 0;
            };
            range
        } else {
            primary.range()
        };

        let query = self.rope.slice(query_range).to_string();
        if query.is_empty() {
            return 0;
        }

        let ranges = self
            .find_matches_with_options(&query, max_matches, true, false)
            .into_iter()
            .map(|range| Selection {
                anchor: range.start,
                cursor: range.end,
            })
            .collect::<Vec<_>>();

        if ranges.is_empty() {
            return 0;
        }

        self.selections = normalize_selections(ranges, self.len_chars());
        self.selections.len()
    }

    fn word_range_at(&self, cursor: usize) -> Option<Range<usize>> {
        let len = self.len_chars();
        if len == 0 {
            return None;
        }

        let cursor = cursor.min(len);
        let word_idx = if cursor < len && self.is_word_char(self.rope.char(cursor)) {
            cursor
        } else if cursor > 0 && self.is_word_char(self.rope.char(cursor - 1)) {
            cursor - 1
        } else {
            return None;
        };

        let mut start = word_idx;
        while start > 0 && self.is_word_char(self.rope.char(start - 1)) {
            start -= 1;
        }

        let mut end = word_idx + 1;
        while end < len && self.is_word_char(self.rope.char(end)) {
            end += 1;
        }

        Some(start..end)
    }

    fn next_occurrence_after(
        &self,
        query: &str,
        after: usize,
        selected: &[Selection],
    ) -> Option<Range<usize>> {
        if query.is_empty() {
            return None;
        }

        let len_chars = self.len_chars();
        let after = after.min(len_chars);
        let mut remaining_query_chars = query.chars();
        let first_query_char = remaining_query_chars.next()?;
        let Some(second_query_char) = remaining_query_chars.next() else {
            return self
                .find_next_unselected_single_char_match(
                    first_query_char,
                    after,
                    len_chars,
                    selected,
                    len_chars,
                )
                .or_else(|| {
                    self.find_next_unselected_single_char_match(
                        first_query_char,
                        0,
                        after,
                        selected,
                        len_chars,
                    )
                });
        };
        let mut query_chars = Vec::with_capacity(2 + remaining_query_chars.size_hint().0);
        query_chars.push(first_query_char);
        query_chars.push(second_query_char);
        query_chars.extend(remaining_query_chars);
        self.find_next_unselected_char_match(&query_chars, after, len_chars, selected, len_chars)
            .or_else(|| {
                self.find_next_unselected_char_match(&query_chars, 0, after, selected, len_chars)
            })
    }

    fn find_next_unselected_single_char_match(
        &self,
        query_char: char,
        start_char: usize,
        end_char: usize,
        selected: &[Selection],
        len_chars: usize,
    ) -> Option<Range<usize>> {
        let start_char = start_char.min(len_chars);
        let end_char = end_char.min(len_chars);
        if start_char >= end_char {
            return None;
        }

        for (offset, ch) in self
            .rope
            .chars_at(start_char)
            .take(end_char - start_char)
            .enumerate()
        {
            if ch != query_char {
                continue;
            }
            let start = start_char + offset;
            let end = start + 1;
            if !selected.iter().any(|selection| {
                let range = selection.range();
                range.start == start && range.end == end
            }) {
                return Some(start..end);
            }
        }

        None
    }

    fn find_next_unselected_char_match(
        &self,
        query_chars: &[char],
        start_char: usize,
        end_char: usize,
        selected: &[Selection],
        len_chars: usize,
    ) -> Option<Range<usize>> {
        let query_len = query_chars.len();
        if query_len == 0 {
            return None;
        }

        let start_char = start_char.min(len_chars);
        let end_char = end_char.min(len_chars);
        if start_char >= end_char || query_len > end_char - start_char {
            return None;
        }

        let max_start = end_char - query_len;
        let (&first_query_char, tail_query_chars) = query_chars.split_first()?;
        for (offset, ch) in self
            .rope
            .chars_at(start_char)
            .take(end_char - start_char)
            .enumerate()
        {
            let start = start_char + offset;
            if start > max_start {
                break;
            }
            if ch != first_query_char
                || !self.query_tail_matches_at(start + 1, tail_query_chars, true, len_chars)
            {
                continue;
            }
            let end = start + query_len;
            if !selected.iter().any(|selection| {
                let range = selection.range();
                range.start == start && range.end == end
            }) {
                return Some(start..end);
            }
        }

        None
    }

    pub fn add_cursor_above(&mut self) {
        let _ = self.add_cursor_above_with_limit(usize::MAX);
    }

    pub fn add_cursor_above_with_limit(&mut self, max_selections: usize) -> bool {
        let cursors = self
            .selections
            .iter()
            .filter_map(|selection| {
                let pos = self.char_position(selection.cursor);
                (pos.line > 0).then(|| self.line_column_to_char(pos.line - 1, pos.column))
            })
            .collect::<Vec<_>>();
        self.add_cursors_with_limit(cursors, max_selections)
    }

    pub fn add_cursor_below(&mut self) {
        let _ = self.add_cursor_below_with_limit(usize::MAX);
    }

    pub fn add_cursor_below_with_limit(&mut self, max_selections: usize) -> bool {
        let cursors = self
            .selections
            .iter()
            .filter_map(|selection| {
                let pos = self.char_position(selection.cursor);
                (pos.line + 1 < self.len_lines())
                    .then(|| self.line_column_to_char(pos.line + 1, pos.column))
            })
            .collect::<Vec<_>>();
        self.add_cursors_with_limit(cursors, max_selections)
    }

    fn add_cursors_with_limit<I>(&mut self, cursors: I, max_selections: usize) -> bool
    where
        I: IntoIterator<Item = usize>,
    {
        if max_selections == 0 {
            return false;
        }

        let len_chars = self.len_chars();
        let mut selections = self.selections.clone();
        if selections.len() >= max_selections {
            return false;
        }

        let mut added = false;
        for cursor in cursors {
            let candidate = Selection::caret(cursor.min(len_chars));
            if selections.contains(&candidate) {
                continue;
            }
            selections.push(candidate);
            added = true;
            if selections.len() >= max_selections {
                break;
            }
        }
        if !added {
            return false;
        }
        selections = normalize_selections(selections, len_chars);

        if selections == self.selections {
            return false;
        }

        self.selections = selections;
        true
    }

    pub fn add_cursors_to_line_ends(&mut self) -> bool {
        self.add_cursors_to_line_ends_with_limit(usize::MAX)
    }

    pub fn add_cursors_to_line_ends_with_limit(&mut self, max_selections: usize) -> bool {
        if max_selections == 0 {
            return false;
        }

        let selections = normalize_selections(
            self.selected_line_indices()
                .into_iter()
                .map(|line| Selection::caret(self.line_content_end_char(line)))
                .collect(),
            self.len_chars(),
        );
        let selections = selections
            .into_iter()
            .take(max_selections)
            .collect::<Vec<_>>();
        if selections == self.selections {
            return false;
        }

        self.selections = selections;
        true
    }

    pub fn select_rectangular_block(&mut self) -> bool {
        self.select_rectangular_block_with_limit(usize::MAX)
    }

    pub fn select_rectangular_block_with_limit(&mut self, max_selections: usize) -> bool {
        if max_selections == 0 {
            return false;
        }

        let Some(selection) = self.selections.last().copied() else {
            return false;
        };
        let anchor = self.char_position(selection.anchor);
        let cursor = self.char_position(selection.cursor);
        if anchor.line == cursor.line {
            return false;
        }

        let start_line = anchor.line.min(cursor.line);
        let end_line = anchor.line.max(cursor.line);
        let start_column = anchor.column.min(cursor.column);
        let end_column = anchor.column.max(cursor.column);
        let reverse_columns = anchor.column > cursor.column;
        let selections = normalize_selections(
            (start_line..=end_line)
                .take(max_selections)
                .map(|line| {
                    let start = self.line_column_to_char(line, start_column);
                    let end = self.line_column_to_char(line, end_column);
                    if reverse_columns {
                        Selection {
                            anchor: end,
                            cursor: start,
                        }
                    } else {
                        Selection {
                            anchor: start,
                            cursor: end,
                        }
                    }
                })
                .collect(),
            self.len_chars(),
        );
        if selections == self.selections {
            return false;
        }

        self.selections = selections;
        true
    }

    pub fn expand_selection(&mut self) -> bool {
        let selections = normalize_selections(
            self.selections
                .iter()
                .copied()
                .map(|selection| self.expand_single_selection(selection))
                .collect(),
            self.len_chars(),
        );
        if selections == self.selections {
            return false;
        }

        self.selections = selections;
        true
    }

    pub fn expanded_selection_for(&self, selection: Selection) -> Selection {
        self.expand_single_selection(selection)
    }

    fn expand_single_selection(&self, selection: Selection) -> Selection {
        let Some(range) = self.expanded_selection_range(selection.range()) else {
            return selection;
        };
        if selection.anchor <= selection.cursor {
            Selection {
                anchor: range.start,
                cursor: range.end,
            }
        } else {
            Selection {
                anchor: range.end,
                cursor: range.start,
            }
        }
    }

    fn expanded_selection_range(&self, range: Range<usize>) -> Option<Range<usize>> {
        let range = range.start.min(self.len_chars())..range.end.min(self.len_chars());
        let mut best = None;
        if let Some(word) = self.word_expansion_for_range(range.clone()) {
            consider_expansion_candidate(&mut best, &range, word);
        }
        self.visit_enclosing_bracket_expansions(&range, |candidate| {
            consider_expansion_candidate(&mut best, &range, candidate);
        });
        consider_expansion_candidate(
            &mut best,
            &range,
            self.line_range_for_char_range(range.clone()),
        );
        consider_expansion_candidate(&mut best, &range, 0..self.len_chars());
        best
    }

    fn word_expansion_for_range(&self, range: Range<usize>) -> Option<Range<usize>> {
        let cursor = range.start.min(self.len_chars());
        let word = self.word_range_at(cursor).or_else(|| {
            (cursor > 0)
                .then(|| self.word_range_at(cursor - 1))
                .flatten()
        })?;
        (word.start <= range.start && word.end >= range.end).then_some(word)
    }

    fn visit_enclosing_bracket_expansions(
        &self,
        range: &Range<usize>,
        mut visit: impl FnMut(Range<usize>),
    ) {
        let mut stack = Vec::new();
        let brackets = self.language.configuration().brackets();
        for idx in 0..self.len_chars() {
            let ch = self.rope.char(idx);
            if let Some(pair) = brackets.iter().copied().find(|pair| pair.open == ch) {
                stack.push((idx, pair));
                continue;
            }
            if let Some((open_idx, pair)) = stack.last().copied()
                && pair.close == ch
            {
                stack.pop();
                let inner = open_idx + 1..idx;
                if inner.start <= range.start && inner.end >= range.end {
                    visit(inner);
                }
                let outer = open_idx..idx + 1;
                if outer.start <= range.start && outer.end >= range.end {
                    visit(outer);
                }
            }
        }
    }

    fn line_range_for_char_range(&self, range: Range<usize>) -> Range<usize> {
        if self.len_lines() == 0 {
            return 0..0;
        }
        let start_line = self.char_position(range.start).line;
        let end_char = if range.end > range.start {
            range.end.saturating_sub(1)
        } else {
            range.end
        };
        let end_line = self.char_position(end_char).line;
        self.line_block_char_range(start_line..end_line.saturating_add(1))
    }

    pub fn cursor_position(&self) -> CursorPosition {
        self.char_position(self.cursor())
    }

    pub fn cursor_positions(&self) -> Vec<CursorPosition> {
        self.selections
            .iter()
            .map(|selection| self.char_position(selection.cursor))
            .collect()
    }

    pub fn char_position(&self, char_idx: usize) -> CursorPosition {
        let char_idx = char_idx.min(self.len_chars());
        let line = if self.len_lines() == 0 {
            0
        } else if char_idx == self.len_chars() && self.len_chars() > 0 {
            self.len_lines().saturating_sub(1)
        } else {
            self.rope.char_to_line(char_idx)
        };
        let line_start = self.rope.line_to_char(line);
        CursorPosition {
            line,
            column: char_idx.saturating_sub(line_start),
            char_idx,
        }
    }

    pub fn line_column_to_char(&self, line: usize, column: usize) -> usize {
        if self.len_lines() == 0 {
            return 0;
        }

        let line = line.min(self.len_lines().saturating_sub(1));
        let start = self.rope.line_to_char(line);
        let end = self.line_content_end_char(line);
        start + column.min(end.saturating_sub(start))
    }
}
