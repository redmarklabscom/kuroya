use crate::{
    path_display::{display_path_label_cow, sanitized_display_label_cow},
    workspace_state::paths_match_lexically,
};
use std::{
    borrow::Cow,
    collections::VecDeque,
    fmt,
    path::{Path, PathBuf},
};

pub(crate) const MAX_LSP_HOVER_CACHE_ENTRIES: usize = 128;
const MAX_LSP_HOVER_CACHE_CONTENT_BYTES: usize = 64 * 1024;
const LSP_HOVER_CACHE_CONTENT_LABEL_MAX_CHARS: usize = 160;
const LSP_HOVER_CACHE_CONTENT_LABEL_FALLBACK: &str = "hover";

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct LspHoverCacheKey {
    pub(crate) path: PathBuf,
    pub(crate) version: u64,
    pub(crate) line: usize,
    pub(crate) column: usize,
}

impl LspHoverCacheKey {
    pub(crate) fn new(path: PathBuf, version: u64, line: usize, column: usize) -> Self {
        Self {
            path,
            version,
            line,
            column,
        }
    }
}

impl fmt::Debug for LspHoverCacheKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LspHoverCacheKey")
            .field("path_label", &display_path_label_cow(&self.path))
            .field("version", &self.version)
            .field("line", &self.line)
            .field("column", &self.column)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct LspHoverCacheEntry {
    key: LspHoverCacheKey,
    contents: String,
}

impl fmt::Debug for LspHoverCacheEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let contents_label = hover_cache_contents_label_cow(&self.contents);
        f.debug_struct("LspHoverCacheEntry")
            .field("key", &self.key)
            .field("contents_label", &contents_label)
            .field("contents_bytes", &self.contents.len())
            .finish()
    }
}

#[cfg(test)]
pub(crate) fn lookup_hover_cache(
    cache: &VecDeque<LspHoverCacheEntry>,
    key: &LspHoverCacheKey,
) -> Option<String> {
    hover_cache_lookup_index(cache, key).map(|index| cache[index].contents.clone())
}

pub(crate) fn lookup_hover_cache_refresh(
    cache: &mut VecDeque<LspHoverCacheEntry>,
    key: &LspHoverCacheKey,
) -> Option<String> {
    if cache.is_empty() {
        return None;
    }

    trim_hover_cache_to_limit(cache, MAX_LSP_HOVER_CACHE_ENTRIES);
    let newest_version = newest_hover_cache_location_version(cache, key)?;
    if newest_version > key.version {
        remove_stale_hover_cache_entries_for_location(cache, key, newest_version);
        return None;
    }
    remove_stale_hover_cache_entries_for_key(cache, key);

    let index = hover_cache_lookup_index(cache, key)?;
    if index + 1 == cache.len() {
        return cache.back().map(|entry| entry.contents.clone());
    }

    let entry = cache.remove(index)?;
    let contents = entry.contents.clone();
    cache.push_back(entry);
    Some(contents)
}

pub(crate) fn store_hover_cache(
    cache: &mut VecDeque<LspHoverCacheEntry>,
    key: LspHoverCacheKey,
    contents: String,
    max_entries: usize,
) {
    let max_entries = max_entries.min(MAX_LSP_HOVER_CACHE_ENTRIES);
    if max_entries == 0 {
        cache.clear();
        return;
    }
    trim_hover_cache_to_limit(cache, max_entries);

    if let Some(newest_version) = newest_hover_cache_location_version(cache, &key) {
        if newest_version > key.version {
            remove_stale_hover_cache_entries_for_location(cache, &key, newest_version);
            trim_hover_cache_to_limit(cache, max_entries);
            return;
        }
    }

    remove_stale_hover_cache_entries_for_key(cache, &key);
    if contents.len() > MAX_LSP_HOVER_CACHE_CONTENT_BYTES {
        remove_hover_cache_entries_for_location(cache, &key);
        return;
    }

    if let Some(entry) = cache
        .back_mut()
        .filter(|entry| hover_cache_key_matches(&entry.key, &key))
    {
        entry.key = key;
        entry.contents = contents;
        while cache.len() > max_entries {
            cache.pop_front();
        }
        return;
    }

    if let Some(index) = hover_cache_lookup_index(cache, &key) {
        cache.remove(index);
    }

    trim_hover_cache_before_push(cache, max_entries);
    cache.push_back(LspHoverCacheEntry { key, contents });
}

pub(crate) fn remove_hover_cache_entries_for_path(
    cache: &mut VecDeque<LspHoverCacheEntry>,
    path: &Path,
) -> usize {
    let before = cache.len();
    cache.retain(|entry| !hover_cache_path_matches(entry.key.path.as_path(), path));
    before.saturating_sub(cache.len())
}

fn trim_hover_cache_before_push(cache: &mut VecDeque<LspHoverCacheEntry>, max_entries: usize) {
    while cache.len() >= max_entries {
        cache.pop_front();
    }
}

fn trim_hover_cache_to_limit(cache: &mut VecDeque<LspHoverCacheEntry>, max_entries: usize) {
    while cache.len() > max_entries {
        cache.pop_front();
    }
}

fn newest_hover_cache_location_version(
    cache: &VecDeque<LspHoverCacheEntry>,
    key: &LspHoverCacheKey,
) -> Option<u64> {
    cache
        .iter()
        .filter(|entry| hover_cache_key_matches_location(&entry.key, key))
        .map(|entry| entry.key.version)
        .max()
}

fn remove_stale_hover_cache_entries_for_key(
    cache: &mut VecDeque<LspHoverCacheEntry>,
    key: &LspHoverCacheKey,
) -> usize {
    let before = cache.len();
    cache.retain(|entry| {
        !hover_cache_key_matches_location(&entry.key, key) || entry.key.version == key.version
    });
    before.saturating_sub(cache.len())
}

fn remove_stale_hover_cache_entries_for_location(
    cache: &mut VecDeque<LspHoverCacheEntry>,
    key: &LspHoverCacheKey,
    newest_version: u64,
) -> usize {
    let before = cache.len();
    cache.retain(|entry| {
        !hover_cache_key_matches_location(&entry.key, key) || entry.key.version >= newest_version
    });
    before.saturating_sub(cache.len())
}

fn remove_hover_cache_entries_for_location(
    cache: &mut VecDeque<LspHoverCacheEntry>,
    key: &LspHoverCacheKey,
) -> usize {
    let before = cache.len();
    cache.retain(|entry| !hover_cache_key_matches_location(&entry.key, key));
    before.saturating_sub(cache.len())
}

fn hover_cache_lookup_index(
    cache: &VecDeque<LspHoverCacheEntry>,
    key: &LspHoverCacheKey,
) -> Option<usize> {
    let mut lexical_match = None;

    for (index, entry) in cache.iter().enumerate().rev() {
        if &entry.key == key {
            return Some(index);
        }

        if lexical_match.is_none() && hover_cache_key_matches_lexically(&entry.key, key) {
            lexical_match = Some(index);
        }
    }

    lexical_match
}

fn hover_cache_key_matches(left: &LspHoverCacheKey, right: &LspHoverCacheKey) -> bool {
    left == right || hover_cache_key_matches_lexically(left, right)
}

fn hover_cache_key_matches_location(left: &LspHoverCacheKey, right: &LspHoverCacheKey) -> bool {
    left.line == right.line
        && left.column == right.column
        && (left.path == right.path || paths_match_lexically(&left.path, &right.path))
}

fn hover_cache_key_matches_lexically(left: &LspHoverCacheKey, right: &LspHoverCacheKey) -> bool {
    left.version == right.version
        && left.line == right.line
        && left.column == right.column
        && paths_match_lexically(&left.path, &right.path)
}

fn hover_cache_path_matches(left: &Path, right: &Path) -> bool {
    left == right || paths_match_lexically(left, right)
}

fn hover_cache_contents_label_cow(contents: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(
        contents,
        LSP_HOVER_CACHE_CONTENT_LABEL_MAX_CHARS,
        LSP_HOVER_CACHE_CONTENT_LABEL_FALLBACK,
    )
}

#[cfg(test)]
fn hover_cache_contents_label(contents: &str) -> String {
    hover_cache_contents_label_cow(contents).into_owned()
}

#[cfg(test)]
mod tests {
    use super::{
        LSP_HOVER_CACHE_CONTENT_LABEL_MAX_CHARS, LspHoverCacheEntry, LspHoverCacheKey,
        MAX_LSP_HOVER_CACHE_CONTENT_BYTES, MAX_LSP_HOVER_CACHE_ENTRIES, hover_cache_contents_label,
        hover_cache_contents_label_cow, lookup_hover_cache, lookup_hover_cache_refresh,
        remove_hover_cache_entries_for_path, store_hover_cache,
    };
    use std::{borrow::Cow, collections::VecDeque, path::PathBuf};

    fn key(version: u64, line: usize, column: usize) -> LspHoverCacheKey {
        LspHoverCacheKey::new(PathBuf::from("src/main.rs"), version, line, column)
    }

    fn path_key(
        path: impl Into<PathBuf>,
        version: u64,
        line: usize,
        column: usize,
    ) -> LspHoverCacheKey {
        LspHoverCacheKey::new(path.into(), version, line, column)
    }

    #[test]
    fn hover_cache_matches_path_version_and_position() {
        let mut cache = VecDeque::new();
        store_hover_cache(
            &mut cache,
            key(7, 3, 4),
            "cached hover".to_owned(),
            MAX_LSP_HOVER_CACHE_ENTRIES,
        );

        assert_eq!(
            lookup_hover_cache(&cache, &key(7, 3, 4)).as_deref(),
            Some("cached hover")
        );
        assert!(lookup_hover_cache(&cache, &key(8, 3, 4)).is_none());
        assert!(lookup_hover_cache(&cache, &key(7, 3, 5)).is_none());
    }

    #[test]
    fn hover_cache_replaces_existing_entries_and_stays_bounded() {
        let mut cache = VecDeque::new();
        store_hover_cache(&mut cache, key(1, 0, 0), "first".to_owned(), 2);
        store_hover_cache(&mut cache, key(1, 1, 0), "second".to_owned(), 2);
        store_hover_cache(&mut cache, key(1, 0, 0), "updated".to_owned(), 2);
        store_hover_cache(&mut cache, key(1, 2, 0), "third".to_owned(), 2);

        assert_eq!(cache.len(), 2);
        assert!(lookup_hover_cache(&cache, &key(1, 1, 0)).is_none());
        assert_eq!(
            lookup_hover_cache(&cache, &key(1, 0, 0)).as_deref(),
            Some("updated")
        );
        assert_eq!(
            lookup_hover_cache(&cache, &key(1, 2, 0)).as_deref(),
            Some("third")
        );
    }

    #[test]
    fn hover_cache_clamps_caller_limit_to_hard_cap() {
        let mut cache = VecDeque::new();
        for line in 0..(MAX_LSP_HOVER_CACHE_ENTRIES + 8) {
            store_hover_cache(
                &mut cache,
                key(1, line, 0),
                format!("hover {line}"),
                usize::MAX,
            );
        }

        assert_eq!(cache.len(), MAX_LSP_HOVER_CACHE_ENTRIES);
        assert!(lookup_hover_cache(&cache, &key(1, 0, 0)).is_none());
        let newest_hover = format!("hover {}", MAX_LSP_HOVER_CACHE_ENTRIES + 7);
        assert_eq!(
            lookup_hover_cache(&cache, &key(1, MAX_LSP_HOVER_CACHE_ENTRIES + 7, 0)),
            Some(newest_hover)
        );
    }

    #[test]
    fn hover_cache_rejects_stale_store_for_same_location() {
        let mut cache = VecDeque::new();
        store_hover_cache(
            &mut cache,
            key(3, 0, 0),
            "current hover".to_owned(),
            MAX_LSP_HOVER_CACHE_ENTRIES,
        );
        store_hover_cache(
            &mut cache,
            key(2, 0, 0),
            "stale hover".to_owned(),
            MAX_LSP_HOVER_CACHE_ENTRIES,
        );

        assert_eq!(cache.len(), 1);
        assert_eq!(
            lookup_hover_cache(&cache, &key(3, 0, 0)).as_deref(),
            Some("current hover")
        );
        assert!(lookup_hover_cache(&cache, &key(2, 0, 0)).is_none());
    }

    #[test]
    fn hover_cache_refresh_prunes_stale_same_location_entries() {
        let mut cache = VecDeque::from([
            LspHoverCacheEntry {
                key: key(1, 0, 0),
                contents: "old hover".to_owned(),
            },
            LspHoverCacheEntry {
                key: key(2, 0, 0),
                contents: "new hover".to_owned(),
            },
        ]);

        assert!(lookup_hover_cache_refresh(&mut cache, &key(1, 0, 0)).is_none());

        assert_eq!(cache.len(), 1);
        assert_eq!(
            lookup_hover_cache(&cache, &key(2, 0, 0)).as_deref(),
            Some("new hover")
        );
    }

    #[test]
    fn hover_cache_preserves_raw_hover_data_and_skips_oversized_bodies() {
        let mut cache = VecDeque::new();
        let raw_hover = "fn main()\n\u{202e}raw hover body".to_owned();
        store_hover_cache(
            &mut cache,
            key(1, 0, 0),
            raw_hover.clone(),
            MAX_LSP_HOVER_CACHE_ENTRIES,
        );

        assert_eq!(
            lookup_hover_cache(&cache, &key(1, 0, 0)).as_deref(),
            Some(raw_hover.as_str())
        );

        let label = hover_cache_contents_label_cow(&raw_hover);
        assert_ne!(label.as_ref(), raw_hover);
        assert!(!label.contains('\n'));
        assert!(!label.contains('\u{202e}'));

        store_hover_cache(
            &mut cache,
            key(1, 0, 0),
            "x".repeat(MAX_LSP_HOVER_CACHE_CONTENT_BYTES + 1),
            MAX_LSP_HOVER_CACHE_ENTRIES,
        );

        assert!(lookup_hover_cache(&cache, &key(1, 0, 0)).is_none());
    }

    #[test]
    fn hover_cache_contents_label_cow_borrows_clean_ascii_and_unicode() {
        assert!(matches!(
            hover_cache_contents_label_cow("fn main() -> usize"),
            Cow::Borrowed("fn main() -> usize")
        ));

        let unicode = "hover docs for \u{03bb}";
        match hover_cache_contents_label_cow(unicode) {
            Cow::Borrowed(label) => assert_eq!(label, unicode),
            Cow::Owned(label) => panic!("expected borrowed hover label, got {label:?}"),
        }
    }

    #[test]
    fn hover_cache_contents_label_cow_owns_dirty_truncated_and_fallback_contents() {
        let dirty = hover_cache_contents_label_cow("first line\n\u{202e}last line");
        assert_eq!(dirty.as_ref(), "first line last line");
        assert!(matches!(dirty, Cow::Owned(_)));

        let long = format!(
            "hover-start-{}-hover-finish",
            "x".repeat(LSP_HOVER_CACHE_CONTENT_LABEL_MAX_CHARS * 2)
        );
        let truncated = hover_cache_contents_label_cow(&long);
        assert!(truncated.contains("..."));
        assert!(truncated.chars().count() <= LSP_HOVER_CACHE_CONTENT_LABEL_MAX_CHARS);
        assert!(matches!(truncated, Cow::Owned(_)));

        let fallback = hover_cache_contents_label_cow("\n\u{202e}");
        assert_eq!(fallback.as_ref(), "hover");
        assert!(matches!(fallback, Cow::Owned(_)));
    }

    #[test]
    fn hover_cache_debug_uses_sanitized_bounded_labels_without_changing_raw_hover() {
        let mut cache = VecDeque::new();
        let raw_hover = format!(
            "first line\n{}\u{202e}last line",
            "very-long-hover-".repeat(LSP_HOVER_CACHE_CONTENT_LABEL_MAX_CHARS)
        );
        store_hover_cache(
            &mut cache,
            path_key("workspace/src/bad\n\u{202e}main.rs", 1, 0, 0),
            raw_hover.clone(),
            MAX_LSP_HOVER_CACHE_ENTRIES,
        );

        assert_eq!(
            lookup_hover_cache(
                &cache,
                &path_key("workspace/src/bad\n\u{202e}main.rs", 1, 0, 0),
            )
            .as_deref(),
            Some(raw_hover.as_str())
        );

        let label = hover_cache_contents_label(&raw_hover);
        assert!(!label.contains('\n'));
        assert!(!label.contains('\u{202e}'));
        assert!(label.contains("..."));
        assert!(label.chars().count() <= LSP_HOVER_CACHE_CONTENT_LABEL_MAX_CHARS);

        let debug = format!("{:?}", cache.back().expect("cached hover"));
        assert!(debug.contains("contents_label"));
        assert!(debug.contains("contents_bytes"));
        assert!(!debug.contains("\\n"));
        assert!(!debug.contains('\u{202e}'));
    }

    #[test]
    fn hover_cache_entries_can_be_removed_by_path() {
        let mut cache = VecDeque::new();
        let main = PathBuf::from("src/main.rs");
        let other = PathBuf::from("src/lib.rs");
        let main_key = LspHoverCacheKey::new(main.clone(), 1, 0, 0);
        let other_key = LspHoverCacheKey::new(other.clone(), 1, 0, 0);
        store_hover_cache(
            &mut cache,
            main_key.clone(),
            "main".to_owned(),
            MAX_LSP_HOVER_CACHE_ENTRIES,
        );
        store_hover_cache(
            &mut cache,
            other_key.clone(),
            "other".to_owned(),
            MAX_LSP_HOVER_CACHE_ENTRIES,
        );

        assert_eq!(remove_hover_cache_entries_for_path(&mut cache, &main), 1);

        assert!(lookup_hover_cache(&cache, &main_key).is_none());
        assert_eq!(
            lookup_hover_cache(&cache, &other_key).as_deref(),
            Some("other")
        );
    }

    #[test]
    fn hover_cache_reuses_lexically_equivalent_paths() {
        let mut cache = VecDeque::new();
        let main = PathBuf::from("workspace/src/main.rs");
        let equivalent_main = PathBuf::from("workspace/src/../src/main.rs");

        store_hover_cache(
            &mut cache,
            path_key(main, 7, 3, 4),
            "cached hover".to_owned(),
            MAX_LSP_HOVER_CACHE_ENTRIES,
        );

        assert_eq!(
            lookup_hover_cache(&cache, &path_key(equivalent_main, 7, 3, 4)).as_deref(),
            Some("cached hover")
        );
        assert!(lookup_hover_cache(&cache, &path_key("workspace/src/main.rs", 8, 3, 4)).is_none());
        assert!(lookup_hover_cache(&cache, &path_key("workspace/src/main.rs", 7, 3, 5)).is_none());
    }

    #[test]
    fn hover_cache_prefers_older_exact_match_over_newer_lexical_fallback() {
        let exact_key = path_key("workspace/src/main.rs", 1, 0, 0);
        let lexical_key = path_key("workspace/src/../src/main.rs", 1, 0, 0);
        let cache = VecDeque::from([
            LspHoverCacheEntry {
                key: exact_key.clone(),
                contents: "exact".to_owned(),
            },
            LspHoverCacheEntry {
                key: lexical_key,
                contents: "lexical".to_owned(),
            },
        ]);

        assert_eq!(
            lookup_hover_cache(&cache, &exact_key).as_deref(),
            Some("exact")
        );
    }

    #[test]
    fn hover_cache_refreshes_lexical_hit_before_eviction() {
        let mut cache = VecDeque::new();
        store_hover_cache(
            &mut cache,
            path_key("workspace/src/main.rs", 1, 0, 0),
            "main".to_owned(),
            2,
        );
        store_hover_cache(
            &mut cache,
            path_key("workspace/src/lib.rs", 1, 0, 0),
            "lib".to_owned(),
            2,
        );

        assert_eq!(
            lookup_hover_cache_refresh(
                &mut cache,
                &path_key("workspace/src/../src/main.rs", 1, 0, 0),
            )
            .as_deref(),
            Some("main")
        );
        store_hover_cache(
            &mut cache,
            path_key("workspace/src/other.rs", 1, 0, 0),
            "other".to_owned(),
            2,
        );

        assert!(lookup_hover_cache(&cache, &path_key("workspace/src/lib.rs", 1, 0, 0)).is_none());
        assert_eq!(
            lookup_hover_cache(&cache, &path_key("workspace/src/main.rs", 1, 0, 0)).as_deref(),
            Some("main")
        );
    }

    #[test]
    fn hover_cache_store_replaces_lexically_equivalent_entry() {
        let mut cache = VecDeque::new();
        store_hover_cache(
            &mut cache,
            path_key("workspace/src/main.rs", 1, 0, 0),
            "old".to_owned(),
            MAX_LSP_HOVER_CACHE_ENTRIES,
        );
        store_hover_cache(
            &mut cache,
            path_key("workspace/src/../src/main.rs", 1, 0, 0),
            "new".to_owned(),
            MAX_LSP_HOVER_CACHE_ENTRIES,
        );

        assert_eq!(cache.len(), 1);
        assert_eq!(
            lookup_hover_cache(&cache, &path_key("workspace/src/main.rs", 1, 0, 0)).as_deref(),
            Some("new")
        );
    }

    #[test]
    fn hover_cache_removes_lexically_equivalent_path_entries() {
        let mut cache = VecDeque::new();
        store_hover_cache(
            &mut cache,
            path_key("workspace/src/../src/main.rs", 1, 0, 0),
            "main".to_owned(),
            MAX_LSP_HOVER_CACHE_ENTRIES,
        );
        store_hover_cache(
            &mut cache,
            path_key("workspace/src/lib.rs", 1, 0, 0),
            "lib".to_owned(),
            MAX_LSP_HOVER_CACHE_ENTRIES,
        );

        assert_eq!(
            remove_hover_cache_entries_for_path(
                &mut cache,
                PathBuf::from("workspace/src/main.rs").as_path()
            ),
            1
        );

        assert!(lookup_hover_cache(&cache, &path_key("workspace/src/main.rs", 1, 0, 0)).is_none());
        assert_eq!(
            lookup_hover_cache(&cache, &path_key("workspace/src/lib.rs", 1, 0, 0)).as_deref(),
            Some("lib")
        );
    }

    #[test]
    fn hover_cache_refreshes_recency_on_mutable_lookup() {
        let mut cache = VecDeque::new();
        store_hover_cache(&mut cache, key(1, 0, 0), "first".to_owned(), 2);
        store_hover_cache(&mut cache, key(1, 1, 0), "second".to_owned(), 2);

        assert_eq!(
            lookup_hover_cache_refresh(&mut cache, &key(1, 0, 0)).as_deref(),
            Some("first")
        );
        store_hover_cache(&mut cache, key(1, 2, 0), "third".to_owned(), 2);

        assert!(lookup_hover_cache(&cache, &key(1, 1, 0)).is_none());
        assert_eq!(
            lookup_hover_cache(&cache, &key(1, 0, 0)).as_deref(),
            Some("first")
        );
        assert_eq!(
            lookup_hover_cache(&cache, &key(1, 2, 0)).as_deref(),
            Some("third")
        );
    }
}
