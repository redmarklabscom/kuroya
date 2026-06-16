use super::{
    CachedGitHunkLookup, HunkDiscardOpenBufferUpdate, SourceControlHunkText,
    cached_git_hunk_index_at_new_line, git_hunk_index_at_new_line, hunk_discard_open_buffer_update,
    hunk_discard_should_replace_open_buffer, source_control_hunk_load_target_matches,
    source_control_hunk_open_path_matches, source_control_hunk_selection_after_reload,
    source_control_hunk_text_for_open_buffer, source_control_hunk_text_source_for_status,
};
use crate::path_display::{display_error_label, display_path_label};
use crate::source_control_runtime::{
    source_control_app_for_test, source_control_mutation_restricted_status,
};
use kuroya_core::{GitChangeStage, GitDiffHunk, GitFileStatus, TextBuffer, TextEdit};
use std::{
    env, fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn assert_restricted_status(app: &crate::KuroyaApp, action: &str) {
    assert_eq!(
        app.status,
        source_control_mutation_restricted_status(action)
    );
}

fn assert_display_safe_status(status: &str) {
    assert!(!status.contains('\n'), "{status:?}");
    assert!(!status.contains('\r'), "{status:?}");
    assert!(!status.contains('\u{202e}'), "{status:?}");
    assert!(!status.contains('\u{2066}'), "{status:?}");
}

fn unsafe_display_path() -> PathBuf {
    Path::new("workspace").join("src").join(format!(
        "bad\nname\u{202e}{}tail.rs",
        "very-long-".repeat(32)
    ))
}

fn unsafe_git_error() -> String {
    format!(
        "git failed\nsecond line \u{2066}{}",
        "error-detail-".repeat(32)
    )
}

fn temp_path(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    env::temp_dir().join(format!(
        "kuroya-source-control-hunk-{name}-{}-{nanos}",
        std::process::id()
    ))
}

#[test]
fn hunk_status_messages_preserve_text() {
    let path = Path::new("workspace").join("src/main.rs");
    let path_label = display_path_label(&path);
    let error = "git failed";
    let error_label = display_error_label(error);

    assert_eq!(
        super::git_hunk_list_pending_status(GitChangeStage::Unstaged, &path),
        format!("Loading unstaged hunks in {path_label}")
    );
    assert_eq!(
        super::git_hunk_list_success_status(GitChangeStage::Unstaged, &path, 0),
        format!("No unstaged hunks in {path_label}")
    );
    assert_eq!(
        super::git_hunk_list_success_status(GitChangeStage::Unstaged, &path, 1),
        format!("Loaded 1 unstaged hunk in {path_label}")
    );
    assert_eq!(
        super::git_hunk_list_success_status(GitChangeStage::Staged, &path, 3),
        format!("Loaded 3 staged hunks in {path_label}")
    );
    assert_eq!(
        super::git_hunk_target_loading_status(GitChangeStage::Staged, &path, "stage"),
        format!("Loading staged hunks in {path_label}; retry stage after they finish")
    );
    assert_eq!(
        super::git_hunk_stage_pending_status(&path, 7),
        format!("Staging hunk 7 in {path_label}")
    );
    assert_eq!(
        super::git_hunk_stage_success_status(&path, 7),
        format!("Staged hunk 7 in {path_label}")
    );
    assert_eq!(
        super::git_hunk_stage_missing_identity_status(&path, 7),
        format!("Reload hunks in {path_label} before staging hunk 7")
    );
    assert_eq!(
        super::git_hunk_stage_failure_status(&path, 7, error),
        format!("Could not stage hunk 7 in {path_label}: {error_label}")
    );
    assert_eq!(
        super::git_hunk_unstage_pending_status(&path, 7),
        format!("Unstaging hunk 7 in {path_label}")
    );
    assert_eq!(
        super::git_hunk_unstage_success_status(&path, 7),
        format!("Unstaged hunk 7 in {path_label}")
    );
    assert_eq!(
        super::git_hunk_unstage_missing_identity_status(&path, 7),
        format!("Reload hunks in {path_label} before unstaging hunk 7")
    );
    assert_eq!(
        super::git_hunk_unstage_failure_status(&path, 7, error),
        format!("Could not unstage hunk 7 in {path_label}: {error_label}")
    );
    assert_eq!(
        super::git_hunk_discard_pending_status(&path, 7),
        format!("Discarding hunk 7 in {path_label}")
    );
    assert_eq!(
        super::git_hunk_discard_dirty_buffer_status(&path),
        format!("Save or reload {path_label} before discarding hunks")
    );
    assert_eq!(
        super::git_hunk_discard_missing_identity_status(&path, 7),
        format!("Reload hunks in {path_label} before discarding hunk 7")
    );
    assert_eq!(
        super::git_hunk_discard_success_status(&path, 7),
        format!("Discarded hunk 7 in {path_label}")
    );
    assert_eq!(
        super::git_hunk_discard_failure_status(&path, 7, error),
        format!("Could not discard hunk 7 in {path_label}: {error_label}")
    );
}

#[test]
fn hunk_path_statuses_use_display_safe_path_label() {
    let path = unsafe_display_path();
    let path_label = display_path_label(&path);
    assert!(path_label.contains("..."));
    assert_display_safe_status(&path_label);

    let statuses = [
        super::git_hunk_list_pending_status(GitChangeStage::Unstaged, &path),
        super::git_hunk_list_success_status(GitChangeStage::Unstaged, &path, 0),
        super::git_hunk_list_success_status(GitChangeStage::Unstaged, &path, 1),
        super::git_hunk_list_success_status(GitChangeStage::Staged, &path, 3),
        super::git_hunk_target_loading_status(GitChangeStage::Staged, &path, "stage"),
        super::git_hunk_stage_pending_status(&path, 7),
        super::git_hunk_stage_success_status(&path, 7),
        super::git_hunk_stage_missing_identity_status(&path, 7),
        super::git_hunk_unstage_pending_status(&path, 7),
        super::git_hunk_unstage_success_status(&path, 7),
        super::git_hunk_unstage_missing_identity_status(&path, 7),
        super::git_hunk_discard_pending_status(&path, 7),
        super::git_hunk_discard_dirty_buffer_status(&path),
        super::git_hunk_discard_success_status(&path, 7),
    ];

    for status in statuses {
        assert_display_safe_status(&status);
        assert!(status.contains(&path_label), "{status:?}");
    }
}

#[test]
fn hunk_failure_statuses_use_display_safe_path_and_error_labels() {
    let path = unsafe_display_path();
    let error = unsafe_git_error();
    let path_label = display_path_label(&path);
    let error_label = display_error_label(&error);
    assert!(error_label.contains("..."));
    assert_display_safe_status(&error_label);

    let statuses = [
        super::git_hunk_list_failure_status(GitChangeStage::Unstaged, &path, &error),
        super::git_hunk_stage_failure_status(&path, 7, &error),
        super::git_hunk_unstage_failure_status(&path, 7, &error),
        super::git_hunk_discard_failure_status(&path, 7, &error),
    ];

    for status in statuses {
        assert_display_safe_status(&status);
        assert!(status.contains(&path_label), "{status:?}");
        assert!(status.ends_with(&error_label), "{status:?}");
    }
}

#[test]
fn active_hunk_target_status_sanitizes_path_without_replacing_raw_path() {
    let root = PathBuf::from("workspace");
    let path = unsafe_display_path();
    let path_label = display_path_label(&path);
    let mut app = source_control_app_for_test(root, true);
    app.active = Some(1);
    app.buffers.push(TextBuffer::from_text(
        1,
        Some(path.clone()),
        "one\ntwo\n".to_owned(),
    ));
    app.source_control_hunks_open = true;
    app.source_control_hunk_path = Some(path.clone());
    app.source_control_hunk_stage = GitChangeStage::Unstaged;

    app.source_control_hunks.clear();
    assert_eq!(app.active_file_worktree_hunk_target("stage"), None);
    assert_display_safe_status(&app.status);
    assert!(app.status.contains(&path_label), "{:?}", app.status);
    assert_eq!(app.source_control_hunk_path.as_ref(), Some(&path));
    assert_eq!(app.buffer(1).and_then(|buffer| buffer.path()), Some(&path));

    app.source_control_hunks = vec![hunk(3, 1, 9)];
    assert_eq!(app.active_file_worktree_hunk_target("stage"), None);
    assert_display_safe_status(&app.status);
    assert!(app.status.contains(&path_label), "{:?}", app.status);
    assert_eq!(app.source_control_hunk_path.as_ref(), Some(&path));
    assert_eq!(app.buffer(1).and_then(|buffer| buffer.path()), Some(&path));
}

#[test]
fn untrusted_workspace_rejects_hunk_mutation_sinks() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = source_control_app_for_test(root, false);

    app.stage_active_file_hunk();
    assert_restricted_status(&app, "staging hunks");

    app.discard_active_file_hunk();
    assert_restricted_status(&app, "discarding hunks");

    app.unstage_active_file_hunk();
    assert_restricted_status(&app, "unstaging hunks");

    app.spawn_stage_git_hunk(
        path.clone(),
        0,
        11,
        SourceControlHunkText::Ready("fn main() {}\n".to_owned()),
    );
    assert_restricted_status(&app, "staging hunks");

    app.spawn_unstage_git_hunk(path.clone(), 0, 11);
    assert_restricted_status(&app, "unstaging hunks");

    app.spawn_discard_git_hunk(
        path,
        0,
        11,
        SourceControlHunkText::Ready("fn main() {}\n".to_owned()),
        None,
    );
    assert_restricted_status(&app, "discarding hunks");
}

#[test]
fn discard_source_control_hunk_rejects_dirty_open_buffer() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = source_control_app_for_test(root, true);
    let mut buffer = TextBuffer::from_text(1, Some(path.clone()), "dirty\n".to_owned());
    buffer.apply_edit(TextEdit {
        range: 0..0,
        inserted: "local ".to_owned(),
    });
    app.buffers.push(buffer);

    app.discard_source_control_hunk(path.clone(), 0, 11);

    assert_eq!(
        app.status,
        super::git_hunk_discard_dirty_buffer_status(&path)
    );
    assert_eq!(app.buffer(1).unwrap().text(), "local dirty\n");
    assert!(app.buffer(1).unwrap().is_dirty());
}

#[test]
fn cached_hunk_fingerprint_requires_matching_hunk_cache() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = source_control_app_for_test(root.clone(), true);
    app.source_control_hunks_open = true;
    app.source_control_hunk_path = Some(path.clone());
    app.source_control_hunk_stage = GitChangeStage::Unstaged;
    app.source_control_hunks = vec![hunk(10, 2, 7), hunk(20, 2, 8)];
    app.source_control_hunks[0].fingerprint = 7007;
    app.source_control_hunks[1].fingerprint = 8008;

    assert_eq!(
        app.cached_source_control_hunk_fingerprint(&path, GitChangeStage::Unstaged, 8),
        Some(8008)
    );
    assert_eq!(
        app.cached_source_control_hunk_fingerprint(&path, GitChangeStage::Staged, 8),
        None
    );
    assert_eq!(
        app.cached_source_control_hunk_fingerprint(
            &root.join("src/lib.rs"),
            GitChangeStage::Unstaged,
            8,
        ),
        None
    );
    assert_eq!(
        app.cached_source_control_hunk_fingerprint(&path, GitChangeStage::Unstaged, 9),
        None
    );
}

#[test]
fn cached_hunk_fingerprint_rejects_duplicate_hunk_indexes() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = source_control_app_for_test(root, true);
    app.source_control_hunks_open = true;
    app.source_control_hunk_path = Some(path.clone());
    app.source_control_hunk_stage = GitChangeStage::Unstaged;
    app.source_control_hunks = vec![hunk(10, 2, 7), hunk(30, 2, 7)];
    app.source_control_hunks[0].fingerprint = 7007;
    app.source_control_hunks[1].fingerprint = 8008;

    assert_eq!(
        app.cached_source_control_hunk_fingerprint(&path, GitChangeStage::Unstaged, 7),
        None
    );
}

#[test]
fn stale_hunk_discard_completion_does_not_replace_newer_open_buffer_edits() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = source_control_app_for_test(root.clone(), true);
    let mut buffer = TextBuffer::from_text(1, Some(path.clone()), "before\n".to_owned());
    let expected_version = buffer.version();
    buffer.apply_edit(TextEdit {
        range: 0..0,
        inserted: "newer ".to_owned(),
    });
    let newer_version = buffer.version();
    app.buffers.push(buffer);

    app.apply_git_hunk_discarded(
        root,
        path,
        0,
        "discarded\n".to_owned(),
        Some((1, expected_version)),
    );

    let buffer = app.buffer(1).expect("buffer");
    assert_eq!(buffer.text(), "newer before\n");
    assert_eq!(buffer.version(), newer_version);
    assert!(buffer.is_dirty());
    assert!(app.external_change_buffers.contains(&1));
}

#[test]
fn current_hunk_discard_completion_replaces_matching_open_buffer_version() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = source_control_app_for_test(root.clone(), true);
    let buffer = TextBuffer::from_text(1, Some(path.clone()), "before\n".to_owned());
    let expected_version = buffer.version();
    app.buffers.push(buffer);
    app.mark_buffer_changed_on_disk(1);

    app.apply_git_hunk_discarded(
        root,
        path,
        0,
        "discarded\n".to_owned(),
        Some((1, expected_version)),
    );

    let buffer = app.buffer(1).expect("buffer");
    assert_eq!(buffer.text(), "discarded\n");
    assert!(!buffer.is_dirty());
    assert!(!app.buffer_changed_on_disk(1));
}

#[test]
fn duplicate_hunk_discard_completion_keeps_clean_buffer_unmarked() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = source_control_app_for_test(root.clone(), true);
    let buffer = TextBuffer::from_text(1, Some(path.clone()), "before\n".to_owned());
    let expected_version = buffer.version();
    app.buffers.push(buffer);

    app.apply_git_hunk_discarded(
        root.clone(),
        path.clone(),
        0,
        "discarded\n".to_owned(),
        Some((1, expected_version)),
    );
    app.apply_git_hunk_discarded(
        root,
        path,
        0,
        "discarded\n".to_owned(),
        Some((1, expected_version)),
    );

    let buffer = app.buffer(1).expect("buffer");
    assert_eq!(buffer.text(), "discarded\n");
    assert!(!buffer.is_dirty());
    assert!(!app.buffer_changed_on_disk(1));
}

#[test]
fn hunk_discard_without_open_buffer_snapshot_does_not_replace_dirty_open_buffer() {
    assert!(!hunk_discard_should_replace_open_buffer(1, 2, true, None));
    assert!(hunk_discard_should_replace_open_buffer(1, 2, false, None));
    assert!(!hunk_discard_should_replace_open_buffer(
        1,
        2,
        true,
        Some((1, 2))
    ));
    assert!(hunk_discard_should_replace_open_buffer(
        1,
        2,
        false,
        Some((1, 2))
    ));
    assert_eq!(
        hunk_discard_open_buffer_update(1, 3, false, true, Some((1, 2))),
        HunkDiscardOpenBufferUpdate::AlreadyApplied
    );
    assert_eq!(
        hunk_discard_open_buffer_update(1, 3, false, false, Some((1, 2))),
        HunkDiscardOpenBufferUpdate::MarkChangedOnDisk
    );
}

#[test]
fn hunk_text_file_respects_size_limit_before_reading() {
    let path = temp_path("oversize.txt");
    fs::write(&path, "too large").unwrap();

    let error = SourceControlHunkText::File(path.clone())
        .load(3)
        .unwrap_err()
        .to_string();

    assert!(error.contains("file is too large to open"));
    assert!(error.contains("9 B"));
    assert!(error.contains("3 B"));
    let _ = fs::remove_file(path);
}

#[test]
fn hunk_text_file_read_error_uses_display_safe_path_label() {
    let path = unsafe_display_path();
    let error = SourceControlHunkText::File(path.clone())
        .load(1024)
        .unwrap_err()
        .to_string();

    assert_display_safe_status(&error);
    assert!(error.contains(&display_path_label(&path)), "{error:?}");
}

#[test]
fn hunk_text_open_buffer_respects_size_limit_before_text_clone() {
    let buffer = TextBuffer::from_text(
        1,
        Some(PathBuf::from("workspace/src/large.rs")),
        "abcdef".to_owned(),
    );

    let error = source_control_hunk_text_for_open_buffer(&buffer, 3)
        .load(3)
        .unwrap_err()
        .to_string();

    assert!(error.contains("file is too large to open"));
    assert!(error.contains("6 B"));
    assert!(error.contains("3 B"));
}

#[test]
fn hunk_text_open_buffer_uses_snapshot_for_allowed_text() {
    let buffer = TextBuffer::from_text(
        1,
        Some(PathBuf::from("workspace/src/main.rs")),
        "abcdef".to_owned(),
    );

    let text = source_control_hunk_text_for_open_buffer(&buffer, 99);

    match text {
        SourceControlHunkText::Snapshot(snapshot) => assert_eq!(snapshot.text(), "abcdef"),
        other => panic!("expected snapshot text source, got {other:?}"),
    }
}

#[test]
fn hunk_text_source_deleted_status_overrides_clean_open_buffer_snapshot() {
    let path = PathBuf::from("workspace/src/deleted.rs");
    let buffer = TextBuffer::from_text(1, Some(path.clone()), "old content\n".to_owned());

    let text = source_control_hunk_text_source_for_status(
        &path,
        Some(GitFileStatus::Deleted),
        Some(&buffer),
        99,
    );

    match text {
        SourceControlHunkText::Ready(text) => assert_eq!(text, ""),
        other => panic!("expected deleted file empty text source, got {other:?}"),
    }
}

#[test]
fn hunk_text_source_uses_open_buffer_when_file_is_not_deleted() {
    let path = PathBuf::from("workspace/src/modified.rs");
    let buffer = TextBuffer::from_text(1, Some(path.clone()), "current\n".to_owned());

    let text = source_control_hunk_text_source_for_status(
        &path,
        Some(GitFileStatus::Modified),
        Some(&buffer),
        99,
    );

    match text {
        SourceControlHunkText::Snapshot(snapshot) => assert_eq!(snapshot.text(), "current\n"),
        other => panic!("expected open buffer snapshot text source, got {other:?}"),
    }
}

#[test]
fn hunk_text_source_uses_lexical_open_buffer_before_file_fallback() {
    let root = PathBuf::from("workspace");
    let mut app = source_control_app_for_test(root.clone(), true);
    let request_path = root.join("src/main.rs");
    let buffer_path = root.join("src").join(".").join("main.rs");
    app.buffers.push(TextBuffer::from_text(
        1,
        Some(buffer_path),
        "open buffer\n".to_owned(),
    ));

    let text = app
        .source_control_hunk_text_source(&request_path, 99)
        .load(99)
        .expect("lexical open buffer should be used without reading a file");

    assert_eq!(text, "open buffer\n");
}

#[test]
fn hunk_text_source_reads_file_when_no_deleted_status_or_open_buffer_exists() {
    let path = PathBuf::from("workspace/src/file.rs");

    let text = source_control_hunk_text_source_for_status(&path, None, None, 99);

    match text {
        SourceControlHunkText::File(file_path) => assert_eq!(file_path, path),
        other => panic!("expected file text source, got {other:?}"),
    }
}

#[test]
fn hunk_text_ready_respects_size_limit() {
    let error = SourceControlHunkText::Ready("abcdef".to_owned())
        .load(3)
        .unwrap_err()
        .to_string();

    assert!(error.contains("file is too large to open"));
    assert!(error.contains("6 B"));
    assert!(error.contains("3 B"));
}

#[test]
fn hunk_text_snapshot_rechecks_size_limit_on_load() {
    let buffer = TextBuffer::from_text(
        1,
        Some(PathBuf::from("workspace/src/main.rs")),
        "abcdef".to_owned(),
    );

    let error = SourceControlHunkText::Snapshot(buffer.text_snapshot())
        .load(3)
        .unwrap_err()
        .to_string();

    assert!(error.contains("file is too large to open"));
    assert!(error.contains("6 B"));
    assert!(error.contains("3 B"));
}

#[test]
fn hunk_text_file_zero_limit_allows_full_read() {
    let path = temp_path("zero-limit.txt");
    fs::write(&path, "abcdef").unwrap();

    let text = SourceControlHunkText::File(path.clone()).load(0).unwrap();

    assert_eq!(text, "abcdef");
    let _ = fs::remove_file(path);
}

#[test]
fn cached_hunk_lookup_uses_loaded_matching_cache_only() {
    let path = Path::new("src/main.rs");
    let other_path = Path::new("src/lib.rs");
    let hunks = vec![hunk(3, 4, 9)];

    assert_eq!(
        cached_git_hunk_index_at_new_line(
            true,
            Some(path),
            GitChangeStage::Unstaged,
            path,
            GitChangeStage::Unstaged,
            &hunks,
            4,
        ),
        CachedGitHunkLookup::Found(9)
    );
    assert_eq!(
        cached_git_hunk_index_at_new_line(
            false,
            Some(path),
            GitChangeStage::Unstaged,
            path,
            GitChangeStage::Unstaged,
            &hunks,
            4,
        ),
        CachedGitHunkLookup::MissingCache
    );
    assert_eq!(
        cached_git_hunk_index_at_new_line(
            true,
            Some(other_path),
            GitChangeStage::Unstaged,
            path,
            GitChangeStage::Unstaged,
            &hunks,
            4,
        ),
        CachedGitHunkLookup::MissingCache
    );
    assert_eq!(
        cached_git_hunk_index_at_new_line(
            true,
            Some(path),
            GitChangeStage::Staged,
            path,
            GitChangeStage::Unstaged,
            &hunks,
            4,
        ),
        CachedGitHunkLookup::MissingCache
    );
}

#[test]
fn cached_hunk_lookup_reports_empty_and_cursor_miss() {
    let path = Path::new("src/main.rs");

    assert_eq!(
        cached_git_hunk_index_at_new_line(
            true,
            Some(path),
            GitChangeStage::Unstaged,
            path,
            GitChangeStage::Unstaged,
            &[],
            4,
        ),
        CachedGitHunkLookup::Empty
    );
    assert_eq!(
        cached_git_hunk_index_at_new_line(
            true,
            Some(path),
            GitChangeStage::Unstaged,
            path,
            GitChangeStage::Unstaged,
            &[hunk(3, 4, 9)],
            8,
        ),
        CachedGitHunkLookup::NoHunkAtLine
    );
}

#[test]
fn cached_hunk_lookup_rejects_ambiguous_overlapping_ranges() {
    let path = Path::new("src/main.rs");
    let hunks = vec![hunk(3, 4, 0), hunk(5, 2, 1)];

    assert_eq!(
        cached_git_hunk_index_at_new_line(
            true,
            Some(path),
            GitChangeStage::Unstaged,
            path,
            GitChangeStage::Unstaged,
            &hunks,
            5,
        ),
        CachedGitHunkLookup::Ambiguous
    );
}

#[test]
fn hunk_line_lookup_finds_ordered_sparse_hunks() {
    let hunks = vec![hunk(3, 2, 0), hunk(30, 4, 1), hunk(80, 1, 2)];

    assert_eq!(git_hunk_index_at_new_line(&hunks, 1), None);
    assert_eq!(git_hunk_index_at_new_line(&hunks, 3), Some(0));
    assert_eq!(git_hunk_index_at_new_line(&hunks, 4), Some(0));
    assert_eq!(git_hunk_index_at_new_line(&hunks, 29), None);
    assert_eq!(git_hunk_index_at_new_line(&hunks, 30), Some(1));
    assert_eq!(git_hunk_index_at_new_line(&hunks, 33), Some(1));
    assert_eq!(git_hunk_index_at_new_line(&hunks, 80), Some(2));
    assert_eq!(git_hunk_index_at_new_line(&hunks, 81), None);
}

#[test]
fn hunk_line_lookup_rejects_overlapping_ranges_as_ambiguous() {
    let hunks = vec![hunk(3, 4, 0), hunk(5, 2, 1), hunk(20, 1, 2)];

    assert_eq!(git_hunk_index_at_new_line(&hunks, 3), Some(0));
    assert_eq!(git_hunk_index_at_new_line(&hunks, 5), None);
    assert_eq!(git_hunk_index_at_new_line(&hunks, 6), None);
    assert_eq!(git_hunk_index_at_new_line(&hunks, 20), Some(2));
}

#[test]
fn hunk_line_lookup_treats_zero_new_line_hunks_as_addressable() {
    let hunks = vec![hunk(0, 0, 0), hunk(12, 0, 1), hunk(20, 3, 2)];

    assert_eq!(git_hunk_index_at_new_line(&hunks, 1), Some(0));
    assert_eq!(git_hunk_index_at_new_line(&hunks, 12), Some(1));
    assert_eq!(git_hunk_index_at_new_line(&hunks, 13), None);
    assert_eq!(git_hunk_index_at_new_line(&hunks, 22), Some(2));
}

#[test]
fn hunk_line_lookup_rejects_non_empty_zero_start_ranges() {
    let hunks = vec![hunk(0, 3, 0)];

    assert_eq!(git_hunk_index_at_new_line(&hunks, 0), None);
    assert_eq!(git_hunk_index_at_new_line(&hunks, 1), None);
    assert_eq!(git_hunk_index_at_new_line(&hunks, 3), None);
    assert_eq!(git_hunk_index_at_new_line(&hunks, 4), None);
}

#[test]
fn hunk_line_lookup_saturates_overflowing_ranges() {
    let hunks = vec![hunk(usize::MAX - 1, 4, 0)];

    assert_eq!(git_hunk_index_at_new_line(&hunks, usize::MAX - 2), None);
    assert_eq!(git_hunk_index_at_new_line(&hunks, usize::MAX - 1), Some(0));
    assert_eq!(git_hunk_index_at_new_line(&hunks, usize::MAX), Some(0));
}

#[test]
fn hunk_line_lookup_falls_back_for_out_of_order_ranges() {
    let hunks = vec![hunk(30, 4, 1), hunk(3, 2, 0), hunk(80, 1, 2)];

    assert_eq!(git_hunk_index_at_new_line(&hunks, 4), Some(0));
    assert_eq!(git_hunk_index_at_new_line(&hunks, 31), Some(1));
    assert_eq!(git_hunk_index_at_new_line(&hunks, 80), Some(2));
    assert_eq!(git_hunk_index_at_new_line(&hunks, 81), None);
}

#[test]
fn hunk_reload_selection_tracks_same_hunk_after_reorder() {
    let previous_hunks = vec![hunk(10, 2, 0), hunk(30, 4, 1), hunk(50, 1, 2)];
    let reloaded_hunks = vec![hunk(50, 1, 0), hunk(10, 2, 1), hunk(30, 4, 2)];

    let selected = source_control_hunk_selection_after_reload(1, &previous_hunks, &reloaded_hunks);

    assert_eq!(selected, 2);
    assert_eq!(reloaded_hunks[selected].new_start, 30);
}

#[test]
fn hunk_reload_selection_clamps_when_selected_hunk_disappears() {
    let previous_hunks = vec![hunk(10, 2, 0), hunk(30, 4, 1), hunk(50, 1, 2)];
    let reloaded_hunks = vec![hunk(10, 2, 0)];

    let selected = source_control_hunk_selection_after_reload(2, &previous_hunks, &reloaded_hunks);

    assert_eq!(selected, 0);
}

#[test]
fn hunk_load_target_requires_current_path_and_stage() {
    let path = Path::new("src/main.rs");
    let other_path = Path::new("src/lib.rs");

    assert!(source_control_hunk_load_target_matches(
        Some(path),
        GitChangeStage::Unstaged,
        path,
        GitChangeStage::Unstaged
    ));
    assert!(!source_control_hunk_load_target_matches(
        None,
        GitChangeStage::Unstaged,
        path,
        GitChangeStage::Unstaged
    ));
    assert!(!source_control_hunk_load_target_matches(
        Some(other_path),
        GitChangeStage::Unstaged,
        path,
        GitChangeStage::Unstaged
    ));
    assert!(!source_control_hunk_load_target_matches(
        Some(path),
        GitChangeStage::Staged,
        path,
        GitChangeStage::Unstaged
    ));
}

#[test]
fn hunk_open_path_requires_open_panel_and_raw_path_match() {
    let path = Path::new("workspace/src/main.rs");
    let other_path = Path::new("workspace/src/lib.rs");

    assert!(source_control_hunk_open_path_matches(
        true,
        Some(path),
        path
    ));
    assert!(!source_control_hunk_open_path_matches(
        false,
        Some(path),
        path
    ));
    assert!(!source_control_hunk_open_path_matches(true, None, path));
    assert!(!source_control_hunk_open_path_matches(
        true,
        Some(other_path),
        path
    ));
}

#[test]
fn hunk_load_request_ids_wrap_to_nonzero_and_skip_in_flight_id() {
    let mut next_request_id = u64::MAX;
    let mut active_request_id = u64::MAX;
    let mut in_flight = None;
    let mut queued = false;

    assert_eq!(
        super::begin_source_control_hunks_request_state(
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

    next_request_id = u64::MAX;
    active_request_id = 1;
    in_flight = Some(1);
    queued = false;
    assert_eq!(
        super::begin_source_control_hunks_request_state(
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
fn apply_git_hunks_loaded_preserves_selected_hunk_identity() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = source_control_app_for_test(root.clone(), true);
    app.source_control_hunks_open = true;
    app.source_control_hunk_path = Some(path.clone());
    app.source_control_hunk_stage = GitChangeStage::Unstaged;
    app.source_control_hunks_active_request_id = 7;
    app.source_control_hunks = vec![hunk(10, 2, 0), hunk(30, 4, 1), hunk(50, 1, 2)];
    app.source_control_hunk_selected = 1;

    app.apply_git_hunks_loaded(
        7,
        root.clone(),
        root,
        path,
        GitChangeStage::Unstaged,
        vec![hunk(50, 1, 0), hunk(10, 2, 1), hunk(30, 4, 2)],
    );

    assert_eq!(app.source_control_hunk_selected, 2);
    assert_eq!(
        app.source_control_hunks[app.source_control_hunk_selected].new_start,
        30
    );
}

#[test]
fn apply_git_hunks_loaded_preserves_raw_hunk_payload() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = source_control_app_for_test(root.clone(), true);
    app.source_control_hunks_open = true;
    app.source_control_hunk_path = Some(path.clone());
    app.source_control_hunk_stage = GitChangeStage::Unstaged;
    app.source_control_hunks_active_request_id = 7;
    let mut loaded = hunk(30, 4, 9);
    loaded.fingerprint = 99;
    loaded.old_start = 20;
    loaded.old_lines = 5;
    loaded.additions = 3;
    loaded.deletions = 2;
    loaded.header = "@@ -20,5 +30,4 @@ fn changed".to_owned();
    let expected = loaded.clone();

    app.apply_git_hunks_loaded(
        7,
        root.clone(),
        root,
        path,
        GitChangeStage::Unstaged,
        vec![loaded],
    );

    assert_eq!(app.source_control_hunks, vec![expected]);
}

#[test]
fn stale_git_hunks_loaded_with_old_request_id_is_ignored() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = source_control_app_for_test(root.clone(), true);
    app.source_control_hunks_open = true;
    app.source_control_hunk_path = Some(path.clone());
    app.source_control_hunk_stage = GitChangeStage::Unstaged;
    app.source_control_hunks_active_request_id = 8;
    app.source_control_hunks = vec![hunk(10, 2, 0)];
    app.status = "current hunk load".to_owned();

    app.apply_git_hunks_loaded(
        7,
        root.clone(),
        root,
        path,
        GitChangeStage::Unstaged,
        vec![hunk(30, 4, 1)],
    );

    assert_eq!(app.source_control_hunks.len(), 1);
    assert_eq!(app.source_control_hunks[0].index, 0);
    assert_eq!(app.status, "current hunk load");
}

#[test]
fn stale_git_hunks_failed_with_old_request_id_is_ignored() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = source_control_app_for_test(root.clone(), true);
    app.source_control_hunks_open = true;
    app.source_control_hunk_path = Some(path.clone());
    app.source_control_hunk_stage = GitChangeStage::Unstaged;
    app.source_control_hunks_active_request_id = 8;
    app.source_control_hunks = vec![hunk(10, 2, 0)];
    app.status = "current hunk load".to_owned();

    app.apply_git_hunks_failed(
        7,
        root.clone(),
        root,
        path,
        GitChangeStage::Unstaged,
        "stale failure".to_owned(),
    );

    assert_eq!(app.source_control_hunks.len(), 1);
    assert_eq!(app.source_control_hunks[0].index, 0);
    assert_eq!(app.status, "current hunk load");
}

#[test]
fn stale_git_hunks_loaded_with_stage_mismatch_is_ignored() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = source_control_app_for_test(root.clone(), true);
    app.source_control_hunks_open = true;
    app.source_control_hunk_path = Some(path.clone());
    app.source_control_hunk_stage = GitChangeStage::Staged;
    app.source_control_hunks_active_request_id = 7;
    app.source_control_hunks = vec![hunk(10, 2, 0)];
    app.status = "current staged hunk load".to_owned();

    app.apply_git_hunks_loaded(
        7,
        root.clone(),
        root,
        path,
        GitChangeStage::Unstaged,
        vec![hunk(30, 4, 1)],
    );

    assert_eq!(app.source_control_hunks.len(), 1);
    assert_eq!(app.source_control_hunks[0].index, 0);
    assert_eq!(app.status, "current staged hunk load");
}

#[test]
fn stale_git_hunks_loaded_without_current_path_is_ignored() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = source_control_app_for_test(root.clone(), true);
    app.source_control_hunks_open = true;
    app.source_control_hunk_path = None;
    app.source_control_hunk_stage = GitChangeStage::Unstaged;
    app.source_control_hunks_active_request_id = 7;
    app.source_control_hunks = vec![hunk(10, 2, 0)];
    app.status = "current hunk load".to_owned();

    app.apply_git_hunks_loaded(
        7,
        root.clone(),
        root,
        path,
        GitChangeStage::Unstaged,
        vec![hunk(30, 4, 1)],
    );

    assert_eq!(app.source_control_hunks.len(), 1);
    assert_eq!(app.source_control_hunks[0].index, 0);
    assert_eq!(app.status, "current hunk load");
}

#[test]
fn stale_git_hunks_failed_without_current_path_is_ignored() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = source_control_app_for_test(root.clone(), true);
    app.source_control_hunks_open = true;
    app.source_control_hunk_path = None;
    app.source_control_hunk_stage = GitChangeStage::Unstaged;
    app.source_control_hunks_active_request_id = 7;
    app.source_control_hunks = vec![hunk(10, 2, 0)];
    app.status = "current hunk load".to_owned();

    app.apply_git_hunks_failed(
        7,
        root.clone(),
        root,
        path,
        GitChangeStage::Unstaged,
        "stale failure".to_owned(),
    );

    assert_eq!(app.source_control_hunks.len(), 1);
    assert_eq!(app.source_control_hunks[0].index, 0);
    assert_eq!(app.status, "current hunk load");
}

#[test]
fn stale_git_hunks_loaded_after_operation_root_change_is_ignored() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = source_control_app_for_test(root.clone(), true);
    app.source_control_hunks_open = true;
    app.source_control_hunk_path = Some(path.clone());
    app.source_control_hunk_stage = GitChangeStage::Unstaged;
    app.source_control_hunks_active_request_id = 7;
    app.source_control_hunks = vec![hunk(10, 2, 0)];
    app.status = "current hunk load".to_owned();

    app.apply_git_hunks_loaded(
        7,
        root.clone(),
        root.join("old-repo"),
        path,
        GitChangeStage::Unstaged,
        vec![hunk(30, 4, 1)],
    );

    assert_eq!(app.source_control_hunks.len(), 1);
    assert_eq!(app.source_control_hunks[0].index, 0);
    assert_eq!(app.status, "current hunk load");
}

#[test]
fn stale_git_hunks_failed_after_operation_root_change_is_ignored() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = source_control_app_for_test(root.clone(), true);
    app.source_control_hunks_open = true;
    app.source_control_hunk_path = Some(path.clone());
    app.source_control_hunk_stage = GitChangeStage::Unstaged;
    app.source_control_hunks_active_request_id = 7;
    app.source_control_hunks = vec![hunk(10, 2, 0)];
    app.status = "current hunk load".to_owned();

    app.apply_git_hunks_failed(
        7,
        root.clone(),
        root.join("old-repo"),
        path,
        GitChangeStage::Unstaged,
        "stale failure".to_owned(),
    );

    assert_eq!(app.source_control_hunks.len(), 1);
    assert_eq!(app.source_control_hunks[0].index, 0);
    assert_eq!(app.status, "current hunk load");
}

fn hunk(new_start: usize, new_lines: usize, index: usize) -> GitDiffHunk {
    GitDiffHunk {
        index,
        fingerprint: index as u64,
        old_start: new_start,
        old_lines: new_lines,
        new_start,
        new_lines,
        additions: 0,
        deletions: 0,
        header: String::new(),
    }
}
