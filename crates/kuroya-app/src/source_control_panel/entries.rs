use super::{
    source_control_path_label, source_control_stage_label, source_control_status_label,
    source_control_status_marker, source_control_tree_path_label,
};
use kuroya_core::{
    GitChangeStage, GitFileStatus, GitStatusEntry, GitUntrackedChanges, ScmDefaultViewMode,
    ScmDefaultViewSortKey,
    text_match::{ascii_case_insensitive_contains, ascii_case_insensitive_starts_with},
};
use std::{borrow::Cow, path::Path};

pub(crate) fn source_control_filtered_entries(
    root: &Path,
    entries: &[GitStatusEntry],
    query: &str,
) -> Vec<GitStatusEntry> {
    let Some(terms) = source_control_filter_terms(query) else {
        return entries.to_vec();
    };

    entries
        .iter()
        .filter(|entry| source_control_entry_matches_filter_terms(root, entry, &terms))
        .cloned()
        .collect()
}

fn source_control_filter_entries(
    root: &Path,
    mut entries: Vec<GitStatusEntry>,
    query: &str,
) -> Vec<GitStatusEntry> {
    let Some(terms) = source_control_filter_terms(query) else {
        return entries;
    };

    entries.retain(|entry| source_control_entry_matches_filter_terms(root, entry, &terms));
    entries
}

pub(super) fn source_control_filter_visible_entries(
    root: &Path,
    entries: Cow<'_, [GitStatusEntry]>,
    query: &str,
) -> Vec<GitStatusEntry> {
    match entries {
        Cow::Borrowed(entries) => source_control_filtered_entries(root, entries, query),
        Cow::Owned(entries) => source_control_filter_entries(root, entries, query),
    }
}

pub(super) fn source_control_filter_terms(query: &str) -> Option<Vec<SourceControlFilterTerm<'_>>> {
    let mut terms = query.split_whitespace();
    let first = terms.next()?;
    let mut parsed = Vec::with_capacity(1 + terms.size_hint().0);
    parsed.push(SourceControlFilterTerm::parse(first));
    parsed.extend(terms.map(SourceControlFilterTerm::parse));
    Some(parsed)
}

fn source_control_entry_matches_filter_terms(
    root: &Path,
    entry: &GitStatusEntry,
    terms: &[SourceControlFilterTerm<'_>],
) -> bool {
    let mut path_label = None;
    let mut absolute_path = None;
    terms.iter().copied().all(|term| {
        source_control_entry_matches_filter_term(
            root,
            entry,
            term,
            &mut path_label,
            &mut absolute_path,
        )
    })
}

fn source_control_entry_matches_filter_term<'a>(
    root: &Path,
    entry: &'a GitStatusEntry,
    term: SourceControlFilterTerm<'_>,
    path_label: &mut Option<String>,
    absolute_path: &mut Option<Cow<'a, str>>,
) -> bool {
    match term {
        SourceControlFilterTerm::Scoped { scope, value } => {
            source_control_entry_matches_scoped_filter_term(entry, scope, value)
        }
        SourceControlFilterTerm::Plain(term) => {
            source_control_stage_matches_filter_value(entry.stage, term)
                || source_control_status_matches_filter_value(entry.status, term)
                || ascii_case_insensitive_contains(source_control_stage_label(entry.stage), term)
                || ascii_case_insensitive_contains(source_control_status_marker(entry.status), term)
                || ascii_case_insensitive_contains(source_control_status_label(entry.status), term)
                || ascii_case_insensitive_contains(
                    path_label.get_or_insert_with(|| source_control_path_label(root, &entry.path)),
                    term,
                )
                || ascii_case_insensitive_contains(
                    absolute_path.get_or_insert_with(|| entry.path.to_string_lossy()),
                    term,
                )
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SourceControlFilterScope {
    Stage,
    Status,
    StageOrStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SourceControlFilterTerm<'a> {
    Plain(&'a str),
    Scoped {
        scope: SourceControlFilterScope,
        value: &'a str,
    },
}

impl<'a> SourceControlFilterTerm<'a> {
    fn parse(term: &'a str) -> Self {
        source_control_scoped_filter_term(term)
            .map(|(scope, value)| Self::Scoped { scope, value })
            .unwrap_or(Self::Plain(term))
    }
}

fn source_control_scoped_filter_term(term: &str) -> Option<(SourceControlFilterScope, &str)> {
    if let Some(value) = term.strip_prefix('@') {
        return Some((SourceControlFilterScope::StageOrStatus, value));
    }

    let (scope, value) = term.split_once(':')?;
    if scope.eq_ignore_ascii_case("stage") || scope.eq_ignore_ascii_case("group") {
        Some((SourceControlFilterScope::Stage, value))
    } else if scope.eq_ignore_ascii_case("status") || scope.eq_ignore_ascii_case("type") {
        Some((SourceControlFilterScope::Status, value))
    } else if scope.eq_ignore_ascii_case("is") {
        Some((SourceControlFilterScope::StageOrStatus, value))
    } else {
        None
    }
}

fn source_control_entry_matches_scoped_filter_term(
    entry: &GitStatusEntry,
    scope: SourceControlFilterScope,
    value: &str,
) -> bool {
    match scope {
        SourceControlFilterScope::Stage => {
            source_control_stage_matches_filter_value(entry.stage, value)
        }
        SourceControlFilterScope::Status => {
            source_control_status_matches_filter_value(entry.status, value)
        }
        SourceControlFilterScope::StageOrStatus => {
            source_control_stage_matches_filter_value(entry.stage, value)
                || source_control_status_matches_filter_value(entry.status, value)
        }
    }
}

fn source_control_stage_matches_filter_value(stage: GitChangeStage, value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    source_control_stage_filter_aliases(stage)
        .iter()
        .any(|alias| source_control_filter_value_matches_alias(value, alias))
}

fn source_control_stage_filter_aliases(stage: GitChangeStage) -> &'static [&'static str] {
    match stage {
        GitChangeStage::Unstaged => &["u", "unstaged", "changes", "working", "worktree"],
        GitChangeStage::Staged => &["s", "staged", "index"],
    }
}

fn source_control_status_matches_filter_value(status: GitFileStatus, value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    source_control_status_filter_aliases(status)
        .iter()
        .any(|alias| source_control_filter_value_matches_alias(value, alias))
}

fn source_control_filter_value_matches_alias(value: &str, alias: &str) -> bool {
    if value.len() == 1 {
        alias.len() == 1 && alias.eq_ignore_ascii_case(value)
    } else {
        ascii_case_insensitive_starts_with(alias, value)
    }
}

fn source_control_status_filter_aliases(status: GitFileStatus) -> &'static [&'static str] {
    match status {
        GitFileStatus::Modified => &["m", "mod", "modified"],
        GitFileStatus::Added => &["a", "add", "added"],
        GitFileStatus::Deleted => &["d", "del", "deleted", "removed"],
        GitFileStatus::Renamed => &["r", "ren", "renamed", "moved"],
        GitFileStatus::Untracked => &["?", "u", "untracked"],
        GitFileStatus::Conflicted => &["!", "c", "conflict", "conflicted", "merge"],
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum SourceControlViewMode {
    #[default]
    List,
    Tree,
}

pub(crate) fn source_control_view_mode_label(mode: SourceControlViewMode) -> &'static str {
    match mode {
        SourceControlViewMode::List => "List",
        SourceControlViewMode::Tree => "Tree",
    }
}

pub(crate) fn source_control_view_mode_from_setting(
    mode: ScmDefaultViewMode,
) -> SourceControlViewMode {
    match mode {
        ScmDefaultViewMode::List => SourceControlViewMode::List,
        ScmDefaultViewMode::Tree => SourceControlViewMode::Tree,
    }
}

pub(super) fn source_control_next_view_mode(mode: SourceControlViewMode) -> SourceControlViewMode {
    match mode {
        SourceControlViewMode::List => SourceControlViewMode::Tree,
        SourceControlViewMode::Tree => SourceControlViewMode::List,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum SourceControlSortMode {
    #[default]
    Path,
    Name,
    Status,
}

pub(crate) fn source_control_sorted_entries(
    root: &Path,
    entries: Vec<GitStatusEntry>,
    mode: SourceControlSortMode,
) -> Vec<GitStatusEntry> {
    let mut keyed_entries = Vec::with_capacity(entries.len());
    for entry in entries {
        keyed_entries.push((source_control_entry_sort_key(root, &entry, mode), entry));
    }
    keyed_entries.sort_by(|(left_key, left), (right_key, right)| {
        left_key
            .stage
            .cmp(&right_key.stage)
            .then_with(|| {
                left_key
                    .primary
                    .cmp(&right_key.primary)
                    .then_with(|| left_key.path_label.cmp(&right_key.path_label))
            })
            .then_with(|| left.path.cmp(&right.path))
    });
    let mut sorted_entries = Vec::with_capacity(keyed_entries.len());
    for (_, entry) in keyed_entries {
        sorted_entries.push(entry);
    }
    sorted_entries
}

pub(crate) fn source_control_sort_mode_label(mode: SourceControlSortMode) -> &'static str {
    match mode {
        SourceControlSortMode::Path => "Path",
        SourceControlSortMode::Name => "Name",
        SourceControlSortMode::Status => "Status",
    }
}

pub(crate) fn source_control_sort_mode_from_setting(
    mode: ScmDefaultViewSortKey,
) -> SourceControlSortMode {
    match mode {
        ScmDefaultViewSortKey::Path => SourceControlSortMode::Path,
        ScmDefaultViewSortKey::Name => SourceControlSortMode::Name,
        ScmDefaultViewSortKey::Status => SourceControlSortMode::Status,
    }
}

pub(super) fn source_control_next_sort_mode(mode: SourceControlSortMode) -> SourceControlSortMode {
    match mode {
        SourceControlSortMode::Path => SourceControlSortMode::Name,
        SourceControlSortMode::Name => SourceControlSortMode::Status,
        SourceControlSortMode::Status => SourceControlSortMode::Path,
    }
}

fn source_control_stage_sort_key(stage: GitChangeStage) -> u8 {
    match stage {
        GitChangeStage::Unstaged => 0,
        GitChangeStage::Staged => 1,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceControlStageSectionKind {
    Changes,
    TrackedChanges,
    UntrackedChanges,
    StagedChanges,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SourceControlStageSection {
    pub(crate) stage: GitChangeStage,
    pub(crate) kind: SourceControlStageSectionKind,
    pub(crate) count: usize,
}

pub(crate) fn source_control_stage_sections(
    entries: &[GitStatusEntry],
    always_show_staged: bool,
    untracked_changes: GitUntrackedChanges,
) -> Vec<SourceControlStageSection> {
    let counts = source_control_stage_counts(entries);
    let unstaged_count = counts.tracked_unstaged + counts.untracked;
    let mut sections = Vec::with_capacity(3);
    match untracked_changes {
        GitUntrackedChanges::Separate => {
            if counts.tracked_unstaged > 0 {
                sections.push(SourceControlStageSection {
                    stage: GitChangeStage::Unstaged,
                    kind: SourceControlStageSectionKind::TrackedChanges,
                    count: counts.tracked_unstaged,
                });
            }
            if counts.untracked > 0 {
                sections.push(SourceControlStageSection {
                    stage: GitChangeStage::Unstaged,
                    kind: SourceControlStageSectionKind::UntrackedChanges,
                    count: counts.untracked,
                });
            }
        }
        GitUntrackedChanges::Mixed | GitUntrackedChanges::Hidden => {
            if unstaged_count > 0 {
                sections.push(SourceControlStageSection {
                    stage: GitChangeStage::Unstaged,
                    kind: SourceControlStageSectionKind::Changes,
                    count: unstaged_count,
                });
            }
        }
    }
    if counts.staged > 0 || always_show_staged {
        sections.push(SourceControlStageSection {
            stage: GitChangeStage::Staged,
            kind: SourceControlStageSectionKind::StagedChanges,
            count: counts.staged,
        });
    }
    sections
}

#[cfg(test)]
pub(crate) fn source_control_entries_for_untracked_changes(
    mut entries: Vec<GitStatusEntry>,
    untracked_changes: GitUntrackedChanges,
) -> Vec<GitStatusEntry> {
    if untracked_changes == GitUntrackedChanges::Hidden {
        entries.retain(|entry| entry.status != GitFileStatus::Untracked);
    }
    entries
}

pub(super) fn source_control_entries_for_untracked_changes_from_slice(
    entries: &[GitStatusEntry],
    untracked_changes: GitUntrackedChanges,
) -> Cow<'_, [GitStatusEntry]> {
    if untracked_changes != GitUntrackedChanges::Hidden {
        return Cow::Borrowed(entries);
    }

    let Some(first_untracked) = entries
        .iter()
        .position(|entry| entry.status == GitFileStatus::Untracked)
    else {
        return Cow::Borrowed(entries);
    };

    let mut visible_entries = Vec::with_capacity(entries.len().saturating_sub(1));
    visible_entries.extend_from_slice(&entries[..first_untracked]);
    visible_entries.extend(
        entries[first_untracked + 1..]
            .iter()
            .filter(|entry| entry.status != GitFileStatus::Untracked)
            .cloned(),
    );
    Cow::Owned(visible_entries)
}

fn source_control_visible_row_section_kind(
    entry: &GitStatusEntry,
    untracked_changes: GitUntrackedChanges,
) -> SourceControlStageSectionKind {
    match entry.stage {
        GitChangeStage::Staged => SourceControlStageSectionKind::StagedChanges,
        GitChangeStage::Unstaged => match untracked_changes {
            GitUntrackedChanges::Separate if entry.status == GitFileStatus::Untracked => {
                SourceControlStageSectionKind::UntrackedChanges
            }
            GitUntrackedChanges::Separate => SourceControlStageSectionKind::TrackedChanges,
            GitUntrackedChanges::Mixed | GitUntrackedChanges::Hidden => {
                SourceControlStageSectionKind::Changes
            }
        },
    }
}

struct SourceControlEntrySortKey {
    stage: u8,
    primary: String,
    path_label: String,
}

fn source_control_entry_sort_key(
    root: &Path,
    entry: &GitStatusEntry,
    mode: SourceControlSortMode,
) -> SourceControlEntrySortKey {
    let path_label = source_control_path_label(root, &entry.path);
    SourceControlEntrySortKey {
        stage: source_control_stage_sort_key(entry.stage),
        primary: source_control_entry_primary_sort_key(root, entry, mode, &path_label),
        path_label,
    }
}

fn source_control_entry_primary_sort_key(
    root: &Path,
    entry: &GitStatusEntry,
    mode: SourceControlSortMode,
    path_label: &str,
) -> String {
    let key = match mode {
        SourceControlSortMode::Path => source_control_path_sort_key(root, &entry.path),
        SourceControlSortMode::Name => entry
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_owned(),
        SourceControlSortMode::Status => {
            let status = source_control_status_label(entry.status);
            let mut key = String::with_capacity(status.len() + 1 + path_label.len());
            key.push_str(status);
            key.push(' ');
            key.push_str(path_label);
            key
        }
    };
    key.to_ascii_lowercase()
}

fn source_control_path_sort_key(root: &Path, path: &Path) -> String {
    source_control_tree_path_label(root, path, true)
}

pub(crate) fn source_control_tree_row_indent(
    root: &Path,
    path: &Path,
    view_mode: SourceControlViewMode,
    compact_folders: bool,
) -> f32 {
    if view_mode != SourceControlViewMode::Tree || compact_folders {
        return 0.0;
    }
    path.strip_prefix(root)
        .ok()
        .and_then(|relative| relative.parent())
        .map(|parent| parent.components().count() as f32 * 12.0)
        .unwrap_or(0.0)
}

#[cfg(test)]
pub(crate) fn source_control_visible_entries(
    entries: &[GitStatusEntry],
    untracked_changes: GitUntrackedChanges,
    unstaged_collapsed: bool,
    untracked_collapsed: bool,
    staged_collapsed: bool,
) -> Vec<&GitStatusEntry> {
    entries
        .iter()
        .filter(|entry| {
            source_control_entry_visible_for_untracked_changes(entry, untracked_changes)
                && !source_control_entry_collapsed(
                    entry,
                    untracked_changes,
                    unstaged_collapsed,
                    untracked_collapsed,
                    staged_collapsed,
                )
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceControlVisibleRow {
    Header(SourceControlStageSection),
    Entry {
        entry_index: usize,
        visible_index: usize,
    },
}

pub(crate) fn source_control_visible_rows(
    entries: &[GitStatusEntry],
    always_show_staged: bool,
    untracked_changes: GitUntrackedChanges,
    unstaged_collapsed: bool,
    untracked_collapsed: bool,
    staged_collapsed: bool,
) -> Vec<SourceControlVisibleRow> {
    let sections = source_control_stage_sections(entries, always_show_staged, untracked_changes);
    let mut expanded_count = 0;
    let mut collect_changes = false;
    let mut collect_tracked = false;
    let mut collect_untracked = false;
    let mut collect_staged = false;
    let mut changes_capacity = 0;
    let mut tracked_capacity = 0;
    let mut untracked_capacity = 0;
    let mut staged_capacity = 0;
    for section in &sections {
        if source_control_section_collapsed(
            section.kind,
            unstaged_collapsed,
            untracked_collapsed,
            staged_collapsed,
        ) {
            continue;
        }
        expanded_count += section.count;
        match section.kind {
            SourceControlStageSectionKind::Changes => {
                collect_changes = true;
                changes_capacity = section.count;
            }
            SourceControlStageSectionKind::TrackedChanges => {
                collect_tracked = true;
                tracked_capacity = section.count;
            }
            SourceControlStageSectionKind::UntrackedChanges => {
                collect_untracked = true;
                untracked_capacity = section.count;
            }
            SourceControlStageSectionKind::StagedChanges => {
                collect_staged = true;
                staged_capacity = section.count;
            }
        }
    }

    let mut changes = Vec::with_capacity(changes_capacity);
    let mut tracked = Vec::with_capacity(tracked_capacity);
    let mut untracked = Vec::with_capacity(untracked_capacity);
    let mut staged = Vec::with_capacity(staged_capacity);
    if expanded_count > 0 {
        for (entry_index, entry) in entries.iter().enumerate() {
            match source_control_visible_row_section_kind(entry, untracked_changes) {
                SourceControlStageSectionKind::Changes if collect_changes => {
                    changes.push(entry_index);
                }
                SourceControlStageSectionKind::TrackedChanges if collect_tracked => {
                    tracked.push(entry_index);
                }
                SourceControlStageSectionKind::UntrackedChanges if collect_untracked => {
                    untracked.push(entry_index);
                }
                SourceControlStageSectionKind::StagedChanges if collect_staged => {
                    staged.push(entry_index);
                }
                _ => {}
            }
        }
    }

    let mut rows = Vec::with_capacity(sections.len() + expanded_count);
    let mut visible_index = 0;
    for section in sections {
        rows.push(SourceControlVisibleRow::Header(section));
        if source_control_section_collapsed(
            section.kind,
            unstaged_collapsed,
            untracked_collapsed,
            staged_collapsed,
        ) {
            continue;
        }

        let entry_indices = match section.kind {
            SourceControlStageSectionKind::Changes => &changes,
            SourceControlStageSectionKind::TrackedChanges => &tracked,
            SourceControlStageSectionKind::UntrackedChanges => &untracked,
            SourceControlStageSectionKind::StagedChanges => &staged,
        };
        for &entry_index in entry_indices {
            rows.push(SourceControlVisibleRow::Entry {
                entry_index,
                visible_index,
            });
            visible_index += 1;
        }
    }
    rows
}

pub(super) fn source_control_visible_entry_count(rows: &[SourceControlVisibleRow]) -> usize {
    rows.iter()
        .rev()
        .find_map(|row| match row {
            SourceControlVisibleRow::Entry { visible_index, .. } => Some(visible_index + 1),
            SourceControlVisibleRow::Header(_) => None,
        })
        .unwrap_or(0)
}

pub(super) fn source_control_visible_entry_for_selection<'a>(
    entries: &'a [GitStatusEntry],
    rows: &[SourceControlVisibleRow],
    selected: usize,
) -> Option<&'a GitStatusEntry> {
    let entry_index = source_control_visible_entry_index_for_selection(rows, selected)?;
    entries.get(entry_index)
}

pub(super) fn source_control_visible_entry_index_for_selection(
    rows: &[SourceControlVisibleRow],
    selected: usize,
) -> Option<usize> {
    rows.iter().find_map(|row| match row {
        SourceControlVisibleRow::Entry {
            entry_index,
            visible_index,
        } if *visible_index == selected => Some(*entry_index),
        _ => None,
    })
}

#[cfg(test)]
pub(crate) fn source_control_visible_row_index_for_selection(
    rows: &[SourceControlVisibleRow],
    selected: usize,
) -> Option<usize> {
    rows.iter().position(|row| {
        matches!(
            row,
            SourceControlVisibleRow::Entry { visible_index, .. } if *visible_index == selected
        )
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SourceControlRevealSelection {
    pub(crate) selected: usize,
    pub(crate) stage: GitChangeStage,
    pub(crate) unstaged_collapsed: bool,
    pub(crate) untracked_collapsed: bool,
    pub(crate) staged_collapsed: bool,
}

pub(crate) fn source_control_reveal_selection(
    entries: &[GitStatusEntry],
    path: &Path,
    preferred_stage: Option<GitChangeStage>,
    untracked_changes: GitUntrackedChanges,
    unstaged_collapsed: bool,
    untracked_collapsed: bool,
    staged_collapsed: bool,
) -> Option<SourceControlRevealSelection> {
    let target_index =
        source_control_reveal_entry_index(entries, path, preferred_stage, untracked_changes)?;
    let target_entry = &entries[target_index];
    let stage = target_entry.stage;
    let target_kind = source_control_section_kind_for_entry(target_entry, untracked_changes)?;
    let unstaged_collapsed = unstaged_collapsed
        && !matches!(
            target_kind,
            SourceControlStageSectionKind::Changes | SourceControlStageSectionKind::TrackedChanges
        );
    let untracked_collapsed =
        untracked_collapsed && target_kind != SourceControlStageSectionKind::UntrackedChanges;
    let staged_collapsed =
        staged_collapsed && target_kind != SourceControlStageSectionKind::StagedChanges;
    let selected = entries
        .iter()
        .take(target_index)
        .filter(|entry| {
            source_control_entry_visible_for_untracked_changes(entry, untracked_changes)
                && !source_control_entry_collapsed(
                    entry,
                    untracked_changes,
                    unstaged_collapsed,
                    untracked_collapsed,
                    staged_collapsed,
                )
        })
        .count();

    Some(SourceControlRevealSelection {
        selected,
        stage,
        unstaged_collapsed,
        untracked_collapsed,
        staged_collapsed,
    })
}

pub(crate) fn source_control_auto_reveal_selection(
    entries: &[GitStatusEntry],
    path: &Path,
    preferred_stage: Option<GitChangeStage>,
    untracked_changes: GitUntrackedChanges,
    auto_reveal: bool,
    source_control_visible: bool,
    unstaged_collapsed: bool,
    untracked_collapsed: bool,
    staged_collapsed: bool,
) -> Option<SourceControlRevealSelection> {
    if !auto_reveal || !source_control_visible {
        return None;
    }
    source_control_reveal_selection(
        entries,
        path,
        preferred_stage,
        untracked_changes,
        unstaged_collapsed,
        untracked_collapsed,
        staged_collapsed,
    )
}

fn source_control_reveal_entry_index(
    entries: &[GitStatusEntry],
    path: &Path,
    preferred_stage: Option<GitChangeStage>,
    untracked_changes: GitUntrackedChanges,
) -> Option<usize> {
    let mut preferred = None;
    let mut unstaged = None;
    let mut staged = None;
    for (index, entry) in entries.iter().enumerate() {
        if !source_control_entry_visible_for_untracked_changes(entry, untracked_changes)
            || entry.path != path
        {
            continue;
        }

        if preferred_stage == Some(entry.stage) && preferred.is_none() {
            preferred = Some(index);
        }
        match entry.stage {
            GitChangeStage::Unstaged if unstaged.is_none() => unstaged = Some(index),
            GitChangeStage::Staged if staged.is_none() => staged = Some(index),
            _ => {}
        }
    }
    preferred.or(unstaged).or(staged)
}

fn source_control_entry_visible_for_untracked_changes(
    entry: &GitStatusEntry,
    untracked_changes: GitUntrackedChanges,
) -> bool {
    untracked_changes != GitUntrackedChanges::Hidden || entry.status != GitFileStatus::Untracked
}

fn source_control_entry_collapsed(
    entry: &GitStatusEntry,
    untracked_changes: GitUntrackedChanges,
    unstaged_collapsed: bool,
    untracked_collapsed: bool,
    staged_collapsed: bool,
) -> bool {
    source_control_section_kind_for_entry(entry, untracked_changes).is_some_and(|kind| {
        source_control_section_collapsed(
            kind,
            unstaged_collapsed,
            untracked_collapsed,
            staged_collapsed,
        )
    })
}

fn source_control_section_kind_for_entry(
    entry: &GitStatusEntry,
    untracked_changes: GitUntrackedChanges,
) -> Option<SourceControlStageSectionKind> {
    match entry.stage {
        GitChangeStage::Staged => Some(SourceControlStageSectionKind::StagedChanges),
        GitChangeStage::Unstaged => match untracked_changes {
            GitUntrackedChanges::Separate if entry.status == GitFileStatus::Untracked => {
                Some(SourceControlStageSectionKind::UntrackedChanges)
            }
            GitUntrackedChanges::Separate => Some(SourceControlStageSectionKind::TrackedChanges),
            GitUntrackedChanges::Mixed => Some(SourceControlStageSectionKind::Changes),
            GitUntrackedChanges::Hidden if entry.status == GitFileStatus::Untracked => None,
            GitUntrackedChanges::Hidden => Some(SourceControlStageSectionKind::Changes),
        },
    }
}

pub(super) fn source_control_section_collapsed(
    kind: SourceControlStageSectionKind,
    unstaged_collapsed: bool,
    untracked_collapsed: bool,
    staged_collapsed: bool,
) -> bool {
    match kind {
        SourceControlStageSectionKind::Changes | SourceControlStageSectionKind::TrackedChanges => {
            unstaged_collapsed
        }
        SourceControlStageSectionKind::UntrackedChanges => untracked_collapsed,
        SourceControlStageSectionKind::StagedChanges => staged_collapsed,
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct SourceControlStageCounts {
    tracked_unstaged: usize,
    untracked: usize,
    staged: usize,
}

fn source_control_stage_counts(entries: &[GitStatusEntry]) -> SourceControlStageCounts {
    let mut counts = SourceControlStageCounts::default();
    for entry in entries {
        match entry.stage {
            GitChangeStage::Staged => counts.staged += 1,
            GitChangeStage::Unstaged if entry.status == GitFileStatus::Untracked => {
                counts.untracked += 1;
            }
            GitChangeStage::Unstaged => counts.tracked_unstaged += 1,
        }
    }
    counts
}
