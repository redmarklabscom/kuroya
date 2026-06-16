use crate::path_display::{compact_path, sanitized_display_label_cow};
use kuroya_core::{WorkspaceTask, WorkspaceTaskKind, workspace_task_command_preview};
use std::{borrow::Cow, path::Path};

pub(crate) const WORKSPACE_TASK_STATUS_NAME_MAX_CHARS: usize = 80;
pub(crate) const WORKSPACE_TASK_STATUS_PATH_MAX_CHARS: usize = 96;
pub(crate) const WORKSPACE_TASK_STATUS_COMMAND_MAX_CHARS: usize = 120;
pub(crate) const WORKSPACE_TASK_STATUS_ERROR_MAX_CHARS: usize = 180;

pub(crate) fn workspace_tasks_loaded_status(count: usize) -> String {
    match count {
        0 => "No workspace tasks configured".to_owned(),
        1 => "Loaded 1 workspace task".to_owned(),
        _ => format!("Loaded {count} workspace tasks"),
    }
}

pub(crate) fn workspace_tasks_restricted_status() -> &'static str {
    "Trust this workspace before loading tasks"
}

pub(crate) fn workspace_tasks_loading_status() -> &'static str {
    "Loading workspace tasks"
}

pub(crate) fn workspace_task_loading_status(kind: WorkspaceTaskKind) -> String {
    format!("Loading {}", workspace_task_kind_run_target(kind))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkspaceTaskStartedStatusLabels {
    name: String,
    command: String,
    cwd: Option<String>,
}

impl WorkspaceTaskStartedStatusLabels {
    fn new(task: &WorkspaceTask, cwd: Option<&Path>) -> Self {
        Self {
            name: workspace_task_name_label(&task.name),
            command: workspace_task_command_label(task),
            cwd: cwd.map(workspace_task_path_label),
        }
    }
}

#[cfg(test)]
pub(crate) fn workspace_task_started_status(task: &WorkspaceTask) -> String {
    workspace_task_started_status_with_cwd(task, task.cwd.as_deref())
}

pub(super) fn workspace_task_started_status_with_cwd(
    task: &WorkspaceTask,
    cwd: Option<&Path>,
) -> String {
    let labels = WorkspaceTaskStartedStatusLabels::new(task, cwd);
    let mut status = String::with_capacity(
        "Started task ``: ".len()
            + labels.name.len()
            + labels.command.len()
            + labels
                .cwd
                .as_ref()
                .map_or(0, |cwd| " in ".len() + cwd.len()),
    );
    status.push_str("Started task `");
    status.push_str(&labels.name);
    status.push('`');
    if let Some(cwd) = labels.cwd {
        status.push_str(" in ");
        status.push_str(&cwd);
    }
    status.push_str(": ");
    status.push_str(&labels.command);
    status
}

pub(crate) fn workspace_task_canceled_status(task: &WorkspaceTask) -> String {
    format!("Canceled task `{}`", workspace_task_name_label(&task.name))
}

pub(crate) fn workspace_task_invalid_cwd_status(task: &WorkspaceTask, cwd: &Path) -> String {
    let cwd = workspace_task_path_label(cwd);
    let mut status = workspace_task_status_with_name(task, " cwd is outside the workspace: ");
    status.push_str(&cwd);
    status
}

pub(crate) fn workspace_task_completed_status(
    task: &WorkspaceTask,
    completion: super::WorkspaceTaskCompletion,
) -> String {
    match completion {
        super::WorkspaceTaskCompletion::FailedExitCode(code) => {
            let mut status = workspace_task_status_with_name(task, " failed with exit code ");
            status.push_str(&code.to_string());
            status
        }
        super::WorkspaceTaskCompletion::TerminalError => {
            workspace_task_status_with_name(task, " failed in terminal")
        }
        super::WorkspaceTaskCompletion::Finished => {
            workspace_task_status_with_name(task, " finished")
        }
    }
}

pub(crate) fn workspace_task_not_running_status(task: &WorkspaceTask) -> String {
    workspace_task_status_with_name(task, " is not running")
}

fn workspace_task_status_with_name(task: &WorkspaceTask, suffix: &str) -> String {
    let mut status = workspace_task_status_prefix(task);
    status.reserve(suffix.len());
    status.push_str(suffix);
    status
}

fn workspace_task_status_prefix(task: &WorkspaceTask) -> String {
    let name = workspace_task_name_label_text(&task.name);
    let name = name.as_ref();
    let mut status = String::with_capacity("Task ``".len() + name.len());
    status.push_str("Task `");
    status.push_str(name);
    status.push('`');
    status
}

pub(crate) fn workspace_task_missing_kind_status(kind: WorkspaceTaskKind) -> String {
    format!("No {} configured", workspace_task_kind_run_target(kind))
}

fn workspace_task_kind_run_target(kind: WorkspaceTaskKind) -> &'static str {
    match kind {
        WorkspaceTaskKind::Build => "build workspace task",
        WorkspaceTaskKind::Test => "test workspace task",
        WorkspaceTaskKind::Run => "run configuration",
        WorkspaceTaskKind::Custom => "workspace task",
    }
}

pub(crate) fn workspace_task_name_label(name: &str) -> String {
    workspace_task_name_label_text(name).into_owned()
}

pub(super) fn workspace_task_name_label_text(name: &str) -> Cow<'_, str> {
    workspace_task_display_text_or_cow(name, WORKSPACE_TASK_STATUS_NAME_MAX_CHARS, "workspace task")
}

pub(crate) fn workspace_task_command_label(task: &WorkspaceTask) -> String {
    let command = workspace_task_display_text_or_cow(
        &task.command,
        WORKSPACE_TASK_STATUS_COMMAND_MAX_CHARS,
        "",
    );
    if command.is_empty() {
        return "command".to_owned();
    }

    if task.args.is_empty()
        && workspace_task_command_preview_matches_sanitized_command(command.as_ref())
    {
        return command.into_owned();
    }

    workspace_task_display_text_or(
        &workspace_task_command_preview(task),
        WORKSPACE_TASK_STATUS_COMMAND_MAX_CHARS,
        "command",
    )
}

fn workspace_task_command_preview_matches_sanitized_command(command: &str) -> bool {
    command
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/' | '\\' | ':'))
}

pub(crate) fn workspace_task_path_label(path: &Path) -> String {
    workspace_task_display_text(&compact_path(path), WORKSPACE_TASK_STATUS_PATH_MAX_CHARS)
}

pub(super) fn workspace_task_error_detail(error: &str) -> String {
    workspace_task_display_text_or(
        error,
        WORKSPACE_TASK_STATUS_ERROR_MAX_CHARS,
        "unknown error",
    )
}

pub(super) fn workspace_task_load_failed_status(error: &str) -> String {
    format!(
        "Could not load workspace tasks: {}",
        workspace_task_error_detail(error)
    )
}

pub(crate) fn workspace_task_display_text(text: &str, max_chars: usize) -> String {
    workspace_task_display_text_or(text, max_chars, ".")
}

fn workspace_task_display_text_or(text: &str, max_chars: usize, fallback: &str) -> String {
    workspace_task_display_text_or_cow(text, max_chars, fallback).into_owned()
}

pub(super) fn workspace_task_display_text_or_cow<'a>(
    text: &'a str,
    max_chars: usize,
    fallback: &str,
) -> Cow<'a, str> {
    sanitized_display_label_cow(text, max_chars, fallback)
}
