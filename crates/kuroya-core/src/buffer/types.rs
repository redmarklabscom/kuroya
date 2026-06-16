use ropey::Rope;
use serde::{Deserialize, Serialize};
use std::ops::Range;

#[derive(Debug, Clone)]
pub struct TextSnapshot {
    pub(super) rope: Rope,
}

impl TextSnapshot {
    pub fn len_bytes(&self) -> usize {
        self.rope.len_bytes()
    }

    pub fn chunks(&self) -> impl Iterator<Item = &str> {
        self.rope.chunks()
    }

    pub fn text(&self) -> String {
        self.rope.to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selection {
    pub anchor: usize,
    pub cursor: usize,
}

impl Selection {
    pub fn caret(position: usize) -> Self {
        Self {
            anchor: position,
            cursor: position,
        }
    }

    pub fn range(self) -> Range<usize> {
        self.anchor.min(self.cursor)..self.anchor.max(self.cursor)
    }

    pub fn is_caret(self) -> bool {
        self.anchor == self.cursor
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutoPairSettings {
    pub brackets: bool,
    pub quotes: bool,
    pub surround: bool,
    pub overtype: bool,
}

impl Default for AutoPairSettings {
    fn default() -> Self {
        Self {
            brackets: true,
            quotes: true,
            surround: true,
            overtype: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    pub range: Range<usize>,
    pub inserted: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegexReplaceAllOptions {
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub scope: Option<Range<usize>>,
    pub max_matches: usize,
    pub preserve_case: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BufferHistorySnapshot {
    pub len_chars: usize,
    pub checksum: u64,
    pub undo: Vec<BufferHistoryEntrySnapshot>,
    pub redo: Vec<BufferHistoryEntrySnapshot>,
}

impl BufferHistorySnapshot {
    pub fn is_empty(&self) -> bool {
        self.undo.is_empty() && self.redo.is_empty()
    }

    pub fn estimated_bytes(&self) -> usize {
        self.undo
            .iter()
            .chain(self.redo.iter())
            .map(BufferHistoryEntrySnapshot::estimated_bytes)
            .sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BufferHistoryEntrySnapshot {
    pub edits: Vec<BufferHistoryEditSnapshot>,
    pub inverses: Vec<BufferHistoryEditSnapshot>,
    pub selections_before: Vec<Selection>,
    pub selections_after: Vec<Selection>,
}

impl BufferHistoryEntrySnapshot {
    pub fn estimated_bytes(&self) -> usize {
        let edit_bytes = self
            .edits
            .iter()
            .chain(self.inverses.iter())
            .map(BufferHistoryEditSnapshot::estimated_bytes)
            .sum::<usize>();
        let selection_bytes = self
            .selections_before
            .len()
            .saturating_add(self.selections_after.len())
            .saturating_mul(16);
        edit_bytes.saturating_add(selection_bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BufferHistoryEditSnapshot {
    pub start: usize,
    pub end: usize,
    pub inserted: String,
}

impl BufferHistoryEditSnapshot {
    fn estimated_bytes(&self) -> usize {
        self.inserted.len().saturating_add(32)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CursorEdit {
    pub(super) edit: TextEdit,
    pub(super) cursor_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LineDuplicateEdit {
    pub(super) edit: TextEdit,
    pub(super) source_range: Range<usize>,
    pub(super) duplicate_start_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LineMoveEdit {
    pub(super) edit: TextEdit,
    pub(super) block: Range<usize>,
    pub(super) moved_block_local_start: usize,
    pub(super) replacement_lines: Vec<String>,
    pub(super) trailing_newline: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineSnapshot {
    pub number: usize,
    pub char_range: Range<usize>,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorPosition {
    pub line: usize,
    pub column: usize,
    pub char_idx: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BracketColor {
    pub char_idx: usize,
    pub depth: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BracketPairGuide {
    pub open_idx: usize,
    pub close_idx: usize,
    pub depth: usize,
}

#[derive(Debug, Clone)]
pub(super) struct HistoryEntry {
    pub(super) edits: Vec<TextEdit>,
    pub(super) inverses: Vec<TextEdit>,
    pub(super) selections_before: Vec<Selection>,
    pub(super) selections_after: Vec<Selection>,
    pub(super) coalescible_typing: bool,
    pub(super) coalescible_delete: Option<DeleteCoalesceKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DeleteCoalesceKind {
    Backward,
    Forward,
}
