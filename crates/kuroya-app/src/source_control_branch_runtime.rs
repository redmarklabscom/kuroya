use crate::{
    KuroyaApp,
    devtools_async_tasks::{branch_operation_detail, branch_rename_detail, path_detail},
    source_control_branch_picker::{
        source_control_branch_display_name, source_control_branch_selected_identity,
        source_control_branch_selection_after_reload,
    },
    source_control_runtime::{
        begin_source_control_load_request_state, finish_source_control_load_request_state,
        source_control_panel_load_event_matches,
    },
    ui_events::UiEvent,
    workspace_state::workspace_event_matches,
};
use kuroya_core::{
    GitBranch, GitCheckoutType, checkout_ref, create_branch, delete_branch, list_checkout_refs,
    rename_branch,
};
use std::{path::Path, path::PathBuf};

mod status;

#[cfg(test)]
use self::status::{SOURCE_CONTROL_BRANCH_REF_INPUT_MAX_CHARS, SourceControlBranchRefInputError};
pub(crate) use self::status::{
    SourceControlBranchOperationFinish, finish_source_control_branch_operation_request_state,
    git_branch_create_failure_status, git_branch_create_pending_status,
    git_branch_create_success_status, git_branch_delete_failure_status,
    git_branch_delete_pending_status, git_branch_delete_success_status,
    git_branch_list_failure_status, git_branch_list_pending_status, git_branch_list_success_status,
    git_branch_rename_failure_status, git_branch_rename_pending_status,
    git_branch_rename_success_status, git_branch_switch_failure_status,
    git_branch_switch_pending_status, git_branch_switch_success_status,
    reserve_source_control_branch_operation_request_id_state,
};
use self::status::{
    cached_git_branch_delete_blocked_status, cached_git_branch_rename_source_blocked_status,
    source_control_branch_action_ref, source_control_branch_mutation_task_name,
    source_control_branch_ref_input_error_status, source_control_branch_rename_action_refs,
};

impl KuroyaApp {
    pub(crate) fn begin_git_branch_switcher(&mut self) {
        self.source_control_branch_picker_open = true;
        self.source_control_branch_query.clear();
        self.source_control_branch_rename_from = None;
        self.source_control_branch_selected = 0;
        self.spawn_git_branch_list();
    }

    pub(crate) fn switch_git_branch(&mut self, branch: String, kind: GitCheckoutType) {
        if !self.require_trusted_source_control_mutation("switching branches") {
            return;
        }
        let branch = match source_control_branch_action_ref(branch) {
            Ok(branch) => branch,
            Err(error) => {
                self.status = source_control_branch_ref_input_error_status(error);
                return;
            }
        };
        if !self.begin_source_control_branch_mutation() {
            return;
        }
        if kind == GitCheckoutType::Local && self.git.branch() == Some(branch.as_str()) {
            self.source_control_branch_picker_open = false;
            self.status = format!("Already on {}", source_control_branch_display_name(&branch));
            return;
        }

        let request_id = self.reserve_source_control_branch_operation_request();
        let event_root = self.workspace.root.clone();
        let git_root = self.source_control_git_operation_root();
        let operation_root = git_root.clone();
        let tx = self.tx.clone();
        self.set_git_progress_status(git_branch_switch_pending_status(&branch));
        self.record_async_task_started(
            "Git Branch Switch",
            branch_operation_detail(request_id, &branch),
        );
        self.runtime.spawn_blocking(move || {
            let result = checkout_ref(&git_root, &branch, kind);
            let event = match result {
                Ok(()) => UiEvent::GitBranchSwitchFinished {
                    request_id,
                    root: event_root,
                    operation_root,
                    branch,
                },
                Err(error) => UiEvent::GitBranchSwitchFailed {
                    request_id,
                    root: event_root,
                    operation_root,
                    branch,
                    error: error.to_string(),
                },
            };
            let _ = crate::ui_event_channel::send_ui_event(&tx, event);
        });
    }

    pub(crate) fn create_git_branch(&mut self, branch: String) {
        if !self.require_trusted_source_control_mutation("creating branches") {
            return;
        }
        let branch = match source_control_branch_action_ref(branch) {
            Ok(branch) => branch,
            Err(error) => {
                self.status = source_control_branch_ref_input_error_status(error);
                return;
            }
        };
        if !self.begin_source_control_branch_mutation() {
            return;
        }
        if self
            .source_control_branches
            .iter()
            .any(|existing| existing.name == branch)
        {
            self.status = format!(
                "Branch {} already exists",
                source_control_branch_display_name(&branch)
            );
            return;
        }

        let request_id = self.reserve_source_control_branch_operation_request();
        let event_root = self.workspace.root.clone();
        let git_root = self.source_control_git_operation_root();
        let operation_root = git_root.clone();
        let tx = self.tx.clone();
        self.set_git_progress_status(git_branch_create_pending_status(&branch));
        self.record_async_task_started(
            "Git Branch Create",
            branch_operation_detail(request_id, &branch),
        );
        self.runtime.spawn_blocking(move || {
            let result = create_branch(&git_root, &branch);
            let event = match result {
                Ok(()) => UiEvent::GitBranchCreateFinished {
                    request_id,
                    root: event_root,
                    operation_root,
                    branch,
                },
                Err(error) => UiEvent::GitBranchCreateFailed {
                    request_id,
                    root: event_root,
                    operation_root,
                    branch,
                    error: error.to_string(),
                },
            };
            let _ = crate::ui_event_channel::send_ui_event(&tx, event);
        });
    }

    pub(crate) fn delete_git_branch(&mut self, branch: String) {
        if !self.require_trusted_source_control_mutation("deleting branches") {
            return;
        }
        let branch = match source_control_branch_action_ref(branch) {
            Ok(branch) => branch,
            Err(error) => {
                self.status = source_control_branch_ref_input_error_status(error);
                return;
            }
        };
        if !self.begin_source_control_branch_mutation() {
            return;
        }
        if self.git.branch() == Some(branch.as_str()) {
            self.status = format!(
                "Cannot delete the current branch {}",
                source_control_branch_display_name(&branch)
            );
            return;
        }
        if let Some(status) =
            cached_git_branch_delete_blocked_status(&branch, &self.source_control_branches)
        {
            self.status = status;
            return;
        }

        let event_root = self.workspace.root.clone();
        let git_root = self.source_control_git_operation_root();
        let operation_root = git_root.clone();
        let tx = self.tx.clone();
        self.set_git_progress_status(git_branch_delete_pending_status(&branch));
        self.record_async_task_started("Git Branch Delete", branch.as_str());
        self.runtime.spawn_blocking(move || {
            let result = delete_branch(&git_root, &branch);
            let event = match result {
                Ok(()) => UiEvent::GitBranchDeleteFinished {
                    root: event_root,
                    operation_root,
                    branch,
                },
                Err(error) => UiEvent::GitBranchDeleteFailed {
                    root: event_root,
                    operation_root,
                    branch,
                    error: error.to_string(),
                },
            };
            let _ = crate::ui_event_channel::send_ui_event(&tx, event);
        });
    }

    pub(crate) fn rename_git_branch(&mut self, old_branch: String, new_branch: String) {
        if !self.require_trusted_source_control_mutation("renaming branches") {
            return;
        }
        let (old_branch, new_branch) =
            match source_control_branch_rename_action_refs(old_branch, new_branch) {
                Ok(branches) => branches,
                Err(error) => {
                    self.status = source_control_branch_ref_input_error_status(error);
                    return;
                }
            };
        if !self.begin_source_control_branch_mutation() {
            return;
        }
        if old_branch == new_branch {
            self.status = format!(
                "Branch already has name {}",
                source_control_branch_display_name(&new_branch)
            );
            return;
        }
        if let Some(status) = cached_git_branch_rename_source_blocked_status(
            &old_branch,
            &self.source_control_branches,
        ) {
            self.status = status;
            return;
        }
        if self
            .source_control_branches
            .iter()
            .any(|existing| existing.name == new_branch)
        {
            self.status = format!(
                "Branch {} already exists",
                source_control_branch_display_name(&new_branch)
            );
            return;
        }

        let event_root = self.workspace.root.clone();
        let git_root = self.source_control_git_operation_root();
        let operation_root = git_root.clone();
        let tx = self.tx.clone();
        self.set_git_progress_status(git_branch_rename_pending_status(&old_branch, &new_branch));
        self.record_async_task_started(
            "Git Branch Rename",
            branch_rename_detail(&old_branch, &new_branch),
        );
        self.runtime.spawn_blocking(move || {
            let result = rename_branch(&git_root, &old_branch, &new_branch);
            let event = match result {
                Ok(()) => UiEvent::GitBranchRenameFinished {
                    root: event_root,
                    operation_root,
                    old_branch,
                    new_branch,
                },
                Err(error) => UiEvent::GitBranchRenameFailed {
                    root: event_root,
                    operation_root,
                    old_branch,
                    new_branch,
                    error: error.to_string(),
                },
            };
            let _ = crate::ui_event_channel::send_ui_event(&tx, event);
        });
    }

    pub(crate) fn apply_git_branches_loaded(
        &mut self,
        request_id: u64,
        root: PathBuf,
        operation_root: PathBuf,
        branches: Vec<GitBranch>,
    ) {
        if !source_control_panel_load_event_matches(
            self.source_control_branch_picker_open,
            &self.workspace.root,
            &root,
            request_id,
            self.source_control_branch_active_request_id,
        ) {
            return;
        }
        if !self.source_control_branch_operation_root_matches(&operation_root) {
            return;
        }

        let selected_branch = source_control_branch_selected_identity(
            &self.source_control_branches,
            &self.source_control_branch_query,
            self.source_control_branch_rename_from.as_deref(),
            self.settings.git_branch_sort_order,
            self.source_control_branch_selected,
        );
        let previous_selected = self.source_control_branch_selected;
        self.source_control_branches = branches;
        self.source_control_branch_selected = source_control_branch_selection_after_reload(
            &self.source_control_branches,
            &self.source_control_branch_query,
            self.source_control_branch_rename_from.as_deref(),
            self.settings.git_branch_sort_order,
            previous_selected,
            selected_branch
                .as_ref()
                .map(|(name, kind)| (name.as_str(), *kind)),
        );
        self.status = git_branch_list_success_status(self.source_control_branches.len());
    }

    pub(crate) fn apply_git_branches_failed(
        &mut self,
        request_id: u64,
        root: PathBuf,
        operation_root: PathBuf,
        error: String,
    ) {
        if !source_control_panel_load_event_matches(
            self.source_control_branch_picker_open,
            &self.workspace.root,
            &root,
            request_id,
            self.source_control_branch_active_request_id,
        ) {
            return;
        }
        if !self.source_control_branch_operation_root_matches(&operation_root) {
            return;
        }

        self.source_control_branches.clear();
        self.status = git_branch_list_failure_status(&error);
    }

    pub(crate) fn apply_git_branch_switch_finished(
        &mut self,
        request_id: u64,
        root: PathBuf,
        operation_root: PathBuf,
        branch: String,
    ) {
        if !self.source_control_branch_event_matches(&root, &operation_root) {
            return;
        }
        match self.finish_source_control_branch_operation_request(request_id) {
            SourceControlBranchOperationFinish::Active => {}
            SourceControlBranchOperationFinish::Stale => {
                self.spawn_index();
                self.spawn_git_scan();
                if self.source_control_branch_picker_open {
                    let status = std::mem::take(&mut self.status);
                    self.spawn_git_branch_list();
                    self.status = status;
                }
                return;
            }
            SourceControlBranchOperationFinish::Unknown => return,
        }

        self.source_control_branch_picker_open = false;
        self.source_control_branch_query.clear();
        self.source_control_branch_rename_from = None;
        self.spawn_index();
        self.spawn_git_scan();
        self.status = git_branch_switch_success_status(&branch);
    }

    pub(crate) fn apply_git_branch_switch_failed(
        &mut self,
        request_id: u64,
        root: PathBuf,
        operation_root: PathBuf,
        branch: String,
        error: String,
    ) {
        if !self.source_control_branch_event_matches(&root, &operation_root) {
            return;
        }
        match self.finish_source_control_branch_operation_request(request_id) {
            SourceControlBranchOperationFinish::Active => {}
            SourceControlBranchOperationFinish::Stale => {
                self.spawn_git_scan();
                return;
            }
            SourceControlBranchOperationFinish::Unknown => return,
        }

        self.spawn_git_scan();
        self.status = git_branch_switch_failure_status(&branch, &error);
    }

    pub(crate) fn apply_git_branch_create_finished(
        &mut self,
        request_id: u64,
        root: PathBuf,
        operation_root: PathBuf,
        branch: String,
    ) {
        if !self.source_control_branch_event_matches(&root, &operation_root) {
            return;
        }
        match self.finish_source_control_branch_operation_request(request_id) {
            SourceControlBranchOperationFinish::Active => {}
            SourceControlBranchOperationFinish::Stale => {
                self.spawn_index();
                self.spawn_git_scan();
                if self.source_control_branch_picker_open {
                    let status = std::mem::take(&mut self.status);
                    self.spawn_git_branch_list();
                    self.status = status;
                }
                return;
            }
            SourceControlBranchOperationFinish::Unknown => return,
        }

        self.source_control_branch_picker_open = false;
        self.source_control_branch_query.clear();
        self.source_control_branch_rename_from = None;
        self.spawn_index();
        self.spawn_git_scan();
        self.status = git_branch_create_success_status(&branch);
    }

    pub(crate) fn apply_git_branch_create_failed(
        &mut self,
        request_id: u64,
        root: PathBuf,
        operation_root: PathBuf,
        branch: String,
        error: String,
    ) {
        if !self.source_control_branch_event_matches(&root, &operation_root) {
            return;
        }
        match self.finish_source_control_branch_operation_request(request_id) {
            SourceControlBranchOperationFinish::Active => {}
            SourceControlBranchOperationFinish::Stale => {
                self.spawn_git_scan();
                return;
            }
            SourceControlBranchOperationFinish::Unknown => return,
        }

        self.spawn_git_scan();
        self.status = git_branch_create_failure_status(&branch, &error);
    }

    pub(crate) fn apply_git_branch_delete_finished(
        &mut self,
        root: PathBuf,
        operation_root: PathBuf,
        branch: String,
    ) {
        if !self.source_control_branch_event_matches(&root, &operation_root) {
            return;
        }

        let preserve_identified_operation =
            self.source_control_branch_identified_operation_active();
        self.reload_git_branch_picker_after_unidentified_operation(preserve_identified_operation);
        if preserve_identified_operation {
            return;
        }
        self.status = git_branch_delete_success_status(&branch);
    }

    pub(crate) fn apply_git_branch_delete_failed(
        &mut self,
        root: PathBuf,
        operation_root: PathBuf,
        branch: String,
        error: String,
    ) {
        if !self.source_control_branch_event_matches(&root, &operation_root) {
            return;
        }

        let preserve_identified_operation =
            self.source_control_branch_identified_operation_active();
        self.reload_git_branch_picker_after_unidentified_operation(preserve_identified_operation);
        if preserve_identified_operation {
            return;
        }
        self.status = git_branch_delete_failure_status(&branch, &error);
    }

    pub(crate) fn apply_git_branch_rename_finished(
        &mut self,
        root: PathBuf,
        operation_root: PathBuf,
        old_branch: String,
        new_branch: String,
    ) {
        if !self.source_control_branch_event_matches(&root, &operation_root) {
            return;
        }

        let preserve_identified_operation =
            self.source_control_branch_identified_operation_active();
        if !preserve_identified_operation {
            self.source_control_branch_rename_from = None;
            self.source_control_branch_query.clear();
        }
        self.spawn_index();
        self.spawn_git_scan();
        self.reload_git_branch_picker_after_unidentified_operation(preserve_identified_operation);
        if preserve_identified_operation {
            return;
        }
        self.status = git_branch_rename_success_status(&old_branch, &new_branch);
    }

    pub(crate) fn apply_git_branch_rename_failed(
        &mut self,
        root: PathBuf,
        operation_root: PathBuf,
        old_branch: String,
        new_branch: String,
        error: String,
    ) {
        if !self.source_control_branch_event_matches(&root, &operation_root) {
            return;
        }

        let preserve_identified_operation =
            self.source_control_branch_identified_operation_active();
        self.reload_git_branch_picker_after_unidentified_operation(preserve_identified_operation);
        if preserve_identified_operation {
            return;
        }
        self.status = git_branch_rename_failure_status(&old_branch, &new_branch, &error);
    }

    pub(crate) fn spawn_git_branch_list(&mut self) -> bool {
        let Some(request_id) = self.begin_source_control_branch_request() else {
            self.set_git_progress_status(git_branch_list_pending_status());
            return false;
        };
        let event_root = self.workspace.root.clone();
        let git_root = self.source_control_git_operation_root();
        let operation_root = git_root.clone();
        let checkout_types = self.settings.git_checkout_type.clone();
        let tx = self.tx.clone();
        self.set_git_progress_status(git_branch_list_pending_status());
        self.record_async_task_started("Git Branches", path_detail(&event_root));
        self.runtime.spawn_blocking(move || {
            let result = list_checkout_refs(&git_root, &checkout_types);
            let event = match result {
                Ok(branches) => UiEvent::GitBranchesLoaded {
                    request_id,
                    root: event_root,
                    operation_root,
                    branches,
                },
                Err(error) => UiEvent::GitBranchesFailed {
                    request_id,
                    root: event_root,
                    operation_root,
                    error: error.to_string(),
                },
            };
            let _ = crate::ui_event_channel::send_ui_event(&tx, event);
        });
        true
    }

    fn begin_source_control_branch_request(&mut self) -> Option<u64> {
        begin_source_control_load_request_state(
            &mut self.source_control_branch_next_request_id,
            &mut self.source_control_branch_active_request_id,
            &mut self.source_control_branch_in_flight_request_id,
            &mut self.source_control_branch_reload_queued,
        )
    }

    pub(crate) fn finish_source_control_branch_request(&mut self, request_id: u64) -> bool {
        finish_source_control_load_request_state(
            &mut self.source_control_branch_in_flight_request_id,
            &mut self.source_control_branch_reload_queued,
            request_id,
        )
    }

    pub(crate) fn reserve_source_control_branch_operation_request(&mut self) -> u64 {
        reserve_source_control_branch_operation_request_id_state(
            &mut self.source_control_branch_operation_next_request_id,
            &mut self.source_control_branch_operation_active_request_id,
            &mut self.source_control_branch_operation_in_flight_request_ids,
        )
    }

    pub(crate) fn invalidate_source_control_branch_operation_requests(&mut self) {
        let _ = reserve_source_control_branch_operation_request_id_state(
            &mut self.source_control_branch_operation_next_request_id,
            &mut self.source_control_branch_operation_active_request_id,
            &mut self.source_control_branch_operation_in_flight_request_ids,
        );
        self.source_control_branch_operation_in_flight_request_ids
            .clear();
    }

    fn finish_source_control_branch_operation_request(
        &mut self,
        request_id: u64,
    ) -> SourceControlBranchOperationFinish {
        finish_source_control_branch_operation_request_state(
            &mut self.source_control_branch_operation_active_request_id,
            &mut self.source_control_branch_operation_in_flight_request_ids,
            request_id,
        )
    }

    fn source_control_branch_event_matches(&self, root: &Path, operation_root: &Path) -> bool {
        workspace_event_matches(&self.workspace.root, root)
            && self.source_control_branch_operation_root_matches(operation_root)
    }

    fn source_control_branch_operation_root_matches(&self, operation_root: &Path) -> bool {
        self.source_control_git_operation_root_matches(operation_root)
    }

    fn source_control_branch_identified_operation_active(&self) -> bool {
        let request_id = self.source_control_branch_operation_active_request_id;
        request_id != 0
            && self
                .source_control_branch_operation_in_flight_request_ids
                .contains(&request_id)
    }

    fn reload_git_branch_picker_after_unidentified_operation(&mut self, preserve_status: bool) {
        if !self.source_control_branch_picker_open {
            return;
        }
        if preserve_status {
            let status = std::mem::take(&mut self.status);
            self.spawn_git_branch_list();
            self.status = status;
        } else {
            self.spawn_git_branch_list();
        }
    }

    fn begin_source_control_branch_mutation(&mut self) -> bool {
        if self
            .source_control_branch_operation_in_flight_request_ids
            .is_empty()
            && !self.source_control_branch_mutation_task_in_flight()
        {
            return true;
        }
        self.status = "Branch operation already in progress".to_owned();
        false
    }

    fn source_control_branch_mutation_task_in_flight(&self) -> bool {
        self.active_async_tasks
            .iter()
            .any(|task| source_control_branch_mutation_task_name(&task.name))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SOURCE_CONTROL_BRANCH_REF_INPUT_MAX_CHARS, SourceControlBranchOperationFinish,
        SourceControlBranchRefInputError, cached_git_branch_delete_blocked_status,
        cached_git_branch_rename_source_blocked_status,
        finish_source_control_branch_operation_request_state, git_branch_create_failure_status,
        git_branch_create_pending_status, git_branch_create_success_status,
        git_branch_delete_failure_status, git_branch_delete_pending_status,
        git_branch_delete_success_status, git_branch_list_failure_status,
        git_branch_rename_failure_status, git_branch_rename_pending_status,
        git_branch_rename_success_status, git_branch_switch_failure_status,
        git_branch_switch_pending_status, git_branch_switch_success_status,
        reserve_source_control_branch_operation_request_id_state, source_control_branch_action_ref,
        source_control_branch_rename_action_refs,
    };
    use crate::source_control_runtime::{
        source_control_app_for_test, source_control_mutation_restricted_status,
    };
    use kuroya_core::{GitBranch, GitCheckoutType};
    use std::{collections::HashSet, path::PathBuf};

    fn assert_restricted_status(app: &crate::KuroyaApp, action: &str) {
        assert_eq!(
            app.status,
            source_control_mutation_restricted_status(action)
        );
    }

    fn branch(name: &str, is_current: bool, time: i64) -> GitBranch {
        GitBranch {
            name: name.to_owned(),
            is_current,
            kind: GitCheckoutType::Local,
            committer_time_seconds: time,
        }
    }

    #[test]
    fn branch_operation_request_ids_track_active_in_flight_request() {
        let mut next_request_id = 0;
        let mut active_request_id = 0;
        let mut in_flight_request_ids = HashSet::new();

        let older = reserve_source_control_branch_operation_request_id_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight_request_ids,
        );
        let newer = reserve_source_control_branch_operation_request_id_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight_request_ids,
        );

        assert_eq!(older, 1);
        assert_eq!(newer, 2);
        assert_eq!(active_request_id, newer);
        assert_eq!(in_flight_request_ids, HashSet::from([older, newer]));

        assert_eq!(
            finish_source_control_branch_operation_request_state(
                &mut active_request_id,
                &mut in_flight_request_ids,
                older,
            ),
            SourceControlBranchOperationFinish::Stale
        );
        assert_eq!(active_request_id, newer);
        assert_eq!(in_flight_request_ids, HashSet::from([newer]));

        assert_eq!(
            finish_source_control_branch_operation_request_state(
                &mut active_request_id,
                &mut in_flight_request_ids,
                newer,
            ),
            SourceControlBranchOperationFinish::Active
        );
        assert_eq!(active_request_id, 0);
        assert!(in_flight_request_ids.is_empty());
    }

    #[test]
    fn branch_operation_request_ids_wrap_without_reusing_in_flight_ids() {
        let mut next_request_id = u64::MAX;
        let mut active_request_id = u64::MAX;
        let mut in_flight_request_ids = HashSet::from([1]);

        let request_id = reserve_source_control_branch_operation_request_id_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight_request_ids,
        );

        assert_eq!(request_id, 2);
        assert_eq!(next_request_id, 2);
        assert_eq!(active_request_id, 2);
        assert_eq!(in_flight_request_ids, HashSet::from([1, 2]));
    }

    #[test]
    fn branch_action_refs_reject_blank_input_without_trimming_raw_refs() {
        assert_eq!(
            source_control_branch_action_ref("   ".to_owned()),
            Err(SourceControlBranchRefInputError::Empty)
        );
        assert_eq!(
            source_control_branch_action_ref("  feature/raw ref  ".to_owned())
                .unwrap()
                .as_str(),
            "  feature/raw ref  "
        );
        assert_eq!(
            source_control_branch_rename_action_refs(
                " old/raw ref ".to_owned(),
                " new/raw ref ".to_owned()
            )
            .unwrap(),
            (" old/raw ref ".to_owned(), " new/raw ref ".to_owned())
        );
        assert_eq!(
            source_control_branch_rename_action_refs("old".to_owned(), "\t\n".to_owned()),
            Err(SourceControlBranchRefInputError::Empty)
        );
    }

    #[test]
    fn branch_action_refs_reject_oversized_input() {
        let max_ref = "x".repeat(SOURCE_CONTROL_BRANCH_REF_INPUT_MAX_CHARS);
        let oversized_ref = "x".repeat(SOURCE_CONTROL_BRANCH_REF_INPUT_MAX_CHARS + 1);

        assert!(source_control_branch_action_ref(max_ref.clone()).is_ok());
        assert_eq!(
            source_control_branch_action_ref(oversized_ref.clone()),
            Err(SourceControlBranchRefInputError::TooLong)
        );
        assert_eq!(
            source_control_branch_rename_action_refs(max_ref, oversized_ref),
            Err(SourceControlBranchRefInputError::TooLong)
        );
    }

    #[test]
    fn cached_branch_delete_guard_rejects_stale_or_non_local_refs() {
        let branches = vec![
            branch("main", true, 40),
            branch("feature/delete", false, 30),
            GitBranch {
                name: "origin/feature/delete".to_owned(),
                is_current: false,
                kind: GitCheckoutType::Remote,
                committer_time_seconds: 20,
            },
        ];

        assert_eq!(
            cached_git_branch_delete_blocked_status("feature/delete", &branches),
            None
        );
        assert_eq!(
            cached_git_branch_delete_blocked_status("main", &branches),
            Some("Cannot delete the current branch main".to_owned())
        );
        assert_eq!(
            cached_git_branch_delete_blocked_status("origin/feature/delete", &branches),
            Some("Can only delete local branch origin/feature/delete".to_owned())
        );
        assert_eq!(
            cached_git_branch_delete_blocked_status("feature/missing", &branches),
            Some("Branch feature/missing is no longer available".to_owned())
        );
        assert_eq!(
            cached_git_branch_delete_blocked_status("feature/missing", &[]),
            None
        );
    }

    #[test]
    fn cached_branch_rename_guard_rejects_stale_or_non_local_sources() {
        let branches = vec![
            branch("main", true, 40),
            GitBranch {
                name: "v1".to_owned(),
                is_current: false,
                kind: GitCheckoutType::Tags,
                committer_time_seconds: 20,
            },
        ];

        assert_eq!(
            cached_git_branch_rename_source_blocked_status("main", &branches),
            None
        );
        assert_eq!(
            cached_git_branch_rename_source_blocked_status("v1", &branches),
            Some("Can only rename local branch v1".to_owned())
        );
        assert_eq!(
            cached_git_branch_rename_source_blocked_status("feature/missing", &branches),
            Some("Branch feature/missing is no longer available".to_owned())
        );
        assert_eq!(
            cached_git_branch_rename_source_blocked_status("feature/missing", &[]),
            None
        );
    }

    #[test]
    fn stale_branch_switch_finished_preserves_newer_picker_state_and_status() {
        let root = PathBuf::from("workspace");
        let mut app = source_control_app_for_test(root.clone(), true);
        let older = app.reserve_source_control_branch_operation_request();
        let newer = app.reserve_source_control_branch_operation_request();
        app.source_control_branch_picker_open = true;
        app.source_control_branch_query = "feature/newer".to_owned();
        app.source_control_branch_rename_from = Some("feature/rename".to_owned());
        app.status = git_branch_switch_pending_status("feature/newer");

        app.apply_git_branch_switch_finished(
            older,
            root.clone(),
            root.clone(),
            "feature/old".to_owned(),
        );

        assert!(app.source_control_branch_picker_open);
        assert_eq!(app.source_control_branch_query, "feature/newer");
        assert_eq!(
            app.source_control_branch_rename_from.as_deref(),
            Some("feature/rename")
        );
        assert_eq!(
            app.status,
            git_branch_switch_pending_status("feature/newer")
        );
        assert_eq!(app.source_control_branch_operation_active_request_id, newer);
        assert!(
            !app.source_control_branch_operation_in_flight_request_ids
                .contains(&older)
        );
        assert!(
            app.source_control_branch_operation_in_flight_request_ids
                .contains(&newer)
        );

        app.apply_git_branch_switch_finished(newer, root.clone(), root, "feature/newer".to_owned());

        assert!(!app.source_control_branch_picker_open);
        assert!(app.source_control_branch_query.is_empty());
        assert!(app.source_control_branch_rename_from.is_none());
        assert_eq!(
            app.status,
            git_branch_switch_success_status("feature/newer")
        );
        assert_eq!(app.source_control_branch_operation_active_request_id, 0);
        assert!(
            app.source_control_branch_operation_in_flight_request_ids
                .is_empty()
        );
    }

    #[test]
    fn stale_operation_root_branch_switch_finished_does_not_finish_current_operation() {
        let root = PathBuf::from("workspace");
        let mut app = source_control_app_for_test(root.clone(), true);
        let request_id = app.reserve_source_control_branch_operation_request();
        app.source_control_branch_picker_open = true;
        app.status = git_branch_switch_pending_status("feature/current");

        app.apply_git_branch_switch_finished(
            request_id,
            root.clone(),
            root.join("old-repo"),
            "feature/old".to_owned(),
        );

        assert!(app.source_control_branch_picker_open);
        assert_eq!(
            app.status,
            git_branch_switch_pending_status("feature/current")
        );
        assert_eq!(
            app.source_control_branch_operation_active_request_id,
            request_id
        );
        assert!(
            app.source_control_branch_operation_in_flight_request_ids
                .contains(&request_id)
        );
    }

    #[test]
    fn stale_branch_switch_failed_preserves_newer_status() {
        let root = PathBuf::from("workspace");
        let mut app = source_control_app_for_test(root.clone(), true);
        let older = app.reserve_source_control_branch_operation_request();
        let newer = app.reserve_source_control_branch_operation_request();
        app.status = git_branch_switch_pending_status("feature/newer");

        app.apply_git_branch_switch_failed(
            older,
            root.clone(),
            root.clone(),
            "feature/old".to_owned(),
            "dirty worktree".to_owned(),
        );

        assert_eq!(
            app.status,
            git_branch_switch_pending_status("feature/newer")
        );
        assert_eq!(app.source_control_branch_operation_active_request_id, newer);

        app.apply_git_branch_switch_failed(
            newer,
            root.clone(),
            root,
            "feature/newer".to_owned(),
            "missing branch".to_owned(),
        );

        assert_eq!(
            app.status,
            git_branch_switch_failure_status("feature/newer", "missing branch")
        );
        assert_eq!(app.source_control_branch_operation_active_request_id, 0);
        assert!(
            app.source_control_branch_operation_in_flight_request_ids
                .is_empty()
        );
    }

    #[test]
    fn stale_branch_create_finished_preserves_newer_picker_state_and_status() {
        let root = PathBuf::from("workspace");
        let mut app = source_control_app_for_test(root.clone(), true);
        let older = app.reserve_source_control_branch_operation_request();
        let newer = app.reserve_source_control_branch_operation_request();
        app.source_control_branch_picker_open = true;
        app.source_control_branch_query = "feature/newer".to_owned();
        app.source_control_branch_rename_from = Some("feature/rename".to_owned());
        app.status = git_branch_create_pending_status("feature/newer");

        app.apply_git_branch_create_finished(
            older,
            root.clone(),
            root.clone(),
            "feature/old".to_owned(),
        );

        assert!(app.source_control_branch_picker_open);
        assert_eq!(app.source_control_branch_query, "feature/newer");
        assert_eq!(
            app.source_control_branch_rename_from.as_deref(),
            Some("feature/rename")
        );
        assert_eq!(
            app.status,
            git_branch_create_pending_status("feature/newer")
        );
        assert_eq!(app.source_control_branch_operation_active_request_id, newer);

        app.apply_git_branch_create_finished(newer, root.clone(), root, "feature/newer".to_owned());

        assert!(!app.source_control_branch_picker_open);
        assert!(app.source_control_branch_query.is_empty());
        assert!(app.source_control_branch_rename_from.is_none());
        assert_eq!(
            app.status,
            git_branch_create_success_status("feature/newer")
        );
        assert_eq!(app.source_control_branch_operation_active_request_id, 0);
        assert!(
            app.source_control_branch_operation_in_flight_request_ids
                .is_empty()
        );
    }

    #[test]
    fn stale_branch_create_failed_preserves_newer_status() {
        let root = PathBuf::from("workspace");
        let mut app = source_control_app_for_test(root.clone(), true);
        let older = app.reserve_source_control_branch_operation_request();
        let newer = app.reserve_source_control_branch_operation_request();
        app.status = git_branch_create_pending_status("feature/newer");

        app.apply_git_branch_create_failed(
            older,
            root.clone(),
            root.clone(),
            "feature/old".to_owned(),
            "invalid name".to_owned(),
        );

        assert_eq!(
            app.status,
            git_branch_create_pending_status("feature/newer")
        );
        assert_eq!(app.source_control_branch_operation_active_request_id, newer);

        app.apply_git_branch_create_failed(
            newer,
            root.clone(),
            root,
            "feature/newer".to_owned(),
            "already exists".to_owned(),
        );

        assert_eq!(
            app.status,
            git_branch_create_failure_status("feature/newer", "already exists")
        );
        assert_eq!(app.source_control_branch_operation_active_request_id, 0);
        assert!(
            app.source_control_branch_operation_in_flight_request_ids
                .is_empty()
        );
    }

    #[test]
    fn unidentified_branch_delete_result_preserves_identified_operation_status() {
        let root = PathBuf::from("workspace");
        let mut app = source_control_app_for_test(root.clone(), true);
        let request_id = app.reserve_source_control_branch_operation_request();
        app.source_control_branch_picker_open = true;
        app.source_control_branch_query = "feature/newer".to_owned();
        app.source_control_branch_rename_from = Some("feature/rename".to_owned());
        app.status = git_branch_switch_pending_status("feature/newer");

        app.apply_git_branch_delete_finished(root.clone(), root.clone(), "feature/old".to_owned());

        assert!(app.source_control_branch_picker_open);
        assert_eq!(app.source_control_branch_query, "feature/newer");
        assert_eq!(
            app.source_control_branch_rename_from.as_deref(),
            Some("feature/rename")
        );
        assert_eq!(
            app.status,
            git_branch_switch_pending_status("feature/newer")
        );
        assert_eq!(
            app.source_control_branch_operation_active_request_id,
            request_id
        );
        assert!(
            app.source_control_branch_operation_in_flight_request_ids
                .contains(&request_id)
        );
    }

    #[test]
    fn unidentified_branch_rename_result_preserves_identified_operation_picker_state() {
        let root = PathBuf::from("workspace");
        let mut app = source_control_app_for_test(root.clone(), true);
        let request_id = app.reserve_source_control_branch_operation_request();
        app.source_control_branch_picker_open = true;
        app.source_control_branch_query = "feature/newer".to_owned();
        app.source_control_branch_rename_from = Some("feature/rename".to_owned());
        app.status = git_branch_create_pending_status("feature/newer");

        app.apply_git_branch_rename_finished(
            root.clone(),
            root.clone(),
            "feature/old".to_owned(),
            "feature/renamed".to_owned(),
        );

        assert!(app.source_control_branch_picker_open);
        assert_eq!(app.source_control_branch_query, "feature/newer");
        assert_eq!(
            app.source_control_branch_rename_from.as_deref(),
            Some("feature/rename")
        );
        assert_eq!(
            app.status,
            git_branch_create_pending_status("feature/newer")
        );
        assert_eq!(
            app.source_control_branch_operation_active_request_id,
            request_id
        );
        assert!(
            app.source_control_branch_operation_in_flight_request_ids
                .contains(&request_id)
        );
    }

    #[test]
    fn untrusted_workspace_rejects_branch_mutations() {
        let mut app = source_control_app_for_test(PathBuf::from("workspace"), false);

        app.switch_git_branch("feature".to_owned(), GitCheckoutType::Local);
        assert_restricted_status(&app, "switching branches");

        app.create_git_branch("feature".to_owned());
        assert_restricted_status(&app, "creating branches");

        app.delete_git_branch("feature".to_owned());
        assert_restricted_status(&app, "deleting branches");

        app.rename_git_branch("feature".to_owned(), "renamed".to_owned());
        assert_restricted_status(&app, "renaming branches");
    }

    #[test]
    fn branch_mutations_reject_duplicate_operation_while_request_is_in_flight() {
        let mut app = source_control_app_for_test(PathBuf::from("workspace"), true);
        let request_id = app.reserve_source_control_branch_operation_request();

        app.create_git_branch("feature/duplicate".to_owned());

        assert_eq!(app.status, "Branch operation already in progress");
        assert_eq!(
            app.source_control_branch_operation_active_request_id,
            request_id
        );
        assert_eq!(
            app.source_control_branch_operation_in_flight_request_ids,
            HashSet::from([request_id])
        );
        assert!(app.active_async_tasks.is_empty());
    }

    #[test]
    fn branch_mutations_reject_duplicate_operation_while_task_is_active() {
        let mut app = source_control_app_for_test(PathBuf::from("workspace"), true);
        app.record_async_task_started("Git Branch Delete", "feature/old");

        app.delete_git_branch("feature/old".to_owned());

        assert_eq!(app.status, "Branch operation already in progress");
        assert_eq!(app.active_async_tasks.len(), 1);
    }

    #[test]
    fn branch_reload_preserves_selected_filtered_branch_after_reorder() {
        let root = PathBuf::from("workspace");
        let mut app = source_control_app_for_test(root.clone(), true);
        app.source_control_branch_picker_open = true;
        app.source_control_branch_active_request_id = 7;
        app.source_control_branch_query = "feature".to_owned();
        app.source_control_branches = vec![
            branch("main", true, 40),
            branch("feature/new", false, 30),
            branch("feature/old", false, 20),
        ];
        app.source_control_branch_selected = 0;

        app.apply_git_branches_loaded(
            7,
            root.clone(),
            root,
            vec![
                branch("feature/extra", false, 50),
                branch("main", true, 40),
                branch("feature/new", false, 10),
                branch("feature/old", false, 30),
            ],
        );

        assert_eq!(app.source_control_branch_selected, 2);
        assert_eq!(app.status, "Loaded 4 git branches");
    }

    #[test]
    fn branch_reload_clamps_selection_when_selected_branch_disappears() {
        let root = PathBuf::from("workspace");
        let mut app = source_control_app_for_test(root.clone(), true);
        app.source_control_branch_picker_open = true;
        app.source_control_branch_active_request_id = 8;
        app.source_control_branch_query = "feature".to_owned();
        app.source_control_branches = vec![
            branch("main", true, 40),
            branch("feature/old", false, 30),
            branch("feature/missing", false, 20),
        ];
        app.source_control_branch_selected = 1;

        app.apply_git_branches_loaded(
            8,
            root.clone(),
            root,
            vec![
                branch("main", true, 40),
                branch("feature/old", false, 30),
                branch("feature/new", false, 20),
            ],
        );

        assert_eq!(app.source_control_branch_selected, 1);
        assert_eq!(app.status, "Loaded 3 git branches");
    }

    #[test]
    fn branch_runtime_statuses_sanitize_branch_names_and_git_errors() {
        let branch = format!("feature/\n{}\u{202e}tail", "branch-".repeat(80));
        let target = format!("target/\r\n{}\u{2066}tail", "target-".repeat(80));
        let error = format!("first line\nsecond line\u{202e}{}", "error-".repeat(80));

        let statuses = [
            git_branch_list_failure_status(&error),
            git_branch_switch_pending_status(&branch),
            git_branch_switch_success_status(&branch),
            git_branch_switch_failure_status(&branch, &error),
            git_branch_create_pending_status(&branch),
            git_branch_create_success_status(&branch),
            git_branch_create_failure_status(&branch, &error),
            git_branch_delete_pending_status(&branch),
            git_branch_delete_success_status(&branch),
            git_branch_delete_failure_status(&branch, &error),
            git_branch_rename_pending_status(&branch, &target),
            git_branch_rename_success_status(&branch, &target),
            git_branch_rename_failure_status(&branch, &target, &error),
        ];

        for status in statuses {
            assert_runtime_display_text_is_safe(&status);
            assert!(status.contains("..."));
        }

        let failure = git_branch_rename_failure_status(&branch, &target, &error);
        assert!(failure.contains("feature/ branch-"));
        assert!(failure.contains("target/ target-"));
        assert!(failure.contains("first line second line"));
    }

    fn assert_runtime_display_text_is_safe(value: &str) {
        assert!(
            !value.chars().any(is_unsafe_runtime_display_char),
            "display text contains unsafe characters: {value:?}"
        );
        assert!(
            value.chars().count() <= 700,
            "display text should be bounded: {} chars",
            value.chars().count()
        );
    }

    fn is_unsafe_runtime_display_char(ch: char) -> bool {
        ch.is_control()
            || matches!(
                ch,
                '\u{061c}'
                    | '\u{200e}'
                    | '\u{200f}'
                    | '\u{2028}'
                    | '\u{2029}'
                    | '\u{202a}'..='\u{202e}'
                    | '\u{2066}'..='\u{2069}'
            )
    }
}
