use crate::{ProjectIndex, text_match::AsciiCaseInsensitiveMatcher};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fmt::{self, Write as _},
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

const MAX_SEARCH_PREVIEW_CHARS: usize = 240;
const SEARCH_PREVIEW_CONTEXT_CHARS: usize = 96;
const SEARCH_FILE_CHUNK_SIZE: usize = 256;
const SEARCH_CANCEL_LINE_INTERVAL: usize = 64;
const SEARCH_CANCEL_MATCH_INTERVAL: usize = 64;
const SEARCH_CANCEL_BYTE_INTERVAL: usize = 16 * 1024;
const MAX_SEARCH_QUERY_BYTES: usize = SEARCH_CANCEL_BYTE_INTERVAL;
const PROJECT_SEARCH_INDEX_MAX_TEXT_BYTES: u64 = 256 * 1024 * 1024;
const MAX_SEARCH_FILE_BYTES: u64 = PROJECT_SEARCH_INDEX_MAX_TEXT_BYTES;
const MAX_SEARCH_GLOB_PATTERNS: usize = 1024;
const MAX_SEARCH_GLOB_PATTERN_BYTES: usize = 4096;
const GLOB_ERROR_PATTERN_MAX_CHARS: usize = 120;
const GLOB_ERROR_DETAIL_MAX_CHARS: usize = 240;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchOptions {
    pub query: String,
    pub max_file_bytes: u64,
    pub max_results: usize,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub include_globs: Vec<String>,
    pub exclude_globs: Vec<String>,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            query: String::new(),
            max_file_bytes: 2 * 1024 * 1024,
            max_results: 500,
            case_sensitive: false,
            whole_word: false,
            include_globs: Vec::new(),
            exclude_globs: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    pub path: PathBuf,
    pub line: usize,
    pub column: usize,
    pub preview: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SearchStats {
    pub searched_files: usize,
    pub matched_files: usize,
    pub skipped_large_files: usize,
    pub skipped_binary_files: usize,
    pub skipped_unreadable_files: usize,
    pub skipped_index_budget_files: usize,
}

impl SearchStats {
    pub fn skipped_files(self) -> usize {
        self.skipped_large_files
            .saturating_add(self.skipped_binary_files)
            .saturating_add(self.skipped_unreadable_files)
            .saturating_add(self.skipped_index_budget_files)
    }

    fn merge(&mut self, other: Self) {
        self.searched_files = self.searched_files.saturating_add(other.searched_files);
        self.matched_files = self.matched_files.saturating_add(other.matched_files);
        self.skipped_large_files = self
            .skipped_large_files
            .saturating_add(other.skipped_large_files);
        self.skipped_binary_files = self
            .skipped_binary_files
            .saturating_add(other.skipped_binary_files);
        self.skipped_unreadable_files = self
            .skipped_unreadable_files
            .saturating_add(other.skipped_unreadable_files);
        self.skipped_index_budget_files = self
            .skipped_index_budget_files
            .saturating_add(other.skipped_index_budget_files);
    }
}

#[derive(Debug, Clone, Default)]
pub struct SearchResult {
    pub matches: Vec<SearchMatch>,
    pub truncated: bool,
    pub error: Option<String>,
    pub stats: SearchStats,
}

#[derive(Debug, Clone, Default)]
pub struct ProjectSearchIndex {
    root: PathBuf,
    max_file_bytes: u64,
    files: Vec<ProjectSearchIndexedFile>,
}

#[derive(Debug, Clone)]
struct ProjectSearchIndexedFile {
    path: PathBuf,
    signature: Option<ProjectSearchIndexedFileSignature>,
    content: ProjectSearchIndexedFileContent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProjectSearchIndexedFileSignature {
    len: u64,
    modified_nanos: u128,
    created_nanos: u128,
}

#[derive(Debug, Clone)]
enum ProjectSearchIndexedFileContent {
    Text { text: String, byte_len: u64 },
    TooLarge,
    BinaryOrInvalid,
    Unreadable,
    IndexBudgetExceeded,
}

impl ProjectSearchIndex {
    pub fn build(index: &ProjectIndex, max_file_bytes: u64) -> Self {
        Self::build_with_cancel(index, max_file_bytes, || false)
            .expect("non-cancellable search index build should complete")
    }

    pub fn build_with_cancel(
        index: &ProjectIndex,
        max_file_bytes: u64,
        is_cancelled: impl Fn() -> bool,
    ) -> Option<Self> {
        Self::build_with_text_budget_and_cancel(
            index,
            max_file_bytes,
            PROJECT_SEARCH_INDEX_MAX_TEXT_BYTES,
            is_cancelled,
        )
    }

    pub fn build_with_text_budget(
        index: &ProjectIndex,
        max_file_bytes: u64,
        max_text_bytes: u64,
    ) -> Self {
        Self::build_with_text_budget_and_cancel(index, max_file_bytes, max_text_bytes, || false)
            .expect("non-cancellable search index build should complete")
    }

    pub fn build_with_text_budget_and_cancel(
        index: &ProjectIndex,
        max_file_bytes: u64,
        max_text_bytes: u64,
        is_cancelled: impl Fn() -> bool,
    ) -> Option<Self> {
        let root = index.root().to_path_buf();
        let max_file_bytes = effective_search_file_byte_limit(max_file_bytes);
        let mut indexed_text_bytes = 0u64;
        let source_files = index.files();
        let mut files = Vec::with_capacity(source_files.len());
        for path in source_files {
            if is_cancelled() {
                return None;
            }
            let (signature, content) = read_indexed_search_file_content(
                path,
                max_file_bytes,
                max_text_bytes,
                &mut indexed_text_bytes,
            );
            if is_cancelled() {
                return None;
            }
            files.push(ProjectSearchIndexedFile {
                path: path.clone(),
                signature,
                content,
            });
        }
        Some(Self {
            root,
            max_file_bytes,
            files,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn max_file_bytes(&self) -> u64 {
        self.max_file_bytes
    }
}

pub fn search_project(index: &ProjectIndex, options: &SearchOptions) -> SearchResult {
    search_project_with_cancel(index, options, || false).unwrap_or_default()
}

pub fn search_project_with_cancel(
    index: &ProjectIndex,
    options: &SearchOptions,
    is_cancelled: impl Fn() -> bool,
) -> Option<SearchResult> {
    let prepared = match PreparedProjectSearch::new_with_cancel(options, &is_cancelled)? {
        Ok(Some(prepared)) => prepared,
        Ok(None) => return Some(SearchResult::default()),
        Err(error) => return Some(search_error_result(error)),
    };
    let search_index =
        ProjectSearchIndex::build_with_cancel(index, options.max_file_bytes, &is_cancelled)?;
    search_project_index_prepared(&search_index, options, &prepared, &is_cancelled)
}

pub fn search_project_index(index: &ProjectSearchIndex, options: &SearchOptions) -> SearchResult {
    search_project_index_with_cancel(index, options, || false).unwrap_or_default()
}

pub fn search_project_index_with_cancel(
    index: &ProjectSearchIndex,
    options: &SearchOptions,
    is_cancelled: impl Fn() -> bool,
) -> Option<SearchResult> {
    let prepared = match PreparedProjectSearch::new_with_cancel(options, &is_cancelled)? {
        Ok(Some(prepared)) => prepared,
        Ok(None) => return Some(SearchResult::default()),
        Err(error) => return Some(search_error_result(error)),
    };

    search_project_index_prepared(index, options, &prepared, &is_cancelled)
}

fn search_error_result(error: String) -> SearchResult {
    SearchResult {
        error: Some(error),
        ..SearchResult::default()
    }
}

struct PreparedProjectSearch<'a> {
    line_needle: LineSearchNeedle<'a>,
    include_globs: Option<GlobSet>,
    exclude_globs: Option<GlobSet>,
}

impl<'a> PreparedProjectSearch<'a> {
    #[cfg(test)]
    fn new(options: &'a SearchOptions) -> Result<Option<Self>, String> {
        let never_cancelled = || false;
        match Self::new_with_cancel(options, &never_cancelled) {
            Some(result) => result,
            None => Ok(None),
        }
    }

    fn new_with_cancel(
        options: &'a SearchOptions,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Option<Result<Option<Self>, String>> {
        let needle = options.query.trim();
        if needle.is_empty() {
            return Some(Ok(None));
        }
        if is_cancelled() {
            return None;
        }
        if needle.len() > MAX_SEARCH_QUERY_BYTES {
            return Some(Err(format!(
                "Search query is too long; maximum is {MAX_SEARCH_QUERY_BYTES} bytes"
            )));
        }

        let include_globs = match build_glob_set(&options.include_globs) {
            Ok(globs) => globs,
            Err(error) => return Some(Err(error)),
        };
        if is_cancelled() {
            return None;
        }
        let exclude_globs = match build_glob_set(&options.exclude_globs) {
            Ok(globs) => globs,
            Err(error) => return Some(Err(error)),
        };
        if is_cancelled() {
            return None;
        }
        Some(Ok(Some(Self::from_parts(
            needle,
            options.case_sensitive,
            include_globs,
            exclude_globs,
        ))))
    }

    fn from_parts(
        needle: &'a str,
        case_sensitive: bool,
        include_globs: Option<GlobSet>,
        exclude_globs: Option<GlobSet>,
    ) -> Self {
        Self {
            line_needle: LineSearchNeedle::new(needle, case_sensitive),
            include_globs,
            exclude_globs,
        }
    }
}

fn search_project_index_prepared(
    index: &ProjectSearchIndex,
    options: &SearchOptions,
    prepared: &PreparedProjectSearch<'_>,
    is_cancelled: &dyn Fn() -> bool,
) -> Option<SearchResult> {
    let context = ProjectSearchContext {
        root: index.root(),
        indexed_max_file_bytes: index.max_file_bytes(),
        line_needle: &prepared.line_needle,
        options,
        include_globs: prepared.include_globs.as_ref(),
        exclude_globs: prepared.exclude_globs.as_ref(),
        is_cancelled,
    };

    let mut result_budget = SearchResultBudget::new(options.max_results);
    let mut stats = SearchStats::default();
    let mut truncated = false;
    let mut matches = Vec::with_capacity(result_budget.limit().min(1024));
    for files in index.files.chunks(SEARCH_FILE_CHUNK_SIZE) {
        if (context.is_cancelled)() {
            return None;
        }
        if result_budget.is_exhausted() {
            truncated = true;
            break;
        }
        let result = search_project_index_chunk(&context, files, &mut result_budget, &mut matches)?;
        stats.merge(result.stats);
        truncated |= result.truncated;
    }
    truncated |= matches.len() > options.max_results;
    matches.truncate(options.max_results);

    Some(SearchResult {
        matches,
        truncated,
        error: None,
        stats,
    })
}

fn reserve_search_text_budget(
    indexed_text_bytes: &mut u64,
    bytes: u64,
    max_text_bytes: u64,
) -> bool {
    if max_text_bytes == 0 {
        return true;
    }

    let Some(next) = indexed_text_bytes.checked_add(bytes) else {
        return false;
    };
    if next > max_text_bytes {
        return false;
    }
    *indexed_text_bytes = next;
    true
}

fn read_indexed_search_file_content(
    path: &Path,
    max_file_bytes: u64,
    max_text_bytes: u64,
    indexed_text_bytes: &mut u64,
) -> (
    Option<ProjectSearchIndexedFileSignature>,
    ProjectSearchIndexedFileContent,
) {
    let mut max_file_bytes = effective_search_file_byte_limit(max_file_bytes);
    let mut file = match fs::File::open(path) {
        Ok(file) => file,
        Err(_) => {
            return indexed_search_open_error_content(path, max_file_bytes);
        }
    };
    let metadata = match file.metadata() {
        Ok(metadata) if metadata.len() > max_file_bytes => {
            return (
                Some(ProjectSearchIndexedFileSignature::from_metadata(&metadata)),
                ProjectSearchIndexedFileContent::TooLarge,
            );
        }
        Ok(metadata) => metadata,
        Err(_) => return (None, ProjectSearchIndexedFileContent::Unreadable),
    };
    let signature = Some(ProjectSearchIndexedFileSignature::from_metadata(&metadata));

    if search_text_budget_exhausted(*indexed_text_bytes, max_text_bytes) {
        return (
            signature,
            ProjectSearchIndexedFileContent::IndexBudgetExceeded,
        );
    }
    if let Some(remaining_budget) =
        remaining_search_text_budget(*indexed_text_bytes, max_text_bytes)
    {
        if metadata.len() > remaining_budget {
            return (
                signature,
                ProjectSearchIndexedFileContent::IndexBudgetExceeded,
            );
        }
        max_file_bytes = max_file_bytes.min(remaining_budget);
    }

    let content = match read_searchable_text_with_metadata(&mut file, max_file_bytes, &metadata) {
        SearchTextRead::Text(text) => {
            let byte_len = u64::try_from(text.len()).unwrap_or(u64::MAX);
            if reserve_search_text_budget(indexed_text_bytes, byte_len, max_text_bytes) {
                ProjectSearchIndexedFileContent::Text { text, byte_len }
            } else {
                ProjectSearchIndexedFileContent::IndexBudgetExceeded
            }
        }
        SearchTextRead::TooLarge => ProjectSearchIndexedFileContent::TooLarge,
        SearchTextRead::BinaryOrInvalid => ProjectSearchIndexedFileContent::BinaryOrInvalid,
        SearchTextRead::Unreadable => ProjectSearchIndexedFileContent::Unreadable,
    };
    (signature, content)
}

fn indexed_search_open_error_content(
    path: &Path,
    max_file_bytes: u64,
) -> (
    Option<ProjectSearchIndexedFileSignature>,
    ProjectSearchIndexedFileContent,
) {
    match fs::metadata(path) {
        Ok(metadata) if metadata.len() > max_file_bytes => (
            Some(ProjectSearchIndexedFileSignature::from_metadata(&metadata)),
            ProjectSearchIndexedFileContent::TooLarge,
        ),
        Ok(metadata) => (
            Some(ProjectSearchIndexedFileSignature::from_metadata(&metadata)),
            ProjectSearchIndexedFileContent::Unreadable,
        ),
        Err(_) => (None, ProjectSearchIndexedFileContent::Unreadable),
    }
}

fn search_text_budget_exhausted(indexed_text_bytes: u64, max_text_bytes: u64) -> bool {
    max_text_bytes > 0 && indexed_text_bytes >= max_text_bytes
}

fn remaining_search_text_budget(indexed_text_bytes: u64, max_text_bytes: u64) -> Option<u64> {
    if max_text_bytes == 0 {
        return None;
    }
    max_text_bytes.checked_sub(indexed_text_bytes)
}

fn effective_search_file_byte_limit(max_file_bytes: u64) -> u64 {
    max_file_bytes.min(MAX_SEARCH_FILE_BYTES)
}

impl ProjectSearchIndexedFileSignature {
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        Self {
            len: metadata.len(),
            modified_nanos: metadata_modified_nanos(metadata),
            created_nanos: metadata_created_nanos(metadata),
        }
    }
}

fn current_indexed_file_signature(path: &Path) -> Option<ProjectSearchIndexedFileSignature> {
    fs::metadata(path)
        .ok()
        .map(|metadata| ProjectSearchIndexedFileSignature::from_metadata(&metadata))
}

fn metadata_modified_nanos(metadata: &fs::Metadata) -> u128 {
    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

fn metadata_created_nanos(metadata: &fs::Metadata) -> u128 {
    metadata
        .created()
        .ok()
        .and_then(|created| created.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

#[derive(Debug)]
struct SearchChunkResult {
    truncated: bool,
    stats: SearchStats,
}

#[derive(Debug)]
struct SearchResultBudget {
    visible_limit: usize,
    remaining_visible: usize,
    remaining_until_truncation: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchMatchBudget {
    Collect,
    HiddenTruncation,
    Exhausted,
}

impl SearchResultBudget {
    fn new(max_results: usize) -> Self {
        Self {
            visible_limit: max_results,
            remaining_visible: max_results,
            remaining_until_truncation: max_results.saturating_add(1).max(1),
        }
    }

    fn limit(&self) -> usize {
        self.visible_limit
    }

    fn is_exhausted(&self) -> bool {
        self.remaining_until_truncation == 0
    }

    fn consume_match(&mut self) -> SearchMatchBudget {
        if self.is_exhausted() {
            return SearchMatchBudget::Exhausted;
        }
        self.remaining_until_truncation -= 1;
        if self.remaining_visible == 0 {
            return SearchMatchBudget::HiddenTruncation;
        }
        self.remaining_visible -= 1;
        SearchMatchBudget::Collect
    }
}

struct ProjectSearchContext<'a, 'needle> {
    root: &'a Path,
    indexed_max_file_bytes: u64,
    line_needle: &'a LineSearchNeedle<'needle>,
    options: &'a SearchOptions,
    include_globs: Option<&'a GlobSet>,
    exclude_globs: Option<&'a GlobSet>,
    is_cancelled: &'a dyn Fn() -> bool,
}

fn search_project_index_chunk(
    context: &ProjectSearchContext<'_, '_>,
    files: &[ProjectSearchIndexedFile],
    result_budget: &mut SearchResultBudget,
    matches: &mut Vec<SearchMatch>,
) -> Option<SearchChunkResult> {
    let mut truncated = false;
    let mut stats = SearchStats::default();

    for file in files {
        if (context.is_cancelled)() {
            return None;
        }
        if result_budget.is_exhausted() {
            truncated = true;
            break;
        }
        let result = match search_project_index_file(context, file, result_budget, matches) {
            FileSearchOutcome::Searched(result) => result,
            FileSearchOutcome::Skipped => continue,
            FileSearchOutcome::Cancelled => return None,
        };
        stats.merge(result.stats);
        truncated |= result.local_truncated;

        if result_budget.is_exhausted() {
            truncated = true;
            break;
        }
    }

    Some(SearchChunkResult { truncated, stats })
}

#[derive(Debug)]
struct FileSearchResult {
    local_truncated: bool,
    stats: SearchStats,
}

#[derive(Debug)]
enum FileSearchOutcome {
    Searched(FileSearchResult),
    Skipped,
    Cancelled,
}

impl FileSearchOutcome {
    fn from_result(result: Option<FileSearchResult>) -> Self {
        match result {
            Some(result) => Self::Searched(result),
            None => Self::Cancelled,
        }
    }
}

impl FileSearchResult {
    fn searched(matched_file: bool, local_truncated: bool) -> Self {
        let matched_files = usize::from(matched_file);
        Self {
            local_truncated,
            stats: SearchStats {
                searched_files: 1,
                matched_files,
                ..SearchStats::default()
            },
        }
    }

    fn skipped_large() -> Self {
        Self::skipped(SearchStats {
            skipped_large_files: 1,
            ..SearchStats::default()
        })
    }

    fn skipped_binary() -> Self {
        Self::skipped(SearchStats {
            skipped_binary_files: 1,
            ..SearchStats::default()
        })
    }

    fn skipped_unreadable() -> Self {
        Self::skipped(SearchStats {
            skipped_unreadable_files: 1,
            ..SearchStats::default()
        })
    }

    fn skipped_index_budget() -> Self {
        Self::skipped(SearchStats {
            skipped_index_budget_files: 1,
            ..SearchStats::default()
        })
    }

    fn skipped(stats: SearchStats) -> Self {
        Self {
            local_truncated: false,
            stats,
        }
    }
}

fn search_project_index_file(
    context: &ProjectSearchContext<'_, '_>,
    file: &ProjectSearchIndexedFile,
    result_budget: &mut SearchResultBudget,
    matches: &mut Vec<SearchMatch>,
) -> FileSearchOutcome {
    if !path_allowed_by_globs(
        context.root,
        &file.path,
        context.include_globs,
        context.exclude_globs,
    ) {
        return FileSearchOutcome::Skipped;
    }
    let needs_live_read = indexed_search_file_is_stale(file)
        || indexed_search_file_needs_larger_limit_read(
            file,
            context.indexed_max_file_bytes,
            context.options.max_file_bytes,
        );
    if needs_live_read {
        if (context.is_cancelled)() {
            return FileSearchOutcome::Cancelled;
        }
        let content = read_live_search_file_content(&file.path, context.options.max_file_bytes);
        if (context.is_cancelled)() {
            return FileSearchOutcome::Cancelled;
        }
        return FileSearchOutcome::from_result(search_project_file_content(
            &file.path,
            &content,
            context.line_needle,
            context.options,
            result_budget,
            matches,
            context.is_cancelled,
        ));
    }
    if matches!(
        file.content,
        ProjectSearchIndexedFileContent::IndexBudgetExceeded
    ) {
        return FileSearchOutcome::from_result(search_project_file_content(
            &file.path,
            &file.content,
            context.line_needle,
            context.options,
            result_budget,
            matches,
            context.is_cancelled,
        ));
    }
    FileSearchOutcome::from_result(search_project_file_content(
        &file.path,
        &file.content,
        context.line_needle,
        context.options,
        result_budget,
        matches,
        context.is_cancelled,
    ))
}

fn indexed_search_file_needs_larger_limit_read(
    file: &ProjectSearchIndexedFile,
    indexed_max_file_bytes: u64,
    search_max_file_bytes: u64,
) -> bool {
    effective_search_file_byte_limit(search_max_file_bytes) > indexed_max_file_bytes
        && matches!(file.content, ProjectSearchIndexedFileContent::TooLarge)
}

fn indexed_search_file_is_stale(file: &ProjectSearchIndexedFile) -> bool {
    current_indexed_file_signature(&file.path) != file.signature
}

fn read_live_search_file_content(
    path: &Path,
    max_file_bytes: u64,
) -> ProjectSearchIndexedFileContent {
    match read_searchable_text(path, max_file_bytes) {
        SearchTextRead::Text(text) => {
            let byte_len = u64::try_from(text.len()).unwrap_or(u64::MAX);
            ProjectSearchIndexedFileContent::Text { text, byte_len }
        }
        SearchTextRead::TooLarge => ProjectSearchIndexedFileContent::TooLarge,
        SearchTextRead::BinaryOrInvalid => ProjectSearchIndexedFileContent::BinaryOrInvalid,
        SearchTextRead::Unreadable => ProjectSearchIndexedFileContent::Unreadable,
    }
}

fn search_project_file_content(
    path: &Path,
    content: &ProjectSearchIndexedFileContent,
    line_needle: &LineSearchNeedle<'_>,
    options: &SearchOptions,
    result_budget: &mut SearchResultBudget,
    matches: &mut Vec<SearchMatch>,
    is_cancelled: &dyn Fn() -> bool,
) -> Option<FileSearchResult> {
    match content {
        ProjectSearchIndexedFileContent::Text { text, byte_len } => {
            if *byte_len > options.max_file_bytes {
                return Some(FileSearchResult::skipped_large());
            }
            search_text_file(
                path,
                text,
                line_needle,
                options,
                result_budget,
                matches,
                is_cancelled,
            )
        }
        ProjectSearchIndexedFileContent::TooLarge => Some(FileSearchResult::skipped_large()),
        ProjectSearchIndexedFileContent::BinaryOrInvalid => {
            Some(FileSearchResult::skipped_binary())
        }
        ProjectSearchIndexedFileContent::Unreadable => Some(FileSearchResult::skipped_unreadable()),
        ProjectSearchIndexedFileContent::IndexBudgetExceeded => {
            Some(FileSearchResult::skipped_index_budget())
        }
    }
}

fn search_text_file(
    path: &Path,
    text: &str,
    line_needle: &LineSearchNeedle<'_>,
    options: &SearchOptions,
    result_budget: &mut SearchResultBudget,
    matches: &mut Vec<SearchMatch>,
    is_cancelled: &dyn Fn() -> bool,
) -> Option<FileSearchResult> {
    let mut matched_file = false;
    let mut local_truncated = false;
    let mut cancelled = false;
    let mut matches_since_cancel_check = 0usize;
    'lines: for (line_idx, line) in text.lines().enumerate() {
        if line_idx % SEARCH_CANCEL_LINE_INTERVAL == 0 && is_cancelled() {
            return None;
        }
        let mut column_counter = SearchLineColumnCounter::new(line);
        let line_scan = for_each_line_match_with_cancel(
            line,
            line_needle,
            options.whole_word,
            is_cancelled,
            |byte_col| {
                matches_since_cancel_check = matches_since_cancel_check.saturating_add(1);
                if matches_since_cancel_check >= SEARCH_CANCEL_MATCH_INTERVAL {
                    matches_since_cancel_check = 0;
                    if is_cancelled() {
                        cancelled = true;
                        return false;
                    }
                }
                match result_budget.consume_match() {
                    SearchMatchBudget::Collect => {}
                    SearchMatchBudget::HiddenTruncation => {
                        matched_file = true;
                        local_truncated = true;
                        return false;
                    }
                    SearchMatchBudget::Exhausted => {
                        local_truncated = true;
                        return false;
                    }
                }
                matched_file = true;
                let column = column_counter.column_for_byte(byte_col);
                matches.push(SearchMatch {
                    path: path.to_path_buf(),
                    line: line_idx.saturating_add(1),
                    column,
                    preview: search_preview(line, byte_col),
                });
                true
            },
        );
        if !matches!(line_scan, LineMatchScan::Completed) {
            if matches!(line_scan, LineMatchScan::Cancelled) {
                return None;
            }
            if cancelled {
                return None;
            }
            break 'lines;
        }
    }
    Some(FileSearchResult::searched(matched_file, local_truncated))
}

struct SearchLineColumnCounter<'a> {
    line: &'a str,
    line_is_ascii: Option<bool>,
    previous_byte: usize,
    previous_chars: usize,
}

impl<'a> SearchLineColumnCounter<'a> {
    fn new(line: &'a str) -> Self {
        Self {
            line,
            line_is_ascii: None,
            previous_byte: 0,
            previous_chars: 0,
        }
    }

    fn column_for_byte(&mut self, byte_offset: usize) -> usize {
        let line_is_ascii = match self.line_is_ascii {
            Some(line_is_ascii) => line_is_ascii,
            None => {
                let line_is_ascii = self.line.is_ascii();
                self.line_is_ascii = Some(line_is_ascii);
                line_is_ascii
            }
        };
        if line_is_ascii {
            return byte_offset.min(self.line.len()).saturating_add(1);
        }
        let byte_offset = floor_char_boundary(self.line, byte_offset);

        self.previous_chars = char_count_to_byte_offset(
            self.line,
            self.previous_byte,
            self.previous_chars,
            byte_offset,
        );
        self.previous_byte = byte_offset;
        self.previous_chars.saturating_add(1)
    }
}

fn char_count_to_byte_offset(
    text: &str,
    previous_byte: usize,
    previous_chars: usize,
    byte_offset: usize,
) -> usize {
    let previous_byte = floor_char_boundary(text, previous_byte);
    let byte_offset = floor_char_boundary(text, byte_offset);
    if byte_offset >= previous_byte {
        previous_chars.saturating_add(char_count_fast(&text[previous_byte..byte_offset]))
    } else {
        char_count_fast(&text[..byte_offset])
    }
}

fn char_count_fast(text: &str) -> usize {
    let bytes = text.as_bytes();
    let Some(first_non_ascii) = bytes.iter().position(|byte| !byte.is_ascii()) else {
        return bytes.len();
    };
    first_non_ascii + text[first_non_ascii..].chars().count()
}

enum SearchTextRead {
    Text(String),
    TooLarge,
    BinaryOrInvalid,
    Unreadable,
}

fn read_searchable_text(path: &Path, max_file_bytes: u64) -> SearchTextRead {
    let max_file_bytes = effective_search_file_byte_limit(max_file_bytes);
    let mut file = match fs::File::open(path) {
        Ok(file) => file,
        Err(_) => return search_text_open_error(path, max_file_bytes),
    };
    let metadata = match file.metadata() {
        Ok(metadata) => metadata,
        Err(_) => return SearchTextRead::Unreadable,
    };
    read_searchable_text_with_metadata(&mut file, max_file_bytes, &metadata)
}

fn search_text_open_error(path: &Path, max_file_bytes: u64) -> SearchTextRead {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() && metadata.len() > max_file_bytes => {
            SearchTextRead::TooLarge
        }
        _ => SearchTextRead::Unreadable,
    }
}

fn read_searchable_text_with_metadata(
    file: &mut fs::File,
    max_file_bytes: u64,
    metadata: &fs::Metadata,
) -> SearchTextRead {
    let max_file_bytes = effective_search_file_byte_limit(max_file_bytes);
    if metadata.is_file() && metadata.len() > max_file_bytes {
        return SearchTextRead::TooLarge;
    }
    let Ok(bytes) = read_file_prefix(file, max_file_bytes) else {
        return SearchTextRead::Unreadable;
    };
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_file_bytes {
        return SearchTextRead::TooLarge;
    }
    if bytes.contains(&0) {
        return SearchTextRead::BinaryOrInvalid;
    }
    match String::from_utf8(bytes) {
        Ok(text) => SearchTextRead::Text(text),
        Err(_) => SearchTextRead::BinaryOrInvalid,
    }
}

fn read_file_prefix(file: &mut fs::File, max_file_bytes: u64) -> io::Result<Vec<u8>> {
    let limit = max_file_bytes.saturating_add(1);
    let capacity = usize::try_from(limit.min(64 * 1024)).unwrap_or(64 * 1024);
    let mut reader = file.take(limit);
    let mut bytes = Vec::with_capacity(capacity);
    reader.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn build_glob_set(patterns: &[String]) -> Result<Option<GlobSet>, String> {
    let mut builder = GlobSetBuilder::new();
    let mut added = HashSet::with_capacity(
        patterns
            .len()
            .min(MAX_SEARCH_GLOB_PATTERNS)
            .saturating_mul(3),
    );
    let mut has_patterns = false;
    let mut pattern_count = 0usize;
    for pattern in patterns {
        let pattern = pattern.trim();
        if pattern.is_empty() {
            continue;
        }
        pattern_count = pattern_count.saturating_add(1);
        if pattern_count > MAX_SEARCH_GLOB_PATTERNS {
            return Err(format_too_many_glob_patterns_error());
        }
        if pattern.len() > MAX_SEARCH_GLOB_PATTERN_BYTES {
            return Err(format_oversized_glob_pattern_error(pattern));
        }
        has_patterns = true;
        add_glob_pattern(&mut builder, &mut added, pattern)?;
        if is_bare_glob_pattern(pattern) {
            add_bare_glob_pattern_variants(&mut builder, &mut added, pattern)?;
        }
    }
    if !has_patterns {
        return Ok(None);
    }

    builder
        .build()
        .map(Some)
        .map_err(format_invalid_glob_build_error)
}

fn is_bare_glob_pattern(pattern: &str) -> bool {
    !pattern.contains(['/', '\\']) && !pattern.starts_with("**")
}

fn add_glob_pattern(
    builder: &mut GlobSetBuilder,
    added: &mut HashSet<String>,
    pattern: &str,
) -> Result<(), String> {
    if !added.insert(pattern.to_owned()) {
        return Ok(());
    }
    let glob =
        Glob::new(pattern).map_err(|error| format_invalid_glob_pattern_error(pattern, error))?;
    builder.add(glob);
    Ok(())
}

fn add_bare_glob_pattern_variants(
    builder: &mut GlobSetBuilder,
    added: &mut HashSet<String>,
    pattern: &str,
) -> Result<(), String> {
    let mut descendant_pattern = String::with_capacity(pattern.len() + 6);
    descendant_pattern.push_str("**/");
    descendant_pattern.push_str(pattern);
    add_glob_pattern(builder, added, &descendant_pattern)?;

    descendant_pattern.push_str("/**");
    add_glob_pattern(builder, added, &descendant_pattern)
}

fn format_too_many_glob_patterns_error() -> String {
    format!("Too many glob patterns; maximum is {MAX_SEARCH_GLOB_PATTERNS}")
}

fn format_oversized_glob_pattern_error(pattern: &str) -> String {
    format!(
        "Glob pattern `{}` is too long; maximum is {MAX_SEARCH_GLOB_PATTERN_BYTES} bytes",
        sanitize_glob_error_text(pattern, GLOB_ERROR_PATTERN_MAX_CHARS)
    )
}

fn format_invalid_glob_pattern_error(pattern: &str, detail: impl fmt::Display) -> String {
    format!(
        "Invalid glob `{}`: {}",
        sanitize_glob_error_text(pattern, GLOB_ERROR_PATTERN_MAX_CHARS),
        sanitize_glob_error_text(&detail.to_string(), GLOB_ERROR_DETAIL_MAX_CHARS)
    )
}

fn format_invalid_glob_build_error(detail: impl fmt::Display) -> String {
    format!(
        "Invalid glob pattern: {}",
        sanitize_glob_error_text(&detail.to_string(), GLOB_ERROR_DETAIL_MAX_CHARS)
    )
}

fn sanitize_glob_error_text(text: &str, max_chars: usize) -> String {
    let mut sanitized = String::with_capacity(text.len().min(max_chars));
    let mut used_chars = 0usize;
    for ch in text.chars() {
        let Some(fragment_chars) = sanitized_glob_error_escape_len(ch) else {
            if used_chars.saturating_add(1) > max_chars {
                append_truncation_marker(&mut sanitized, max_chars, &mut used_chars);
                return sanitized;
            }
            sanitized.push(ch);
            used_chars += 1;
            continue;
        };
        if used_chars.saturating_add(fragment_chars) > max_chars {
            append_truncation_marker(&mut sanitized, max_chars, &mut used_chars);
            return sanitized;
        }
        push_sanitized_glob_error_escape(&mut sanitized, ch);
        used_chars += fragment_chars;
    }
    sanitized
}

fn sanitized_glob_error_escape_len(ch: char) -> Option<usize> {
    match ch {
        '\n' | '\r' | '\t' => Some(2),
        ch if ch.is_control() || is_bidi_control(ch) => Some(unicode_escape_len(ch)),
        _ => None,
    }
}

fn unicode_escape_len(ch: char) -> usize {
    4 + hex_digit_count(u32::from(ch)).max(4)
}

fn hex_digit_count(mut value: u32) -> usize {
    let mut digits = 1;
    while value >= 16 {
        value /= 16;
        digits += 1;
    }
    digits
}

fn push_sanitized_glob_error_escape(output: &mut String, ch: char) {
    match ch {
        '\n' => output.push_str("\\n"),
        '\r' => output.push_str("\\r"),
        '\t' => output.push_str("\\t"),
        ch if ch.is_control() || is_bidi_control(ch) => {
            let _ = write!(output, "\\u{{{:04x}}}", u32::from(ch));
        }
        _ => {}
    }
}

fn append_truncation_marker(text: &mut String, max_chars: usize, used_chars: &mut usize) {
    const MARKER: &str = "...";
    const MARKER_CHARS: usize = MARKER.len();

    if max_chars <= MARKER_CHARS {
        text.clear();
        for _ in 0..max_chars {
            text.push('.');
        }
        *used_chars = max_chars;
        return;
    }

    while used_chars.saturating_add(MARKER_CHARS) > max_chars {
        if text.pop().is_none() {
            break;
        }
        *used_chars = used_chars.saturating_sub(1);
    }
    text.push_str(MARKER);
    *used_chars += MARKER_CHARS;
}

fn is_bidi_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{061c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
    )
}

fn path_allowed_by_globs(
    root: &Path,
    path: &Path,
    include_globs: Option<&GlobSet>,
    exclude_globs: Option<&GlobSet>,
) -> bool {
    if include_globs.is_none() && exclude_globs.is_none() {
        return true;
    }

    let relative = path.strip_prefix(root).unwrap_or(path);
    let file_name = path.file_name().map(Path::new);
    if let Some(include_globs) = include_globs
        && !include_globs.is_match(relative)
        && !file_name.is_some_and(|name| include_globs.is_match(name))
    {
        return false;
    }
    if let Some(exclude_globs) = exclude_globs
        && (exclude_globs.is_match(relative)
            || file_name.is_some_and(|name| exclude_globs.is_match(name)))
    {
        return false;
    }

    true
}

#[derive(Debug, Clone, Copy)]
enum LineSearchNeedle<'a> {
    CaseSensitive(&'a str),
    CaseInsensitive(AsciiCaseInsensitiveMatcher<'a>),
}

impl<'a> LineSearchNeedle<'a> {
    fn new(needle: &'a str, case_sensitive: bool) -> Self {
        if case_sensitive {
            Self::CaseSensitive(needle)
        } else {
            Self::CaseInsensitive(AsciiCaseInsensitiveMatcher::new(needle))
        }
    }

    fn len(&self) -> usize {
        match self {
            Self::CaseSensitive(needle) => needle.len(),
            Self::CaseInsensitive(matcher) => matcher.needle_len(),
        }
    }

    fn find_next(&self, haystack: &str, search_from: usize) -> Option<usize> {
        match self {
            Self::CaseSensitive(needle) => {
                if search_from > haystack.len() {
                    return None;
                }
                let search_from = ceil_char_boundary(haystack, search_from);
                haystack[search_from..]
                    .find(needle)
                    .map(|offset| search_from + offset)
            }
            Self::CaseInsensitive(matcher) => matcher.find_from(haystack, search_from),
        }
    }
}

#[cfg(test)]
fn for_each_line_match<F>(
    line: &str,
    needle: &str,
    case_sensitive: bool,
    whole_word: bool,
    on_match: F,
) -> bool
where
    F: FnMut(usize) -> bool,
{
    let needle = LineSearchNeedle::new(needle, case_sensitive);
    for_each_line_match_with(line, &needle, whole_word, on_match)
}

#[cfg(test)]
fn for_each_line_match_with<F>(
    line: &str,
    needle: &LineSearchNeedle<'_>,
    whole_word: bool,
    mut on_match: F,
) -> bool
where
    F: FnMut(usize) -> bool,
{
    matches!(
        for_each_line_match_with_cancel(
            line,
            needle,
            whole_word,
            || false,
            |byte_col| { on_match(byte_col) }
        ),
        LineMatchScan::Completed
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineMatchScan {
    Completed,
    Stopped,
    Cancelled,
}

fn for_each_line_match_with_cancel<F, C>(
    line: &str,
    needle: &LineSearchNeedle<'_>,
    whole_word: bool,
    mut is_cancelled: C,
    mut on_match: F,
) -> LineMatchScan
where
    F: FnMut(usize) -> bool,
    C: FnMut() -> bool,
{
    let needle_len = needle.len();
    if needle_len == 0 {
        return LineMatchScan::Completed;
    }

    let mut search_from = 0;
    let cancel_byte_interval = SEARCH_CANCEL_BYTE_INTERVAL;
    let mut next_cancel_check = cancel_byte_interval.min(line.len());
    let mut whole_word_matcher = SearchLineWholeWordMatcher::new(line, whole_word);
    while search_from <= line.len() {
        if search_from >= next_cancel_check {
            if is_cancelled() {
                return LineMatchScan::Cancelled;
            }
            next_cancel_check = next_cancel_check
                .max(search_from)
                .saturating_add(cancel_byte_interval)
                .min(line.len());
        }

        let search_limit = line_match_search_limit(line, needle_len, next_cancel_check);
        let haystack = &line[..search_limit];
        let Some(start) = needle.find_next(haystack, search_from) else {
            if search_limit == line.len() {
                break;
            }
            search_from = next_line_match_search_start(line, search_limit, needle_len);
            continue;
        };
        let end = start + needle_len;
        if whole_word_matcher.is_match(start, end) && !on_match(start) {
            return LineMatchScan::Stopped;
        }
        search_from = end.max(start + 1);
    }
    LineMatchScan::Completed
}

fn line_match_search_limit(line: &str, needle_len: usize, next_cancel_check: usize) -> usize {
    if next_cancel_check >= line.len() {
        return line.len();
    }
    let limit = next_cancel_check
        .saturating_add(needle_len.saturating_sub(1))
        .min(line.len());
    ceil_char_boundary(line, limit)
}

fn next_line_match_search_start(line: &str, search_limit: usize, needle_len: usize) -> usize {
    if search_limit >= line.len() {
        return line.len();
    }
    ceil_char_boundary(
        line,
        search_limit.saturating_sub(needle_len).saturating_add(1),
    )
}

fn ceil_char_boundary(text: &str, mut byte_idx: usize) -> usize {
    byte_idx = byte_idx.min(text.len());
    while byte_idx < text.len() && !text.is_char_boundary(byte_idx) {
        byte_idx += 1;
    }
    byte_idx
}

fn floor_char_boundary(text: &str, mut byte_idx: usize) -> usize {
    byte_idx = byte_idx.min(text.len());
    while byte_idx > 0 && !text.is_char_boundary(byte_idx) {
        byte_idx -= 1;
    }
    byte_idx
}

struct SearchLineWholeWordMatcher<'a> {
    line: &'a str,
    whole_word: bool,
    line_is_ascii: Option<bool>,
}

impl<'a> SearchLineWholeWordMatcher<'a> {
    fn new(line: &'a str, whole_word: bool) -> Self {
        Self {
            line,
            whole_word,
            line_is_ascii: None,
        }
    }

    fn is_match(&mut self, start: usize, end: usize) -> bool {
        if !self.whole_word {
            return true;
        }

        let line_is_ascii = match self.line_is_ascii {
            Some(line_is_ascii) => line_is_ascii,
            None => {
                let line_is_ascii = self.line.is_ascii();
                self.line_is_ascii = Some(line_is_ascii);
                line_is_ascii
            }
        };
        if line_is_ascii {
            return is_ascii_whole_word_match(self.line.as_bytes(), start, end);
        }

        is_unicode_whole_word_match(self.line, start, end)
    }
}

fn is_ascii_whole_word_match(line: &[u8], start: usize, end: usize) -> bool {
    let before = start.checked_sub(1).and_then(|index| line.get(index));
    let after = line.get(end);
    !before.is_some_and(|byte| is_ascii_word_byte(*byte))
        && !after.is_some_and(|byte| is_ascii_word_byte(*byte))
}

fn is_ascii_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn is_unicode_whole_word_match(line: &str, start: usize, end: usize) -> bool {
    if start > end
        || end > line.len()
        || !line.is_char_boundary(start)
        || !line.is_char_boundary(end)
    {
        return false;
    }
    let before = line[..start].chars().next_back();
    let after = line[end..].chars().next();
    !before.is_some_and(is_word_char) && !after.is_some_and(is_word_char)
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

fn search_preview(line: &str, match_byte_col: usize) -> String {
    let match_byte_col = floor_char_boundary(line, match_byte_col);
    let trim_start_byte = line.len().saturating_sub(line.trim_start().len());
    let trimmed = line[trim_start_byte..].trim_end();
    if trimmed.len() <= MAX_SEARCH_PREVIEW_CHARS {
        return trimmed.to_owned();
    }

    if trimmed.is_ascii() {
        let match_offset = match_byte_col
            .saturating_sub(trim_start_byte)
            .min(trimmed.len().saturating_sub(1));
        let start = match_offset
            .saturating_sub(SEARCH_PREVIEW_CONTEXT_CHARS)
            .min(trimmed.len().saturating_sub(MAX_SEARCH_PREVIEW_CHARS));
        let end = start
            .saturating_add(MAX_SEARCH_PREVIEW_CHARS)
            .min(trimmed.len());
        let mut preview = String::with_capacity(end.saturating_sub(start).saturating_add(6));
        if start > 0 {
            preview.push_str("...");
        }
        preview.push_str(&trimmed[start..end]);
        if end < trimmed.len() {
            preview.push_str("...");
        }
        return preview;
    }

    let total_chars = trimmed.chars().count();
    if total_chars <= MAX_SEARCH_PREVIEW_CHARS {
        return trimmed.to_owned();
    }
    let match_char = if match_byte_col <= trim_start_byte {
        0
    } else {
        line[trim_start_byte..match_byte_col]
            .chars()
            .count()
            .min(total_chars.saturating_sub(1))
    };
    let start_char = match_char
        .saturating_sub(SEARCH_PREVIEW_CONTEXT_CHARS)
        .min(total_chars.saturating_sub(MAX_SEARCH_PREVIEW_CHARS));
    let end_char = start_char
        .saturating_add(MAX_SEARCH_PREVIEW_CHARS)
        .min(total_chars);
    let mut preview = String::with_capacity(
        end_char
            .saturating_sub(start_char)
            .saturating_mul(4)
            .min(trimmed.len())
            .saturating_add(6),
    );
    if start_char > 0 {
        preview.push_str("...");
    }
    preview.push_str(slice_chars(trimmed, start_char, end_char));
    if end_char < total_chars {
        preview.push_str("...");
    }
    preview
}

fn slice_chars(text: &str, start_char: usize, end_char: usize) -> &str {
    let (start, end) = byte_range_for_char_window(text, start_char, end_char);
    &text[start..end]
}

fn byte_range_for_char_window(text: &str, start_char: usize, end_char: usize) -> (usize, usize) {
    if start_char == 0 && end_char == 0 {
        return (0, 0);
    }

    let mut start = if start_char == 0 { Some(0) } else { None };
    for (char_idx, (byte_idx, _)) in text.char_indices().enumerate() {
        if char_idx == start_char {
            start = Some(byte_idx);
        }
        if char_idx == end_char {
            return (start.unwrap_or(byte_idx), byte_idx);
        }
    }
    let end = text.len();
    (start.unwrap_or(end), end)
}

#[cfg(test)]
mod tests;
