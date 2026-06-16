use super::VirtualDiffOpenRequest;
use crate::path_display::{display_path_label_cow, sanitized_display_label_cow};
use kuroya_core::{GitCommitSummary, GitStashEntry};
use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

pub(crate) const VIRTUAL_DIFF_GIT_LABEL_MAX_CHARS: usize = 64;
pub(crate) const VIRTUAL_DIFF_DETAIL_MAX_CHARS: usize = 160;
pub(crate) const VIRTUAL_DIFF_OPEN_LABEL_MAX_CHARS: usize = 160;
pub(crate) const VIRTUAL_DIFF_TARGET_LABEL_MAX_CHARS: usize = 160;
pub(crate) const VIRTUAL_DIFF_STATUS_MAX_CHARS: usize = 240;

pub(crate) fn virtual_diff_open_detail(request: &VirtualDiffOpenRequest) -> String {
    match request {
        VirtualDiffOpenRequest::FileCompare { base_path, path } => virtual_diff_path_pair_label(
            base_path,
            " <-> ",
            path,
            "",
            VIRTUAL_DIFF_DETAIL_MAX_CHARS,
        ),
        VirtualDiffOpenRequest::SavedCompare { path, .. } => {
            virtual_diff_path_label(path).into_owned()
        }
        VirtualDiffOpenRequest::GitCommit { commit } => {
            format!("commit {}", virtual_diff_commit_label(commit))
        }
        VirtualDiffOpenRequest::GitStash { stash } => format!("stash@{{{}}}", stash.index),
    }
}

pub(crate) fn virtual_diff_open_pending_status(detail: &str) -> String {
    virtual_diff_status_text_owned(format!("Preparing diff for {detail}"))
}

#[derive(Debug)]
pub(crate) struct PreparedVirtualDiffPath {
    pub(crate) raw: PathBuf,
    pub(crate) label: String,
    pub(crate) diff_display: String,
}

impl PreparedVirtualDiffPath {
    pub(crate) fn new(root: &Path, raw: PathBuf) -> Self {
        let label = virtual_diff_path_label(&raw).into_owned();
        let diff_display = diff_display_path(root, &raw);
        Self {
            raw,
            label,
            diff_display,
        }
    }
}

pub(crate) fn diff_display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
        .replace('\\', "/")
}

pub(crate) fn virtual_diff_path_label(path: &Path) -> Cow<'_, str> {
    display_path_label_cow(path)
}

pub(crate) fn virtual_diff_file_compare_open_label(
    base_path: &PreparedVirtualDiffPath,
    path: &PreparedVirtualDiffPath,
) -> String {
    virtual_diff_label_pair(
        &base_path.label,
        " <-> ",
        &path.label,
        " (Compare)",
        VIRTUAL_DIFF_OPEN_LABEL_MAX_CHARS,
    )
}

pub(crate) fn virtual_diff_file_compare_target(
    base_path: &PreparedVirtualDiffPath,
    path: &PreparedVirtualDiffPath,
) -> String {
    virtual_diff_label_pair(
        &base_path.label,
        " and ",
        &path.label,
        "",
        VIRTUAL_DIFF_TARGET_LABEL_MAX_CHARS,
    )
}

pub(crate) fn virtual_diff_path_pair_label(
    base_path: &Path,
    separator: &str,
    path: &Path,
    suffix: &str,
    max_chars: usize,
) -> String {
    let left = virtual_diff_path_label(base_path);
    let right = virtual_diff_path_label(path);
    virtual_diff_label_pair(left.as_ref(), separator, right.as_ref(), suffix, max_chars)
}

pub(crate) fn virtual_diff_label_pair(
    left: &str,
    separator: &str,
    right: &str,
    suffix: &str,
    max_chars: usize,
) -> String {
    let fixed_chars = separator
        .chars()
        .count()
        .saturating_add(suffix.chars().count());
    let component_max = max_chars.saturating_sub(fixed_chars).max(2) / 2;
    let component_max = component_max.max(1);
    let left = virtual_diff_display_text_cow(left, component_max, ".");
    let right = virtual_diff_display_text_cow(right, component_max, ".");
    virtual_diff_display_text_owned(
        format!("{}{separator}{}{}", left.as_ref(), right.as_ref(), suffix),
        max_chars,
        ".",
    )
}

pub(crate) fn virtual_diff_open_label(value: impl AsRef<str>) -> String {
    virtual_diff_open_label_cow(value.as_ref()).into_owned()
}

pub(crate) fn virtual_diff_open_label_owned(value: String) -> String {
    virtual_diff_display_text_owned(value, VIRTUAL_DIFF_OPEN_LABEL_MAX_CHARS, "Virtual Diff")
}

pub(crate) fn virtual_diff_open_label_cow(value: &str) -> Cow<'_, str> {
    virtual_diff_display_text_cow(value, VIRTUAL_DIFF_OPEN_LABEL_MAX_CHARS, "Virtual Diff")
}

pub(crate) fn virtual_diff_target_label(value: impl AsRef<str>) -> String {
    virtual_diff_target_label_cow(value.as_ref()).into_owned()
}

pub(crate) fn virtual_diff_target_label_owned(value: String) -> String {
    virtual_diff_display_text_owned(value, VIRTUAL_DIFF_TARGET_LABEL_MAX_CHARS, "diff target")
}

pub(crate) fn virtual_diff_target_label_cow(value: &str) -> Cow<'_, str> {
    virtual_diff_display_text_cow(value, VIRTUAL_DIFF_TARGET_LABEL_MAX_CHARS, "diff target")
}

pub(crate) fn virtual_diff_status_text(value: impl AsRef<str>) -> String {
    virtual_diff_status_text_cow(value.as_ref()).into_owned()
}

pub(crate) fn virtual_diff_status_text_owned(value: String) -> String {
    virtual_diff_display_text_owned(
        value,
        VIRTUAL_DIFF_STATUS_MAX_CHARS,
        "Virtual diff status unavailable",
    )
}

pub(crate) fn virtual_diff_status_text_cow(value: &str) -> Cow<'_, str> {
    virtual_diff_display_text_cow(
        value,
        VIRTUAL_DIFF_STATUS_MAX_CHARS,
        "Virtual diff status unavailable",
    )
}

#[cfg(test)]
pub(crate) fn virtual_diff_display_text(
    value: impl AsRef<str>,
    max_chars: usize,
    fallback: &str,
) -> String {
    virtual_diff_display_text_cow(value.as_ref(), max_chars, fallback).into_owned()
}

pub(crate) fn virtual_diff_display_text_owned(
    value: String,
    max_chars: usize,
    fallback: &str,
) -> String {
    let sanitized = match virtual_diff_display_text_cow(&value, max_chars, fallback) {
        Cow::Borrowed(label) => {
            if label.len() == value.len() && label.as_ptr() == value.as_ptr() {
                None
            } else {
                Some(label.to_owned())
            }
        }
        Cow::Owned(label) => Some(label),
    };
    sanitized.unwrap_or(value)
}

pub(crate) fn virtual_diff_display_text_cow<'a>(
    value: &'a str,
    max_chars: usize,
    fallback: &str,
) -> Cow<'a, str> {
    sanitized_display_label_cow(value, max_chars, fallback)
}

pub(crate) fn virtual_diff_commit_label(commit: &GitCommitSummary) -> String {
    virtual_diff_git_label(&commit.short_oid, "commit")
}

pub(crate) fn virtual_diff_stash_label(stash_ref: &str, stash: &GitStashEntry) -> String {
    virtual_diff_open_label(format!(
        "{} {} (Stash Changes)",
        stash_ref,
        virtual_diff_git_label(&stash.short_oid, "stash")
    ))
}

pub(crate) fn virtual_diff_git_label(value: &str, fallback: &str) -> String {
    virtual_diff_git_label_cow(value, fallback).into_owned()
}

pub(crate) fn virtual_diff_git_label_cow<'a>(value: &'a str, fallback: &str) -> Cow<'a, str> {
    virtual_diff_display_text_cow(value, VIRTUAL_DIFF_GIT_LABEL_MAX_CHARS, fallback)
}
