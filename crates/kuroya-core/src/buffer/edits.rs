use super::{
    CursorEdit, LineDuplicateEdit, LineMoveEdit, MAX_EDIT_DELTA_CHARS, Selection, TextBuffer,
    TextEdit,
};
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CharGroup {
    Whitespace,
    Word,
    Symbol,
}

pub(super) fn is_word_char_with_separators(ch: char, separators: &str) -> bool {
    !ch.is_whitespace() && !separators.contains(ch)
}

pub(super) fn normalize_selections(
    mut selections: Vec<Selection>,
    len_chars: usize,
) -> Vec<Selection> {
    for selection in &mut selections {
        selection.anchor = selection.anchor.min(len_chars);
        selection.cursor = selection.cursor.min(len_chars);
    }
    selections.sort_by(|a, b| {
        a.range()
            .start
            .cmp(&b.range().start)
            .then(a.range().end.cmp(&b.range().end))
            .then(a.cursor.cmp(&b.cursor))
    });
    selections.dedup();
    if selections.is_empty() {
        selections.push(Selection::caret(0));
    }
    selections
}

pub(super) fn range_strictly_expands(candidate: &Range<usize>, range: &Range<usize>) -> bool {
    candidate.start <= range.start
        && candidate.end >= range.end
        && (candidate.start < range.start || candidate.end > range.end)
}

pub(super) fn consider_expansion_candidate(
    best: &mut Option<Range<usize>>,
    range: &Range<usize>,
    candidate: Range<usize>,
) {
    if !range_strictly_expands(&candidate, range) {
        return;
    }

    let candidate_key = (range_len(&candidate), candidate.start, candidate.end);
    let should_replace = best
        .as_ref()
        .map(|current| candidate_key < (range_len(current), current.start, current.end))
        .unwrap_or(true);
    if should_replace {
        *best = Some(candidate);
    }
}

pub(super) fn range_len(range: &Range<usize>) -> usize {
    range.end.saturating_sub(range.start)
}

pub(super) fn edit_range_is_valid(range: &Range<usize>, len_chars: usize) -> bool {
    range.start <= range.end && range.end <= len_chars && range_len(range) <= MAX_EDIT_DELTA_CHARS
}

pub(super) fn inserted_text_is_bounded(text: &str) -> bool {
    text.len() <= MAX_EDIT_DELTA_CHARS
}

pub(super) fn edit_delta_after(
    delta: isize,
    inserted_len: usize,
    removed_len: usize,
) -> Option<isize> {
    if inserted_len > MAX_EDIT_DELTA_CHARS || removed_len > MAX_EDIT_DELTA_CHARS {
        return None;
    }

    let inserted_len = inserted_len as isize;
    let removed_len = removed_len as isize;
    delta.checked_add(inserted_len.checked_sub(removed_len)?)
}

pub(super) fn edits_are_replayable<'a>(
    edits: impl IntoIterator<Item = &'a TextEdit>,
    len_chars: usize,
) -> bool {
    let mut current_len = len_chars;
    let mut delta = 0_isize;
    let mut previous_end = 0;

    for edit in edits {
        if !edit_range_is_valid(&edit.range, len_chars) || edit.range.start < previous_end {
            return false;
        }

        let adjusted_start = adjust_index(edit.range.start, delta, current_len);
        let adjusted_end = adjust_index(edit.range.end, delta, current_len).max(adjusted_start);
        if adjusted_start > current_len || adjusted_end > current_len {
            return false;
        }

        let inserted_len = edit.inserted.chars().count();
        let removed_len = edit.range.end.saturating_sub(edit.range.start);
        let Some(next_delta) = edit_delta_after(delta, inserted_len, removed_len) else {
            return false;
        };
        let Some(next_len) = current_len
            .checked_sub(adjusted_end.saturating_sub(adjusted_start))
            .and_then(|len| len.checked_add(inserted_len))
        else {
            return false;
        };

        current_len = next_len;
        delta = next_delta;
        previous_end = edit.range.end;
    }

    true
}

pub(super) fn normalize_edits(edits: Vec<TextEdit>, len_chars: usize) -> Option<Vec<TextEdit>> {
    let mut filtered: Vec<TextEdit> = Vec::with_capacity(edits.len());
    for edit in edits {
        if !edit_range_is_valid(&edit.range, len_chars) || !inserted_text_is_bounded(&edit.inserted)
        {
            return None;
        }
        if edit.range.start != edit.range.end || !edit.inserted.is_empty() {
            filtered.push(edit);
        }
    }

    filtered.sort_by(|a, b| {
        a.range
            .start
            .cmp(&b.range.start)
            .then(a.range.end.cmp(&b.range.end))
    });

    let mut normalized: Vec<TextEdit> = Vec::with_capacity(filtered.len());
    for edit in filtered {
        let Some(last) = normalized.last_mut() else {
            normalized.push(edit);
            continue;
        };

        if last.range == edit.range && last.inserted == edit.inserted {
            continue;
        }

        if last.range.end > edit.range.start {
            if last.inserted.is_empty() && edit.inserted.is_empty() {
                last.range.end = last.range.end.max(edit.range.end);
            } else {
                return None;
            }
            continue;
        }

        normalized.push(edit);
    }

    if !edits_are_replayable(&normalized, len_chars) {
        return None;
    }

    Some(normalized)
}

pub(super) fn normalize_cursor_edits(
    edits: Vec<CursorEdit>,
    len_chars: usize,
) -> Option<Vec<CursorEdit>> {
    let mut filtered: Vec<CursorEdit> = Vec::with_capacity(edits.len());
    for cursor_edit in edits {
        if !edit_range_is_valid(&cursor_edit.edit.range, len_chars)
            || !inserted_text_is_bounded(&cursor_edit.edit.inserted)
        {
            return None;
        }
        if cursor_edit.edit.range.start != cursor_edit.edit.range.end
            || !cursor_edit.edit.inserted.is_empty()
        {
            let inserted_len = cursor_edit.edit.inserted.chars().count();
            filtered.push(CursorEdit {
                cursor_offset: cursor_edit.cursor_offset.min(inserted_len),
                ..cursor_edit
            });
        }
    }

    filtered.sort_by(|a, b| {
        a.edit
            .range
            .start
            .cmp(&b.edit.range.start)
            .then(a.edit.range.end.cmp(&b.edit.range.end))
    });

    let mut normalized: Vec<CursorEdit> = Vec::with_capacity(filtered.len());
    for edit in filtered {
        let Some(last) = normalized.last_mut() else {
            normalized.push(edit);
            continue;
        };

        if last.edit.range == edit.edit.range
            && last.edit.inserted == edit.edit.inserted
            && last.cursor_offset == edit.cursor_offset
        {
            continue;
        }

        if last.edit.range.end > edit.edit.range.start {
            if last.edit.inserted.is_empty() && edit.edit.inserted.is_empty() {
                last.edit.range.end = last.edit.range.end.max(edit.edit.range.end);
                last.cursor_offset = 0;
            } else {
                return None;
            }
            continue;
        }

        normalized.push(edit);
    }

    if !edits_are_replayable(
        normalized.iter().map(|cursor_edit| &cursor_edit.edit),
        len_chars,
    ) {
        return None;
    }

    Some(normalized)
}

pub(super) fn inserted_selections_after_edit(
    edits: &[TextEdit],
    primary_edit: &TextEdit,
    inserted_selections: &[Range<usize>],
    len_chars: usize,
) -> Option<Vec<Range<usize>>> {
    let mut delta = 0_isize;
    let mut current_len = len_chars;
    for edit in edits {
        let adjusted_start = adjust_index(edit.range.start, delta, current_len);
        let adjusted_end = adjust_index(edit.range.end, delta, current_len).max(adjusted_start);
        let inserted_len = edit.inserted.chars().count();
        if edit.range == primary_edit.range && edit.inserted == primary_edit.inserted {
            return Some(
                inserted_selections
                    .iter()
                    .map(|selection| {
                        let start = selection.start.min(inserted_len);
                        let end = selection.end.min(inserted_len).max(start);
                        adjusted_start.saturating_add(start)..adjusted_start.saturating_add(end)
                    })
                    .collect(),
            );
        }

        current_len = current_len
            .saturating_sub(adjusted_end.saturating_sub(adjusted_start))
            .saturating_add(inserted_len);
        delta += inserted_len as isize - edit.range.end.saturating_sub(edit.range.start) as isize;
    }
    None
}

pub(super) fn inserted_end_selections_after_edits(
    edits: &[TextEdit],
    len_chars: usize,
) -> Vec<Selection> {
    let mut delta = 0_isize;
    let mut current_len = len_chars;
    let mut selections = Vec::with_capacity(edits.len());
    for edit in edits {
        let adjusted_start = adjust_index(edit.range.start, delta, current_len);
        let adjusted_end = adjust_index(edit.range.end, delta, current_len).max(adjusted_start);
        let inserted_len = edit.inserted.chars().count();
        selections.push(Selection::caret(
            adjusted_start.saturating_add(inserted_len),
        ));
        current_len = current_len
            .saturating_sub(adjusted_end.saturating_sub(adjusted_start))
            .saturating_add(inserted_len);
        delta += inserted_len as isize - edit.range.end.saturating_sub(edit.range.start) as isize;
    }
    selections
}

pub(super) fn transform_selection_after_edits(
    selection: Selection,
    edits: &[TextEdit],
) -> Selection {
    if selection.is_caret() {
        let cursor = transform_position_after_edits(selection.cursor, edits, false);
        return Selection::caret(cursor);
    }

    let range = selection.range();
    Selection {
        anchor: transform_position_after_edits(
            selection.anchor,
            edits,
            selection.anchor == range.start,
        ),
        cursor: transform_position_after_edits(
            selection.cursor,
            edits,
            selection.cursor == range.start,
        ),
    }
}

pub(super) fn transform_position_after_edits(
    position: usize,
    edits: &[TextEdit],
    stick_before_insert_at_position: bool,
) -> usize {
    let mut delta = 0_isize;

    for edit in edits {
        let start = edit.range.start;
        let end = edit.range.end;
        let inserted_len = edit.inserted.chars().count();
        let removed_len = end.saturating_sub(start);

        if position < start {
            break;
        }

        if position == start && removed_len == 0 {
            if !stick_before_insert_at_position {
                delta += inserted_len as isize;
            }
            continue;
        }

        if position <= end {
            return apply_delta(start.saturating_add(inserted_len), delta);
        }

        delta += inserted_len as isize - removed_len as isize;
    }

    apply_delta(position, delta)
}

pub(super) fn transform_duplicate_position(
    position: usize,
    duplicates: &[LineDuplicateEdit],
    edits: &[TextEdit],
) -> usize {
    let mut delta = 0_isize;

    for duplicate in duplicates {
        let start = duplicate.source_range.start;
        let end = duplicate.source_range.end;
        let inserted_len = duplicate.edit.inserted.chars().count();
        if (start..=end).contains(&position) {
            let duplicated_position = end
                .saturating_add(duplicate.duplicate_start_offset)
                .saturating_add(position.saturating_sub(start));
            return apply_delta(duplicated_position, delta);
        }
        if position > end {
            delta += inserted_len as isize;
        }
    }

    transform_position_after_edits(position, edits, false)
}

pub(super) fn transform_line_move_position(
    buffer: &TextBuffer,
    position: usize,
    moves: &[LineMoveEdit],
    edits: &[TextEdit],
) -> usize {
    let mut delta = 0_isize;

    for line_move in moves {
        let block_range = buffer.line_block_char_range(line_move.block.clone());
        if (block_range.start..=block_range.end).contains(&position) {
            let (target_local_line, column) =
                if position == block_range.end && position != block_range.start {
                    (
                        line_move.moved_block_local_start.saturating_add(
                            line_move.block.end.saturating_sub(line_move.block.start),
                        ),
                        0,
                    )
                } else {
                    let position = buffer.char_position(position);
                    (
                        line_move
                            .moved_block_local_start
                            .saturating_add(position.line.saturating_sub(line_move.block.start)),
                        position.column,
                    )
                };
            let local_offset = joined_line_char_offset(
                &line_move.replacement_lines,
                target_local_line,
                column,
                line_move.trailing_newline,
            );
            return apply_delta(
                line_move.edit.range.start.saturating_add(local_offset),
                delta,
            );
        }

        if position > line_move.edit.range.end {
            let inserted_len = line_move.edit.inserted.chars().count();
            let removed_len = line_move
                .edit
                .range
                .end
                .saturating_sub(line_move.edit.range.start);
            delta += inserted_len as isize - removed_len as isize;
        }
    }

    transform_position_after_edits(position, edits, false)
}

pub(super) fn join_line_contents(lines: &[String], trailing_newline: bool) -> String {
    let byte_len = lines.iter().map(String::len).sum::<usize>()
        + lines.len().saturating_sub(1)
        + usize::from(trailing_newline);
    let mut text = String::with_capacity(byte_len);
    for (idx, line) in lines.iter().enumerate() {
        if idx > 0 {
            text.push('\n');
        }
        text.push_str(line);
    }
    if trailing_newline {
        text.push('\n');
    }
    text
}

pub(super) fn joined_line_char_count(lines: &[String], trailing_newline: bool) -> usize {
    lines
        .iter()
        .fold(0usize, |count, line| {
            count.saturating_add(line.chars().count())
        })
        .saturating_add(lines.len().saturating_sub(1))
        .saturating_add(usize::from(trailing_newline))
}

pub(super) fn joined_line_char_offset(
    lines: &[String],
    line_idx: usize,
    column: usize,
    trailing_newline: bool,
) -> usize {
    if lines.is_empty() {
        return 0;
    }
    if line_idx >= lines.len() {
        return joined_line_char_count(lines, trailing_newline);
    }

    let mut offset = 0usize;
    for line in lines.iter().take(line_idx) {
        offset = offset
            .saturating_add(line.chars().count())
            .saturating_add(1);
    }
    offset.saturating_add(column.min(lines[line_idx].chars().count()))
}

pub(super) fn adjust_index(index: usize, delta: isize, current_len: usize) -> usize {
    if delta.is_negative() {
        index.saturating_sub(delta.unsigned_abs()).min(current_len)
    } else {
        index.saturating_add(delta as usize).min(current_len)
    }
}

pub(super) fn apply_delta(index: usize, delta: isize) -> usize {
    if delta.is_negative() {
        index.saturating_sub(delta.unsigned_abs())
    } else {
        index.saturating_add(delta as usize)
    }
}
