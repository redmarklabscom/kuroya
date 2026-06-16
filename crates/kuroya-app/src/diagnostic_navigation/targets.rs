use crate::{
    diagnostic_location::{diagnostic_jump_location, normalize_diagnostic_location},
    file_runtime::file_path_known_openable,
};
use kuroya_core::{BufferId, Diagnostic, TextBuffer};
use std::{
    collections::{HashMap, hash_map::Entry},
    ffi::OsStr,
    fs,
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DiagnosticTargetOpenability {
    OpenBuffer(BufferId),
    OpenableFile,
}

pub(crate) struct DiagnosticOpenTarget {
    pub(super) path: PathBuf,
    pub(super) line: usize,
    pub(super) column: usize,
    pub(super) openability: DiagnosticTargetOpenability,
}

pub(super) struct DiagnosticTargetOpenabilityCache<'a> {
    openabilities: Vec<Option<DiagnosticTargetOpenability>>,
    exact_paths: HashMap<&'a Path, usize>,
    equivalent_paths: HashMap<DiagnosticPathKey, usize>,
    max_entries: usize,
}

impl<'a> DiagnosticTargetOpenabilityCache<'a> {
    pub(super) fn with_capacity(capacity: usize) -> Self {
        let capacity = capacity.min(super::MAX_DIAGNOSTIC_NAVIGATION_CACHE_ENTRIES);
        Self {
            openabilities: Vec::with_capacity(capacity),
            exact_paths: HashMap::with_capacity(capacity),
            equivalent_paths: HashMap::with_capacity(capacity),
            max_entries: capacity,
        }
    }

    #[cfg(test)]
    pub(super) fn new() -> Self {
        Self::with_capacity(super::MAX_DIAGNOSTIC_NAVIGATION_CACHE_ENTRIES)
    }

    fn get_exact(&self, path: &Path) -> Option<Option<DiagnosticTargetOpenability>> {
        self.exact_paths
            .get(path)
            .and_then(|index| self.openabilities.get(*index).copied())
    }

    fn get_equivalent_key(
        &self,
        key: &DiagnosticPathKey,
    ) -> Option<Option<DiagnosticTargetOpenability>> {
        self.equivalent_paths
            .get(key)
            .and_then(|index| self.openabilities.get(*index).copied())
    }

    fn insert(
        &mut self,
        path: &'a Path,
        openability: Option<DiagnosticTargetOpenability>,
        equivalent_key: Option<DiagnosticPathKey>,
    ) {
        let Some(index) = self.insert_exact(path, openability) else {
            return;
        };
        if let Some(key) = equivalent_key {
            self.insert_equivalent_key(key, index);
        }
    }

    fn insert_exact(
        &mut self,
        path: &'a Path,
        openability: Option<DiagnosticTargetOpenability>,
    ) -> Option<usize> {
        if let Some(index) = self.exact_paths.get(path).copied() {
            self.openabilities[index] = openability;
            return Some(index);
        }
        if self.openabilities.len() >= self.max_entries {
            return None;
        }
        let index = self.openabilities.len();
        self.openabilities.push(openability);
        self.exact_paths.insert(path, index);
        Some(index)
    }

    fn insert_equivalent_key(&mut self, key: DiagnosticPathKey, index: usize) {
        let can_cache = self.equivalent_paths.len() < self.max_entries;
        match self.equivalent_paths.entry(key) {
            Entry::Occupied(_) => {}
            Entry::Vacant(entry) if can_cache => {
                entry.insert(index);
            }
            Entry::Vacant(_) => {}
        }
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.openabilities.len()
    }
}

pub(super) struct DiagnosticBufferLookup<'a> {
    exact_paths: HashMap<&'a Path, BufferId>,
    pub(super) lexical_paths: HashMap<DiagnosticPathKey, BufferId>,
    untitled_paths: HashMap<PathBuf, BufferId>,
}

impl<'a> DiagnosticBufferLookup<'a> {
    pub(super) fn new(buffers: &'a [TextBuffer]) -> Self {
        let mut exact_paths = HashMap::with_capacity(buffers.len());
        let mut lexical_paths = HashMap::with_capacity(buffers.len());
        let mut untitled_paths = HashMap::new();
        for buffer in buffers {
            let id = buffer.id();
            match buffer.path() {
                Some(path) => {
                    exact_paths.entry(path.as_path()).or_insert(id);
                    if let Some(key) = diagnostic_path_key(path) {
                        lexical_paths.entry(key).or_insert(id);
                    }
                }
                None => {
                    untitled_paths
                        .entry(diagnostic_untitled_path(id))
                        .or_insert(id);
                }
            }
        }
        Self {
            exact_paths,
            lexical_paths,
            untitled_paths,
        }
    }

    pub(super) fn id_for_path(&self, path: &Path) -> Option<BufferId> {
        if let Some(id) = self.exact_id_for_path(path) {
            return Some(id);
        }
        let key = diagnostic_path_key(path)?;
        self.lexical_id_for_key(&key)
    }

    fn exact_id_for_path(&self, path: &Path) -> Option<BufferId> {
        self.exact_paths
            .get(path)
            .copied()
            .or_else(|| self.untitled_paths.get(path).copied())
    }

    fn lexical_id_for_key(&self, key: &DiagnosticPathKey) -> Option<BufferId> {
        self.lexical_paths.get(key).copied()
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub(super) struct DiagnosticPathKey {
    prefix: Option<String>,
    rooted: bool,
    components: Vec<String>,
}

pub(super) fn diagnostic_path_key(path: &Path) -> Option<DiagnosticPathKey> {
    if path.as_os_str().is_empty() {
        return None;
    }

    let mut key = DiagnosticPathKey {
        prefix: None,
        rooted: false,
        components: Vec::new(),
    };
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => {
                key.prefix = Some(normalize_diagnostic_path_component(prefix.as_os_str()));
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
                    .push(normalize_diagnostic_path_component(component));
            }
        }
    }

    Some(key)
}

pub(super) fn normalize_diagnostic_path_component(component: &OsStr) -> String {
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

pub(super) fn diagnostic_open_target_for_resolved_path(
    path: &Path,
    line: usize,
    column: usize,
    openability: DiagnosticTargetOpenability,
) -> DiagnosticOpenTarget {
    let (line, column) = normalize_diagnostic_location(line, column);
    DiagnosticOpenTarget {
        path: path.to_path_buf(),
        line,
        column,
        openability,
    }
}

pub(super) fn diagnostic_resolved_jump_location(
    buffers: &[TextBuffer],
    diagnostic: &Diagnostic,
    openability: DiagnosticTargetOpenability,
) -> Option<(usize, usize)> {
    let (line, column) = diagnostic_jump_location(diagnostic);
    match openability {
        DiagnosticTargetOpenability::OpenBuffer(id) => buffers
            .iter()
            .find(|buffer| buffer.id() == id)
            .map(|buffer| diagnostic_buffer_jump_location(buffer, line, column)),
        DiagnosticTargetOpenability::OpenableFile => Some((line, column)),
    }
}

pub(super) fn diagnostic_buffer_jump_location(
    buffer: &TextBuffer,
    line: usize,
    column: usize,
) -> (usize, usize) {
    let line_idx = line
        .saturating_sub(1)
        .min(buffer.len_lines().saturating_sub(1));
    let cursor = buffer.line_column_to_char(line_idx, column.saturating_sub(1));
    let position = buffer.char_position(cursor);
    (position.line + 1, position.column + 1)
}

pub(super) fn diagnostic_target_openability_cached<'a>(
    cache: &mut DiagnosticTargetOpenabilityCache<'a>,
    buffer_lookup: &DiagnosticBufferLookup<'_>,
    indexed_files: &[PathBuf],
    path: &'a Path,
    path_is_openable: impl FnOnce(&Path) -> bool,
) -> Option<DiagnosticTargetOpenability> {
    if let Some(openability) = cache.get_exact(path) {
        return openability;
    }

    if let Some(id) = buffer_lookup.exact_id_for_path(path) {
        let openability = Some(DiagnosticTargetOpenability::OpenBuffer(id));
        cache.insert(path, openability, None);
        return openability;
    }

    let path_key = diagnostic_path_key(path);
    if let Some(key) = path_key.as_ref() {
        if let Some(openability) = cache.get_equivalent_key(key) {
            return openability;
        }
    }

    let openability = if let Some(id) = path_key
        .as_ref()
        .and_then(|key| buffer_lookup.lexical_id_for_key(key))
    {
        Some(DiagnosticTargetOpenability::OpenBuffer(id))
    } else {
        file_path_known_openable(indexed_files, path, path_is_openable)
            .then_some(DiagnosticTargetOpenability::OpenableFile)
    };
    cache.insert(path, openability, path_key);
    openability
}

pub(super) fn diagnostic_target_openability_for(
    buffer_lookup: &DiagnosticBufferLookup<'_>,
    indexed_files: &[PathBuf],
    path: &Path,
    path_is_openable: impl FnOnce(&Path) -> bool,
) -> Option<DiagnosticTargetOpenability> {
    if let Some(id) = buffer_lookup.id_for_path(path) {
        return Some(DiagnosticTargetOpenability::OpenBuffer(id));
    }
    file_path_known_openable(indexed_files, path, path_is_openable)
        .then_some(DiagnosticTargetOpenability::OpenableFile)
}

pub(super) fn diagnostic_path_is_openable_file(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|metadata| metadata.is_file())
}

#[cfg(test)]
pub(super) fn diagnostic_buffer_id_for_buffers(
    buffers: &[TextBuffer],
    path: &Path,
) -> Option<BufferId> {
    DiagnosticBufferLookup::new(buffers).id_for_path(path)
}

pub(super) fn diagnostic_untitled_path(id: BufferId) -> PathBuf {
    PathBuf::from(format!("<untitled-{id}>"))
}
