use crate::{
    file_io::file_too_large_message,
    large_file_mode::{LARGE_FILE_MODE_MAX_BYTES, LARGE_FILE_MODE_MAX_LINES},
    path_display::{
        DISPLAY_PATH_LABEL_MAX_CHARS, display_error_label_cow, display_path_label_cow,
        sanitized_display_label_cow,
    },
};
use kuroya_core::{GitChangeStage, TextBuffer};
#[cfg(test)]
use kuroya_core::{GitCommitSummary, GitStashEntry};
use std::{borrow::Cow, path::Path};

pub(super) const SOURCE_CONTROL_DIFF_STATUS_MAX_CHARS: usize = 240;
const ACCESSIBLE_DIFF_SUFFIX: &str = " (Accessible Diff)";

pub(crate) fn diff_label_for_path(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(source_control_diff_display_label)
        .unwrap_or_else(|| source_control_diff_path_label(path))
        .into_owned()
}

pub(crate) fn accessible_diff_label(label: &str) -> String {
    accessible_diff_label_cow(label).into_owned()
}

fn accessible_diff_label_owned(label: String) -> String {
    let owned_label = {
        let raw = label.as_str();
        match accessible_diff_label_cow(raw) {
            Cow::Borrowed(label) => {
                let borrowed_original =
                    !raw.is_empty() && label.as_ptr() == raw.as_ptr() && label.len() == raw.len();
                if borrowed_original {
                    None
                } else {
                    Some(label.to_owned())
                }
            }
            Cow::Owned(label) => Some(label),
        }
    };

    match owned_label {
        Some(label) => label,
        None => label,
    }
}

pub(super) fn accessible_diff_label_cow<'a>(label: &'a str) -> Cow<'a, str> {
    let label = source_control_diff_display_label_cow(label);
    if label.ends_with(ACCESSIBLE_DIFF_SUFFIX) {
        return label;
    }

    let mut accessible_label = String::with_capacity(label.len() + ACCESSIBLE_DIFF_SUFFIX.len());
    accessible_label.push_str(label.as_ref());
    accessible_label.push_str(ACCESSIBLE_DIFF_SUFFIX);
    Cow::Owned(source_control_diff_display_label_owned(accessible_label))
}

pub(crate) fn diff_buffer_display_label(label: String, only_accessible: bool) -> String {
    if only_accessible {
        accessible_diff_label_owned(label)
    } else {
        source_control_diff_display_label_owned(label)
    }
}

pub(crate) fn diff_buffer_display_kind(kind: &str, only_accessible: bool) -> &str {
    if only_accessible {
        "accessible diff"
    } else {
        kind
    }
}

#[cfg(test)]
pub(crate) fn join_unified_patches(patches: Vec<String>) -> String {
    let mut joined = String::new();
    for patch in patches {
        let patch = patch.trim_end();
        if patch.is_empty() {
            continue;
        }
        if !joined.is_empty() {
            joined.push('\n');
        }
        joined.push_str(patch);
    }
    joined
}

pub(super) fn source_control_diff_path_label(path: &Path) -> Cow<'_, str> {
    display_path_label_cow(path)
}

pub(super) fn source_control_diff_display_label(label: &str) -> Cow<'_, str> {
    source_control_diff_display_label_cow(label)
}

fn source_control_diff_display_label_owned(label: String) -> String {
    let owned_label = {
        let raw = label.as_str();
        match source_control_diff_display_label_cow(raw) {
            Cow::Borrowed(label) => {
                let borrowed_original =
                    !raw.is_empty() && label.as_ptr() == raw.as_ptr() && label.len() == raw.len();
                if borrowed_original {
                    None
                } else {
                    Some(label.to_owned())
                }
            }
            Cow::Owned(label) => Some(label),
        }
    };

    match owned_label {
        Some(label) => label,
        None => label,
    }
}

pub(super) fn source_control_diff_display_label_cow<'a>(label: &'a str) -> Cow<'a, str> {
    sanitized_display_label_cow(label, DISPLAY_PATH_LABEL_MAX_CHARS, "diff")
}

fn source_control_diff_error_label(error: &str) -> Cow<'_, str> {
    display_error_label_cow(error)
}

pub(super) fn source_control_diff_status_text<'a>(value: impl Into<Cow<'a, str>>) -> String {
    match value.into() {
        Cow::Borrowed(value) => source_control_diff_status_text_cow(value).into_owned(),
        Cow::Owned(value) => source_control_diff_status_text_owned(value),
    }
}

fn source_control_diff_status_text_cow(value: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(
        value,
        SOURCE_CONTROL_DIFF_STATUS_MAX_CHARS,
        "Source control diff status unavailable",
    )
}

fn source_control_diff_status_text_owned(value: String) -> String {
    let normalized = {
        let raw = value.as_str();
        match source_control_diff_status_text_cow(raw) {
            Cow::Borrowed(label) => {
                let borrowed_original =
                    !raw.is_empty() && label.as_ptr() == raw.as_ptr() && label.len() == raw.len();
                if borrowed_original {
                    None
                } else {
                    Some(label.to_owned())
                }
            }
            Cow::Owned(label) => Some(label),
        }
    };

    normalized.unwrap_or(value)
}

#[cfg(test)]
pub(crate) fn source_control_patch_copy_success_status(
    stage: GitChangeStage,
    path: &Path,
) -> String {
    source_control_diff_status_text(format!(
        "Copied {}patch for {}",
        source_control_patch_stage_prefix(stage),
        source_control_diff_path_label(path)
    ))
}

#[cfg(test)]
pub(crate) fn source_control_patch_copy_empty_status(stage: GitChangeStage, path: &Path) -> String {
    source_control_diff_status_text(format!(
        "No {}patch to copy for {}",
        source_control_patch_stage_prefix(stage),
        source_control_diff_path_label(path)
    ))
}

#[cfg(test)]
pub(crate) fn source_control_patch_copy_failure_status(
    stage: GitChangeStage,
    path: &Path,
    error: &str,
) -> String {
    source_control_diff_status_text(format!(
        "Could not copy {}patch for {}: {}",
        source_control_patch_stage_prefix(stage),
        source_control_diff_path_label(path),
        source_control_diff_error_label(error)
    ))
}

#[cfg(test)]
fn source_control_patch_stage_prefix(stage: GitChangeStage) -> &'static str {
    match stage {
        GitChangeStage::Staged => "staged ",
        GitChangeStage::Unstaged => "",
    }
}

#[cfg(test)]
pub(crate) fn source_control_all_patch_copy_success_status(count: usize) -> String {
    let noun = if count == 1 { "change" } else { "changes" };
    source_control_diff_status_text(format!("Copied all changes patch for {count} {noun}"))
}

#[cfg(test)]
pub(crate) fn source_control_all_patch_copy_empty_status() -> String {
    "No changes patch to copy".to_owned()
}

#[cfg(test)]
pub(crate) fn source_control_all_patch_copy_failure_status(error: &str) -> String {
    source_control_diff_status_text(format!(
        "Could not copy all changes patch: {}",
        source_control_diff_error_label(error)
    ))
}

#[cfg(test)]
pub(crate) fn source_control_stage_patch_copy_success_status(
    stage: GitChangeStage,
    count: usize,
) -> String {
    let stage_label = source_control_patch_group_label(stage);
    let noun = if count == 1 { "file" } else { "files" };
    source_control_diff_status_text(format!("Copied {stage_label} patch for {count} {noun}"))
}

#[cfg(test)]
pub(crate) fn source_control_stage_patch_copy_empty_status(stage: GitChangeStage) -> String {
    let stage_label = source_control_patch_group_label(stage);
    source_control_diff_status_text(format!("No {stage_label} patch to copy"))
}

#[cfg(test)]
pub(crate) fn source_control_stage_patch_copy_failure_status(
    stage: GitChangeStage,
    error: &str,
) -> String {
    let stage_label = source_control_patch_group_label(stage);
    source_control_diff_status_text(format!(
        "Could not copy {stage_label} patch: {}",
        source_control_diff_error_label(error)
    ))
}

#[cfg(test)]
fn source_control_patch_group_label(stage: GitChangeStage) -> &'static str {
    match stage {
        GitChangeStage::Staged => "staged",
        GitChangeStage::Unstaged => "unstaged",
    }
}

pub(crate) fn source_control_open_all_stage_empty_status(stage: GitChangeStage) -> String {
    match stage {
        GitChangeStage::Staged => "No staged changes to open".to_owned(),
        GitChangeStage::Unstaged => "No unstaged changes to open".to_owned(),
    }
}

pub(crate) fn source_control_open_all_stage_success_status(
    stage: GitChangeStage,
    count: usize,
) -> String {
    let stage_label = match stage {
        GitChangeStage::Staged => "staged",
        GitChangeStage::Unstaged => "unstaged",
    };
    let noun = if count == 1 { "file" } else { "files" };
    source_control_diff_status_text(format!("Opened {stage_label} changes for {count} {noun}"))
}

#[cfg(test)]
pub(crate) fn source_control_commit_patch_copy_success_status(commit: &GitCommitSummary) -> String {
    source_control_diff_status_text(format!("Copied patch for commit {}", commit.short_oid))
}

#[cfg(test)]
pub(crate) fn source_control_commit_patch_copy_empty_status(commit: &GitCommitSummary) -> String {
    source_control_diff_status_text(format!("No patch to copy for commit {}", commit.short_oid))
}

#[cfg(test)]
pub(crate) fn source_control_commit_patch_copy_failure_status(
    commit: &GitCommitSummary,
    error: &str,
) -> String {
    source_control_diff_status_text(format!(
        "Could not copy patch for commit {}: {}",
        commit.short_oid,
        source_control_diff_error_label(error)
    ))
}

#[cfg(test)]
pub(crate) fn source_control_stash_patch_copy_success_status(stash: &GitStashEntry) -> String {
    source_control_diff_status_text(format!(
        "Copied patch for {}",
        source_control_stash_patch_ref(stash)
    ))
}

#[cfg(test)]
pub(crate) fn source_control_stash_patch_copy_empty_status(stash: &GitStashEntry) -> String {
    source_control_diff_status_text(format!(
        "No patch to copy for {}",
        source_control_stash_patch_ref(stash)
    ))
}

#[cfg(test)]
pub(crate) fn source_control_stash_patch_copy_failure_status(
    stash: &GitStashEntry,
    error: &str,
) -> String {
    source_control_diff_status_text(format!(
        "Could not copy patch for {}: {}",
        source_control_stash_patch_ref(stash),
        source_control_diff_error_label(error)
    ))
}

#[cfg(test)]
fn source_control_stash_patch_ref(stash: &GitStashEntry) -> String {
    format!("stash@{{{}}}", stash.index)
}

pub(crate) fn source_control_diff_buffer_patch_copy_success_status(label: &str) -> String {
    let label = source_control_diff_display_label(label);
    source_control_diff_status_text(format!("Copied patch from {label}"))
}

pub(crate) fn source_control_diff_buffer_patch_copy_empty_status(label: &str) -> String {
    let label = source_control_diff_display_label(label);
    source_control_diff_status_text(format!("No patch to copy from {label}"))
}

pub(crate) fn source_control_diff_buffer_patch_copy_unavailable_status() -> String {
    "No diff patch to copy".to_owned()
}

pub(crate) fn source_control_diff_buffer_patch_copy_too_large_status(
    label: &str,
    buffer: &TextBuffer,
) -> String {
    let label = source_control_diff_display_label(label);
    source_control_diff_status_text(format!(
        "Diff {label} is too large to copy at once: {}",
        diff_buffer_large_file_message(buffer)
    ))
}

pub(crate) fn source_control_accessible_diff_too_large_status(
    label: &str,
    buffer: &TextBuffer,
) -> String {
    let label = source_control_diff_display_label(label);
    source_control_diff_status_text(format!(
        "Diff {label} is too large to duplicate into an accessible viewer: {}",
        diff_buffer_large_file_message(buffer)
    ))
}

pub(crate) fn source_control_diff_hunk_patch_copy_success_status(
    label: &str,
    hunk_index: usize,
) -> String {
    let label = source_control_diff_display_label(label);
    source_control_diff_status_text(format!("Copied hunk {hunk_index} patch from {label}"))
}

pub(crate) fn source_control_diff_hunk_discard_stale_status(
    path: &Path,
    hunk_index: usize,
) -> String {
    source_control_diff_hunk_identity_stale_status("discarding", path, hunk_index)
}

pub(crate) fn source_control_diff_hunk_identity_stale_status(
    action: &str,
    path: &Path,
    hunk_index: usize,
) -> String {
    source_control_diff_status_text(format!(
        "Reload diff for {} before {action} hunk {hunk_index}",
        source_control_diff_path_label(path)
    ))
}

pub(crate) fn source_control_diff_hunk_patch_copy_empty_status(
    label: &str,
    hunk_index: usize,
) -> String {
    let label = source_control_diff_display_label(label);
    source_control_diff_status_text(format!("No hunk {hunk_index} patch to copy from {label}"))
}

pub(crate) fn source_control_diff_hunk_patch_copy_no_hunk_status(
    label: &str,
    line: usize,
) -> String {
    let label = source_control_diff_display_label(label);
    source_control_diff_status_text(format!("No diff hunk at {label}:{line}"))
}

pub(crate) fn source_control_diff_hunk_patch_copy_unavailable_status() -> String {
    "No diff hunk patch to copy".to_owned()
}

pub(crate) fn source_control_diff_refresh_unavailable_status() -> String {
    "No refreshable diff to update".to_owned()
}

pub(crate) fn source_control_diff_source_open_unavailable_status() -> String {
    "No diff source file to open".to_owned()
}

pub(crate) fn source_control_diff_base_open_unavailable_status() -> String {
    "No diff base file to open".to_owned()
}

pub(crate) fn source_control_diff_base_open_missing_status(path: &Path) -> String {
    source_control_diff_status_text(format!(
        "No base file for diff {}",
        source_control_diff_path_label(path)
    ))
}

pub(crate) fn source_control_head_revision_missing_status(path: &Path) -> String {
    source_control_diff_status_text(format!(
        "No HEAD revision for {}",
        source_control_diff_path_label(path)
    ))
}

pub(crate) fn source_control_head_revision_failure_status(path: &Path, error: &str) -> String {
    source_control_diff_status_text(format!(
        "Could not open HEAD revision for {}: {}",
        source_control_diff_path_label(path),
        source_control_diff_error_label(error)
    ))
}

pub(crate) fn source_control_index_revision_missing_status(path: &Path) -> String {
    source_control_diff_status_text(format!(
        "No index revision for {}",
        source_control_diff_path_label(path)
    ))
}

pub(crate) fn source_control_index_revision_failure_status(path: &Path, error: &str) -> String {
    source_control_diff_status_text(format!(
        "Could not open index revision for {}: {}",
        source_control_diff_path_label(path),
        source_control_diff_error_label(error)
    ))
}

pub(crate) fn source_control_diff_hunk_source_open_success_status(
    label: &str,
    path: &Path,
    hunk_index: usize,
    line: usize,
) -> String {
    let label = source_control_diff_display_label(label);
    source_control_diff_status_text(format!(
        "Opened source for hunk {hunk_index} from {label} at {}:{line}",
        source_control_diff_path_label(path)
    ))
}

pub(crate) fn source_control_diff_hunk_source_open_no_hunk_status(
    label: &str,
    line: usize,
) -> String {
    let label = source_control_diff_display_label(label);
    source_control_diff_status_text(format!("No diff hunk at {label}:{line}"))
}

pub(crate) fn source_control_diff_hunk_source_open_missing_hunk_status(
    label: &str,
    hunk_index: usize,
) -> String {
    let label = source_control_diff_display_label(label);
    source_control_diff_status_text(format!(
        "Could not find hunk {hunk_index} source line in {label}"
    ))
}

pub(crate) fn source_control_diff_hunk_source_open_missing_status(path: &Path) -> String {
    source_control_diff_status_text(format!(
        "No source file for diff {}",
        source_control_diff_path_label(path)
    ))
}

pub(crate) fn source_control_diff_hunk_source_open_unavailable_status() -> String {
    "No diff hunk source to open".to_owned()
}

pub(crate) fn source_control_diff_hunk_base_open_success_status(
    label: &str,
    path: &Path,
    hunk_index: usize,
    line: usize,
) -> String {
    let label = source_control_diff_display_label(label);
    source_control_diff_status_text(format!(
        "Opened base for hunk {hunk_index} from {label} at {}:{line}",
        source_control_diff_path_label(path)
    ))
}

pub(crate) fn source_control_diff_hunk_base_open_no_hunk_status(
    label: &str,
    line: usize,
) -> String {
    let label = source_control_diff_display_label(label);
    source_control_diff_status_text(format!("No diff hunk at {label}:{line}"))
}

pub(crate) fn source_control_diff_hunk_base_open_missing_hunk_status(
    label: &str,
    hunk_index: usize,
) -> String {
    let label = source_control_diff_display_label(label);
    source_control_diff_status_text(format!(
        "Could not find hunk {hunk_index} base line in {label}"
    ))
}

pub(crate) fn source_control_diff_hunk_base_open_unavailable_status() -> String {
    "No diff hunk base to open".to_owned()
}

#[cfg(test)]
pub(crate) fn source_control_hunk_patch_copy_success_status(
    stage: GitChangeStage,
    path: &Path,
    hunk_index: usize,
) -> String {
    source_control_diff_status_text(format!(
        "Copied {} hunk {hunk_index} patch for {}",
        crate::source_control_hunk_runtime::hunk_stage_label(stage),
        source_control_diff_path_label(path)
    ))
}

#[cfg(test)]
pub(crate) fn source_control_hunk_patch_copy_empty_status(
    stage: GitChangeStage,
    path: &Path,
    hunk_index: usize,
) -> String {
    source_control_diff_status_text(format!(
        "No {} hunk {hunk_index} patch to copy for {}",
        crate::source_control_hunk_runtime::hunk_stage_label(stage),
        source_control_diff_path_label(path)
    ))
}

#[cfg(test)]
pub(crate) fn source_control_hunk_patch_copy_failure_status(
    stage: GitChangeStage,
    path: &Path,
    hunk_index: usize,
    error: &str,
) -> String {
    source_control_diff_status_text(format!(
        "Could not copy {} hunk {hunk_index} patch for {}: {}",
        crate::source_control_hunk_runtime::hunk_stage_label(stage),
        source_control_diff_path_label(path),
        source_control_diff_error_label(error)
    ))
}

pub(crate) fn source_control_hunk_diff_open_success_status(
    stage: GitChangeStage,
    label: &str,
    hunk_index: usize,
    line: usize,
) -> String {
    let label = source_control_diff_display_label(label);
    source_control_diff_status_text(format!(
        "Opened {} hunk {hunk_index} in {label}:{line}",
        crate::source_control_hunk_runtime::hunk_stage_label(stage)
    ))
}

pub(crate) fn source_control_hunk_diff_open_missing_status(
    stage: GitChangeStage,
    path: &Path,
    hunk_index: usize,
) -> String {
    source_control_diff_status_text(format!(
        "Could not find {} hunk {hunk_index} in {}",
        crate::source_control_hunk_runtime::hunk_stage_label(stage),
        source_control_diff_path_label(path)
    ))
}

fn diff_buffer_large_file_message(buffer: &TextBuffer) -> String {
    if buffer.len_bytes() > LARGE_FILE_MODE_MAX_BYTES {
        return file_too_large_message(
            u64::try_from(buffer.len_bytes()).unwrap_or(u64::MAX),
            u64::try_from(LARGE_FILE_MODE_MAX_BYTES).unwrap_or(u64::MAX),
        );
    }

    format!(
        "diff has too many lines ({}; limit {})",
        buffer.len_lines(),
        LARGE_FILE_MODE_MAX_LINES
    )
}
