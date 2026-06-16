use super::*;

#[test]
fn close_diff_on_operation_targets_matching_source_control_diffs() {
    let modified = PathBuf::from("C:/repo/src/main.rs");
    let staged = PathBuf::from("C:/repo/src/lib.rs");
    let other = PathBuf::from("C:/repo/src/other.rs");
    let sources = HashMap::from([
        (
            1,
            DiffBufferSource {
                path: modified.clone(),
                base_path: None,
                hunk_stage: Some(GitChangeStage::Unstaged),
                saved_buffer_id: None,
            },
        ),
        (
            2,
            DiffBufferSource {
                path: staged.clone(),
                base_path: None,
                hunk_stage: Some(GitChangeStage::Staged),
                saved_buffer_id: None,
            },
        ),
        (
            3,
            DiffBufferSource {
                path: other,
                base_path: None,
                hunk_stage: Some(GitChangeStage::Unstaged),
                saved_buffer_id: None,
            },
        ),
        (
            4,
            DiffBufferSource {
                path: modified.clone(),
                base_path: None,
                hunk_stage: None,
                saved_buffer_id: None,
            },
        ),
        (
            5,
            DiffBufferSource {
                path: modified.clone(),
                base_path: None,
                hunk_stage: Some(GitChangeStage::Staged),
                saved_buffer_id: None,
            },
        ),
    ]);

    assert_eq!(
        source_control_diff_buffers_for_operation(
            &sources,
            Some(std::slice::from_ref(&modified)),
            Some(GitChangeStage::Unstaged),
        ),
        vec![1]
    );
    assert_eq!(
        source_control_diff_buffers_for_operation(
            &sources,
            Some(std::slice::from_ref(&staged)),
            Some(GitChangeStage::Staged),
        ),
        vec![2]
    );
    assert_eq!(
        source_control_diff_buffers_for_operation(
            &sources,
            Some(std::slice::from_ref(&modified)),
            None,
        ),
        vec![1, 5]
    );
    assert_eq!(
        source_control_diff_buffers_for_operation(&sources, None, None),
        vec![1, 2, 3, 5]
    );
}

#[test]
fn source_control_open_all_stage_statuses_identify_stage_and_count() {
    assert_eq!(
        source_control_open_all_stage_empty_status(GitChangeStage::Unstaged),
        "No unstaged changes to open"
    );
    assert_eq!(
        source_control_open_all_stage_empty_status(GitChangeStage::Staged),
        "No staged changes to open"
    );
    assert_eq!(
        source_control_open_all_stage_success_status(GitChangeStage::Unstaged, 2),
        "Opened unstaged changes for 2 files"
    );
    assert_eq!(
        source_control_open_all_stage_success_status(GitChangeStage::Staged, 1),
        "Opened staged changes for 1 file"
    );
}

#[test]
fn source_control_stage_patch_copy_statuses_identify_stage_and_count() {
    assert_eq!(
        source_control_all_patch_copy_empty_status(),
        "No changes patch to copy"
    );
    assert_eq!(
        source_control_all_patch_copy_success_status(1),
        "Copied all changes patch for 1 change"
    );
    assert_eq!(
        source_control_all_patch_copy_success_status(3),
        "Copied all changes patch for 3 changes"
    );
    assert_eq!(
        source_control_all_patch_copy_failure_status("main.rs: denied"),
        "Could not copy all changes patch: main.rs: denied"
    );
    assert_eq!(
        source_control_stage_patch_copy_empty_status(GitChangeStage::Unstaged),
        "No unstaged patch to copy"
    );
    assert_eq!(
        source_control_stage_patch_copy_empty_status(GitChangeStage::Staged),
        "No staged patch to copy"
    );
    assert_eq!(
        source_control_stage_patch_copy_success_status(GitChangeStage::Unstaged, 2),
        "Copied unstaged patch for 2 files"
    );
    assert_eq!(
        source_control_stage_patch_copy_success_status(GitChangeStage::Staged, 1),
        "Copied staged patch for 1 file"
    );
    assert_eq!(
        source_control_stage_patch_copy_failure_status(GitChangeStage::Staged, "main.rs: denied"),
        "Could not copy staged patch: main.rs: denied"
    );
}

#[test]
fn source_control_patch_copy_runtime_statuses_identify_async_request_targets() {
    let file = PathBuf::from("workspace/main.rs");
    let file_request = SourceControlPatchCopyRequest::File {
        path: file.clone(),
        stage: GitChangeStage::Staged,
    };
    assert_eq!(
        source_control_patch_copy_pending_status(&file_request),
        "Preparing staged patch for main.rs"
    );
    assert_eq!(source_control_patch_copy_detail(&file_request), "main.rs");
    assert_eq!(
        source_control_patch_copy_success_status_for_request(&file_request, 1),
        "Copied staged patch for main.rs"
    );
    assert_eq!(
        source_control_patch_copy_failure_status_for_request(&file_request, "denied"),
        "Could not copy staged patch for main.rs: denied"
    );

    let stage_request = SourceControlPatchCopyRequest::Stage {
        stage: GitChangeStage::Unstaged,
    };
    assert_eq!(
        source_control_patch_copy_pending_status(&stage_request),
        "Preparing unstaged patch"
    );
    assert_eq!(
        source_control_patch_copy_empty_status_for_request(&stage_request),
        "No unstaged patch to copy"
    );
    assert_eq!(
        source_control_patch_copy_success_status_for_request(&stage_request, 2),
        "Copied unstaged patch for 2 files"
    );

    let hunk_request = SourceControlPatchCopyRequest::Hunk {
        path: file,
        stage: GitChangeStage::Unstaged,
        hunk_index: 3,
    };
    assert_eq!(
        source_control_patch_copy_pending_status(&hunk_request),
        "Preparing hunk 3 patch for main.rs"
    );
    assert_eq!(
        source_control_patch_copy_empty_status_for_request(&hunk_request),
        "No unstaged hunk 3 patch to copy for main.rs"
    );

    let commit = GitCommitSummary {
        oid: "1234567890abcdef".to_owned(),
        short_oid: "12345678".to_owned(),
        summary: "Fix bug".to_owned(),
        author: "Ada".to_owned(),
        time_seconds: 10,
    };
    let commit_request = SourceControlPatchCopyRequest::Commit { commit };
    assert_eq!(
        source_control_patch_copy_pending_status(&commit_request),
        "Preparing patch for commit 12345678"
    );
    assert_eq!(
        source_control_patch_copy_detail(&commit_request),
        "commit 12345678"
    );

    let stash_request = SourceControlPatchCopyRequest::Stash {
        stash: GitStashEntry {
            index: 2,
            short_oid: "abcdef12".to_owned(),
            message: "WIP".to_owned(),
        },
    };
    assert_eq!(
        source_control_patch_copy_pending_status(&stash_request),
        "Preparing patch for stash@{2}"
    );
    assert_eq!(
        source_control_patch_copy_detail(&stash_request),
        "stash@{2}"
    );

    assert_eq!(
        source_control_patch_copy_empty_status_for_request(&SourceControlPatchCopyRequest::All),
        "No changes patch to copy"
    );
}

#[test]
fn join_unified_patches_separates_files_and_skips_empty_patches() {
    assert_eq!(
        join_unified_patches(vec![
            "diff --git a/src/a.rs b/src/a.rs\n@@ -1 +1 @@\n-old\n+new\n\n".to_owned(),
            "\n".to_owned(),
            "diff --git a/src/b.rs b/src/b.rs\n@@ -1 +1 @@\n-one\n+two\n".to_owned(),
        ]),
        "diff --git a/src/a.rs b/src/a.rs\n@@ -1 +1 @@\n-old\n+new\ndiff --git a/src/b.rs b/src/b.rs\n@@ -1 +1 @@\n-one\n+two"
    );
}

#[test]
fn source_control_patch_copy_statuses_identify_stage_and_path() {
    let root = PathBuf::from("C:/repo");
    let file = root.join("src/main.rs");

    assert_eq!(
        source_control_patch_copy_success_status(GitChangeStage::Unstaged, &file),
        "Copied patch for main.rs"
    );
    assert_eq!(
        source_control_patch_copy_success_status(GitChangeStage::Staged, &file),
        "Copied staged patch for main.rs"
    );
    assert_eq!(
        source_control_patch_copy_empty_status(GitChangeStage::Unstaged, &file),
        "No patch to copy for main.rs"
    );
    assert_eq!(
        source_control_patch_copy_empty_status(GitChangeStage::Staged, &file),
        "No staged patch to copy for main.rs"
    );
    assert_eq!(
        source_control_patch_copy_failure_status(GitChangeStage::Unstaged, &file, "diff failed"),
        "Could not copy patch for main.rs: diff failed"
    );
    assert_eq!(
        source_control_patch_copy_failure_status(GitChangeStage::Staged, &file, "diff failed"),
        "Could not copy staged patch for main.rs: diff failed"
    );
}

#[test]
fn source_control_diff_statuses_sanitize_paths_labels_and_errors() {
    let file = PathBuf::from("C:/repo/src").join(format!(
        "bad\n{}\u{202e}tail.rs",
        "very-long-component-".repeat(16)
    ));
    let error = format!("first line\n{}\u{2066}tail", "error-detail-".repeat(24));

    let status = source_control_patch_copy_failure_status(GitChangeStage::Unstaged, &file, &error);

    assert!(status.starts_with("Could not copy patch for bad "));
    assert!(!status.contains('\n'));
    assert!(!status.contains('\u{202e}'));
    assert!(!status.contains('\u{2066}'));
    assert!(status.contains("..."));
    assert!(
        status.chars().count()
            <= "Could not copy patch for : ".chars().count()
                + DISPLAY_PATH_LABEL_MAX_CHARS
                + DISPLAY_ERROR_LABEL_MAX_CHARS
    );

    let label = format!("diff\n{}\u{202e}view", "very-long-label-".repeat(16));
    let status = source_control_diff_hunk_patch_copy_no_hunk_status(&label, 9);

    assert!(status.starts_with("No diff hunk at diff "));
    assert!(!status.contains('\n'));
    assert!(!status.contains('\u{202e}'));
    assert!(status.contains("..."));
    assert!(
        status
            .trim_start_matches("No diff hunk at ")
            .trim_end_matches(":9")
            .chars()
            .count()
            <= DISPLAY_PATH_LABEL_MAX_CHARS
    );
}

#[test]
fn source_control_commit_and_stash_patch_copy_statuses_identify_targets() {
    let commit = GitCommitSummary {
        oid: "12345678bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned(),
        short_oid: "12345678".to_owned(),
        summary: "Add search panel".to_owned(),
        author: "Kuroya Test".to_owned(),
        time_seconds: 10,
    };
    let stash = GitStashEntry {
        index: 2,
        short_oid: "abcdef12".to_owned(),
        message: "On main: work in progress".to_owned(),
    };

    assert_eq!(
        source_control_commit_patch_copy_success_status(&commit),
        "Copied patch for commit 12345678"
    );
    assert_eq!(
        source_control_commit_patch_copy_empty_status(&commit),
        "No patch to copy for commit 12345678"
    );
    assert_eq!(
        source_control_commit_patch_copy_failure_status(&commit, "diff failed"),
        "Could not copy patch for commit 12345678: diff failed"
    );
    assert_eq!(
        source_control_stash_patch_copy_success_status(&stash),
        "Copied patch for stash@{2}"
    );
    assert_eq!(
        source_control_stash_patch_copy_empty_status(&stash),
        "No patch to copy for stash@{2}"
    );
    assert_eq!(
        source_control_stash_patch_copy_failure_status(&stash, "diff failed"),
        "Could not copy patch for stash@{2}: diff failed"
    );
    assert_eq!(
        source_control_diff_buffer_patch_copy_success_status("main.rs (Changes)"),
        "Copied patch from main.rs (Changes)"
    );
    assert_eq!(
        source_control_diff_buffer_patch_copy_empty_status("main.rs (Changes)"),
        "No patch to copy from main.rs (Changes)"
    );
    assert_eq!(
        source_control_diff_buffer_patch_copy_unavailable_status(),
        "No diff patch to copy"
    );
    assert_eq!(
        source_control_diff_hunk_patch_copy_success_status("main.rs (Changes)", 2),
        "Copied hunk 2 patch from main.rs (Changes)"
    );
    assert_eq!(
        source_control_diff_hunk_patch_copy_empty_status("main.rs (Changes)", 2),
        "No hunk 2 patch to copy from main.rs (Changes)"
    );
    assert_eq!(
        source_control_diff_hunk_patch_copy_no_hunk_status("main.rs (Changes)", 9),
        "No diff hunk at main.rs (Changes):9"
    );
    assert_eq!(
        source_control_diff_hunk_patch_copy_unavailable_status(),
        "No diff hunk patch to copy"
    );
    assert_eq!(
        source_control_diff_refresh_unavailable_status(),
        "No refreshable diff to update"
    );
    assert_eq!(
        source_control_diff_source_open_unavailable_status(),
        "No diff source file to open"
    );
    assert_eq!(
        source_control_diff_base_open_unavailable_status(),
        "No diff base file to open"
    );
    let file = PathBuf::from("C:/repo/src/main.rs");
    assert_eq!(
        source_control_diff_base_open_missing_status(&file),
        "No base file for diff main.rs"
    );
    assert_eq!(
        source_control_head_revision_missing_status(&file),
        "No HEAD revision for main.rs"
    );
    assert_eq!(
        source_control_head_revision_failure_status(&file, "no HEAD"),
        "Could not open HEAD revision for main.rs: no HEAD"
    );
    assert_eq!(
        source_control_index_revision_missing_status(&file),
        "No index revision for main.rs"
    );
    assert_eq!(
        source_control_index_revision_failure_status(&file, "no index"),
        "Could not open index revision for main.rs: no index"
    );
    assert_eq!(
        source_control_diff_hunk_source_open_success_status("main.rs (Changes)", &file, 2, 14),
        "Opened source for hunk 2 from main.rs (Changes) at main.rs:14"
    );
    assert_eq!(
        source_control_diff_hunk_source_open_no_hunk_status("main.rs (Changes)", 9),
        "No diff hunk at main.rs (Changes):9"
    );
    assert_eq!(
        source_control_diff_hunk_source_open_missing_hunk_status("main.rs (Changes)", 2),
        "Could not find hunk 2 source line in main.rs (Changes)"
    );
    assert_eq!(
        source_control_diff_hunk_source_open_missing_status(&file),
        "No source file for diff main.rs"
    );
    assert_eq!(
        source_control_diff_hunk_source_open_unavailable_status(),
        "No diff hunk source to open"
    );
    assert_eq!(
        source_control_diff_hunk_base_open_success_status("main.rs (Changes)", &file, 2, 8),
        "Opened base for hunk 2 from main.rs (Changes) at main.rs:8"
    );
    assert_eq!(
        source_control_diff_hunk_base_open_no_hunk_status("main.rs (Changes)", 9),
        "No diff hunk at main.rs (Changes):9"
    );
    assert_eq!(
        source_control_diff_hunk_base_open_missing_hunk_status("main.rs (Changes)", 2),
        "Could not find hunk 2 base line in main.rs (Changes)"
    );
    assert_eq!(
        source_control_diff_hunk_base_open_unavailable_status(),
        "No diff hunk base to open"
    );
}

#[test]
fn source_control_hunk_patch_copy_statuses_identify_stage_path_and_hunk() {
    let root = PathBuf::from("C:/repo");
    let file = root.join("src/main.rs");

    assert_eq!(
        source_control_hunk_patch_copy_success_status(GitChangeStage::Unstaged, &file, 1),
        "Copied unstaged hunk 1 patch for main.rs"
    );
    assert_eq!(
        source_control_hunk_patch_copy_success_status(GitChangeStage::Staged, &file, 1),
        "Copied staged hunk 1 patch for main.rs"
    );
    assert_eq!(
        source_control_hunk_patch_copy_empty_status(GitChangeStage::Unstaged, &file, 1),
        "No unstaged hunk 1 patch to copy for main.rs"
    );
    assert_eq!(
        source_control_hunk_patch_copy_failure_status(
            GitChangeStage::Staged,
            &file,
            1,
            "diff failed"
        ),
        "Could not copy staged hunk 1 patch for main.rs: diff failed"
    );
}

#[test]
fn source_control_hunk_diff_open_statuses_identify_stage_path_and_hunk() {
    let root = PathBuf::from("C:/repo");
    let file = root.join("src/main.rs");

    assert_eq!(
        source_control_hunk_diff_open_success_status(
            GitChangeStage::Unstaged,
            "main.rs (Changes)",
            1,
            9,
        ),
        "Opened unstaged hunk 1 in main.rs (Changes):9"
    );
    assert_eq!(
        source_control_hunk_diff_open_success_status(
            GitChangeStage::Staged,
            "main.rs (Staged Changes)",
            2,
            12,
        ),
        "Opened staged hunk 2 in main.rs (Staged Changes):12"
    );
    assert_eq!(
        source_control_hunk_diff_open_missing_status(GitChangeStage::Unstaged, &file, 3),
        "Could not find unstaged hunk 3 in main.rs"
    );
}

#[test]
fn source_control_hunk_source_open_statuses_identify_stage_path_hunk_and_line() {
    let root = PathBuf::from("C:/repo");
    let file = root.join("src/main.rs");

    assert_eq!(
        source_control_hunk_source_open_success_status(GitChangeStage::Unstaged, &file, 1, 12),
        "Opened unstaged hunk 1 source at main.rs:12"
    );
    assert_eq!(
        source_control_hunk_source_open_success_status(GitChangeStage::Staged, &file, 2, 5),
        "Opened staged hunk 2 source at main.rs:5"
    );
    assert_eq!(
        source_control_hunk_source_open_missing_status(GitChangeStage::Staged, &file, 3),
        "No source file for staged hunk 3 in main.rs"
    );
}

#[test]
fn hunk_header_line_in_unified_diff_tracks_target_hunk() {
    let diff = concat!(
        "diff --git a/src/main.rs b/src/main.rs\n",
        "--- a/src/main.rs\n",
        "+++ b/src/main.rs\n",
        "@@ -1,2 +1,2 @@\n",
        " one\n",
        "-two\n",
        "+dos\n",
        "@@ -9,2 +9,3 @@\n",
        " nine\n",
        "+ten\n",
        "diff --git a/src/lib.rs b/src/lib.rs\n",
        "--- a/src/lib.rs\n",
        "+++ b/src/lib.rs\n",
        "@@@ -3,1 -3,1 +3,1 @@@\n",
        "-old\n",
        "+new\n",
    );

    assert_eq!(hunk_header_line_in_unified_diff(diff, 0), Some(4));
    assert_eq!(hunk_header_line_in_unified_diff(diff, 1), Some(8));
    assert_eq!(hunk_header_line_in_unified_diff(diff, 2), Some(14));
    assert_eq!(hunk_header_line_in_unified_diff(diff, 3), None);
}

#[test]
fn hunk_modified_start_line_in_unified_diff_tracks_target_hunk_source_line() {
    let diff = concat!(
        "diff --git a/src/main.rs b/src/main.rs\n",
        "--- a/src/main.rs\n",
        "+++ b/src/main.rs\n",
        "@@ -1,2 +1,2 @@\n",
        " one\n",
        "-two\n",
        "+dos\n",
        "@@ -9,2 +12,3 @@\n",
        " nine\n",
        "+ten\n",
        "diff --git a/src/lib.rs b/src/lib.rs\n",
        "--- a/src/lib.rs\n",
        "+++ b/src/lib.rs\n",
        "@@@ -3,1 -3,1 +7,1 @@@\n",
        "-old\n",
        "+new\n",
        "diff --git a/src/new.rs b/src/new.rs\n",
        "--- /dev/null\n",
        "+++ b/src/new.rs\n",
        "@@ -0,0 +1,1 @@\n",
    );

    assert_eq!(hunk_modified_start_line_in_unified_diff(diff, 0), Some(1));
    assert_eq!(hunk_modified_start_line_in_unified_diff(diff, 1), Some(12));
    assert_eq!(hunk_modified_start_line_in_unified_diff(diff, 2), Some(7));
    assert_eq!(hunk_modified_start_line_in_unified_diff(diff, 3), Some(1));
    assert_eq!(hunk_modified_start_line_in_unified_diff(diff, 4), None);
}

#[test]
fn hunk_original_start_line_in_unified_diff_tracks_target_hunk_base_line() {
    let diff = concat!(
        "diff --git a/src/main.rs b/src/main.rs\n",
        "--- a/src/main.rs\n",
        "+++ b/src/main.rs\n",
        "@@ -1,2 +1,2 @@\n",
        " one\n",
        "-two\n",
        "+dos\n",
        "@@ -9,2 +12,3 @@\n",
        " nine\n",
        "+ten\n",
        "diff --git a/src/lib.rs b/src/lib.rs\n",
        "--- a/src/lib.rs\n",
        "+++ b/src/lib.rs\n",
        "@@@ -3,1 -5,1 +7,1 @@@\n",
        "-old\n",
        "+new\n",
        "diff --git a/src/new.rs b/src/new.rs\n",
        "--- /dev/null\n",
        "+++ b/src/new.rs\n",
        "@@ -0,0 +1,3 @@\n",
    );

    assert_eq!(hunk_original_start_line_in_unified_diff(diff, 0), Some(1));
    assert_eq!(hunk_original_start_line_in_unified_diff(diff, 1), Some(9));
    assert_eq!(hunk_original_start_line_in_unified_diff(diff, 2), Some(5));
    assert_eq!(hunk_original_start_line_in_unified_diff(diff, 3), Some(1));
    assert_eq!(hunk_original_start_line_in_unified_diff(diff, 4), None);
}

#[test]
fn hunk_patch_from_unified_diff_extracts_target_hunk_with_file_header() {
    let diff = concat!(
        "diff --git a/src/main.rs b/src/main.rs\n",
        "--- a/src/main.rs\n",
        "+++ b/src/main.rs\n",
        "@@ -1,2 +1,2 @@\n",
        " one\n",
        "-two\n",
        "+dos\n",
        "@@ -9,2 +9,3 @@\n",
        " nine\n",
        "+ten\n",
        "diff --git a/src/lib.rs b/src/lib.rs\n",
        "--- a/src/lib.rs\n",
        "+++ b/src/lib.rs\n",
        "@@ -3,1 +3,1 @@\n",
        "-old\n",
        "+new\n",
    );

    assert_eq!(
        hunk_patch_from_unified_diff(diff, 1).as_deref(),
        Some(concat!(
            "diff --git a/src/main.rs b/src/main.rs\n",
            "--- a/src/main.rs\n",
            "+++ b/src/main.rs\n",
            "@@ -9,2 +9,3 @@\n",
            " nine\n",
            "+ten\n",
        ))
    );
    assert_eq!(
        hunk_patch_from_unified_diff(diff, 2).as_deref(),
        Some(concat!(
            "diff --git a/src/lib.rs b/src/lib.rs\n",
            "--- a/src/lib.rs\n",
            "+++ b/src/lib.rs\n",
            "@@ -3,1 +3,1 @@\n",
            "-old\n",
            "+new\n",
        ))
    );
    assert!(hunk_patch_from_unified_diff(diff, 3).is_none());
}

#[test]
fn source_control_hunk_label_includes_index_header_and_counts() {
    let hunk = GitDiffHunk {
        index: 3,
        fingerprint: 3,
        old_start: 10,
        old_lines: 4,
        new_start: 10,
        new_lines: 5,
        additions: 2,
        deletions: 1,
        header: "@@ -10,4 +10,5 @@".to_owned(),
    };

    assert_eq!(
        source_control_hunk_label(&hunk),
        "#3  @@ -10,4 +10,5 @@  +2 -1"
    );
}

#[test]
fn source_control_hunk_source_line_uses_diff_new_start() {
    let hunk = GitDiffHunk {
        index: 3,
        fingerprint: 3,
        old_start: 10,
        old_lines: 4,
        new_start: 12,
        new_lines: 5,
        additions: 2,
        deletions: 1,
        header: "@@ -10,4 +12,5 @@".to_owned(),
    };
    let empty_new_file_hunk = GitDiffHunk {
        index: 0,
        fingerprint: 0,
        old_start: 0,
        old_lines: 0,
        new_start: 0,
        new_lines: 0,
        additions: 0,
        deletions: 0,
        header: "@@ -0,0 +0,0 @@".to_owned(),
    };

    assert_eq!(source_control_hunk_source_line(&hunk), 12);
    assert_eq!(source_control_hunk_source_line(&empty_new_file_hunk), 1);
}

#[test]
fn source_control_hunk_keyboard_actions_match_stage() {
    assert_eq!(
        source_control_hunk_keyboard_action_labels(GitChangeStage::Unstaged),
        vec![
            "Enter Stage Hunk",
            "O Open File at Hunk",
            "D Open Hunk Diff",
            "P Copy Hunk Patch",
            "Delete Discard Hunk"
        ]
    );
    assert_eq!(
        source_control_hunk_keyboard_action_labels(GitChangeStage::Staged),
        vec![
            "Enter Unstage Hunk",
            "O Open File at Hunk",
            "D Open Hunk Diff",
            "P Copy Hunk Patch"
        ]
    );
    assert_eq!(
        source_control_hunk_panel_action_labels(GitChangeStage::Unstaged),
        vec![
            "Stage Hunk",
            "Open File at Hunk",
            "Open Hunk Diff",
            "Copy Hunk Patch",
            "Discard Hunk"
        ]
    );
    assert_eq!(
        source_control_hunk_panel_action_labels(GitChangeStage::Staged),
        vec![
            "Unstage Hunk",
            "Open File at Hunk",
            "Open Hunk Diff",
            "Copy Hunk Patch"
        ]
    );
    assert_eq!(
        source_control_hunk_panel_action_tooltips(GitChangeStage::Unstaged),
        vec![
            "Stage Hunk (Enter)",
            "Open File at Hunk (O)",
            "Open Hunk Diff (D)",
            "Copy Hunk Patch (P)",
            "Discard Hunk (Delete)"
        ]
    );
    assert_eq!(
        source_control_hunk_panel_action_tooltips(GitChangeStage::Staged),
        vec![
            "Unstage Hunk (Enter)",
            "Open File at Hunk (O)",
            "Open Hunk Diff (D)",
            "Copy Hunk Patch (P)"
        ]
    );
}

#[test]
fn worktree_hunk_index_tracks_cursor_line_in_hunk_ranges() {
    let hunks = vec![
        GitDiffHunk {
            index: 0,
            fingerprint: 0,
            old_start: 2,
            old_lines: 3,
            new_start: 2,
            new_lines: 4,
            additions: 1,
            deletions: 0,
            header: "@@ -2,3 +2,4 @@".to_owned(),
        },
        GitDiffHunk {
            index: 1,
            fingerprint: 1,
            old_start: 12,
            old_lines: 2,
            new_start: 12,
            new_lines: 0,
            additions: 0,
            deletions: 2,
            header: "@@ -12,2 +12,0 @@".to_owned(),
        },
    ];

    assert_eq!(worktree_hunk_index_at_line(&hunks, 2), Some(0));
    assert_eq!(worktree_hunk_index_at_line(&hunks, 5), Some(0));
    assert_eq!(worktree_hunk_index_at_line(&hunks, 12), Some(1));
    assert_eq!(worktree_hunk_index_at_line(&hunks, 6), None);
    assert_eq!(git_hunk_index_at_new_line(&hunks, 12), Some(1));
}

#[test]
fn accessible_diff_label_is_idempotent() {
    assert_eq!(
        accessible_diff_label("main.rs (Changes)"),
        "main.rs (Changes) (Accessible Diff)"
    );
    assert_eq!(
        accessible_diff_label("main.rs (Changes) (Accessible Diff)"),
        "main.rs (Changes) (Accessible Diff)"
    );
}

#[test]
fn accessible_diff_labels_are_sanitized_and_bounded() {
    let label = accessible_diff_label(&format!(
        "diff\n{}\u{202e}view",
        "very-long-label-".repeat(16)
    ));

    assert!(!label.contains('\n'));
    assert!(!label.contains('\u{202e}'));
    assert!(label.contains("..."));
    assert!(label.ends_with(" (Accessible Diff)"));
    assert!(
        label.trim_end_matches(" (Accessible Diff)").chars().count()
            <= DISPLAY_PATH_LABEL_MAX_CHARS
    );
    assert_eq!(
        accessible_diff_label("\n\u{202e}\u{0007}"),
        "diff (Accessible Diff)"
    );
}

#[test]
fn accessible_diff_setting_controls_diff_buffer_presentation() {
    assert_eq!(
        diff_buffer_display_label("main.rs (Changes)".to_owned(), false),
        "main.rs (Changes)"
    );
    assert_eq!(
        diff_buffer_display_label("main.rs (Changes)".to_owned(), true),
        "main.rs (Changes) (Accessible Diff)"
    );
    assert_eq!(
        diff_buffer_display_label("main.rs (Changes) (Accessible Diff)".to_owned(), true),
        "main.rs (Changes) (Accessible Diff)"
    );
    assert_eq!(diff_buffer_display_kind("changes", false), "changes");
    assert_eq!(diff_buffer_display_kind("changes", true), "accessible diff");
}

#[test]
fn updating_existing_virtual_diff_buffer_clears_stale_pending_scroll_target() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_source_control_test(root);
    let source = DiffBufferSource {
        path,
        base_path: None,
        hunk_stage: Some(GitChangeStage::Unstaged),
        saved_buffer_id: None,
    };
    let first_diff = "\
diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,1 +1,1 @@
-old
+new
";
    let second_diff = "\
diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,1 +1,1 @@
-before
+after
";

    let id = app.open_virtual_diff_buffer(
        "main.rs (Changes)".to_owned(),
        first_diff.to_owned(),
        "main.rs".to_owned(),
        "changes",
        Some(source.clone()),
    );
    app.pending_scroll_lines.insert(id, 200);

    let reused_id = app.open_virtual_diff_buffer(
        "main.rs (Changes)".to_owned(),
        second_diff.to_owned(),
        "main.rs".to_owned(),
        "changes",
        Some(source),
    );

    assert_eq!(reused_id, id);
    assert!(!app.pending_scroll_lines.contains_key(&id));
    assert_eq!(app.buffer(id).unwrap().text(), second_diff);
}

#[test]
fn open_active_diff_hunk_source_reuses_open_missing_source_buffer() {
    let root = PathBuf::from(format!(
        "missing-source-control-workspace-{}",
        std::process::id()
    ));
    let path = root.join("src/main.rs");
    let mut app = app_for_source_control_test(root);
    app.buffers.push(TextBuffer::from_text(
        7,
        Some(path.clone()),
        "new\nsecond\n".to_owned(),
    ));
    let diff = "\
diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,1 +1,1 @@
-old
+new
";
    let source = DiffBufferSource {
        path: path.clone(),
        base_path: None,
        hunk_stage: Some(GitChangeStage::Unstaged),
        saved_buffer_id: None,
    };
    let id = app.open_virtual_diff_buffer(
        "main.rs (Changes)".to_owned(),
        diff.to_owned(),
        "main.rs".to_owned(),
        "changes",
        Some(source),
    );
    let cursor = app
        .buffer(id)
        .expect("diff buffer")
        .line_column_to_char(4, 0);
    app.buffer_mut(id)
        .expect("diff buffer")
        .set_single_cursor(cursor);
    app.set_active_buffer(id);

    assert!(!path.exists());
    app.open_active_diff_hunk_source();

    assert_eq!(app.active, Some(7));
    assert!(app.pending_open_paths.is_empty());
    assert_eq!(
        app.buffer(7).expect("source buffer").cursor_position().line,
        0
    );
    assert_eq!(
        app.status,
        source_control_diff_hunk_source_open_success_status("main.rs (Changes)", &path, 0, 1)
    );
}

#[test]
fn active_diff_hunk_discard_requires_current_hunk_fingerprint() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_source_control_test(root.clone());
    let diff = "\
diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,1 +1,1 @@
-old
+new
";
    let source = DiffBufferSource {
        path: path.clone(),
        base_path: None,
        hunk_stage: Some(GitChangeStage::Unstaged),
        saved_buffer_id: None,
    };
    let id = app.open_virtual_diff_buffer(
        "main.rs (Changes)".to_owned(),
        diff.to_owned(),
        "main.rs".to_owned(),
        "changes",
        Some(source),
    );
    let cursor = app
        .buffer(id)
        .expect("diff buffer")
        .line_column_to_char(4, 0);
    app.buffer_mut(id)
        .expect("diff buffer")
        .set_single_cursor(cursor);

    app.discard_active_diff_hunk();

    assert_eq!(
        app.status,
        source_control_diff_hunk_discard_stale_status(&path, 0)
    );
}

#[test]
fn active_diff_hunk_stage_requires_current_hunk_fingerprint() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_source_control_test(root.clone());
    let diff = "\
diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,1 +1,1 @@
-old
+new
";
    let source = DiffBufferSource {
        path: path.clone(),
        base_path: None,
        hunk_stage: Some(GitChangeStage::Unstaged),
        saved_buffer_id: None,
    };
    let id = app.open_virtual_diff_buffer(
        "main.rs (Changes)".to_owned(),
        diff.to_owned(),
        "main.rs".to_owned(),
        "changes",
        Some(source),
    );
    let cursor = app
        .buffer(id)
        .expect("diff buffer")
        .line_column_to_char(4, 0);
    app.buffer_mut(id)
        .expect("diff buffer")
        .set_single_cursor(cursor);

    app.stage_active_diff_hunk();

    assert_eq!(
        app.status,
        source_control_diff_hunk_identity_stale_status("staging", &path, 0)
    );
}

#[test]
fn active_diff_hunk_unstage_requires_current_hunk_fingerprint() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_source_control_test(root.clone());
    let diff = "\
diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,1 +1,1 @@
-old
+new
";
    let source = DiffBufferSource {
        path: path.clone(),
        base_path: None,
        hunk_stage: Some(GitChangeStage::Staged),
        saved_buffer_id: None,
    };
    let id = app.open_virtual_diff_buffer(
        "main.rs (Staged Changes)".to_owned(),
        diff.to_owned(),
        "main.rs".to_owned(),
        "staged changes",
        Some(source),
    );
    let cursor = app
        .buffer(id)
        .expect("diff buffer")
        .line_column_to_char(4, 0);
    app.buffer_mut(id)
        .expect("diff buffer")
        .set_single_cursor(cursor);

    app.unstage_active_diff_hunk();

    assert_eq!(
        app.status,
        source_control_diff_hunk_identity_stale_status("unstaging", &path, 0)
    );
}

#[test]
fn active_diff_hunk_stage_uses_current_cached_fingerprint() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_source_control_test(root.clone());
    let diff = "\
diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,1 +1,1 @@
-old
+new
";
    let source = DiffBufferSource {
        path: path.clone(),
        base_path: None,
        hunk_stage: Some(GitChangeStage::Unstaged),
        saved_buffer_id: None,
    };
    let id = app.open_virtual_diff_buffer(
        "main.rs (Changes)".to_owned(),
        diff.to_owned(),
        "main.rs".to_owned(),
        "changes",
        Some(source),
    );
    let cursor = app
        .buffer(id)
        .expect("diff buffer")
        .line_column_to_char(4, 0);
    app.buffer_mut(id)
        .expect("diff buffer")
        .set_single_cursor(cursor);
    app.source_control_hunks_open = true;
    app.source_control_hunk_path = Some(path.clone());
    app.source_control_hunk_stage = GitChangeStage::Unstaged;
    app.source_control_hunks = vec![GitDiffHunk {
        index: 0,
        fingerprint: 99,
        old_start: 1,
        old_lines: 1,
        new_start: 1,
        new_lines: 1,
        additions: 1,
        deletions: 1,
        header: "@@ -1,1 +1,1 @@".to_owned(),
    }];

    app.stage_active_diff_hunk();

    assert_eq!(app.status, git_hunk_stage_pending_status(&path, 0));
    assert!(app.command_bus.is_empty());
}

#[test]
fn active_diff_hunk_unstage_uses_current_cached_fingerprint() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_source_control_test(root.clone());
    let diff = "\
diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,1 +1,1 @@
-old
+new
";
    let source = DiffBufferSource {
        path: path.clone(),
        base_path: None,
        hunk_stage: Some(GitChangeStage::Staged),
        saved_buffer_id: None,
    };
    let id = app.open_virtual_diff_buffer(
        "main.rs (Staged Changes)".to_owned(),
        diff.to_owned(),
        "main.rs".to_owned(),
        "staged changes",
        Some(source),
    );
    let cursor = app
        .buffer(id)
        .expect("diff buffer")
        .line_column_to_char(4, 0);
    app.buffer_mut(id)
        .expect("diff buffer")
        .set_single_cursor(cursor);
    app.source_control_hunks_open = true;
    app.source_control_hunk_path = Some(path.clone());
    app.source_control_hunk_stage = GitChangeStage::Staged;
    app.source_control_hunks = vec![GitDiffHunk {
        index: 0,
        fingerprint: 99,
        old_start: 1,
        old_lines: 1,
        new_start: 1,
        new_lines: 1,
        additions: 1,
        deletions: 1,
        header: "@@ -1,1 +1,1 @@".to_owned(),
    }];

    app.unstage_active_diff_hunk();

    assert_eq!(app.status, git_hunk_unstage_pending_status(&path, 0));
    assert!(app.command_bus.is_empty());
}

#[test]
fn discard_file_hunk_without_fingerprint_is_rejected() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_source_control_test(root.clone());

    app.run_command(Command::DiscardFileHunk {
        path: path.clone(),
        hunk_index: 4,
        hunk_fingerprint: None,
    });

    assert_eq!(
        app.status,
        git_hunk_discard_missing_identity_status(&path, 4)
    );
}

#[test]
fn stage_file_hunk_without_fingerprint_is_rejected() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_source_control_test(root.clone());

    app.run_command(Command::StageFileHunk {
        path: path.clone(),
        hunk_index: 4,
        hunk_fingerprint: None,
    });

    assert_eq!(app.status, git_hunk_stage_missing_identity_status(&path, 4));
    assert_eq!(app.source_control_hunk_path.as_ref(), Some(&path));
    assert_eq!(app.source_control_hunk_stage, GitChangeStage::Unstaged);
}

#[test]
fn unstage_file_hunk_without_fingerprint_is_rejected() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_source_control_test(root.clone());

    app.run_command(Command::UnstageFileHunk {
        path: path.clone(),
        hunk_index: 4,
        hunk_fingerprint: None,
    });

    assert_eq!(
        app.status,
        git_hunk_unstage_missing_identity_status(&path, 4)
    );
    assert_eq!(app.source_control_hunk_path.as_ref(), Some(&path));
    assert_eq!(app.source_control_hunk_stage, GitChangeStage::Staged);
}

#[test]
fn active_diff_hunk_discard_uses_current_cached_fingerprint() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = app_for_source_control_test(root.clone());
    let diff = "\
diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,1 +1,1 @@
-old
+new
";
    let source = DiffBufferSource {
        path: path.clone(),
        base_path: None,
        hunk_stage: Some(GitChangeStage::Unstaged),
        saved_buffer_id: None,
    };
    let id = app.open_virtual_diff_buffer(
        "main.rs (Changes)".to_owned(),
        diff.to_owned(),
        "main.rs".to_owned(),
        "changes",
        Some(source),
    );
    let cursor = app
        .buffer(id)
        .expect("diff buffer")
        .line_column_to_char(4, 0);
    app.buffer_mut(id)
        .expect("diff buffer")
        .set_single_cursor(cursor);
    app.source_control_hunks_open = true;
    app.source_control_hunk_path = Some(path.clone());
    app.source_control_hunk_stage = GitChangeStage::Unstaged;
    app.source_control_hunks = vec![GitDiffHunk {
        index: 0,
        fingerprint: 99,
        old_start: 1,
        old_lines: 1,
        new_start: 1,
        new_lines: 1,
        additions: 1,
        deletions: 1,
        header: "@@ -1,1 +1,1 @@".to_owned(),
    }];

    app.discard_active_diff_hunk();

    assert_eq!(app.status, git_hunk_discard_pending_status(&path, 0));
    assert!(app.command_bus.is_empty());
}

#[test]
fn source_control_hunk_statuses_report_lifecycle() {
    let path = PathBuf::from("C:/repo/src/main.rs");

    assert_eq!(
        git_hunk_list_pending_status(GitChangeStage::Unstaged, &path),
        "Loading unstaged hunks in main.rs"
    );
    assert_eq!(
        git_hunk_list_success_status(GitChangeStage::Unstaged, &path, 0),
        "No unstaged hunks in main.rs"
    );
    assert_eq!(
        git_hunk_list_success_status(GitChangeStage::Unstaged, &path, 1),
        "Loaded 1 unstaged hunk in main.rs"
    );
    assert_eq!(
        git_hunk_list_success_status(GitChangeStage::Staged, &path, 2),
        "Loaded 2 staged hunks in main.rs"
    );
    assert_eq!(
        git_hunk_list_failure_status(GitChangeStage::Staged, &path, "diff failed"),
        "Could not load staged hunks in main.rs: diff failed"
    );
    assert_eq!(
        git_hunk_stage_pending_status(&path, 1),
        "Staging hunk 1 in main.rs"
    );
    assert_eq!(
        git_hunk_stage_success_status(&path, 1),
        "Staged hunk 1 in main.rs"
    );
    assert_eq!(
        git_hunk_stage_failure_status(&path, 1, "stale"),
        "Could not stage hunk 1 in main.rs: stale"
    );
    assert_eq!(
        git_hunk_unstage_pending_status(&path, 1),
        "Unstaging hunk 1 in main.rs"
    );
    assert_eq!(
        git_hunk_unstage_success_status(&path, 1),
        "Unstaged hunk 1 in main.rs"
    );
    assert_eq!(
        git_hunk_unstage_failure_status(&path, 1, "stale"),
        "Could not unstage hunk 1 in main.rs: stale"
    );
    assert_eq!(
        git_hunk_discard_pending_status(&path, 1),
        "Discarding hunk 1 in main.rs"
    );
    assert_eq!(
        git_hunk_discard_success_status(&path, 1),
        "Discarded hunk 1 in main.rs"
    );
    assert_eq!(
        git_hunk_discard_failure_status(&path, 1, "stale"),
        "Could not discard hunk 1 in main.rs: stale"
    );
}
