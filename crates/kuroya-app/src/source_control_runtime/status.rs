use crate::{
    path_display::{display_error_label_cow, display_path_label_cow, sanitized_display_label_cow},
    source_control_branch_picker::source_control_branch_display_name,
    workspace_trust::workspace_path_contains_lexically,
};
use kuroya_core::{
    BufferId, GitBranchProtectionPrompt, GitChangeStage, GitFileStatus,
    GitPromptToSaveFilesBeforeCommit, GitSmartCommitChanges, GitStatusEntry, TextBuffer,
};
use std::{
    borrow::Cow,
    collections::HashSet,
    path::{Path, PathBuf},
};

pub(super) const SOURCE_CONTROL_COMMIT_HASH_DISPLAY_MAX_CHARS: usize = 64;
pub(super) const SOURCE_CONTROL_PROTECTED_BRANCH_PATTERN_DISPLAY_MAX_CHARS: usize = 160;

pub(crate) fn git_stage_pending_status(paths: &[PathBuf]) -> String {
    git_stage_status("Staging", paths)
}

pub(crate) fn git_progress_status(show_progress: bool, status: String) -> Option<String> {
    show_progress.then_some(status)
}

pub(super) fn no_source_control_changes_status(path: &Path) -> String {
    let path_label = source_control_status_path_label_cow(path);
    format!("No source control changes in {}", path_label.as_ref())
}

pub(super) fn source_control_revealed_status(path: &Path) -> String {
    let path_label = source_control_status_path_label_cow(path);
    format!("Revealed {} in Source Control", path_label.as_ref())
}

pub(super) fn no_unstaged_changes_status(path: &Path) -> String {
    let path_label = source_control_status_path_label_cow(path);
    format!("No unstaged changes in {}", path_label.as_ref())
}

pub(super) fn no_staged_changes_status(path: &Path) -> String {
    let path_label = source_control_status_path_label_cow(path);
    format!("No staged changes in {}", path_label.as_ref())
}

pub(super) fn first_stale_source_control_path(
    paths: &[PathBuf],
    mut is_current: impl FnMut(&Path) -> bool,
) -> Option<&Path> {
    paths
        .iter()
        .map(PathBuf::as_path)
        .find(|path| !is_current(path))
}

pub(super) fn first_stale_source_control_operation_path<'a>(
    paths: &'a [PathBuf],
    operation_root: &Path,
    mut is_current: impl FnMut(&Path) -> bool,
) -> Option<&'a Path> {
    first_stale_source_control_path(paths, |path| {
        workspace_path_contains_lexically(operation_root, path) && is_current(path)
    })
}

pub(super) fn stale_source_control_stage_status(
    stage: GitChangeStage,
    paths: &[PathBuf],
    stale_path: &Path,
) -> String {
    if paths.len() == 1 {
        match stage {
            GitChangeStage::Staged => no_staged_changes_status(stale_path),
            GitChangeStage::Unstaged => no_unstaged_changes_status(stale_path),
        }
    } else {
        format!(
            "Source control selection changed; refresh before {}",
            source_control_stage_action_label(stage)
        )
    }
}

fn source_control_stage_action_label(stage: GitChangeStage) -> &'static str {
    match stage {
        GitChangeStage::Staged => "unstaging changes",
        GitChangeStage::Unstaged => "staging changes",
    }
}

pub(super) fn stale_source_control_discard_status(paths: &[PathBuf], stale_path: &Path) -> String {
    if paths.len() == 1 {
        no_source_control_changes_status(stale_path)
    } else {
        "Source control selection changed; refresh before discarding changes".to_owned()
    }
}

pub(crate) fn source_control_save_pause_external_change_status(
    action: &str,
    count: usize,
) -> String {
    format!(
        "{action} paused; {} changed on disk",
        source_control_file_count(count)
    )
}

pub(crate) fn source_control_save_pause_unsaved_status(action: &str, count: usize) -> String {
    let files = source_control_file_count(count);
    let verb = if count == 1 { "has" } else { "have" };
    format!("{action} paused; {files} still {verb} unsaved changes")
}

fn source_control_file_count(count: usize) -> String {
    if count == 1 {
        "1 file".to_owned()
    } else {
        format!("{count} files")
    }
}

pub(crate) fn git_stage_success_status(paths: &[PathBuf]) -> String {
    git_stage_status("Staged", paths)
}

pub(crate) fn git_stage_failure_status(paths: &[PathBuf], error: &str) -> String {
    let target = git_source_control_target(paths);
    let error = display_error_label_cow(error);
    format!("Could not stage {target}: {}", error.as_ref())
}

fn git_stage_status(prefix: &str, paths: &[PathBuf]) -> String {
    format!("{prefix} {}", git_source_control_target(paths))
}

pub(crate) fn git_unstage_pending_status(paths: &[PathBuf]) -> String {
    git_unstage_status("Unstaging", paths)
}

pub(crate) fn git_unstage_success_status(paths: &[PathBuf]) -> String {
    git_unstage_status("Unstaged", paths)
}

pub(crate) fn git_unstage_failure_status(paths: &[PathBuf], error: &str) -> String {
    let target = git_source_control_target(paths);
    let error = display_error_label_cow(error);
    format!("Could not unstage {target}: {}", error.as_ref())
}

fn git_unstage_status(prefix: &str, paths: &[PathBuf]) -> String {
    format!("{prefix} {}", git_source_control_target(paths))
}

pub(crate) fn git_discard_pending_status(paths: &[PathBuf]) -> String {
    git_discard_status("Discarding", paths)
}

pub(crate) fn git_discard_success_status(paths: &[PathBuf]) -> String {
    git_discard_status("Discarded", paths)
}

pub(crate) fn git_discard_failure_status(paths: &[PathBuf], error: &str) -> String {
    let target = git_source_control_target(paths);
    let error = display_error_label_cow(error);
    format!("Could not discard {target}: {}", error.as_ref())
}

fn git_discard_status(prefix: &str, paths: &[PathBuf]) -> String {
    format!("{prefix} {}", git_source_control_target(paths))
}

pub(crate) fn git_commit_pending_status(smart_commit: bool) -> String {
    if smart_commit {
        "Smart committing changes".to_owned()
    } else {
        "Committing staged changes".to_owned()
    }
}

pub(crate) fn git_commit_success_status(short_oid: &str, smart_commit: bool) -> String {
    let short_oid = git_commit_hash_display(short_oid);
    if smart_commit {
        format!("Smart committed changes ({short_oid})")
    } else {
        format!("Committed staged changes ({short_oid})")
    }
}

pub(super) fn git_commit_hash_display(short_oid: &str) -> String {
    git_commit_hash_display_cow(short_oid).into_owned()
}

pub(super) fn git_commit_hash_display_cow(short_oid: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(
        short_oid,
        SOURCE_CONTROL_COMMIT_HASH_DISPLAY_MAX_CHARS,
        "unknown",
    )
}

pub(crate) fn git_commit_failure_status(error: &str, smart_commit: bool) -> String {
    let error = display_error_label_cow(error);
    if smart_commit {
        format!("Could not smart commit changes: {}", error.as_ref())
    } else {
        format!("Could not commit staged changes: {}", error.as_ref())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SourceControlProtectedBranchCommitAction {
    Allow,
    Prompt { pattern: String },
    RequireNewBranch { pattern: String },
}

pub(crate) fn source_control_protected_branch_commit_action(
    branch: Option<&str>,
    patterns: &[String],
    prompt: GitBranchProtectionPrompt,
) -> SourceControlProtectedBranchCommitAction {
    let Some(branch) = branch else {
        return SourceControlProtectedBranchCommitAction::Allow;
    };
    let Some(pattern) = source_control_protected_branch_match(branch, patterns) else {
        return SourceControlProtectedBranchCommitAction::Allow;
    };

    match prompt {
        GitBranchProtectionPrompt::AlwaysCommit => SourceControlProtectedBranchCommitAction::Allow,
        GitBranchProtectionPrompt::AlwaysCommitToNewBranch => {
            SourceControlProtectedBranchCommitAction::RequireNewBranch { pattern }
        }
        GitBranchProtectionPrompt::AlwaysPrompt => {
            SourceControlProtectedBranchCommitAction::Prompt { pattern }
        }
    }
}

pub(crate) fn source_control_protected_branch_match(
    branch: &str,
    patterns: &[String],
) -> Option<String> {
    patterns
        .iter()
        .map(|pattern| pattern.trim())
        .filter(|pattern| !pattern.is_empty())
        .find(|pattern| source_control_branch_protection_pattern_matches(pattern, branch))
        .map(ToOwned::to_owned)
}

pub(crate) fn source_control_branch_protection_pattern_matches(
    pattern: &str,
    branch: &str,
) -> bool {
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return false;
    }
    if pattern == branch {
        return true;
    }
    if pattern == "*" {
        return true;
    }
    if !pattern.contains('*') {
        return false;
    }

    let mut remaining = branch;
    let mut first = true;
    for part in pattern.split('*') {
        if part.is_empty() {
            first = false;
            continue;
        }
        if first {
            let Some(stripped) = remaining.strip_prefix(part) else {
                return false;
            };
            remaining = stripped;
        } else {
            let Some(index) = remaining.find(part) else {
                return false;
            };
            remaining = &remaining[index + part.len()..];
        }
        first = false;
    }

    pattern.ends_with('*') || remaining.is_empty()
}

#[cfg(test)]
pub(crate) fn source_control_protected_branch_prompt_title(branch: &str) -> String {
    format!("Commit to protected branch {branch}?")
}

#[cfg(test)]
pub(crate) fn source_control_protected_branch_prompt_body(branch: &str, pattern: &str) -> String {
    format!("Branch {branch} matches protected branch pattern {pattern}.")
}

pub(crate) fn source_control_protected_branch_new_branch_required_status(
    branch: &str,
    pattern: &str,
) -> String {
    format!(
        "Branch {} is protected by {}; create or switch branches before committing",
        source_control_branch_display_name(branch),
        source_control_protected_branch_pattern_display(pattern)
    )
}

pub(super) fn source_control_protected_branch_pattern_display(pattern: &str) -> String {
    source_control_protected_branch_pattern_display_cow(pattern).into_owned()
}

pub(super) fn source_control_protected_branch_pattern_display_cow(pattern: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(
        pattern,
        SOURCE_CONTROL_PROTECTED_BRANCH_PATTERN_DISPLAY_MAX_CHARS,
        "protected branch pattern",
    )
}

pub(super) fn smart_commit_path_count(
    entries: &[GitStatusEntry],
    smart_commit_changes: GitSmartCommitChanges,
) -> usize {
    source_control_unique_path_count(
        entries
            .iter()
            .filter(|entry| smart_commit_entry_included(entry, smart_commit_changes))
            .map(|entry| entry.path.as_path()),
    )
}

pub(super) fn source_control_has_stage(entries: &[GitStatusEntry], stage: GitChangeStage) -> bool {
    entries.iter().any(|entry| entry.stage == stage)
}

#[cfg(test)]
pub(super) fn source_control_stage_path_count(
    entries: &[GitStatusEntry],
    stage: GitChangeStage,
) -> usize {
    source_control_unique_path_count(
        entries
            .iter()
            .filter(|entry| entry.stage == stage)
            .map(|entry| entry.path.as_path()),
    )
}

pub(super) fn source_control_paths_for_stage(
    entries: &[GitStatusEntry],
    stage: GitChangeStage,
) -> Vec<PathBuf> {
    let mut seen = HashSet::with_capacity(entries.len());
    let mut paths = Vec::with_capacity(entries.len());
    for entry in entries {
        if entry.stage == stage {
            let path = entry.path.as_path();
            if seen.insert(path) {
                paths.push(path.to_path_buf());
            }
        }
    }
    paths
}

fn source_control_unique_path_count<'a>(paths: impl Iterator<Item = &'a Path>) -> usize {
    let (lower, upper) = paths.size_hint();
    let mut seen = HashSet::with_capacity(upper.unwrap_or(lower));
    paths.filter(|path| seen.insert(*path)).count()
}

pub(crate) fn source_control_commit_save_prompt_ids(
    buffers: &[TextBuffer],
    entries: &[GitStatusEntry],
    behavior: GitPromptToSaveFilesBeforeCommit,
) -> Vec<BufferId> {
    source_control_commit_save_prompt_ids_for_commit(buffers, entries, behavior, None)
}

pub(crate) fn source_control_commit_save_prompt_ids_for_commit(
    buffers: &[TextBuffer],
    entries: &[GitStatusEntry],
    behavior: GitPromptToSaveFilesBeforeCommit,
    smart_commit_changes: Option<GitSmartCommitChanges>,
) -> Vec<BufferId> {
    if behavior == GitPromptToSaveFilesBeforeCommit::Never {
        return Vec::new();
    }

    let staged_paths = (behavior == GitPromptToSaveFilesBeforeCommit::Staged).then(|| {
        let mut paths = HashSet::with_capacity(entries.len());
        for entry in entries {
            if entry.stage == GitChangeStage::Staged
                || smart_commit_changes
                    .is_some_and(|changes| smart_commit_entry_included(entry, changes))
            {
                paths.insert(entry.path.as_path());
            }
        }
        paths
    });

    let mut ids = Vec::with_capacity(buffers.len());
    for buffer in buffers {
        if buffer.is_dirty()
            && staged_paths.as_ref().is_none_or(|paths| {
                buffer
                    .path()
                    .is_some_and(|path| paths.contains(&path.as_path()))
            })
        {
            ids.push(buffer.id());
        }
    }
    ids
}

fn smart_commit_entry_included(
    entry: &GitStatusEntry,
    smart_commit_changes: GitSmartCommitChanges,
) -> bool {
    if entry.stage != GitChangeStage::Unstaged {
        return false;
    }
    match entry.status {
        GitFileStatus::Modified | GitFileStatus::Deleted | GitFileStatus::Renamed => true,
        GitFileStatus::Untracked => smart_commit_changes == GitSmartCommitChanges::All,
        GitFileStatus::Added | GitFileStatus::Conflicted => false,
    }
}

pub(super) fn git_source_control_target(paths: &[PathBuf]) -> String {
    if paths.len() == 1 {
        let path_label = source_control_status_path_label_cow(&paths[0]);
        format!("changes in {}", path_label.as_ref())
    } else {
        format!("changes in {} files", paths.len())
    }
}

fn source_control_status_path_label_cow(path: &Path) -> Cow<'_, str> {
    display_path_label_cow(path)
}
