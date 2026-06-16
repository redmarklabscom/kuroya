use crate::{
    KuroyaApp,
    app_startup_context::AppStartupContext,
    editor_input::EditorContextAction,
    git_diff_state::DiffBufferSource,
    git_diff_view::{
        accessible_diff_label, diff_buffer_display_kind, diff_buffer_display_label,
        hunk_header_line_in_unified_diff, hunk_modified_start_line_in_unified_diff,
        hunk_original_start_line_in_unified_diff, hunk_patch_from_unified_diff,
        join_unified_patches, source_control_all_patch_copy_empty_status,
        source_control_all_patch_copy_failure_status, source_control_all_patch_copy_success_status,
        source_control_commit_patch_copy_empty_status,
        source_control_commit_patch_copy_failure_status,
        source_control_commit_patch_copy_success_status,
        source_control_diff_base_open_missing_status,
        source_control_diff_base_open_unavailable_status,
        source_control_diff_buffer_patch_copy_empty_status,
        source_control_diff_buffer_patch_copy_success_status,
        source_control_diff_buffer_patch_copy_unavailable_status,
        source_control_diff_hunk_base_open_missing_hunk_status,
        source_control_diff_hunk_base_open_no_hunk_status,
        source_control_diff_hunk_base_open_success_status,
        source_control_diff_hunk_base_open_unavailable_status,
        source_control_diff_hunk_discard_stale_status,
        source_control_diff_hunk_identity_stale_status,
        source_control_diff_hunk_patch_copy_empty_status,
        source_control_diff_hunk_patch_copy_no_hunk_status,
        source_control_diff_hunk_patch_copy_success_status,
        source_control_diff_hunk_patch_copy_unavailable_status,
        source_control_diff_hunk_source_open_missing_hunk_status,
        source_control_diff_hunk_source_open_missing_status,
        source_control_diff_hunk_source_open_no_hunk_status,
        source_control_diff_hunk_source_open_success_status,
        source_control_diff_hunk_source_open_unavailable_status,
        source_control_diff_refresh_unavailable_status,
        source_control_diff_source_open_unavailable_status,
        source_control_head_revision_failure_status, source_control_head_revision_missing_status,
        source_control_hunk_diff_open_missing_status, source_control_hunk_diff_open_success_status,
        source_control_hunk_patch_copy_empty_status, source_control_hunk_patch_copy_failure_status,
        source_control_hunk_patch_copy_success_status,
        source_control_index_revision_failure_status, source_control_index_revision_missing_status,
        source_control_open_all_stage_empty_status, source_control_open_all_stage_success_status,
        source_control_patch_copy_empty_status, source_control_patch_copy_failure_status,
        source_control_patch_copy_success_status, source_control_stage_patch_copy_empty_status,
        source_control_stage_patch_copy_failure_status,
        source_control_stage_patch_copy_success_status,
        source_control_stash_patch_copy_empty_status,
        source_control_stash_patch_copy_failure_status,
        source_control_stash_patch_copy_success_status,
    },
    path_display::{DISPLAY_ERROR_LABEL_MAX_CHARS, DISPLAY_PATH_LABEL_MAX_CHARS},
    source_control_blame_runtime::{
        format_git_blame_view, git_blame_editor_decoration_hover_text_at,
        git_blame_editor_decoration_label_at, git_blame_status_bar_label_at,
        source_control_blame_key_for_path,
    },
    source_control_branch_picker::{
        source_control_branch_can_delete, source_control_branch_can_rename,
        source_control_branch_copy_status, source_control_branch_copy_text,
        source_control_branch_create_blocked_reason, source_control_branch_create_name,
        source_control_branch_create_tooltip, source_control_branch_delete_tooltip,
        source_control_branch_empty_label, source_control_branch_keyboard_action_labels,
        source_control_branch_label, source_control_branch_rename_action_tooltip,
        source_control_branch_rename_blocked_reason, source_control_branch_rename_target,
        source_control_branch_rename_tooltip, source_control_filtered_branches,
        source_control_new_branch_name, source_control_sorted_branches,
    },
    source_control_branch_runtime::{
        git_branch_create_failure_status, git_branch_create_pending_status,
        git_branch_create_success_status, git_branch_delete_failure_status,
        git_branch_delete_pending_status, git_branch_delete_success_status,
        git_branch_list_failure_status, git_branch_list_pending_status,
        git_branch_list_success_status, git_branch_rename_failure_status,
        git_branch_rename_pending_status, git_branch_rename_success_status,
        git_branch_switch_failure_status, git_branch_switch_pending_status,
        git_branch_switch_success_status,
    },
    source_control_conflicts::merge_conflict_resolution_success_status,
    source_control_history_panel::{
        SourceControlCommitCopyKind, source_control_commit_age_label_at,
        source_control_commit_copy_status, source_control_commit_copy_text,
        source_control_commit_copy_text_at, source_control_commit_label,
        source_control_filtered_history, source_control_graph_divergence_label,
        source_control_history_keyboard_action_labels,
    },
    source_control_history_runtime::{
        GIT_UNCOMMITTED_HISTORY_OID, git_history_failure_status, git_history_pending_status,
        git_history_success_status, next_git_history_limit, source_control_committed_history_len,
        source_control_history_can_load_more, source_control_history_commit_is_uncommitted,
        source_control_history_has_more, source_control_history_should_page_on_scroll,
        source_control_history_with_uncommitted,
    },
    source_control_hunk_panel::{
        source_control_hunk_keyboard_action_labels, source_control_hunk_label,
        source_control_hunk_panel_action_labels, source_control_hunk_panel_action_tooltips,
        source_control_hunk_source_line, source_control_hunk_source_open_missing_status,
        source_control_hunk_source_open_success_status,
    },
    source_control_hunk_runtime::{
        git_hunk_discard_failure_status, git_hunk_discard_missing_identity_status,
        git_hunk_discard_pending_status, git_hunk_discard_success_status,
        git_hunk_index_at_new_line, git_hunk_list_failure_status, git_hunk_list_pending_status,
        git_hunk_list_success_status, git_hunk_stage_failure_status,
        git_hunk_stage_missing_identity_status, git_hunk_stage_pending_status,
        git_hunk_stage_success_status, git_hunk_unstage_failure_status,
        git_hunk_unstage_missing_identity_status, git_hunk_unstage_pending_status,
        git_hunk_unstage_success_status, worktree_hunk_index_at_line,
    },
    source_control_panel::{
        SourceControlSortMode, SourceControlStageSection, SourceControlStageSectionKind,
        SourceControlViewMode, SourceControlVisibleRow, normalize_source_control_commit_history,
        record_source_control_commit_history, source_control_auto_reveal_selection,
        source_control_clear_commit_input, source_control_commit_action_button_visible,
        source_control_commit_enabled, source_control_commit_history_message,
        source_control_commit_input_font, source_control_commit_input_rows,
        source_control_commit_input_rows_for_mode,
        source_control_commit_input_validation_diagnostics, source_control_commit_input_visible,
        source_control_commit_tooltip, source_control_display_path_label,
        source_control_empty_changes_commit_input_visible, source_control_empty_changes_label,
        source_control_entries_for_untracked_changes, source_control_filter_empty_label,
        source_control_filtered_entries, source_control_keyboard_action_labels,
        source_control_repositories_section_visible, source_control_repository_label,
        source_control_result_count_label, source_control_reveal_selection,
        source_control_row_action_label_commands, source_control_row_action_labels,
        source_control_row_actions_visible, source_control_row_click_command,
        source_control_smart_commit_count, source_control_sort_mode_from_setting,
        source_control_sort_mode_label, source_control_sorted_entries,
        source_control_stage_collapse_tooltip, source_control_stage_header_action_labels,
        source_control_stage_header_label, source_control_stage_label,
        source_control_stage_section_header_label, source_control_stage_section_label,
        source_control_stage_sections, source_control_status_label, source_control_status_marker,
        source_control_tree_row_indent, source_control_verbose_commit_preview,
        source_control_view_action_button_visible, source_control_view_mode_from_setting,
        source_control_view_mode_label, source_control_visible_entries,
        source_control_visible_row_index_for_selection, source_control_visible_rows,
    },
    source_control_patch_runtime::{
        SourceControlPatchCopyRequest, source_control_patch_copy_detail,
        source_control_patch_copy_empty_status_for_request,
        source_control_patch_copy_failure_status_for_request,
        source_control_patch_copy_pending_status,
        source_control_patch_copy_success_status_for_request,
    },
    source_control_runtime::{
        SourceControlProtectedBranchCommitAction, git_commit_failure_status,
        git_commit_pending_status, git_commit_success_status, git_discard_failure_status,
        git_discard_pending_status, git_discard_success_status, git_progress_status,
        git_stage_failure_status, git_stage_pending_status, git_stage_success_status,
        git_unstage_failure_status, git_unstage_pending_status, git_unstage_success_status,
        invalidate_source_control_load_request_id_state,
        reserve_source_control_load_request_id_state,
        source_control_branch_protection_pattern_matches, source_control_commit_save_prompt_ids,
        source_control_commit_save_prompt_ids_for_commit,
        source_control_diff_buffers_for_operation, source_control_load_event_matches,
        source_control_mutation_restricted_status, source_control_panel_load_event_matches,
        source_control_protected_branch_commit_action,
        source_control_protected_branch_new_branch_required_status,
        source_control_protected_branch_prompt_body, source_control_protected_branch_prompt_title,
    },
    source_control_smart_commit_dialog::{
        source_control_commit_save_prompt_body, source_control_commit_save_prompt_title,
        source_control_empty_commit_confirmation_body, source_control_save_prompt_primary_label,
        source_control_smart_commit_always_button_label,
        source_control_smart_commit_never_button_label,
        source_control_smart_commit_once_button_label, source_control_smart_commit_suggestion_body,
        source_control_stash_save_prompt_body,
    },
    source_control_stash_panel::{
        SourceControlStashCopyKind, source_control_stash_copy_status,
        source_control_stash_copy_text, source_control_stash_footer_action_labels,
        source_control_stash_keyboard_action_labels, source_control_stash_label,
        source_control_stash_ref,
    },
    source_control_stash_runtime::{
        git_stash_apply_failure_status, git_stash_apply_pending_status,
        git_stash_apply_success_status, git_stash_drop_failure_status,
        git_stash_drop_pending_status, git_stash_drop_success_status,
        git_stash_list_failure_status, git_stash_list_pending_status,
        git_stash_list_success_status, git_stash_pop_failure_status, git_stash_pop_pending_status,
        git_stash_pop_success_status, git_stash_save_failure_status, git_stash_save_pending_status,
        git_stash_save_success_status, source_control_stash_message_from_inputs,
    },
    terminal::TerminalPane,
};
use kuroya_core::{
    Command, EditorSettings, GitBlameLine, GitBranch, GitBranchProtectionPrompt,
    GitBranchSortOrder, GitChangeStage, GitCheckoutType, GitCommitSummary, GitDiffHunk,
    GitFileStatus, GitPromptToSaveFilesBeforeCommit, GitRemoteDivergence, GitSmartCommitChanges,
    GitStashEntry, GitStatusEntry, GitUntrackedChanges, MAX_GIT_INPUT_VALIDATION_LENGTH,
    MergeConflictResolution, TextBuffer, TextEdit, Workspace,
};
use std::{collections::HashMap, path::PathBuf, time::Instant};
use tokio::runtime::Runtime;

mod blame;
mod branch;
mod commit_status;
mod conflicts;
mod history_stash;
mod hunk_diff;
mod row_actions;

#[test]
fn source_control_status_text_matches_git_statuses() {
    assert_eq!(source_control_status_marker(GitFileStatus::Modified), "M");
    assert_eq!(
        source_control_status_label(GitFileStatus::Modified),
        "Modified"
    );
    assert_eq!(source_control_status_marker(GitFileStatus::Added), "A");
    assert_eq!(source_control_status_label(GitFileStatus::Added), "Added");
    assert_eq!(source_control_status_marker(GitFileStatus::Deleted), "D");
    assert_eq!(
        source_control_status_label(GitFileStatus::Deleted),
        "Deleted"
    );
    assert_eq!(source_control_status_marker(GitFileStatus::Renamed), "R");
    assert_eq!(
        source_control_status_label(GitFileStatus::Renamed),
        "Renamed"
    );
    assert_eq!(source_control_status_marker(GitFileStatus::Untracked), "?");
    assert_eq!(
        source_control_status_label(GitFileStatus::Untracked),
        "Untracked"
    );
    assert_eq!(source_control_status_marker(GitFileStatus::Conflicted), "!");
    assert_eq!(
        source_control_status_label(GitFileStatus::Conflicted),
        "Conflicted"
    );
}

#[test]
fn source_control_loader_guards_reject_stale_requests() {
    let root = std::path::Path::new("workspace/current");

    assert!(source_control_load_event_matches(root, root, 5, 5));
    assert!(!source_control_load_event_matches(root, root, 4, 5));
    assert!(!source_control_load_event_matches(
        root,
        std::path::Path::new("workspace/old"),
        5,
        5
    ));
    assert!(source_control_panel_load_event_matches(
        true, root, root, 5, 5
    ));
    assert!(!source_control_panel_load_event_matches(
        false, root, root, 5, 5
    ));
}

#[test]
fn source_control_load_request_invalidation_keeps_request_ids_monotonic() {
    let mut next_request_id = 4;
    let mut active_request_id = 4;

    invalidate_source_control_load_request_id_state(&mut next_request_id, &mut active_request_id);

    assert_eq!(next_request_id, 5);
    assert_eq!(active_request_id, 5);
    assert_eq!(
        reserve_source_control_load_request_id_state(&mut next_request_id, &mut active_request_id),
        6
    );

    next_request_id = u64::MAX;
    active_request_id = u64::MAX;
    invalidate_source_control_load_request_id_state(&mut next_request_id, &mut active_request_id);

    assert_eq!(next_request_id, 1);
    assert_eq!(active_request_id, 1);
    assert_eq!(
        reserve_source_control_load_request_id_state(&mut next_request_id, &mut active_request_id),
        2
    );
}

#[test]
fn source_control_filter_matches_path_status_and_multiple_terms() {
    let root = PathBuf::from("C:/repo");
    let entries = vec![
        GitStatusEntry {
            path: root.join("src/main.rs"),
            status: GitFileStatus::Modified,
            stage: GitChangeStage::Unstaged,
        },
        GitStatusEntry {
            path: root.join("README.md"),
            status: GitFileStatus::Untracked,
            stage: GitChangeStage::Unstaged,
        },
        GitStatusEntry {
            path: root.join("old.txt"),
            status: GitFileStatus::Deleted,
            stage: GitChangeStage::Staged,
        },
    ];

    let rust = source_control_filtered_entries(&root, &entries, "src modified");
    assert_eq!(rust.len(), 1);
    assert_eq!(rust[0].path, root.join("src/main.rs"));

    let untracked = source_control_filtered_entries(&root, &entries, "? readme");
    assert_eq!(untracked.len(), 1);
    assert_eq!(untracked[0].path, root.join("README.md"));

    let missing = source_control_filtered_entries(&root, &entries, "src deleted");
    assert!(missing.is_empty());

    let staged = source_control_filtered_entries(&root, &entries, "staged old");
    assert_eq!(staged.len(), 1);
    assert_eq!(staged[0].path, root.join("old.txt"));

    let uppercase = source_control_filtered_entries(&root, &entries, "SRC MODIFIED");
    assert_eq!(uppercase.len(), 1);
    assert_eq!(uppercase[0].path, root.join("src/main.rs"));
}

#[test]
fn source_control_filter_supports_scoped_stage_and_status_terms() {
    let root = PathBuf::from("C:/repo");
    let entries = vec![
        GitStatusEntry {
            path: root.join("src/main.rs"),
            status: GitFileStatus::Modified,
            stage: GitChangeStage::Unstaged,
        },
        GitStatusEntry {
            path: root.join("README.md"),
            status: GitFileStatus::Untracked,
            stage: GitChangeStage::Unstaged,
        },
        GitStatusEntry {
            path: root.join("old.txt"),
            status: GitFileStatus::Deleted,
            stage: GitChangeStage::Staged,
        },
        GitStatusEntry {
            path: root.join("conflict.txt"),
            status: GitFileStatus::Conflicted,
            stage: GitChangeStage::Unstaged,
        },
    ];

    let unstaged_modified =
        source_control_filtered_entries(&root, &entries, "stage:unstaged status:m");
    assert_eq!(unstaged_modified.len(), 1);
    assert_eq!(unstaged_modified[0].path, root.join("src/main.rs"));

    let staged_deleted = source_control_filtered_entries(&root, &entries, "stage:index status:del");
    assert_eq!(staged_deleted.len(), 1);
    assert_eq!(staged_deleted[0].path, root.join("old.txt"));

    let untracked = source_control_filtered_entries(&root, &entries, "@untracked readme");
    assert_eq!(untracked.len(), 1);
    assert_eq!(untracked[0].path, root.join("README.md"));

    let conflicted = source_control_filtered_entries(&root, &entries, "is:conflict");
    assert_eq!(conflicted.len(), 1);
    assert_eq!(conflicted[0].path, root.join("conflict.txt"));

    assert!(source_control_filtered_entries(&root, &entries, "status:staged").is_empty());
}

#[test]
fn source_control_filter_preserves_ascii_only_case_folding() {
    let root = PathBuf::from("C:/repo");
    let entries = vec![GitStatusEntry {
        path: root.join("R\u{00e9}sum\u{00e9}.md"),
        status: GitFileStatus::Modified,
        stage: GitChangeStage::Unstaged,
    }];

    assert_eq!(
        source_control_filtered_entries(&root, &entries, "r\u{00e9}sum\u{00e9}").len(),
        1
    );
    assert!(source_control_filtered_entries(&root, &entries, "R\u{00c9}SUM\u{00c9}").is_empty());
}

#[test]
fn source_control_empty_state_text_explains_hidden_untracked_changes() {
    assert_eq!(
        source_control_empty_changes_label(0, GitUntrackedChanges::Hidden),
        "No source control changes"
    );
    assert_eq!(
        source_control_empty_changes_label(2, GitUntrackedChanges::Hidden),
        "Untracked changes are hidden"
    );
    assert_eq!(
        source_control_empty_changes_label(2, GitUntrackedChanges::Mixed),
        "No source control changes"
    );
}

#[test]
fn source_control_filter_empty_state_names_query() {
    assert_eq!(source_control_filter_empty_label(""), "No matching changes");
    assert_eq!(
        source_control_filter_empty_label("  src modified  "),
        "No matching changes for \"src modified\""
    );
    assert_eq!(
        source_control_filter_empty_label(
            "abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ"
        ),
        "No matching changes for \"abcdefghijklmnopqrstuv...DEFGHIJKLMNOPQRSTUVWXYZ\""
    );
}

#[test]
fn source_control_result_count_label_only_appears_for_filters() {
    assert_eq!(source_control_result_count_label(5, 2, ""), None);
    assert_eq!(
        source_control_result_count_label(5, 2, "src"),
        Some("2 of 5 changes".to_owned())
    );
    assert_eq!(
        source_control_result_count_label(1, 1, "main"),
        Some("1 of 1 change".to_owned())
    );
}

#[test]
fn source_control_sort_modes_match_vscode_view_options() {
    let root = PathBuf::from("C:/repo");
    let entries = vec![
        GitStatusEntry {
            path: root.join("src/zeta.rs"),
            status: GitFileStatus::Modified,
            stage: GitChangeStage::Unstaged,
        },
        GitStatusEntry {
            path: root.join("src/a.rs"),
            status: GitFileStatus::Deleted,
            stage: GitChangeStage::Unstaged,
        },
        GitStatusEntry {
            path: root.join("README.md"),
            status: GitFileStatus::Added,
            stage: GitChangeStage::Unstaged,
        },
        GitStatusEntry {
            path: root.join("staged-z.rs"),
            status: GitFileStatus::Modified,
            stage: GitChangeStage::Staged,
        },
        GitStatusEntry {
            path: root.join("staged-a.rs"),
            status: GitFileStatus::Added,
            stage: GitChangeStage::Staged,
        },
    ];

    assert_eq!(
        source_control_sort_mode_label(SourceControlSortMode::Path),
        "Path"
    );
    assert_eq!(
        source_control_sort_mode_label(SourceControlSortMode::Name),
        "Name"
    );
    assert_eq!(
        source_control_sort_mode_label(SourceControlSortMode::Status),
        "Status"
    );

    let by_path =
        source_control_sorted_entries(&root, entries.clone(), SourceControlSortMode::Path)
            .into_iter()
            .map(|entry| entry.path)
            .collect::<Vec<_>>();
    assert_eq!(
        by_path,
        vec![
            root.join("README.md"),
            root.join("src/a.rs"),
            root.join("src/zeta.rs"),
            root.join("staged-a.rs"),
            root.join("staged-z.rs"),
        ]
    );

    let by_name =
        source_control_sorted_entries(&root, entries.clone(), SourceControlSortMode::Name)
            .into_iter()
            .map(|entry| entry.path)
            .collect::<Vec<_>>();
    assert_eq!(
        by_name,
        vec![
            root.join("src/a.rs"),
            root.join("README.md"),
            root.join("src/zeta.rs"),
            root.join("staged-a.rs"),
            root.join("staged-z.rs"),
        ]
    );

    let by_status = source_control_sorted_entries(&root, entries, SourceControlSortMode::Status)
        .into_iter()
        .map(|entry| (entry.status, entry.path))
        .collect::<Vec<_>>();
    assert_eq!(
        by_status,
        vec![
            (GitFileStatus::Added, root.join("README.md")),
            (GitFileStatus::Deleted, root.join("src/a.rs")),
            (GitFileStatus::Modified, root.join("src/zeta.rs")),
            (GitFileStatus::Added, root.join("staged-a.rs")),
            (GitFileStatus::Modified, root.join("staged-z.rs")),
        ]
    );
}

#[test]
fn source_control_sorting_uses_case_insensitive_primary_keys() {
    let root = PathBuf::from("C:/repo");
    let entries = vec![
        GitStatusEntry {
            path: root.join("Zeta.rs"),
            status: GitFileStatus::Modified,
            stage: GitChangeStage::Unstaged,
        },
        GitStatusEntry {
            path: root.join("alpha.rs"),
            status: GitFileStatus::Modified,
            stage: GitChangeStage::Unstaged,
        },
    ];

    let by_path =
        source_control_sorted_entries(&root, entries.clone(), SourceControlSortMode::Path)
            .into_iter()
            .map(|entry| entry.path)
            .collect::<Vec<_>>();
    assert_eq!(by_path, vec![root.join("alpha.rs"), root.join("Zeta.rs")]);

    let by_name = source_control_sorted_entries(&root, entries, SourceControlSortMode::Name)
        .into_iter()
        .map(|entry| entry.path)
        .collect::<Vec<_>>();
    assert_eq!(by_name, vec![root.join("alpha.rs"), root.join("Zeta.rs")]);
}

#[test]
fn source_control_view_modes_match_vscode_list_and_tree_views() {
    let root = PathBuf::from("C:/repo");
    let nested = root.join("src/bin/main.rs");
    let top_level = root.join("README.md");

    assert_eq!(
        source_control_view_mode_label(SourceControlViewMode::List),
        "List"
    );
    assert_eq!(
        source_control_view_mode_label(SourceControlViewMode::Tree),
        "Tree"
    );
    assert_eq!(
        source_control_display_path_label(&root, &nested, SourceControlViewMode::List, true),
        "main.rs"
    );
    assert_eq!(
        source_control_display_path_label(&root, &nested, SourceControlViewMode::Tree, true),
        "src/bin/main.rs"
    );
    assert_eq!(
        source_control_display_path_label(&root, &nested, SourceControlViewMode::Tree, false),
        "main.rs"
    );
    assert_eq!(
        source_control_tree_row_indent(&root, &nested, SourceControlViewMode::Tree, false),
        24.0
    );
    assert_eq!(
        source_control_display_path_label(&root, &top_level, SourceControlViewMode::Tree, true),
        "README.md"
    );
    assert_eq!(
        source_control_tree_row_indent(&root, &top_level, SourceControlViewMode::Tree, false),
        0.0
    );
}

#[test]
fn source_control_default_settings_map_to_view_state() {
    assert_eq!(
        source_control_view_mode_from_setting(kuroya_core::ScmDefaultViewMode::List),
        SourceControlViewMode::List
    );
    assert_eq!(
        source_control_view_mode_from_setting(kuroya_core::ScmDefaultViewMode::Tree),
        SourceControlViewMode::Tree
    );
    assert_eq!(
        source_control_sort_mode_from_setting(kuroya_core::ScmDefaultViewSortKey::Path),
        SourceControlSortMode::Path
    );
    assert_eq!(
        source_control_sort_mode_from_setting(kuroya_core::ScmDefaultViewSortKey::Name),
        SourceControlSortMode::Name
    );
    assert_eq!(
        source_control_sort_mode_from_setting(kuroya_core::ScmDefaultViewSortKey::Status),
        SourceControlSortMode::Status
    );
}

#[test]
fn source_control_commit_input_visibility_follows_setting() {
    assert!(source_control_commit_input_visible(true));
    assert!(!source_control_commit_input_visible(false));
}

#[test]
fn source_control_clean_changes_commit_input_visibility_follows_setting() {
    let entries = vec![GitStatusEntry {
        path: PathBuf::from("src/main.rs"),
        status: GitFileStatus::Modified,
        stage: GitChangeStage::Unstaged,
    }];

    assert!(source_control_empty_changes_commit_input_visible(
        true,
        0,
        &[]
    ));
    assert!(!source_control_empty_changes_commit_input_visible(
        false,
        0,
        &[]
    ));
    assert!(!source_control_empty_changes_commit_input_visible(
        true, 1, &entries
    ));
    assert!(!source_control_empty_changes_commit_input_visible(
        true,
        1,
        &[]
    ));
}

#[test]
fn source_control_commit_action_button_visibility_follows_setting() {
    assert!(source_control_commit_action_button_visible(true, true));
    assert!(!source_control_commit_action_button_visible(false, true));
    assert!(!source_control_commit_action_button_visible(true, false));
    assert!(!source_control_commit_action_button_visible(false, false));
}

#[test]
fn source_control_view_action_button_visibility_follows_setting() {
    assert!(source_control_view_action_button_visible(true));
    assert!(!source_control_view_action_button_visible(false));
}

#[test]
fn source_control_repositories_section_follows_visibility_settings() {
    assert!(source_control_repositories_section_visible(true, 1));
    assert!(source_control_repositories_section_visible(true, 10));
    assert!(!source_control_repositories_section_visible(false, 10));
    assert!(!source_control_repositories_section_visible(true, 0));
}

#[test]
fn source_control_repository_label_names_repo_and_branch() {
    assert_eq!(
        source_control_repository_label(&PathBuf::from("workspace/project"), Some("main"), true),
        "project (main)"
    );
    assert_eq!(
        source_control_repository_label(&PathBuf::from("workspace/project"), None, true),
        "project (detached)"
    );
    assert_eq!(
        source_control_repository_label(&PathBuf::from(""), Some("main"), true),
        "repository (main)"
    );
    assert_eq!(
        source_control_repository_label(&PathBuf::from("workspace/project"), Some("main"), false),
        "project"
    );
}

#[test]
fn source_control_commit_input_rows_follow_min_max_and_message_lines() {
    assert_eq!(source_control_commit_input_rows("", 1, 10), 1);
    assert_eq!(
        source_control_commit_input_rows("one\ntwo\nthree", 1, 10),
        3
    );
    assert_eq!(source_control_commit_input_rows("one", 3, 10), 3);
    assert_eq!(source_control_commit_input_rows("1\n2\n3\n4\n5", 1, 3), 3);
    assert_eq!(source_control_commit_input_rows("one", 0, 0), 1);
    assert_eq!(source_control_commit_input_rows("one", 8, 2), 8);
}

#[test]
fn source_control_commit_input_rows_follow_editor_mode() {
    assert_eq!(
        source_control_commit_input_rows_for_mode(true, "one\ntwo", 1, 8),
        2
    );
    assert_eq!(
        source_control_commit_input_rows_for_mode(false, "one\ntwo", 1, 8),
        1
    );
}

#[test]
fn source_control_verbose_commit_preview_requires_editor_mode_and_staged_changes() {
    let entries = vec![
        GitStatusEntry {
            path: PathBuf::from("src/main.rs"),
            status: GitFileStatus::Modified,
            stage: GitChangeStage::Staged,
        },
        GitStatusEntry {
            path: PathBuf::from("src/new.rs"),
            status: GitFileStatus::Added,
            stage: GitChangeStage::Unstaged,
        },
    ];

    let preview = source_control_verbose_commit_preview(&entries, true, true).unwrap();

    assert!(preview.contains("# Changes to be committed:"));
    assert!(preview.contains("modified"));
    assert!(preview.contains("main.rs"));
    assert!(!preview.contains("new.rs"));
    assert!(source_control_verbose_commit_preview(&entries, false, true).is_none());
    assert!(source_control_verbose_commit_preview(&entries, true, false).is_none());
    assert!(source_control_verbose_commit_preview(&entries[1..], true, true).is_none());
}

#[test]
fn source_control_commit_input_font_follows_family_and_size_settings() {
    let default_font = source_control_commit_input_font("default", 14.0, 16.0, 13.0);
    assert_eq!(default_font.size, 14.0);
    assert_eq!(default_font.family, eframe::egui::FontFamily::Proportional);

    let editor_font = source_control_commit_input_font("editor", 15.0, 16.0, 13.0);
    assert_eq!(editor_font.size, 15.0);
    assert_eq!(editor_font.family, eframe::egui::FontFamily::Monospace);

    let custom_font = source_control_commit_input_font("CommitSans", f32::NAN, 16.0, 13.0);
    assert_eq!(custom_font.size, kuroya_core::DEFAULT_SCM_INPUT_FONT_SIZE);
    assert_eq!(
        custom_font.family,
        eframe::egui::FontFamily::Name("CommitSans".into())
    );
}

#[test]
fn source_control_commit_input_escape_clears_message() {
    let mut message = "ship it".to_owned();
    assert!(source_control_clear_commit_input(&mut message, true));
    assert!(message.is_empty());

    assert!(!source_control_clear_commit_input(&mut message, true));

    message = "keep draft".to_owned();
    assert!(!source_control_clear_commit_input(&mut message, false));
    assert_eq!(message, "keep draft");
}

#[test]
fn source_control_commit_history_records_unique_recent_messages() {
    let mut history = vec!["older".to_owned(), "repeat".to_owned()];

    record_source_control_commit_history(&mut history, " repeat ", 3);
    record_source_control_commit_history(&mut history, "newer", 3);
    record_source_control_commit_history(&mut history, "latest", 3);
    record_source_control_commit_history(&mut history, "   ", 3);

    assert_eq!(history, vec!["repeat", "newer", "latest"]);
}

#[test]
fn source_control_commit_history_normalizes_carriage_return_line_endings() {
    let mut history = vec!["subject\nbody".to_owned(), "older".to_owned()];

    record_source_control_commit_history(&mut history, " subject\r\nbody ", 5);
    record_source_control_commit_history(&mut history, "other\rbody", 5);

    assert_eq!(
        history,
        vec![
            "older".to_owned(),
            "subject\nbody".to_owned(),
            "other\nbody".to_owned()
        ]
    );
    assert_eq!(
        normalize_source_control_commit_history(
            vec![
                "subject\r\nbody".to_owned(),
                "subject\rbody".to_owned(),
                "subject\nbody".to_owned(),
                "tail\rline".to_owned(),
            ],
            5
        ),
        vec!["subject\nbody".to_owned(), "tail\nline".to_owned()]
    );
}

#[test]
fn source_control_commit_history_normalization_matches_recording_order() {
    let history = vec![
        " older ".to_owned(),
        "repeat".to_owned(),
        "newer".to_owned(),
        "repeat".to_owned(),
        "   ".to_owned(),
        "latest".to_owned(),
    ];

    assert_eq!(
        normalize_source_control_commit_history(history, 3),
        vec!["newer".to_owned(), "repeat".to_owned(), "latest".to_owned()]
    );
    assert!(normalize_source_control_commit_history(vec!["message".to_owned()], 0).is_empty());
}

#[test]
fn source_control_commit_history_normalization_handles_owned_clean_trimmed_duplicates() {
    let history = vec![
        "clean".to_owned(),
        " trimmed ".to_owned(),
        "clean".to_owned(),
        "\t".to_owned(),
        "trimmed".to_owned(),
        "latest".to_owned(),
    ];

    assert_eq!(
        normalize_source_control_commit_history(history, 4),
        vec![
            "clean".to_owned(),
            "trimmed".to_owned(),
            "latest".to_owned()
        ]
    );
}

#[test]
fn source_control_commit_history_navigation_matches_alt_arrow_order() {
    let history = vec!["first".to_owned(), "second".to_owned(), "third".to_owned()];
    let mut index = None;

    assert_eq!(
        source_control_commit_history_message(&history, &mut index, -1).as_deref(),
        Some("third")
    );
    assert_eq!(
        source_control_commit_history_message(&history, &mut index, -1).as_deref(),
        Some("second")
    );
    assert_eq!(
        source_control_commit_history_message(&history, &mut index, 1).as_deref(),
        Some("third")
    );
    assert_eq!(
        source_control_commit_history_message(&history, &mut index, 1).as_deref(),
        Some("third")
    );
}

#[test]
fn source_control_stage_labels_match_vscode_groups() {
    assert_eq!(
        source_control_stage_label(GitChangeStage::Unstaged),
        "Changes"
    );
    assert_eq!(
        source_control_stage_label(GitChangeStage::Staged),
        "Staged Changes"
    );
}

#[test]
fn source_control_stage_headers_include_counts_and_group_actions() {
    assert_eq!(
        source_control_stage_header_label(GitChangeStage::Unstaged, 2),
        "Changes (2)"
    );
    assert_eq!(
        source_control_stage_header_label(GitChangeStage::Staged, 1),
        "Staged Changes (1)"
    );
    assert_eq!(
        source_control_stage_header_action_labels(GitChangeStage::Unstaged),
        vec![
            "Open all unstaged changes",
            "Copy unstaged patch",
            "Stage all changes",
            "Discard all changes"
        ]
    );
    assert_eq!(
        source_control_stage_header_action_labels(GitChangeStage::Staged),
        vec![
            "Open all staged changes",
            "Copy staged patch",
            "Unstage all changes"
        ]
    );
}

#[test]
fn source_control_stage_sections_can_keep_empty_staged_group_visible() {
    let root = PathBuf::from("C:/repo");
    let entries = vec![GitStatusEntry {
        path: root.join("src/main.rs"),
        status: GitFileStatus::Modified,
        stage: GitChangeStage::Unstaged,
    }];

    let hidden_empty = source_control_stage_sections(&entries, false, GitUntrackedChanges::Mixed);
    assert_eq!(hidden_empty.len(), 1);
    assert_eq!(hidden_empty[0].stage, GitChangeStage::Unstaged);
    assert_eq!(hidden_empty[0].count, 1);

    let visible_empty = source_control_stage_sections(&entries, true, GitUntrackedChanges::Mixed);
    assert_eq!(
        visible_empty
            .iter()
            .map(|section| (section.stage, section.kind, section.count))
            .collect::<Vec<_>>(),
        vec![
            (
                GitChangeStage::Unstaged,
                SourceControlStageSectionKind::Changes,
                1
            ),
            (
                GitChangeStage::Staged,
                SourceControlStageSectionKind::StagedChanges,
                0
            )
        ]
    );
}

#[test]
fn source_control_untracked_changes_setting_filters_and_groups_entries() {
    let root = PathBuf::from("C:/repo");
    let entries = vec![
        GitStatusEntry {
            path: root.join("modified.rs"),
            status: GitFileStatus::Modified,
            stage: GitChangeStage::Unstaged,
        },
        GitStatusEntry {
            path: root.join("new.rs"),
            status: GitFileStatus::Untracked,
            stage: GitChangeStage::Unstaged,
        },
        GitStatusEntry {
            path: root.join("staged.rs"),
            status: GitFileStatus::Added,
            stage: GitChangeStage::Staged,
        },
    ];

    let mixed = source_control_stage_sections(&entries, false, GitUntrackedChanges::Mixed);
    assert_eq!(
        mixed
            .iter()
            .map(|section| (section.kind, section.count))
            .collect::<Vec<_>>(),
        vec![
            (SourceControlStageSectionKind::Changes, 2),
            (SourceControlStageSectionKind::StagedChanges, 1)
        ]
    );

    let separate = source_control_stage_sections(&entries, false, GitUntrackedChanges::Separate);
    assert_eq!(
        separate
            .iter()
            .map(|section| {
                (
                    source_control_stage_section_label(section.kind),
                    source_control_stage_section_header_label(section.kind, section.count),
                )
            })
            .collect::<Vec<_>>(),
        vec![
            ("Changes", "Changes (1)".to_owned()),
            ("Untracked Changes", "Untracked Changes (1)".to_owned()),
            ("Staged Changes", "Staged Changes (1)".to_owned())
        ]
    );

    let hidden = source_control_entries_for_untracked_changes(entries, GitUntrackedChanges::Hidden);
    assert_eq!(hidden.len(), 2);
    assert!(
        hidden
            .iter()
            .all(|entry| entry.status != GitFileStatus::Untracked)
    );
}

#[test]
fn source_control_stage_headers_explain_collapsed_state() {
    assert_eq!(
        source_control_stage_collapse_tooltip(GitChangeStage::Unstaged, false),
        "Collapse Changes"
    );
    assert_eq!(
        source_control_stage_collapse_tooltip(GitChangeStage::Staged, true),
        "Expand Staged Changes"
    );
}

#[test]
fn source_control_collapsed_groups_hide_rows_from_selection() {
    let root = PathBuf::from("C:/repo");
    let entries = vec![
        GitStatusEntry {
            path: root.join("src/main.rs"),
            status: GitFileStatus::Modified,
            stage: GitChangeStage::Unstaged,
        },
        GitStatusEntry {
            path: root.join("README.md"),
            status: GitFileStatus::Added,
            stage: GitChangeStage::Staged,
        },
    ];

    let all =
        source_control_visible_entries(&entries, GitUntrackedChanges::Mixed, false, false, false);
    assert_eq!(all.len(), 2);

    let without_changes =
        source_control_visible_entries(&entries, GitUntrackedChanges::Mixed, true, false, false);
    assert_eq!(without_changes.len(), 1);
    assert_eq!(without_changes[0].stage, GitChangeStage::Staged);

    let without_staged =
        source_control_visible_entries(&entries, GitUntrackedChanges::Mixed, false, false, true);
    assert_eq!(without_staged.len(), 1);
    assert_eq!(without_staged[0].stage, GitChangeStage::Unstaged);

    assert!(
        source_control_visible_entries(&entries, GitUntrackedChanges::Mixed, true, false, true)
            .is_empty()
    );
}

#[test]
fn source_control_visible_rows_include_headers_and_only_expanded_entries() {
    let root = PathBuf::from("C:/repo");
    let entries = vec![
        GitStatusEntry {
            path: root.join("src/main.rs"),
            status: GitFileStatus::Modified,
            stage: GitChangeStage::Unstaged,
        },
        GitStatusEntry {
            path: root.join("README.md"),
            status: GitFileStatus::Added,
            stage: GitChangeStage::Staged,
        },
    ];

    let rows = source_control_visible_rows(
        &entries,
        true,
        GitUntrackedChanges::Mixed,
        false,
        false,
        true,
    );

    assert_eq!(
        rows,
        vec![
            SourceControlVisibleRow::Header(SourceControlStageSection {
                stage: GitChangeStage::Unstaged,
                kind: SourceControlStageSectionKind::Changes,
                count: 1,
            }),
            SourceControlVisibleRow::Entry {
                entry_index: 0,
                visible_index: 0,
            },
            SourceControlVisibleRow::Header(SourceControlStageSection {
                stage: GitChangeStage::Staged,
                kind: SourceControlStageSectionKind::StagedChanges,
                count: 1,
            }),
        ]
    );
}

#[test]
fn source_control_separate_untracked_group_has_independent_collapse_state() {
    let root = PathBuf::from("C:/repo");
    let entries = vec![
        GitStatusEntry {
            path: root.join("modified.rs"),
            status: GitFileStatus::Modified,
            stage: GitChangeStage::Unstaged,
        },
        GitStatusEntry {
            path: root.join("new.rs"),
            status: GitFileStatus::Untracked,
            stage: GitChangeStage::Unstaged,
        },
        GitStatusEntry {
            path: root.join("staged.rs"),
            status: GitFileStatus::Added,
            stage: GitChangeStage::Staged,
        },
    ];

    let tracked_collapsed = source_control_visible_rows(
        &entries,
        true,
        GitUntrackedChanges::Separate,
        true,
        false,
        true,
    );
    assert!(tracked_collapsed.contains(&SourceControlVisibleRow::Entry {
        entry_index: 1,
        visible_index: 0,
    }));
    assert!(
        !tracked_collapsed.contains(&SourceControlVisibleRow::Entry {
            entry_index: 0,
            visible_index: 0,
        })
    );
    assert!(
        !tracked_collapsed.contains(&SourceControlVisibleRow::Entry {
            entry_index: 2,
            visible_index: 0,
        })
    );

    let untracked_collapsed = source_control_visible_rows(
        &entries,
        true,
        GitUntrackedChanges::Separate,
        false,
        true,
        true,
    );
    assert!(
        untracked_collapsed.contains(&SourceControlVisibleRow::Entry {
            entry_index: 0,
            visible_index: 0,
        })
    );
    assert!(
        !untracked_collapsed.contains(&SourceControlVisibleRow::Entry {
            entry_index: 1,
            visible_index: 0,
        })
    );
    assert!(
        !untracked_collapsed.contains(&SourceControlVisibleRow::Entry {
            entry_index: 2,
            visible_index: 0,
        })
    );
}

#[test]
fn source_control_visible_row_index_maps_selected_entry_rows() {
    let rows = vec![
        SourceControlVisibleRow::Header(SourceControlStageSection {
            stage: GitChangeStage::Unstaged,
            kind: SourceControlStageSectionKind::Changes,
            count: 2,
        }),
        SourceControlVisibleRow::Entry {
            entry_index: 0,
            visible_index: 0,
        },
        SourceControlVisibleRow::Entry {
            entry_index: 1,
            visible_index: 1,
        },
        SourceControlVisibleRow::Header(SourceControlStageSection {
            stage: GitChangeStage::Staged,
            kind: SourceControlStageSectionKind::StagedChanges,
            count: 1,
        }),
        SourceControlVisibleRow::Entry {
            entry_index: 2,
            visible_index: 2,
        },
    ];

    assert_eq!(
        source_control_visible_row_index_for_selection(&rows, 0),
        Some(1)
    );
    assert_eq!(
        source_control_visible_row_index_for_selection(&rows, 2),
        Some(4)
    );
    assert_eq!(
        source_control_visible_row_index_for_selection(&rows, 3),
        None
    );
}

#[test]
fn source_control_reveal_selects_changed_path_and_expands_group() {
    let root = PathBuf::from("C:/repo");
    let entries = vec![
        GitStatusEntry {
            path: root.join("README.md"),
            status: GitFileStatus::Modified,
            stage: GitChangeStage::Staged,
        },
        GitStatusEntry {
            path: root.join("src/main.rs"),
            status: GitFileStatus::Modified,
            stage: GitChangeStage::Unstaged,
        },
    ];

    let selection = source_control_reveal_selection(
        &entries,
        &root.join("src/main.rs"),
        None,
        GitUntrackedChanges::Mixed,
        true,
        false,
        false,
    )
    .unwrap();

    assert_eq!(selection.selected, 1);
    assert_eq!(selection.stage, GitChangeStage::Unstaged);
    assert!(!selection.unstaged_collapsed);
    assert!(!selection.staged_collapsed);

    let staged_selection = source_control_reveal_selection(
        &entries,
        &root.join("README.md"),
        None,
        GitUntrackedChanges::Mixed,
        false,
        false,
        true,
    )
    .unwrap();

    assert_eq!(staged_selection.selected, 0);
    assert_eq!(staged_selection.stage, GitChangeStage::Staged);
    assert!(!staged_selection.staged_collapsed);
}

#[test]
fn source_control_reveal_prefers_unstaged_unless_stage_is_requested() {
    let root = PathBuf::from("C:/repo");
    let path = root.join("src/main.rs");
    let entries = vec![
        GitStatusEntry {
            path: path.clone(),
            status: GitFileStatus::Modified,
            stage: GitChangeStage::Staged,
        },
        GitStatusEntry {
            path: path.clone(),
            status: GitFileStatus::Modified,
            stage: GitChangeStage::Unstaged,
        },
    ];

    let default_selection = source_control_reveal_selection(
        &entries,
        &path,
        None,
        GitUntrackedChanges::Mixed,
        false,
        false,
        false,
    )
    .unwrap();
    assert_eq!(default_selection.stage, GitChangeStage::Unstaged);
    assert_eq!(default_selection.selected, 1);

    let staged_selection = source_control_reveal_selection(
        &entries,
        &path,
        Some(GitChangeStage::Staged),
        GitUntrackedChanges::Mixed,
        false,
        false,
        false,
    )
    .unwrap();
    assert_eq!(staged_selection.stage, GitChangeStage::Staged);
    assert_eq!(staged_selection.selected, 0);
}

#[test]
fn source_control_reveal_ignores_hidden_untracked_entries() {
    let root = PathBuf::from("C:/repo");
    let untracked_path = root.join("scratch.txt");
    let modified_path = root.join("src/main.rs");
    let entries = vec![
        GitStatusEntry {
            path: untracked_path.clone(),
            status: GitFileStatus::Untracked,
            stage: GitChangeStage::Unstaged,
        },
        GitStatusEntry {
            path: modified_path.clone(),
            status: GitFileStatus::Modified,
            stage: GitChangeStage::Unstaged,
        },
    ];

    assert!(
        source_control_reveal_selection(
            &entries,
            &untracked_path,
            None,
            GitUntrackedChanges::Hidden,
            false,
            false,
            false,
        )
        .is_none()
    );

    let selection = source_control_reveal_selection(
        &entries,
        &modified_path,
        None,
        GitUntrackedChanges::Hidden,
        false,
        false,
        false,
    )
    .unwrap();

    assert_eq!(selection.selected, 0);
    assert_eq!(selection.stage, GitChangeStage::Unstaged);
}

#[test]
fn source_control_reveal_expands_only_matching_separate_group() {
    let root = PathBuf::from("C:/repo");
    let tracked_path = root.join("modified.rs");
    let untracked_path = root.join("new.rs");
    let staged_path = root.join("staged.rs");
    let entries = vec![
        GitStatusEntry {
            path: tracked_path.clone(),
            status: GitFileStatus::Modified,
            stage: GitChangeStage::Unstaged,
        },
        GitStatusEntry {
            path: untracked_path.clone(),
            status: GitFileStatus::Untracked,
            stage: GitChangeStage::Unstaged,
        },
        GitStatusEntry {
            path: staged_path.clone(),
            status: GitFileStatus::Added,
            stage: GitChangeStage::Staged,
        },
    ];

    let untracked = source_control_reveal_selection(
        &entries,
        &untracked_path,
        None,
        GitUntrackedChanges::Separate,
        true,
        true,
        true,
    )
    .unwrap();
    assert!(untracked.unstaged_collapsed);
    assert!(!untracked.untracked_collapsed);
    assert!(untracked.staged_collapsed);
    assert_eq!(untracked.selected, 0);

    let tracked = source_control_reveal_selection(
        &entries,
        &tracked_path,
        None,
        GitUntrackedChanges::Separate,
        true,
        true,
        true,
    )
    .unwrap();
    assert!(!tracked.unstaged_collapsed);
    assert!(tracked.untracked_collapsed);
    assert!(tracked.staged_collapsed);
    assert_eq!(tracked.selected, 0);
}

#[test]
fn source_control_auto_reveal_follows_visibility_and_setting() {
    let root = PathBuf::from("C:/repo");
    let path = root.join("src/main.rs");
    let entries = vec![GitStatusEntry {
        path: path.clone(),
        status: GitFileStatus::Modified,
        stage: GitChangeStage::Unstaged,
    }];

    assert!(
        source_control_auto_reveal_selection(
            &entries,
            &path,
            None,
            GitUntrackedChanges::Mixed,
            true,
            false,
            true,
            false,
            false
        )
        .is_none()
    );
    assert!(
        source_control_auto_reveal_selection(
            &entries,
            &path,
            None,
            GitUntrackedChanges::Mixed,
            false,
            true,
            true,
            false,
            false
        )
        .is_none()
    );

    let selection = source_control_auto_reveal_selection(
        &entries,
        &path,
        None,
        GitUntrackedChanges::Mixed,
        true,
        true,
        true,
        false,
        false,
    )
    .unwrap();
    assert_eq!(selection.selected, 0);
    assert!(!selection.unstaged_collapsed);
}

#[test]
fn source_control_auto_reveal_ignores_hidden_untracked_entries() {
    let root = PathBuf::from("C:/repo");
    let path = root.join("scratch.txt");
    let entries = vec![GitStatusEntry {
        path: path.clone(),
        status: GitFileStatus::Untracked,
        stage: GitChangeStage::Unstaged,
    }];

    assert!(
        source_control_auto_reveal_selection(
            &entries,
            &path,
            None,
            GitUntrackedChanges::Hidden,
            true,
            true,
            false,
            false,
            false,
        )
        .is_none()
    );
}

fn dirty_test_buffer(id: kuroya_core::BufferId, path: Option<PathBuf>) -> TextBuffer {
    let mut buffer = TextBuffer::from_text(id, path, "base".to_owned());
    buffer.apply_edit(TextEdit {
        range: 0..0,
        inserted: "dirty ".to_owned(),
    });
    buffer
}

fn app_for_source_control_test(root: PathBuf) -> KuroyaApp {
    let (tx, rx) = crate::ui_event_channel::ui_event_channel();
    let settings = EditorSettings::default();
    KuroyaApp::from_startup_context(AppStartupContext {
        runtime: Runtime::new().expect("test runtime"),
        tx,
        rx,
        workspace: Workspace::new(root.clone()),
        settings: settings.clone(),
        settings_panel_draft: settings,
        settings_editor_font_path: String::new(),
        settings_ui_font_path: String::new(),
        theme_picker_selected: 0,
        saved_session: None,
        terminal: TerminalPane::new(root.clone(), 100, 12.0, 1.2),
        watcher: None,
        recent_projects: Vec::new(),
        trusted_workspaces: vec![root],
        now: Instant::now(),
        startup_timings: Vec::new(),
    })
}
