use crate::source_control_branch_picker::{
    source_control_branch_can_delete, source_control_branch_display_name,
    source_control_branch_status_detail,
};
use kuroya_core::{GitBranch, GitCheckoutType};
use std::collections::HashSet;

pub(super) const SOURCE_CONTROL_BRANCH_REF_INPUT_MAX_CHARS: usize = 1024;

fn next_source_control_branch_operation_request_id(current: u64) -> u64 {
    current.checked_add(1).filter(|id| *id != 0).unwrap_or(1)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SourceControlBranchRefInputError {
    Empty,
    TooLong,
}

pub(super) fn source_control_branch_action_ref(
    branch: String,
) -> Result<String, SourceControlBranchRefInputError> {
    if branch.trim().is_empty() {
        return Err(SourceControlBranchRefInputError::Empty);
    }
    if branch.chars().count() > SOURCE_CONTROL_BRANCH_REF_INPUT_MAX_CHARS {
        return Err(SourceControlBranchRefInputError::TooLong);
    }
    Ok(branch)
}

pub(super) fn source_control_branch_rename_action_refs(
    old_branch: String,
    new_branch: String,
) -> Result<(String, String), SourceControlBranchRefInputError> {
    if old_branch.trim().is_empty() || new_branch.trim().is_empty() {
        return Err(SourceControlBranchRefInputError::Empty);
    }
    if old_branch.chars().count() > SOURCE_CONTROL_BRANCH_REF_INPUT_MAX_CHARS
        || new_branch.chars().count() > SOURCE_CONTROL_BRANCH_REF_INPUT_MAX_CHARS
    {
        return Err(SourceControlBranchRefInputError::TooLong);
    }
    Ok((old_branch, new_branch))
}

pub(super) fn source_control_branch_ref_input_error_status(
    error: SourceControlBranchRefInputError,
) -> String {
    match error {
        SourceControlBranchRefInputError::Empty => "Branch name cannot be empty",
        SourceControlBranchRefInputError::TooLong => "Branch name is too long",
    }
    .to_owned()
}

pub(super) fn source_control_branch_mutation_task_name(name: &str) -> bool {
    matches!(
        name,
        "Git Branch Switch" | "Git Branch Create" | "Git Branch Delete" | "Git Branch Rename"
    )
}

pub(super) fn cached_git_branch_delete_blocked_status(
    branch: &str,
    branches: &[GitBranch],
) -> Option<String> {
    if branches.is_empty() {
        return None;
    }
    if let Some(candidate) = branches
        .iter()
        .find(|candidate| candidate.name == branch && candidate.kind == GitCheckoutType::Local)
    {
        if source_control_branch_can_delete(candidate) {
            return None;
        }
        return Some(format!(
            "Cannot delete the current branch {}",
            source_control_branch_display_name(branch)
        ));
    }
    if branches.iter().any(|candidate| candidate.name == branch) {
        Some(format!(
            "Can only delete local branch {}",
            source_control_branch_display_name(branch)
        ))
    } else {
        Some(format!(
            "Branch {} is no longer available",
            source_control_branch_display_name(branch)
        ))
    }
}

pub(super) fn cached_git_branch_rename_source_blocked_status(
    branch: &str,
    branches: &[GitBranch],
) -> Option<String> {
    if branches.is_empty()
        || branches
            .iter()
            .any(|candidate| candidate.name == branch && candidate.kind == GitCheckoutType::Local)
    {
        return None;
    }
    if branches.iter().any(|candidate| candidate.name == branch) {
        Some(format!(
            "Can only rename local branch {}",
            source_control_branch_display_name(branch)
        ))
    } else {
        Some(format!(
            "Branch {} is no longer available",
            source_control_branch_display_name(branch)
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceControlBranchOperationFinish {
    Active,
    Stale,
    Unknown,
}

pub(crate) fn reserve_source_control_branch_operation_request_id_state(
    next_request_id: &mut u64,
    active_request_id: &mut u64,
    in_flight_request_ids: &mut HashSet<u64>,
) -> u64 {
    let mut request_id = next_source_control_branch_operation_request_id(*next_request_id);
    while in_flight_request_ids.contains(&request_id) {
        request_id = next_source_control_branch_operation_request_id(request_id);
    }
    *next_request_id = request_id;
    *active_request_id = request_id;
    in_flight_request_ids.insert(request_id);
    request_id
}

pub(crate) fn finish_source_control_branch_operation_request_state(
    active_request_id: &mut u64,
    in_flight_request_ids: &mut HashSet<u64>,
    request_id: u64,
) -> SourceControlBranchOperationFinish {
    if !in_flight_request_ids.remove(&request_id) {
        return SourceControlBranchOperationFinish::Unknown;
    }
    if *active_request_id != request_id {
        return SourceControlBranchOperationFinish::Stale;
    }
    *active_request_id = 0;
    SourceControlBranchOperationFinish::Active
}

pub(crate) fn git_branch_list_pending_status() -> String {
    "Loading git branches".to_owned()
}

pub(crate) fn git_branch_list_success_status(count: usize) -> String {
    match count {
        0 => "No local git branches found".to_owned(),
        1 => "Loaded 1 git branch".to_owned(),
        _ => format!("Loaded {count} git branches"),
    }
}

pub(crate) fn git_branch_list_failure_status(error: &str) -> String {
    format!(
        "Could not load git branches: {}",
        source_control_branch_status_detail(error)
    )
}

pub(crate) fn git_branch_switch_pending_status(branch: &str) -> String {
    format!(
        "Switching to {}",
        source_control_branch_display_name(branch)
    )
}

pub(crate) fn git_branch_switch_success_status(branch: &str) -> String {
    format!("Switched to {}", source_control_branch_display_name(branch))
}

pub(crate) fn git_branch_switch_failure_status(branch: &str, error: &str) -> String {
    format!(
        "Could not switch to {}: {}",
        source_control_branch_display_name(branch),
        source_control_branch_status_detail(error)
    )
}

pub(crate) fn git_branch_create_pending_status(branch: &str) -> String {
    format!(
        "Creating branch {}",
        source_control_branch_display_name(branch)
    )
}

pub(crate) fn git_branch_create_success_status(branch: &str) -> String {
    format!(
        "Created and switched to {}",
        source_control_branch_display_name(branch)
    )
}

pub(crate) fn git_branch_create_failure_status(branch: &str, error: &str) -> String {
    format!(
        "Could not create branch {}: {}",
        source_control_branch_display_name(branch),
        source_control_branch_status_detail(error)
    )
}

pub(crate) fn git_branch_delete_pending_status(branch: &str) -> String {
    format!(
        "Deleting branch {}",
        source_control_branch_display_name(branch)
    )
}

pub(crate) fn git_branch_delete_success_status(branch: &str) -> String {
    format!(
        "Deleted branch {}",
        source_control_branch_display_name(branch)
    )
}

pub(crate) fn git_branch_delete_failure_status(branch: &str, error: &str) -> String {
    format!(
        "Could not delete branch {}: {}",
        source_control_branch_display_name(branch),
        source_control_branch_status_detail(error)
    )
}

pub(crate) fn git_branch_rename_pending_status(old_branch: &str, new_branch: &str) -> String {
    format!(
        "Renaming branch {} to {}",
        source_control_branch_display_name(old_branch),
        source_control_branch_display_name(new_branch)
    )
}

pub(crate) fn git_branch_rename_success_status(old_branch: &str, new_branch: &str) -> String {
    format!(
        "Renamed branch {} to {}",
        source_control_branch_display_name(old_branch),
        source_control_branch_display_name(new_branch)
    )
}

pub(crate) fn git_branch_rename_failure_status(
    old_branch: &str,
    new_branch: &str,
    error: &str,
) -> String {
    format!(
        "Could not rename branch {} to {}: {}",
        source_control_branch_display_name(old_branch),
        source_control_branch_display_name(new_branch),
        source_control_branch_status_detail(error)
    )
}

#[cfg(test)]
mod tests {
    use super::reserve_source_control_branch_operation_request_id_state;
    use std::collections::HashSet;

    #[test]
    fn branch_operation_request_ids_wrap_without_zero_or_reusing_in_flight_id() {
        let mut next_request_id = u64::MAX - 1;
        let mut active_request_id = 0;
        let mut in_flight = HashSet::new();

        assert_eq!(
            reserve_source_control_branch_operation_request_id_state(
                &mut next_request_id,
                &mut active_request_id,
                &mut in_flight,
            ),
            u64::MAX
        );

        assert_eq!(active_request_id, u64::MAX);
        assert!(in_flight.contains(&u64::MAX));

        assert_eq!(
            reserve_source_control_branch_operation_request_id_state(
                &mut next_request_id,
                &mut active_request_id,
                &mut in_flight,
            ),
            1
        );

        assert_eq!(next_request_id, 1);
        assert_eq!(active_request_id, 1);
        assert!(!in_flight.contains(&0));
    }

    #[test]
    fn branch_operation_request_ids_skip_wrapped_in_flight_id() {
        let mut next_request_id = u64::MAX;
        let mut active_request_id = 0;
        let mut in_flight = HashSet::from([1]);

        assert_eq!(
            reserve_source_control_branch_operation_request_id_state(
                &mut next_request_id,
                &mut active_request_id,
                &mut in_flight,
            ),
            2
        );

        assert_eq!(next_request_id, 2);
        assert_eq!(active_request_id, 2);
        assert!(in_flight.contains(&1));
        assert!(in_flight.contains(&2));
        assert!(!in_flight.contains(&0));
    }
}
