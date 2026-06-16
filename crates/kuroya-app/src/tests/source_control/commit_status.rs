use super::*;

#[test]
fn source_control_commit_button_requires_message_and_staged_changes() {
    let root = PathBuf::from("C:/repo");
    let unstaged = vec![GitStatusEntry {
        path: root.join("src/main.rs"),
        status: GitFileStatus::Modified,
        stage: GitChangeStage::Unstaged,
    }];
    let staged = vec![GitStatusEntry {
        path: root.join("src/main.rs"),
        status: GitFileStatus::Modified,
        stage: GitChangeStage::Staged,
    }];
    let conflicted = vec![GitStatusEntry {
        path: root.join("src/main.rs"),
        status: GitFileStatus::Conflicted,
        stage: GitChangeStage::Unstaged,
    }];

    assert!(source_control_commit_enabled(
        &[],
        "ship it",
        false,
        false,
        GitSmartCommitChanges::All,
        true
    ));
    assert!(!source_control_commit_enabled(
        &unstaged,
        "ship it",
        false,
        false,
        GitSmartCommitChanges::All,
        true
    ));
    assert!(source_control_commit_enabled(
        &unstaged,
        "ship it",
        true,
        false,
        GitSmartCommitChanges::All,
        true
    ));
    assert!(source_control_commit_enabled(
        &unstaged,
        "ship it",
        false,
        true,
        GitSmartCommitChanges::All,
        true
    ));
    assert!(!source_control_commit_enabled(
        &staged,
        "  ",
        true,
        false,
        GitSmartCommitChanges::All,
        true
    ));
    assert!(source_control_commit_enabled(
        &staged,
        "ship it",
        false,
        false,
        GitSmartCommitChanges::All,
        true
    ));
    assert!(!source_control_commit_enabled(
        &conflicted,
        "ship it",
        true,
        false,
        GitSmartCommitChanges::All,
        true
    ));
    assert_eq!(
        source_control_commit_tooltip(0, "ship it", false, false, false, 1, true),
        "Stage changes before committing"
    );
    assert_eq!(
        source_control_commit_tooltip(1, "  ", false, true, false, 0, true),
        "Enter a commit message before committing"
    );
    assert_eq!(
        source_control_commit_tooltip(1, "ship it", false, true, false, 0, true),
        "Commit staged changes (Ctrl+Enter)"
    );
    assert_eq!(
        source_control_commit_tooltip(0, "ship it", false, true, false, 1, true),
        "Smart commit eligible changes (Ctrl+Enter)"
    );
    assert_eq!(
        source_control_commit_tooltip(0, "ship it", false, false, true, 1, true),
        "Stage eligible changes and commit (Ctrl+Enter)"
    );
    assert_eq!(
        source_control_commit_tooltip(0, "ship it", false, true, false, 0, true),
        "Confirm empty commit (Ctrl+Enter)"
    );
    assert_eq!(
        source_control_commit_tooltip(0, "ship it", false, true, false, 0, false),
        "Create empty commit (Ctrl+Enter)"
    );
    assert_eq!(
        source_control_commit_tooltip(0, "ship it", true, true, false, 0, true),
        "Resolve merge conflicts before committing"
    );
}

#[test]
fn source_control_smart_commit_count_excludes_conflicted_entries() {
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
    ];
    let conflicts_only = vec![GitStatusEntry {
        path: PathBuf::from("src/conflict.rs"),
        status: GitFileStatus::Conflicted,
        stage: GitChangeStage::Unstaged,
    }];

    assert_eq!(
        source_control_smart_commit_count(&entries, GitSmartCommitChanges::All),
        2
    );
    assert_eq!(
        source_control_smart_commit_count(&entries, GitSmartCommitChanges::Tracked),
        1
    );
    assert_eq!(
        source_control_smart_commit_count(&conflicts_only, GitSmartCommitChanges::All),
        0
    );
    assert_eq!(
        source_control_smart_commit_count(&conflicts_only, GitSmartCommitChanges::Tracked),
        0
    );
}

#[test]
fn source_control_commit_can_empty_commit_when_smart_commit_skips_untracked_changes() {
    let untracked = vec![GitStatusEntry {
        path: PathBuf::from("new.rs"),
        status: GitFileStatus::Untracked,
        stage: GitChangeStage::Unstaged,
    }];

    assert!(source_control_commit_enabled(
        &untracked,
        "ship it",
        true,
        false,
        GitSmartCommitChanges::All,
        true
    ));
    assert!(source_control_commit_enabled(
        &untracked,
        "ship it",
        true,
        false,
        GitSmartCommitChanges::Tracked,
        true
    ));
    assert!(source_control_commit_enabled(
        &untracked,
        "ship it",
        false,
        true,
        GitSmartCommitChanges::All,
        true
    ));
    assert!(source_control_commit_enabled(
        &untracked,
        "ship it",
        false,
        true,
        GitSmartCommitChanges::Tracked,
        true
    ));
}

#[test]
fn source_control_commit_input_validation_reports_long_subject_and_body_lines() {
    assert!(source_control_commit_input_validation_diagnostics("short", false, 72, 50).is_empty());
    assert!(source_control_commit_input_validation_diagnostics("short", true, 72, 50).is_empty());
    assert_eq!(
        source_control_commit_input_validation_diagnostics("123456", true, 72, 5),
        vec!["Subject is 6 characters, above the 5 character limit".to_owned()]
    );
    assert_eq!(
        source_control_commit_input_validation_diagnostics(
            "subject\nbody line is too long",
            true,
            8,
            50,
        ),
        vec!["Line 2 is 21 characters, above the 8 character limit".to_owned()]
    );
}

#[test]
fn source_control_commit_input_validation_clamps_body_line_length_limit() {
    let body = "x".repeat(MAX_GIT_INPUT_VALIDATION_LENGTH + 1);
    let message = format!("subject\n{body}");

    assert_eq!(
        source_control_commit_input_validation_diagnostics(&message, true, usize::MAX, 50),
        vec![format!(
            "Line 2 is {} characters, above the {} character limit",
            MAX_GIT_INPUT_VALIDATION_LENGTH + 1,
            MAX_GIT_INPUT_VALIDATION_LENGTH
        )]
    );
}

#[test]
fn source_control_smart_commit_suggestion_body_names_change_count() {
    assert_eq!(
        source_control_smart_commit_suggestion_body(1),
        "Would you like to stage 1 eligible change and commit it directly?"
    );
    assert_eq!(
        source_control_smart_commit_suggestion_body(3),
        "Would you like to stage 3 eligible changes and commit them directly?"
    );
}

#[test]
fn source_control_smart_commit_suggestion_buttons_use_action_labels() {
    assert_eq!(
        source_control_smart_commit_once_button_label(),
        "Stage and Commit"
    );
    assert_eq!(
        source_control_smart_commit_always_button_label(),
        "Always Stage and Commit"
    );
    assert_eq!(
        source_control_smart_commit_never_button_label(),
        "Never Ask Again"
    );
}

#[test]
fn source_control_empty_commit_confirmation_body_names_message() {
    assert_eq!(
        source_control_empty_commit_confirmation_body(" empty checkpoint "),
        "Create an empty commit with message \"empty checkpoint\"?"
    );
}

#[test]
fn source_control_commit_save_prompt_ids_follow_vs_code_scope() {
    let staged_path = PathBuf::from("workspace/src/staged.rs");
    let unstaged_path = PathBuf::from("workspace/src/unstaged.rs");
    let clean_path = PathBuf::from("workspace/src/clean.rs");
    let entries = vec![
        GitStatusEntry {
            path: staged_path.clone(),
            status: GitFileStatus::Modified,
            stage: GitChangeStage::Staged,
        },
        GitStatusEntry {
            path: unstaged_path.clone(),
            status: GitFileStatus::Modified,
            stage: GitChangeStage::Unstaged,
        },
    ];
    let buffers = vec![
        dirty_test_buffer(1, Some(staged_path)),
        dirty_test_buffer(2, Some(unstaged_path)),
        dirty_test_buffer(3, None),
        TextBuffer::from_text(4, Some(clean_path), "clean".to_owned()),
    ];

    assert_eq!(
        source_control_commit_save_prompt_ids(
            &buffers,
            &entries,
            GitPromptToSaveFilesBeforeCommit::Always
        ),
        vec![1, 2, 3]
    );
    assert_eq!(
        source_control_commit_save_prompt_ids(
            &buffers,
            &entries,
            GitPromptToSaveFilesBeforeCommit::Staged
        ),
        vec![1]
    );
    assert!(
        source_control_commit_save_prompt_ids(
            &buffers,
            &entries,
            GitPromptToSaveFilesBeforeCommit::Never
        )
        .is_empty()
    );
}

#[test]
fn source_control_commit_save_prompt_ids_include_smart_commit_scope() {
    let tracked_path = PathBuf::from("workspace/src/tracked.rs");
    let untracked_path = PathBuf::from("workspace/src/untracked.rs");
    let conflict_path = PathBuf::from("workspace/src/conflict.rs");
    let unrelated_path = PathBuf::from("workspace/src/unrelated.rs");
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
            path: conflict_path.clone(),
            status: GitFileStatus::Conflicted,
            stage: GitChangeStage::Unstaged,
        },
    ];
    let buffers = vec![
        dirty_test_buffer(1, Some(tracked_path)),
        dirty_test_buffer(2, Some(untracked_path)),
        dirty_test_buffer(3, Some(conflict_path)),
        dirty_test_buffer(4, Some(unrelated_path)),
        dirty_test_buffer(5, None),
    ];

    assert_eq!(
        source_control_commit_save_prompt_ids_for_commit(
            &buffers,
            &entries,
            GitPromptToSaveFilesBeforeCommit::Staged,
            Some(GitSmartCommitChanges::All)
        ),
        vec![1, 2]
    );
    assert_eq!(
        source_control_commit_save_prompt_ids_for_commit(
            &buffers,
            &entries,
            GitPromptToSaveFilesBeforeCommit::Staged,
            Some(GitSmartCommitChanges::Tracked)
        ),
        vec![1]
    );
    assert_eq!(
        source_control_commit_save_prompt_ids_for_commit(
            &buffers,
            &entries,
            GitPromptToSaveFilesBeforeCommit::Always,
            Some(GitSmartCommitChanges::Tracked)
        ),
        vec![1, 2, 3, 4, 5]
    );
}

#[test]
fn source_control_commit_save_prompt_copy_names_files() {
    let root = PathBuf::from("workspace");
    let buffers = vec![
        dirty_test_buffer(7, Some(root.join("src/main.rs"))),
        dirty_test_buffer(8, Some(root.join("src/lib.rs"))),
        dirty_test_buffer(9, Some(root.join("src/mod.rs"))),
    ];

    assert_eq!(
        source_control_commit_save_prompt_title(1),
        "1 file has unsaved changes"
    );
    assert_eq!(
        source_control_commit_save_prompt_title(2),
        "2 files have unsaved changes"
    );
    assert_eq!(
        source_control_commit_save_prompt_body(&buffers, &[7]),
        "Save main.rs before committing, commit anyway, or cancel."
    );
    assert_eq!(
        source_control_commit_save_prompt_body(&buffers, &[7, 8]),
        "Save main.rs and 1 other file before committing, commit anyway, or cancel."
    );
    assert_eq!(
        source_control_commit_save_prompt_body(&buffers, &[7, 8, 9]),
        "Save main.rs and 2 other files before committing, commit anyway, or cancel."
    );
    assert_eq!(
        source_control_stash_save_prompt_body(&buffers, &[7]),
        "Save main.rs before stashing, stash anyway, or cancel."
    );
    assert_eq!(
        source_control_stash_save_prompt_body(&buffers, &[7, 8]),
        "Save main.rs and 1 other file before stashing, stash anyway, or cancel."
    );
    assert_eq!(
        source_control_stash_save_prompt_body(&buffers, &[7, 8, 9]),
        "Save main.rs and 2 other files before stashing, stash anyway, or cancel."
    );
    assert_eq!(
        source_control_save_prompt_primary_label(1, "Commit"),
        "Save and Commit"
    );
    assert_eq!(
        source_control_save_prompt_primary_label(2, "Commit"),
        "Save All and Commit"
    );
}

#[test]
fn source_control_stage_statuses_identify_single_or_bulk_work() {
    let root = PathBuf::from("C:/repo");
    let file = root.join("src/main.rs");
    let files = vec![file.clone(), root.join("README.md")];

    assert_eq!(
        git_stage_pending_status(std::slice::from_ref(&file)),
        "Staging changes in main.rs"
    );
    assert_eq!(
        git_stage_success_status(std::slice::from_ref(&file)),
        "Staged changes in main.rs"
    );
    assert_eq!(
        git_stage_failure_status(std::slice::from_ref(&file), "permission denied"),
        "Could not stage changes in main.rs: permission denied"
    );
    assert_eq!(
        git_stage_pending_status(&files),
        "Staging changes in 2 files"
    );
    assert_eq!(
        git_stage_success_status(&files),
        "Staged changes in 2 files"
    );
    assert_eq!(
        git_stage_failure_status(&files, "index locked"),
        "Could not stage changes in 2 files: index locked"
    );
}

#[test]
fn source_control_unstage_statuses_identify_single_or_bulk_work() {
    let root = PathBuf::from("C:/repo");
    let file = root.join("src/main.rs");
    let files = vec![file.clone(), root.join("README.md")];

    assert_eq!(
        git_unstage_pending_status(std::slice::from_ref(&file)),
        "Unstaging changes in main.rs"
    );
    assert_eq!(
        git_unstage_success_status(std::slice::from_ref(&file)),
        "Unstaged changes in main.rs"
    );
    assert_eq!(
        git_unstage_failure_status(std::slice::from_ref(&file), "permission denied"),
        "Could not unstage changes in main.rs: permission denied"
    );
    assert_eq!(
        git_unstage_pending_status(&files),
        "Unstaging changes in 2 files"
    );
    assert_eq!(
        git_unstage_success_status(&files),
        "Unstaged changes in 2 files"
    );
    assert_eq!(
        git_unstage_failure_status(&files, "index locked"),
        "Could not unstage changes in 2 files: index locked"
    );
}

#[test]
fn source_control_discard_statuses_identify_single_or_bulk_work() {
    let root = PathBuf::from("C:/repo");
    let file = root.join("src/main.rs");
    let files = vec![file.clone(), root.join("README.md")];

    assert_eq!(
        git_discard_pending_status(std::slice::from_ref(&file)),
        "Discarding changes in main.rs"
    );
    assert_eq!(
        git_discard_success_status(std::slice::from_ref(&file)),
        "Discarded changes in main.rs"
    );
    assert_eq!(
        git_discard_failure_status(std::slice::from_ref(&file), "permission denied"),
        "Could not discard changes in main.rs: permission denied"
    );
    assert_eq!(
        git_discard_pending_status(&files),
        "Discarding changes in 2 files"
    );
    assert_eq!(
        git_discard_success_status(&files),
        "Discarded changes in 2 files"
    );
    assert_eq!(
        git_discard_failure_status(&files, "index locked"),
        "Could not discard changes in 2 files: index locked"
    );
}

#[test]
fn source_control_commit_statuses_match_commit_flow() {
    assert_eq!(
        git_progress_status(true, "Committing staged changes".to_owned()).as_deref(),
        Some("Committing staged changes")
    );
    assert_eq!(
        git_progress_status(false, "Committing staged changes".to_owned()),
        None
    );
    assert_eq!(
        git_commit_pending_status(false),
        "Committing staged changes"
    );
    assert_eq!(git_commit_pending_status(true), "Smart committing changes");
    assert_eq!(
        git_commit_success_status("12345678", false),
        "Committed staged changes (12345678)"
    );
    assert_eq!(
        git_commit_success_status("12345678", true),
        "Smart committed changes (12345678)"
    );
    assert_eq!(
        git_commit_failure_status("missing identity", false),
        "Could not commit staged changes: missing identity"
    );
    assert_eq!(
        git_commit_failure_status("missing identity", true),
        "Could not smart commit changes: missing identity"
    );
}

#[test]
fn source_control_mutation_restricted_status_names_action() {
    assert_eq!(
        source_control_mutation_restricted_status("staging changes"),
        "Trust this workspace before staging changes"
    );
}

#[test]
fn source_control_branch_protection_matches_exact_and_wildcard_patterns() {
    assert!(source_control_branch_protection_pattern_matches(
        "main", "main"
    ));
    assert!(source_control_branch_protection_pattern_matches(
        "release/*",
        "release/1.2"
    ));
    assert!(source_control_branch_protection_pattern_matches(
        "release/*/hotfix",
        "release/1.2/hotfix"
    ));
    assert!(source_control_branch_protection_pattern_matches(
        "*-stable",
        "main-stable"
    ));
    assert!(!source_control_branch_protection_pattern_matches(
        "release/*/hotfix",
        "release/1.2/hotfix/extra"
    ));
    assert!(!source_control_branch_protection_pattern_matches(
        "", "main"
    ));
}

#[test]
fn source_control_branch_protection_commit_action_follows_prompt_setting() {
    let patterns = vec!["main".to_owned(), "release/*".to_owned()];

    assert_eq!(
        source_control_protected_branch_commit_action(
            Some("feature/search"),
            &patterns,
            GitBranchProtectionPrompt::AlwaysPrompt,
        ),
        SourceControlProtectedBranchCommitAction::Allow
    );
    assert_eq!(
        source_control_protected_branch_commit_action(
            Some("main"),
            &patterns,
            GitBranchProtectionPrompt::AlwaysCommit,
        ),
        SourceControlProtectedBranchCommitAction::Allow
    );
    assert_eq!(
        source_control_protected_branch_commit_action(
            Some("main"),
            &patterns,
            GitBranchProtectionPrompt::AlwaysPrompt,
        ),
        SourceControlProtectedBranchCommitAction::Prompt {
            pattern: "main".to_owned()
        }
    );
    assert_eq!(
        source_control_protected_branch_commit_action(
            Some("release/1.2"),
            &patterns,
            GitBranchProtectionPrompt::AlwaysCommitToNewBranch,
        ),
        SourceControlProtectedBranchCommitAction::RequireNewBranch {
            pattern: "release/*".to_owned()
        }
    );
}

#[test]
fn source_control_branch_protection_prompt_copy_names_branch_and_pattern() {
    assert_eq!(
        source_control_protected_branch_prompt_title("main"),
        "Commit to protected branch main?"
    );
    assert_eq!(
        source_control_protected_branch_prompt_body("main", "main"),
        "Branch main matches protected branch pattern main."
    );
    assert_eq!(
        source_control_protected_branch_new_branch_required_status("main", "main"),
        "Branch main is protected by main; create or switch branches before committing"
    );
}
