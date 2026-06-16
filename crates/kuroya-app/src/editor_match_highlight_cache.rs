use crate::editor_pane_data::{
    occurrence_highlight_ranges_for_buffer, selection_highlight_ranges_for_buffer,
};
use crate::large_file_mode::buffer_uses_large_file_mode;
use kuroya_core::{BufferId, EditorOccurrencesHighlight, Selection, TextBuffer};
use std::{collections::VecDeque, ops::Range};

const MATCH_HIGHLIGHT_CACHE_CAPACITY: usize = 32;
const MAX_MATCH_HIGHLIGHT_RANGES: usize = 1_000;

#[derive(Debug, Default)]
pub(crate) struct EditorMatchHighlightCache {
    occurrence: VecDeque<OccurrenceHighlightCacheEntry>,
    selection: VecDeque<SelectionHighlightCacheEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OccurrenceHighlightCacheKey {
    buffer_id: BufferId,
    version: u64,
    len_chars: usize,
    cursor: usize,
    word_separators: String,
    mode: EditorOccurrencesHighlight,
}

#[derive(Debug, Clone)]
struct OccurrenceHighlightCacheEntry {
    key: OccurrenceHighlightCacheKey,
    ranges: Vec<Range<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SelectionHighlightCacheKey {
    buffer_id: BufferId,
    version: u64,
    len_chars: usize,
    selection_range: Range<usize>,
    max_length: usize,
    multiline: bool,
}

#[derive(Debug, Clone)]
struct SelectionHighlightCacheEntry {
    key: SelectionHighlightCacheKey,
    ranges: Vec<Range<usize>>,
}

impl EditorMatchHighlightCache {
    pub(crate) fn occurrence_highlight_ranges(
        &mut self,
        buffer: &TextBuffer,
        mode: EditorOccurrencesHighlight,
    ) -> Vec<Range<usize>> {
        let buffer_id = buffer.id();
        if buffer_uses_large_file_mode(buffer) {
            self.clear_for_buffer(buffer_id);
            return Vec::new();
        }

        if !mode.shows_current_file() {
            clear_occurrence_for_buffer(&mut self.occurrence, buffer_id);
            return Vec::new();
        }

        let version = buffer.version();
        let len_chars = buffer.len_chars();
        let word_separators = buffer.word_separators();
        retain_current_occurrence_inputs(
            &mut self.occurrence,
            buffer_id,
            version,
            len_chars,
            word_separators,
            mode,
        );

        let Some(cursor) = caret_cursor_key(buffer.selections(), len_chars) else {
            return Vec::new();
        };
        if let Some(ranges) = lookup_occurrence(
            &mut self.occurrence,
            buffer_id,
            version,
            len_chars,
            cursor,
            word_separators,
            mode,
        ) {
            return ranges;
        }

        let key = OccurrenceHighlightCacheKey {
            buffer_id,
            version,
            len_chars,
            cursor,
            word_separators: word_separators.to_owned(),
            mode,
        };

        let mut ranges = occurrence_highlight_ranges_for_buffer(buffer, mode);
        retain_valid_highlight_ranges(&mut ranges, len_chars);
        push_occurrence(&mut self.occurrence, key, ranges.clone());
        ranges
    }

    pub(crate) fn selection_highlight_ranges(
        &mut self,
        buffer: &TextBuffer,
        enabled: bool,
        max_length: usize,
        multiline: bool,
    ) -> Vec<Range<usize>> {
        let buffer_id = buffer.id();
        if buffer_uses_large_file_mode(buffer) {
            self.clear_for_buffer(buffer_id);
            return Vec::new();
        }

        if !enabled {
            clear_selection_for_buffer(&mut self.selection, buffer_id);
            return Vec::new();
        }

        let version = buffer.version();
        let len_chars = buffer.len_chars();
        retain_current_selection_inputs(
            &mut self.selection,
            buffer_id,
            version,
            len_chars,
            max_length,
            multiline,
        );

        let Some(selection_range) = active_selection_range(buffer.selections(), len_chars) else {
            return Vec::new();
        };
        if max_length > 0 && selection_range.end.saturating_sub(selection_range.start) > max_length
        {
            return Vec::new();
        }

        let key = SelectionHighlightCacheKey {
            buffer_id,
            version,
            len_chars,
            selection_range,
            max_length,
            multiline,
        };
        if let Some(ranges) = lookup_selection(&mut self.selection, &key) {
            return ranges;
        }

        let mut ranges =
            selection_highlight_ranges_for_buffer(buffer, enabled, max_length, multiline);
        retain_valid_highlight_ranges(&mut ranges, len_chars);
        push_selection(&mut self.selection, key, ranges.clone());
        ranges
    }

    pub(crate) fn clear_for_buffer(&mut self, buffer_id: BufferId) {
        clear_occurrence_for_buffer(&mut self.occurrence, buffer_id);
        clear_selection_for_buffer(&mut self.selection, buffer_id);
    }

    pub(crate) fn clear(&mut self) {
        self.occurrence.clear();
        self.selection.clear();
    }

    #[cfg(test)]
    pub(crate) fn contains_buffer_for_test(&self, buffer_id: BufferId) -> bool {
        self.occurrence
            .iter()
            .any(|entry| entry.key.buffer_id == buffer_id)
            || self
                .selection
                .iter()
                .any(|entry| entry.key.buffer_id == buffer_id)
    }
}

fn caret_cursor_key(selections: &[Selection], len_chars: usize) -> Option<usize> {
    if selections.iter().any(|selection| !selection.is_caret()) {
        return None;
    }
    Some(selections.last()?.cursor.min(len_chars))
}

fn active_selection_range(selections: &[Selection], len_chars: usize) -> Option<Range<usize>> {
    let range = selections
        .iter()
        .rev()
        .map(|selection| selection.range())
        .find(|range| range.start != range.end)?;
    (range.end <= len_chars).then_some(range)
}

fn clear_occurrence_for_buffer(
    entries: &mut VecDeque<OccurrenceHighlightCacheEntry>,
    buffer_id: BufferId,
) {
    entries.retain(|entry| entry.key.buffer_id != buffer_id);
}

fn clear_selection_for_buffer(
    entries: &mut VecDeque<SelectionHighlightCacheEntry>,
    buffer_id: BufferId,
) {
    entries.retain(|entry| entry.key.buffer_id != buffer_id);
}

fn retain_current_occurrence_inputs(
    entries: &mut VecDeque<OccurrenceHighlightCacheEntry>,
    buffer_id: BufferId,
    version: u64,
    len_chars: usize,
    word_separators: &str,
    mode: EditorOccurrencesHighlight,
) {
    entries.retain(|entry| {
        entry.key.buffer_id != buffer_id
            || (entry.key.version == version
                && entry.key.len_chars == len_chars
                && entry.key.word_separators.as_str() == word_separators
                && entry.key.mode == mode
                && highlight_ranges_fit_buffer(&entry.ranges, len_chars))
    });
}

fn retain_current_selection_inputs(
    entries: &mut VecDeque<SelectionHighlightCacheEntry>,
    buffer_id: BufferId,
    version: u64,
    len_chars: usize,
    max_length: usize,
    multiline: bool,
) {
    entries.retain(|entry| {
        entry.key.buffer_id != buffer_id
            || (entry.key.version == version
                && entry.key.len_chars == len_chars
                && entry.key.max_length == max_length
                && entry.key.multiline == multiline
                && highlight_ranges_fit_buffer(&entry.ranges, len_chars))
    });
}

fn lookup_occurrence(
    entries: &mut VecDeque<OccurrenceHighlightCacheEntry>,
    buffer_id: BufferId,
    version: u64,
    len_chars: usize,
    cursor: usize,
    word_separators: &str,
    mode: EditorOccurrencesHighlight,
) -> Option<Vec<Range<usize>>> {
    let index = entries.iter().rposition(|entry| {
        entry.key.buffer_id == buffer_id
            && entry.key.version == version
            && entry.key.len_chars == len_chars
            && entry.key.cursor == cursor
            && entry.key.word_separators.as_str() == word_separators
            && entry.key.mode == mode
    })?;
    if entries
        .get(index)
        .is_some_and(|entry| !highlight_ranges_fit_buffer(&entry.ranges, len_chars))
    {
        entries.remove(index);
        return None;
    }
    if index + 1 == entries.len() {
        return entries.get(index).map(|entry| entry.ranges.clone());
    }
    let entry = entries.remove(index)?;
    let ranges = entry.ranges.clone();
    entries.push_back(entry);
    Some(ranges)
}

fn push_occurrence(
    entries: &mut VecDeque<OccurrenceHighlightCacheEntry>,
    key: OccurrenceHighlightCacheKey,
    ranges: Vec<Range<usize>>,
) {
    if entries.len() >= MATCH_HIGHLIGHT_CACHE_CAPACITY {
        entries.pop_front();
    }
    entries.push_back(OccurrenceHighlightCacheEntry { key, ranges });
}

fn lookup_selection(
    entries: &mut VecDeque<SelectionHighlightCacheEntry>,
    key: &SelectionHighlightCacheKey,
) -> Option<Vec<Range<usize>>> {
    let index = entries.iter().rposition(|entry| entry.key == *key)?;
    if entries
        .get(index)
        .is_some_and(|entry| !highlight_ranges_fit_buffer(&entry.ranges, key.len_chars))
    {
        entries.remove(index);
        return None;
    }
    if index + 1 == entries.len() {
        return entries.get(index).map(|entry| entry.ranges.clone());
    }
    let entry = entries.remove(index)?;
    let ranges = entry.ranges.clone();
    entries.push_back(entry);
    Some(ranges)
}

fn push_selection(
    entries: &mut VecDeque<SelectionHighlightCacheEntry>,
    key: SelectionHighlightCacheKey,
    ranges: Vec<Range<usize>>,
) {
    if entries.len() >= MATCH_HIGHLIGHT_CACHE_CAPACITY {
        entries.pop_front();
    }
    entries.push_back(SelectionHighlightCacheEntry { key, ranges });
}

fn retain_valid_highlight_ranges(ranges: &mut Vec<Range<usize>>, len_chars: usize) {
    ranges.retain(|range| highlight_range_fits_buffer(range, len_chars));
    ranges.truncate(MAX_MATCH_HIGHLIGHT_RANGES);
}

fn highlight_ranges_fit_buffer(ranges: &[Range<usize>], len_chars: usize) -> bool {
    ranges.len() <= MAX_MATCH_HIGHLIGHT_RANGES
        && ranges
            .iter()
            .all(|range| highlight_range_fits_buffer(range, len_chars))
}

fn highlight_range_fits_buffer(range: &Range<usize>, len_chars: usize) -> bool {
    range.start < range.end && range.end <= len_chars
}

#[cfg(test)]
mod tests {
    use super::{
        EditorMatchHighlightCache, MAX_MATCH_HIGHLIGHT_RANGES, OccurrenceHighlightCacheEntry,
        OccurrenceHighlightCacheKey, SelectionHighlightCacheEntry, SelectionHighlightCacheKey,
        highlight_ranges_fit_buffer, lookup_occurrence, lookup_selection,
        retain_valid_highlight_ranges,
    };
    use crate::large_file_mode::LARGE_FILE_MODE_MAX_LINES;
    use kuroya_core::{EditorOccurrencesHighlight, Selection, TextBuffer};
    use std::ops::Range;

    fn ranges(range: Range<usize>) -> Vec<Range<usize>> {
        std::iter::once(range).collect()
    }
    use std::collections::VecDeque;

    #[test]
    fn occurrence_cache_tracks_cursor_and_word_separators() {
        let mut cache = EditorMatchHighlightCache::default();
        let mut buffer = TextBuffer::from_text(
            7,
            None,
            "alpha beta alpha\nfoo.bar foo.bar foo\n".to_owned(),
        );
        buffer.set_single_cursor(2);

        assert_eq!(
            cache.occurrence_highlight_ranges(&buffer, EditorOccurrencesHighlight::SingleFile),
            vec![0..5, 11..16]
        );

        buffer.set_single_cursor(6);
        assert_eq!(
            cache.occurrence_highlight_ranges(&buffer, EditorOccurrencesHighlight::SingleFile),
            vec![6..10]
        );

        buffer.set_single_cursor(17);
        assert_eq!(
            cache.occurrence_highlight_ranges(&buffer, EditorOccurrencesHighlight::SingleFile),
            vec![17..20, 25..28, 33..36]
        );

        buffer.set_word_separators("");
        assert_eq!(
            cache.occurrence_highlight_ranges(&buffer, EditorOccurrencesHighlight::SingleFile),
            vec![17..24, 25..32]
        );
        assert_eq!(cache.occurrence.len(), 1);
    }

    #[test]
    fn selection_cache_tracks_selection_and_settings() {
        let mut cache = EditorMatchHighlightCache::default();
        let mut buffer =
            TextBuffer::from_text(8, None, "alpha beta alpha\nalpha beta alpha\n".to_owned());
        buffer.set_selections([Selection {
            anchor: 0,
            cursor: 5,
        }]);

        assert_eq!(
            cache.selection_highlight_ranges(&buffer, true, 200, false),
            vec![11..16, 17..22, 28..33]
        );
        assert_eq!(cache.selection.len(), 1);
        assert!(
            cache
                .selection_highlight_ranges(&buffer, true, 4, false)
                .is_empty()
        );

        buffer.set_selections([Selection {
            anchor: 6,
            cursor: 10,
        }]);
        assert_eq!(
            cache.selection_highlight_ranges(&buffer, true, 200, false),
            vec![23..27]
        );

        buffer.set_selections([Selection {
            anchor: 0,
            cursor: 17,
        }]);
        assert!(
            cache
                .selection_highlight_ranges(&buffer, true, 200, false)
                .is_empty()
        );
        assert_eq!(
            cache.selection_highlight_ranges(&buffer, true, 200, true),
            vec![17..34]
        );
    }

    #[test]
    fn match_highlight_cache_prunes_stale_versions_for_current_buffer() {
        let mut cache = EditorMatchHighlightCache::default();
        let mut buffer = TextBuffer::from_text(9, None, "alpha beta alpha\n".to_owned());
        let mut other = TextBuffer::from_text(10, None, "gamma gamma\n".to_owned());

        buffer.set_single_cursor(2);
        other.set_single_cursor(2);
        cache.occurrence_highlight_ranges(&buffer, EditorOccurrencesHighlight::SingleFile);
        cache.occurrence_highlight_ranges(&other, EditorOccurrencesHighlight::SingleFile);

        buffer.set_selections([Selection {
            anchor: 0,
            cursor: 5,
        }]);
        cache.selection_highlight_ranges(&buffer, true, 200, false);
        let stale_version = buffer.version();

        buffer.set_single_cursor(buffer.len_chars());
        buffer.insert_at_cursor("alpha\n");
        let current_version = buffer.version();
        assert_ne!(stale_version, current_version);

        cache.occurrence_highlight_ranges(&buffer, EditorOccurrencesHighlight::SingleFile);
        buffer.set_selections([Selection {
            anchor: 0,
            cursor: 5,
        }]);
        cache.selection_highlight_ranges(&buffer, true, 200, false);

        assert!(cache.occurrence.iter().all(|entry| {
            entry.key.buffer_id != buffer.id() || entry.key.version == current_version
        }));
        assert!(cache.selection.iter().all(|entry| {
            entry.key.buffer_id != buffer.id() || entry.key.version == current_version
        }));
        assert!(
            cache
                .occurrence
                .iter()
                .any(|entry| entry.key.buffer_id == other.id())
        );
    }

    #[test]
    fn disabled_match_highlight_modes_do_not_cache_empty_results() {
        let mut cache = EditorMatchHighlightCache::default();
        let mut buffer = TextBuffer::from_text(11, None, "alpha alpha\n".to_owned());
        buffer.set_single_cursor(2);

        assert_eq!(
            cache.occurrence_highlight_ranges(&buffer, EditorOccurrencesHighlight::SingleFile),
            vec![0..5, 6..11]
        );
        assert!(!cache.occurrence.is_empty());
        assert!(
            cache
                .occurrence_highlight_ranges(&buffer, EditorOccurrencesHighlight::Off)
                .is_empty()
        );
        assert!(cache.occurrence.is_empty());

        buffer.set_selections([Selection {
            anchor: 0,
            cursor: 5,
        }]);
        assert_eq!(
            cache.selection_highlight_ranges(&buffer, true, 200, false),
            vec![6..11]
        );
        assert!(!cache.selection.is_empty());
        assert!(
            cache
                .selection_highlight_ranges(&buffer, false, 200, false)
                .is_empty()
        );
        assert!(cache.selection.is_empty());
    }

    #[test]
    fn match_highlight_cache_clears_large_buffers_without_scanning() {
        let mut cache = EditorMatchHighlightCache::default();
        let mut buffer = TextBuffer::from_text(19, None, "alpha alpha\n".to_owned());
        buffer.set_single_cursor(2);
        cache.occurrence_highlight_ranges(&buffer, EditorOccurrencesHighlight::SingleFile);
        buffer.set_selections([Selection {
            anchor: 0,
            cursor: 5,
        }]);
        cache.selection_highlight_ranges(&buffer, true, 200, false);
        assert!(cache.contains_buffer_for_test(19));

        let large_text = "alpha\n".repeat(LARGE_FILE_MODE_MAX_LINES + 1);
        let len_chars = buffer.len_chars();
        assert!(buffer.replace_range(0..len_chars, &large_text));
        buffer.set_single_cursor(2);

        assert!(
            cache
                .occurrence_highlight_ranges(&buffer, EditorOccurrencesHighlight::SingleFile)
                .is_empty()
        );
        buffer.set_selections([Selection {
            anchor: 0,
            cursor: 5,
        }]);
        assert!(
            cache
                .selection_highlight_ranges(&buffer, true, 200, false)
                .is_empty()
        );
        assert!(!cache.contains_buffer_for_test(19));
    }

    #[test]
    fn occurrence_cache_skips_non_caret_selection_states() {
        let mut cache = EditorMatchHighlightCache::default();
        let mut buffer = TextBuffer::from_text(12, None, "alpha alpha\n".to_owned());
        buffer.set_selections([Selection {
            anchor: 0,
            cursor: 5,
        }]);

        assert!(
            cache
                .occurrence_highlight_ranges(&buffer, EditorOccurrencesHighlight::SingleFile)
                .is_empty()
        );
        assert!(cache.occurrence.is_empty());
    }

    #[test]
    fn occurrence_cache_keys_by_primary_caret_without_caret_vector_allocation() {
        let mut cache = EditorMatchHighlightCache::default();
        let mut buffer = TextBuffer::from_text(13, None, "alpha beta alpha beta\n".to_owned());
        buffer.set_selections([Selection::caret(0), Selection::caret(6)]);

        assert_eq!(
            cache.occurrence_highlight_ranges(&buffer, EditorOccurrencesHighlight::SingleFile),
            vec![6..10, 17..21]
        );

        buffer.set_selections([Selection::caret(2), Selection::caret(6)]);
        assert_eq!(
            cache.occurrence_highlight_ranges(&buffer, EditorOccurrencesHighlight::SingleFile),
            vec![6..10, 17..21]
        );
        assert_eq!(cache.occurrence.len(), 1);
    }

    #[test]
    fn selection_cache_uses_active_selection_range_and_skips_no_selection_state() {
        let mut cache = EditorMatchHighlightCache::default();
        let mut buffer = TextBuffer::from_text(14, None, "alpha beta alpha beta\n".to_owned());

        buffer.set_selections([Selection::caret(0)]);
        assert!(
            cache
                .selection_highlight_ranges(&buffer, true, 200, false)
                .is_empty()
        );
        assert!(cache.selection.is_empty());

        buffer.set_selections([
            Selection::caret(0),
            Selection {
                anchor: 6,
                cursor: 10,
            },
        ]);
        assert_eq!(
            cache.selection_highlight_ranges(&buffer, true, 200, false),
            vec![17..21]
        );

        buffer.set_selections([
            Selection::caret(12),
            Selection {
                anchor: 6,
                cursor: 10,
            },
        ]);
        assert_eq!(
            cache.selection_highlight_ranges(&buffer, true, 200, false),
            vec![17..21]
        );
        assert_eq!(cache.selection.len(), 1);
    }

    #[test]
    fn occurrence_cache_reuses_and_promotes_cursor_entries() {
        let mut cache = EditorMatchHighlightCache::default();
        let mut buffer = TextBuffer::from_text(15, None, "alpha beta alpha beta\n".to_owned());

        buffer.set_single_cursor(2);
        assert_eq!(
            cache.occurrence_highlight_ranges(&buffer, EditorOccurrencesHighlight::SingleFile),
            vec![0..5, 11..16]
        );
        buffer.set_single_cursor(7);
        assert_eq!(
            cache.occurrence_highlight_ranges(&buffer, EditorOccurrencesHighlight::SingleFile),
            vec![6..10, 17..21]
        );
        assert_eq!(cache.occurrence.len(), 2);

        buffer.set_single_cursor(2);
        assert_eq!(
            cache.occurrence_highlight_ranges(&buffer, EditorOccurrencesHighlight::SingleFile),
            vec![0..5, 11..16]
        );
        assert_eq!(cache.occurrence.len(), 2);
        assert_eq!(
            cache.occurrence.back().map(|entry| entry.key.cursor),
            Some(2)
        );
    }

    #[test]
    fn occurrence_lookup_uses_newest_matching_entry_and_promotes_it() {
        let buffer_id = 21;
        let version = 1;
        let len_chars = 24;
        let word_separators = ".";
        let key = OccurrenceHighlightCacheKey {
            buffer_id,
            version,
            len_chars,
            cursor: 2,
            word_separators: word_separators.to_owned(),
            mode: EditorOccurrencesHighlight::SingleFile,
        };
        let mut other_key = key.clone();
        other_key.cursor = 7;
        let mut entries = VecDeque::from([
            OccurrenceHighlightCacheEntry {
                key: key.clone(),
                ranges: ranges(0..5),
            },
            OccurrenceHighlightCacheEntry {
                key: key.clone(),
                ranges: ranges(11..16),
            },
            OccurrenceHighlightCacheEntry {
                key: other_key,
                ranges: vec![6..10, 17..21],
            },
        ]);

        assert_eq!(
            lookup_occurrence(
                &mut entries,
                buffer_id,
                version,
                len_chars,
                2,
                word_separators,
                EditorOccurrencesHighlight::SingleFile,
            ),
            Some(ranges(11..16))
        );

        assert_eq!(entries.len(), 3);
        assert_eq!(
            entries.back().map(|entry| entry.ranges.clone()),
            Some(ranges(11..16))
        );
    }

    #[test]
    fn selection_lookup_uses_newest_matching_entry_and_promotes_it() {
        let key = SelectionHighlightCacheKey {
            buffer_id: 22,
            version: 1,
            len_chars: 30,
            selection_range: 0..5,
            max_length: 200,
            multiline: false,
        };
        let mut other_key = key.clone();
        other_key.selection_range = 6..10;
        let mut entries = VecDeque::from([
            SelectionHighlightCacheEntry {
                key: key.clone(),
                ranges: ranges(6..11),
            },
            SelectionHighlightCacheEntry {
                key: key.clone(),
                ranges: ranges(17..22),
            },
            SelectionHighlightCacheEntry {
                key: other_key,
                ranges: ranges(0..5),
            },
        ]);

        assert_eq!(lookup_selection(&mut entries, &key), Some(ranges(17..22)));

        assert_eq!(entries.len(), 3);
        assert_eq!(
            entries.back().map(|entry| entry.ranges.clone()),
            Some(ranges(17..22))
        );
    }

    #[test]
    fn match_highlight_lookup_prunes_out_of_bounds_matching_entries() {
        let buffer_id = 23;
        let version = 1;
        let len_chars = 12;
        let word_separators = ".";
        let mut occurrence_entries = VecDeque::from([OccurrenceHighlightCacheEntry {
            key: OccurrenceHighlightCacheKey {
                buffer_id,
                version,
                len_chars,
                cursor: 2,
                word_separators: word_separators.to_owned(),
                mode: EditorOccurrencesHighlight::SingleFile,
            },
            ranges: vec![0..5, 20..25],
        }]);
        assert_eq!(
            lookup_occurrence(
                &mut occurrence_entries,
                buffer_id,
                version,
                len_chars,
                2,
                word_separators,
                EditorOccurrencesHighlight::SingleFile,
            ),
            None
        );
        assert!(occurrence_entries.is_empty());

        let selection_key = SelectionHighlightCacheKey {
            buffer_id,
            version,
            len_chars,
            selection_range: 0..5,
            max_length: 200,
            multiline: false,
        };
        let mut selection_entries = VecDeque::from([SelectionHighlightCacheEntry {
            key: selection_key.clone(),
            ranges: vec![6..11, 20..25],
        }]);
        assert_eq!(
            lookup_selection(&mut selection_entries, &selection_key),
            None
        );
        assert!(selection_entries.is_empty());
    }

    #[test]
    fn selection_cache_prunes_entries_when_settings_change() {
        let mut cache = EditorMatchHighlightCache::default();
        let mut buffer =
            TextBuffer::from_text(16, None, "alpha beta alpha\nalpha beta alpha\n".to_owned());
        buffer.set_selections([Selection {
            anchor: 0,
            cursor: 17,
        }]);

        assert_eq!(
            cache.selection_highlight_ranges(&buffer, true, 200, true),
            vec![17..34]
        );
        assert_eq!(cache.selection.len(), 1);
        assert!(
            cache
                .selection_highlight_ranges(&buffer, true, 200, false)
                .is_empty()
        );
        assert_eq!(cache.selection.len(), 1);
        assert!(cache.selection.iter().all(|entry| !entry.key.multiline));
        assert!(cache.selection.iter().all(|entry| entry.ranges.is_empty()));
    }

    #[test]
    fn occurrence_cache_drops_out_of_bounds_ranges_before_reuse() {
        let mut cache = EditorMatchHighlightCache::default();
        let mut buffer = TextBuffer::from_text(17, None, "alpha alpha\n".to_owned());
        buffer.set_single_cursor(2);

        cache.occurrence.push_back(OccurrenceHighlightCacheEntry {
            key: OccurrenceHighlightCacheKey {
                buffer_id: buffer.id(),
                version: buffer.version(),
                len_chars: buffer.len_chars(),
                cursor: 2,
                word_separators: buffer.word_separators().to_owned(),
                mode: EditorOccurrencesHighlight::SingleFile,
            },
            ranges: vec![0..5, 30..35],
        });

        assert_eq!(
            cache.occurrence_highlight_ranges(&buffer, EditorOccurrencesHighlight::SingleFile),
            vec![0..5, 6..11]
        );
        assert_eq!(cache.occurrence.len(), 1);
        assert_eq!(
            cache.occurrence.back().map(|entry| entry.ranges.clone()),
            Some(vec![0..5, 6..11])
        );
    }

    #[test]
    fn selection_cache_drops_entries_for_stale_buffer_bounds() {
        let mut cache = EditorMatchHighlightCache::default();
        let mut buffer = TextBuffer::from_text(18, None, "alpha alpha\n".to_owned());
        buffer.set_selections([Selection {
            anchor: 0,
            cursor: 5,
        }]);

        cache.selection.push_back(SelectionHighlightCacheEntry {
            key: SelectionHighlightCacheKey {
                buffer_id: buffer.id(),
                version: buffer.version(),
                len_chars: buffer.len_chars() + 1,
                selection_range: 0..5,
                max_length: 200,
                multiline: false,
            },
            ranges: std::iter::once(6..11).collect(),
        });

        assert_eq!(
            cache.selection_highlight_ranges(&buffer, true, 200, false),
            std::iter::once(6..11).collect::<Vec<_>>()
        );
        assert_eq!(cache.selection.len(), 1);
        assert_eq!(
            cache.selection.back().map(|entry| entry.key.len_chars),
            Some(buffer.len_chars())
        );
    }

    #[test]
    fn highlight_range_normalization_bounds_payload_without_rebasing_ranges() {
        let mut ranges = (0..MAX_MATCH_HIGHLIGHT_RANGES + 5)
            .map(|idx| idx * 2..idx * 2 + 1)
            .collect::<Vec<_>>();
        ranges.push(usize::MAX - 1..usize::MAX);

        retain_valid_highlight_ranges(&mut ranges, (MAX_MATCH_HIGHLIGHT_RANGES + 10) * 2);

        assert_eq!(ranges.len(), MAX_MATCH_HIGHLIGHT_RANGES);
        assert_eq!(ranges.first(), Some(&(0..1)));
        assert_eq!(
            ranges.last(),
            Some(&((MAX_MATCH_HIGHLIGHT_RANGES - 1) * 2..(MAX_MATCH_HIGHLIGHT_RANGES - 1) * 2 + 1))
        );
        assert!(highlight_ranges_fit_buffer(
            &ranges,
            (MAX_MATCH_HIGHLIGHT_RANGES + 10) * 2
        ));
    }
}
