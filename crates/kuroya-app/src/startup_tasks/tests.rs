use super::{
    GitScanRootCacheEntry, PendingWorkspaceRefresh, WORKSPACE_PLUGIN_RELOAD_DEBOUNCE,
    WORKSPACE_REFRESH_DEBOUNCE, WORKSPACE_REFRESH_MAX_WAIT, begin_git_scan_request_state,
    begin_workspace_index_request_state, begin_workspace_plugin_discovery_request_state,
    cached_git_scan_root_for_auto_repository_detection, finish_git_scan_request_state,
    finish_workspace_index_request_state, finish_workspace_plugin_discovery_request_state,
    git_auto_refresh_enabled, git_open_parent_repositories, git_repository_ignored,
    git_repository_in_subfolders_with_limits, git_repository_marker_exists,
    git_repository_scan_children, git_repository_scan_folder_ignored,
    git_scan_root_for_auto_repository_detection, invalidate_git_scan_request_state,
    invalidate_startup_task_request_state, invalidate_workspace_index_request_state,
    invalidate_workspace_plugin_discovery_request_state, next_startup_task_request_id,
    resolved_cached_git_scan_root_for_auto_repository_detection, workspace_plugin_reload_due,
    workspace_plugins_enabled, workspace_plugins_restricted_status, workspace_refresh_due,
};
use crate::ui_events::UiEvent;
use kuroya_core::{
    GitAutoRepositoryDetection, GitOpenRepositoryInParentFolders, ProjectIndex, ProjectSearchIndex,
};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

#[test]
fn workspace_plugin_reload_waits_for_debounce_window() {
    let scheduled = std::time::Instant::now();

    assert!(!workspace_plugin_reload_due(
        None,
        scheduled + WORKSPACE_PLUGIN_RELOAD_DEBOUNCE,
        WORKSPACE_PLUGIN_RELOAD_DEBOUNCE
    ));
    assert!(!workspace_plugin_reload_due(
        Some(scheduled),
        scheduled + WORKSPACE_PLUGIN_RELOAD_DEBOUNCE - Duration::from_millis(1),
        WORKSPACE_PLUGIN_RELOAD_DEBOUNCE
    ));
    assert!(workspace_plugin_reload_due(
        Some(scheduled),
        scheduled + WORKSPACE_PLUGIN_RELOAD_DEBOUNCE,
        WORKSPACE_PLUGIN_RELOAD_DEBOUNCE
    ));
}

#[test]
fn workspace_refresh_waits_for_debounce_window() {
    let scheduled = std::time::Instant::now();
    let pending = PendingWorkspaceRefresh::new(scheduled);

    assert!(!workspace_refresh_due(
        None,
        scheduled + WORKSPACE_REFRESH_DEBOUNCE,
        WORKSPACE_REFRESH_DEBOUNCE,
        WORKSPACE_REFRESH_MAX_WAIT
    ));
    assert!(!workspace_refresh_due(
        Some(pending),
        scheduled + WORKSPACE_REFRESH_DEBOUNCE - Duration::from_millis(1),
        WORKSPACE_REFRESH_DEBOUNCE,
        WORKSPACE_REFRESH_MAX_WAIT
    ));
    assert!(workspace_refresh_due(
        Some(pending),
        scheduled + WORKSPACE_REFRESH_DEBOUNCE,
        WORKSPACE_REFRESH_DEBOUNCE,
        WORKSPACE_REFRESH_MAX_WAIT
    ));
}

#[test]
fn workspace_refresh_max_wait_prevents_debounce_starvation() {
    let scheduled = std::time::Instant::now();
    let mut pending = PendingWorkspaceRefresh::new(scheduled);
    pending.record_change(scheduled + WORKSPACE_REFRESH_MAX_WAIT - Duration::from_millis(1));

    assert!(workspace_refresh_due(
        Some(pending),
        scheduled + WORKSPACE_REFRESH_MAX_WAIT,
        WORKSPACE_REFRESH_DEBOUNCE,
        WORKSPACE_REFRESH_MAX_WAIT
    ));
}

#[test]
fn workspace_index_request_starts_when_idle() {
    let mut next_request_id = 0;
    let mut active_request_id = 0;
    let mut in_flight = None;
    let mut queued = false;

    assert_eq!(
        begin_workspace_index_request_state(
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
fn workspace_index_event_uses_immediate_empty_project_search_index() {
    let root = temp_workspace("broad-search-index");
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/main.rs"), "fn indexed() {}\n").unwrap();
    let index = ProjectIndex::rebuild(&root, 40_000);

    assert_eq!(ProjectSearchIndex::default().len(), 0);
    assert_eq!(index.files().len(), 1);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn startup_task_request_ids_wrap_without_zero() {
    assert_eq!(next_startup_task_request_id(0), 1);
    assert_eq!(next_startup_task_request_id(41), 42);
    assert_eq!(next_startup_task_request_id(u64::MAX - 1), u64::MAX);
    assert_eq!(next_startup_task_request_id(u64::MAX), 1);
}

#[test]
fn workspace_index_request_queues_once_while_in_flight() {
    let mut next_request_id = 0;
    let mut active_request_id = 0;
    let mut in_flight = None;
    let mut queued = false;

    assert_eq!(
        begin_workspace_index_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        ),
        Some(1)
    );
    assert_eq!(
        begin_workspace_index_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        ),
        None
    );
    assert_eq!(
        begin_workspace_index_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        ),
        None
    );

    assert_eq!(next_request_id, 2);
    assert_eq!(active_request_id, 2);
    assert_eq!(in_flight, Some(1));
    assert!(queued);
}

#[test]
fn startup_task_queued_request_does_not_reuse_saturated_in_flight_id() {
    let mut next_request_id = u64::MAX - 1;
    let mut active_request_id = u64::MAX - 1;
    let mut in_flight = None;
    let mut queued = false;

    assert_eq!(
        begin_workspace_index_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        ),
        Some(u64::MAX)
    );
    assert_eq!(
        begin_workspace_index_request_state(
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

    assert!(finish_workspace_index_request_state(
        &mut in_flight,
        &mut queued,
        u64::MAX,
    ));
    assert_eq!(
        begin_workspace_index_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        ),
        Some(2)
    );
    assert_eq!(active_request_id, 2);
    assert_eq!(in_flight, Some(2));
}

#[test]
fn workspace_index_finish_drains_queued_refresh_once() {
    let mut next_request_id = 0;
    let mut active_request_id = 0;
    let mut in_flight = None;
    let mut queued = false;

    assert_eq!(
        begin_workspace_index_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        ),
        Some(1)
    );
    assert_eq!(
        begin_workspace_index_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        ),
        None
    );

    assert!(finish_workspace_index_request_state(
        &mut in_flight,
        &mut queued,
        1
    ));
    assert_eq!(in_flight, None);
    assert!(!queued);
    assert_eq!(
        begin_workspace_index_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        ),
        Some(3)
    );
}

#[test]
fn startup_task_invalidation_does_not_reuse_saturated_in_flight_id() {
    let mut next_request_id = u64::MAX;
    let mut active_request_id = u64::MAX;
    let mut in_flight = Some(u64::MAX);
    let mut queued = true;

    invalidate_startup_task_request_state(
        &mut next_request_id,
        &mut active_request_id,
        &mut in_flight,
        &mut queued,
    );

    assert_eq!(next_request_id, 1);
    assert_eq!(active_request_id, 1);
    assert_eq!(in_flight, None);
    assert!(!queued);
    assert_eq!(
        begin_workspace_plugin_discovery_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        ),
        Some(2)
    );
}

#[test]
fn workspace_index_finish_ignores_unrelated_request_id() {
    let mut in_flight = Some(4);
    let mut queued = true;

    assert!(!finish_workspace_index_request_state(
        &mut in_flight,
        &mut queued,
        3
    ));

    assert_eq!(in_flight, Some(4));
    assert!(queued);
}

#[test]
fn workspace_index_invalidation_keeps_request_ids_monotonic() {
    let mut next_request_id = 4;
    let mut active_request_id = 4;
    let mut in_flight = Some(4);
    let mut queued = true;

    invalidate_workspace_index_request_state(
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
        begin_workspace_index_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        ),
        Some(6)
    );
}

#[test]
fn git_scan_request_starts_when_idle() {
    let mut next_request_id = 0;
    let mut active_request_id = 0;
    let mut in_flight = None;
    let mut queued = false;

    assert_eq!(
        begin_git_scan_request_state(
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
fn git_scan_request_queues_once_while_in_flight() {
    let mut next_request_id = 0;
    let mut active_request_id = 0;
    let mut in_flight = None;
    let mut queued = false;

    assert_eq!(
        begin_git_scan_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        ),
        Some(1)
    );
    assert_eq!(
        begin_git_scan_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        ),
        None
    );
    assert_eq!(
        begin_git_scan_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        ),
        None
    );

    assert_eq!(next_request_id, 2);
    assert_eq!(active_request_id, 2);
    assert_eq!(in_flight, Some(1));
    assert!(queued);
}

#[test]
fn git_scan_finish_drains_queued_refresh_once() {
    let mut next_request_id = 0;
    let mut active_request_id = 0;
    let mut in_flight = None;
    let mut queued = false;

    assert_eq!(
        begin_git_scan_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        ),
        Some(1)
    );
    assert_eq!(
        begin_git_scan_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        ),
        None
    );

    assert!(finish_git_scan_request_state(
        &mut in_flight,
        &mut queued,
        1
    ));
    assert_eq!(in_flight, None);
    assert!(!queued);
    assert_eq!(
        begin_git_scan_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        ),
        Some(3)
    );
}

#[test]
fn git_scan_finish_ignores_unrelated_request_id() {
    let mut in_flight = Some(4);
    let mut queued = true;

    assert!(!finish_git_scan_request_state(
        &mut in_flight,
        &mut queued,
        3
    ));

    assert_eq!(in_flight, Some(4));
    assert!(queued);
}

#[test]
fn git_scan_invalidation_keeps_request_ids_monotonic() {
    let mut next_request_id = 4;
    let mut active_request_id = 4;
    let mut in_flight = Some(4);
    let mut queued = true;

    invalidate_git_scan_request_state(
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
        begin_git_scan_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        ),
        Some(6)
    );
}

#[test]
fn workspace_plugin_discovery_request_starts_when_idle() {
    let mut next_request_id = 0;
    let mut active_request_id = 0;
    let mut in_flight = None;
    let mut queued = false;

    assert_eq!(
        begin_workspace_plugin_discovery_request_state(
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
fn workspace_plugin_discovery_request_queues_once_while_in_flight() {
    let mut next_request_id = 0;
    let mut active_request_id = 0;
    let mut in_flight = None;
    let mut queued = false;

    assert_eq!(
        begin_workspace_plugin_discovery_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        ),
        Some(1)
    );
    assert_eq!(
        begin_workspace_plugin_discovery_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        ),
        None
    );
    assert_eq!(
        begin_workspace_plugin_discovery_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        ),
        None
    );

    assert_eq!(next_request_id, 2);
    assert_eq!(active_request_id, 2);
    assert_eq!(in_flight, Some(1));
    assert!(queued);
}

#[test]
fn workspace_plugin_discovery_finish_drains_queued_reload_once() {
    let mut next_request_id = 0;
    let mut active_request_id = 0;
    let mut in_flight = None;
    let mut queued = false;

    assert_eq!(
        begin_workspace_plugin_discovery_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        ),
        Some(1)
    );
    assert_eq!(
        begin_workspace_plugin_discovery_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        ),
        None
    );

    assert!(finish_workspace_plugin_discovery_request_state(
        &mut in_flight,
        &mut queued,
        1
    ));
    assert_eq!(in_flight, None);
    assert!(!queued);
    assert_eq!(
        begin_workspace_plugin_discovery_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        ),
        Some(3)
    );
}

#[test]
fn workspace_plugin_discovery_finish_ignores_unrelated_request_id() {
    let mut in_flight = Some(4);
    let mut queued = true;

    assert!(!finish_workspace_plugin_discovery_request_state(
        &mut in_flight,
        &mut queued,
        3
    ));

    assert_eq!(in_flight, Some(4));
    assert!(queued);
}

#[test]
fn workspace_plugin_discovery_invalidation_keeps_request_ids_monotonic() {
    let mut next_request_id = 4;
    let mut active_request_id = 4;
    let mut in_flight = Some(4);
    let mut queued = true;

    invalidate_workspace_plugin_discovery_request_state(
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
        begin_workspace_plugin_discovery_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        ),
        Some(6)
    );
}

#[test]
fn git_auto_refresh_requires_git_and_autorefresh() {
    assert!(git_auto_refresh_enabled(true, true));
    assert!(!git_auto_refresh_enabled(false, true));
    assert!(!git_auto_refresh_enabled(true, false));
    assert!(!git_auto_refresh_enabled(false, false));
}

#[test]
fn git_parent_repository_policy_matches_vs_code_setting_values() {
    assert!(git_open_parent_repositories(
        GitOpenRepositoryInParentFolders::Always
    ));
    assert!(git_open_parent_repositories(
        GitOpenRepositoryInParentFolders::Prompt
    ));
    assert!(!git_open_parent_repositories(
        GitOpenRepositoryInParentFolders::Never
    ));
}

#[test]
fn git_auto_repository_detection_selects_workspace_subfolder_or_open_editor_repo() {
    let root = std::env::temp_dir()
        .join(format!("kuroya-auto-repo-{}", std::process::id()))
        .join("workspace");
    let repo = root.join("packages").join("app");
    let source = repo.join("src").join("main.rs");
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    std::fs::create_dir_all(source.parent().unwrap()).unwrap();
    std::fs::write(&source, "fn main() {}\n").unwrap();

    assert_eq!(
        git_scan_root_for_auto_repository_detection(
            &root,
            GitAutoRepositoryDetection::False,
            1,
            &[],
            &[]
        ),
        None
    );
    assert_eq!(
        git_scan_root_for_auto_repository_detection(
            &root,
            GitAutoRepositoryDetection::True,
            1,
            &[],
            &[]
        ),
        Some(root.clone())
    );
    assert_eq!(
        git_scan_root_for_auto_repository_detection(
            &root,
            GitAutoRepositoryDetection::SubFolders,
            1,
            &[],
            &[]
        ),
        None
    );
    assert_eq!(
        git_scan_root_for_auto_repository_detection(
            &root,
            GitAutoRepositoryDetection::SubFolders,
            2,
            &[],
            &[]
        ),
        Some(repo.clone())
    );
    assert_eq!(
        git_scan_root_for_auto_repository_detection(
            &root,
            GitAutoRepositoryDetection::SubFolders,
            2,
            &["packages".to_owned()],
            &[]
        ),
        None
    );
    assert_eq!(
        git_scan_root_for_auto_repository_detection(
            &root,
            GitAutoRepositoryDetection::OpenEditors,
            1,
            &[],
            std::slice::from_ref(&source),
        ),
        Some(repo)
    );
    assert_eq!(
        git_scan_root_for_auto_repository_detection(
            &root,
            GitAutoRepositoryDetection::OpenEditors,
            1,
            &["packages".to_owned()],
            std::slice::from_ref(&source),
        ),
        None
    );

    std::fs::remove_dir_all(root.parent().unwrap()).unwrap();
}

#[test]
fn git_open_editor_detection_ignores_paths_outside_workspace() {
    let base =
        std::env::temp_dir().join(format!("kuroya-open-editor-outside-{}", std::process::id()));
    let root = base.join("workspace");
    let outside_repo = base.join("outside_repo");
    let source = outside_repo.join("src").join("main.rs");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(outside_repo.join(".git")).unwrap();
    std::fs::create_dir_all(source.parent().unwrap()).unwrap();
    std::fs::write(&source, "fn main() {}\n").unwrap();

    assert_eq!(
        git_scan_root_for_auto_repository_detection(
            &root,
            GitAutoRepositoryDetection::OpenEditors,
            1,
            &[],
            std::slice::from_ref(&source),
        ),
        None
    );

    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn git_repository_marker_accepts_directory_and_file_markers() {
    let root = temp_workspace("kuroya-git-marker-kind");
    fs::create_dir_all(&root).unwrap();

    assert!(!git_repository_marker_exists(&root));

    fs::create_dir_all(root.join(".git")).unwrap();
    assert!(git_repository_marker_exists(&root));

    fs::remove_dir_all(root.join(".git")).unwrap();
    fs::write(root.join(".git"), "gitdir: ../actual.git\n").unwrap();
    assert!(git_repository_marker_exists(&root));

    fs::remove_dir_all(root.parent().unwrap()).unwrap();
}

#[test]
fn git_subfolder_scan_detects_worktree_file_marker() {
    let root = temp_workspace("kuroya-auto-repo-worktree-file");
    let repo = root.join("packages").join("app");
    fs::create_dir_all(&repo).unwrap();
    fs::write(repo.join(".git"), "gitdir: ../../.git/worktrees/app\n").unwrap();

    assert_eq!(
        git_scan_root_for_auto_repository_detection(
            &root,
            GitAutoRepositoryDetection::SubFolders,
            2,
            &[],
            &[]
        ),
        Some(repo)
    );

    fs::remove_dir_all(root.parent().unwrap()).unwrap();
}

#[test]
fn git_subfolder_scan_children_are_bounded_and_sorted() {
    let root = temp_workspace("kuroya-auto-repo-children");
    fs::create_dir_all(root.join("b")).unwrap();
    fs::create_dir_all(root.join("a")).unwrap();
    fs::create_dir_all(root.join("c")).unwrap();
    fs::write(root.join("file.txt"), "").unwrap();

    let children = git_repository_scan_children(&root, 2);

    assert_eq!(children.len(), 2);
    assert!(children.iter().all(|path| path.is_dir()));
    assert!(children.windows(2).all(|pair| pair[0] <= pair[1]));

    fs::remove_dir_all(root.parent().unwrap()).unwrap();
}

#[test]
fn git_subfolder_scan_children_follow_symlinked_directories() {
    let root = temp_workspace("kuroya-auto-repo-symlink-child");
    let target = root.join("target");
    let link = root.join("linked");
    fs::create_dir_all(&target).unwrap();
    if create_directory_symlink(&target, &link).is_err() {
        fs::remove_dir_all(root.parent().unwrap()).unwrap();
        return;
    }

    let children = git_repository_scan_children(&root, 8);

    assert!(children.contains(&link));

    fs::remove_dir_all(root.parent().unwrap()).unwrap();
}

#[test]
fn git_subfolder_scan_respects_visited_folder_limit() {
    let root = temp_workspace("kuroya-auto-repo-visited");
    let nested_repo = root.join("a").join("nested");
    fs::create_dir_all(nested_repo.join(".git")).unwrap();

    assert_eq!(
        git_repository_in_subfolders_with_limits(&root, 3, &[], 16, 1),
        None
    );
    assert_eq!(
        git_repository_in_subfolders_with_limits(&root, 3, &[], 16, 4),
        Some(nested_repo)
    );

    fs::remove_dir_all(root.parent().unwrap()).unwrap();
}

#[test]
fn git_scan_root_cache_reuses_detected_subfolder_repo_until_key_changes() {
    let root = temp_workspace("kuroya-auto-repo-cache");
    let first_repo = root.join("b_repo");
    let earlier_repo = root.join("a_repo");
    fs::create_dir_all(first_repo.join(".git")).unwrap();

    let mut cache = None;
    assert_eq!(
        cached_git_scan_root_for_auto_repository_detection(
            &mut cache,
            &root,
            GitAutoRepositoryDetection::SubFolders,
            2,
            &[],
            &[]
        ),
        Some(first_repo.clone())
    );

    fs::create_dir_all(earlier_repo.join(".git")).unwrap();
    assert_eq!(
        git_scan_root_for_auto_repository_detection(
            &root,
            GitAutoRepositoryDetection::SubFolders,
            2,
            &[],
            &[]
        ),
        Some(earlier_repo.clone())
    );
    assert_eq!(
        cached_git_scan_root_for_auto_repository_detection(
            &mut cache,
            &root,
            GitAutoRepositoryDetection::SubFolders,
            2,
            &[],
            &[]
        ),
        Some(first_repo)
    );

    assert_eq!(
        cached_git_scan_root_for_auto_repository_detection(
            &mut cache,
            &root,
            GitAutoRepositoryDetection::SubFolders,
            2,
            &["b_repo".to_owned()],
            &[]
        ),
        Some(earlier_repo)
    );

    fs::remove_dir_all(root.parent().unwrap()).unwrap();
}

#[test]
fn git_scan_root_cache_recomputes_when_cached_marker_disappears() {
    let root = temp_workspace("kuroya-auto-repo-cache-stale");
    let first_repo = root.join("a_repo");
    let second_repo = root.join("b_repo");
    fs::create_dir_all(first_repo.join(".git")).unwrap();
    fs::create_dir_all(second_repo.join(".git")).unwrap();

    let mut cache = None;
    assert_eq!(
        cached_git_scan_root_for_auto_repository_detection(
            &mut cache,
            &root,
            GitAutoRepositoryDetection::SubFolders,
            2,
            &[],
            &[]
        ),
        Some(first_repo.clone())
    );

    fs::remove_dir_all(first_repo.join(".git")).unwrap();
    assert_eq!(
        cached_git_scan_root_for_auto_repository_detection(
            &mut cache,
            &root,
            GitAutoRepositoryDetection::SubFolders,
            2,
            &[],
            &[]
        ),
        Some(second_repo)
    );

    fs::remove_dir_all(root.parent().unwrap()).unwrap();
}

#[test]
fn git_scan_root_cache_tracks_open_editor_inputs() {
    let root = temp_workspace("kuroya-auto-repo-open-editor-cache");
    let app_repo = root.join("packages").join("app");
    let lib_repo = root.join("packages").join("lib");
    let app_source = app_repo.join("src").join("main.rs");
    let lib_source = lib_repo.join("src").join("lib.rs");
    fs::create_dir_all(app_repo.join(".git")).unwrap();
    fs::create_dir_all(lib_repo.join(".git")).unwrap();
    fs::create_dir_all(app_source.parent().unwrap()).unwrap();
    fs::create_dir_all(lib_source.parent().unwrap()).unwrap();
    fs::write(&app_source, "").unwrap();
    fs::write(&lib_source, "").unwrap();

    let mut cache = None;
    assert_eq!(
        cached_git_scan_root_for_auto_repository_detection(
            &mut cache,
            &root,
            GitAutoRepositoryDetection::OpenEditors,
            2,
            &[],
            std::slice::from_ref(&app_source),
        ),
        Some(app_repo)
    );
    assert_eq!(
        cached_git_scan_root_for_auto_repository_detection(
            &mut cache,
            &root,
            GitAutoRepositoryDetection::OpenEditors,
            2,
            &[],
            std::slice::from_ref(&lib_source),
        ),
        Some(lib_repo)
    );

    fs::remove_dir_all(root.parent().unwrap()).unwrap();
}

#[test]
fn spawn_git_scan_defers_auto_repository_discovery_and_cache_update() {
    let root = temp_workspace("kuroya-auto-repo-deferred-scan");
    let repo = root.join("packages").join("app");
    fs::create_dir_all(repo.join(".git")).unwrap();

    {
        let mut app =
            crate::source_control_runtime::source_control_app_for_test(root.clone(), true);
        app.settings.git_auto_repository_detection = GitAutoRepositoryDetection::SubFolders;
        app.settings.git_repository_scan_max_depth = 2;

        assert!(app.spawn_git_scan());
        assert_eq!(app.git_scan_active_request_id, 1);
        assert_eq!(app.git_scan_in_flight_request_id, Some(1));
        assert!(app.git_scan_root_cache.is_none());
    }

    fs::remove_dir_all(root.parent().unwrap()).unwrap();
}

#[test]
fn startup_tasks_skip_placeholder_workspace() {
    let root = temp_workspace("kuroya-placeholder-startup-tasks");
    fs::create_dir_all(&root).unwrap();
    let mut app = crate::source_control_runtime::source_control_app_for_test(root.clone(), true);
    app.workspace_placeholder = true;

    app.spawn_index();

    assert_eq!(app.workspace_index_in_flight_request_id, None);
    assert!(!app.spawn_git_scan());
    assert_eq!(app.git_scan_in_flight_request_id, None);
    assert!(!app.spawn_plugin_discovery());
    assert_eq!(app.workspace_plugins_in_flight_request_id, None);
    assert_eq!(app.status, "No folder open");

    fs::remove_dir_all(root.parent().unwrap()).unwrap();
}

#[test]
fn current_git_scan_completion_applies_worker_root_cache_entry() {
    let root = temp_workspace("kuroya-auto-repo-worker-cache");
    let repo = root.join("packages").join("app");
    fs::create_dir_all(repo.join(".git")).unwrap();

    let (scan_root, cache_entry) = resolved_cached_git_scan_root_for_auto_repository_detection(
        None,
        &root,
        GitAutoRepositoryDetection::SubFolders,
        2,
        &[],
        &[],
    );
    assert_eq!(scan_root, Some(repo.clone()));
    let cache_entry = cache_entry.expect("subfolder scan should create a cache entry");

    let mut app = crate::source_control_runtime::source_control_app_for_test(root.clone(), true);
    app.settings.git_auto_repository_detection = GitAutoRepositoryDetection::SubFolders;
    app.git_scan_next_request_id = 1;
    app.git_scan_active_request_id = 1;
    app.git_scan_in_flight_request_id = Some(1);

    assert!(crate::ui_event_channel::send_ui_event(
        &app.tx,
        UiEvent::GitScanned {
            request_id: 1,
            root: root.clone(),
            scan_root: Some(repo),
            root_cache_entry: Some(cache_entry.clone()),
            git: kuroya_core::GitSnapshot::default(),
        }
    ));

    assert_eq!(app.handle_events(), 1);
    assert_eq!(app.git_scan_root_cache, Some(cache_entry));

    fs::remove_dir_all(root.parent().unwrap()).unwrap();
}

#[test]
fn active_git_scan_without_resolved_root_clears_git_cache_and_selection() {
    let root = temp_workspace("kuroya-auto-repo-no-root");
    fs::create_dir_all(&root).unwrap();

    let mut app = crate::source_control_runtime::source_control_app_for_test(root.clone(), true);
    app.git_scan_next_request_id = 1;
    app.git_scan_active_request_id = 1;
    app.git_scan_in_flight_request_id = Some(1);
    app.git_scan_root_cache = Some(GitScanRootCacheEntry {
        key: super::git_scan_root_cache_key(
            &root,
            GitAutoRepositoryDetection::SubFolders,
            2,
            &[],
            &[],
        ),
        scan_root: root.join("old-repo"),
    });
    app.source_control_selected = 3;

    assert!(crate::ui_event_channel::send_ui_event(
        &app.tx,
        UiEvent::GitScanned {
            request_id: 1,
            root: root.clone(),
            scan_root: None,
            root_cache_entry: None,
            git: kuroya_core::GitSnapshot::default(),
        }
    ));

    assert_eq!(app.handle_events(), 1);
    assert_eq!(app.git_scan_root_cache, None);
    assert_eq!(app.source_control_selected, 0);

    fs::remove_dir_all(root.parent().unwrap()).unwrap();
}

#[test]
fn git_repository_ignored_matches_absolute_relative_and_name_entries() {
    let root = std::env::temp_dir()
        .join(format!("kuroya-ignore-repo-{}", std::process::id()))
        .join("workspace");
    std::fs::create_dir_all(&root).unwrap();

    assert!(git_repository_ignored(&root, &[root.display().to_string()]));
    assert!(git_repository_ignored(&root, &[".".to_owned()]));
    assert!(git_repository_ignored(&root, &["workspace".to_owned()]));
    assert!(!git_repository_ignored(&root, &["other".to_owned()]));

    std::fs::remove_dir_all(root.parent().unwrap()).unwrap();
}

#[test]
fn git_repository_scan_ignored_folders_match_named_relative_and_absolute_folders() {
    let root = std::env::temp_dir()
        .join(format!("kuroya-scan-ignore-{}", std::process::id()))
        .join("workspace")
        .join("node_modules")
        .join("pkg");
    std::fs::create_dir_all(&root).unwrap();

    assert!(git_repository_scan_folder_ignored(
        &root,
        &["node_modules".to_owned()]
    ));
    assert!(git_repository_scan_folder_ignored(
        &root,
        &["workspace/node_modules".to_owned()]
    ));
    assert!(git_repository_scan_folder_ignored(
        &root,
        &["packages/../node_modules".to_owned()]
    ));
    assert!(git_repository_scan_folder_ignored(
        &root,
        &[root.parent().unwrap().display().to_string()]
    ));
    assert!(!git_repository_scan_folder_ignored(
        &root,
        &["modules".to_owned()]
    ));
    assert!(!git_repository_scan_folder_ignored(
        &root,
        &["../somewhere".to_owned()]
    ));

    std::fs::remove_dir_all(root.ancestors().nth(3).unwrap()).unwrap();
}

#[test]
fn workspace_plugins_follow_workspace_trust() {
    assert!(workspace_plugins_enabled(true));
    assert!(!workspace_plugins_enabled(false));
    assert_eq!(
        workspace_plugins_restricted_status(),
        "Trust this workspace to enable workspace plugins"
    );
}

fn temp_workspace(name: &str) -> PathBuf {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir()
        .join(format!("{name}-{}-{suffix}", std::process::id()))
        .join("workspace")
}

#[cfg(unix)]
fn create_directory_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn create_directory_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(target, link)
}
