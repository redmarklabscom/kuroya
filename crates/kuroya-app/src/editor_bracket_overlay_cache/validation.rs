use super::{
    BracketColorOverlayCacheEntry, BracketMatchOverlayCacheEntry, BracketPairGuideOverlayCacheEntry,
};
use kuroya_core::{
    BufferId, LanguageId, TextBuffer,
    buffer::{BracketColor, BracketPairGuide, Selection},
};
use std::{collections::VecDeque, ops::Range};

pub(super) fn ordered_cursor_indices(selections: &[Selection], len_chars: usize) -> Vec<usize> {
    match selections {
        [] => return Vec::new(),
        [selection] => return vec![selection.cursor.min(len_chars)],
        _ => {}
    }

    let mut indices = selections
        .iter()
        .map(|selection| selection.cursor.min(len_chars))
        .collect::<Vec<_>>();
    indices.sort_unstable();
    indices.dedup();
    indices
}

pub(super) fn normalized_line_count(
    buffer: &TextBuffer,
    first_line: usize,
    line_count: usize,
) -> Option<usize> {
    if line_count == 0 || first_line >= buffer.len_lines() {
        return None;
    }
    let end_line = first_line
        .saturating_add(line_count)
        .min(buffer.len_lines());
    Some(end_line.saturating_sub(first_line))
}

pub(super) fn line_char_range(
    buffer: &TextBuffer,
    first_line: usize,
    line_count: usize,
) -> Option<Range<usize>> {
    if line_count == 0 || first_line >= buffer.len_lines() {
        return None;
    }
    let start = buffer.line_column_to_char(first_line, 0);
    let next_line = first_line.saturating_add(line_count);
    let end = if next_line < buffer.len_lines() {
        buffer.line_column_to_char(next_line, 0)
    } else {
        buffer.len_chars()
    };
    Some(start..end)
}

pub(super) fn retain_current_color_state(
    entries: &mut VecDeque<BracketColorOverlayCacheEntry>,
    buffer_id: BufferId,
    version: u64,
    len_chars: usize,
    language: LanguageId,
) {
    entries.retain(|entry| {
        entry.key.buffer_id != buffer_id
            || (entry.key.version == version
                && entry.key.len_chars == len_chars
                && entry.key.language == language
                && bracket_color_entry_fits_buffer(entry, len_chars))
    });
}

pub(super) fn retain_current_guide_state(
    entries: &mut VecDeque<BracketPairGuideOverlayCacheEntry>,
    buffer_id: BufferId,
    version: u64,
    len_chars: usize,
    language: LanguageId,
) {
    entries.retain(|entry| {
        entry.key.buffer_id != buffer_id
            || (entry.key.version == version
                && entry.key.len_chars == len_chars
                && entry.key.language == language
                && bracket_pair_guides_fit_buffer(&entry.guides, len_chars))
    });
}

pub(super) fn retain_current_match_state(
    entries: &mut VecDeque<BracketMatchOverlayCacheEntry>,
    buffer_id: BufferId,
    version: u64,
    len_chars: usize,
    language: LanguageId,
) {
    entries.retain(|entry| {
        entry.key.buffer_id != buffer_id
            || (entry.key.version == version
                && entry.key.len_chars == len_chars
                && entry.key.language == language
                && bracket_matches_fit_buffer(&entry.matches, len_chars))
    });
}

pub(super) fn bracket_color_entry_fits_buffer(
    entry: &BracketColorOverlayCacheEntry,
    len_chars: usize,
) -> bool {
    char_range_fits_buffer(&entry.char_range, len_chars)
        && entry.colors.iter().all(|color| {
            color.char_idx >= entry.char_range.start && color.char_idx < entry.char_range.end
        })
}

pub(super) fn retain_valid_bracket_colors(
    colors: &mut Vec<BracketColor>,
    char_range: &Range<usize>,
) {
    colors.retain(|color| color.char_idx >= char_range.start && color.char_idx < char_range.end);
}

pub(super) fn char_range_fits_buffer(range: &Range<usize>, len_chars: usize) -> bool {
    range.start <= range.end && range.end <= len_chars
}

pub(super) fn retain_valid_bracket_pair_guides(
    guides: &mut Vec<BracketPairGuide>,
    len_chars: usize,
) {
    guides.retain(|guide| bracket_pair_guide_fits_buffer(guide, len_chars));
}

pub(super) fn bracket_pair_guides_fit_buffer(
    guides: &[BracketPairGuide],
    len_chars: usize,
) -> bool {
    guides
        .iter()
        .all(|guide| bracket_pair_guide_fits_buffer(guide, len_chars))
}

pub(super) fn bracket_pair_guide_fits_buffer(guide: &BracketPairGuide, len_chars: usize) -> bool {
    guide.open_idx < guide.close_idx && guide.close_idx < len_chars
}

pub(super) fn retain_valid_bracket_matches(matches: &mut Vec<(usize, usize)>, len_chars: usize) {
    matches.retain(|pair| bracket_match_fits_buffer(*pair, len_chars));
}

pub(super) fn bracket_matches_fit_buffer(matches: &[(usize, usize)], len_chars: usize) -> bool {
    matches
        .iter()
        .all(|pair| bracket_match_fits_buffer(*pair, len_chars))
}

pub(super) fn bracket_match_fits_buffer((left, right): (usize, usize), len_chars: usize) -> bool {
    left != right && left < len_chars && right < len_chars
}
