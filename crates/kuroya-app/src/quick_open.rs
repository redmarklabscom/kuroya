use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet, VecDeque},
    path::{Component, Path, PathBuf},
};

#[cfg(not(windows))]
use std::ffi::OsString;

use crate::history::NavigationLocation;
use serde::{Deserialize, Serialize};

#[path = "quick_open/labels.rs"]
mod labels;
#[path = "quick_open/query.rs"]
mod query;
#[path = "quick_open/ranking.rs"]
mod ranking;

#[cfg(test)]
pub(crate) use labels::quick_open_relative_label;
#[cfg(test)]
pub(crate) use labels::quick_open_result_label;
pub(crate) use labels::quick_open_result_label_with_navigation_line_column;
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use labels::{
    is_clean_quick_open_result_label, is_quick_open_format_control, quick_open_decimal_digits,
    quick_open_line_column_suffix_chars, quick_open_path_display_label,
    quick_open_path_display_label_owned, quick_open_relative_label_path_is_clean,
    quick_open_result_label_from_parts, quick_open_result_label_with_navigation,
    sanitized_quick_open_result_label, sanitized_quick_open_result_label_text,
    truncate_quick_open_result_label,
};
use query::quick_open_lowercase;
#[cfg(test)]
pub(crate) use query::{MAX_QUICK_OPEN_QUERY_MEMORY_CHARS, MAX_QUICK_OPEN_QUERY_PATTERN_CHARS};
pub(crate) use query::{
    QuickOpenMatchQuery, QuickOpenQuery, normalize_quick_open_memory_query, parse_line_column,
    parse_quick_open_query, sanitize_quick_open_query_input,
};
#[allow(unused_imports)]
pub(crate) use ranking::QuickOpenMatch;
use ranking::QuickOpenNavigationRankKeys;
#[cfg(test)]
use ranking::{
    QUICK_OPEN_OPEN_FILE_BONUS, QUICK_OPEN_OPEN_FILE_CANDIDATE_LIMIT,
    QUICK_OPEN_OPEN_FILE_SCAN_LIMIT, QuickOpenCandidateRankData, QuickOpenLowercaseMatch,
    QuickOpenRankingBonusContext, quick_open_candidate_beats_result,
    quick_open_empty_query_index_scan_limit, quick_open_empty_query_ranked_results,
    quick_open_for_each_candidate_path, quick_open_lowercase_match_kind,
    quick_open_lowercase_word_start_match, quick_open_open_file_candidates,
    quick_open_unboosted_empty_query_results,
};
#[cfg(test)]
pub(crate) use ranking::{
    quick_open_latest_navigation_locations_by_path, quick_open_match_score,
    quick_open_navigation_target, quick_open_rank_score, quick_open_rank_score_with_navigation,
    quick_open_rank_score_with_open_files, quick_open_ranked_results,
};
pub(crate) use ranking::{
    quick_open_latest_navigation_locations_from_history, quick_open_ranked_results_from_open_paths,
};

pub(crate) const MAX_QUICK_OPEN_RECENT_FILES: usize = 80;
pub(crate) const MAX_QUICK_OPEN_QUERY_MEMORY: usize = 128;
pub(crate) const QUICK_OPEN_RESULT_LIMIT: usize = 80;
pub(crate) const QUICK_OPEN_RESULT_LABEL_MAX_CHARS: usize = 160;
const QUICK_OPEN_INDEX_IDENTITY_SAMPLE_LIMIT: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QuickOpenResult {
    pub(crate) rank_score: i64,
    pub(crate) fuzzy_score: i64,
    pub(crate) path: PathBuf,
    pub(crate) rel: String,
    pub(crate) navigation_line_column: Option<(usize, usize)>,
}

impl Ord for QuickOpenResult {
    fn cmp(&self, other: &Self) -> Ordering {
        self.rank_score
            .cmp(&other.rank_score)
            .then(self.fuzzy_score.cmp(&other.fuzzy_score))
            .then_with(|| other.rel.cmp(&self.rel))
            .then_with(|| other.path.cmp(&self.path))
            .then_with(|| {
                self.navigation_line_column
                    .cmp(&other.navigation_line_column)
            })
    }
}

impl PartialOrd for QuickOpenResult {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QuickOpenIndexFileIdentity {
    files_len: usize,
    samples: Vec<(usize, QuickOpenPathKey)>,
}

impl QuickOpenIndexFileIdentity {
    #[cfg(test)]
    pub(crate) fn files_len(&self) -> usize {
        self.files_len
    }
}

pub(crate) fn quick_open_index_file_identity(files: &[PathBuf]) -> QuickOpenIndexFileIdentity {
    let files_len = files.len();
    let mut samples = Vec::with_capacity(files_len.min(QUICK_OPEN_INDEX_IDENTITY_SAMPLE_LIMIT));
    for index in quick_open_index_identity_sample_indices(files_len) {
        samples.push((index, quick_open_path_key(&files[index])));
    }
    QuickOpenIndexFileIdentity { files_len, samples }
}

fn quick_open_index_identity_sample_indices(files_len: usize) -> Vec<usize> {
    let sample_count = files_len.min(QUICK_OPEN_INDEX_IDENTITY_SAMPLE_LIMIT);
    let mut samples = Vec::with_capacity(sample_count);
    if files_len <= QUICK_OPEN_INDEX_IDENTITY_SAMPLE_LIMIT {
        samples.extend(0..files_len);
        return samples;
    }

    let max_index = files_len - 1;
    for sample in 0..QUICK_OPEN_INDEX_IDENTITY_SAMPLE_LIMIT {
        samples.push(sample * max_index / (QUICK_OPEN_INDEX_IDENTITY_SAMPLE_LIMIT - 1));
    }
    samples
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuickOpenQueryMemoryEntry {
    pub query: String,
    pub path: PathBuf,
    #[serde(default = "default_quick_open_query_memory_uses")]
    pub uses: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QuickOpenResultsCache {
    pub(crate) query_input: String,
    pub(crate) index_generation: u64,
    pub(crate) index_file_identity: QuickOpenIndexFileIdentity,
    pub(crate) recent_files: VecDeque<PathBuf>,
    pub(crate) open_files: Vec<PathBuf>,
    pub(crate) query_memory: VecDeque<QuickOpenQueryMemoryEntry>,
    pub(crate) navigation_back: VecDeque<NavigationLocation>,
    pub(crate) navigation_forward: VecDeque<NavigationLocation>,
    pub(crate) current_navigation_location: Option<NavigationLocation>,
    pub(crate) parsed_query: QuickOpenQuery,
    pub(crate) result_labels: Vec<String>,
    pub(crate) results: Vec<QuickOpenResult>,
}

impl QuickOpenResultsCache {
    fn non_navigation_inputs_match<'a>(
        &self,
        query_input: &str,
        index_generation: u64,
        index_file_identity: &QuickOpenIndexFileIdentity,
        recent_files: &VecDeque<PathBuf>,
        open_files: impl IntoIterator<Item = &'a Path>,
        query_memory: &VecDeque<QuickOpenQueryMemoryEntry>,
    ) -> bool {
        self.query_input == query_input
            && self.index_generation == index_generation
            && self.index_file_identity == *index_file_identity
            && self.recent_files.iter().eq(recent_files.iter())
            && self.open_files.iter().map(PathBuf::as_path).eq(open_files)
            && self.query_memory.iter().eq(query_memory.iter())
    }

    pub(crate) fn matches<'a>(
        &self,
        query_input: &str,
        index_generation: u64,
        index_file_identity: &QuickOpenIndexFileIdentity,
        recent_files: &VecDeque<PathBuf>,
        open_files: impl IntoIterator<Item = &'a Path>,
        query_memory: &VecDeque<QuickOpenQueryMemoryEntry>,
        navigation_back: &VecDeque<NavigationLocation>,
        navigation_forward: &VecDeque<NavigationLocation>,
        current_navigation_location: Option<&NavigationLocation>,
    ) -> bool {
        self.non_navigation_inputs_match(
            query_input,
            index_generation,
            index_file_identity,
            recent_files,
            open_files,
            query_memory,
        ) && self.navigation_back.iter().eq(navigation_back.iter())
            && self.navigation_forward.iter().eq(navigation_forward.iter())
            && self.current_navigation_location.as_ref() == current_navigation_location
    }

    pub(crate) fn ranking_inputs_match<'a>(
        &self,
        query_input: &str,
        index_generation: u64,
        index_file_identity: &QuickOpenIndexFileIdentity,
        recent_files: &VecDeque<PathBuf>,
        open_files: impl IntoIterator<Item = &'a Path>,
        query_memory: &VecDeque<QuickOpenQueryMemoryEntry>,
        navigation_locations: &[NavigationLocation],
    ) -> bool {
        self.non_navigation_inputs_match(
            query_input,
            index_generation,
            index_file_identity,
            recent_files,
            open_files,
            query_memory,
        ) && QuickOpenNavigationRankKeys::from_history(
            &self.navigation_back,
            &self.navigation_forward,
            self.current_navigation_location.as_ref(),
        )
        .matches_locations(navigation_locations)
    }

    pub(crate) fn refresh_navigation_metadata(
        &mut self,
        navigation_back: &VecDeque<NavigationLocation>,
        navigation_forward: &VecDeque<NavigationLocation>,
        current_navigation_location: Option<NavigationLocation>,
        navigation_locations: &[NavigationLocation],
    ) {
        self.navigation_back = navigation_back.clone();
        self.navigation_forward = navigation_forward.clone();
        self.current_navigation_location = current_navigation_location;

        for result in &mut self.results {
            result.navigation_line_column = None;
        }
        quick_open_attach_navigation_line_columns(&mut self.results, navigation_locations);
        let mut result_labels = Vec::with_capacity(self.results.len());
        for result in &self.results {
            result_labels.push(quick_open_result_label_with_navigation_line_column(
                &result.rel,
                &self.parsed_query,
                result.navigation_line_column,
            ));
        }
        self.result_labels = result_labels;
    }
}

fn default_quick_open_query_memory_uses() -> u32 {
    1
}

#[cfg(test)]
pub(crate) fn quick_open_target(
    path: PathBuf,
    query: &QuickOpenQuery,
) -> (PathBuf, Option<(usize, usize)>) {
    (path, query.line.map(|line| (line, query.column)))
}

#[cfg(test)]
pub(crate) fn quick_open_target_with_navigation(
    path: PathBuf,
    query: &QuickOpenQuery,
    navigation_locations: &[NavigationLocation],
) -> (PathBuf, Option<(usize, usize)>) {
    let navigation_line_column = quick_open_navigation_line_column(navigation_locations, &path);
    quick_open_target_with_navigation_line_column(path, query, navigation_line_column)
}

pub(crate) fn quick_open_target_with_navigation_line_column(
    path: PathBuf,
    query: &QuickOpenQuery,
    navigation_line_column: Option<(usize, usize)>,
) -> (PathBuf, Option<(usize, usize)>) {
    let explicit_line_column = query
        .line
        .map(|line| quick_open_normalized_line_column(line, query.column));
    let line_column = explicit_line_column.or_else(|| {
        navigation_line_column.map(|(line, column)| quick_open_normalized_line_column(line, column))
    });
    (path, line_column)
}

pub(crate) fn quick_open_normalized_line_column(line: usize, column: usize) -> (usize, usize) {
    (line.max(1), column.max(1))
}

#[cfg(test)]
fn quick_open_navigation_line_column(
    navigation_locations: &[NavigationLocation],
    path: &Path,
) -> Option<(usize, usize)> {
    quick_open_navigation_target(navigation_locations, path)
        .map(|location| (location.line, location.column))
}

fn quick_open_navigation_line_columns_by_path(
    navigation_locations: &[NavigationLocation],
) -> HashMap<QuickOpenPathKey, (usize, usize)> {
    let mut targets = HashMap::with_capacity(navigation_locations.len());
    for location in navigation_locations.iter().rev() {
        targets
            .entry(quick_open_path_key(&location.path))
            .or_insert(quick_open_normalized_line_column(
                location.line,
                location.column,
            ));
    }
    targets
}

fn quick_open_attach_navigation_line_columns(
    results: &mut [QuickOpenResult],
    navigation_locations: &[NavigationLocation],
) {
    if results.is_empty() || navigation_locations.is_empty() {
        return;
    }

    let targets = quick_open_navigation_line_columns_by_path(navigation_locations);
    for result in results {
        result.navigation_line_column = targets.get(&quick_open_path_key(&result.path)).copied();
    }
}

pub(crate) fn record_quick_open_recent_file(
    recent: &mut VecDeque<PathBuf>,
    path: PathBuf,
    max_entries: usize,
) {
    if max_entries == 0 {
        recent.clear();
        return;
    }
    if let Some(index) = recent
        .iter()
        .position(|entry| quick_open_paths_match(entry, &path))
    {
        recent.remove(index);
    }

    recent.push_front(path);
    while recent.len() > max_entries {
        recent.pop_back();
    }
}

pub(crate) fn record_quick_open_navigation(
    recent: &mut VecDeque<PathBuf>,
    workspace_root: &Path,
    path: &Path,
    max_entries: usize,
) {
    if max_entries == 0 {
        recent.clear();
        return;
    }
    let workspace_root = lexical_normalize_path(workspace_root);
    let Some(path) =
        normalize_quick_open_workspace_path_with_normalized_root(&workspace_root, path)
    else {
        return;
    };
    let entries = std::mem::take(recent);
    *recent = normalize_quick_open_recent_files_with_normalized_root(
        entries,
        &workspace_root,
        max_entries,
    );
    record_quick_open_recent_file(recent, path, max_entries);
}

pub(crate) fn normalize_quick_open_recent_files(
    entries: impl IntoIterator<Item = PathBuf>,
    workspace_root: &Path,
    max_entries: usize,
) -> VecDeque<PathBuf> {
    if max_entries == 0 {
        return VecDeque::new();
    }

    let workspace_root = lexical_normalize_path(workspace_root);
    normalize_quick_open_recent_files_with_normalized_root(entries, &workspace_root, max_entries)
}

fn normalize_quick_open_recent_files_with_normalized_root(
    entries: impl IntoIterator<Item = PathBuf>,
    workspace_root: &Path,
    max_entries: usize,
) -> VecDeque<PathBuf> {
    let entries = entries.into_iter();
    let capacity = entries.size_hint().0.min(max_entries);
    let mut recent = VecDeque::with_capacity(capacity);
    let mut seen = HashSet::with_capacity(capacity);
    for path in entries {
        let Some(path) =
            normalize_quick_open_workspace_path_with_normalized_root(workspace_root, &path)
        else {
            continue;
        };

        if seen.insert(quick_open_path_key(&path)) {
            recent.push_back(path);
        }
        if recent.len() >= max_entries {
            break;
        }
    }
    recent
}

pub(crate) fn normalize_quick_open_workspace_path(
    workspace_root: &Path,
    path: &Path,
) -> Option<PathBuf> {
    let workspace_root = lexical_normalize_path(workspace_root);
    normalize_quick_open_workspace_path_with_normalized_root(&workspace_root, path)
}

fn normalize_quick_open_workspace_path_with_normalized_root(
    workspace_root: &Path,
    path: &Path,
) -> Option<PathBuf> {
    let path = lexical_normalize_path(path);
    quick_open_workspace_contains_normalized_lexically(workspace_root, &path).then_some(path)
}

fn lexical_normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                let can_pop = matches!(
                    normalized.components().next_back(),
                    Some(Component::Normal(_))
                );
                if can_pop {
                    normalized.pop();
                } else if !normalized.has_root() {
                    normalized.push("..");
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
}

fn quick_open_workspace_relative_path(workspace_root: &Path, path: &Path) -> Option<PathBuf> {
    let workspace_root = lexical_normalize_path(workspace_root);
    let path = lexical_normalize_path(path);
    if let Ok(relative) = path.strip_prefix(&workspace_root) {
        return Some(relative.to_path_buf());
    }

    #[cfg(windows)]
    {
        if !quick_open_workspace_contains_normalized_lexically(&workspace_root, &path) {
            return None;
        }

        let root_components = workspace_root.components().count();
        let mut relative = PathBuf::new();
        for component in path.components().skip(root_components) {
            relative.push(component.as_os_str());
        }
        Some(relative)
    }

    #[cfg(not(windows))]
    {
        None
    }
}

#[cfg(windows)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct QuickOpenPathKey {
    prefix: Option<String>,
    rooted: bool,
    components: Vec<String>,
}

#[cfg(not(windows))]
type QuickOpenPathKey = PathBuf;

#[cfg(windows)]
type QuickOpenFileNameKey = String;

#[cfg(not(windows))]
type QuickOpenFileNameKey = OsString;

#[cfg(windows)]
fn quick_open_path_key(path: &Path) -> QuickOpenPathKey {
    let path = lexical_normalize_path(path);
    quick_open_normalized_path_key(&path)
}

#[cfg(windows)]
fn quick_open_normalized_path_key(path: &Path) -> QuickOpenPathKey {
    let component_capacity = path.components().size_hint().0;
    let mut key = QuickOpenPathKey {
        prefix: None,
        rooted: false,
        components: Vec::with_capacity(component_capacity),
    };
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => {
                let prefix = prefix.as_os_str().to_string_lossy();
                key.prefix = Some(quick_open_lowercase(prefix.as_ref()));
            }
            Component::RootDir => key.rooted = true,
            Component::CurDir => {}
            Component::ParentDir => key.components.push("..".to_owned()),
            Component::Normal(component) => {
                let component = component.to_string_lossy();
                key.components
                    .push(quick_open_lowercase(component.as_ref()));
            }
        }
    }
    key
}

#[cfg(not(windows))]
fn quick_open_path_key(path: &Path) -> QuickOpenPathKey {
    let path = lexical_normalize_path(path);
    quick_open_normalized_path_key(&path)
}

#[cfg(not(windows))]
fn quick_open_normalized_path_key(path: &Path) -> QuickOpenPathKey {
    path.to_path_buf()
}

#[cfg(windows)]
fn quick_open_file_name_key(path: &Path) -> Option<QuickOpenFileNameKey> {
    let file_name = path.file_name()?.to_string_lossy();
    Some(quick_open_lowercase(file_name.as_ref()))
}

#[cfg(not(windows))]
fn quick_open_file_name_key(path: &Path) -> Option<QuickOpenFileNameKey> {
    Some(path.file_name()?.to_os_string())
}

pub(crate) fn quick_open_paths_match(left: &Path, right: &Path) -> bool {
    quick_open_path_key(left) == quick_open_path_key(right)
}

fn quick_open_insert_unseen_path_key(
    seen: &mut HashSet<QuickOpenPathKey>,
    path_key: &QuickOpenPathKey,
) -> bool {
    seen.insert(path_key.clone())
}

fn quick_open_workspace_contains_normalized_lexically(workspace_root: &Path, path: &Path) -> bool {
    if workspace_root.as_os_str().is_empty() {
        return quick_open_empty_root_contains_lexically(path);
    }

    #[cfg(windows)]
    {
        let root = quick_open_normalized_path_key(workspace_root);
        let path = quick_open_normalized_path_key(path);
        root.prefix == path.prefix
            && root.rooted == path.rooted
            && path.components.starts_with(&root.components)
    }

    #[cfg(not(windows))]
    {
        path.starts_with(workspace_root)
    }
}

fn quick_open_empty_root_contains_lexically(path: &Path) -> bool {
    !matches!(
        path.components().next(),
        Some(Component::Prefix(_) | Component::RootDir | Component::ParentDir)
    )
}

pub(crate) fn record_quick_open_query_memory(
    memory: &mut VecDeque<QuickOpenQueryMemoryEntry>,
    workspace_root: &Path,
    query: &str,
    path: &Path,
    max_entries: usize,
) {
    if max_entries == 0 {
        memory.clear();
        return;
    }
    let workspace_root = lexical_normalize_path(workspace_root);
    let Some(path) =
        normalize_quick_open_workspace_path_with_normalized_root(&workspace_root, path)
    else {
        return;
    };
    let Some(query) = normalize_quick_open_memory_query(query) else {
        return;
    };
    let entries = std::mem::take(memory);
    *memory = normalize_quick_open_query_memory_with_normalized_root(
        entries,
        &workspace_root,
        max_entries,
    );
    let mut uses = 1;
    if let Some(index) = memory
        .iter()
        .position(|entry| entry.query == query && quick_open_paths_match(&entry.path, &path))
    {
        if let Some(entry) = memory.remove(index) {
            uses = entry.uses.saturating_add(1).max(1);
        }
    }

    memory.push_front(QuickOpenQueryMemoryEntry { query, path, uses });
    while memory.len() > max_entries {
        memory.pop_back();
    }
}

pub(crate) fn normalize_quick_open_query_memory(
    entries: impl IntoIterator<Item = QuickOpenQueryMemoryEntry>,
    workspace_root: &Path,
    max_entries: usize,
) -> VecDeque<QuickOpenQueryMemoryEntry> {
    if max_entries == 0 {
        return VecDeque::new();
    }

    let workspace_root = lexical_normalize_path(workspace_root);
    normalize_quick_open_query_memory_with_normalized_root(entries, &workspace_root, max_entries)
}

fn normalize_quick_open_query_memory_with_normalized_root(
    entries: impl IntoIterator<Item = QuickOpenQueryMemoryEntry>,
    workspace_root: &Path,
    max_entries: usize,
) -> VecDeque<QuickOpenQueryMemoryEntry> {
    let entries = entries.into_iter();
    let capacity = entries.size_hint().0.min(max_entries);
    let mut memory: VecDeque<QuickOpenQueryMemoryEntry> = VecDeque::with_capacity(capacity);
    for entry in entries {
        let Some(path) =
            normalize_quick_open_workspace_path_with_normalized_root(workspace_root, &entry.path)
        else {
            continue;
        };
        let Some(query) = normalize_quick_open_memory_query(&entry.query) else {
            continue;
        };
        if let Some(existing) = memory.iter_mut().find(|existing| {
            existing.query == query && quick_open_paths_match(&existing.path, &path)
        }) {
            existing.uses = existing.uses.max(entry.uses.max(1));
            continue;
        }
        memory.push_back(QuickOpenQueryMemoryEntry {
            query,
            path,
            uses: entry.uses.max(1),
        });
        if memory.len() >= max_entries {
            break;
        }
    }
    memory
}

#[cfg(test)]
#[path = "quick_open/tests.rs"]
mod tests;
