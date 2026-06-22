use crate::{
    KuroyaApp,
    path_display::display_path_label_cow,
    terminal::TerminalProcessSessionState,
    ui_events::UiEvent,
    workspace_state::background_workspace_event_matches,
    workspace_trust::{
        workspace_path_contains_lexically, workspace_path_stays_within_root_lexically,
    },
};
use kuroya_core::{
    WorkspaceTask, WorkspaceTaskKind, load_workspace_tasks, workspace_task_default_index,
};
use std::{
    collections::{HashMap, HashSet, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    path::{Component, Path, PathBuf},
};

mod requests;
mod status;

use requests::{
    begin_workspace_task_load_request_state, finish_workspace_task_load_request_state,
    invalidate_workspace_task_load_request_state,
};
#[cfg(test)]
use status::{
    WORKSPACE_TASK_STATUS_COMMAND_MAX_CHARS, WORKSPACE_TASK_STATUS_ERROR_MAX_CHARS,
    WORKSPACE_TASK_STATUS_NAME_MAX_CHARS, WORKSPACE_TASK_STATUS_PATH_MAX_CHARS,
    workspace_task_display_text_or_cow, workspace_task_error_detail,
    workspace_task_name_label_text,
};
pub(crate) use status::{
    workspace_task_canceled_status, workspace_task_command_label, workspace_task_completed_status,
    workspace_task_invalid_cwd_status, workspace_task_loading_status,
    workspace_task_missing_kind_status, workspace_task_name_label,
    workspace_task_not_running_status, workspace_tasks_loaded_status,
    workspace_tasks_loading_status, workspace_tasks_restricted_status,
};
#[cfg(test)]
pub(crate) use status::{
    workspace_task_display_text, workspace_task_path_label, workspace_task_started_status,
};
use status::{workspace_task_load_failed_status, workspace_task_started_status_with_cwd};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RunningWorkspaceTask {
    pub(crate) task_index: usize,
    pub(crate) fingerprint: u64,
    pub(crate) session_id: usize,
}

impl KuroyaApp {
    pub(crate) fn begin_workspace_tasks(&mut self) {
        self.workspace_tasks_open = true;
        self.workspace_tasks_selected = 0;
        if self.workspace_placeholder {
            self.clear_workspace_tasks_for_restricted_workspace();
            self.status = "No folder open".to_owned();
            return;
        }
        if self.workspace_trusted {
            self.spawn_workspace_task_load();
            self.status = "Workspace tasks".to_owned();
        } else {
            self.clear_workspace_tasks_for_restricted_workspace();
            self.status = workspace_tasks_restricted_status().to_owned();
        }
    }

    pub(crate) fn spawn_workspace_task_load(&mut self) -> bool {
        if self.workspace_placeholder {
            self.clear_workspace_tasks_for_restricted_workspace();
            self.status = "No folder open".to_owned();
            return false;
        }
        if !self.workspace_trusted {
            self.clear_workspace_tasks_for_restricted_workspace();
            if self.workspace_tasks_open {
                self.status = workspace_tasks_restricted_status().to_owned();
            }
            return false;
        }

        let Some(request_id) = self.begin_workspace_task_load_request() else {
            self.workspace_tasks_loading = true;
            return false;
        };
        self.workspace_tasks_loading = true;
        let root = self.workspace.root.clone();
        let tx = self.tx.clone();
        self.record_async_task_started("Workspace Tasks", display_path_label_cow(&root));
        self.runtime.spawn_blocking(move || {
            let event = match load_workspace_tasks(&root) {
                Ok(tasks) => UiEvent::WorkspaceTasksLoaded {
                    request_id,
                    root,
                    tasks,
                },
                Err(error) => UiEvent::WorkspaceTasksFailed {
                    request_id,
                    root,
                    error: error.to_string(),
                },
            };
            let _ = crate::ui_event_channel::send_critical_ui_event(&tx, event);
        });
        true
    }

    fn begin_workspace_task_load_request(&mut self) -> Option<u64> {
        begin_workspace_task_load_request_state(
            &mut self.workspace_tasks_next_request_id,
            &mut self.workspace_tasks_active_request_id,
            &mut self.workspace_tasks_in_flight_request_id,
            &mut self.workspace_tasks_reload_queued,
        )
    }

    pub(crate) fn finish_workspace_task_load_request(&mut self, request_id: u64) -> bool {
        finish_workspace_task_load_request_state(
            &mut self.workspace_tasks_in_flight_request_id,
            &mut self.workspace_tasks_reload_queued,
            request_id,
        )
    }

    pub(crate) fn invalidate_workspace_task_load_requests(&mut self) {
        invalidate_workspace_task_load_request_state(
            &mut self.workspace_tasks_next_request_id,
            &mut self.workspace_tasks_active_request_id,
            &mut self.workspace_tasks_in_flight_request_id,
            &mut self.workspace_tasks_reload_queued,
        );
    }

    pub(crate) fn clear_workspace_tasks_for_restricted_workspace(&mut self) {
        let terminal = &mut self.terminal;
        close_running_workspace_task_sessions(&mut self.running_workspace_tasks, |session_id| {
            terminal.close_session_by_id(session_id)
        });
        self.invalidate_workspace_task_load_requests();
        self.workspace_tasks.clear();
        self.workspace_tasks_selected = 0;
        self.workspace_tasks_loading = false;
        self.workspace_tasks_loaded = false;
        self.pending_workspace_task_kind = None;
    }

    pub(crate) fn apply_workspace_tasks_loaded(
        &mut self,
        request_id: u64,
        root: PathBuf,
        tasks: Vec<WorkspaceTask>,
    ) {
        if !background_workspace_event_matches(
            &self.workspace.root,
            &root,
            request_id,
            self.workspace_tasks_active_request_id,
        ) {
            return;
        }

        let count = tasks.len();
        self.workspace_tasks = tasks;
        prune_stale_workspace_task_records(
            &mut self.running_workspace_tasks,
            &self.workspace_tasks,
        );
        self.workspace_tasks_loading = false;
        self.workspace_tasks_loaded = true;
        self.workspace_tasks_selected = self
            .workspace_tasks_selected
            .min(self.workspace_tasks.len().saturating_sub(1));
        if let Some(kind) = self.pending_workspace_task_kind.take() {
            match workspace_task_kind_run_plan(
                &self.workspace_tasks,
                kind,
                self.workspace_tasks_loaded,
                self.workspace_tasks_loading,
            ) {
                WorkspaceTaskKindRunPlan::Run(index) => self.run_workspace_task(index),
                WorkspaceTaskKindRunPlan::Load | WorkspaceTaskKindRunPlan::Missing => {
                    self.status = workspace_task_missing_kind_status(kind);
                }
            }
            return;
        }
        if self.workspace_tasks_open {
            self.status = workspace_tasks_loaded_status(count);
        }
    }

    pub(crate) fn apply_workspace_tasks_failed(
        &mut self,
        request_id: u64,
        root: PathBuf,
        error: String,
    ) {
        if !background_workspace_event_matches(
            &self.workspace.root,
            &root,
            request_id,
            self.workspace_tasks_active_request_id,
        ) {
            return;
        }

        self.workspace_tasks.clear();
        self.workspace_tasks_selected = 0;
        self.workspace_tasks_loading = false;
        self.workspace_tasks_loaded = false;
        self.pending_workspace_task_kind = None;
        self.status = workspace_task_load_failed_status(&error);
    }

    pub(crate) fn run_workspace_task(&mut self, index: usize) {
        if !self.workspace_trusted {
            self.status = "Trust this workspace before running tasks".to_owned();
            return;
        }
        if self.workspace_tasks_loading {
            self.status = workspace_tasks_loading_status().to_owned();
            return;
        }
        let Some(task) = self.workspace_tasks.get(index) else {
            self.status = "No workspace task selected".to_owned();
            return;
        };

        let fingerprint = workspace_task_fingerprint(task);
        let cwd = match workspace_task_launch_cwd(&self.workspace.root, task) {
            Ok(cwd) => cwd,
            Err(cwd) => {
                self.status = workspace_task_invalid_cwd_status(task, &cwd);
                return;
            }
        };
        let status = workspace_task_started_status_with_cwd(task, Some(cwd.as_path()));
        let launch_task = workspace_task_launch_task(task, cwd);

        if !self.terminal.can_open_session() {
            self.status = "Terminal session limit reached".to_owned();
            return;
        }
        self.prepare_terminal_open_height();
        let session_id = self
            .terminal
            .open_workspace_task(&launch_task)
            .expect("terminal session capacity was checked before opening workspace task");
        self.running_workspace_tasks.push(RunningWorkspaceTask {
            task_index: index,
            fingerprint,
            session_id,
        });
        self.status = status;
    }

    pub(crate) fn run_workspace_task_snapshot(&mut self, index: usize, fingerprint: u64) {
        if !self.workspace_trusted {
            self.status = "Trust this workspace before running tasks".to_owned();
            return;
        }
        if self.workspace_tasks_loading {
            self.status = workspace_tasks_loading_status().to_owned();
            return;
        }
        let Some(task) = self.workspace_tasks.get(index) else {
            self.status = "Workspace task changed; run it again".to_owned();
            return;
        };
        if workspace_task_fingerprint(task) != fingerprint {
            self.status = "Workspace task changed; run it again".to_owned();
            return;
        }

        self.run_workspace_task(index);
    }

    pub(crate) fn cancel_workspace_task_snapshot(&mut self, index: usize, fingerprint: u64) {
        if !self.workspace_trusted {
            self.status = "Trust this workspace before canceling tasks".to_owned();
            return;
        }
        if self.workspace_tasks_loading {
            self.status = workspace_tasks_loading_status().to_owned();
            return;
        }
        let Some(task) = self.workspace_tasks.get(index) else {
            self.status = "Workspace task changed; cancel it again".to_owned();
            return;
        };
        if workspace_task_fingerprint(task) != fingerprint {
            self.status = "Workspace task changed; cancel it again".to_owned();
            return;
        }

        self.prune_finished_workspace_tasks();
        let Some(running_index) =
            running_workspace_task_position(&self.running_workspace_tasks, index, fingerprint)
        else {
            let Some(task) = self.workspace_tasks.get(index) else {
                self.status = "Workspace task changed; cancel it again".to_owned();
                return;
            };
            self.status = workspace_task_not_running_status(task);
            return;
        };
        let running = self.running_workspace_tasks.remove(running_index);
        let closed = self.terminal.close_session_by_id(running.session_id);
        let Some(task) = self.workspace_tasks.get(index) else {
            self.status = "Workspace task changed; cancel it again".to_owned();
            return;
        };
        if closed {
            self.status = workspace_task_canceled_status(task);
        } else {
            self.status = workspace_task_not_running_status(task);
        }
    }

    pub(crate) fn run_workspace_task_kind(&mut self, kind: WorkspaceTaskKind) {
        if !self.workspace_trusted {
            self.status = "Trust this workspace before running tasks".to_owned();
            return;
        }
        match workspace_task_kind_run_plan(
            &self.workspace_tasks,
            kind,
            self.workspace_tasks_loaded,
            self.workspace_tasks_loading,
        ) {
            WorkspaceTaskKindRunPlan::Run(index) => self.run_workspace_task(index),
            WorkspaceTaskKindRunPlan::Load => {
                self.pending_workspace_task_kind = Some(kind);
                if !self.workspace_tasks_loading {
                    self.spawn_workspace_task_load();
                }
                self.status = workspace_task_loading_status(kind);
            }
            WorkspaceTaskKindRunPlan::Missing => {
                self.status = workspace_task_missing_kind_status(kind);
            }
        }
    }

    pub(crate) fn prune_finished_workspace_tasks(&mut self) {
        let terminal = &self.terminal;
        if let Some(status) = prune_finished_workspace_task_records(
            &mut self.running_workspace_tasks,
            &self.workspace_tasks,
            |session_id| terminal.process_session_state_by_id(session_id),
        ) {
            self.status = status;
        }
    }
}

pub(crate) fn workspace_task_fingerprint(task: &WorkspaceTask) -> u64 {
    let mut hasher = DefaultHasher::new();
    task.hash(&mut hasher);
    hasher.finish()
}

fn workspace_task_launch_task(task: &WorkspaceTask, cwd: PathBuf) -> WorkspaceTask {
    let mut launch_task = task.clone();
    launch_task.cwd = Some(cwd);
    launch_task
}

pub(crate) fn workspace_task_launch_cwd(
    workspace_root: &Path,
    task: &WorkspaceTask,
) -> Result<PathBuf, PathBuf> {
    let cwd = workspace_task_resolved_cwd(workspace_root, task);
    if workspace_path_stays_within_root_lexically(workspace_root, &cwd) {
        Ok(cwd)
    } else {
        Err(cwd)
    }
}

fn workspace_task_resolved_cwd(workspace_root: &Path, task: &WorkspaceTask) -> PathBuf {
    match task.cwd.as_deref() {
        None => workspace_root.to_path_buf(),
        Some(cwd) if workspace_path_contains_lexically(workspace_root, cwd) => cwd.to_path_buf(),
        Some(cwd) if workspace_task_cwd_is_workspace_relative(cwd) => workspace_root.join(cwd),
        Some(cwd) => cwd.to_path_buf(),
    }
}

fn workspace_task_cwd_is_workspace_relative(cwd: &Path) -> bool {
    !cwd.has_root()
        && !cwd
            .components()
            .any(|component| matches!(component, Component::Prefix(_)))
}

pub(crate) fn workspace_task_snapshot_is_running(
    task_index: usize,
    fingerprint: u64,
    running_tasks: &[RunningWorkspaceTask],
) -> bool {
    running_workspace_task_position(running_tasks, task_index, fingerprint).is_some()
}

pub(crate) fn running_workspace_task_position(
    running_tasks: &[RunningWorkspaceTask],
    task_index: usize,
    fingerprint: u64,
) -> Option<usize> {
    running_tasks
        .iter()
        .rposition(|running| running.task_index == task_index && running.fingerprint == fingerprint)
}

pub(crate) fn prune_stale_workspace_task_records(
    running_tasks: &mut Vec<RunningWorkspaceTask>,
    tasks: &[WorkspaceTask],
) {
    if running_tasks.is_empty() {
        return;
    }

    let task_fingerprints = workspace_task_fingerprints(tasks);
    let task_indexes_by_fingerprint = workspace_task_indexes_by_fingerprint(&task_fingerprints);
    let current_session_ids = current_workspace_task_session_ids(running_tasks, &task_fingerprints);
    let original_len = running_tasks.len();
    let mut retained_session_ids = HashSet::with_capacity(original_len);
    let mut retained_len = 0usize;
    let mut index = 0usize;

    while index < original_len {
        let mut running = running_tasks[index];
        if running_workspace_task_matches_current_task(&running, &task_fingerprints) {
            if retained_session_ids.insert(running.session_id) {
                running_tasks[retained_len] = running;
                retained_len += 1;
            }
            index += 1;
            continue;
        }

        if current_session_ids.contains(&running.session_id) {
            index += 1;
            continue;
        }

        if let Some(task_index) = current_workspace_task_index_for_fingerprint(
            &task_indexes_by_fingerprint,
            running.fingerprint,
        ) {
            if retained_session_ids.insert(running.session_id) {
                running.task_index = task_index;
                running_tasks[retained_len] = running;
                retained_len += 1;
            }
        }
        index += 1;
    }

    running_tasks.truncate(retained_len);
}

pub(crate) fn prune_finished_workspace_task_records(
    running_tasks: &mut Vec<RunningWorkspaceTask>,
    tasks: &[WorkspaceTask],
    mut process_session_state_by_id: impl FnMut(usize) -> Option<TerminalProcessSessionState>,
) -> Option<String> {
    if running_tasks.is_empty() {
        return None;
    }

    let task_fingerprints = workspace_task_fingerprints(tasks);
    let mut retained_session_ids = HashSet::with_capacity(running_tasks.len());
    let mut completion_status: Option<(WorkspaceTaskCompletionPriority, String)> = None;

    running_tasks.retain(|running| {
        let Some(task) = running_workspace_task_current_task(running, tasks, &task_fingerprints)
        else {
            return false;
        };
        if !retained_session_ids.insert(running.session_id) {
            return false;
        };

        let Some(state) = process_session_state_by_id(running.session_id) else {
            return false;
        };
        let completion = match state {
            TerminalProcessSessionState::Running => return true,
            TerminalProcessSessionState::Exited(0) | TerminalProcessSessionState::Stopped => {
                WorkspaceTaskCompletion::Finished
            }
            TerminalProcessSessionState::Exited(exit_code) => {
                WorkspaceTaskCompletion::FailedExitCode(exit_code)
            }
            TerminalProcessSessionState::TerminalError => WorkspaceTaskCompletion::TerminalError,
        };

        let priority = workspace_task_completion_priority(completion);
        if completion_status
            .as_ref()
            .is_none_or(|(current, _)| priority >= *current)
        {
            completion_status = Some((priority, workspace_task_completed_status(task, completion)));
        }

        false
    });

    completion_status.map(|(_, status)| status)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum WorkspaceTaskCompletionPriority {
    Finished,
    Failed,
    TerminalError,
}

fn workspace_task_completion_priority(
    completion: WorkspaceTaskCompletion,
) -> WorkspaceTaskCompletionPriority {
    match completion {
        WorkspaceTaskCompletion::Finished => WorkspaceTaskCompletionPriority::Finished,
        WorkspaceTaskCompletion::FailedExitCode(_) => WorkspaceTaskCompletionPriority::Failed,
        WorkspaceTaskCompletion::TerminalError => WorkspaceTaskCompletionPriority::TerminalError,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkspaceTaskCompletion {
    Finished,
    FailedExitCode(i32),
    TerminalError,
}

pub(crate) fn close_running_workspace_task_sessions(
    running_tasks: &mut Vec<RunningWorkspaceTask>,
    mut close_session_by_id: impl FnMut(usize) -> bool,
) -> usize {
    let mut closed = 0usize;
    let mut closed_session_ids = HashSet::with_capacity(running_tasks.len());
    for running in running_tasks.drain(..) {
        if closed_session_ids.insert(running.session_id) && close_session_by_id(running.session_id)
        {
            closed = closed.saturating_add(1);
        }
    }
    closed
}

fn running_workspace_task_matches_current_task(
    running: &RunningWorkspaceTask,
    task_fingerprints: &[u64],
) -> bool {
    task_fingerprints
        .get(running.task_index)
        .is_some_and(|fingerprint| *fingerprint == running.fingerprint)
}

fn running_workspace_task_current_task<'a>(
    running: &RunningWorkspaceTask,
    tasks: &'a [WorkspaceTask],
    task_fingerprints: &[u64],
) -> Option<&'a WorkspaceTask> {
    let task = tasks.get(running.task_index)?;
    let fingerprint = task_fingerprints.get(running.task_index)?;
    (*fingerprint == running.fingerprint).then_some(task)
}

fn current_workspace_task_session_ids(
    running_tasks: &[RunningWorkspaceTask],
    task_fingerprints: &[u64],
) -> HashSet<usize> {
    let mut session_ids = HashSet::with_capacity(running_tasks.len());
    for running in running_tasks {
        if running_workspace_task_matches_current_task(running, task_fingerprints) {
            session_ids.insert(running.session_id);
        }
    }
    session_ids
}

fn workspace_task_fingerprints(tasks: &[WorkspaceTask]) -> Vec<u64> {
    let mut fingerprints = Vec::with_capacity(tasks.len());
    fingerprints.extend(tasks.iter().map(workspace_task_fingerprint));
    fingerprints
}

fn workspace_task_indexes_by_fingerprint(task_fingerprints: &[u64]) -> HashMap<u64, Vec<usize>> {
    let mut indexes_by_fingerprint = HashMap::with_capacity(task_fingerprints.len());
    for (index, fingerprint) in task_fingerprints.iter().copied().enumerate() {
        indexes_by_fingerprint
            .entry(fingerprint)
            .or_insert_with(Vec::new)
            .push(index);
    }
    indexes_by_fingerprint
}

fn current_workspace_task_index_for_fingerprint(
    indexes_by_fingerprint: &HashMap<u64, Vec<usize>>,
    fingerprint: u64,
) -> Option<usize> {
    let indexes = indexes_by_fingerprint.get(&fingerprint)?;
    if indexes.len() == 1 {
        indexes.first().copied()
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkspaceTaskKindRunPlan {
    Run(usize),
    Load,
    Missing,
}

pub(crate) fn workspace_task_kind_run_plan(
    tasks: &[WorkspaceTask],
    kind: WorkspaceTaskKind,
    tasks_loaded: bool,
    tasks_loading: bool,
) -> WorkspaceTaskKindRunPlan {
    if tasks_loading {
        return WorkspaceTaskKindRunPlan::Load;
    }
    if let Some(index) = workspace_task_default_index(tasks, kind) {
        WorkspaceTaskKindRunPlan::Run(index)
    } else if tasks_loaded {
        WorkspaceTaskKindRunPlan::Missing
    } else {
        WorkspaceTaskKindRunPlan::Load
    }
}

#[cfg(test)]
mod tests {
    use super::{
        RunningWorkspaceTask, WORKSPACE_TASK_STATUS_COMMAND_MAX_CHARS,
        WORKSPACE_TASK_STATUS_ERROR_MAX_CHARS, WORKSPACE_TASK_STATUS_NAME_MAX_CHARS,
        WORKSPACE_TASK_STATUS_PATH_MAX_CHARS, WorkspaceTaskCompletion,
        prune_stale_workspace_task_records, workspace_task_command_label,
        workspace_task_completed_status, workspace_task_display_text,
        workspace_task_display_text_or_cow, workspace_task_error_detail,
        workspace_task_fingerprint, workspace_task_invalid_cwd_status, workspace_task_launch_cwd,
        workspace_task_launch_task, workspace_task_load_failed_status, workspace_task_name_label,
        workspace_task_name_label_text, workspace_task_path_label, workspace_task_started_status,
        workspace_task_started_status_with_cwd, workspace_tasks_loaded_status,
    };
    use crate::{
        source_control_runtime::source_control_app_for_test, terminal_process::TerminalCommand,
    };
    use kuroya_core::{Command, WorkspaceTask, WorkspaceTaskKind};
    use std::{borrow::Cow, collections::BTreeMap, path::PathBuf};

    #[test]
    fn workspace_tasks_loaded_status_reuses_count_labels() {
        assert_eq!(
            workspace_tasks_loaded_status(0),
            "No workspace tasks configured"
        );
        assert_eq!(workspace_tasks_loaded_status(1), "Loaded 1 workspace task");
        assert_eq!(workspace_tasks_loaded_status(2), "Loaded 2 workspace tasks");
    }

    #[test]
    fn workspace_tasks_skip_placeholder_workspace() {
        let root = PathBuf::from("workspace/current");
        let mut app = source_control_app_for_test(root, true);
        app.workspace_placeholder = true;
        app.workspace_tasks_open = true;

        assert!(!app.spawn_workspace_task_load());
        assert_eq!(app.workspace_tasks_in_flight_request_id, None);
        assert_eq!(app.status, "No folder open");

        app.status = "Ready".to_owned();
        app.begin_workspace_tasks();

        assert!(app.workspace_tasks_open);
        assert_eq!(app.workspace_tasks_in_flight_request_id, None);
        assert_eq!(app.status, "No folder open");
    }

    #[test]
    fn workspace_task_stale_pruning_drops_ambiguous_duplicate_fingerprint_remap() {
        let task = workspace_task_for_status();
        let tasks = vec![task.clone(), task.clone()];
        let mut running_tasks = vec![RunningWorkspaceTask {
            task_index: 9,
            fingerprint: workspace_task_fingerprint(&task),
            session_id: 7,
        }];

        prune_stale_workspace_task_records(&mut running_tasks, &tasks);

        assert!(running_tasks.is_empty());
    }

    #[test]
    fn workspace_task_stale_pruning_keeps_exact_duplicate_fingerprint_row() {
        let task = workspace_task_for_status();
        let tasks = vec![task.clone(), task.clone()];
        let mut running_tasks = vec![RunningWorkspaceTask {
            task_index: 1,
            fingerprint: workspace_task_fingerprint(&task),
            session_id: 7,
        }];

        prune_stale_workspace_task_records(&mut running_tasks, &tasks);

        assert_eq!(
            running_tasks,
            vec![RunningWorkspaceTask {
                task_index: 1,
                fingerprint: workspace_task_fingerprint(&task),
                session_id: 7,
            }]
        );
    }

    #[test]
    fn workspace_task_launch_cwd_defaults_missing_cwd_to_workspace_root() {
        let root = PathBuf::from("workspace/current");
        let task = workspace_task_for_status();

        assert_eq!(workspace_task_launch_cwd(&root, &task), Ok(root));
    }

    #[test]
    fn workspace_task_launch_cwd_resolves_relative_cwd_under_workspace() {
        let root = PathBuf::from("workspace/current");
        let mut task = workspace_task_for_status();
        task.cwd = Some(PathBuf::from("crates/app"));

        assert_eq!(
            workspace_task_launch_cwd(&root, &task),
            Ok(root.join("crates/app"))
        );
    }

    #[test]
    fn workspace_task_launch_cwd_accepts_loaded_workspace_relative_cwd() {
        let root = PathBuf::from("workspace/current");
        let mut task = workspace_task_for_status();
        let cwd = root.join("crates/app");
        task.cwd = Some(cwd.clone());

        assert_eq!(workspace_task_launch_cwd(&root, &task), Ok(cwd));
    }

    #[test]
    fn workspace_task_launch_cwd_rejects_parent_escape() {
        let root = PathBuf::from("workspace/current");
        let mut task = workspace_task_for_status();
        task.cwd = Some(PathBuf::from("..").join("outside"));

        assert_eq!(
            workspace_task_launch_cwd(&root, &task),
            Err(root.join("..").join("outside"))
        );
    }

    #[test]
    fn workspace_task_launch_cwd_rejects_parent_reentry() {
        let root = PathBuf::from("workspace/current");
        let mut relative_task = workspace_task_for_status();
        let relative_cwd = PathBuf::from("..").join("current").join("tools");
        relative_task.cwd = Some(relative_cwd.clone());
        assert_eq!(
            workspace_task_launch_cwd(&root, &relative_task),
            Err(root.join(relative_cwd))
        );

        let mut loaded_task = workspace_task_for_status();
        let loaded_cwd = root.join("..").join("current").join("tools");
        loaded_task.cwd = Some(loaded_cwd.clone());
        assert_eq!(
            workspace_task_launch_cwd(&root, &loaded_task),
            Err(loaded_cwd)
        );
    }

    #[test]
    fn workspace_task_launch_clone_preserves_raw_process_fields_and_source_task() {
        let mut task = workspace_task_for_status();
        task.command = "cargo\n\u{202e}raw".to_owned();
        task.args = vec![
            "run\tmode".to_owned(),
            "\u{2066}actual-arg\u{2069}".to_owned(),
        ];
        task.cwd = Some(PathBuf::from("crates/raw\n\u{202e}dir"));
        task.env
            .insert("RAW_ENV".to_owned(), "value\nraw".to_owned());
        let original = task.clone();
        let launch_cwd = PathBuf::from("workspace/current/crates/raw-dir");

        let launch_task = workspace_task_launch_task(&task, launch_cwd.clone());

        assert_eq!(task, original);
        assert_eq!(launch_task.command, original.command);
        assert_eq!(launch_task.args, original.args);
        assert_eq!(launch_task.env, original.env);
        assert_eq!(launch_task.cwd.as_deref(), Some(launch_cwd.as_path()));
    }

    #[test]
    fn run_workspace_task_rejects_runtime_cwd_escape_without_terminal_session() {
        let root = PathBuf::from("workspace/current");
        let mut app = source_control_app_for_test(root, true);
        let mut task = workspace_task_for_status();
        task.cwd = Some(PathBuf::from("..").join(format!(
            "bad\n{}\u{202e}",
            "path-fragment-".repeat(WORKSPACE_TASK_STATUS_PATH_MAX_CHARS)
        )));
        app.workspace_tasks = vec![task];
        app.workspace_tasks_loaded = true;

        app.run_workspace_task(0);

        assert!(app.running_workspace_tasks.is_empty());
        assert!(app.terminal.session_ids_for_test().is_empty());
        assert!(
            app.status
                .starts_with("Task `Test All` cwd is outside the workspace: ")
        );
        assert_safe_display_text(&app.status);
        assert!(app.status.contains("..."));
    }

    #[test]
    fn run_workspace_task_snapshot_rejects_stale_fingerprint_without_terminal_session() {
        let root = PathBuf::from("workspace/current");
        let mut app = source_control_app_for_test(root.clone(), true);
        let mut task = workspace_task_for_status();
        task.cwd = Some(root);
        let mut changed_task = task.clone();
        changed_task.args.push("--changed".to_owned());
        app.workspace_tasks = vec![task];
        app.workspace_tasks_loaded = true;

        app.run_workspace_task_snapshot(0, workspace_task_fingerprint(&changed_task));

        assert!(app.running_workspace_tasks.is_empty());
        assert!(app.terminal.session_ids_for_test().is_empty());
        assert_eq!(app.status, "Workspace task changed; run it again");
    }

    #[test]
    fn run_workspace_task_index_command_is_rejected_as_stale_without_terminal_session() {
        let root = PathBuf::from("workspace/current");
        let mut app = source_control_app_for_test(root.clone(), true);
        let mut task = workspace_task_for_status();
        task.cwd = Some(root);
        app.workspace_tasks = vec![task];
        app.workspace_tasks_loaded = true;

        app.run_command(Command::RunWorkspaceTask(0));

        assert!(app.running_workspace_tasks.is_empty());
        assert!(app.terminal.session_ids_for_test().is_empty());
        assert_eq!(app.status, "Workspace task changed; run it again");
    }

    #[test]
    fn cancel_workspace_task_snapshot_rejects_stale_fingerprint_without_terminal_close() {
        let root = PathBuf::from("workspace/current");
        let mut app = source_control_app_for_test(root.clone(), true);
        let mut task = workspace_task_for_status();
        task.cwd = Some(root);
        let fingerprint = workspace_task_fingerprint(&task);
        let mut changed_task = task.clone();
        changed_task.args.push("--changed".to_owned());
        app.workspace_tasks = vec![task];
        app.workspace_tasks_loaded = true;
        app.running_workspace_tasks = vec![RunningWorkspaceTask {
            task_index: 0,
            fingerprint,
            session_id: 7,
        }];
        let rx_command = app.terminal.add_process_session_for_test(7);

        app.cancel_workspace_task_snapshot(0, workspace_task_fingerprint(&changed_task));

        assert_eq!(app.terminal.session_ids_for_test(), vec![7]);
        assert!(rx_command.try_recv().is_err());
        assert_eq!(app.running_workspace_tasks.len(), 1);
        assert_eq!(app.status, "Workspace task changed; cancel it again");
    }

    #[test]
    fn cancel_workspace_task_snapshot_closes_recorded_session_not_active_session() {
        let root = PathBuf::from("workspace/current");
        let mut app = source_control_app_for_test(root.clone(), true);
        let mut task = workspace_task_for_status();
        task.cwd = Some(root);
        let fingerprint = workspace_task_fingerprint(&task);
        app.workspace_tasks = vec![task];
        app.workspace_tasks_loaded = true;
        app.running_workspace_tasks = vec![RunningWorkspaceTask {
            task_index: 0,
            fingerprint,
            session_id: 7,
        }];
        let target_rx = app.terminal.add_process_session_for_test(7);
        let active_rx = app.terminal.add_process_session_for_test(8);

        app.cancel_workspace_task_snapshot(0, fingerprint);

        assert_eq!(app.terminal.session_ids_for_test(), vec![8]);
        match target_rx
            .try_recv()
            .expect("target task session should receive close")
        {
            TerminalCommand::Close => {}
            TerminalCommand::Input(_) | TerminalCommand::Resize(_) => panic!("expected close"),
        }
        assert!(active_rx.try_recv().is_err());
        assert!(app.running_workspace_tasks.is_empty());
        assert_eq!(app.status, "Canceled task `Test All`");
    }

    #[test]
    fn frame_terminal_drain_prunes_completed_workspace_task_record() {
        let root = PathBuf::from("workspace/current");
        let mut app = source_control_app_for_test(root.clone(), true);
        let mut task = workspace_task_for_status();
        task.cwd = Some(root);
        app.workspace_tasks = vec![task.clone()];
        app.workspace_tasks_loaded = true;
        app.running_workspace_tasks = vec![RunningWorkspaceTask {
            task_index: 0,
            fingerprint: workspace_task_fingerprint(&task),
            session_id: 7,
        }];
        let _rx_command = app.terminal.add_process_session_for_test(7);

        assert!(app.terminal.finish_process_session_for_test(7, Some(17)));
        let (terminal_events, terminal_output_pending) = app.drain_terminal_output_for_frame();

        assert_eq!(terminal_events, 1);
        assert!(!terminal_output_pending);
        assert!(app.running_workspace_tasks.is_empty());
        assert_eq!(app.status, "Task `Test All` failed with exit code 17");
    }

    #[test]
    fn frame_terminal_drain_reports_workspace_task_terminal_failure() {
        let root = PathBuf::from("workspace/current");
        let mut app = source_control_app_for_test(root.clone(), true);
        let mut task = workspace_task_for_status();
        task.cwd = Some(root);
        app.workspace_tasks = vec![task.clone()];
        app.workspace_tasks_loaded = true;
        app.running_workspace_tasks = vec![RunningWorkspaceTask {
            task_index: 0,
            fingerprint: workspace_task_fingerprint(&task),
            session_id: 7,
        }];
        let _rx_command = app.terminal.add_process_session_for_test(7);

        assert!(app.terminal.fail_process_session_for_test(7));
        let (terminal_events, terminal_output_pending) = app.drain_terminal_output_for_frame();

        assert_eq!(terminal_events, 1);
        assert!(!terminal_output_pending);
        assert!(app.running_workspace_tasks.is_empty());
        assert_eq!(app.status, "Task `Test All` failed in terminal");
    }

    #[test]
    fn frame_terminal_drain_prunes_missing_workspace_task_session_without_status_change() {
        let root = PathBuf::from("workspace/current");
        let mut app = source_control_app_for_test(root.clone(), true);
        let mut task = workspace_task_for_status();
        task.cwd = Some(root);
        app.workspace_tasks = vec![task.clone()];
        app.workspace_tasks_loaded = true;
        app.running_workspace_tasks = vec![RunningWorkspaceTask {
            task_index: 0,
            fingerprint: workspace_task_fingerprint(&task),
            session_id: 404,
        }];
        app.status = "Ready".to_owned();

        let (terminal_events, terminal_output_pending) = app.drain_terminal_output_for_frame();

        assert_eq!(terminal_events, 0);
        assert!(!terminal_output_pending);
        assert!(app.running_workspace_tasks.is_empty());
        assert_eq!(app.status, "Ready");
    }

    #[test]
    fn workspace_task_statuses_use_name_fallback_for_control_only_task_names() {
        let mut task = workspace_task_for_status();
        task.name = "\n\t\u{202e}\u{2066}".to_owned();

        assert_eq!(
            workspace_task_completed_status(&task, WorkspaceTaskCompletion::Finished),
            "Task `workspace task` finished"
        );
    }

    #[test]
    fn workspace_task_statuses_use_command_fallback_for_control_only_commands() {
        let mut task = workspace_task_for_status();
        task.command = "\n\t\u{202e}\u{2066}".to_owned();

        assert_eq!(
            workspace_task_started_status(&task),
            "Started task `Test All`: command"
        );
    }

    #[test]
    fn workspace_task_load_failed_status_uses_unknown_error_fallback() {
        assert_eq!(
            workspace_task_load_failed_status("\n\t\u{202e}\u{2066}"),
            "Could not load workspace tasks: unknown error"
        );
    }

    #[test]
    fn workspace_task_display_labels_cow_helpers_borrow_clean_values() {
        let clean_ascii = "cargo build --workspace";
        assert!(matches!(
            workspace_task_display_text_or_cow(
                clean_ascii,
                WORKSPACE_TASK_STATUS_COMMAND_MAX_CHARS,
                "."
            ),
            Cow::Borrowed("cargo build --workspace")
        ));

        let clean_unicode =
            "build \u{65e5}\u{672c}\u{8a9e} \u{41f}\u{440}\u{438}\u{432}\u{435}\u{442}";
        match workspace_task_display_text_or_cow(
            clean_unicode,
            WORKSPACE_TASK_STATUS_COMMAND_MAX_CHARS,
            ".",
        ) {
            Cow::Borrowed(label) => assert_eq!(label, clean_unicode),
            Cow::Owned(label) => panic!("expected borrowed label, got {label:?}"),
        }

        assert!(matches!(
            workspace_task_name_label_text("Build All"),
            Cow::Borrowed("Build All")
        ));
    }

    #[test]
    fn workspace_task_display_labels_cow_helpers_own_dirty_truncated_and_fallback_values() {
        let dirty = workspace_task_display_text_or_cow(
            "cargo\n\u{202e}build",
            WORKSPACE_TASK_STATUS_COMMAND_MAX_CHARS,
            ".",
        );
        assert!(matches!(dirty, Cow::Owned(_)));
        assert_eq!(dirty, "cargo build");

        let overlong = "command-fragment-".repeat(WORKSPACE_TASK_STATUS_COMMAND_MAX_CHARS);
        let truncated = workspace_task_display_text_or_cow(
            &overlong,
            WORKSPACE_TASK_STATUS_COMMAND_MAX_CHARS,
            ".",
        );
        assert!(matches!(truncated, Cow::Owned(_)));
        assert!(truncated.contains("..."));
        assert!(truncated.chars().count() <= WORKSPACE_TASK_STATUS_COMMAND_MAX_CHARS);

        let fallback = workspace_task_name_label_text("\n\t\u{202e}\u{2066}");
        assert!(matches!(fallback, Cow::Owned(_)));
        assert_eq!(fallback, "workspace task");
    }

    #[test]
    fn workspace_task_display_labels_cow_helpers_match_string_wrappers() {
        let samples = [
            "Build All",
            " Build All ",
            "Build\n\u{202e}All",
            "task-fragment-task-fragment-task-fragment-task-fragment-task-fragment",
            "\n\t\u{202e}\u{2066}",
        ];

        for sample in samples {
            assert_eq!(
                workspace_task_name_label_text(sample).into_owned(),
                workspace_task_name_label(sample)
            );
            assert_eq!(
                workspace_task_display_text_or_cow(
                    sample,
                    WORKSPACE_TASK_STATUS_NAME_MAX_CHARS,
                    "."
                )
                .into_owned(),
                workspace_task_display_text(sample, WORKSPACE_TASK_STATUS_NAME_MAX_CHARS)
            );
        }
    }

    #[test]
    fn workspace_task_command_label_uses_fallback_for_blank_command_with_args() {
        let mut task = workspace_task_for_status();
        task.command = "\n\t\u{202e}\u{2066}".to_owned();
        task.args = vec!["--visible".to_owned()];

        assert_eq!(workspace_task_command_label(&task), "command");
    }

    #[test]
    fn workspace_task_started_status_uses_command_preview_with_args() {
        let task = workspace_task_for_status();

        assert_eq!(
            workspace_task_started_status(&task),
            "Started task `Test All`: cargo test"
        );
    }

    #[test]
    fn workspace_task_display_labels_sanitize_hostile_task_text() {
        let mut task = workspace_task_for_status();
        task.name = format!(
            "Task\n{}\u{202e}\u{2066}",
            "name-fragment-".repeat(WORKSPACE_TASK_STATUS_NAME_MAX_CHARS)
        );
        task.command = format!(
            "cargo\r{}\u{202e}",
            "command-fragment-".repeat(WORKSPACE_TASK_STATUS_COMMAND_MAX_CHARS)
        );
        task.args = vec![format!(
            "--message\t{}\u{2029}",
            "arg-fragment-".repeat(WORKSPACE_TASK_STATUS_COMMAND_MAX_CHARS)
        )];
        task.cwd = Some(PathBuf::from(format!(
            "workspace/bad\n{}\u{2067}",
            "path-fragment-".repeat(WORKSPACE_TASK_STATUS_PATH_MAX_CHARS)
        )));
        let error = format!(
            "first line\r\n{}\u{202e}",
            "error-fragment-".repeat(WORKSPACE_TASK_STATUS_ERROR_MAX_CHARS)
        );

        let name_label = workspace_task_name_label(&task.name);
        let command_label = workspace_task_command_label(&task);
        let path_label = workspace_task_path_label(task.cwd.as_ref().unwrap());
        let error_detail = workspace_task_error_detail(&error);
        let started = workspace_task_started_status(&task);
        let completed =
            workspace_task_completed_status(&task, WorkspaceTaskCompletion::FailedExitCode(17));
        let invalid_cwd = workspace_task_invalid_cwd_status(&task, task.cwd.as_ref().unwrap());
        let load_failed = workspace_task_load_failed_status(&error);

        assert_safe_display_text(&name_label);
        assert_safe_display_text(&command_label);
        assert_safe_display_text(&path_label);
        assert_safe_display_text(&error_detail);
        assert_safe_display_text(&started);
        assert_safe_display_text(&completed);
        assert_safe_display_text(&invalid_cwd);
        assert_safe_display_text(&load_failed);
        assert!(name_label.chars().count() <= WORKSPACE_TASK_STATUS_NAME_MAX_CHARS);
        assert!(command_label.chars().count() <= WORKSPACE_TASK_STATUS_COMMAND_MAX_CHARS);
        assert!(path_label.chars().count() <= WORKSPACE_TASK_STATUS_PATH_MAX_CHARS);
        assert!(error_detail.chars().count() <= WORKSPACE_TASK_STATUS_ERROR_MAX_CHARS);
        assert!(started.contains("..."));
        assert!(completed.contains("..."));
        assert!(load_failed.contains("..."));
    }

    #[test]
    fn workspace_task_display_labels_use_blank_fallbacks() {
        let mut task = workspace_task_for_status();
        task.name = "\n\t\u{202e}\u{2066}".to_owned();
        task.command = "\n\t\u{202e}\u{2066}".to_owned();

        assert_eq!(workspace_task_name_label(&task.name), "workspace task");
        assert_eq!(workspace_task_command_label(&task), "command");
        assert_eq!(
            workspace_task_error_detail("\n\t\u{202e}\u{2066}"),
            "unknown error"
        );
        assert_eq!(workspace_task_display_text("\n\t\u{202e}\u{2066}", 80), ".");
    }

    #[test]
    fn workspace_task_status_labels_do_not_rewrite_raw_task_values() {
        let mut task = workspace_task_for_status();
        task.name = "Build\n\u{202e}All".to_owned();
        task.command = "cargo\r\u{2066}build".to_owned();
        task.args = vec!["--message\tformat=json".to_owned()];
        task.cwd = Some(PathBuf::from("workspace/bad\n\u{202e}dir"));
        let original = task.clone();

        let _ = workspace_task_name_label(&task.name);
        let _ = workspace_task_command_label(&task);
        let _ = task.cwd.as_ref().map(|cwd| workspace_task_path_label(cwd));
        let _ = workspace_task_started_status(&task);
        let resolved_cwd = PathBuf::from("workspace/current/resolved");
        let _ = workspace_task_started_status_with_cwd(&task, Some(resolved_cwd.as_path()));
        let _ = workspace_task_completed_status(&task, WorkspaceTaskCompletion::FailedExitCode(17));

        assert_eq!(task, original);
    }

    fn workspace_task_for_status() -> WorkspaceTask {
        WorkspaceTask {
            name: "Test All".to_owned(),
            command: "cargo".to_owned(),
            args: vec!["test".to_owned()],
            cwd: None,
            env: BTreeMap::new(),
            kind: WorkspaceTaskKind::Test,
            default: true,
        }
    }

    fn assert_safe_display_text(text: &str) {
        assert!(
            !text
                .chars()
                .any(|ch| ch.is_control() || matches!(ch, '\u{2028}' | '\u{2029}'))
        );
        assert!(!text.chars().any(is_test_bidi_format_control));
    }

    fn is_test_bidi_format_control(ch: char) -> bool {
        matches!(
            ch,
            '\u{061c}'
                | '\u{200e}'
                | '\u{200f}'
                | '\u{202a}'..='\u{202e}'
                | '\u{2066}'..='\u{2069}'
        )
    }
}
