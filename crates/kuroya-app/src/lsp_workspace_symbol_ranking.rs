use crate::workspace_trust::{trusted_workspace_paths_match, workspace_path_contains_lexically};
use kuroya_core::LspWorkspaceSymbol;
use serde::{Deserialize, Serialize};
use std::{
    cmp::Reverse,
    collections::{HashMap, VecDeque},
    ffi::OsStr,
    path::{Component, Path, PathBuf},
};

pub(crate) const MAX_WORKSPACE_SYMBOL_QUERY_MEMORY: usize = 128;
const WORKSPACE_SYMBOL_MEMORY_QUERY_MAX_CHARS: usize = 128;
const WORKSPACE_SYMBOL_MEMORY_NAME_MAX_CHARS: usize = 256;
const WORKSPACE_SYMBOL_QUERY_MEMORY_BONUS: i64 = 96;
const WORKSPACE_SYMBOL_QUERY_MEMORY_PREFIX_BONUS: i64 = 48;
const WORKSPACE_SYMBOL_QUERY_MEMORY_PREFIX_MIN_CHARS: usize = 3;
const WORKSPACE_SYMBOL_QUERY_MEMORY_USE_BONUS: i64 = 8;
const WORKSPACE_SYMBOL_QUERY_MEMORY_MIN_EXACT_TOTAL: i64 =
    WORKSPACE_SYMBOL_QUERY_MEMORY_BONUS + WORKSPACE_SYMBOL_QUERY_MEMORY_USE_BONUS;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceSymbolQueryMemoryEntry {
    pub query: String,
    pub path: PathBuf,
    pub name: String,
    #[serde(default)]
    pub kind: u8,
    pub line: usize,
    pub column: usize,
    #[serde(default = "default_workspace_symbol_query_memory_uses")]
    pub uses: u32,
}

fn default_workspace_symbol_query_memory_uses() -> u32 {
    1
}

pub(crate) fn rank_workspace_symbols_by_navigation_context(
    symbols: &mut [LspWorkspaceSymbol],
    recent_files: &VecDeque<PathBuf>,
    open_files: &[&Path],
    query_memory: &VecDeque<WorkspaceSymbolQueryMemoryEntry>,
    query: &str,
) {
    if symbols.len() < 2
        || (recent_files.is_empty() && open_files.is_empty() && query_memory.is_empty())
    {
        return;
    }

    let normalized_query = (!query_memory.is_empty())
        .then(|| normalize_workspace_symbol_memory_query(query))
        .flatten();
    let query_memory = WorkspaceSymbolRankQueryMemory::new(query_memory);
    let open_file_ranks = WorkspaceSymbolPathRankMap::from_paths(
        open_files
            .iter()
            .enumerate()
            .map(|(index, path)| (index, *path)),
    );
    let recent_file_ranks = WorkspaceSymbolPathRankMap::from_paths(
        recent_files
            .iter()
            .enumerate()
            .map(|(index, path)| (index, path.as_path())),
    );
    symbols.sort_by_cached_key(|symbol| {
        workspace_symbol_navigation_rank_key(
            symbol,
            &recent_file_ranks,
            &open_file_ranks,
            &query_memory,
            normalized_query.as_deref(),
        )
    });
}

pub(crate) fn record_workspace_symbol_query_memory(
    memory: &mut VecDeque<WorkspaceSymbolQueryMemoryEntry>,
    workspace_root: &Path,
    query: &str,
    symbol: &LspWorkspaceSymbol,
    max_entries: usize,
) {
    if max_entries == 0 {
        memory.clear();
        return;
    }
    let Some(path) = normalize_workspace_symbol_memory_path(workspace_root, &symbol.path) else {
        return;
    };
    let Some(query) = normalize_workspace_symbol_memory_query(query) else {
        return;
    };
    let Some(name) = normalize_workspace_symbol_memory_name(&symbol.name) else {
        return;
    };
    let entries = std::mem::take(memory);
    *memory = normalize_workspace_symbol_query_memory(entries, workspace_root, max_entries);

    let mut uses = 1;
    if let Some(index) = memory.iter().position(|entry| {
        entry.query == query
            && workspace_symbol_memory_matches_key(entry, &path, &name, symbol.kind)
    }) {
        if let Some(entry) = memory.remove(index) {
            uses = entry.uses.saturating_add(1).max(1);
        }
    }

    memory.push_front(WorkspaceSymbolQueryMemoryEntry {
        query,
        path,
        name,
        kind: symbol.kind,
        line: symbol.line,
        column: symbol.column,
        uses,
    });
    while memory.len() > max_entries {
        memory.pop_back();
    }
}

pub(crate) fn normalize_workspace_symbol_query_memory(
    entries: impl IntoIterator<Item = WorkspaceSymbolQueryMemoryEntry>,
    workspace_root: &Path,
    max_entries: usize,
) -> VecDeque<WorkspaceSymbolQueryMemoryEntry> {
    if max_entries == 0 {
        return VecDeque::new();
    }

    let mut memory: VecDeque<WorkspaceSymbolQueryMemoryEntry> = VecDeque::new();
    for entry in entries {
        let Some(path) = normalize_workspace_symbol_memory_path(workspace_root, &entry.path) else {
            continue;
        };
        let Some(query) = normalize_workspace_symbol_memory_query(&entry.query) else {
            continue;
        };
        let Some(name) = normalize_workspace_symbol_memory_name(&entry.name) else {
            continue;
        };
        let normalized = WorkspaceSymbolQueryMemoryEntry {
            query,
            path,
            name,
            kind: entry.kind,
            line: entry.line.max(1),
            column: entry.column.max(1),
            uses: entry.uses.max(1),
        };
        if let Some(existing) = memory.iter_mut().find(|existing| {
            workspace_symbol_memory_entries_share_query_and_key(existing, &normalized)
        }) {
            existing.uses = existing.uses.max(normalized.uses);
            continue;
        }
        if memory.len() >= max_entries {
            continue;
        }
        memory.push_back(normalized);
    }
    memory
}

struct WorkspaceSymbolRankQueryMemory {
    rows: Vec<WorkspaceSymbolRankQueryMemoryRow>,
}

impl WorkspaceSymbolRankQueryMemory {
    fn new(memory: &VecDeque<WorkspaceSymbolQueryMemoryEntry>) -> Self {
        let mut rows = Vec::with_capacity(memory.len().min(MAX_WORKSPACE_SYMBOL_QUERY_MEMORY));
        for entry in memory.iter().take(MAX_WORKSPACE_SYMBOL_QUERY_MEMORY) {
            let Some(path_key) = workspace_symbol_path_rank_key(&entry.path) else {
                continue;
            };
            let Some(query) = normalize_workspace_symbol_memory_query(&entry.query) else {
                continue;
            };
            let Some(name) = normalize_workspace_symbol_memory_name(&entry.name) else {
                continue;
            };
            rows.push(WorkspaceSymbolRankQueryMemoryRow {
                query,
                path_key,
                name,
                kind: entry.kind,
                line: entry.line,
                column: entry.column,
                uses: entry.uses,
            });
        }
        Self { rows }
    }
}

struct WorkspaceSymbolRankQueryMemoryRow {
    query: String,
    path_key: WorkspaceSymbolPathRankKey,
    name: String,
    kind: u8,
    line: usize,
    column: usize,
    uses: u32,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct WorkspaceSymbolNavigationRankKey {
    memory_bonus: Reverse<i64>,
    open_index: usize,
    recent_index: usize,
}

fn workspace_symbol_navigation_rank_key(
    symbol: &LspWorkspaceSymbol,
    recent_file_ranks: &WorkspaceSymbolPathRankMap,
    open_file_ranks: &WorkspaceSymbolPathRankMap,
    query_memory: &WorkspaceSymbolRankQueryMemory,
    normalized_query: Option<&str>,
) -> WorkspaceSymbolNavigationRankKey {
    let path_key = workspace_symbol_path_rank_key(&symbol.path);
    WorkspaceSymbolNavigationRankKey {
        memory_bonus: Reverse(workspace_symbol_query_memory_bonus(
            query_memory,
            normalized_query,
            symbol,
            path_key.as_ref(),
        )),
        open_index: path_key
            .as_ref()
            .and_then(|key| open_file_ranks.rank(key))
            .unwrap_or(usize::MAX),
        recent_index: path_key
            .as_ref()
            .and_then(|key| recent_file_ranks.rank(key))
            .unwrap_or(usize::MAX),
    }
}

fn workspace_symbol_query_memory_bonus(
    memory: &WorkspaceSymbolRankQueryMemory,
    query: Option<&str>,
    symbol: &LspWorkspaceSymbol,
    symbol_path_key: Option<&WorkspaceSymbolPathRankKey>,
) -> i64 {
    let Some(query) = query else {
        return 0;
    };
    let Some(symbol_path_key) = symbol_path_key else {
        return 0;
    };
    let Some(name) = normalize_workspace_symbol_memory_name(&symbol.name) else {
        return 0;
    };

    memory
        .rows
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            if !workspace_symbol_rank_memory_matches_key(entry, symbol_path_key, &name, symbol.kind)
            {
                return None;
            }
            let query_affinity = workspace_symbol_memory_query_affinity(&entry.query, query);
            if query_affinity == 0 {
                return None;
            }
            let location = workspace_symbol_memory_exact_location_bonus(entry, symbol);
            let uses = i64::from(entry.uses.clamp(1, 8));
            let recency = (MAX_WORKSPACE_SYMBOL_QUERY_MEMORY.saturating_sub(index) as i64).min(24);
            let score = query_affinity
                + uses * WORKSPACE_SYMBOL_QUERY_MEMORY_USE_BONUS
                + recency
                + location;
            Some(
                if query_affinity == WORKSPACE_SYMBOL_QUERY_MEMORY_PREFIX_BONUS {
                    score.min(WORKSPACE_SYMBOL_QUERY_MEMORY_MIN_EXACT_TOTAL - 1)
                } else {
                    score
                },
            )
        })
        .max()
        .unwrap_or_default()
}

fn workspace_symbol_rank_memory_matches_key(
    entry: &WorkspaceSymbolRankQueryMemoryRow,
    path_key: &WorkspaceSymbolPathRankKey,
    name: &str,
    kind: u8,
) -> bool {
    &entry.path_key == path_key
        && entry.name == name
        && workspace_symbol_memory_kinds_compatible(entry.kind, kind)
}

fn workspace_symbol_memory_entries_share_query_and_key(
    left: &WorkspaceSymbolQueryMemoryEntry,
    right: &WorkspaceSymbolQueryMemoryEntry,
) -> bool {
    left.query == right.query
        && workspace_symbol_paths_match(&left.path, &right.path)
        && left.name == right.name
        && workspace_symbol_memory_kinds_compatible(left.kind, right.kind)
}

fn workspace_symbol_memory_matches_key(
    entry: &WorkspaceSymbolQueryMemoryEntry,
    path: &Path,
    name: &str,
    kind: u8,
) -> bool {
    workspace_symbol_paths_match(&entry.path, path)
        && entry.name == name
        && workspace_symbol_memory_kinds_compatible(entry.kind, kind)
}

fn workspace_symbol_paths_match(left: &Path, right: &Path) -> bool {
    trusted_workspace_paths_match(left, right)
}

fn workspace_symbol_memory_kinds_compatible(left: u8, right: u8) -> bool {
    left == right || left == 0 || right == 0
}

fn workspace_symbol_memory_exact_location_bonus(
    entry: &WorkspaceSymbolRankQueryMemoryRow,
    symbol: &LspWorkspaceSymbol,
) -> i64 {
    if entry.line == symbol.line && entry.column == symbol.column {
        4
    } else {
        0
    }
}

fn workspace_symbol_memory_query_affinity(memory_query: &str, current_query: &str) -> i64 {
    if memory_query == current_query {
        WORKSPACE_SYMBOL_QUERY_MEMORY_BONUS
    } else if workspace_symbol_memory_queries_share_prefix(memory_query, current_query) {
        WORKSPACE_SYMBOL_QUERY_MEMORY_PREFIX_BONUS
    } else {
        0
    }
}

fn workspace_symbol_memory_queries_share_prefix(memory_query: &str, current_query: &str) -> bool {
    (memory_query.starts_with(current_query) || current_query.starts_with(memory_query))
        && memory_query
            .chars()
            .zip(current_query.chars())
            .take_while(|(left, right)| left == right)
            .count()
            >= WORKSPACE_SYMBOL_QUERY_MEMORY_PREFIX_MIN_CHARS
}

fn normalize_workspace_symbol_memory_query(query: &str) -> Option<String> {
    normalize_workspace_symbol_memory_text(
        query,
        WORKSPACE_SYMBOL_MEMORY_QUERY_MAX_CHARS,
        WorkspaceSymbolMemoryTextCase::Lowercase,
    )
}

fn normalize_workspace_symbol_memory_name(name: &str) -> Option<String> {
    normalize_workspace_symbol_memory_text(
        name,
        WORKSPACE_SYMBOL_MEMORY_NAME_MAX_CHARS,
        WorkspaceSymbolMemoryTextCase::Preserve,
    )
}

#[derive(Clone, Copy)]
enum WorkspaceSymbolMemoryTextCase {
    Preserve,
    Lowercase,
}

fn normalize_workspace_symbol_memory_text(
    text: &str,
    max_chars: usize,
    case: WorkspaceSymbolMemoryTextCase,
) -> Option<String> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if let Some(normalized) =
        normalize_workspace_symbol_memory_text_ascii_fast_path(text, max_chars, case)
    {
        return Some(normalized);
    }

    let mut normalized = String::with_capacity(text.len().min(max_chars));
    let mut normalized_chars = 0;
    let mut previous_was_space = false;
    for ch in text.chars() {
        if normalized_chars >= max_chars {
            break;
        }
        if is_workspace_symbol_memory_format_control(ch) {
            continue;
        }
        if ch.is_whitespace() {
            if !normalized.is_empty() && !previous_was_space {
                normalized.push(' ');
                normalized_chars += 1;
                previous_was_space = true;
            }
            continue;
        }
        if ch.is_control() {
            continue;
        }

        match case {
            WorkspaceSymbolMemoryTextCase::Preserve => {
                normalized.push(ch);
                normalized_chars += 1;
            }
            WorkspaceSymbolMemoryTextCase::Lowercase => {
                for lower in ch.to_lowercase() {
                    if normalized_chars >= max_chars {
                        break;
                    }
                    normalized.push(lower);
                    normalized_chars += 1;
                }
            }
        }
        previous_was_space = false;
    }

    if previous_was_space {
        normalized.pop();
    }
    (!normalized.is_empty()).then_some(normalized)
}

fn normalize_workspace_symbol_memory_text_ascii_fast_path(
    text: &str,
    max_chars: usize,
    case: WorkspaceSymbolMemoryTextCase,
) -> Option<String> {
    if max_chars == 0
        || text.len() > max_chars
        || !workspace_symbol_memory_ascii_text_is_clean(text)
    {
        return None;
    }

    match case {
        WorkspaceSymbolMemoryTextCase::Preserve => Some(text.to_owned()),
        WorkspaceSymbolMemoryTextCase::Lowercase => {
            if text.bytes().any(|byte| byte.is_ascii_uppercase()) {
                Some(text.to_ascii_lowercase())
            } else {
                Some(text.to_owned())
            }
        }
    }
}

fn workspace_symbol_memory_ascii_text_is_clean(text: &str) -> bool {
    let mut previous_space = false;
    for byte in text.bytes() {
        match byte {
            b' '..=b'~' => {
                if byte == b' ' {
                    if previous_space {
                        return false;
                    }
                    previous_space = true;
                } else {
                    previous_space = false;
                }
            }
            _ => return false,
        }
    }
    true
}

fn is_workspace_symbol_memory_format_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
    )
}

struct WorkspaceSymbolPathRankMap {
    ranks: HashMap<WorkspaceSymbolPathRankKey, usize>,
}

impl WorkspaceSymbolPathRankMap {
    fn from_paths<'a>(paths: impl IntoIterator<Item = (usize, &'a Path)>) -> Self {
        let paths = paths.into_iter();
        let mut ranks = HashMap::with_capacity(paths.size_hint().0);
        for (index, path) in paths {
            let Some(key) = workspace_symbol_path_rank_key(path) else {
                continue;
            };
            ranks.entry(key).or_insert(index);
        }
        Self { ranks }
    }

    fn rank(&self, key: &WorkspaceSymbolPathRankKey) -> Option<usize> {
        self.ranks.get(key).copied()
    }
}

#[derive(Debug, Eq, Hash, PartialEq)]
struct WorkspaceSymbolPathRankKey {
    prefix: Option<String>,
    rooted: bool,
    components: Vec<String>,
}

fn workspace_symbol_path_rank_key(path: &Path) -> Option<WorkspaceSymbolPathRankKey> {
    if path.as_os_str().is_empty() {
        return None;
    }

    let mut key = WorkspaceSymbolPathRankKey {
        prefix: None,
        rooted: false,
        components: Vec::new(),
    };
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => {
                key.prefix = Some(workspace_symbol_rank_component(prefix.as_os_str()));
            }
            Component::RootDir => key.rooted = true,
            Component::CurDir => {}
            Component::ParentDir => {
                if key
                    .components
                    .last()
                    .is_some_and(|component| component != "..")
                {
                    key.components.pop();
                } else {
                    key.components.push("..".to_owned());
                }
            }
            Component::Normal(component) => {
                key.components
                    .push(workspace_symbol_rank_component(component));
            }
        }
    }

    Some(key)
}

fn workspace_symbol_rank_component(component: &OsStr) -> String {
    let component = component.to_string_lossy();
    #[cfg(windows)]
    {
        if component.is_ascii() {
            let mut component = component.into_owned();
            component.make_ascii_lowercase();
            component
        } else {
            component.to_lowercase()
        }
    }
    #[cfg(not(windows))]
    {
        component.into_owned()
    }
}

fn normalize_workspace_symbol_memory_path(workspace_root: &Path, path: &Path) -> Option<PathBuf> {
    let path = lexically_normalize_workspace_symbol_path(path);
    workspace_path_contains_lexically(workspace_root, &path).then_some(path)
}

fn lexically_normalize_workspace_symbol_path(path: &Path) -> PathBuf {
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

#[cfg(test)]
mod tests {
    use super::{
        MAX_WORKSPACE_SYMBOL_QUERY_MEMORY, WORKSPACE_SYMBOL_MEMORY_NAME_MAX_CHARS,
        WORKSPACE_SYMBOL_MEMORY_QUERY_MAX_CHARS, WorkspaceSymbolPathRankMap,
        WorkspaceSymbolQueryMemoryEntry, normalize_workspace_symbol_query_memory,
        rank_workspace_symbols_by_navigation_context, record_workspace_symbol_query_memory,
        workspace_symbol_path_rank_key,
    };
    use kuroya_core::LspWorkspaceSymbol;
    use std::{
        collections::VecDeque,
        path::{Path, PathBuf},
    };

    #[test]
    fn workspace_symbol_ranking_boosts_recent_files() {
        let mut symbols = vec![
            symbol("src/lib.rs", "lib_symbol"),
            symbol("src/main.rs", "main_symbol"),
            symbol("src/task.rs", "task_symbol"),
        ];
        let recent = VecDeque::from([PathBuf::from("src/task.rs"), PathBuf::from("src/main.rs")]);

        rank_workspace_symbols_by_navigation_context(
            &mut symbols,
            &recent,
            &[],
            &VecDeque::new(),
            "task",
        );

        let names = symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["task_symbol", "main_symbol", "lib_symbol"]);
    }

    #[test]
    fn workspace_symbol_ranking_boosts_open_files_before_recents() {
        let mut symbols = vec![
            symbol("src/lib.rs", "lib_symbol"),
            symbol("src/main.rs", "main_symbol"),
            symbol("src/task.rs", "task_symbol"),
        ];
        let recent = VecDeque::from([PathBuf::from("src/task.rs")]);
        let open = PathBuf::from("src/main.rs");
        let open_files = [open.as_path()];

        rank_workspace_symbols_by_navigation_context(
            &mut symbols,
            &recent,
            &open_files,
            &VecDeque::new(),
            "main",
        );

        let names = symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["main_symbol", "task_symbol", "lib_symbol"]);
    }

    #[test]
    fn workspace_symbol_ranking_applies_memory_open_and_recent_priority_order() {
        let mut symbols = vec![
            symbol("src/lib.rs", "lib_symbol"),
            symbol("src/recent.rs", "recent_symbol"),
            symbol("src/open.rs", "open_symbol"),
            symbol("src/memory.rs", "memory_symbol"),
        ];
        let recent = VecDeque::from([PathBuf::from("src/recent.rs")]);
        let open = PathBuf::from("src/open.rs");
        let open_files = [open.as_path()];
        let memory = VecDeque::from([memory_entry("target", "src/memory.rs", "memory_symbol", 1)]);

        rank_workspace_symbols_by_navigation_context(
            &mut symbols,
            &recent,
            &open_files,
            &memory,
            "target",
        );

        let names = symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "memory_symbol",
                "open_symbol",
                "recent_symbol",
                "lib_symbol"
            ]
        );
    }

    #[test]
    fn workspace_symbol_ranking_boosts_lexically_equivalent_open_files() {
        let mut symbols = vec![
            symbol("workspace/src/lib.rs", "lib_symbol"),
            symbol("workspace/src/../src/main.rs", "main_symbol"),
        ];
        let open = PathBuf::from("workspace/src/main.rs");
        let open_files = [open.as_path()];

        rank_workspace_symbols_by_navigation_context(
            &mut symbols,
            &VecDeque::new(),
            &open_files,
            &VecDeque::new(),
            "symbol",
        );

        let names = symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["main_symbol", "lib_symbol"]);
    }

    #[test]
    fn workspace_symbol_path_rank_map_indexes_equivalent_paths_once() {
        let first = PathBuf::from("workspace/src/main.rs");
        let duplicate = PathBuf::from("workspace/src/../src/main.rs");
        let other = PathBuf::from("workspace/src/lib.rs");
        let paths = [first.as_path(), duplicate.as_path(), other.as_path()];
        let ranks = WorkspaceSymbolPathRankMap::from_paths(
            paths.iter().enumerate().map(|(index, path)| (index, *path)),
        );

        let main_key =
            workspace_symbol_path_rank_key(Path::new("workspace/./src/main.rs")).unwrap();
        let other_key = workspace_symbol_path_rank_key(Path::new("workspace/src/lib.rs")).unwrap();

        assert_eq!(ranks.rank(&main_key), Some(0));
        assert_eq!(ranks.rank(&other_key), Some(2));
    }

    #[test]
    fn workspace_symbol_ranking_boosts_lexically_equivalent_recent_files() {
        let mut symbols = vec![
            symbol("workspace/src/lib.rs", "lib_symbol"),
            symbol("workspace/src/../src/main.rs", "main_symbol"),
        ];
        let recent = VecDeque::from([PathBuf::from("workspace/src/main.rs")]);

        rank_workspace_symbols_by_navigation_context(
            &mut symbols,
            &recent,
            &[],
            &VecDeque::new(),
            "symbol",
        );

        let names = symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["main_symbol", "lib_symbol"]);
    }

    #[test]
    fn workspace_symbol_ranking_preserves_non_recent_order() {
        let mut symbols = vec![
            symbol("src/lib.rs", "lib_symbol"),
            symbol("src/main.rs", "main_symbol"),
        ];
        let recent = VecDeque::from([PathBuf::from("src/other.rs")]);

        rank_workspace_symbols_by_navigation_context(
            &mut symbols,
            &recent,
            &[],
            &VecDeque::new(),
            "symbol",
        );

        let names = symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["lib_symbol", "main_symbol"]);
    }

    #[test]
    fn workspace_symbol_ranking_boosts_remembered_query_choice() {
        let mut symbols = vec![
            symbol("src/lib.rs", "main_symbol"),
            symbol("src/main.rs", "main_symbol"),
        ];
        let memory = VecDeque::from([memory_entry("main", "src/main.rs", "main_symbol", 3)]);

        rank_workspace_symbols_by_navigation_context(
            &mut symbols,
            &VecDeque::new(),
            &[],
            &memory,
            " MAIN ",
        );

        let paths = symbols
            .iter()
            .map(|symbol| symbol.path.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec![PathBuf::from("src/main.rs"), PathBuf::from("src/lib.rs")]
        );
    }

    #[test]
    fn workspace_symbol_ranking_boosts_remembered_choice_with_lexical_path_variant() {
        let mut symbols = vec![
            symbol("workspace/src/lib.rs", "main_symbol"),
            symbol("workspace/src/../src/main.rs", "main_symbol"),
        ];
        let memory = VecDeque::from([memory_entry(
            "main",
            "workspace/src/main.rs",
            "main_symbol",
            3,
        )]);

        rank_workspace_symbols_by_navigation_context(
            &mut symbols,
            &VecDeque::new(),
            &[],
            &memory,
            "main",
        );

        let paths = symbols
            .iter()
            .map(|symbol| symbol.path.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec![
                PathBuf::from("workspace/src/../src/main.rs"),
                PathBuf::from("workspace/src/lib.rs"),
            ]
        );
    }

    #[cfg(windows)]
    #[test]
    fn workspace_symbol_ranking_boosts_remembered_choice_with_windows_case_variant() {
        let mut symbols = vec![
            symbol(r"C:\Repo\Project\src\lib.rs", "main_symbol"),
            symbol(r"c:\repo\project\src\main.rs", "main_symbol"),
        ];
        let memory = VecDeque::from([memory_entry(
            "main",
            r"C:\Repo\Project\src\main.rs",
            "main_symbol",
            3,
        )]);

        rank_workspace_symbols_by_navigation_context(
            &mut symbols,
            &VecDeque::new(),
            &[],
            &memory,
            "main",
        );

        assert_eq!(
            symbols[0].path,
            PathBuf::from(r"c:\repo\project\src\main.rs")
        );
    }

    #[test]
    fn workspace_symbol_ranking_boosts_refined_remembered_queries() {
        let mut symbols = vec![
            symbol("src/data.rs", "DatabasePool"),
            symbol("src/domain.rs", "DatabaseUser"),
        ];
        let memory = VecDeque::from([memory_entry(
            "database pool",
            "src/data.rs",
            "DatabasePool",
            1,
        )]);

        rank_workspace_symbols_by_navigation_context(
            &mut symbols,
            &VecDeque::new(),
            &[],
            &memory,
            "database",
        );

        let names = symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["DatabasePool", "DatabaseUser"]);
    }

    #[test]
    fn workspace_symbol_ranking_does_not_boost_short_query_prefixes() {
        let mut symbols = vec![
            symbol("src/domain.rs", "DataUser"),
            symbol("src/data.rs", "DataPool"),
        ];
        let memory = VecDeque::from([memory_entry("da", "src/data.rs", "DataPool", 1)]);

        rank_workspace_symbols_by_navigation_context(
            &mut symbols,
            &VecDeque::new(),
            &[],
            &memory,
            "data",
        );

        let names = symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["DataUser", "DataPool"]);
    }

    #[test]
    fn workspace_symbol_ranking_keeps_exact_memory_above_hot_prefix_memory() {
        let mut symbols = vec![
            symbol("src/prefix.rs", "DatabasePool"),
            symbol("src/exact.rs", "DatabaseUser"),
        ];
        let memory = VecDeque::from([
            memory_entry("database pool", "src/prefix.rs", "DatabasePool", 8),
            memory_entry("database", "src/exact.rs", "DatabaseUser", 1),
        ]);

        rank_workspace_symbols_by_navigation_context(
            &mut symbols,
            &VecDeque::new(),
            &[],
            &memory,
            "database",
        );

        let names = symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["DatabaseUser", "DatabasePool"]);
    }

    #[test]
    fn workspace_symbol_ranking_matches_legacy_memory_without_kind() {
        let mut symbols = vec![
            symbol("src/lib.rs", "main_symbol"),
            symbol("src/main.rs", "main_symbol"),
        ];
        let mut entry = memory_entry("main", "src/main.rs", "main_symbol", 1);
        entry.kind = 0;
        let memory = VecDeque::from([entry]);

        rank_workspace_symbols_by_navigation_context(
            &mut symbols,
            &VecDeque::new(),
            &[],
            &memory,
            "main",
        );

        let paths = symbols
            .iter()
            .map(|symbol| symbol.path.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec![PathBuf::from("src/main.rs"), PathBuf::from("src/lib.rs")]
        );
    }

    #[test]
    fn workspace_symbol_ranking_normalizes_stale_memory_rows_before_boosting() {
        let mut symbols = vec![
            symbol("src/lib.rs", "Main Symbol"),
            symbol("src/main.rs", "Main Symbol"),
        ];
        let memory = VecDeque::from([WorkspaceSymbolQueryMemoryEntry {
            query: " MAIN\n\t\u{202e} ".to_owned(),
            path: PathBuf::from("src/main.rs"),
            name: " Main\n\t\u{2066}Symbol ".to_owned(),
            kind: 12,
            line: 1,
            column: 1,
            uses: 3,
        }]);

        rank_workspace_symbols_by_navigation_context(
            &mut symbols,
            &VecDeque::new(),
            &[],
            &memory,
            "main",
        );

        let paths = symbols
            .iter()
            .map(|symbol| symbol.path.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec![PathBuf::from("src/main.rs"), PathBuf::from("src/lib.rs")]
        );
    }

    #[test]
    fn workspace_symbol_ranking_ignores_stale_memory_rows_past_cap() {
        let mut symbols = vec![
            symbol("src/lib.rs", "main_symbol"),
            symbol("src/tail.rs", "main_symbol"),
        ];
        let mut memory = VecDeque::new();
        for index in 0..MAX_WORKSPACE_SYMBOL_QUERY_MEMORY {
            memory.push_back(memory_entry(
                "other",
                &format!("src/{index}.rs"),
                "other_symbol",
                1,
            ));
        }
        memory.push_back(memory_entry("main", "src/tail.rs", "main_symbol", 8));

        rank_workspace_symbols_by_navigation_context(
            &mut symbols,
            &VecDeque::new(),
            &[],
            &memory,
            "main",
        );

        let paths = symbols
            .iter()
            .map(|symbol| symbol.path.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec![PathBuf::from("src/lib.rs"), PathBuf::from("src/tail.rs")]
        );
    }

    #[test]
    fn workspace_symbol_query_memory_records_choices_by_query_and_symbol() {
        let root = PathBuf::from("src");
        let mut memory = VecDeque::new();
        let selected = symbol("src/main.rs", "main_symbol");
        let other = symbol("src/lib.rs", "main_symbol");

        record_workspace_symbol_query_memory(&mut memory, &root, " Main ", &selected, 3);
        record_workspace_symbol_query_memory(&mut memory, &root, "main", &selected, 3);
        record_workspace_symbol_query_memory(&mut memory, &root, "entry", &selected, 3);
        record_workspace_symbol_query_memory(&mut memory, &root, "main", &other, 3);
        record_workspace_symbol_query_memory(&mut memory, &root, "", &selected, 3);

        assert_eq!(
            memory,
            VecDeque::from([
                memory_entry("main", "src/lib.rs", "main_symbol", 1),
                memory_entry("entry", "src/main.rs", "main_symbol", 1),
                memory_entry("main", "src/main.rs", "main_symbol", 2),
            ])
        );
    }

    #[test]
    fn workspace_symbol_query_memory_ignores_choices_outside_workspace() {
        let root = PathBuf::from("workspace");
        let mut memory = VecDeque::new();
        let outside = symbol("outside/src/main.rs", "main_symbol");
        let escaped = symbol("workspace/../outside/src/main.rs", "main_symbol");

        record_workspace_symbol_query_memory(&mut memory, &root, "main", &outside, 3);
        record_workspace_symbol_query_memory(&mut memory, &root, "main", &escaped, 3);

        assert!(memory.is_empty());
    }

    #[test]
    fn workspace_symbol_query_memory_invalid_symbol_path_keeps_existing_state() {
        let root = PathBuf::from("workspace");
        let mut memory = VecDeque::from([memory_entry(
            "main",
            "workspace/src/main.rs",
            "main_symbol",
            2,
        )]);
        let outside = symbol("outside/src/main.rs", "outside_symbol");

        record_workspace_symbol_query_memory(&mut memory, &root, "outside", &outside, 8);

        assert_eq!(
            memory,
            VecDeque::from([memory_entry(
                "main",
                "workspace/src/main.rs",
                "main_symbol",
                2,
            )])
        );
    }

    #[test]
    fn workspace_symbol_query_memory_records_lexically_equivalent_symbol_paths() {
        let root = PathBuf::from("workspace");
        let mut memory = VecDeque::new();
        let noisy = symbol("workspace/src/../src/main.rs", "main_symbol");
        let selected = symbol("workspace/src/main.rs", "main_symbol");

        record_workspace_symbol_query_memory(&mut memory, &root, " Main ", &noisy, 8);
        record_workspace_symbol_query_memory(&mut memory, &root, "main", &selected, 8);

        assert_eq!(
            memory,
            VecDeque::from([memory_entry(
                "main",
                "workspace/src/main.rs",
                "main_symbol",
                2,
            )])
        );
    }

    #[test]
    fn workspace_symbol_query_memory_normalizes_persisted_entries() {
        let root = PathBuf::from("workspace");
        let memory = normalize_workspace_symbol_query_memory(
            vec![
                WorkspaceSymbolQueryMemoryEntry {
                    query: " Main ".to_owned(),
                    path: root.join("src/main.rs"),
                    name: "main_symbol".to_owned(),
                    kind: 12,
                    line: 0,
                    column: 0,
                    uses: 0,
                },
                WorkspaceSymbolQueryMemoryEntry {
                    query: "main".to_owned(),
                    path: root.join("src/main.rs"),
                    name: "main_symbol".to_owned(),
                    kind: 12,
                    line: 1,
                    column: 1,
                    uses: 3,
                },
                WorkspaceSymbolQueryMemoryEntry {
                    query: "other".to_owned(),
                    path: PathBuf::from("outside/src/main.rs"),
                    name: "main_symbol".to_owned(),
                    kind: 12,
                    line: 1,
                    column: 1,
                    uses: 1,
                },
                WorkspaceSymbolQueryMemoryEntry {
                    query: "escaped".to_owned(),
                    path: root.join("..").join("outside").join("main.rs"),
                    name: "escaped_symbol".to_owned(),
                    kind: 12,
                    line: 1,
                    column: 1,
                    uses: 1,
                },
                WorkspaceSymbolQueryMemoryEntry {
                    query: "task".to_owned(),
                    path: root.join("src/task.rs"),
                    name: "task_symbol".to_owned(),
                    kind: 12,
                    line: 4,
                    column: 2,
                    uses: 2,
                },
            ],
            &root,
            8,
        );

        assert_eq!(
            memory,
            VecDeque::from([
                WorkspaceSymbolQueryMemoryEntry {
                    query: "main".to_owned(),
                    path: root.join("src/main.rs"),
                    name: "main_symbol".to_owned(),
                    kind: 12,
                    line: 1,
                    column: 1,
                    uses: 3,
                },
                WorkspaceSymbolQueryMemoryEntry {
                    query: "task".to_owned(),
                    path: root.join("src/task.rs"),
                    name: "task_symbol".to_owned(),
                    kind: 12,
                    line: 4,
                    column: 2,
                    uses: 2,
                },
            ])
        );
    }

    #[test]
    fn workspace_symbol_query_memory_normalization_merges_lexically_equivalent_paths() {
        let root = PathBuf::from("workspace");
        let memory = normalize_workspace_symbol_query_memory(
            vec![
                memory_entry("main", "workspace/src/main.rs", "main_symbol", 2),
                memory_entry("main", "workspace/src/../src/./main.rs", "main_symbol", 5),
            ],
            &root,
            8,
        );

        assert_eq!(
            memory,
            VecDeque::from([memory_entry(
                "main",
                "workspace/src/main.rs",
                "main_symbol",
                5,
            )])
        );
    }

    #[test]
    fn workspace_symbol_query_memory_record_normalizes_existing_entries_before_counting() {
        let root = PathBuf::from("workspace");
        let selected = symbol("workspace/src/main.rs", "main_symbol");
        let mut memory = VecDeque::from([
            WorkspaceSymbolQueryMemoryEntry {
                query: " Main ".to_owned(),
                path: root.join("src/main.rs"),
                name: "main_symbol".to_owned(),
                kind: 12,
                line: 1,
                column: 1,
                uses: 2,
            },
            WorkspaceSymbolQueryMemoryEntry {
                query: "main".to_owned(),
                path: root.join("src/main.rs"),
                name: "main_symbol".to_owned(),
                kind: 12,
                line: 1,
                column: 1,
                uses: 7,
            },
            WorkspaceSymbolQueryMemoryEntry {
                query: "\u{202e}".to_owned(),
                path: root.join("src/main.rs"),
                name: "main_symbol".to_owned(),
                kind: 12,
                line: 1,
                column: 1,
                uses: 5,
            },
        ]);

        record_workspace_symbol_query_memory(&mut memory, &root, "MAIN", &selected, 8);

        assert_eq!(
            memory,
            VecDeque::from([memory_entry(
                "main",
                "workspace/src/main.rs",
                "main_symbol",
                8,
            )])
        );
    }

    #[test]
    fn workspace_symbol_query_memory_normalization_keeps_late_duplicate_uses_after_cap() {
        let root = PathBuf::from("workspace");
        let memory = normalize_workspace_symbol_query_memory(
            vec![
                memory_entry("main", "workspace/src/main.rs", "main_symbol", 1),
                memory_entry("task", "workspace/src/task.rs", "task_symbol", 1),
                memory_entry("dev", "workspace/src/dev.rs", "dev_symbol", 1),
                memory_entry(" Main ", "workspace/src/main.rs", "main_symbol", 9),
            ],
            &root,
            2,
        );

        assert_eq!(
            memory,
            VecDeque::from([
                memory_entry("main", "workspace/src/main.rs", "main_symbol", 9),
                memory_entry("task", "workspace/src/task.rs", "task_symbol", 1),
            ])
        );
    }

    #[test]
    fn workspace_symbol_query_memory_sanitizes_and_bounds_query_and_name() {
        let root = PathBuf::from("workspace");
        let raw_name = format!(
            "{}\n\t{}\u{202e}{}",
            "VeryLongSymbol".repeat(64),
            "inner",
            "tail".repeat(64)
        );
        let selected = symbol("workspace/src/main.rs", &raw_name);
        let mut memory = VecDeque::new();

        record_workspace_symbol_query_memory(
            &mut memory,
            &root,
            " Main\n\t\u{202e}Query ",
            &selected,
            8,
        );

        assert_eq!(memory.len(), 1);
        let entry = &memory[0];
        assert_eq!(entry.query, "main query");
        assert!(entry.query.chars().count() <= WORKSPACE_SYMBOL_MEMORY_QUERY_MAX_CHARS);
        assert!(entry.name.chars().count() <= WORKSPACE_SYMBOL_MEMORY_NAME_MAX_CHARS);
        assert!(!entry.name.contains('\n'));
        assert!(!entry.name.contains('\t'));
        assert!(!entry.name.contains('\u{202e}'));

        let mut symbols = vec![symbol("workspace/src/lib.rs", "OtherSymbol"), selected];
        rank_workspace_symbols_by_navigation_context(
            &mut symbols,
            &VecDeque::new(),
            &[],
            &memory,
            "main query",
        );

        assert_eq!(symbols[0].path, PathBuf::from("workspace/src/main.rs"));
    }

    fn symbol(path: &str, name: &str) -> LspWorkspaceSymbol {
        LspWorkspaceSymbol {
            name: name.to_owned(),
            detail: None,
            kind: 12,
            path: PathBuf::from(path),
            line: 1,
            column: 1,
            end_line: 1,
            end_column: 1,
        }
    }

    fn memory_entry(
        query: &str,
        path: &str,
        name: &str,
        uses: u32,
    ) -> WorkspaceSymbolQueryMemoryEntry {
        WorkspaceSymbolQueryMemoryEntry {
            query: query.to_owned(),
            path: PathBuf::from(path),
            name: name.to_owned(),
            kind: 12,
            line: 1,
            column: 1,
            uses,
        }
    }
}
