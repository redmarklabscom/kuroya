use crate::{
    large_file_mode::buffer_needs_bracket_scan_protection,
    syntax_cache::MAX_VISIBLE_LAYOUT_ROWS_PER_RANGE,
};
use kuroya_core::{
    BufferId, EditorMatchBrackets, LanguageId, TextBuffer,
    buffer::{BracketColor, BracketPairGuide},
};
use std::{collections::VecDeque, ops::Range};

mod validation;

use self::validation::{
    bracket_color_entry_fits_buffer, bracket_matches_fit_buffer, bracket_pair_guides_fit_buffer,
    line_char_range, normalized_line_count, ordered_cursor_indices, retain_current_color_state,
    retain_current_guide_state, retain_current_match_state, retain_valid_bracket_colors,
    retain_valid_bracket_matches, retain_valid_bracket_pair_guides,
};

const BRACKET_COLOR_CACHE_CAPACITY: usize = 32;
const BRACKET_PAIR_GUIDE_CACHE_CAPACITY: usize = 16;
const BRACKET_MATCH_CACHE_CAPACITY: usize = 16;
const MAX_BRACKET_COLOR_LINES_PER_REQUEST: usize = MAX_VISIBLE_LAYOUT_ROWS_PER_RANGE;

#[derive(Debug, Default)]
pub(crate) struct EditorBracketOverlayCache {
    colors: VecDeque<BracketColorOverlayCacheEntry>,
    guides: VecDeque<BracketPairGuideOverlayCacheEntry>,
    matches: VecDeque<BracketMatchOverlayCacheEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BracketColorOverlayCacheKey {
    buffer_id: BufferId,
    version: u64,
    len_chars: usize,
    language: LanguageId,
    first_line: usize,
    line_count: usize,
    independent_color_pool_per_bracket_type: bool,
}

#[derive(Debug, Clone)]
struct BracketColorOverlayCacheEntry {
    key: BracketColorOverlayCacheKey,
    char_range: Range<usize>,
    colors: Vec<BracketColor>,
}

impl BracketColorOverlayCacheKey {
    fn can_serve(&self, requested: &Self) -> bool {
        self.buffer_id == requested.buffer_id
            && self.version == requested.version
            && self.len_chars == requested.len_chars
            && self.language == requested.language
            && self.independent_color_pool_per_bracket_type
                == requested.independent_color_pool_per_bracket_type
            && self.first_line <= requested.first_line
            && self.first_line.saturating_add(self.line_count)
                >= requested.first_line.saturating_add(requested.line_count)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BracketPairGuideOverlayCacheKey {
    buffer_id: BufferId,
    version: u64,
    len_chars: usize,
    language: LanguageId,
}

#[derive(Debug, Clone)]
struct BracketPairGuideOverlayCacheEntry {
    key: BracketPairGuideOverlayCacheKey,
    guides: Vec<BracketPairGuide>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BracketMatchOverlayCacheKey {
    buffer_id: BufferId,
    version: u64,
    len_chars: usize,
    language: LanguageId,
    cursors: Vec<usize>,
    mode: EditorMatchBrackets,
}

#[derive(Debug, Clone, Copy)]
struct BracketMatchOverlayCacheLookupKey<'a> {
    buffer_id: BufferId,
    version: u64,
    len_chars: usize,
    language: LanguageId,
    cursors: &'a [usize],
    mode: EditorMatchBrackets,
}

impl BracketMatchOverlayCacheKey {
    fn matches_lookup(&self, requested: &BracketMatchOverlayCacheLookupKey<'_>) -> bool {
        self.buffer_id == requested.buffer_id
            && self.version == requested.version
            && self.len_chars == requested.len_chars
            && self.language == requested.language
            && self.cursors == requested.cursors
            && self.mode == requested.mode
    }
}

#[derive(Debug, Clone)]
struct BracketMatchOverlayCacheEntry {
    key: BracketMatchOverlayCacheKey,
    matches: Vec<(usize, usize)>,
}

impl EditorBracketOverlayCache {
    pub(crate) fn bracket_colors_for_lines(
        &mut self,
        buffer: &TextBuffer,
        first_line: usize,
        line_count: usize,
        independent_color_pool_per_bracket_type: bool,
    ) -> Vec<BracketColor> {
        let buffer_id = buffer.id();
        if buffer_needs_bracket_scan_protection(buffer) {
            self.clear_for_buffer(buffer_id);
            return Vec::new();
        }

        let version = buffer.version();
        let len_chars = buffer.len_chars();
        let language = buffer.language();
        retain_current_color_state(&mut self.colors, buffer_id, version, len_chars, language);

        let Some(line_count) = normalized_line_count(buffer, first_line, line_count) else {
            return Vec::new();
        };
        if line_count > MAX_BRACKET_COLOR_LINES_PER_REQUEST {
            return Vec::new();
        }
        let Some(char_range) = line_char_range(buffer, first_line, line_count) else {
            return Vec::new();
        };
        let key = BracketColorOverlayCacheKey {
            buffer_id,
            version,
            len_chars,
            language,
            first_line,
            line_count,
            independent_color_pool_per_bracket_type,
        };
        if let Some(colors) = lookup_colors(&mut self.colors, &key, &char_range) {
            return colors;
        }

        let mut colors = buffer.bracket_colors_for_lines_with_options(
            first_line,
            line_count,
            independent_color_pool_per_bracket_type,
        );
        retain_valid_bracket_colors(&mut colors, &char_range);
        push_colors(&mut self.colors, key, char_range, colors.clone());
        colors
    }

    pub(crate) fn bracket_pair_guides(&mut self, buffer: &TextBuffer) -> Vec<BracketPairGuide> {
        let buffer_id = buffer.id();
        if buffer_needs_bracket_scan_protection(buffer) {
            self.clear_for_buffer(buffer_id);
            return Vec::new();
        }

        let version = buffer.version();
        let len_chars = buffer.len_chars();
        let language = buffer.language();
        retain_current_guide_state(&mut self.guides, buffer_id, version, len_chars, language);

        let key = BracketPairGuideOverlayCacheKey {
            buffer_id,
            version,
            len_chars,
            language,
        };
        if let Some(guides) = lookup_guides(&mut self.guides, &key) {
            return guides;
        }

        let mut guides = buffer.bracket_pair_guides();
        retain_valid_bracket_pair_guides(&mut guides, len_chars);
        push_guides(&mut self.guides, key, guides.clone());
        guides
    }

    pub(crate) fn bracket_matches(
        &mut self,
        buffer: &TextBuffer,
        mode: EditorMatchBrackets,
    ) -> Vec<(usize, usize)> {
        let buffer_id = buffer.id();
        if buffer_needs_bracket_scan_protection(buffer) {
            self.clear_for_buffer(buffer_id);
            return Vec::new();
        }

        let version = buffer.version();
        let len_chars = buffer.len_chars();
        let language = buffer.language();
        retain_current_match_state(&mut self.matches, buffer_id, version, len_chars, language);

        if !mode.enabled() {
            return Vec::new();
        }

        let key = match buffer.selections() {
            [selection] => {
                let cursor = selection.cursor.min(len_chars);
                let cursors = [cursor];
                let lookup_key = BracketMatchOverlayCacheLookupKey {
                    buffer_id,
                    version,
                    len_chars,
                    language,
                    cursors: &cursors,
                    mode,
                };
                if let Some(matches) = lookup_matches(&mut self.matches, lookup_key) {
                    return matches;
                }

                BracketMatchOverlayCacheKey {
                    buffer_id,
                    version,
                    len_chars,
                    language,
                    cursors: vec![cursor],
                    mode,
                }
            }
            selections => {
                let cursors = ordered_cursor_indices(selections, len_chars);
                let lookup_key = BracketMatchOverlayCacheLookupKey {
                    buffer_id,
                    version,
                    len_chars,
                    language,
                    cursors: &cursors,
                    mode,
                };
                if let Some(matches) = lookup_matches(&mut self.matches, lookup_key) {
                    return matches;
                }

                BracketMatchOverlayCacheKey {
                    buffer_id,
                    version,
                    len_chars,
                    language,
                    cursors,
                    mode,
                }
            }
        };

        let matches = match mode {
            EditorMatchBrackets::Always => buffer.matching_brackets_including_enclosing(),
            EditorMatchBrackets::Near => buffer.matching_brackets(),
            EditorMatchBrackets::Never => Vec::new(),
        };
        let mut matches = matches;
        retain_valid_bracket_matches(&mut matches, len_chars);
        push_matches(&mut self.matches, key, matches.clone());
        matches
    }

    pub(crate) fn clear(&mut self) {
        self.colors.clear();
        self.guides.clear();
        self.matches.clear();
    }

    pub(crate) fn clear_for_buffer(&mut self, buffer_id: BufferId) {
        self.colors.retain(|entry| entry.key.buffer_id != buffer_id);
        self.guides.retain(|entry| entry.key.buffer_id != buffer_id);
        self.matches
            .retain(|entry| entry.key.buffer_id != buffer_id);
    }

    #[cfg(test)]
    pub(crate) fn contains_buffer_for_test(&self, buffer_id: BufferId) -> bool {
        self.colors
            .iter()
            .any(|entry| entry.key.buffer_id == buffer_id)
            || self
                .guides
                .iter()
                .any(|entry| entry.key.buffer_id == buffer_id)
            || self
                .matches
                .iter()
                .any(|entry| entry.key.buffer_id == buffer_id)
    }

    #[cfg(test)]
    fn contains_color_range_for_test(
        &self,
        buffer_id: BufferId,
        first_line: usize,
        line_count: usize,
    ) -> bool {
        self.colors.iter().any(|entry| {
            entry.key.buffer_id == buffer_id
                && entry.key.first_line == first_line
                && entry.key.line_count == line_count
        })
    }

    #[cfg(test)]
    fn contains_match_key_for_test(
        &self,
        buffer_id: BufferId,
        version: u64,
        language: LanguageId,
        cursors: &[usize],
        mode: EditorMatchBrackets,
    ) -> bool {
        self.matches.iter().any(|entry| {
            entry.key.buffer_id == buffer_id
                && entry.key.version == version
                && entry.key.language == language
                && entry.key.cursors == cursors
                && entry.key.mode == mode
        })
    }
}

fn lookup_colors(
    entries: &mut VecDeque<BracketColorOverlayCacheEntry>,
    key: &BracketColorOverlayCacheKey,
    char_range: &Range<usize>,
) -> Option<Vec<BracketColor>> {
    loop {
        let index = color_lookup_index(entries, key, char_range)?;
        if !entries
            .get(index)
            .is_some_and(|entry| bracket_color_entry_fits_buffer(entry, key.len_chars))
        {
            entries.remove(index);
            continue;
        }
        if index + 1 == entries.len() {
            return entries
                .get(index)
                .map(|entry| colors_for_requested_range(entry, key, char_range));
        }
        let entry = entries.remove(index)?;
        let colors = colors_for_requested_range(&entry, key, char_range);
        entries.push_back(entry);
        return Some(colors);
    }
}

fn color_lookup_index(
    entries: &VecDeque<BracketColorOverlayCacheEntry>,
    key: &BracketColorOverlayCacheKey,
    char_range: &Range<usize>,
) -> Option<usize> {
    let mut reusable_index = None;
    let mut exact_index = None;
    for (index, entry) in entries.iter().enumerate() {
        if entry.key == *key {
            exact_index = Some(index);
            break;
        }
        if reusable_index.is_none()
            && entry.key.can_serve(key)
            && entry.char_range.start <= char_range.start
            && entry.char_range.end >= char_range.end
        {
            reusable_index = Some(index);
        }
    }

    exact_index.or(reusable_index)
}

fn push_colors(
    entries: &mut VecDeque<BracketColorOverlayCacheEntry>,
    key: BracketColorOverlayCacheKey,
    char_range: Range<usize>,
    colors: Vec<BracketColor>,
) {
    if entries.len() >= BRACKET_COLOR_CACHE_CAPACITY {
        entries.pop_front();
    }
    entries.push_back(BracketColorOverlayCacheEntry {
        key,
        char_range,
        colors,
    });
}

fn colors_for_requested_range(
    entry: &BracketColorOverlayCacheEntry,
    key: &BracketColorOverlayCacheKey,
    char_range: &Range<usize>,
) -> Vec<BracketColor> {
    if entry.key == *key {
        return entry.colors.clone();
    }
    entry
        .colors
        .iter()
        .copied()
        .filter(|color| color.char_idx >= char_range.start && color.char_idx < char_range.end)
        .collect()
}

fn lookup_guides(
    entries: &mut VecDeque<BracketPairGuideOverlayCacheEntry>,
    key: &BracketPairGuideOverlayCacheKey,
) -> Option<Vec<BracketPairGuide>> {
    let index = entries.iter().position(|entry| entry.key == *key)?;
    if !entries
        .get(index)
        .is_some_and(|entry| bracket_pair_guides_fit_buffer(&entry.guides, key.len_chars))
    {
        entries.remove(index);
        return None;
    }
    if index + 1 == entries.len() {
        return entries.get(index).map(|entry| entry.guides.clone());
    }
    let entry = entries.remove(index)?;
    let guides = entry.guides.clone();
    entries.push_back(entry);
    Some(guides)
}

fn push_guides(
    entries: &mut VecDeque<BracketPairGuideOverlayCacheEntry>,
    key: BracketPairGuideOverlayCacheKey,
    guides: Vec<BracketPairGuide>,
) {
    if entries.len() >= BRACKET_PAIR_GUIDE_CACHE_CAPACITY {
        entries.pop_front();
    }
    entries.push_back(BracketPairGuideOverlayCacheEntry { key, guides });
}

fn lookup_matches(
    entries: &mut VecDeque<BracketMatchOverlayCacheEntry>,
    key: BracketMatchOverlayCacheLookupKey<'_>,
) -> Option<Vec<(usize, usize)>> {
    let index = entries
        .iter()
        .position(|entry| entry.key.matches_lookup(&key))?;
    if !entries
        .get(index)
        .is_some_and(|entry| bracket_matches_fit_buffer(&entry.matches, key.len_chars))
    {
        entries.remove(index);
        return None;
    }
    if index + 1 == entries.len() {
        return entries.get(index).map(|entry| entry.matches.clone());
    }
    let entry = entries.remove(index)?;
    let matches = entry.matches.clone();
    entries.push_back(entry);
    Some(matches)
}

fn push_matches(
    entries: &mut VecDeque<BracketMatchOverlayCacheEntry>,
    key: BracketMatchOverlayCacheKey,
    matches: Vec<(usize, usize)>,
) {
    if entries.len() >= BRACKET_MATCH_CACHE_CAPACITY {
        entries.pop_front();
    }
    entries.push_back(BracketMatchOverlayCacheEntry { key, matches });
}

#[cfg(test)]
mod tests {
    use super::{
        BRACKET_COLOR_CACHE_CAPACITY, BRACKET_MATCH_CACHE_CAPACITY,
        BRACKET_PAIR_GUIDE_CACHE_CAPACITY, BracketColorOverlayCacheEntry,
        BracketColorOverlayCacheKey, BracketMatchOverlayCacheEntry, BracketMatchOverlayCacheKey,
        BracketMatchOverlayCacheLookupKey, BracketPairGuideOverlayCacheEntry,
        BracketPairGuideOverlayCacheKey, EditorBracketOverlayCache,
        MAX_BRACKET_COLOR_LINES_PER_REQUEST, line_char_range, lookup_colors, lookup_guides,
        lookup_matches, ordered_cursor_indices,
    };
    use crate::large_file_mode::LARGE_FILE_MODE_MAX_BYTES;
    use kuroya_core::{
        EditorMatchBrackets, LanguageId, Selection, TextBuffer,
        buffer::{BracketColor, BracketPairGuide},
    };
    use std::collections::VecDeque;

    #[test]
    fn bracket_color_cache_tracks_buffer_version_language_range_and_color_pool() {
        let mut cache = EditorBracketOverlayCache::default();
        let mut buffer = TextBuffer::from_text_with_language(
            7,
            None,
            "fn main() {\n  call([1]);\n}\n".to_owned(),
            LanguageId::Rust,
        );

        let first = cache.bracket_colors_for_lines(&buffer, 0, 3, false);
        assert_eq!(
            first,
            buffer.bracket_colors_for_lines_with_options(0, 3, false)
        );
        assert!(cache.contains_buffer_for_test(7));

        let pool_specific = cache.bracket_colors_for_lines(&buffer, 0, 3, true);
        assert_eq!(
            pool_specific,
            buffer.bracket_colors_for_lines_with_options(0, 3, true)
        );

        let second_line = cache.bracket_colors_for_lines(&buffer, 1, 1, false);
        assert_eq!(
            second_line,
            buffer.bracket_colors_for_lines_with_options(1, 1, false)
        );

        buffer.set_path("notes.txt".into());
        let plain_text = cache.bracket_colors_for_lines(&buffer, 0, 3, false);
        assert_eq!(
            plain_text,
            buffer.bracket_colors_for_lines_with_options(0, 3, false)
        );

        buffer.insert_at_cursor("(");
        let changed = cache.bracket_colors_for_lines(&buffer, 0, 3, false);
        assert_eq!(
            changed,
            buffer.bracket_colors_for_lines_with_options(0, 3, false)
        );
    }

    #[test]
    fn bracket_color_cache_normalizes_ranges_and_skips_impossible_ranges() {
        let mut cache = EditorBracketOverlayCache::default();
        let buffer = TextBuffer::from_text(8, None, "{\n}\n".to_owned());

        assert!(
            cache
                .bracket_colors_for_lines(&buffer, 99, 2, false)
                .is_empty()
        );
        assert!(!cache.contains_buffer_for_test(8));
        assert!(
            cache
                .bracket_colors_for_lines(&buffer, 0, 0, false)
                .is_empty()
        );
        assert!(!cache.contains_buffer_for_test(8));

        let full = cache.bracket_colors_for_lines(&buffer, 0, 99, false);
        assert_eq!(
            full,
            buffer.bracket_colors_for_lines_with_options(0, buffer.len_lines(), false)
        );
        assert!(cache.contains_buffer_for_test(8));
    }

    #[test]
    fn bracket_color_cache_rejects_oversized_line_batches() {
        let mut cache = EditorBracketOverlayCache::default();
        let text = (0..=MAX_BRACKET_COLOR_LINES_PER_REQUEST)
            .map(|_| "{}")
            .collect::<Vec<_>>()
            .join("\n");
        let buffer = TextBuffer::from_text(23, None, text);

        assert!(
            cache
                .bracket_colors_for_lines(
                    &buffer,
                    0,
                    MAX_BRACKET_COLOR_LINES_PER_REQUEST + 1,
                    false,
                )
                .is_empty()
        );
        assert!(!cache.contains_buffer_for_test(23));

        assert!(
            !cache
                .bracket_colors_for_lines(&buffer, 0, MAX_BRACKET_COLOR_LINES_PER_REQUEST, false,)
                .is_empty()
        );
        assert!(cache.contains_color_range_for_test(23, 0, MAX_BRACKET_COLOR_LINES_PER_REQUEST));
    }

    #[test]
    fn bracket_color_cache_reuses_containing_range_for_subranges() {
        let mut cache = EditorBracketOverlayCache::default();
        let buffer = TextBuffer::from_text_with_language(
            19,
            None,
            "fn main() {\n  call([1]);\n}\n".to_owned(),
            LanguageId::Rust,
        );

        assert_eq!(
            cache.bracket_colors_for_lines(&buffer, 0, 3, false),
            buffer.bracket_colors_for_lines_with_options(0, 3, false)
        );
        assert!(cache.contains_color_range_for_test(19, 0, 3));

        assert_eq!(
            cache.bracket_colors_for_lines(&buffer, 1, 1, false),
            buffer.bracket_colors_for_lines_with_options(1, 1, false)
        );
        assert!(!cache.contains_color_range_for_test(19, 1, 1));
    }

    #[test]
    fn bracket_color_subrange_reuse_preserves_raw_char_indices() {
        let mut cache = EditorBracketOverlayCache::default();
        let buffer = TextBuffer::from_text_with_language(
            21,
            None,
            "fn main() {\n  call([1]);\n}\n".to_owned(),
            LanguageId::Rust,
        );

        cache.bracket_colors_for_lines(&buffer, 0, 3, false);
        let second_line = cache.bracket_colors_for_lines(&buffer, 1, 1, false);
        let second_line_range = line_char_range(&buffer, 1, 1).unwrap();

        assert_eq!(
            second_line,
            buffer.bracket_colors_for_lines_with_options(1, 1, false)
        );
        assert!(!second_line.is_empty());
        assert!(second_line.iter().all(|color| {
            color.char_idx >= second_line_range.start && color.char_idx < second_line_range.end
        }));
    }

    #[test]
    fn bracket_color_lookup_prefers_exact_entry_over_reusable_containing_range() {
        let exact_key = BracketColorOverlayCacheKey {
            buffer_id: 25,
            version: 1,
            len_chars: 12,
            language: LanguageId::PlainText,
            first_line: 1,
            line_count: 1,
            independent_color_pool_per_bracket_type: false,
        };
        let reusable_key = BracketColorOverlayCacheKey {
            first_line: 0,
            line_count: 3,
            ..exact_key.clone()
        };
        let reusable_colors = vec![BracketColor {
            char_idx: 5,
            depth: 7,
        }];
        let exact_colors = vec![BracketColor {
            char_idx: 5,
            depth: 1,
        }];
        let mut entries = VecDeque::new();
        entries.push_back(BracketColorOverlayCacheEntry {
            key: reusable_key,
            char_range: 0..12,
            colors: reusable_colors,
        });
        entries.push_back(BracketColorOverlayCacheEntry {
            key: exact_key.clone(),
            char_range: 4..8,
            colors: exact_colors.clone(),
        });

        assert_eq!(
            lookup_colors(&mut entries, &exact_key, &(4..8)),
            Some(exact_colors)
        );
    }

    #[test]
    fn bracket_color_lookup_falls_back_after_invalid_exact_entry() {
        let exact_key = BracketColorOverlayCacheKey {
            buffer_id: 25,
            version: 1,
            len_chars: 12,
            language: LanguageId::PlainText,
            first_line: 1,
            line_count: 1,
            independent_color_pool_per_bracket_type: false,
        };
        let reusable_key = BracketColorOverlayCacheKey {
            first_line: 0,
            line_count: 3,
            ..exact_key.clone()
        };
        let mut entries = VecDeque::new();
        entries.push_back(BracketColorOverlayCacheEntry {
            key: reusable_key,
            char_range: 0..12,
            colors: vec![BracketColor {
                char_idx: 5,
                depth: 7,
            }],
        });
        entries.push_back(BracketColorOverlayCacheEntry {
            key: exact_key.clone(),
            char_range: 4..8,
            colors: vec![BracketColor {
                char_idx: 99,
                depth: 1,
            }],
        });

        assert_eq!(
            lookup_colors(&mut entries, &exact_key, &(4..8)),
            Some(vec![BracketColor {
                char_idx: 5,
                depth: 7,
            }])
        );
        assert_eq!(entries.len(), 1);
        assert_eq!(entries.back().unwrap().key.first_line, 0);
    }

    #[test]
    fn bracket_color_lookup_skips_invalid_reusable_entry_before_later_fallback() {
        let request_key = BracketColorOverlayCacheKey {
            buffer_id: 26,
            version: 1,
            len_chars: 12,
            language: LanguageId::PlainText,
            first_line: 1,
            line_count: 1,
            independent_color_pool_per_bracket_type: false,
        };
        let reusable_key = BracketColorOverlayCacheKey {
            first_line: 0,
            line_count: 3,
            ..request_key.clone()
        };
        let mut entries = VecDeque::new();
        entries.push_back(BracketColorOverlayCacheEntry {
            key: reusable_key.clone(),
            char_range: 0..12,
            colors: vec![BracketColor {
                char_idx: 99,
                depth: 7,
            }],
        });
        entries.push_back(BracketColorOverlayCacheEntry {
            key: reusable_key,
            char_range: 0..12,
            colors: vec![BracketColor {
                char_idx: 5,
                depth: 1,
            }],
        });

        assert_eq!(
            lookup_colors(&mut entries, &request_key, &(4..8)),
            Some(vec![BracketColor {
                char_idx: 5,
                depth: 1,
            }])
        );
        assert_eq!(entries.len(), 1);
        assert_eq!(entries.back().unwrap().colors[0].char_idx, 5);
    }

    #[test]
    fn bracket_pair_guide_cache_tracks_buffer_version_and_language() {
        let mut cache = EditorBracketOverlayCache::default();
        let mut buffer = TextBuffer::from_text_with_language(
            9,
            None,
            "{\n  [x]\n}\n".to_owned(),
            LanguageId::Rust,
        );

        assert_eq!(
            cache.bracket_pair_guides(&buffer),
            buffer.bracket_pair_guides()
        );
        assert!(cache.contains_buffer_for_test(9));

        buffer.set_path("notes.txt".into());
        assert_eq!(
            cache.bracket_pair_guides(&buffer),
            buffer.bracket_pair_guides()
        );

        buffer.insert_at_cursor("(");
        assert_eq!(
            cache.bracket_pair_guides(&buffer),
            buffer.bracket_pair_guides()
        );
    }

    #[test]
    fn bracket_match_cache_tracks_mode_cursor_and_language() {
        let mut cache = EditorBracketOverlayCache::default();
        let mut buffer = TextBuffer::from_text_with_language(
            13,
            None,
            "fn main() { call(); }".to_owned(),
            LanguageId::Rust,
        );
        buffer.set_single_cursor(14);
        let initial_version = buffer.version();

        assert_eq!(
            cache.bracket_matches(&buffer, EditorMatchBrackets::Near),
            Vec::<(usize, usize)>::new()
        );
        assert!(cache.contains_match_key_for_test(
            13,
            initial_version,
            LanguageId::Rust,
            &[14],
            EditorMatchBrackets::Near,
        ));

        assert_eq!(
            cache.bracket_matches(&buffer, EditorMatchBrackets::Always),
            buffer.matching_brackets_including_enclosing()
        );
        assert!(cache.contains_match_key_for_test(
            13,
            initial_version,
            LanguageId::Rust,
            &[14],
            EditorMatchBrackets::Always,
        ));

        buffer.set_single_cursor(10);
        assert_eq!(
            cache.bracket_matches(&buffer, EditorMatchBrackets::Near),
            buffer.matching_brackets()
        );
        assert!(cache.contains_match_key_for_test(
            13,
            initial_version,
            LanguageId::Rust,
            &[10],
            EditorMatchBrackets::Near,
        ));

        buffer.set_path("notes.txt".into());
        assert_eq!(
            cache.bracket_matches(&buffer, EditorMatchBrackets::Near),
            buffer.matching_brackets()
        );
        assert!(cache.contains_match_key_for_test(
            13,
            buffer.version(),
            buffer.language(),
            &[10],
            EditorMatchBrackets::Near,
        ));
    }

    #[test]
    fn bracket_match_cache_tracks_buffer_version() {
        let mut cache = EditorBracketOverlayCache::default();
        let mut buffer = TextBuffer::from_text(14, None, "{}".to_owned());
        buffer.set_single_cursor(0);
        let initial_version = buffer.version();

        let matched = cache.bracket_matches(&buffer, EditorMatchBrackets::Near);
        assert_eq!(matched, buffer.matching_brackets());
        buffer.insert_at_cursor("[");
        buffer.set_single_cursor(0);
        let changed = cache.bracket_matches(&buffer, EditorMatchBrackets::Near);
        assert_eq!(changed, buffer.matching_brackets());
        assert_ne!(changed, matched);
        assert!(!cache.contains_match_key_for_test(
            14,
            initial_version,
            buffer.language(),
            &[0],
            EditorMatchBrackets::Near,
        ));
        assert!(cache.contains_match_key_for_test(
            14,
            buffer.version(),
            buffer.language(),
            &[0],
            EditorMatchBrackets::Near,
        ));
    }

    #[test]
    fn bracket_overlay_cache_prunes_stale_current_buffer_state() {
        let mut cache = EditorBracketOverlayCache::default();
        let mut buffer = TextBuffer::from_text_with_language(
            16,
            None,
            "fn main() { call(); }\n".to_owned(),
            LanguageId::Rust,
        );
        let other = TextBuffer::from_text(17, None, "{}\n".to_owned());
        buffer.set_single_cursor(10);

        cache.bracket_colors_for_lines(&buffer, 0, 1, false);
        cache.bracket_pair_guides(&buffer);
        cache.bracket_matches(&buffer, EditorMatchBrackets::Near);
        cache.bracket_colors_for_lines(&other, 0, 1, false);
        cache.bracket_pair_guides(&other);
        cache.bracket_matches(&other, EditorMatchBrackets::Near);

        let stale_version = buffer.version();
        buffer.insert_at_cursor("(");
        let current_version = buffer.version();
        assert_ne!(stale_version, current_version);

        assert!(
            cache
                .bracket_colors_for_lines(&buffer, 99, 1, false)
                .is_empty()
        );
        cache.bracket_pair_guides(&buffer);
        assert!(
            cache
                .bracket_matches(&buffer, EditorMatchBrackets::Never)
                .is_empty()
        );

        assert!(cache.colors.iter().all(|entry| {
            entry.key.buffer_id != buffer.id() || entry.key.version == current_version
        }));
        assert!(cache.guides.iter().all(|entry| {
            entry.key.buffer_id != buffer.id() || entry.key.version == current_version
        }));
        assert!(cache.matches.iter().all(|entry| {
            entry.key.buffer_id != buffer.id() || entry.key.version == current_version
        }));
        assert!(cache.contains_buffer_for_test(other.id()));
    }

    #[test]
    fn bracket_overlay_cache_prunes_entries_after_language_changes() {
        let mut cache = EditorBracketOverlayCache::default();
        let mut buffer = TextBuffer::from_text_with_language(
            18,
            None,
            "fn main() { call(); }\n".to_owned(),
            LanguageId::Rust,
        );
        buffer.set_single_cursor(10);

        cache.bracket_colors_for_lines(&buffer, 0, 1, false);
        cache.bracket_pair_guides(&buffer);
        cache.bracket_matches(&buffer, EditorMatchBrackets::Near);

        buffer.set_path("notes.txt".into());
        let language = buffer.language();

        cache.bracket_colors_for_lines(&buffer, 0, 1, false);
        cache.bracket_pair_guides(&buffer);
        cache.bracket_matches(&buffer, EditorMatchBrackets::Near);

        assert!(
            cache.colors.iter().all(|entry| {
                entry.key.buffer_id != buffer.id() || entry.key.language == language
            })
        );
        assert!(
            cache.guides.iter().all(|entry| {
                entry.key.buffer_id != buffer.id() || entry.key.language == language
            })
        );
        assert!(
            cache.matches.iter().all(|entry| {
                entry.key.buffer_id != buffer.id() || entry.key.language == language
            })
        );
    }

    #[test]
    fn bracket_overlay_cache_clears_protected_buffers_without_scanning() {
        let mut cache = EditorBracketOverlayCache::default();
        let mut buffer = TextBuffer::from_text(24, None, "{}\n".to_owned());
        buffer.set_single_cursor(0);

        cache.bracket_colors_for_lines(&buffer, 0, 1, false);
        cache.bracket_pair_guides(&buffer);
        cache.bracket_matches(&buffer, EditorMatchBrackets::Near);
        assert!(cache.contains_buffer_for_test(24));

        let len_chars = buffer.len_chars();
        assert!(buffer.replace_range(0..len_chars, &"x".repeat(LARGE_FILE_MODE_MAX_BYTES + 1),));
        buffer.set_single_cursor(0);

        assert!(
            cache
                .bracket_colors_for_lines(&buffer, 0, 1, false)
                .is_empty()
        );
        assert!(cache.bracket_pair_guides(&buffer).is_empty());
        assert!(
            cache
                .bracket_matches(&buffer, EditorMatchBrackets::Always)
                .is_empty()
        );
        assert!(!cache.contains_buffer_for_test(24));
    }

    #[test]
    fn bracket_match_cache_skips_never_mode() {
        let mut cache = EditorBracketOverlayCache::default();
        let buffer = TextBuffer::from_text(14, None, "{}".to_owned());

        assert_eq!(
            cache.bracket_matches(&buffer, EditorMatchBrackets::Never),
            Vec::<(usize, usize)>::new()
        );
        assert!(!cache.contains_buffer_for_test(14));
    }

    #[test]
    fn cache_clear_removes_all_entries_or_one_buffer() {
        let mut cache = EditorBracketOverlayCache::default();
        let first = TextBuffer::from_text(10, None, "{}".to_owned());
        let second = TextBuffer::from_text(11, None, "[]".to_owned());

        cache.bracket_colors_for_lines(&first, 0, 1, false);
        cache.bracket_matches(&first, EditorMatchBrackets::Near);
        cache.bracket_pair_guides(&second);
        assert!(cache.contains_buffer_for_test(10));
        assert!(cache.contains_buffer_for_test(11));

        cache.clear_for_buffer(10);
        assert!(!cache.contains_buffer_for_test(10));
        assert!(cache.contains_buffer_for_test(11));

        cache.clear();
        assert!(!cache.contains_buffer_for_test(11));
    }

    #[test]
    fn bracket_color_cache_evicts_least_recently_used_range() {
        let mut cache = EditorBracketOverlayCache::default();
        let text = (0..BRACKET_COLOR_CACHE_CAPACITY + 1)
            .map(|_| "{}")
            .collect::<Vec<_>>()
            .join("\n");
        let buffer = TextBuffer::from_text(12, None, text);

        for line in 0..BRACKET_COLOR_CACHE_CAPACITY {
            cache.bracket_colors_for_lines(&buffer, line, 1, false);
        }

        cache.bracket_colors_for_lines(&buffer, 0, 1, false);
        cache.bracket_colors_for_lines(&buffer, BRACKET_COLOR_CACHE_CAPACITY, 1, false);

        assert!(cache.contains_color_range_for_test(12, 0, 1));
        assert!(!cache.contains_color_range_for_test(12, 1, 1));
        assert!(cache.contains_color_range_for_test(12, BRACKET_COLOR_CACHE_CAPACITY, 1));
    }

    #[test]
    fn bracket_pair_guide_cache_evicts_least_recently_used_buffer() {
        let mut cache = EditorBracketOverlayCache::default();
        let buffers = (0..BRACKET_PAIR_GUIDE_CACHE_CAPACITY + 1)
            .map(|offset| TextBuffer::from_text(100 + offset as u64, None, "{}".to_owned()))
            .collect::<Vec<_>>();

        for buffer in buffers.iter().take(BRACKET_PAIR_GUIDE_CACHE_CAPACITY) {
            cache.bracket_pair_guides(buffer);
        }

        cache.bracket_pair_guides(&buffers[0]);
        cache.bracket_pair_guides(&buffers[BRACKET_PAIR_GUIDE_CACHE_CAPACITY]);

        assert!(cache.contains_buffer_for_test(100));
        assert!(!cache.contains_buffer_for_test(101));
        assert!(cache.contains_buffer_for_test(100 + BRACKET_PAIR_GUIDE_CACHE_CAPACITY as u64));
    }

    #[test]
    fn bracket_match_cache_evicts_least_recently_used_cursor_set() {
        let mut cache = EditorBracketOverlayCache::default();
        let text = "{} ".repeat(BRACKET_MATCH_CACHE_CAPACITY + 1);
        let mut buffer = TextBuffer::from_text(15, None, text);
        let version = buffer.version();

        for pair in 0..BRACKET_MATCH_CACHE_CAPACITY {
            buffer.set_single_cursor(pair * 3);
            cache.bracket_matches(&buffer, EditorMatchBrackets::Near);
        }

        buffer.set_single_cursor(0);
        cache.bracket_matches(&buffer, EditorMatchBrackets::Near);
        buffer.set_single_cursor(BRACKET_MATCH_CACHE_CAPACITY * 3);
        cache.bracket_matches(&buffer, EditorMatchBrackets::Near);

        assert!(cache.contains_match_key_for_test(
            15,
            version,
            buffer.language(),
            &[0],
            EditorMatchBrackets::Near,
        ));
        assert!(!cache.contains_match_key_for_test(
            15,
            version,
            buffer.language(),
            &[3],
            EditorMatchBrackets::Near,
        ));
        assert!(cache.contains_match_key_for_test(
            15,
            version,
            buffer.language(),
            &[BRACKET_MATCH_CACHE_CAPACITY * 3],
            EditorMatchBrackets::Near,
        ));
    }

    #[test]
    fn bracket_overlay_cache_drops_cached_ranges_that_do_not_fit_buffer() {
        let mut cache = EditorBracketOverlayCache::default();
        let mut buffer = TextBuffer::from_text(20, None, "{}\n".to_owned());
        buffer.set_single_cursor(0);
        let len_chars = buffer.len_chars();
        let version = buffer.version();
        let language = buffer.language();

        cache.colors.push_back(BracketColorOverlayCacheEntry {
            key: BracketColorOverlayCacheKey {
                buffer_id: buffer.id(),
                version,
                len_chars,
                language,
                first_line: 0,
                line_count: 1,
                independent_color_pool_per_bracket_type: false,
            },
            char_range: 0..len_chars + 1,
            colors: vec![BracketColor {
                char_idx: len_chars + 1,
                depth: 0,
            }],
        });
        cache.guides.push_back(BracketPairGuideOverlayCacheEntry {
            key: BracketPairGuideOverlayCacheKey {
                buffer_id: buffer.id(),
                version,
                len_chars,
                language,
            },
            guides: vec![BracketPairGuide {
                open_idx: 0,
                close_idx: len_chars + 1,
                depth: 0,
            }],
        });
        cache.matches.push_back(BracketMatchOverlayCacheEntry {
            key: BracketMatchOverlayCacheKey {
                buffer_id: buffer.id(),
                version,
                len_chars,
                language,
                cursors: vec![0],
                mode: EditorMatchBrackets::Near,
            },
            matches: vec![(0, len_chars + 1)],
        });

        assert_eq!(
            cache.bracket_colors_for_lines(&buffer, 0, 1, false),
            buffer.bracket_colors_for_lines_with_options(0, 1, false)
        );
        assert_eq!(
            cache.bracket_pair_guides(&buffer),
            buffer.bracket_pair_guides()
        );
        assert_eq!(
            cache.bracket_matches(&buffer, EditorMatchBrackets::Near),
            buffer.matching_brackets()
        );
        assert_eq!(cache.colors.len(), 1);
        assert_eq!(cache.guides.len(), 1);
        assert_eq!(cache.matches.len(), 1);
        assert!(
            cache
                .colors
                .back()
                .unwrap()
                .colors
                .iter()
                .all(|color| color.char_idx < len_chars)
        );
        assert!(
            cache
                .guides
                .back()
                .unwrap()
                .guides
                .iter()
                .all(|guide| guide.close_idx < len_chars)
        );
        assert!(
            cache
                .matches
                .back()
                .unwrap()
                .matches
                .iter()
                .all(|(left, right)| *left < len_chars && *right < len_chars)
        );
    }

    #[test]
    fn bracket_match_lookup_promotes_single_cursor_hit_from_borrowed_slice() {
        let mut entries = VecDeque::new();
        entries.push_back(BracketMatchOverlayCacheEntry {
            key: BracketMatchOverlayCacheKey {
                buffer_id: 22,
                version: 1,
                len_chars: 4,
                language: LanguageId::PlainText,
                cursors: vec![0],
                mode: EditorMatchBrackets::Near,
            },
            matches: vec![(0, 1)],
        });
        entries.push_back(BracketMatchOverlayCacheEntry {
            key: BracketMatchOverlayCacheKey {
                buffer_id: 22,
                version: 1,
                len_chars: 4,
                language: LanguageId::PlainText,
                cursors: vec![2],
                mode: EditorMatchBrackets::Near,
            },
            matches: vec![(2, 3)],
        });

        let cursor = [0];
        let lookup_key = BracketMatchOverlayCacheLookupKey {
            buffer_id: 22,
            version: 1,
            len_chars: 4,
            language: LanguageId::PlainText,
            cursors: &cursor,
            mode: EditorMatchBrackets::Near,
        };

        assert_eq!(lookup_matches(&mut entries, lookup_key), Some(vec![(0, 1)]));
        assert_eq!(entries.len(), 2);
        assert_eq!(entries.front().unwrap().key.cursors.as_slice(), &[2]);
        assert_eq!(entries.back().unwrap().key.cursors.as_slice(), &[0]);
    }

    #[test]
    fn bracket_lookup_drops_invalid_payload_before_reuse() {
        let color_key = BracketColorOverlayCacheKey {
            buffer_id: 22,
            version: 1,
            len_chars: 4,
            language: LanguageId::PlainText,
            first_line: 0,
            line_count: 1,
            independent_color_pool_per_bracket_type: false,
        };
        let mut color_entries = VecDeque::new();
        color_entries.push_back(BracketColorOverlayCacheEntry {
            key: color_key.clone(),
            char_range: 0..4,
            colors: vec![BracketColor {
                char_idx: 8,
                depth: 0,
            }],
        });

        assert_eq!(lookup_colors(&mut color_entries, &color_key, &(0..4)), None);
        assert!(color_entries.is_empty());

        let guide_key = BracketPairGuideOverlayCacheKey {
            buffer_id: 22,
            version: 1,
            len_chars: 4,
            language: LanguageId::PlainText,
        };
        let mut guide_entries = VecDeque::new();
        guide_entries.push_back(BracketPairGuideOverlayCacheEntry {
            key: guide_key.clone(),
            guides: vec![BracketPairGuide {
                open_idx: 0,
                close_idx: 8,
                depth: 0,
            }],
        });

        assert_eq!(lookup_guides(&mut guide_entries, &guide_key), None);
        assert!(guide_entries.is_empty());

        let match_key = BracketMatchOverlayCacheKey {
            buffer_id: 22,
            version: 1,
            len_chars: 4,
            language: LanguageId::PlainText,
            cursors: vec![0],
            mode: EditorMatchBrackets::Near,
        };
        let mut match_entries = VecDeque::new();
        match_entries.push_back(BracketMatchOverlayCacheEntry {
            key: match_key,
            matches: vec![(0, 8)],
        });
        let match_cursor = [0];
        let match_lookup_key = BracketMatchOverlayCacheLookupKey {
            buffer_id: 22,
            version: 1,
            len_chars: 4,
            language: LanguageId::PlainText,
            cursors: &match_cursor,
            mode: EditorMatchBrackets::Near,
        };

        assert_eq!(lookup_matches(&mut match_entries, match_lookup_key), None);
        assert!(match_entries.is_empty());
    }

    #[test]
    fn bracket_match_cursor_key_is_sorted_and_deduped() {
        let cursors = ordered_cursor_indices(
            &[
                Selection::caret(12),
                Selection::caret(4),
                Selection::caret(4),
                Selection::caret(9),
            ],
            99,
        );

        assert_eq!(cursors, vec![4, 9, 12]);
    }

    #[test]
    fn bracket_match_cursor_key_clamps_stale_cursor_positions() {
        let cursors = ordered_cursor_indices(
            &[
                Selection::caret(12),
                Selection::caret(4),
                Selection::caret(9),
            ],
            8,
        );

        assert_eq!(cursors, vec![4, 8]);
    }
}
