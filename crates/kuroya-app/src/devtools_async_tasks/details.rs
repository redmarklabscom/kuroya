use super::{AsyncTaskEventLabel, AsyncTaskOutcome, MAX_ASYNC_TASK_DETAIL_CHARS};
use crate::{
    explorer::ExplorerOperationResult,
    path_display::{display_path_label_cow, sanitized_display_label_cow},
};
use std::{
    borrow::Cow,
    fmt::Write as _,
    path::{Path, PathBuf},
};

const BRANCH_RENAME_DETAIL_SEPARATOR: &str = " -> ";
const PLUGIN_COMMAND_DETAIL_MAX_CHARS: usize = 96;

pub(super) fn finished(name: &'static str, detail: String) -> AsyncTaskEventLabel {
    AsyncTaskEventLabel {
        name,
        detail,
        outcome: AsyncTaskOutcome::Finished,
    }
}

pub(super) fn failed(name: &'static str, detail: String) -> AsyncTaskEventLabel {
    AsyncTaskEventLabel {
        name,
        detail,
        outcome: AsyncTaskOutcome::Failed,
    }
}

pub(crate) fn path_detail(path: &Path) -> String {
    path_detail_cow(path).into_owned()
}

fn path_detail_cow(path: &Path) -> Cow<'_, str> {
    display_path_label_cow(path)
}

pub(crate) fn file_reload_task_detail(request_id: u64, path: &Path) -> String {
    request_path_detail(request_id, path)
}

pub(crate) fn git_scan_task_detail(request_id: u64, root: &Path) -> String {
    request_path_detail(request_id, root)
}

fn request_path_detail(request_id: u64, path: &Path) -> String {
    let path = path_detail_cow(path);
    let mut detail = String::with_capacity(2 + decimal_digit_count_u64(request_id) + path.len());
    let _ = write!(detail, "#{request_id} ");
    detail.push_str(path.as_ref());
    detail
}

pub(super) fn async_task_detail(value: &str) -> String {
    async_task_detail_cow(value).into_owned()
}

pub(super) fn async_task_detail_cow(value: &str) -> Cow<'_, str> {
    async_task_detail_with_max_cow(value, MAX_ASYNC_TASK_DETAIL_CHARS, "task detail")
}

pub(super) fn async_task_detail_with_max(value: &str, max_chars: usize, fallback: &str) -> String {
    async_task_detail_with_max_cow(value, max_chars, fallback).into_owned()
}

pub(super) fn async_task_detail_with_max_cow<'a>(
    value: &'a str,
    max_chars: usize,
    fallback: &str,
) -> Cow<'a, str> {
    sanitized_display_label_cow(value, max_chars, fallback)
}

pub(super) fn query_detail(query: &str) -> String {
    let query = async_task_detail_with_max_cow(
        query,
        MAX_ASYNC_TASK_DETAIL_CHARS.saturating_sub(2),
        "search text",
    );
    let mut detail = String::with_capacity(query.len() + 2);
    detail.push('`');
    detail.push_str(&query);
    detail.push('`');
    detail
}

pub(crate) fn plugin_command_task_detail(command_id: &str) -> String {
    async_task_detail_with_max(command_id, PLUGIN_COMMAND_DETAIL_MAX_CHARS, "command")
}

pub(crate) fn paths_detail(paths: &[PathBuf]) -> String {
    match paths {
        [] => "0 paths".to_owned(),
        [path] => path_detail_cow(path).into_owned(),
        _ => {
            let mut detail =
                String::with_capacity(decimal_digit_count_usize(paths.len()) + " paths".len());
            let _ = write!(detail, "{} paths", paths.len());
            detail
        }
    }
}

pub(super) fn git_commit_task_detail(smart_commit: bool) -> String {
    if smart_commit {
        "smart commit".to_owned()
    } else {
        "staged changes".to_owned()
    }
}

pub(super) fn explorer_operation_detail(operation: &ExplorerOperationResult) -> String {
    match operation {
        ExplorerOperationResult::Created { path, .. } => operation_path_detail("created ", path),
        ExplorerOperationResult::Renamed { new_path, .. } => {
            operation_path_detail("renamed ", new_path)
        }
        ExplorerOperationResult::Deleted { path, .. } => operation_path_detail("deleted ", path),
    }
}

fn operation_path_detail(prefix: &str, path: &Path) -> String {
    let path = path_detail_cow(path);
    let mut detail = String::with_capacity(prefix.len() + path.len());
    detail.push_str(prefix);
    detail.push_str(path.as_ref());
    detail
}

pub(super) fn explorer_action_detail(action: &str) -> String {
    async_task_detail_with_max(action, MAX_ASYNC_TASK_DETAIL_CHARS, "complete operation")
}

pub(crate) fn hunk_detail(path: &Path, hunk_index: usize) -> String {
    let path = path_detail_cow(path);
    let hunk_number = hunk_index + 1;
    let mut detail = String::with_capacity(path.len() + 2 + decimal_digit_count_usize(hunk_number));
    detail.push_str(path.as_ref());
    let _ = write!(detail, " #{hunk_number}");
    detail
}

pub(crate) fn index_detail(index: usize) -> String {
    let mut detail = String::with_capacity(1 + decimal_digit_count_usize(index));
    let _ = write!(detail, "#{index}");
    detail
}

pub(super) fn branch_detail(branch: &str) -> String {
    async_task_detail_with_max(branch, MAX_ASYNC_TASK_DETAIL_CHARS, "unnamed branch")
}

pub(crate) fn branch_operation_detail(request_id: u64, branch: &str) -> String {
    let prefix_chars = 2 + decimal_digit_count_u64(request_id);
    let branch_max_chars = MAX_ASYNC_TASK_DETAIL_CHARS.saturating_sub(prefix_chars);
    let branch = async_task_detail_with_max_cow(branch, branch_max_chars, "unnamed branch");
    let mut detail = String::with_capacity(prefix_chars + branch.len());
    let _ = write!(detail, "#{request_id} ");
    detail.push_str(&branch);
    detail
}

pub(crate) fn branch_rename_detail(old_branch: &str, new_branch: &str) -> String {
    let branch_max_chars =
        MAX_ASYNC_TASK_DETAIL_CHARS.saturating_sub(BRANCH_RENAME_DETAIL_SEPARATOR.len()) / 2;
    let old_branch = async_task_detail_with_max_cow(old_branch, branch_max_chars, "unnamed branch");
    let new_branch = async_task_detail_with_max_cow(new_branch, branch_max_chars, "unnamed branch");
    let mut detail = String::with_capacity(
        old_branch.len() + BRANCH_RENAME_DETAIL_SEPARATOR.len() + new_branch.len(),
    );
    detail.push_str(&old_branch);
    detail.push_str(BRANCH_RENAME_DETAIL_SEPARATOR);
    detail.push_str(&new_branch);
    detail
}

fn decimal_digit_count_u64(mut value: u64) -> usize {
    let mut digits = 1;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

fn decimal_digit_count_usize(mut value: usize) -> usize {
    let mut digits = 1;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

pub(super) fn bounded_async_task_text(value: &str, max_chars: usize) -> String {
    bounded_async_task_text_cow(value, max_chars).into_owned()
}

pub(super) fn bounded_async_task_text_cow(value: &str, max_chars: usize) -> Cow<'_, str> {
    sanitized_display_label_cow(value, max_chars, "")
}
