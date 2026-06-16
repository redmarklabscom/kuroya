use super::brackets::auto_pair_matches;
use super::edits::{CharGroup, is_word_char_with_separators, normalize_selections};
use super::{DeleteCoalesceKind, Selection, TextBuffer, TextEdit};
use std::ops::Range;

impl TextBuffer {
    pub fn delete_backward(&mut self) -> bool {
        self.delete_backward_with_auto_pair_delete(true)
    }

    pub fn delete_backward_with_auto_pair_delete(&mut self, delete_auto_pairs: bool) -> bool {
        let language_config = self.language.configuration();
        let edits = self
            .selections
            .iter()
            .filter_map(|selection| {
                let range = selection.range();
                if !selection.is_caret() {
                    Some(range)
                } else if selection.cursor > 0 {
                    let start = selection.cursor - 1;
                    let end = if delete_auto_pairs
                        && selection.cursor < self.len_chars()
                        && auto_pair_matches(
                            self.rope.char(start),
                            self.rope.char(selection.cursor),
                            language_config,
                        ) {
                        selection.cursor + 1
                    } else {
                        selection.cursor
                    };
                    Some(start..end)
                } else {
                    None
                }
            })
            .map(|range| TextEdit {
                range,
                inserted: String::new(),
            })
            .collect();
        self.apply_delete_transaction(edits, DeleteCoalesceKind::Backward)
    }

    pub fn delete_forward(&mut self) -> bool {
        let edits = self
            .selections
            .iter()
            .filter_map(|selection| {
                let range = selection.range();
                if !selection.is_caret() {
                    Some(range)
                } else if selection.cursor < self.len_chars() {
                    Some(selection.cursor..selection.cursor + 1)
                } else {
                    None
                }
            })
            .map(|range| TextEdit {
                range,
                inserted: String::new(),
            })
            .collect();
        self.apply_delete_transaction(edits, DeleteCoalesceKind::Forward)
    }

    pub fn delete_forward_with_trim_whitespace_on_delete(&mut self) -> bool {
        let edits = self
            .selections
            .iter()
            .filter_map(|selection| {
                let range = selection.range();
                if !selection.is_caret() {
                    Some(range)
                } else if selection.cursor < self.len_chars() {
                    self.trim_whitespace_on_delete_range(selection.cursor)
                        .or_else(|| Some(selection.cursor..selection.cursor + 1))
                } else {
                    None
                }
            })
            .map(|range| TextEdit {
                range,
                inserted: String::new(),
            })
            .collect();
        self.apply_transaction(edits)
    }

    fn trim_whitespace_on_delete_range(&self, cursor: usize) -> Option<Range<usize>> {
        let position = self.char_position(cursor);
        if position.line + 1 >= self.len_lines()
            || cursor != self.line_content_end_char(position.line)
        {
            return None;
        }

        let next_content_start = self.line_first_non_whitespace_char(position.line + 1);
        (next_content_start > cursor).then_some(cursor..next_content_start)
    }

    pub fn delete_word_backward(&mut self) -> bool {
        let edits = self
            .selections
            .iter()
            .filter_map(|selection| {
                let range = selection.range();
                if !selection.is_caret() {
                    Some(range)
                } else if selection.cursor > 0 {
                    Some(self.previous_word_boundary(selection.cursor)..selection.cursor)
                } else {
                    None
                }
            })
            .map(|range| TextEdit {
                range,
                inserted: String::new(),
            })
            .collect();
        self.apply_transaction(edits)
    }

    pub fn delete_word_forward(&mut self) -> bool {
        let edits = self
            .selections
            .iter()
            .filter_map(|selection| {
                let range = selection.range();
                if !selection.is_caret() {
                    Some(range)
                } else if selection.cursor < self.len_chars() {
                    Some(selection.cursor..self.next_word_boundary(selection.cursor))
                } else {
                    None
                }
            })
            .map(|range| TextEdit {
                range,
                inserted: String::new(),
            })
            .collect();
        self.apply_transaction(edits)
    }

    pub fn delete_selection_ranges(&mut self) -> bool {
        if !self.has_selection() {
            return false;
        }

        let edits = self
            .selections
            .iter()
            .filter_map(|selection| {
                let range = selection.range();
                (range.start != range.end).then_some(TextEdit {
                    range,
                    inserted: String::new(),
                })
            })
            .collect();
        self.apply_transaction(edits)
    }

    pub fn delete_selection_or_lines(&mut self) -> bool {
        if self.has_selection() {
            self.delete_selection_ranges()
        } else {
            self.delete_lines()
        }
    }

    pub fn move_left(&mut self) {
        let cursors = self
            .selections
            .iter()
            .map(|selection| selection.cursor.saturating_sub(1))
            .collect::<Vec<_>>();
        self.set_cursors(cursors);
    }

    pub fn extend_left(&mut self) {
        self.extend_cursors(|_, selection| selection.cursor.saturating_sub(1));
    }

    pub fn move_right(&mut self) {
        let cursors = self
            .selections
            .iter()
            .map(|selection| (selection.cursor + 1).min(self.len_chars()))
            .collect::<Vec<_>>();
        self.set_cursors(cursors);
    }

    pub fn extend_right(&mut self) {
        self.extend_cursors(|buffer, selection| (selection.cursor + 1).min(buffer.len_chars()));
    }

    pub fn move_word_left(&mut self) {
        let cursors = self
            .selections
            .iter()
            .map(|selection| self.previous_word_boundary(selection.cursor))
            .collect::<Vec<_>>();
        self.set_cursors(cursors);
    }

    pub fn move_big_word_left(&mut self) {
        let cursors = self
            .selections
            .iter()
            .map(|selection| self.previous_big_word_boundary(selection.cursor))
            .collect::<Vec<_>>();
        self.set_cursors(cursors);
    }

    pub fn extend_word_left(&mut self) {
        self.extend_cursors(|buffer, selection| buffer.previous_word_boundary(selection.cursor));
    }

    pub fn move_word_right(&mut self) {
        let cursors = self
            .selections
            .iter()
            .map(|selection| self.next_word_boundary(selection.cursor))
            .collect::<Vec<_>>();
        self.set_cursors(cursors);
    }

    pub fn move_big_word_right(&mut self) {
        let cursors = self
            .selections
            .iter()
            .map(|selection| self.next_big_word_boundary(selection.cursor))
            .collect::<Vec<_>>();
        self.set_cursors(cursors);
    }

    pub fn move_word_end(&mut self) {
        let cursors = self
            .selections
            .iter()
            .map(|selection| self.next_word_end(selection.cursor))
            .collect::<Vec<_>>();
        self.set_cursors(cursors);
    }

    pub fn move_big_word_end(&mut self) {
        let cursors = self
            .selections
            .iter()
            .map(|selection| self.next_big_word_end(selection.cursor))
            .collect::<Vec<_>>();
        self.set_cursors(cursors);
    }

    pub fn move_previous_word_end(&mut self) {
        let cursors = self
            .selections
            .iter()
            .map(|selection| self.previous_word_end(selection.cursor))
            .collect::<Vec<_>>();
        self.set_cursors(cursors);
    }

    pub fn extend_word_right(&mut self) {
        self.extend_cursors(|buffer, selection| buffer.next_word_boundary(selection.cursor));
    }

    pub fn move_up(&mut self) {
        let cursors = self
            .selections
            .iter()
            .map(|selection| {
                let pos = self.char_position(selection.cursor);
                if pos.line == 0 {
                    selection.cursor
                } else {
                    self.line_column_to_char(pos.line - 1, pos.column)
                }
            })
            .collect::<Vec<_>>();
        self.set_cursors(cursors);
    }

    pub fn extend_up(&mut self) {
        self.extend_cursors(|buffer, selection| {
            let pos = buffer.char_position(selection.cursor);
            if pos.line == 0 {
                selection.cursor
            } else {
                buffer.line_column_to_char(pos.line - 1, pos.column)
            }
        });
    }

    pub fn move_down(&mut self) {
        let cursors = self
            .selections
            .iter()
            .map(|selection| {
                let pos = self.char_position(selection.cursor);
                if pos.line + 1 >= self.len_lines() {
                    selection.cursor
                } else {
                    self.line_column_to_char(pos.line + 1, pos.column)
                }
            })
            .collect::<Vec<_>>();
        self.set_cursors(cursors);
    }

    pub fn extend_down(&mut self) {
        self.extend_cursors(|buffer, selection| {
            let pos = buffer.char_position(selection.cursor);
            if pos.line + 1 >= buffer.len_lines() {
                selection.cursor
            } else {
                buffer.line_column_to_char(pos.line + 1, pos.column)
            }
        });
    }

    pub fn move_line_start(&mut self) {
        let cursors = self
            .selections
            .iter()
            .map(|selection| self.smart_line_start_char(selection.cursor))
            .collect::<Vec<_>>();
        self.set_cursors(cursors);
    }

    pub fn move_line_column_start(&mut self) {
        let cursors = self
            .selections
            .iter()
            .map(|selection| {
                let pos = self.char_position(selection.cursor);
                self.rope.line_to_char(pos.line)
            })
            .collect::<Vec<_>>();
        self.set_cursors(cursors);
    }

    pub fn move_line_first_non_whitespace(&mut self) {
        let cursors = self
            .selections
            .iter()
            .map(|selection| {
                let pos = self.char_position(selection.cursor);
                self.line_first_non_whitespace_char(pos.line)
            })
            .collect::<Vec<_>>();
        self.set_cursors(cursors);
    }

    pub fn extend_line_start(&mut self) {
        self.extend_cursors(|buffer, selection| buffer.smart_line_start_char(selection.cursor));
    }

    fn smart_line_start_char(&self, cursor: usize) -> usize {
        let pos = self.char_position(cursor);
        let line_start = self.rope.line_to_char(pos.line);
        let content_start = self.line_first_non_whitespace_char(pos.line);
        let content_end = self.line_content_end_char(pos.line);

        if content_start >= content_end || cursor == content_start {
            line_start
        } else {
            content_start
        }
    }

    pub fn move_line_end(&mut self) {
        let cursors = self
            .selections
            .iter()
            .map(|selection| {
                let pos = self.char_position(selection.cursor);
                self.line_content_end_char(pos.line)
            })
            .collect::<Vec<_>>();
        self.set_cursors(cursors);
    }

    pub fn extend_line_end(&mut self) {
        self.extend_cursors(|buffer, selection| {
            let pos = buffer.char_position(selection.cursor);
            buffer.line_content_end_char(pos.line)
        });
    }

    fn extend_cursors(&mut self, next_cursor: impl Fn(&Self, Selection) -> usize) {
        let len_chars = self.len_chars();
        self.selections = normalize_selections(
            self.selections
                .iter()
                .map(|selection| Selection {
                    anchor: selection.anchor.min(len_chars),
                    cursor: next_cursor(self, *selection).min(len_chars),
                })
                .collect(),
            len_chars,
        );
    }

    fn previous_word_boundary(&self, cursor: usize) -> usize {
        let mut idx = cursor.min(self.len_chars());
        if idx == 0 {
            return 0;
        }

        idx -= 1;
        while idx > 0 && self.char_group(self.rope.char(idx)) == CharGroup::Whitespace {
            idx -= 1;
        }

        let group = self.char_group(self.rope.char(idx));
        while idx > 0 && self.char_group(self.rope.char(idx - 1)) == group {
            idx -= 1;
        }
        idx
    }

    fn next_word_boundary(&self, cursor: usize) -> usize {
        let len = self.len_chars();
        let mut idx = cursor.min(len);
        if idx >= len {
            return len;
        }

        while idx < len && self.char_group(self.rope.char(idx)) == CharGroup::Whitespace {
            idx += 1;
        }
        if idx >= len {
            return len;
        }

        let group = self.char_group(self.rope.char(idx));
        while idx < len && self.char_group(self.rope.char(idx)) == group {
            idx += 1;
        }
        idx
    }

    fn previous_big_word_boundary(&self, cursor: usize) -> usize {
        let mut idx = cursor.min(self.len_chars());
        if idx == 0 {
            return 0;
        }

        idx -= 1;
        while idx > 0 && self.rope.char(idx).is_whitespace() {
            idx -= 1;
        }

        while idx > 0 && !self.rope.char(idx - 1).is_whitespace() {
            idx -= 1;
        }
        idx
    }

    fn next_big_word_boundary(&self, cursor: usize) -> usize {
        let len = self.len_chars();
        let mut idx = cursor.min(len);
        if idx >= len {
            return len;
        }

        if !self.rope.char(idx).is_whitespace() {
            while idx < len && !self.rope.char(idx).is_whitespace() {
                idx += 1;
            }
        }
        while idx < len && self.rope.char(idx).is_whitespace() {
            idx += 1;
        }
        idx
    }

    fn next_word_end(&self, cursor: usize) -> usize {
        let len = self.len_chars();
        let mut idx = cursor.min(len);
        if idx >= len {
            return len;
        }

        if self.char_group(self.rope.char(idx)) != CharGroup::Whitespace {
            let end = self.next_word_boundary(idx).saturating_sub(1);
            if end > idx {
                return end;
            }
            idx = (idx + 1).min(len);
        }

        while idx < len && self.char_group(self.rope.char(idx)) == CharGroup::Whitespace {
            idx += 1;
        }
        if idx >= len {
            return len;
        }
        self.next_word_boundary(idx).saturating_sub(1)
    }

    fn next_big_word_end(&self, cursor: usize) -> usize {
        let len = self.len_chars();
        let mut idx = cursor.min(len);
        if idx >= len {
            return len;
        }

        if !self.rope.char(idx).is_whitespace() {
            let end = self.big_word_end_at(idx);
            if end > idx {
                return end;
            }
            idx = (idx + 1).min(len);
        }

        while idx < len && self.rope.char(idx).is_whitespace() {
            idx += 1;
        }
        if idx >= len {
            return len;
        }
        self.big_word_end_at(idx)
    }

    fn big_word_end_at(&self, cursor: usize) -> usize {
        let len = self.len_chars();
        let mut idx = cursor.min(len);
        while idx < len && !self.rope.char(idx).is_whitespace() {
            idx += 1;
        }
        idx.saturating_sub(1)
    }

    fn previous_word_end(&self, cursor: usize) -> usize {
        let len = self.len_chars();
        let idx = cursor.min(len);
        if idx == 0 {
            return 0;
        }

        let mut probe = idx - 1;
        if idx < len {
            let current_group = self.char_group(self.rope.char(idx));
            if current_group != CharGroup::Whitespace {
                while probe > 0 && self.char_group(self.rope.char(probe)) == current_group {
                    probe -= 1;
                }
                if self.char_group(self.rope.char(probe)) == current_group {
                    return 0;
                }
            }
        }

        while probe > 0 && self.char_group(self.rope.char(probe)) == CharGroup::Whitespace {
            probe -= 1;
        }
        if self.char_group(self.rope.char(probe)) == CharGroup::Whitespace {
            0
        } else {
            probe
        }
    }

    fn char_group(&self, ch: char) -> CharGroup {
        if ch.is_whitespace() {
            CharGroup::Whitespace
        } else if self.is_word_char(ch) {
            CharGroup::Word
        } else {
            CharGroup::Symbol
        }
    }

    pub(super) fn is_word_char(&self, ch: char) -> bool {
        is_word_char_with_separators(ch, &self.word_separators)
    }

    pub(super) fn is_whole_word_match(
        &self,
        text: &str,
        start_byte: usize,
        end_byte: usize,
    ) -> bool {
        let before = text[..start_byte].chars().next_back();
        let after = text[end_byte..].chars().next();
        !before.is_some_and(|ch| self.is_word_char(ch))
            && !after.is_some_and(|ch| self.is_word_char(ch))
    }
}
