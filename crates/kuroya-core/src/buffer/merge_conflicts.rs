use super::text::rope_slice_text;
use super::{TextBuffer, TextEdit};
use crate::conflicts::{
    MergeConflict, MergeConflictResolution, is_conflict_end_line, is_conflict_separator_line,
    is_conflict_start_line,
};
use std::ops::Range;

impl TextBuffer {
    pub fn merge_conflicts(&self) -> Vec<MergeConflict> {
        let mut conflicts = Vec::new();
        let mut index = 0;

        while index < self.len_lines() {
            if !self.line_matches_conflict_marker(index, is_conflict_start_line) {
                index += 1;
                continue;
            }

            let start_line = index;
            let mut separator_line = None;
            let mut end_line = None;
            index += 1;

            while index < self.len_lines() {
                if separator_line.is_none()
                    && self.line_matches_conflict_marker(index, is_conflict_separator_line)
                {
                    separator_line = Some(index);
                } else if separator_line.is_some()
                    && self.line_matches_conflict_marker(index, is_conflict_end_line)
                {
                    end_line = Some(index);
                    break;
                }
                index += 1;
            }

            if let (Some(separator_line), Some(end_line)) = (separator_line, end_line) {
                conflicts.push(MergeConflict {
                    start_line,
                    separator_line,
                    end_line,
                });
                index = end_line + 1;
            } else {
                index = start_line + 1;
            }
        }

        conflicts
    }

    pub fn resolve_merge_conflict_at_cursor(
        &mut self,
        resolution: MergeConflictResolution,
    ) -> bool {
        self.resolve_merge_conflict_at_line(self.cursor_position().line, resolution)
    }

    pub fn resolve_merge_conflict_at_line(
        &mut self,
        line: usize,
        resolution: MergeConflictResolution,
    ) -> bool {
        if self.read_only {
            return false;
        }
        let Some(conflict) = self.merge_conflict_containing_line(line) else {
            return false;
        };
        let range = self.merge_conflict_char_range(&conflict);
        let replacement = self.merge_conflict_resolution_text(&conflict, resolution);
        self.apply_transaction(vec![TextEdit {
            range,
            inserted: replacement,
        }])
    }

    fn merge_conflict_containing_line(&self, line: usize) -> Option<MergeConflict> {
        if line >= self.len_lines() {
            return None;
        }

        for start_line in (0..=line).rev() {
            if !self.line_matches_conflict_marker(start_line, is_conflict_start_line) {
                continue;
            }

            let mut separator_line = None;
            for line_idx in start_line + 1..self.len_lines() {
                if separator_line.is_none()
                    && self.line_matches_conflict_marker(line_idx, is_conflict_separator_line)
                {
                    separator_line = Some(line_idx);
                } else if separator_line.is_some()
                    && self.line_matches_conflict_marker(line_idx, is_conflict_end_line)
                {
                    let conflict = MergeConflict {
                        start_line,
                        separator_line: separator_line?,
                        end_line: line_idx,
                    };
                    if conflict.contains_line(line) {
                        return Some(conflict);
                    }
                    break;
                }
            }
        }

        None
    }

    fn line_matches_conflict_marker(&self, line_idx: usize, marker: fn(&str) -> bool) -> bool {
        if line_idx >= self.len_lines() {
            return false;
        }

        let line = self.rope.line(line_idx);
        let line = rope_slice_text(&line);
        marker(line.as_ref())
    }

    fn merge_conflict_char_range(&self, conflict: &MergeConflict) -> Range<usize> {
        let start = self.rope.line_to_char(conflict.start_line);
        let end = if conflict.end_line + 1 < self.len_lines() {
            self.rope.line_to_char(conflict.end_line + 1)
        } else {
            self.len_chars()
        };
        start..end
    }

    fn merge_conflict_resolution_text(
        &self,
        conflict: &MergeConflict,
        resolution: MergeConflictResolution,
    ) -> String {
        let mut resolved =
            String::with_capacity(self.merge_conflict_resolution_len(conflict, resolution));
        match resolution {
            MergeConflictResolution::Current => {
                self.push_lines(
                    &mut resolved,
                    conflict.start_line + 1..conflict.separator_line,
                );
            }
            MergeConflictResolution::Incoming => {
                self.push_lines(
                    &mut resolved,
                    conflict.separator_line + 1..conflict.end_line,
                );
            }
            MergeConflictResolution::Both => {
                self.push_lines(
                    &mut resolved,
                    conflict.start_line + 1..conflict.separator_line,
                );
                self.push_lines(
                    &mut resolved,
                    conflict.separator_line + 1..conflict.end_line,
                );
            }
        }
        resolved
    }

    fn merge_conflict_resolution_len(
        &self,
        conflict: &MergeConflict,
        resolution: MergeConflictResolution,
    ) -> usize {
        match resolution {
            MergeConflictResolution::Current => {
                self.lines_byte_len(conflict.start_line + 1..conflict.separator_line)
            }
            MergeConflictResolution::Incoming => {
                self.lines_byte_len(conflict.separator_line + 1..conflict.end_line)
            }
            MergeConflictResolution::Both => {
                self.lines_byte_len(conflict.start_line + 1..conflict.separator_line)
                    + self.lines_byte_len(conflict.separator_line + 1..conflict.end_line)
            }
        }
    }

    fn push_lines(&self, output: &mut String, lines: Range<usize>) {
        for line_idx in lines {
            let line = self.rope.line(line_idx);
            let line = rope_slice_text(&line);
            output.push_str(line.as_ref());
        }
    }

    fn lines_byte_len(&self, lines: Range<usize>) -> usize {
        lines
            .map(|line_idx| self.rope.line(line_idx).len_bytes())
            .sum()
    }
}
