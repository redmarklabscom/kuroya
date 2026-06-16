use super::text::rope_slice_to_string;
use super::{LineSnapshot, TextBuffer, TextSnapshot};
use std::ops::Range;

impl TextBuffer {
    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    pub fn len_bytes(&self) -> usize {
        self.rope.len_bytes()
    }

    pub fn char_to_byte(&self, char_idx: usize) -> usize {
        self.rope.char_to_byte(char_idx.min(self.len_chars()))
    }

    pub fn byte_to_char(&self, byte_idx: usize) -> usize {
        self.rope.byte_to_char(byte_idx.min(self.len_bytes()))
    }

    pub fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }

    pub fn ends_with_newline(&self) -> bool {
        self.len_chars() > 0 && matches!(self.rope.char(self.len_chars() - 1), '\n' | '\r')
    }

    pub fn is_final_newline_line(&self, line_idx: usize) -> bool {
        self.ends_with_newline()
            && self.len_lines() > 1
            && line_idx.saturating_add(1) == self.len_lines()
    }

    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    pub fn text_snapshot(&self) -> TextSnapshot {
        TextSnapshot {
            rope: self.rope.clone(),
        }
    }

    pub fn text_equals(&self, text: &str) -> bool {
        if self.len_bytes() != text.len() {
            return false;
        }

        let mut remaining = text;
        for chunk in self.rope.chunks() {
            if !remaining.starts_with(chunk) {
                return false;
            }
            remaining = &remaining[chunk.len()..];
        }
        remaining.is_empty()
    }

    pub fn text_equals_buffer(&self, other: &TextBuffer) -> bool {
        if self.len_bytes() != other.len_bytes() {
            return false;
        }

        let mut left_chunks = self.rope.chunks();
        let mut right_chunks = other.rope.chunks();
        let mut left = left_chunks.next();
        let mut right = right_chunks.next();
        let mut left_offset = 0usize;
        let mut right_offset = 0usize;

        loop {
            while left.is_some_and(|chunk| left_offset >= chunk.len()) {
                left = left_chunks.next();
                left_offset = 0;
            }
            while right.is_some_and(|chunk| right_offset >= chunk.len()) {
                right = right_chunks.next();
                right_offset = 0;
            }

            match (left, right) {
                (None, None) => return true,
                (Some(left_chunk), Some(right_chunk)) => {
                    let left_bytes = left_chunk.as_bytes();
                    let right_bytes = right_chunk.as_bytes();
                    let take =
                        (left_bytes.len() - left_offset).min(right_bytes.len() - right_offset);
                    if left_bytes[left_offset..left_offset + take]
                        != right_bytes[right_offset..right_offset + take]
                    {
                        return false;
                    }
                    left_offset += take;
                    right_offset += take;
                }
                _ => return false,
            }
        }
    }

    pub fn line(&self, line_idx: usize) -> Option<String> {
        (line_idx < self.len_lines()).then(|| rope_slice_to_string(self.rope.line(line_idx)))
    }

    pub fn text_range(&self, range: Range<usize>) -> Option<String> {
        if range.start > range.end || range.end > self.len_chars() {
            return None;
        }
        Some(rope_slice_to_string(self.rope.slice(range)))
    }

    pub fn char_at(&self, char_idx: usize) -> Option<char> {
        (char_idx < self.len_chars()).then(|| self.rope.char(char_idx))
    }

    pub fn line_starts_with(&self, line_idx: usize, prefix: &str) -> bool {
        if line_idx >= self.len_lines() {
            return false;
        }
        let mut expected = prefix.chars();
        let mut current = expected.next();
        if current.is_none() {
            return true;
        }
        for ch in self.rope.line(line_idx).chars() {
            let Some(needle) = current else {
                return true;
            };
            if ch != needle {
                return false;
            }
            current = expected.next();
        }
        current.is_none()
    }

    pub fn line_content_prefix(&self, line_idx: usize, max_chars: usize) -> Option<String> {
        if line_idx >= self.len_lines() {
            return None;
        }
        let start = self.rope.line_to_char(line_idx);
        let end = start.saturating_add(self.line_content_char_count_capped(line_idx, max_chars));
        Some(rope_slice_to_string(self.rope.slice(start..end)))
    }

    pub fn line_content_end_char(&self, line_idx: usize) -> usize {
        if line_idx >= self.len_lines() {
            return self.len_chars();
        }

        let start = self.rope.line_to_char(line_idx);
        let next_line = line_idx.saturating_add(1);
        let mut end = if next_line < self.len_lines() {
            self.rope.line_to_char(next_line)
        } else {
            self.len_chars()
        };
        while end > start && matches!(self.rope.char(end - 1), '\r' | '\n') {
            end -= 1;
        }
        end
    }

    pub fn line_content_char_count_capped(&self, line_idx: usize, max_chars: usize) -> usize {
        if line_idx >= self.len_lines() || max_chars == 0 {
            return 0;
        }

        let mut content_count = 0usize;
        let mut pending_line_endings = 0usize;
        for ch in self.rope.line(line_idx).chars() {
            if matches!(ch, '\r' | '\n') {
                pending_line_endings = pending_line_endings.saturating_add(1);
                continue;
            }

            content_count = content_count
                .saturating_add(pending_line_endings)
                .saturating_add(1);
            pending_line_endings = 0;
            if content_count >= max_chars {
                return max_chars;
            }
        }

        content_count.min(max_chars)
    }

    pub fn line_leading_indent_visual_width_capped(
        &self,
        line_idx: usize,
        max_chars: usize,
        tab_width: usize,
    ) -> Option<usize> {
        if line_idx >= self.len_lines() {
            return None;
        }

        let tab_width = tab_width.max(1);
        let mut width = 0usize;
        for (count, ch) in self.rope.line(line_idx).chars().enumerate() {
            if count >= max_chars {
                break;
            }
            match ch {
                ' ' => width = width.saturating_add(1),
                '\t' => {
                    let remainder = width % tab_width;
                    width = width.saturating_add(if remainder == 0 {
                        tab_width
                    } else {
                        tab_width - remainder
                    });
                }
                '\r' | '\n' => break,
                _ => break,
            }
        }

        Some(width)
    }

    pub fn line_snapshot(&self, line_idx: usize) -> Option<LineSnapshot> {
        if line_idx >= self.len_lines() {
            return None;
        }

        let start = self.rope.line_to_char(line_idx);
        let next_line = line_idx.saturating_add(1);
        let next = if next_line < self.len_lines() {
            self.rope.line_to_char(next_line)
        } else {
            self.rope.len_chars()
        };
        Some(LineSnapshot {
            number: line_idx.saturating_add(1),
            char_range: start..next,
            text: rope_slice_to_string(self.rope.slice(start..next)),
        })
    }

    pub fn line_snapshot_prefix(&self, line_idx: usize, max_chars: usize) -> Option<LineSnapshot> {
        if line_idx >= self.len_lines() {
            return None;
        }

        let start = self.rope.line_to_char(line_idx);
        let end = start.saturating_add(self.line_content_char_count_capped(line_idx, max_chars));
        Some(LineSnapshot {
            number: line_idx.saturating_add(1),
            char_range: start..end,
            text: rope_slice_to_string(self.rope.slice(start..end)),
        })
    }

    pub fn visible_lines(&self, first_line: usize, count: usize) -> Vec<LineSnapshot> {
        if count == 0 {
            return Vec::new();
        }

        let len_lines = self.len_lines();
        if first_line >= len_lines {
            return Vec::new();
        }

        let end = first_line.saturating_add(count).min(len_lines);
        let mut snapshots = Vec::with_capacity(end - first_line);
        let mut start = self.rope.line_to_char(first_line);
        for line_idx in first_line..end {
            let next_line = line_idx.saturating_add(1);
            let next = if next_line < len_lines {
                self.rope.line_to_char(next_line)
            } else {
                self.rope.len_chars()
            };
            snapshots.push(LineSnapshot {
                number: line_idx.saturating_add(1),
                char_range: start..next,
                text: rope_slice_to_string(self.rope.slice(start..next)),
            });
            start = next;
        }
        snapshots
    }
}
