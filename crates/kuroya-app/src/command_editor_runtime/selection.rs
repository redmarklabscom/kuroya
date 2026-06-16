use crate::{KuroyaApp, syntax_tree_cache::TreeSitterSyntaxCache};
use kuroya_core::{BufferId, Command, Selection, TextBuffer, clamp_editor_multi_cursor_limit};
use std::ops::Range;

pub(super) fn run_selection_editor_command(app: &mut KuroyaApp, command: &Command) -> bool {
    match command {
        Command::SelectLines => {
            app.select_lines();
            true
        }
        Command::SelectRectangularBlock => {
            app.select_rectangular_block();
            true
        }
        Command::ExpandSelection => {
            app.expand_selection();
            true
        }
        Command::SelectNextOccurrence => {
            app.select_next_occurrence();
            true
        }
        Command::SelectAllOccurrences => {
            app.select_all_occurrences();
            true
        }
        Command::AddCursorAbove => {
            app.add_cursor_above();
            true
        }
        Command::AddCursorBelow => {
            app.add_cursor_below();
            true
        }
        Command::AddCursorsToLineEnds => {
            app.add_cursors_to_line_ends();
            true
        }
        _ => false,
    }
}

impl KuroyaApp {
    fn select_lines(&mut self) {
        let Some(id) = self.active_editor_buffer_id() else {
            return;
        };
        if self.buffer_mut(id).is_some_and(TextBuffer::select_lines) {
            self.status = "Selected lines".to_owned();
        }
    }

    fn select_rectangular_block(&mut self) {
        let multi_cursor_limit = bounded_multi_cursor_limit(self.settings.multi_cursor_limit);
        let Some(id) = self.active_editor_buffer_id() else {
            return;
        };
        if let Some(buffer) = self.buffer_mut(id) {
            if buffer.select_rectangular_block_with_limit(multi_cursor_limit) {
                let count = buffer.selections().len();
                self.status = format!("Selected rectangular block across {count} lines");
            } else {
                self.status = "No rectangular selection range".to_owned();
            }
        }
    }

    fn expand_selection(&mut self) {
        let Some(id) = self.active_editor_buffer_id() else {
            return;
        };
        self.expand_selection_for_buffer(id);
    }

    pub(crate) fn expand_selection_for_buffer(&mut self, id: BufferId) {
        let changed = self
            .expand_tree_sitter_selection_for_buffer(id)
            .unwrap_or_else(|| {
                self.buffer_mut(id)
                    .is_some_and(TextBuffer::expand_selection)
            });
        if changed {
            self.status = "Expanded selection".to_owned();
        } else {
            self.status = "No larger selection range".to_owned();
        }
    }

    fn expand_tree_sitter_selection_for_buffer(&mut self, id: BufferId) -> Option<bool> {
        let buffer_index = self.buffers.iter().position(|buffer| buffer.id() == id)?;
        let (selections, changed) = {
            let buffer = &self.buffers[buffer_index];
            let selections = expanded_selections_with_tree_sitter_fallback(
                &mut self.syntax_tree_cache,
                buffer,
                buffer.selections(),
            );
            let changed = selections.as_slice() != buffer.selections();
            (selections, changed)
        };
        if changed {
            self.buffers[buffer_index].set_selections(selections);
        }
        Some(changed)
    }

    fn select_next_occurrence(&mut self) {
        let multi_cursor_limit = bounded_multi_cursor_limit(self.settings.multi_cursor_limit);
        let Some(id) = self.active_editor_buffer_id() else {
            return;
        };
        if let Some(buffer) = self.buffer_mut(id) {
            if buffer.selections().len() >= multi_cursor_limit {
                self.status = format!("Selection limit reached ({multi_cursor_limit})");
                return;
            }
            if buffer.select_next_occurrence() {
                let count = buffer.selections().len();
                self.status = format!("{count} selections");
            } else {
                self.status = "No next occurrence".to_owned();
            }
        }
    }

    fn select_all_occurrences(&mut self) {
        let multi_cursor_limit = bounded_multi_cursor_limit(self.settings.multi_cursor_limit);
        let Some(id) = self.active_editor_buffer_id() else {
            return;
        };
        if let Some(buffer) = self.buffer_mut(id) {
            let count = buffer.select_all_occurrences(multi_cursor_limit);
            self.status = if count > 0 {
                format!("{count} selections")
            } else {
                "No occurrences".to_owned()
            };
        }
    }

    fn add_cursor_above(&mut self) {
        let multi_cursor_limit = bounded_multi_cursor_limit(self.settings.multi_cursor_limit);
        let Some(id) = self.active_editor_buffer_id() else {
            return;
        };
        if let Some(buffer) = self.buffer_mut(id)
            && !buffer.add_cursor_above_with_limit(multi_cursor_limit)
        {
            self.status = if buffer.selections().len() >= multi_cursor_limit {
                format!("Selection limit reached ({multi_cursor_limit})")
            } else {
                "No line above".to_owned()
            };
        }
    }

    fn add_cursor_below(&mut self) {
        let multi_cursor_limit = bounded_multi_cursor_limit(self.settings.multi_cursor_limit);
        let Some(id) = self.active_editor_buffer_id() else {
            return;
        };
        if let Some(buffer) = self.buffer_mut(id)
            && !buffer.add_cursor_below_with_limit(multi_cursor_limit)
        {
            self.status = if buffer.selections().len() >= multi_cursor_limit {
                format!("Selection limit reached ({multi_cursor_limit})")
            } else {
                "No line below".to_owned()
            };
        }
    }

    fn add_cursors_to_line_ends(&mut self) {
        let multi_cursor_limit = bounded_multi_cursor_limit(self.settings.multi_cursor_limit);
        let Some(id) = self.active_editor_buffer_id() else {
            return;
        };
        if let Some(buffer) = self.buffer_mut(id) {
            if buffer.add_cursors_to_line_ends_with_limit(multi_cursor_limit) {
                self.status = "Added cursors to selected line ends".to_owned();
            } else if buffer.selections().len() >= multi_cursor_limit {
                self.status = format!("Selection limit reached ({multi_cursor_limit})");
            } else {
                self.status = "Cursors already at selected line ends".to_owned();
            }
        }
    }
}

fn expanded_selections_with_tree_sitter_fallback(
    syntax_tree_cache: &mut TreeSitterSyntaxCache,
    buffer: &TextBuffer,
    selections: &[Selection],
) -> Vec<Selection> {
    selections
        .iter()
        .copied()
        .map(|selection| {
            syntax_tree_cache
                .selection_expansion_for_buffer(buffer, selection.range())
                .map(|range| selection_for_expanded_range(selection, range, buffer.len_chars()))
                .unwrap_or_else(|| buffer.expanded_selection_for(selection))
        })
        .collect()
}

fn selection_for_expanded_range(
    selection: Selection,
    range: Range<usize>,
    len_chars: usize,
) -> Selection {
    let start = range.start.min(range.end).min(len_chars);
    let end = range.start.max(range.end).min(len_chars);
    if selection.anchor <= selection.cursor {
        Selection {
            anchor: start,
            cursor: end,
        }
    } else {
        Selection {
            anchor: end,
            cursor: start,
        }
    }
}

fn bounded_multi_cursor_limit(limit: usize) -> usize {
    clamp_editor_multi_cursor_limit(limit).max(1)
}

#[cfg(test)]
mod tests {
    use super::{
        bounded_multi_cursor_limit, expanded_selections_with_tree_sitter_fallback,
        selection_for_expanded_range,
    };
    use crate::syntax_tree_cache::TreeSitterSyntaxCache;
    use kuroya_core::{LanguageId, Selection, TextBuffer};
    use std::ops::Range;

    #[test]
    fn tree_sitter_selection_expansion_falls_back_per_selection() {
        let text = "fn run() {\n    let value = compute(1 + 2);\n}\n".to_owned();
        let buffer = TextBuffer::from_text_with_language(91, None, text.clone(), LanguageId::Rust);
        let compute_start = char_index_of(&text, "compute");
        let compute_end = compute_start + "compute".chars().count();
        let whole_file = Selection {
            anchor: 0,
            cursor: buffer.len_chars(),
        };
        let selections = [
            Selection {
                anchor: compute_start,
                cursor: compute_end,
            },
            whole_file,
        ];
        let mut cache = TreeSitterSyntaxCache::default();

        let expanded =
            expanded_selections_with_tree_sitter_fallback(&mut cache, &buffer, &selections);

        assert_eq!(text_for_range(&text, expanded[0].range()), "compute(1 + 2)");
        assert_eq!(expanded[1], whole_file);
    }

    #[test]
    fn selection_expansion_uses_core_fallback_for_unsupported_languages() {
        let text = "alpha beta\n".to_owned();
        let buffer =
            TextBuffer::from_text_with_language(92, None, text.clone(), LanguageId::PlainText);
        let selection = Selection::caret(char_index_of(&text, "beta") + 1);
        let mut cache = TreeSitterSyntaxCache::default();

        let expanded =
            expanded_selections_with_tree_sitter_fallback(&mut cache, &buffer, &[selection]);

        assert_eq!(text_for_range(&text, expanded[0].range()), "beta");
    }

    #[test]
    fn tree_sitter_selection_expansion_is_clamped_to_buffer_bounds() {
        let reversed_start = 200;
        let reversed_end = 2;
        assert_eq!(
            selection_for_expanded_range(Selection::caret(4), 2..200, 12),
            Selection {
                anchor: 2,
                cursor: 12,
            }
        );
        assert_eq!(
            selection_for_expanded_range(
                Selection {
                    anchor: 8,
                    cursor: 3,
                },
                reversed_start..reversed_end,
                12,
            ),
            Selection {
                anchor: 12,
                cursor: 2,
            }
        );
    }

    #[test]
    fn multi_cursor_limit_is_bounded_for_runtime_commands() {
        assert_eq!(bounded_multi_cursor_limit(0), 1);
        assert_eq!(bounded_multi_cursor_limit(4), 4);
        assert_eq!(bounded_multi_cursor_limit(usize::MAX), 100_000);
    }

    fn char_index_of(text: &str, needle: &str) -> usize {
        let byte_index = text.find(needle).expect("needle should exist in text");
        text[..byte_index].chars().count()
    }

    fn text_for_range(text: &str, range: Range<usize>) -> String {
        text.chars()
            .skip(range.start)
            .take(range.end.saturating_sub(range.start))
            .collect()
    }
}
