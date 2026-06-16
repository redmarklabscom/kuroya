#[cfg(test)]
use crate::text_match::ascii_case_insensitive_contains as contains_ascii_case_insensitive;
#[cfg(test)]
use crate::text_match::ascii_case_insensitive_starts_with as starts_with_ascii_case_insensitive;
use serde::{Deserialize, Deserializer, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

mod symbol_search;
mod symbols;

#[cfg(test)]
use symbol_search::{ProjectSymbolQuery, project_symbol_search_path};
use symbol_search::{project_symbol_search_paths, workspace_symbols};
use symbols::extract_project_symbols;
#[cfg(test)]
use symbols::read_symbol_text_with_limit;

const MAX_PROJECT_SYMBOLS: usize = 20_000;
const MAX_SYMBOLS_PER_FILE: usize = 128;
const MAX_SYMBOL_FILE_BYTES: u64 = 512 * 1024;
const MAX_SYMBOL_LINE_BYTES: usize = 8 * 1024;
const MAX_PROJECT_SYMBOL_QUERY_CHARS: usize = 512;
const MAX_PROJECT_SYMBOL_QUERY_TERMS: usize = 32;
const PROJECT_INDEX_PRUNED_WORKSPACE_DIRS: &[&str] = &[
    ".git",
    ".kuroya",
    "node_modules",
    "target",
    "dist",
    "build",
    "coverage",
    ".next",
    "out",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    pub root: PathBuf,
    pub opened_at: SystemTime,
}

impl Workspace {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            opened_at: SystemTime::now(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectEntry {
    pub path: PathBuf,
    pub relative_path: PathBuf,
    pub is_dir: bool,
    pub depth: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectSymbolKind {
    Module,
    Class,
    Function,
    Variable,
    Constant,
    Enum,
    Interface,
    Struct,
    Type,
}

impl ProjectSymbolKind {
    pub fn lsp_kind(self) -> u8 {
        match self {
            Self::Module => 2,
            Self::Class => 5,
            Self::Function => 12,
            Self::Variable => 13,
            Self::Constant => 14,
            Self::Enum => 10,
            Self::Interface => 11,
            Self::Struct => 23,
            Self::Type => 26,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectSymbol {
    pub name: String,
    pub kind: ProjectSymbolKind,
    pub path: PathBuf,
    pub relative_path: PathBuf,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectIndexSignature {
    pub max_files: usize,
    pub file_count: usize,
    pub entry_count: usize,
    pub truncated: bool,
    pub fingerprint: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ProjectIndex {
    root: PathBuf,
    files: Vec<PathBuf>,
    entries: Vec<ProjectEntry>,
    symbols: Vec<ProjectSymbol>,
    #[serde(skip)]
    symbol_search_paths: Vec<Arc<str>>,
    truncated: bool,
}

#[derive(Deserialize)]
struct ProjectIndexSerde {
    root: PathBuf,
    files: Vec<PathBuf>,
    entries: Vec<ProjectEntry>,
    symbols: Vec<ProjectSymbol>,
    truncated: bool,
}

impl<'de> Deserialize<'de> for ProjectIndex {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let ProjectIndexSerde {
            root,
            files,
            entries,
            symbols,
            truncated,
        } = ProjectIndexSerde::deserialize(deserializer)?;
        let symbol_search_paths = project_symbol_search_paths(&symbols);
        Ok(Self {
            root,
            files,
            entries,
            symbols,
            symbol_search_paths,
            truncated,
        })
    }
}

impl ProjectIndex {
    pub fn rebuild(root: &Path, max_files: usize) -> Self {
        Self::rebuild_with_signature(root, max_files).0
    }

    pub fn rebuild_with_signature(root: &Path, max_files: usize) -> (Self, ProjectIndexSignature) {
        Self::rebuild_with_signature_inner(root, max_files, true)
    }

    fn rebuild_with_signature_inner(
        root: &Path,
        max_files: usize,
        extract_symbols_enabled: bool,
    ) -> (Self, ProjectIndexSignature) {
        let mut files = Vec::with_capacity(max_files.min(MAX_PROJECT_SYMBOLS));
        let mut entries = Vec::new();
        let mut signature_entries = Vec::new();
        let mut symbols = Vec::with_capacity(
            max_files
                .saturating_mul(MAX_SYMBOLS_PER_FILE)
                .min(MAX_PROJECT_SYMBOLS),
        );
        let mut truncated = false;
        let walker = ignore::WalkBuilder::new(root)
            .hidden(false)
            .git_ignore(true)
            .git_exclude(true)
            .parents(true)
            .filter_entry(project_index_entry_is_not_pruned)
            .build();

        for entry in walker.flatten() {
            let path = entry.path();
            if path == root {
                continue;
            }

            let Some(file_type) = entry.file_type() else {
                continue;
            };
            let is_dir = file_type.is_dir();
            let is_file = file_type.is_file();
            if is_file && files.len() >= max_files {
                truncated = true;
                break;
            }
            if !(is_dir || is_file) {
                continue;
            }

            let path = path.to_path_buf();
            let relative_path = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            let metadata = if is_file { entry.metadata().ok() } else { None };

            if is_file {
                files.push(path.clone());
                if extract_symbols_enabled && symbols.len() < MAX_PROJECT_SYMBOLS {
                    symbols.extend(extract_project_symbols(
                        &path,
                        &relative_path,
                        metadata.as_ref().map(fs::Metadata::len),
                        MAX_PROJECT_SYMBOLS - symbols.len(),
                    ));
                }
            }

            let depth = relative_path.components().count().saturating_sub(1);
            let signature_entry = ProjectIndexSignatureEntry::from_parts(
                relative_path.clone(),
                is_dir,
                metadata.as_ref(),
            );
            entries.push(ProjectEntry {
                path,
                relative_path,
                is_dir,
                depth,
            });
            signature_entries.push(signature_entry);
        }

        files.sort_unstable();
        entries.sort_unstable_by(|a, b| {
            a.relative_path
                .cmp(&b.relative_path)
                .then(a.is_dir.cmp(&b.is_dir).reverse())
        });
        signature_entries.sort_unstable_by(|a, b| {
            a.relative_path
                .cmp(&b.relative_path)
                .then(a.is_dir.cmp(&b.is_dir).reverse())
        });
        let signature = ProjectIndexSignature::from_entries(
            max_files,
            files.len(),
            entries.len(),
            truncated,
            &signature_entries,
        );
        let index = Self {
            root: root.to_path_buf(),
            files,
            entries,
            symbol_search_paths: project_symbol_search_paths(&symbols),
            symbols,
            truncated,
        };
        (index, signature)
    }

    pub fn scan_signature(root: &Path, max_files: usize) -> ProjectIndexSignature {
        let mut file_count = 0usize;
        let mut signature_entries = Vec::with_capacity(max_files.min(MAX_PROJECT_SYMBOLS));
        let mut truncated = false;
        let walker = ignore::WalkBuilder::new(root)
            .hidden(false)
            .git_ignore(true)
            .git_exclude(true)
            .parents(true)
            .filter_entry(project_index_entry_is_not_pruned)
            .build();

        for entry in walker.flatten() {
            let path = entry.path();
            if path == root {
                continue;
            }

            let Some(file_type) = entry.file_type() else {
                continue;
            };
            let is_dir = file_type.is_dir();
            let is_file = file_type.is_file();
            if is_file && file_count >= max_files {
                truncated = true;
                break;
            }
            if is_file {
                file_count += 1;
            }

            if is_dir || is_file {
                let metadata = if is_file { entry.metadata().ok() } else { None };
                let relative_path = path.strip_prefix(root).unwrap_or(path).to_path_buf();
                signature_entries.push(ProjectIndexSignatureEntry::from_parts(
                    relative_path,
                    is_dir,
                    metadata.as_ref(),
                ));
            }
        }

        signature_entries.sort_unstable_by(|a, b| {
            a.relative_path
                .cmp(&b.relative_path)
                .then(a.is_dir.cmp(&b.is_dir).reverse())
        });
        ProjectIndexSignature::from_entries(
            max_files,
            file_count,
            signature_entries.len(),
            truncated,
            &signature_entries,
        )
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn files(&self) -> &[PathBuf] {
        &self.files
    }

    pub fn symbols(&self) -> &[ProjectSymbol] {
        &self.symbols
    }

    pub fn all_entries(&self) -> &[ProjectEntry] {
        &self.entries
    }

    pub fn truncated(&self) -> bool {
        self.truncated
    }

    pub fn workspace_symbols(&self, query: &str, limit: usize) -> Vec<ProjectSymbol> {
        workspace_symbols(&self.symbols, &self.symbol_search_paths, query, limit)
    }

    pub fn entries(&self, root: &Path, limit: usize) -> Vec<ProjectEntry> {
        let capacity = limit.min(self.entries.len());
        if root == self.root.as_path() {
            let mut entries = Vec::with_capacity(capacity);
            entries.extend(self.entries.iter().take(limit).cloned());
            return entries;
        }

        let mut entries = Vec::with_capacity(capacity);
        for entry in &self.entries {
            if entries.len() >= limit {
                break;
            }
            let Ok(relative_path) = entry.path.strip_prefix(root) else {
                continue;
            };
            if relative_path.as_os_str().is_empty() {
                continue;
            }
            let mut entry = entry.clone();
            entry.relative_path = relative_path.to_path_buf();
            entry.depth = entry.relative_path.components().count().saturating_sub(1);
            entries.push(entry);
        }
        entries
    }
}

fn project_index_entry_is_not_pruned(entry: &ignore::DirEntry) -> bool {
    entry.depth() == 0
        || entry.file_name().to_str().is_none_or(|name| {
            !PROJECT_INDEX_PRUNED_WORKSPACE_DIRS
                .iter()
                .any(|pruned| name.eq_ignore_ascii_case(pruned))
        })
}

#[derive(Debug, Clone)]
struct ProjectIndexSignatureEntry {
    relative_path: PathBuf,
    is_dir: bool,
    len: u64,
    modified_nanos: u128,
    created_nanos: u128,
}

impl ProjectIndexSignatureEntry {
    fn from_parts(relative_path: PathBuf, is_dir: bool, metadata: Option<&fs::Metadata>) -> Self {
        Self {
            relative_path,
            is_dir,
            len: if is_dir {
                0
            } else {
                metadata.map_or(0, fs::Metadata::len)
            },
            modified_nanos: if is_dir {
                0
            } else {
                metadata.map(metadata_modified_nanos).unwrap_or_default()
            },
            created_nanos: if is_dir {
                0
            } else {
                metadata.map(metadata_created_nanos).unwrap_or_default()
            },
        }
    }
}

impl ProjectIndexSignature {
    fn from_entries(
        max_files: usize,
        file_count: usize,
        entry_count: usize,
        truncated: bool,
        entries: &[ProjectIndexSignatureEntry],
    ) -> Self {
        Self {
            max_files,
            file_count,
            entry_count,
            truncated,
            fingerprint: project_index_signature_fingerprint(max_files, truncated, entries),
        }
    }
}

fn project_index_signature_fingerprint(
    max_files: usize,
    truncated: bool,
    entries: &[ProjectIndexSignatureEntry],
) -> u64 {
    let mut hash = FNV_OFFSET;
    fnv_hash_u64(&mut hash, max_files as u64);
    fnv_hash_u8(&mut hash, u8::from(truncated));
    for entry in entries {
        fnv_hash_path(&mut hash, &entry.relative_path);
        fnv_hash_u8(&mut hash, u8::from(entry.is_dir));
        fnv_hash_u64(&mut hash, entry.len);
        fnv_hash_u128(&mut hash, entry.modified_nanos);
        fnv_hash_u128(&mut hash, entry.created_nanos);
    }
    hash
}

const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

fn fnv_hash_path(hash: &mut u64, path: &Path) {
    for component in path.components() {
        if let std::path::Component::Normal(name) = component {
            for byte in name.to_string_lossy().as_bytes() {
                fnv_hash_u8(hash, *byte);
            }
            fnv_hash_u8(hash, b'/');
        }
    }
}

fn fnv_hash_u8(hash: &mut u64, value: u8) {
    *hash ^= u64::from(value);
    *hash = hash.wrapping_mul(FNV_PRIME);
}

fn fnv_hash_u64(hash: &mut u64, value: u64) {
    for byte in value.to_le_bytes() {
        fnv_hash_u8(hash, byte);
    }
}

fn fnv_hash_u128(hash: &mut u64, value: u128) {
    for byte in value.to_le_bytes() {
        fnv_hash_u8(hash, byte);
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn project_index_contains_directories_and_files() {
        let root = std::env::temp_dir().join(format!(
            "kuroya-project-index-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("src/nested")).unwrap();
        fs::write(root.join("src/lib.rs"), "").unwrap();
        fs::write(root.join("src/nested/mod.rs"), "").unwrap();

        let index = ProjectIndex::rebuild(&root, 40_000);
        let entries = index.entries(&root, 16);

        assert_eq!(index.files().len(), 2);
        assert!(
            entries
                .iter()
                .any(|entry| entry.is_dir && entry.relative_path == Path::new("src"))
        );
        assert!(
            entries
                .iter()
                .any(|entry| entry.is_dir && entry.relative_path == Path::new("src/nested"))
        );
        assert!(
            entries
                .iter()
                .any(|entry| !entry.is_dir && entry.relative_path == Path::new("src/lib.rs"))
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_index_reports_when_file_limit_truncates_workspace() {
        let root = std::env::temp_dir().join(format!(
            "kuroya-project-truncated-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/first.rs"), "").unwrap();
        fs::write(root.join("src/second.rs"), "").unwrap();

        let truncated = ProjectIndex::rebuild(&root, 1);
        assert_eq!(truncated.files().len(), 1);
        assert!(truncated.truncated());

        let complete = ProjectIndex::rebuild(&root, 2);
        assert_eq!(complete.files().len(), 2);
        assert!(!complete.truncated());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_index_entries_for_subroot_filter_before_limit_and_relabel() {
        let root = std::env::temp_dir().join(format!(
            "kuroya-project-subroot-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("src/nested")).unwrap();
        fs::write(root.join("README.md"), "").unwrap();
        fs::write(root.join("src/main.rs"), "").unwrap();
        fs::write(root.join("src/nested/mod.rs"), "").unwrap();

        let index = ProjectIndex::rebuild(&root, 40_000);
        let entries = index.entries(&root.join("src"), 2);

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].relative_path, Path::new("main.rs"));
        assert_eq!(entries[0].depth, 0);
        assert_eq!(entries[1].relative_path, Path::new("nested"));
        assert_eq!(entries[1].depth, 0);
        assert!(
            entries
                .iter()
                .all(|entry| entry.path.starts_with(root.join("src")))
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_index_signature_matches_rebuilt_index() {
        let root = std::env::temp_dir().join(format!(
            "kuroya-project-signature-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/lib.rs"), "fn indexed() {}\n").unwrap();

        let (index, signature) = ProjectIndex::rebuild_with_signature(&root, 40_000);
        let scanned = ProjectIndex::scan_signature(&root, 40_000);

        assert_eq!(index.files().len(), 1);
        assert_eq!(signature, scanned);
        assert_eq!(signature.file_count, 1);
        assert!(!signature.truncated);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_index_ignores_workspace_state_dir() {
        let root = std::env::temp_dir().join(format!(
            "kuroya-project-state-dir-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join(".kuroya/plugins/example")).unwrap();
        fs::write(root.join("src/lib.rs"), "fn indexed() {}\n").unwrap();
        fs::write(root.join(".kuroya/project-index.json"), "{}").unwrap();
        fs::write(root.join(".kuroya/plugins/example/plugin.toml"), "").unwrap();

        let index = ProjectIndex::rebuild(&root, 40_000);

        assert_eq!(index.files(), &[root.join("src/lib.rs")]);
        assert!(
            index
                .all_entries()
                .iter()
                .all(|entry| !entry.relative_path.starts_with(".kuroya"))
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_index_prunes_generated_dependency_dirs() {
        let root = std::env::temp_dir().join(format!(
            "kuroya-project-generated-pruned-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("target/debug")).unwrap();
        fs::create_dir_all(root.join("node_modules/dep")).unwrap();
        fs::create_dir_all(root.join("packages/web/.next/cache")).unwrap();
        fs::write(root.join("src/lib.rs"), "fn indexed() {}\n").unwrap();
        fs::write(root.join("target/debug/generated.rs"), "fn skipped() {}\n").unwrap();
        fs::write(root.join("node_modules/dep/index.js"), "skipped();\n").unwrap();
        fs::write(
            root.join("packages/web/.next/cache/page.js"),
            "skipped();\n",
        )
        .unwrap();

        let index = ProjectIndex::rebuild(&root, 40_000);

        assert_eq!(index.files(), &[root.join("src/lib.rs")]);
        assert!(index.all_entries().iter().all(|entry| {
            !entry.relative_path.starts_with("target")
                && !entry.relative_path.starts_with("node_modules")
                && !entry.relative_path.starts_with("packages/web/.next")
        }));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_index_signature_ignores_workspace_state_dir_changes() {
        let root = std::env::temp_dir().join(format!(
            "kuroya-project-state-dir-signature-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/lib.rs"), "fn indexed() {}\n").unwrap();
        let before = ProjectIndex::scan_signature(&root, 40_000);

        fs::create_dir_all(root.join(".kuroya")).unwrap();
        fs::write(root.join(".kuroya/project-index.json"), "{}").unwrap();
        let after = ProjectIndex::scan_signature(&root, 40_000);

        assert_eq!(before, after);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_index_signature_changes_when_indexed_file_changes() {
        let root = std::env::temp_dir().join(format!(
            "kuroya-project-signature-stale-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        let path = root.join("src/lib.rs");
        fs::write(&path, "fn indexed() {}\n").unwrap();
        let first = ProjectIndex::scan_signature(&root, 40_000);

        fs::write(&path, "fn indexed() {}\nfn newer() {}\n").unwrap();
        let second = ProjectIndex::scan_signature(&root, 40_000);

        assert_ne!(first, second);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_index_signature_fingerprint_includes_created_identity() {
        let entry = ProjectIndexSignatureEntry {
            relative_path: PathBuf::from("src/lib.rs"),
            is_dir: false,
            len: 12,
            modified_nanos: 34,
            created_nanos: 56,
        };
        let changed = ProjectIndexSignatureEntry {
            created_nanos: 57,
            ..entry.clone()
        };

        let first = project_index_signature_fingerprint(40_000, false, &[entry]);
        let second = project_index_signature_fingerprint(40_000, false, &[changed]);

        assert_ne!(first, second);
    }

    #[test]
    fn project_index_extracts_workspace_symbols_from_supported_languages() {
        let root = std::env::temp_dir().join(format!(
            "kuroya-project-symbols-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("src/lib.rs"),
            "pub struct AppState {}\nasync fn load_workspace() {}\nconst MAX_ITEMS: usize = 4;\n",
        )
        .unwrap();
        fs::write(
            root.join("src/app.ts"),
            "export class EditorView {}\nexport const launchTask = () => {}\nconst loadData = async () => {}\nconst makeStore = function () {}\nconst MAX_RETRIES = 3;\n",
        )
        .unwrap();
        fs::write(
            root.join("src/app.py"),
            "class Runner:\n    async def run_task(self):\n        pass\n",
        )
        .unwrap();
        fs::write(
            root.join("src/service.go"),
            "type Server struct{}\nfunc (s *Server) Serve() {}\nconst DefaultPort = 8080\n",
        )
        .unwrap();
        fs::write(
            root.join("src/App.java"),
            "public class App {}\nprivate void render() {}\n",
        )
        .unwrap();
        fs::write(
            root.join("src/native.cpp"),
            "struct NativeState {};\nint compute_value(int input) { return input; }\n",
        )
        .unwrap();
        fs::write(
            root.join("src/Program.cs"),
            "public partial class Program {}\ninternal async Task RunAsync() {}\n",
        )
        .unwrap();

        let index = ProjectIndex::rebuild(&root, 40_000);
        let symbols = index
            .symbols()
            .iter()
            .map(|symbol| {
                (
                    symbol.name.as_str(),
                    symbol.kind,
                    symbol.relative_path.as_path(),
                    symbol.line,
                    symbol.column,
                )
            })
            .collect::<Vec<_>>();

        assert!(symbols.contains(&(
            "AppState",
            ProjectSymbolKind::Struct,
            Path::new("src/lib.rs"),
            1,
            12
        )));
        assert!(symbols.contains(&(
            "load_workspace",
            ProjectSymbolKind::Function,
            Path::new("src/lib.rs"),
            2,
            10
        )));
        assert!(symbols.contains(&(
            "EditorView",
            ProjectSymbolKind::Class,
            Path::new("src/app.ts"),
            1,
            14
        )));
        assert!(symbols.contains(&(
            "launchTask",
            ProjectSymbolKind::Function,
            Path::new("src/app.ts"),
            2,
            14
        )));
        assert!(symbols.contains(&(
            "loadData",
            ProjectSymbolKind::Function,
            Path::new("src/app.ts"),
            3,
            7
        )));
        assert!(symbols.contains(&(
            "makeStore",
            ProjectSymbolKind::Function,
            Path::new("src/app.ts"),
            4,
            7
        )));
        assert!(symbols.contains(&(
            "MAX_RETRIES",
            ProjectSymbolKind::Constant,
            Path::new("src/app.ts"),
            5,
            7
        )));
        assert!(symbols.contains(&(
            "Runner",
            ProjectSymbolKind::Class,
            Path::new("src/app.py"),
            1,
            7
        )));
        assert!(symbols.contains(&(
            "run_task",
            ProjectSymbolKind::Function,
            Path::new("src/app.py"),
            2,
            15
        )));
        assert!(symbols.contains(&(
            "Server",
            ProjectSymbolKind::Struct,
            Path::new("src/service.go"),
            1,
            6
        )));
        assert!(symbols.contains(&(
            "Serve",
            ProjectSymbolKind::Function,
            Path::new("src/service.go"),
            2,
            18
        )));
        assert!(symbols.contains(&(
            "App",
            ProjectSymbolKind::Class,
            Path::new("src/App.java"),
            1,
            14
        )));
        assert!(symbols.contains(&(
            "compute_value",
            ProjectSymbolKind::Function,
            Path::new("src/native.cpp"),
            2,
            5
        )));
        assert!(symbols.contains(&(
            "Program",
            ProjectSymbolKind::Class,
            Path::new("src/Program.cs"),
            1,
            22
        )));
        assert!(symbols.contains(&(
            "RunAsync",
            ProjectSymbolKind::Function,
            Path::new("src/Program.cs"),
            2,
            21
        )));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_symbol_reader_enforces_limit_while_reading() {
        let root = std::env::temp_dir().join(format!(
            "kuroya-project-symbol-read-limit-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("large.rs");
        fs::write(&path, "abcdef").unwrap();

        assert_eq!(
            read_symbol_text_with_limit(&path, 6).as_deref(),
            Some("abcdef")
        );
        assert!(read_symbol_text_with_limit(&path, 5).is_none());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_symbol_extraction_uses_walk_file_len_to_skip_oversized_files() {
        let root = std::env::temp_dir().join(format!(
            "kuroya-project-symbol-known-large-file-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        let path = root.join("src/lib.rs");
        let relative_path = Path::new("src/lib.rs");
        fs::write(&path, "fn indexed() {}\n").unwrap();

        assert!(
            extract_project_symbols(&path, relative_path, Some(MAX_SYMBOL_FILE_BYTES + 1), 8)
                .is_empty()
        );

        let symbols = extract_project_symbols(&path, relative_path, Some(16), 8);
        assert_eq!(
            symbols
                .iter()
                .map(|symbol| symbol.name.as_str())
                .collect::<Vec<_>>(),
            vec!["indexed"]
        );
        assert_eq!(symbols[0].path.as_path(), path.as_path());
        assert_eq!(symbols[0].relative_path.as_path(), relative_path);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_symbol_extraction_uses_supplied_relative_path() {
        let root = std::env::temp_dir().join(format!(
            "kuroya-project-symbol-relative-path-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        let path = root.join("src/lib.rs");
        let relative_path = Path::new("cached/src/lib.rs");
        fs::write(&path, "fn indexed() {}\n").unwrap();

        let symbols = extract_project_symbols(&path, relative_path, Some(16), 8);

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name.as_str(), "indexed");
        assert_eq!(symbols[0].relative_path.as_path(), relative_path);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_symbol_extraction_uses_source_extensions_before_reading() {
        let root = std::env::temp_dir().join(format!(
            "kuroya-project-symbol-source-extension-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let go_mod = root.join("go.mod");
        let rust_path = root.join("LIB.RS");
        fs::write(&go_mod, "func skipped() {}\n").unwrap();
        fs::write(&rust_path, "fn indexed() {}\n").unwrap();

        assert!(extract_project_symbols(&go_mod, Path::new("go.mod"), Some(18), 8).is_empty());

        let symbols = extract_project_symbols(&rust_path, Path::new("LIB.RS"), Some(16), 8);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name.as_str(), "indexed");

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_symbol_extraction_skips_oversized_lines() {
        let root = std::env::temp_dir().join(format!(
            "kuroya-project-symbol-line-limit-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("lib.rs");
        let text = format!(
            "fn skipped() {{}} {}\nfn indexed() {{}}\n",
            "x".repeat(MAX_SYMBOL_LINE_BYTES)
        );
        fs::write(&path, text).unwrap();

        let symbols = extract_project_symbols(&path, Path::new("lib.rs"), None, 8);

        assert_eq!(
            symbols
                .iter()
                .map(|symbol| symbol.name.as_str())
                .collect::<Vec<_>>(),
            vec!["indexed"]
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_index_skips_oversized_symbol_files() {
        let root = std::env::temp_dir().join(format!(
            "kuroya-project-symbol-large-file-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        let text = format!(
            "fn indexed() {{}}\n{}",
            "x".repeat(MAX_SYMBOL_FILE_BYTES as usize)
        );
        fs::write(root.join("src/large.rs"), text).unwrap();

        let index = ProjectIndex::rebuild(&root, 40_000);

        assert!(index.symbols().is_empty());
        assert_eq!(index.files().len(), 1);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_workspace_symbol_search_scores_names_before_paths() {
        let root = std::env::temp_dir().join(format!(
            "kuroya-project-symbol-search-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("src/task_runner")).unwrap();
        fs::write(root.join("src/task_runner/mod.rs"), "fn unrelated() {}\n").unwrap();
        fs::write(root.join("src/lib.rs"), "fn task_runner() {}\n").unwrap();

        let index = ProjectIndex::rebuild(&root, 40_000);
        let symbols = index.workspace_symbols("task", 8);

        assert_eq!(
            symbols.first().map(|symbol| symbol.name.as_str()),
            Some("task_runner")
        );
        assert!(symbols.iter().any(|symbol| symbol.name == "unrelated"
            && symbol.relative_path == Path::new("src/task_runner/mod.rs")));
        assert!(index.workspace_symbols("", 8).is_empty());
        assert!(index.workspace_symbols("missing", 8).is_empty());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_workspace_symbol_search_matches_mixed_case_paths() {
        let root = std::env::temp_dir().join(format!(
            "kuroya-project-symbol-path-search-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("src/TaskRunner")).unwrap();
        fs::write(root.join("src/TaskRunner/mod.rs"), "fn unrelated() {}\n").unwrap();

        let index = ProjectIndex::rebuild(&root, 40_000);
        let symbols = index.workspace_symbols("taskrunner", 8);

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "unrelated");
        assert_eq!(symbols[0].relative_path, Path::new("src/TaskRunner/mod.rs"));

        let symbols = index.workspace_symbols("src/taskrunner", 8);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "unrelated");

        let symbols = index.workspace_symbols("src\\taskrunner", 8);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "unrelated");

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_workspace_symbol_query_uses_vec_only_after_two_terms() {
        assert!(ProjectSymbolQuery::new("").is_none());

        let Some(ProjectSymbolQuery::Single(term)) = ProjectSymbolQuery::new("load") else {
            panic!("single-term query should stay stack-backed");
        };
        assert_eq!(term.text, "load");
        assert_eq!(term.path_text.as_ref(), "load");

        let Some(ProjectSymbolQuery::Single(term)) =
            ProjectSymbolQuery::new("load\u{00a0}taskrunner")
        else {
            panic!("non-ASCII whitespace should preserve the existing single-term path");
        };
        assert_eq!(term.text, "load\u{00a0}taskrunner");
        assert_eq!(term.path_text.as_ref(), "load\u{00a0}taskrunner");

        let Some(ProjectSymbolQuery::Pair(terms)) = ProjectSymbolQuery::new("load taskrunner")
        else {
            panic!("two-term query should stay stack-backed");
        };
        assert_eq!(terms[0].text, "load");
        assert_eq!(terms[1].text, "taskrunner");
        assert_eq!(terms[0].path_text.as_ref(), "load");
        assert_eq!(terms[1].path_text.as_ref(), "taskrunner");

        let Some(ProjectSymbolQuery::Many(terms)) =
            ProjectSymbolQuery::new("load taskrunner module")
        else {
            panic!("three-term query should use the general matcher path");
        };
        assert_eq!(
            terms.iter().map(|term| term.text).collect::<Vec<_>>(),
            vec!["load", "taskrunner", "module"]
        );
        assert_eq!(
            terms
                .iter()
                .map(|term| term.path_text.as_ref())
                .collect::<Vec<_>>(),
            vec!["load", "taskrunner", "module"]
        );

        assert!(ProjectSymbolQuery::new(&"x".repeat(MAX_PROJECT_SYMBOL_QUERY_CHARS + 1)).is_none());
        assert!(
            ProjectSymbolQuery::new(&vec!["x"; MAX_PROJECT_SYMBOL_QUERY_TERMS + 1].join(" "))
                .is_none()
        );
    }

    #[test]
    fn project_workspace_symbol_query_folds_ascii_path_terms_once() {
        let Some(ProjectSymbolQuery::Single(term)) = ProjectSymbolQuery::new("LOAD/Über") else {
            panic!("single-term query should stay stack-backed");
        };

        assert_eq!(term.text, "LOAD/Über");
        assert_eq!(term.path_text.as_ref(), "load/Über");
    }

    #[test]
    fn project_workspace_symbol_query_normalizes_path_separators_once() {
        let Some(ProjectSymbolQuery::Single(term)) = ProjectSymbolQuery::new("SRC\\TaskRunner")
        else {
            panic!("single-term query should stay stack-backed");
        };

        assert_eq!(term.text, "SRC\\TaskRunner");
        assert_eq!(term.path_text.as_ref(), "src/taskrunner");
    }

    #[test]
    fn project_workspace_symbol_search_prepares_path_cache_after_rebuild_and_load() {
        let root = std::env::temp_dir().join(format!(
            "kuroya-project-symbol-path-cache-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("src/TaskRunner")).unwrap();
        fs::write(root.join("src/TaskRunner/mod.rs"), "fn unrelated() {}\n").unwrap();

        let index = ProjectIndex::rebuild(&root, 40_000);
        let expected_path = "src/taskrunner/mod.rs";
        assert_eq!(index.symbol_search_paths.len(), 1);
        assert_eq!(index.symbol_search_paths[0].as_ref(), expected_path);

        let bytes = serde_json::to_vec(&index).unwrap();
        assert!(
            !std::str::from_utf8(&bytes)
                .unwrap()
                .contains("symbol_search_paths")
        );

        let loaded = serde_json::from_slice::<ProjectIndex>(&bytes).unwrap();
        assert_eq!(loaded.symbol_search_paths, index.symbol_search_paths);
        let symbols = loaded.workspace_symbols("taskrunner", 8);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "unrelated");

        let mut legacy_index = loaded.clone();
        legacy_index.symbol_search_paths.clear();
        let symbols = legacy_index.workspace_symbols("taskrunner", 8);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "unrelated");

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_workspace_symbol_search_ignores_stale_path_cache_lengths() {
        let root = PathBuf::from("workspace");
        let mut index = ProjectIndex {
            root: root.clone(),
            files: Vec::new(),
            entries: Vec::new(),
            symbols: vec![
                ProjectSymbol {
                    name: "unrelated".to_owned(),
                    kind: ProjectSymbolKind::Function,
                    path: root.join("src/TaskRunner/mod.rs"),
                    relative_path: PathBuf::from("src/TaskRunner/mod.rs"),
                    line: 1,
                    column: 1,
                },
                ProjectSymbol {
                    name: "other".to_owned(),
                    kind: ProjectSymbolKind::Function,
                    path: root.join("src/lib.rs"),
                    relative_path: PathBuf::from("src/lib.rs"),
                    line: 1,
                    column: 1,
                },
            ],
            symbol_search_paths: vec![project_symbol_search_path(Path::new("src/lib.rs"))],
            truncated: false,
        };

        let symbols = index.workspace_symbols("taskrunner", 8);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "unrelated");

        index.symbol_search_paths.clear();
        let symbols = index.workspace_symbols("taskrunner", 8);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "unrelated");
    }

    #[test]
    fn project_workspace_symbol_search_shares_repeated_file_path_cache_entries() {
        let root = std::env::temp_dir().join(format!(
            "kuroya-project-symbol-shared-path-cache-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("src/TaskRunner")).unwrap();
        fs::write(
            root.join("src/TaskRunner/mod.rs"),
            "fn first_task() {}\nfn second_task() {}\n",
        )
        .unwrap();
        fs::write(root.join("src/lib.rs"), "fn third_task() {}\n").unwrap();

        let index = ProjectIndex::rebuild(&root, 40_000);
        let first_index = index
            .symbols
            .iter()
            .position(|symbol| symbol.name == "first_task")
            .unwrap();
        let second_index = index
            .symbols
            .iter()
            .position(|symbol| symbol.name == "second_task")
            .unwrap();
        let third_index = index
            .symbols
            .iter()
            .position(|symbol| symbol.name == "third_task")
            .unwrap();
        let task_runner_path = "src/taskrunner/mod.rs";

        assert_eq!(
            index.symbol_search_paths[first_index].as_ref(),
            task_runner_path
        );
        assert!(std::sync::Arc::ptr_eq(
            &index.symbol_search_paths[first_index],
            &index.symbol_search_paths[second_index]
        ));
        assert!(!std::sync::Arc::ptr_eq(
            &index.symbol_search_paths[first_index],
            &index.symbol_search_paths[third_index]
        ));

        let symbols = index.workspace_symbols("second taskrunner", 8);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "second_task");

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_symbol_search_paths_share_non_adjacent_normalized_duplicates() {
        let root = PathBuf::from("workspace");
        let symbols = vec![
            ProjectSymbol {
                name: "first_task".to_owned(),
                kind: ProjectSymbolKind::Function,
                path: root.join("src/TaskRunner/mod.rs"),
                relative_path: PathBuf::from("src/TaskRunner/mod.rs"),
                line: 1,
                column: 1,
            },
            ProjectSymbol {
                name: "middle".to_owned(),
                kind: ProjectSymbolKind::Function,
                path: root.join("src/lib.rs"),
                relative_path: PathBuf::from("src/lib.rs"),
                line: 1,
                column: 1,
            },
            ProjectSymbol {
                name: "second_task".to_owned(),
                kind: ProjectSymbolKind::Function,
                path: root.join("src/TaskRunner/mod.rs"),
                relative_path: PathBuf::from("src\\TaskRunner\\mod.rs"),
                line: 2,
                column: 1,
            },
        ];

        let paths = project_symbol_search_paths(&symbols);

        assert_eq!(paths[0].as_ref(), "src/taskrunner/mod.rs");
        assert_eq!(paths[1].as_ref(), "src/lib.rs");
        assert_eq!(paths[2].as_ref(), "src/taskrunner/mod.rs");
        assert!(std::sync::Arc::ptr_eq(&paths[0], &paths[2]));
        assert!(!std::sync::Arc::ptr_eq(&paths[0], &paths[1]));
    }

    #[test]
    fn project_workspace_symbol_search_reuses_prepared_multi_term_matchers() {
        let root = std::env::temp_dir().join(format!(
            "kuroya-project-symbol-multi-term-search-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("src/TaskRunner")).unwrap();
        fs::write(
            root.join("src/TaskRunner/mod.rs"),
            "fn load_workspace() {}\nfn unrelated() {}\n",
        )
        .unwrap();

        let index = ProjectIndex::rebuild(&root, 40_000);
        let symbols = index.workspace_symbols("LOAD taskrunner", 8);

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "load_workspace");
        assert_eq!(symbols[0].relative_path, Path::new("src/TaskRunner/mod.rs"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn ascii_case_insensitive_symbol_matching_preserves_ascii_only_folding() {
        assert!(contains_ascii_case_insensitive("TaskRunner", "task"));
        assert!(starts_with_ascii_case_insensitive("TaskRunner", "task"));
        assert!(!contains_ascii_case_insensitive("Über", "über"));
    }
}
