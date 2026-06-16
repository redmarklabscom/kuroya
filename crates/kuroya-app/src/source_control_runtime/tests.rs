use super::{
    SOURCE_CONTROL_COMMIT_HASH_DISPLAY_MAX_CHARS, SOURCE_CONTROL_EMPTY_COMMIT_MESSAGE_STATUS,
    SOURCE_CONTROL_PROTECTED_BRANCH_PATTERN_DISPLAY_MAX_CHARS, SourceControlCommitRequestFinish,
    begin_source_control_load_request_state, finish_source_control_commit_request_state,
    finish_source_control_load_request_state, first_stale_source_control_operation_path,
    first_stale_source_control_path, git_commit_failure_status, git_commit_hash_display,
    git_commit_hash_display_cow, git_commit_success_status, git_discard_failure_status,
    git_source_control_target, git_stage_failure_status, git_stage_pending_status,
    git_unstage_failure_status, invalidate_source_control_load_request_state,
    mark_source_control_commit_request_in_flight_state, no_source_control_changes_status,
    no_staged_changes_status, no_unstaged_changes_status,
    reserve_source_control_commit_request_id_state, smart_commit_path_count,
    source_control_app_for_test, source_control_git_operation_root_for_snapshot,
    source_control_git_operation_root_matches_snapshot, source_control_has_stage,
    source_control_load_event_matches, source_control_mutation_restricted_status,
    source_control_paths_for_stage, source_control_protected_branch_new_branch_required_status,
    source_control_protected_branch_pattern_display,
    source_control_protected_branch_pattern_display_cow, source_control_revealed_status,
    source_control_root_matches_workspace, source_control_stage_path_count,
    stale_source_control_discard_status, stale_source_control_stage_status,
};
use crate::transient_state::PendingSourceControlCommitSave;
use crate::{
    app_state::{PendingFileReload, PendingFormatOnSave, QueuedFileReload},
    path_display::{DISPLAY_ERROR_LABEL_MAX_CHARS, DISPLAY_PATH_LABEL_MAX_CHARS},
};
use kuroya_core::{
    BufferId, GitChangeStage, GitFileStatus, GitSmartCommitChanges, GitStatusEntry, TextBuffer,
};
use std::{borrow::Cow, collections::HashSet, path::PathBuf};

fn assert_display_status_sanitized(status: &str) {
    assert!(!status.contains('\n'));
    assert!(!status.contains('\r'));
    assert!(!status.contains('\t'));
    assert!(!status.contains('\u{2028}'));
    assert!(!status.contains('\u{2029}'));
    assert!(!status.contains('\u{202e}'));
}

fn assert_restricted_status(app: &crate::KuroyaApp, action: &str) {
    assert_eq!(
        app.status,
        source_control_mutation_restricted_status(action)
    );
}

#[test]
fn source_control_git_operation_root_prefers_matching_child_or_parent_repo_root() {
    let workspace = PathBuf::from("workspace");
    let child_repo = workspace.join("packages").join("app");
    assert_eq!(
        source_control_git_operation_root_for_snapshot(&workspace, Some(&child_repo)),
        child_repo
    );

    let repo_parent = PathBuf::from("repo");
    let workspace_child = repo_parent.join("src").join("tooling");
    assert_eq!(
        source_control_git_operation_root_for_snapshot(&workspace_child, Some(&repo_parent)),
        repo_parent
    );

    assert_eq!(
        source_control_git_operation_root_for_snapshot(
            &workspace,
            Some(PathBuf::from("other").as_path())
        ),
        workspace
    );
}

#[test]
fn source_control_git_operation_root_match_uses_current_snapshot_without_pathbuf_clone() {
    let workspace = PathBuf::from("workspace");
    let child_repo = workspace.join("packages").join("app");
    assert!(source_control_git_operation_root_matches_snapshot(
        &workspace,
        Some(&child_repo),
        &child_repo
    ));
    assert!(!source_control_git_operation_root_matches_snapshot(
        &workspace,
        Some(&child_repo),
        &workspace
    ));

    assert!(source_control_git_operation_root_matches_snapshot(
        &workspace,
        Some(PathBuf::from("other").as_path()),
        &workspace
    ));
    assert!(!source_control_git_operation_root_matches_snapshot(
        &workspace,
        Some(PathBuf::from("other").as_path()),
        &child_repo
    ));
}

#[test]
fn source_control_load_events_accept_child_and_parent_repo_roots() {
    let workspace = PathBuf::from("workspace");
    let child_repo = workspace.join("packages").join("app");
    let repo_parent = PathBuf::from("repo");
    let workspace_child = repo_parent.join("src").join("tooling");

    assert!(source_control_root_matches_workspace(
        &workspace,
        &child_repo
    ));
    assert!(source_control_root_matches_workspace(
        &workspace_child,
        &repo_parent
    ));
    assert!(source_control_load_event_matches(
        &workspace,
        &child_repo,
        7,
        7
    ));
    assert!(source_control_load_event_matches(
        &workspace_child,
        &repo_parent,
        8,
        8
    ));
    assert!(!source_control_load_event_matches(
        &workspace,
        &PathBuf::from("other"),
        7,
        7
    ));
    assert!(!source_control_load_event_matches(
        &workspace,
        &child_repo,
        6,
        7
    ));
}

#[test]
fn source_control_load_invalidation_invalidates_virtual_open_requests() {
    let mut app = source_control_app_for_test(PathBuf::from("workspace"), true);
    app.source_control_history_loading = true;
    app.virtual_diff_open_next_request_id = 7;
    app.virtual_diff_open_active_request_id = 7;
    app.virtual_revision_open_next_request_id = 11;
    app.virtual_revision_open_active_request_id = 11;

    app.invalidate_source_control_load_requests();

    assert!(!app.source_control_history_loading);
    assert_eq!(app.virtual_diff_open_next_request_id, 8);
    assert_eq!(app.virtual_diff_open_active_request_id, 8);
    assert_eq!(app.virtual_revision_open_next_request_id, 12);
    assert_eq!(app.virtual_revision_open_active_request_id, 12);
}

#[test]
fn pending_restored_source_control_loads_wait_for_accepted_git_root() {
    let root = PathBuf::from("workspace");
    let mut app = source_control_app_for_test(root, true);
    app.source_control_history_open = true;
    app.source_control_stashes_open = true;
    app.pending_restored_git_history_load = true;
    app.pending_restored_git_stashes_load = true;

    app.drain_pending_restored_source_control_loads();

    assert!(app.pending_restored_git_history_load);
    assert!(app.pending_restored_git_stashes_load);
    assert_eq!(app.source_control_history_next_request_id, 0);
    assert_eq!(app.source_control_stashes_next_request_id, 0);
}

#[test]
fn pending_restored_source_control_loads_clear_when_git_disabled() {
    let root = PathBuf::from("workspace");
    let mut app = source_control_app_for_test(root, true);
    app.settings.git_enabled = false;
    app.pending_restored_git_history_load = true;
    app.pending_restored_git_stashes_load = true;

    app.drain_pending_restored_source_control_loads();

    assert!(!app.pending_restored_git_history_load);
    assert!(!app.pending_restored_git_stashes_load);
}

fn insert_dirty_pending_format_on_save(app: &mut crate::KuroyaApp, id: BufferId, path: PathBuf) {
    let mut buffer = TextBuffer::from_text(id, Some(path.clone()), "fn main() {}\n".to_owned());
    let version = buffer.version();
    buffer.mark_dirty();
    app.buffers.push(buffer);
    app.pending_format_on_save.insert(
        id,
        PendingFormatOnSave {
            save_path: path.clone(),
            format_path: path,
            version,
            request_id: 1,
        },
    );
}

#[test]
fn source_control_path_statuses_sanitize_and_bound_display_only_paths() {
    let path = PathBuf::from("workspace")
        .join("src")
        .join(format!("bad\n{}\u{202e}tail.rs", "very-long-".repeat(24)));
    let original_path = path.clone();
    let statuses = [
        no_source_control_changes_status(&path),
        source_control_revealed_status(&path),
        no_unstaged_changes_status(&path),
        no_staged_changes_status(&path),
        git_stage_pending_status(std::slice::from_ref(&path)),
    ];

    for status in statuses {
        assert_display_status_sanitized(&status);
        assert!(status.contains("..."));
    }
    assert!(
        no_source_control_changes_status(&path).chars().count()
            <= "No source control changes in ".chars().count() + DISPLAY_PATH_LABEL_MAX_CHARS
    );
    assert!(
        source_control_revealed_status(&path).chars().count()
            <= "Revealed ".chars().count()
                + DISPLAY_PATH_LABEL_MAX_CHARS
                + " in Source Control".chars().count()
    );
    assert_eq!(path, original_path);
}

#[test]
fn source_control_error_statuses_sanitize_and_bound_display_only_errors() {
    let path = PathBuf::from("workspace")
        .join("src")
        .join(format!("bad\n{}\u{202e}tail.rs", "very-long-".repeat(24)));
    let paths = vec![path];
    let error = format!(
        "first line\nsecond line\t\u{202e}{}",
        "x".repeat(DISPLAY_ERROR_LABEL_MAX_CHARS * 2)
    );
    let original_paths = paths.clone();
    let original_error = error.clone();
    let statuses = [
        git_stage_failure_status(&paths, &error),
        git_unstage_failure_status(&paths, &error),
        git_discard_failure_status(&paths, &error),
        git_commit_failure_status(&error, false),
        git_commit_failure_status(&error, true),
    ];

    for status in statuses {
        assert_display_status_sanitized(&status);
        assert!(status.contains("..."));
    }
    assert!(
        git_commit_failure_status(&error, false).chars().count()
            <= "Could not commit staged changes: ".chars().count() + DISPLAY_ERROR_LABEL_MAX_CHARS
    );
    assert_eq!(paths, original_paths);
    assert_eq!(error, original_error);
}

#[test]
fn source_control_commit_success_status_preserves_normal_hash_wording() {
    assert_eq!(
        git_commit_success_status("12345678", false),
        "Committed staged changes (12345678)"
    );
    assert_eq!(
        git_commit_success_status("12345678", true),
        "Smart committed changes (12345678)"
    );
}

#[test]
fn source_control_commit_hash_display_cow_borrows_clean_labels() {
    let ascii = "12345678";
    assert!(matches!(
        git_commit_hash_display_cow(ascii),
        Cow::Borrowed(label) if label == ascii
    ));
    assert_eq!(git_commit_hash_display(ascii), ascii);

    let unicode = "commit-\u{03c0}";
    match git_commit_hash_display_cow(unicode) {
        Cow::Borrowed(label) => assert_eq!(label, unicode),
        Cow::Owned(label) => panic!("expected borrowed commit hash label, got {label:?}"),
    }
    assert_eq!(git_commit_hash_display(unicode), unicode);
}

#[test]
fn source_control_commit_hash_display_cow_owns_dirty_truncated_and_fallback_labels() {
    let truncated = format!(
        "abc{}",
        "def".repeat(SOURCE_CONTROL_COMMIT_HASH_DISPLAY_MAX_CHARS)
    );
    let cases = ["12\n34\u{202e}", truncated.as_str(), "   "];

    for short_oid in cases {
        let label = git_commit_hash_display_cow(short_oid);
        assert_eq!(git_commit_hash_display(short_oid), label.as_ref());
        assert!(
            matches!(label, Cow::Owned(_)),
            "expected owned commit hash label for {short_oid:?}"
        );
    }

    assert_eq!(git_commit_hash_display("   "), "unknown");
}

#[test]
fn source_control_commit_success_status_sanitizes_and_bounds_hash() {
    let short_oid = format!("12\n34\u{202e}{}", "a".repeat(128));
    let status = git_commit_success_status(&short_oid, false);

    assert!(status.starts_with("Committed staged changes (12 34"));
    assert!(status.contains("..."));
    assert_display_status_sanitized(&status);
    assert!(
        status.chars().count()
            <= "Committed staged changes ()".chars().count()
                + SOURCE_CONTROL_COMMIT_HASH_DISPLAY_MAX_CHARS
    );
}

#[test]
fn source_control_commit_request_ids_track_active_in_flight_request() {
    let mut next_request_id = 0;
    let mut active_request_id = 0;
    let mut in_flight_request_ids = HashSet::new();

    let older = reserve_source_control_commit_request_id_state(
        &mut next_request_id,
        &mut active_request_id,
        &mut in_flight_request_ids,
    );
    let newer = reserve_source_control_commit_request_id_state(
        &mut next_request_id,
        &mut active_request_id,
        &mut in_flight_request_ids,
    );

    assert_eq!(older, 1);
    assert_eq!(newer, 2);
    assert_eq!(active_request_id, newer);
    assert!(in_flight_request_ids.is_empty());
    mark_source_control_commit_request_in_flight_state(&mut in_flight_request_ids, older);
    mark_source_control_commit_request_in_flight_state(&mut in_flight_request_ids, newer);
    assert_eq!(in_flight_request_ids, HashSet::from([older, newer]));

    assert_eq!(
        finish_source_control_commit_request_state(
            &mut active_request_id,
            &mut in_flight_request_ids,
            older,
        ),
        SourceControlCommitRequestFinish::Stale
    );
    assert_eq!(active_request_id, newer);
    assert_eq!(in_flight_request_ids, HashSet::from([newer]));

    assert_eq!(
        finish_source_control_commit_request_state(
            &mut active_request_id,
            &mut in_flight_request_ids,
            newer,
        ),
        SourceControlCommitRequestFinish::Active
    );
    assert_eq!(active_request_id, 0);
    assert!(in_flight_request_ids.is_empty());
}

#[test]
fn source_control_commit_request_ids_wrap_without_reusing_in_flight_ids() {
    let mut next_request_id = u64::MAX;
    let mut active_request_id = u64::MAX;
    let mut in_flight_request_ids = HashSet::from([1]);

    let request_id = reserve_source_control_commit_request_id_state(
        &mut next_request_id,
        &mut active_request_id,
        &mut in_flight_request_ids,
    );

    assert_eq!(request_id, 2);
    assert_eq!(next_request_id, 2);
    assert_eq!(active_request_id, 2);
    assert_eq!(in_flight_request_ids, HashSet::from([1]));
}

#[test]
fn stale_git_commit_finished_does_not_clear_newer_draft_or_status() {
    let root = PathBuf::from("workspace");
    let mut app = source_control_app_for_test(root.clone(), true);
    let older = app.reserve_source_control_commit_request();
    app.mark_source_control_commit_request_in_flight(older);
    let newer = app.reserve_source_control_commit_request();
    app.mark_source_control_commit_request_in_flight(newer);
    app.source_control_commit_message = "newer draft".to_owned();
    app.status = "Committing newer request".to_owned();

    app.apply_git_commit_finished(
        older,
        root.clone(),
        "11111111".to_owned(),
        "older commit".to_owned(),
        false,
    );

    assert_eq!(app.source_control_commit_message, "newer draft");
    assert!(app.source_control_commit_history.is_empty());
    assert_eq!(app.status, "Committing newer request");
    assert_eq!(app.source_control_commit_active_request_id, newer);
    assert!(
        !app.source_control_commit_in_flight_request_ids
            .contains(&older)
    );
    assert!(
        app.source_control_commit_in_flight_request_ids
            .contains(&newer)
    );

    app.apply_git_commit_finished(
        newer,
        root,
        "22222222".to_owned(),
        "newer draft".to_owned(),
        false,
    );

    assert!(app.source_control_commit_message.is_empty());
    assert_eq!(app.source_control_commit_history, vec!["newer draft"]);
    assert_eq!(app.status, "Committed staged changes (22222222)");
    assert_eq!(app.source_control_commit_active_request_id, 0);
    assert!(app.source_control_commit_in_flight_request_ids.is_empty());
}

#[test]
fn stale_git_commit_finished_ignores_newer_unspawned_commit_intent() {
    let root = PathBuf::from("workspace");
    let mut app = source_control_app_for_test(root.clone(), true);
    let older = app.reserve_source_control_commit_request();
    app.mark_source_control_commit_request_in_flight(older);
    let newer_prompt = app.reserve_source_control_commit_request();
    app.source_control_commit_message = "draft for pending prompt".to_owned();
    app.status = "Waiting for commit confirmation".to_owned();

    app.apply_git_commit_finished(
        older,
        root,
        "11111111".to_owned(),
        "older commit".to_owned(),
        false,
    );

    assert_eq!(
        app.source_control_commit_message,
        "draft for pending prompt"
    );
    assert!(app.source_control_commit_history.is_empty());
    assert_eq!(app.status, "Waiting for commit confirmation");
    assert_eq!(app.source_control_commit_active_request_id, newer_prompt);
    assert!(app.source_control_commit_in_flight_request_ids.is_empty());
}

#[test]
fn current_git_commit_finished_preserves_user_edited_newer_draft() {
    let root = PathBuf::from("workspace");
    let mut app = source_control_app_for_test(root.clone(), true);
    let request_id = app.reserve_source_control_commit_request();
    app.mark_source_control_commit_request_in_flight(request_id);
    app.source_control_commit_message = "new draft after submit".to_owned();

    app.apply_git_commit_finished(
        request_id,
        root,
        "33333333".to_owned(),
        "submitted commit".to_owned(),
        false,
    );

    assert_eq!(app.source_control_commit_message, "new draft after submit");
    assert_eq!(app.source_control_commit_history, vec!["submitted commit"]);
    assert_eq!(app.status, "Committed staged changes (33333333)");
    assert!(app.source_control_commit_in_flight_request_ids.is_empty());
}

#[test]
fn stale_git_commit_failed_does_not_replace_newer_status() {
    let root = PathBuf::from("workspace");
    let mut app = source_control_app_for_test(root.clone(), true);
    let older = app.reserve_source_control_commit_request();
    app.mark_source_control_commit_request_in_flight(older);
    let newer = app.reserve_source_control_commit_request();
    app.mark_source_control_commit_request_in_flight(newer);
    app.status = "Committing newer request".to_owned();

    app.apply_git_commit_failed(older, root.clone(), "older failure".to_owned(), false);

    assert_eq!(app.status, "Committing newer request");
    assert_eq!(app.source_control_commit_active_request_id, newer);
    assert!(
        !app.source_control_commit_in_flight_request_ids
            .contains(&older)
    );
    assert!(
        app.source_control_commit_in_flight_request_ids
            .contains(&newer)
    );

    app.apply_git_commit_failed(newer, root, "newer failure".to_owned(), true);

    assert_eq!(app.status, "Could not smart commit changes: newer failure");
    assert_eq!(app.source_control_commit_active_request_id, 0);
    assert!(app.source_control_commit_in_flight_request_ids.is_empty());
}

#[test]
fn source_control_protected_branch_pattern_display_cow_borrows_clean_labels() {
    let ascii = "release/*";
    assert!(matches!(
        source_control_protected_branch_pattern_display_cow(ascii),
        Cow::Borrowed(label) if label == ascii
    ));
    assert_eq!(
        source_control_protected_branch_pattern_display(ascii),
        ascii
    );

    let unicode = "release/\u{03c0}-*";
    match source_control_protected_branch_pattern_display_cow(unicode) {
        Cow::Borrowed(label) => assert_eq!(label, unicode),
        Cow::Owned(label) => {
            panic!("expected borrowed protected branch pattern label, got {label:?}")
        }
    }
    assert_eq!(
        source_control_protected_branch_pattern_display(unicode),
        unicode
    );
}

#[test]
fn source_control_protected_branch_pattern_display_cow_owns_dirty_truncated_and_fallback_labels() {
    let truncated = format!(
        "release/{}",
        "pattern-".repeat(SOURCE_CONTROL_PROTECTED_BRANCH_PATTERN_DISPLAY_MAX_CHARS)
    );
    let cases = ["release/*\r\nnext\u{2066}", truncated.as_str(), "   "];

    for pattern in cases {
        let label = source_control_protected_branch_pattern_display_cow(pattern);
        assert_eq!(
            source_control_protected_branch_pattern_display(pattern),
            label.as_ref()
        );
        assert!(
            matches!(label, Cow::Owned(_)),
            "expected owned protected branch pattern label for {pattern:?}"
        );
    }

    assert_eq!(
        source_control_protected_branch_pattern_display("   "),
        "protected branch pattern"
    );
}

#[test]
fn source_control_protected_branch_required_status_sanitizes_branch_and_pattern() {
    let branch = format!("main\n{}\u{202e}tail", "branch-".repeat(80));
    let pattern = format!("release/*\r\n{}\u{2066}tail", "pattern-".repeat(80));
    let status = source_control_protected_branch_new_branch_required_status(&branch, &pattern);

    assert_display_status_sanitized(&status);
    assert!(status.starts_with("Branch main branch-"));
    assert!(status.contains(" is protected by release/* pattern-"));
    assert!(status.contains("..."));
    assert!(
        status.chars().count()
            <= "Branch  is protected by ; create or switch branches before committing"
                .chars()
                .count()
                + 160
                + SOURCE_CONTROL_PROTECTED_BRANCH_PATTERN_DISPLAY_MAX_CHARS
    );
}

#[test]
fn source_control_target_label_sanitizes_single_path_without_changing_bulk_target() {
    let path = PathBuf::from("workspace")
        .join("src")
        .join(format!("bad\r\n{}\u{202e}tail.rs", "very-long-".repeat(24)));
    let original_path = path.clone();
    let target = git_source_control_target(std::slice::from_ref(&path));

    assert_display_status_sanitized(&target);
    assert!(target.starts_with("changes in "));
    assert!(target.contains("..."));
    assert!(target.chars().count() <= "changes in ".chars().count() + DISPLAY_PATH_LABEL_MAX_CHARS);
    assert_eq!(path, original_path);
    assert_eq!(
        git_source_control_target(&[path, PathBuf::from("workspace/README.md")]),
        "changes in 2 files"
    );
}

#[test]
fn source_control_load_request_starts_when_idle() {
    let mut next_request_id = 0;
    let mut active_request_id = 0;
    let mut in_flight = None;
    let mut queued = false;

    assert_eq!(
        begin_source_control_load_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        ),
        Some(1)
    );

    assert_eq!(next_request_id, 1);
    assert_eq!(active_request_id, 1);
    assert_eq!(in_flight, Some(1));
    assert!(!queued);
}

#[test]
fn source_control_load_request_queues_once_while_in_flight() {
    let mut next_request_id = 0;
    let mut active_request_id = 0;
    let mut in_flight = None;
    let mut queued = false;

    assert_eq!(
        begin_source_control_load_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        ),
        Some(1)
    );
    assert_eq!(
        begin_source_control_load_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        ),
        None
    );
    assert_eq!(
        begin_source_control_load_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        ),
        None
    );

    assert_eq!(next_request_id, 3);
    assert_eq!(active_request_id, 3);
    assert_eq!(in_flight, Some(1));
    assert!(queued);
}

#[test]
fn source_control_load_request_ids_wrap_without_reusing_in_flight_request() {
    let mut next_request_id = u64::MAX - 1;
    let mut active_request_id = u64::MAX - 1;
    let mut in_flight = None;
    let mut queued = false;

    assert_eq!(
        begin_source_control_load_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        ),
        Some(u64::MAX)
    );
    assert_eq!(
        begin_source_control_load_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        ),
        None
    );

    assert_eq!(next_request_id, 1);
    assert_eq!(active_request_id, 1);
    assert_eq!(in_flight, Some(u64::MAX));
    assert!(queued);
    assert!(!source_control_load_event_matches(
        &PathBuf::from("workspace"),
        &PathBuf::from("workspace"),
        u64::MAX,
        active_request_id,
    ));

    assert!(finish_source_control_load_request_state(
        &mut in_flight,
        &mut queued,
        u64::MAX,
    ));
    assert_eq!(
        begin_source_control_load_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        ),
        Some(2)
    );
}

#[test]
fn source_control_load_finish_drains_queued_reload_once() {
    let mut next_request_id = 0;
    let mut active_request_id = 0;
    let mut in_flight = None;
    let mut queued = false;

    assert_eq!(
        begin_source_control_load_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        ),
        Some(1)
    );
    assert_eq!(
        begin_source_control_load_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        ),
        None
    );

    assert!(finish_source_control_load_request_state(
        &mut in_flight,
        &mut queued,
        1,
    ));
    assert_eq!(in_flight, None);
    assert!(!queued);
    assert_eq!(
        begin_source_control_load_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        ),
        Some(3)
    );
}

#[test]
fn source_control_load_finish_ignores_unrelated_request_id() {
    let mut in_flight = Some(4);
    let mut queued = true;

    assert!(!finish_source_control_load_request_state(
        &mut in_flight,
        &mut queued,
        3,
    ));

    assert_eq!(in_flight, Some(4));
    assert!(queued);
}

#[test]
fn source_control_load_invalidation_keeps_request_ids_monotonic() {
    let mut next_request_id = 4;
    let mut active_request_id = 4;
    let mut in_flight = Some(4);
    let mut queued = true;

    invalidate_source_control_load_request_state(
        &mut next_request_id,
        &mut active_request_id,
        &mut in_flight,
        &mut queued,
    );

    assert_eq!(next_request_id, 5);
    assert_eq!(active_request_id, 5);
    assert_eq!(in_flight, None);
    assert!(!queued);
    assert_eq!(
        begin_source_control_load_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        ),
        Some(6)
    );
}

#[test]
fn untrusted_workspace_rejects_stage_unstage_discard_and_commit_sinks() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = source_control_app_for_test(root, false);

    app.spawn_stage_changes(vec![path.clone()]);
    assert_restricted_status(&app, "staging changes");

    app.spawn_unstage_changes(vec![path.clone()]);
    assert_restricted_status(&app, "unstaging changes");

    app.spawn_discard_changes(vec![path]);
    assert_restricted_status(&app, "discarding changes");

    app.spawn_commit_changes("ship it".to_owned(), None, false);
    assert_restricted_status(&app, "committing changes");
}

#[test]
fn stale_stage_unstage_and_discard_paths_do_not_spawn_async_work() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = source_control_app_for_test(root, true);

    app.spawn_stage_changes(vec![path.clone()]);
    assert_eq!(app.status, no_unstaged_changes_status(&path));
    assert!(app.active_async_tasks.is_empty());
    assert!(app.async_task_trace.is_empty());

    app.status.clear();
    app.spawn_unstage_changes(vec![path.clone()]);
    assert_eq!(app.status, no_staged_changes_status(&path));
    assert!(app.active_async_tasks.is_empty());
    assert!(app.async_task_trace.is_empty());

    app.status.clear();
    app.spawn_discard_changes(vec![path.clone()]);
    assert_eq!(app.status, no_source_control_changes_status(&path));
    assert!(app.active_async_tasks.is_empty());
    assert!(app.async_task_trace.is_empty());
}

#[test]
fn stale_source_control_path_checks_short_circuit_and_use_bulk_statuses() {
    let first = PathBuf::from("workspace/src/first.rs");
    let second = PathBuf::from("workspace/src/second.rs");
    let third = PathBuf::from("workspace/src/third.rs");
    let paths = vec![first.clone(), second.clone(), third.clone()];
    let mut checked = Vec::new();

    let stale = first_stale_source_control_path(&paths, |path| {
        checked.push(path.to_path_buf());
        path != second.as_path()
    });

    assert_eq!(stale, Some(second.as_path()));
    assert_eq!(checked, vec![first, second.clone()]);
    assert_eq!(
        stale_source_control_stage_status(GitChangeStage::Unstaged, &paths, &second),
        "Source control selection changed; refresh before staging changes"
    );
    assert_eq!(
        stale_source_control_stage_status(GitChangeStage::Staged, &paths, &second),
        "Source control selection changed; refresh before unstaging changes"
    );
    assert_eq!(
        stale_source_control_discard_status(&paths, &second),
        "Source control selection changed; refresh before discarding changes"
    );
}

#[test]
fn source_control_operation_path_checks_reject_targets_outside_operation_root() {
    let root = PathBuf::from("workspace");
    let inside = root.join("src/main.rs");
    let outside = PathBuf::from("workspace-other/src/main.rs");
    let paths = vec![inside.clone(), outside.clone()];
    let mut checked = Vec::new();

    let stale = first_stale_source_control_operation_path(&paths, &root, |path| {
        checked.push(path.to_path_buf());
        true
    });

    assert_eq!(stale, Some(outside.as_path()));
    assert_eq!(checked, vec![inside]);
}

#[test]
fn source_control_operation_path_checks_reject_sibling_paths_for_child_repo_root() {
    let workspace = PathBuf::from("workspace");
    let repo_root = workspace.join("packages/app");
    let repo_path = repo_root.join("src/main.rs");
    let sibling_path = workspace.join("shared/lib.rs");
    let paths = vec![repo_path.clone(), sibling_path.clone()];
    let mut checked = Vec::new();

    let stale = first_stale_source_control_operation_path(&paths, &repo_root, |path| {
        checked.push(path.to_path_buf());
        true
    });

    assert_eq!(stale, Some(sibling_path.as_path()));
    assert_eq!(checked, vec![repo_path]);
}

#[test]
fn untrusted_workspace_rejects_commit_request_paths() {
    let mut app = source_control_app_for_test(PathBuf::from("workspace"), false);

    app.request_commit_changes("ship it".to_owned(), None, false);
    assert_restricted_status(&app, "committing changes");

    app.request_commit_changes_after_branch_protection("ship it".to_owned(), None, false);
    assert_restricted_status(&app, "committing changes");

    app.pending_source_control_commit_save = Some(PendingSourceControlCommitSave::Saving {
        request_id: 1,
        message: "ship it".to_owned(),
        smart_commit_changes: None,
        allow_empty: false,
        ids: vec![1],
    });
    app.advance_pending_source_control_commit_after_save();
    assert_restricted_status(&app, "committing changes");
    assert!(app.pending_source_control_commit_save.is_none());
}

#[test]
fn smart_commit_path_count_excludes_conflicted_entries() {
    let entries = vec![
        GitStatusEntry {
            path: PathBuf::from("src/tracked.rs"),
            status: GitFileStatus::Modified,
            stage: GitChangeStage::Unstaged,
        },
        GitStatusEntry {
            path: PathBuf::from("src/untracked.rs"),
            status: GitFileStatus::Untracked,
            stage: GitChangeStage::Unstaged,
        },
        GitStatusEntry {
            path: PathBuf::from("src/conflict.rs"),
            status: GitFileStatus::Conflicted,
            stage: GitChangeStage::Unstaged,
        },
        GitStatusEntry {
            path: PathBuf::from("src/added.rs"),
            status: GitFileStatus::Added,
            stage: GitChangeStage::Unstaged,
        },
        GitStatusEntry {
            path: PathBuf::from("src/staged.rs"),
            status: GitFileStatus::Modified,
            stage: GitChangeStage::Staged,
        },
        GitStatusEntry {
            path: PathBuf::from("src/tracked.rs"),
            status: GitFileStatus::Modified,
            stage: GitChangeStage::Unstaged,
        },
    ];
    let conflicts_only = vec![GitStatusEntry {
        path: PathBuf::from("src/conflict.rs"),
        status: GitFileStatus::Conflicted,
        stage: GitChangeStage::Unstaged,
    }];

    assert_eq!(
        smart_commit_path_count(&entries, GitSmartCommitChanges::All),
        2
    );
    assert_eq!(
        smart_commit_path_count(&entries, GitSmartCommitChanges::Tracked),
        1
    );
    assert_eq!(
        smart_commit_path_count(&conflicts_only, GitSmartCommitChanges::All),
        0
    );
    assert_eq!(
        smart_commit_path_count(&conflicts_only, GitSmartCommitChanges::Tracked),
        0
    );
}

#[test]
fn source_control_has_stage_detects_any_matching_entry() {
    let entries = vec![
        GitStatusEntry {
            path: PathBuf::from("src/unstaged.rs"),
            status: GitFileStatus::Modified,
            stage: GitChangeStage::Unstaged,
        },
        GitStatusEntry {
            path: PathBuf::from("src/staged.rs"),
            status: GitFileStatus::Modified,
            stage: GitChangeStage::Staged,
        },
    ];

    assert!(source_control_has_stage(&entries, GitChangeStage::Staged));
    assert!(source_control_has_stage(&entries, GitChangeStage::Unstaged));
    assert!(!source_control_has_stage(&[], GitChangeStage::Staged));
}

#[test]
fn source_control_stage_paths_deduplicate_without_losing_order() {
    let first_staged = PathBuf::from("src/z_staged.rs");
    let second_staged = PathBuf::from("src/a_staged.rs");
    let entries = vec![
        GitStatusEntry {
            path: PathBuf::from("src/unstaged.rs"),
            status: GitFileStatus::Modified,
            stage: GitChangeStage::Unstaged,
        },
        GitStatusEntry {
            path: first_staged.clone(),
            status: GitFileStatus::Modified,
            stage: GitChangeStage::Staged,
        },
        GitStatusEntry {
            path: second_staged.clone(),
            status: GitFileStatus::Modified,
            stage: GitChangeStage::Staged,
        },
        GitStatusEntry {
            path: first_staged.clone(),
            status: GitFileStatus::Conflicted,
            stage: GitChangeStage::Staged,
        },
    ];

    assert_eq!(
        source_control_paths_for_stage(&entries, GitChangeStage::Staged),
        vec![first_staged, second_staged]
    );
    assert_eq!(
        source_control_stage_path_count(&entries, GitChangeStage::Staged),
        2
    );
}

#[test]
fn pending_commit_waits_for_pending_format_on_save() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = source_control_app_for_test(root, true);
    insert_dirty_pending_format_on_save(&mut app, 7, path);
    app.pending_source_control_commit_save = Some(PendingSourceControlCommitSave::Saving {
        request_id: 1,
        message: "ship it".to_owned(),
        smart_commit_changes: None,
        allow_empty: false,
        ids: vec![7],
    });

    app.advance_pending_source_control_commit_after_save();

    assert!(matches!(
        app.pending_source_control_commit_save,
        Some(PendingSourceControlCommitSave::Saving { ids, .. }) if ids == vec![7]
    ));
    assert!(!app.status.starts_with("Commit paused;"));
    assert!(app.active_async_tasks.is_empty());
    assert!(app.async_task_trace.is_empty());
}

#[test]
fn pending_commit_pauses_on_clean_external_change_after_save() {
    let root = PathBuf::from("workspace");
    let main_path = root.join("src/main.rs");
    let lib_path = root.join("src/lib.rs");
    let mut app = source_control_app_for_test(root, true);
    app.buffers.push(TextBuffer::from_text(
        7,
        Some(main_path),
        "fn main() {}\n".to_owned(),
    ));
    app.buffers.push(TextBuffer::from_text(
        8,
        Some(lib_path),
        "pub fn helper() {}\n".to_owned(),
    ));
    app.external_change_buffers.insert(7);
    app.pending_source_control_commit_save = Some(PendingSourceControlCommitSave::Saving {
        request_id: 1,
        message: "ship it".to_owned(),
        smart_commit_changes: Some(GitSmartCommitChanges::All),
        allow_empty: true,
        ids: vec![8, 7],
    });

    app.advance_pending_source_control_commit_after_save();

    assert!(matches!(
        app.pending_source_control_commit_save,
        Some(PendingSourceControlCommitSave::Confirm {
            ref message,
            smart_commit_changes: Some(GitSmartCommitChanges::All),
            allow_empty: true,
            ref ids,
            ..
        }) if message == "ship it" && ids == &vec![8, 7]
    ));
    assert_eq!(app.status, "Commit paused; 1 file changed on disk");
    assert!(app.active_async_tasks.is_empty());
    assert!(app.async_task_trace.is_empty());
}

#[test]
fn pending_commit_pauses_on_clean_pending_reload_after_save() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = source_control_app_for_test(root, true);
    let buffer = TextBuffer::from_text(7, Some(path.clone()), "fn main() {}\n".to_owned());
    let version = buffer.version();
    app.buffers.push(buffer);
    app.in_flight_reloads.insert(
        7,
        PendingFileReload {
            request_id: 1,
            path,
            version,
            force_dirty: false,
        },
    );
    app.pending_source_control_commit_save = Some(PendingSourceControlCommitSave::Saving {
        request_id: 1,
        message: "ship it".to_owned(),
        smart_commit_changes: Some(GitSmartCommitChanges::Tracked),
        allow_empty: false,
        ids: vec![7],
    });

    app.advance_pending_source_control_commit_after_save();

    assert!(app.external_change_buffers.is_empty());
    assert!(matches!(
        app.pending_source_control_commit_save,
        Some(PendingSourceControlCommitSave::Confirm {
            ref message,
            smart_commit_changes: Some(GitSmartCommitChanges::Tracked),
            allow_empty: false,
            ref ids,
            ..
        }) if message == "ship it" && ids == &vec![7]
    ));
    assert_eq!(app.status, "Commit paused; 1 file changed on disk");
    assert!(app.active_async_tasks.is_empty());
    assert!(app.async_task_trace.is_empty());
}

#[test]
fn pending_commit_pauses_on_queued_clean_reload_after_save() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = source_control_app_for_test(root, true);
    app.buffers.push(TextBuffer::from_text(
        7,
        Some(path.clone()),
        "fn main() {}\n".to_owned(),
    ));
    app.queued_file_reloads.insert(
        7,
        QueuedFileReload {
            path,
            force_dirty: false,
        },
    );
    app.pending_source_control_commit_save = Some(PendingSourceControlCommitSave::Saving {
        request_id: 1,
        message: "ship it".to_owned(),
        smart_commit_changes: Some(GitSmartCommitChanges::Tracked),
        allow_empty: false,
        ids: vec![7],
    });

    app.advance_pending_source_control_commit_after_save();

    assert!(app.external_change_buffers.is_empty());
    assert!(matches!(
        app.pending_source_control_commit_save,
        Some(PendingSourceControlCommitSave::Confirm {
            ref message,
            smart_commit_changes: Some(GitSmartCommitChanges::Tracked),
            allow_empty: false,
            ref ids,
            ..
        }) if message == "ship it" && ids == &vec![7]
    ));
    assert_eq!(app.status, "Commit paused; 1 file changed on disk");
    assert!(app.active_async_tasks.is_empty());
    assert!(app.async_task_trace.is_empty());
}

#[test]
fn commit_request_sinks_reject_empty_messages() {
    let mut app = source_control_app_for_test(PathBuf::from("workspace"), true);

    app.source_control_commit_message = " \n\t ".to_owned();
    app.commit_staged_changes();
    assert_eq!(app.status, SOURCE_CONTROL_EMPTY_COMMIT_MESSAGE_STATUS);

    app.status.clear();
    app.request_commit_changes(" \n ".to_owned(), Some(GitSmartCommitChanges::All), false);
    assert_eq!(app.status, SOURCE_CONTROL_EMPTY_COMMIT_MESSAGE_STATUS);
    assert!(app.pending_source_control_protected_branch_commit.is_none());
    assert!(app.pending_source_control_commit_save.is_none());

    app.status.clear();
    app.request_commit_changes_after_branch_protection(" \t ".to_owned(), None, true);
    assert_eq!(app.status, SOURCE_CONTROL_EMPTY_COMMIT_MESSAGE_STATUS);
    assert!(app.pending_source_control_commit_save.is_none());

    app.status.clear();
    app.spawn_commit_changes("\n".to_owned(), None, false);
    assert_eq!(app.status, SOURCE_CONTROL_EMPTY_COMMIT_MESSAGE_STATUS);
}

#[test]
fn pending_commit_prompts_reject_empty_messages() {
    let mut app = source_control_app_for_test(PathBuf::from("workspace"), true);

    app.begin_source_control_smart_commit_suggestion(
        1,
        " \t ".to_owned(),
        GitSmartCommitChanges::All,
        2,
    );
    assert_eq!(app.status, SOURCE_CONTROL_EMPTY_COMMIT_MESSAGE_STATUS);
    assert!(app.pending_source_control_smart_commit.is_none());

    app.status.clear();
    app.begin_source_control_empty_commit_confirmation(2, "\n".to_owned());
    assert_eq!(app.status, SOURCE_CONTROL_EMPTY_COMMIT_MESSAGE_STATUS);
    assert!(app.pending_source_control_empty_commit.is_none());

    app.status.clear();
    app.begin_source_control_protected_branch_commit_prompt(
        3,
        " ".to_owned(),
        None,
        false,
        "main".to_owned(),
        "main".to_owned(),
    );
    assert_eq!(app.status, SOURCE_CONTROL_EMPTY_COMMIT_MESSAGE_STATUS);
    assert!(app.pending_source_control_protected_branch_commit.is_none());

    app.status.clear();
    app.begin_source_control_commit_save_prompt(4, " \r\n ".to_owned(), None, false, vec![1]);
    assert_eq!(app.status, SOURCE_CONTROL_EMPTY_COMMIT_MESSAGE_STATUS);
    assert!(app.pending_source_control_commit_save.is_none());
}
