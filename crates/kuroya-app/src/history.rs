use kuroya_core::TextBuffer;
#[cfg(windows)]
use std::ffi::{OsStr, OsString};
use std::{
    collections::VecDeque,
    path::{Component, Path, PathBuf},
};

pub(crate) const NAVIGATION_HISTORY_LIMIT: usize = 128;
pub(crate) const CLOSED_FILE_HISTORY_LIMIT: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NavigationLocation {
    pub(crate) path: PathBuf,
    pub(crate) line: usize,
    pub(crate) column: usize,
}

impl NavigationLocation {
    pub(crate) fn new(path: PathBuf, line: usize, column: usize) -> Self {
        Self { path, line, column }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClosedFileEntry {
    pub(crate) path: PathBuf,
    pub(crate) line: usize,
    pub(crate) column: usize,
}

impl ClosedFileEntry {
    pub(crate) fn new(path: PathBuf, line: usize, column: usize) -> Self {
        Self { path, line, column }
    }
}

pub(crate) fn push_navigation_location(
    history: &mut VecDeque<NavigationLocation>,
    location: NavigationLocation,
) {
    push_navigation_location_with_limit(history, location, NAVIGATION_HISTORY_LIMIT);
}

pub(crate) fn normalize_navigation_history(
    entries: impl IntoIterator<Item = NavigationLocation>,
    max_entries: usize,
) -> VecDeque<NavigationLocation> {
    if max_entries == 0 {
        return VecDeque::new();
    }

    let mut history = VecDeque::new();
    for location in entries {
        let location = normalize_navigation_location_for_history(location);
        if !history_path_is_recordable(&location.path) {
            continue;
        }
        push_navigation_location_with_limit(&mut history, location, max_entries);
    }
    history
}

pub(crate) fn collect_navigation_locations(
    back: &VecDeque<NavigationLocation>,
    forward: &VecDeque<NavigationLocation>,
    current: Option<NavigationLocation>,
) -> Vec<NavigationLocation> {
    let mut locations = Vec::with_capacity(
        back.len()
            .saturating_add(forward.len())
            .saturating_add(usize::from(current.is_some())),
    );
    locations.extend(back.iter().cloned());
    locations.extend(forward.iter().cloned());
    if let Some(current) = current {
        locations.push(current);
    }
    locations
}

fn push_navigation_location_with_limit(
    history: &mut VecDeque<NavigationLocation>,
    mut location: NavigationLocation,
    max_entries: usize,
) {
    if max_entries == 0 {
        history.clear();
        return;
    }
    prune_history_to_limit(history, max_entries);
    normalize_navigation_location_position(&mut location);

    if !history_path_is_recordable(&location.path) {
        return;
    }

    if history
        .back()
        .is_some_and(|existing| navigation_locations_coalesce(existing, &location))
    {
        if let Some(existing) = history.back_mut() {
            *existing = location;
        }
        return;
    }

    history.retain(|existing| !navigation_locations_coalesce(existing, &location));
    history.push_back(location);
    prune_history_to_limit(history, max_entries);
}

fn normalize_navigation_location_for_history(
    mut location: NavigationLocation,
) -> NavigationLocation {
    normalize_navigation_location_position(&mut location);
    location
}

fn normalize_navigation_location_position(location: &mut NavigationLocation) {
    location.line = location.line.max(1);
    location.column = location.column.max(1);
}

pub(crate) fn navigation_locations_coalesce(
    left: &NavigationLocation,
    right: &NavigationLocation,
) -> bool {
    history_paths_coalesce(&left.path, &right.path)
        && history_line_key(left.line) == history_line_key(right.line)
}

fn history_paths_coalesce(left: &Path, right: &Path) -> bool {
    let Some(left) = history_path_key(left) else {
        return false;
    };
    let Some(right) = history_path_key(right) else {
        return false;
    };
    left == right
}

fn history_path_is_recordable(path: &Path) -> bool {
    history_path_key(path).is_some()
}

fn history_path_key(path: &Path) -> Option<PathBuf> {
    if path.as_os_str().is_empty() {
        return None;
    }
    let normalized = lexical_normalize_path(path);
    let normalized = history_path_case_key(&normalized);
    (!normalized.as_os_str().is_empty()).then_some(normalized)
}

#[cfg(windows)]
fn history_path_case_key(path: &Path) -> PathBuf {
    let mut key = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => key.push(history_component_case_key(prefix.as_os_str())),
            Component::RootDir => key.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => key.push(".."),
            Component::Normal(part) => key.push(history_component_case_key(part)),
        }
    }
    key
}

#[cfg(windows)]
fn history_component_case_key(component: &OsStr) -> OsString {
    let component = component.to_string_lossy();
    if component.is_ascii() {
        let mut component = component.into_owned();
        component.make_ascii_lowercase();
        component.into()
    } else {
        component.to_lowercase().into()
    }
}

#[cfg(not(windows))]
fn history_path_case_key(path: &Path) -> PathBuf {
    path.to_path_buf()
}

fn history_line_key(line: usize) -> usize {
    line.max(1)
}

fn prune_history_to_limit<T>(history: &mut VecDeque<T>, max_entries: usize) {
    while history.len() > max_entries {
        history.pop_front();
    }
}

fn lexical_normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                let can_pop_normal = normalized
                    .components()
                    .next_back()
                    .is_some_and(|component| matches!(component, Component::Normal(_)));
                if can_pop_normal {
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

pub(crate) fn take_navigation_history_target(
    back: &mut VecDeque<NavigationLocation>,
    forward: &mut VecDeque<NavigationLocation>,
    current: Option<NavigationLocation>,
    direction: isize,
) -> Option<NavigationLocation> {
    if direction < 0 {
        take_navigation_history_target_from_stack(back, forward, current.as_ref())
    } else {
        take_navigation_history_target_from_stack(forward, back, current.as_ref())
    }
}

fn take_navigation_history_target_from_stack(
    source: &mut VecDeque<NavigationLocation>,
    destination: &mut VecDeque<NavigationLocation>,
    current: Option<&NavigationLocation>,
) -> Option<NavigationLocation> {
    while let Some(target) = source.pop_back() {
        if current.is_some_and(|current| navigation_locations_coalesce(current, &target)) {
            continue;
        }

        if let Some(current) = current {
            push_navigation_location(destination, current.clone());
        }
        return Some(target);
    }

    None
}

pub(crate) fn closed_file_entry_for_buffer(buffer: &TextBuffer) -> Option<ClosedFileEntry> {
    let path = buffer.path()?.clone();
    let position = buffer.cursor_position();
    Some(ClosedFileEntry::new(
        path,
        position.line + 1,
        position.column + 1,
    ))
}

pub(crate) fn push_closed_file_entry(
    history: &mut VecDeque<ClosedFileEntry>,
    entry: ClosedFileEntry,
) {
    push_closed_file_entry_with_limit(history, entry, CLOSED_FILE_HISTORY_LIMIT);
}

pub(crate) fn normalize_closed_file_history(
    entries: impl IntoIterator<Item = ClosedFileEntry>,
    max_entries: usize,
) -> VecDeque<ClosedFileEntry> {
    if max_entries == 0 {
        return VecDeque::new();
    }

    let mut history = VecDeque::new();
    for entry in entries {
        let entry = normalize_closed_file_entry_for_history(entry);
        push_closed_file_entry_with_limit(&mut history, entry, max_entries);
    }
    history
}

fn push_closed_file_entry_with_limit(
    history: &mut VecDeque<ClosedFileEntry>,
    mut entry: ClosedFileEntry,
    max_entries: usize,
) {
    if max_entries == 0 {
        history.clear();
        return;
    }
    prune_history_to_limit(history, max_entries);
    normalize_closed_file_entry_position(&mut entry);

    if !history_path_is_recordable(&entry.path) {
        return;
    }

    if history
        .back()
        .is_some_and(|existing| closed_file_entries_coalesce(existing, &entry))
    {
        if let Some(existing) = history.back_mut() {
            *existing = entry;
        }
        return;
    }

    history.retain(|existing| !closed_file_entries_coalesce(existing, &entry));
    history.push_back(entry);
    prune_history_to_limit(history, max_entries);
}

fn normalize_closed_file_entry_for_history(mut entry: ClosedFileEntry) -> ClosedFileEntry {
    normalize_closed_file_entry_position(&mut entry);
    entry
}

fn normalize_closed_file_entry_position(entry: &mut ClosedFileEntry) {
    entry.line = entry.line.max(1);
    entry.column = entry.column.max(1);
}

fn closed_file_entries_coalesce(left: &ClosedFileEntry, right: &ClosedFileEntry) -> bool {
    history_paths_coalesce(&left.path, &right.path)
}

#[cfg(test)]
mod tests {
    use super::{
        ClosedFileEntry, NavigationLocation, push_closed_file_entry,
        push_closed_file_entry_with_limit, push_navigation_location,
        push_navigation_location_with_limit, take_navigation_history_target,
    };
    use std::{collections::VecDeque, path::PathBuf};

    fn location(name: &str, line: usize, column: usize) -> NavigationLocation {
        NavigationLocation::new(
            PathBuf::from(format!("workspace/src/{name}.rs")),
            line,
            column,
        )
    }

    #[test]
    fn navigation_location_constructor_preserves_raw_path_and_position() {
        let path = PathBuf::from("workspace/src/../src/main.rs");
        let location = NavigationLocation::new(path.clone(), 0, 0);

        assert_eq!(location.path, path);
        assert_eq!(location.line, 0);
        assert_eq!(location.column, 0);
    }

    #[test]
    fn navigation_history_coalesces_lexical_paths_without_rewriting_newest_path() {
        let canonical = PathBuf::from("workspace/src/main.rs");
        let raw = PathBuf::from("workspace/src/./main.rs");
        let mut history = VecDeque::new();

        push_navigation_location(&mut history, NavigationLocation::new(canonical, 7, 1));
        push_navigation_location(&mut history, NavigationLocation::new(raw.clone(), 7, 9));

        assert_eq!(
            history,
            VecDeque::from([NavigationLocation::new(raw, 7, 9)])
        );
    }

    #[test]
    fn navigation_history_prunes_oversized_state_before_duplicate_scan() {
        let mut history = VecDeque::from([
            location("oldest", 1, 1),
            location("older", 2, 1),
            location("recent", 3, 1),
            location("newest", 4, 1),
        ]);

        push_navigation_location_with_limit(&mut history, location("oldest", 1, 9), 2);

        assert_eq!(
            history,
            VecDeque::from([location("newest", 4, 1), location("oldest", 1, 9)])
        );
    }

    #[test]
    fn navigation_history_clamps_stale_zero_positions_without_rewriting_path() {
        let raw = PathBuf::from("workspace/src/../src/main.rs");
        let mut history = VecDeque::new();

        push_navigation_location(&mut history, NavigationLocation::new(raw.clone(), 0, 0));

        assert_eq!(
            history,
            VecDeque::from([NavigationLocation::new(raw, 1, 1)])
        );
    }

    #[test]
    fn closed_file_history_prunes_oversized_state_before_duplicate_scan() {
        let mut history = VecDeque::from([
            ClosedFileEntry::new(PathBuf::from("workspace/src/oldest.rs"), 1, 1),
            ClosedFileEntry::new(PathBuf::from("workspace/src/older.rs"), 2, 1),
            ClosedFileEntry::new(PathBuf::from("workspace/src/recent.rs"), 3, 1),
            ClosedFileEntry::new(PathBuf::from("workspace/src/newest.rs"), 4, 1),
        ]);

        push_closed_file_entry_with_limit(
            &mut history,
            ClosedFileEntry::new(PathBuf::from("workspace/src/oldest.rs"), 9, 3),
            2,
        );

        assert_eq!(
            history,
            VecDeque::from([
                ClosedFileEntry::new(PathBuf::from("workspace/src/newest.rs"), 4, 1),
                ClosedFileEntry::new(PathBuf::from("workspace/src/oldest.rs"), 9, 3),
            ])
        );
    }

    #[test]
    fn closed_file_history_clamps_stale_zero_positions_without_rewriting_path() {
        let raw = PathBuf::from("workspace/src/../src/main.rs");
        let mut history = VecDeque::new();

        push_closed_file_entry(&mut history, ClosedFileEntry::new(raw.clone(), 0, 0));

        assert_eq!(history, VecDeque::from([ClosedFileEntry::new(raw, 1, 1)]));
    }

    #[test]
    fn navigation_history_skips_current_back_stack_entries() {
        let older = location("older", 2, 1);
        let stale_current_line = location("current", 9, 1);
        let current = location("current", 9, 7);
        let mut back = VecDeque::from([older.clone(), stale_current_line]);
        let mut forward = VecDeque::new();

        assert_eq!(
            take_navigation_history_target(&mut back, &mut forward, Some(current.clone()), -1),
            Some(older)
        );
        assert!(back.is_empty());
        assert_eq!(forward, VecDeque::from([current]));
    }

    #[test]
    fn navigation_history_skips_current_forward_stack_entries() {
        let next = location("next", 4, 1);
        let stale_current_line = location("current", 12, 2);
        let current = location("current", 12, 8);
        let mut back = VecDeque::new();
        let mut forward = VecDeque::from([next.clone(), stale_current_line]);

        assert_eq!(
            take_navigation_history_target(&mut back, &mut forward, Some(current.clone()), 1),
            Some(next)
        );
        assert_eq!(back, VecDeque::from([current]));
        assert!(forward.is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn history_coalesces_windows_case_equivalent_paths() {
        let original = PathBuf::from(r"C:\Workspace\src\main.rs");
        let case_variant = PathBuf::from(r"c:\workspace\SRC\MAIN.rs");
        let mut navigation = VecDeque::new();
        let mut closed_files = VecDeque::new();

        push_navigation_location(
            &mut navigation,
            NavigationLocation::new(original.clone(), 7, 1),
        );
        push_navigation_location(
            &mut navigation,
            NavigationLocation::new(case_variant.clone(), 7, 9),
        );
        push_closed_file_entry(
            &mut closed_files,
            ClosedFileEntry::new(original.clone(), 3, 1),
        );
        push_closed_file_entry(
            &mut closed_files,
            ClosedFileEntry::new(case_variant.clone(), 9, 4),
        );

        assert_eq!(
            navigation,
            VecDeque::from([NavigationLocation::new(case_variant.clone(), 7, 9)])
        );
        assert_eq!(
            closed_files,
            VecDeque::from([ClosedFileEntry::new(case_variant, 9, 4)])
        );
    }
}
