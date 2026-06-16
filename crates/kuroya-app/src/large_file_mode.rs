use kuroya_core::{BufferId, TextBuffer};
use std::collections::{HashMap, hash_map::Entry};

pub(crate) const LARGE_FILE_MODE_MAX_BYTES: usize = 2 * 1024 * 1024;
pub(crate) const LARGE_FILE_MODE_MAX_LINES: usize = 60_000;
pub(crate) const LARGE_FILE_PERFORMANCE_MODE_MAX_BYTES: usize = 1024 * 1024;
pub(crate) const LARGE_FILE_PERFORMANCE_MODE_MAX_LINES: usize = 20_000;
pub(crate) const LARGE_FILE_MODE_LINE_RENDER_CHAR_LIMIT: usize = 10_000;
const LINE_RENDER_PROTECTION_CACHE_MAX_ENTRIES: usize = 1024;

pub(crate) fn buffer_uses_large_file_mode(buffer: &TextBuffer) -> bool {
    buffer.len_bytes() > LARGE_FILE_MODE_MAX_BYTES || buffer.len_lines() > LARGE_FILE_MODE_MAX_LINES
}

pub(crate) fn buffer_uses_large_file_performance_mode(buffer: &TextBuffer) -> bool {
    buffer_uses_large_file_mode(buffer)
        || buffer.len_bytes() > LARGE_FILE_PERFORMANCE_MODE_MAX_BYTES
        || buffer.len_lines() > LARGE_FILE_PERFORMANCE_MODE_MAX_LINES
}

pub(crate) fn buffer_needs_line_render_protection(buffer: &TextBuffer) -> bool {
    buffer_uses_large_file_mode(buffer)
        || buffer.len_bytes() > LARGE_FILE_MODE_LINE_RENDER_CHAR_LIMIT
}

pub(crate) fn buffer_needs_bracket_scan_protection(buffer: &TextBuffer) -> bool {
    buffer_uses_large_file_mode(buffer)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LineRenderProtectionCacheEntry {
    version: u64,
    len_bytes: usize,
    len_lines: usize,
    needs_protection: bool,
}

impl LineRenderProtectionCacheEntry {
    fn for_buffer(buffer: &TextBuffer) -> Self {
        Self {
            version: buffer.version(),
            len_bytes: buffer.len_bytes(),
            len_lines: buffer.len_lines(),
            needs_protection: buffer_needs_line_render_protection(buffer),
        }
    }

    fn matches_buffer(&self, buffer: &TextBuffer) -> bool {
        self.version == buffer.version()
            && self.len_bytes == buffer.len_bytes()
            && self.len_lines == buffer.len_lines()
    }
}

pub(crate) fn buffer_needs_line_render_protection_cached(
    cache: &mut HashMap<BufferId, LineRenderProtectionCacheEntry>,
    buffer: &TextBuffer,
) -> bool {
    let id = buffer.id();
    prune_line_render_protection_cache(cache, id);
    match cache.entry(id) {
        Entry::Occupied(entry) if entry.get().matches_buffer(buffer) => {
            entry.get().needs_protection
        }
        entry => {
            let cache_entry = LineRenderProtectionCacheEntry::for_buffer(buffer);
            let needs_protection = cache_entry.needs_protection;
            match entry {
                Entry::Occupied(mut occupied) => {
                    occupied.insert(cache_entry);
                }
                Entry::Vacant(vacant) => {
                    vacant.insert(cache_entry);
                }
            }
            needs_protection
        }
    }
}

fn prune_line_render_protection_cache(
    cache: &mut HashMap<BufferId, LineRenderProtectionCacheEntry>,
    incoming_id: BufferId,
) {
    if cache.contains_key(&incoming_id) {
        return;
    }

    while cache.len() >= LINE_RENDER_PROTECTION_CACHE_MAX_ENTRIES {
        let Some(stale_id) = cache.keys().copied().filter(|id| *id != incoming_id).min() else {
            break;
        };
        cache.remove(&stale_id);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        LARGE_FILE_MODE_LINE_RENDER_CHAR_LIMIT, LARGE_FILE_MODE_MAX_BYTES,
        LARGE_FILE_MODE_MAX_LINES, LARGE_FILE_PERFORMANCE_MODE_MAX_BYTES,
        LARGE_FILE_PERFORMANCE_MODE_MAX_LINES, LINE_RENDER_PROTECTION_CACHE_MAX_ENTRIES,
        buffer_needs_bracket_scan_protection, buffer_needs_line_render_protection,
        buffer_needs_line_render_protection_cached, buffer_uses_large_file_mode,
        buffer_uses_large_file_performance_mode,
    };
    use kuroya_core::TextBuffer;
    use std::collections::HashMap;

    #[test]
    fn large_file_mode_tracks_byte_threshold() {
        let small = TextBuffer::from_text(1, None, "a".repeat(1024));
        let large = TextBuffer::from_text(2, None, "a".repeat(LARGE_FILE_MODE_MAX_BYTES + 1));

        assert!(!buffer_uses_large_file_mode(&small));
        assert!(buffer_uses_large_file_mode(&large));
    }

    #[test]
    fn large_file_mode_tracks_line_threshold() {
        let mut text = "x\n".repeat(LARGE_FILE_MODE_MAX_LINES);
        text.push('x');
        let buffer = TextBuffer::from_text(1, None, text);

        assert!(buffer_uses_large_file_mode(&buffer));
    }

    #[test]
    fn performance_mode_starts_before_hard_large_file_mode() {
        let small = TextBuffer::from_text(1, None, "x\n".repeat(11_000));
        let mut many_lines = "x\n".repeat(LARGE_FILE_PERFORMANCE_MODE_MAX_LINES);
        many_lines.push('x');
        let many_lines = TextBuffer::from_text(2, None, many_lines);
        let many_bytes = TextBuffer::from_text(
            3,
            None,
            "x".repeat(LARGE_FILE_PERFORMANCE_MODE_MAX_BYTES + 1),
        );

        assert!(!buffer_uses_large_file_mode(&small));
        assert!(!buffer_uses_large_file_performance_mode(&small));
        assert!(!buffer_uses_large_file_mode(&many_lines));
        assert!(buffer_uses_large_file_performance_mode(&many_lines));
        assert!(!buffer_uses_large_file_mode(&many_bytes));
        assert!(buffer_uses_large_file_performance_mode(&many_bytes));
    }

    #[test]
    fn line_render_protection_tracks_buffers_that_can_exceed_line_cap() {
        let small =
            TextBuffer::from_text(1, None, "x".repeat(LARGE_FILE_MODE_LINE_RENDER_CHAR_LIMIT));
        let protected = TextBuffer::from_text(
            2,
            None,
            "x".repeat(LARGE_FILE_MODE_LINE_RENDER_CHAR_LIMIT + 1),
        );

        assert!(!buffer_uses_large_file_mode(&protected));
        assert!(!buffer_needs_line_render_protection(&small));
        assert!(buffer_needs_line_render_protection(&protected));
    }

    #[test]
    fn line_render_protection_detects_only_lines_over_cap() {
        let at_cap =
            TextBuffer::from_text(1, None, "x".repeat(LARGE_FILE_MODE_LINE_RENDER_CHAR_LIMIT));
        let over_cap = TextBuffer::from_text(
            2,
            None,
            "x".repeat(LARGE_FILE_MODE_LINE_RENDER_CHAR_LIMIT + 1),
        );

        assert!(!buffer_needs_line_render_protection(&at_cap));
        assert!(buffer_needs_line_render_protection(&over_cap));
    }

    #[test]
    fn line_render_protection_uses_total_byte_guard_for_many_short_lines() {
        let mut text = "short line\n".repeat(1_999);
        text.push_str("short line");
        let buffer = TextBuffer::from_text(1, None, text);

        assert!(buffer.len_bytes() > LARGE_FILE_MODE_LINE_RENDER_CHAR_LIMIT);
        assert!(!buffer_uses_large_file_mode(&buffer));
        assert!(buffer_needs_line_render_protection(&buffer));
        assert!(!buffer_needs_bracket_scan_protection(&buffer));
    }

    #[test]
    fn bracket_scan_protection_only_uses_hard_large_file_mode() {
        let medium = TextBuffer::from_text(
            1,
            None,
            "short line\n".repeat(LARGE_FILE_PERFORMANCE_MODE_MAX_LINES + 1),
        );
        let hard_large = TextBuffer::from_text(2, None, "x".repeat(LARGE_FILE_MODE_MAX_BYTES + 1));

        assert!(buffer_uses_large_file_performance_mode(&medium));
        assert!(!buffer_uses_large_file_mode(&medium));
        assert!(!buffer_needs_bracket_scan_protection(&medium));
        assert!(buffer_uses_large_file_mode(&hard_large));
        assert!(buffer_needs_bracket_scan_protection(&hard_large));
    }

    #[test]
    fn line_render_protection_cache_refreshes_on_buffer_version_change() {
        let mut cache = HashMap::new();
        let mut buffer = TextBuffer::from_text(1, None, "short".to_owned());

        assert!(!buffer_needs_line_render_protection_cached(
            &mut cache, &buffer
        ));

        assert!(buffer.replace_range(
            0..buffer.len_chars(),
            &"x".repeat(LARGE_FILE_MODE_LINE_RENDER_CHAR_LIMIT + 1),
        ));
        assert!(buffer_needs_line_render_protection_cached(
            &mut cache, &buffer
        ));
    }

    #[test]
    fn line_render_protection_cache_rejects_same_id_version_shape_mismatch() {
        let mut cache = HashMap::new();
        let small = TextBuffer::from_text(1, None, "short".to_owned());
        let protected = TextBuffer::from_text(
            1,
            None,
            "x".repeat(LARGE_FILE_MODE_LINE_RENDER_CHAR_LIMIT + 1),
        );

        assert!(!buffer_needs_line_render_protection_cached(
            &mut cache, &small
        ));
        assert!(buffer_needs_line_render_protection_cached(
            &mut cache, &protected
        ));
    }

    #[test]
    fn line_render_protection_cache_is_bounded_for_stale_buffer_churn() {
        let mut cache = HashMap::new();

        for id in 1..=(LINE_RENDER_PROTECTION_CACHE_MAX_ENTRIES as u64 + 5) {
            let buffer = TextBuffer::from_text(id, None, "short".to_owned());
            assert!(!buffer_needs_line_render_protection_cached(
                &mut cache, &buffer
            ));
        }

        assert!(cache.len() <= LINE_RENDER_PROTECTION_CACHE_MAX_ENTRIES);
        assert!(!cache.contains_key(&1));
        assert!(cache.contains_key(&(LINE_RENDER_PROTECTION_CACHE_MAX_ENTRIES as u64 + 5)));
    }
}
