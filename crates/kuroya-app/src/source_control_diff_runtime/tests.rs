use super::{
    SourceControlDiffOpen, SourceControlDiffOpenJob, SourceControlDiffOpenOutcome,
    SourceControlDiffOpenRequest, SourceControlDiffPathLabels, SourceControlDiffText,
    SourceControlSideBySideBuffer, head_side_by_side_open, source_control_diff_display_label,
    source_control_diff_display_label_cow, source_control_diff_error_label,
    source_control_diff_open_detail, source_control_diff_open_pending_status,
    source_control_diff_opens_side_by_side, source_control_diff_path_label,
    source_control_diff_read_failure_status, source_control_diff_split_weights,
    source_control_diff_target_display_label, source_control_diff_target_display_label_cow,
    source_control_diff_target_label, source_control_diff_title_label,
    source_control_head_compare_failure_status, source_control_head_side_open_failure_status,
    source_control_index_and_head_side_open_failure_status,
    source_control_side_by_side_open_if_needed, source_control_side_by_side_open_status,
    staged_side_by_side_open, staged_side_by_side_open_with_labels, worktree_side_by_side_open,
    worktree_side_by_side_source,
};
use crate::{
    KuroyaApp,
    app_startup_context::AppStartupContext,
    path_display::{
        DISPLAY_ERROR_LABEL_MAX_CHARS, DISPLAY_PATH_LABEL_MAX_CHARS, sanitized_display_label,
    },
    terminal::TerminalPane,
    virtual_diff_runtime::VirtualDiffOpen,
};
use kuroya_core::{DiffOptions, EditorSettings, TextBuffer, Workspace};
use std::{
    borrow::Cow,
    env, fs,
    path::PathBuf,
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use tokio::runtime::Runtime;

fn temp_path(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    env::temp_dir().join(format!(
        "kuroya-source-control-diff-{name}-{}-{nanos}",
        std::process::id()
    ))
}

fn hostile_display_path() -> PathBuf {
    PathBuf::from("workspace/src").join(format!("bad\n{}\u{202e}tail.rs", "very-long-".repeat(32)))
}

fn hostile_error() -> String {
    format!(
        "first line\nsecond line \u{202e}{}",
        "detail-".repeat(DISPLAY_ERROR_LABEL_MAX_CHARS)
    )
}

fn assert_display_safe(label: &str) {
    assert!(!label.contains('\n'), "{label:?}");
    assert!(!label.contains('\r'), "{label:?}");
    assert!(!label.contains('\u{202e}'), "{label:?}");
    assert!(!label.contains('\u{2066}'), "{label:?}");
    assert!(!label.contains('\u{2069}'), "{label:?}");
}

fn assert_cow_borrows_original(label: Cow<'_, str>, original: &str) {
    match label {
        Cow::Borrowed(borrowed) => {
            assert_eq!(borrowed, original);
            assert_eq!(borrowed.as_ptr(), original.as_ptr());
            assert_eq!(borrowed.len(), original.len());
        }
        Cow::Owned(owned) => panic!("expected borrowed label, got owned {owned:?}"),
    }
}

fn assert_cow_owned_eq(label: Cow<'_, str>, expected: &str) {
    match label {
        Cow::Owned(owned) => assert_eq!(owned, expected),
        Cow::Borrowed(borrowed) => panic!("expected owned label, got borrowed {borrowed:?}"),
    }
}

#[test]
fn source_control_diff_display_label_cows_borrow_clean_ascii_and_unicode() {
    let ascii = "clean-file.rs";
    let unicode = "resume-\u{5909}\u{66f4}.rs";

    assert_cow_borrows_original(source_control_diff_display_label_cow(ascii), ascii);
    assert_cow_borrows_original(source_control_diff_target_display_label_cow(ascii), ascii);
    assert_cow_borrows_original(source_control_diff_display_label_cow(unicode), unicode);
    assert_cow_borrows_original(
        source_control_diff_target_display_label_cow(unicode),
        unicode,
    );
}

#[test]
fn source_control_diff_display_label_cows_own_dirty_truncated_and_fallback_output() {
    assert_cow_owned_eq(
        source_control_diff_display_label_cow("bad\nname.rs"),
        "bad name.rs",
    );
    assert_cow_owned_eq(source_control_diff_display_label_cow("\r\n"), "diff");
    assert_cow_owned_eq(source_control_diff_target_display_label_cow("\r\n"), ".");
    assert_cow_owned_eq(
        source_control_diff_display_label_cow("bad\u{202e}\u{2066}name.rs"),
        "badname.rs",
    );

    let long = format!(
        "head-{}-tail",
        "segment-".repeat(DISPLAY_PATH_LABEL_MAX_CHARS)
    );
    match source_control_diff_display_label_cow(&long) {
        Cow::Owned(label) => {
            assert_display_safe(&label);
            assert!(label.contains("..."), "{label}");
            assert!(label.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);
        }
        Cow::Borrowed(label) => panic!("expected truncated owned label, got {label:?}"),
    }
}

#[test]
fn source_control_diff_target_display_label_cow_owns_fallback_and_truncated_output() {
    assert_cow_owned_eq(source_control_diff_target_display_label_cow("\r\n"), ".");

    let long = format!(
        "target-{}-tail",
        "segment-".repeat(DISPLAY_PATH_LABEL_MAX_CHARS)
    );
    let label = source_control_diff_target_display_label_cow(&long);
    match label {
        Cow::Owned(label) => {
            assert_display_safe(&label);
            assert!(label.contains("..."), "{label}");
            assert!(label.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);
            assert_eq!(source_control_diff_target_display_label(&long), label);
        }
        Cow::Borrowed(label) => panic!("expected truncated owned label, got {label:?}"),
    }
}

#[test]
fn source_control_diff_string_wrappers_match_cow_helpers() {
    let long = format!(
        "head-{}-tail",
        "segment-".repeat(DISPLAY_PATH_LABEL_MAX_CHARS)
    );
    let cases = [
        "clean.rs".to_owned(),
        "resume-\u{5909}\u{66f4}.rs".to_owned(),
        "bad\nname\u{202e}.rs".to_owned(),
        String::new(),
        long,
    ];

    for value in cases {
        assert_eq!(
            source_control_diff_display_label(&value),
            source_control_diff_display_label_cow(&value).into_owned()
        );
        assert_eq!(
            source_control_diff_target_display_label(&value),
            source_control_diff_target_display_label_cow(&value).into_owned()
        );
    }
}

#[test]
fn source_control_diff_title_label_preserves_suffix_budget_semantics() {
    assert_eq!(
        source_control_diff_title_label("clean.rs", "Changes"),
        "clean.rs (Changes)"
    );
    assert_eq!(
        source_control_diff_title_label("\r\n", "Changes"),
        "diff (Changes)"
    );

    let dirty_long_label = format!(
        "bad\n{}tail.rs",
        "segment-".repeat(DISPLAY_PATH_LABEL_MAX_CHARS)
    );
    let title = source_control_diff_title_label(&dirty_long_label, "Staged Changes");
    assert_display_safe(&title);
    assert!(title.ends_with(" (Staged Changes)"), "{title}");
    assert!(title.contains("..."), "{title}");
    assert!(title.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);

    let full_budget_suffix = "S".repeat(DISPLAY_PATH_LABEL_MAX_CHARS);
    let full_label = format!("clean.rs ({full_budget_suffix})");
    let title = source_control_diff_title_label("clean.rs", &full_budget_suffix);
    let expected = sanitized_display_label(&full_label, DISPLAY_PATH_LABEL_MAX_CHARS, "diff");
    assert_eq!(title, expected);
    assert_display_safe(&title);
    assert!(title.contains("..."), "{title}");
    assert!(title.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);
}

#[test]
fn source_control_diff_text_file_respects_size_limit_before_reading() {
    let path = temp_path("oversize.txt");
    fs::write(&path, "too large").unwrap();

    let error = SourceControlDiffText::File(path.clone())
        .load(3)
        .unwrap_err();

    assert!(error.contains("file is too large to open"));
    assert!(error.contains("9 B"));
    assert!(error.contains("3 B"));
    let _ = fs::remove_file(path);
}

#[test]
fn source_control_diff_text_open_buffer_respects_size_limit_before_text_clone() {
    let buffer = TextBuffer::from_text(
        1,
        Some(PathBuf::from("workspace/src/large.rs")),
        "abcdef".to_owned(),
    );

    let error = SourceControlDiffText::open_buffer(&buffer, 3)
        .load(3)
        .unwrap_err();

    assert!(error.contains("file is too large to open"));
    assert!(error.contains("6 B"));
    assert!(error.contains("3 B"));
}

#[test]
fn source_control_diff_text_open_buffer_uses_snapshot_for_allowed_text() {
    let buffer = TextBuffer::from_text(
        1,
        Some(PathBuf::from("workspace/src/main.rs")),
        "abcdef".to_owned(),
    );

    let text = SourceControlDiffText::open_buffer(&buffer, 99);

    match text {
        SourceControlDiffText::Snapshot(snapshot) => assert_eq!(snapshot.text(), "abcdef"),
        other => panic!("expected snapshot text source, got {other:?}"),
    }
}

#[test]
fn source_control_diff_text_preserves_worktree_source_availability() {
    let buffer = TextBuffer::from_text(
        1,
        Some(PathBuf::from("workspace/src/main.rs")),
        "abcdef".to_owned(),
    );

    let (text, available) = SourceControlDiffText::open_buffer(&buffer, 99)
        .load_with_worktree_source_availability(99)
        .unwrap();
    assert_eq!(text, "abcdef");
    assert!(available);

    let (text, available) = SourceControlDiffText::Deleted
        .load_with_worktree_source_availability(99)
        .unwrap();
    assert!(text.is_empty());
    assert!(!available);
}

#[test]
fn side_by_side_source_uses_loaded_source_availability_without_path_probe() {
    let path = temp_path("missing-side-by-side.rs");
    assert!(!path.exists());

    let source = worktree_side_by_side_source(&path, "current text", true);
    match source {
        SourceControlSideBySideBuffer::Worktree { path: source_path } => {
            assert_eq!(source_path, path);
        }
        other => panic!("expected worktree source, got {other:?}"),
    }

    let source = worktree_side_by_side_source(&path, "current text", false);
    match source {
        SourceControlSideBySideBuffer::Virtual {
            label,
            path: source_path,
            target,
            text,
        } => {
            assert!(label.ends_with(" (Working Tree)"));
            assert_eq!(source_path, path);
            assert_eq!(target, source_control_diff_target_label(&path));
            assert_eq!(text, "current text");
        }
        other => panic!("expected virtual source, got {other:?}"),
    }
}

#[test]
fn worktree_side_by_side_uses_preloaded_index_text_without_repo_probe() {
    let root = temp_path("missing-side-by-side-root");
    let path = root.join("src/main.rs");

    let open = worktree_side_by_side_open(
        &root,
        &path,
        Some(0),
        "current text",
        false,
        Some("index text".to_owned()),
    )
    .unwrap();

    match open.base {
        SourceControlSideBySideBuffer::Virtual { label, text, .. } => {
            assert!(label.ends_with(" (Index)"));
            assert_eq!(text, "index text");
        }
        other => panic!("expected virtual base, got {other:?}"),
    }
    assert_eq!(open.kind, "hunk changes");
}

#[test]
fn staged_and_head_side_by_side_use_preloaded_texts() {
    let path = PathBuf::from("workspace/src/main.rs");

    let staged = staged_side_by_side_open(
        &path,
        Some("head text".to_owned()),
        Some("index text".to_owned()),
    );
    match staged.base {
        SourceControlSideBySideBuffer::Virtual { text, .. } => {
            assert_eq!(text, "head text");
        }
        other => panic!("expected virtual staged base, got {other:?}"),
    }
    match staged.source {
        SourceControlSideBySideBuffer::Virtual { text, .. } => {
            assert_eq!(text, "index text");
        }
        other => panic!("expected virtual staged source, got {other:?}"),
    }

    let head = head_side_by_side_open(&path, "current text", false, Some("head text".into()));
    match head.base {
        SourceControlSideBySideBuffer::Virtual { text, .. } => {
            assert_eq!(text, "head text");
        }
        other => panic!("expected virtual head base, got {other:?}"),
    }
}

#[test]
fn source_control_diff_detail_names_stage_path_and_hunk() {
    assert_eq!(
        source_control_diff_open_detail(&SourceControlDiffOpenRequest::Worktree {
            path: PathBuf::from("src/main.rs"),
            focus_hunk: Some(2),
        }),
        "worktree main.rs hunk 3"
    );
    assert_eq!(
        source_control_diff_open_detail(&SourceControlDiffOpenRequest::Staged {
            path: PathBuf::from("src/lib.rs"),
            focus_hunk: None,
        }),
        "staged lib.rs"
    );
    assert_eq!(
        source_control_diff_open_detail(&SourceControlDiffOpenRequest::Head {
            path: PathBuf::from("src/app.rs"),
        }),
        "HEAD app.rs"
    );
}

#[test]
fn source_control_diff_side_by_side_mode_requires_setting_supported_diff_and_accessible_off() {
    assert!(source_control_diff_opens_side_by_side(
        true, false, true, true, 900, None
    ));
    assert!(!source_control_diff_opens_side_by_side(
        false, false, true, true, 900, None
    ));
    assert!(!source_control_diff_opens_side_by_side(
        true, true, true, true, 900, None
    ));
    assert!(!source_control_diff_opens_side_by_side(
        true, false, false, true, 900, None
    ));
}

#[test]
fn source_control_diff_side_by_side_mode_falls_back_inline_when_pane_is_narrow() {
    assert!(!source_control_diff_opens_side_by_side(
        true,
        false,
        true,
        true,
        900,
        Some(720.0)
    ));
    assert!(source_control_diff_opens_side_by_side(
        true,
        false,
        true,
        true,
        900,
        Some(900.0)
    ));
    assert!(source_control_diff_opens_side_by_side(
        true,
        false,
        true,
        false,
        900,
        Some(720.0)
    ));
}

#[test]
fn source_control_diff_split_weights_follow_default_ratio() {
    assert_split_weights(source_control_diff_split_weights(1.0, 0.35), 0.35, 0.65);
    assert_split_weights(source_control_diff_split_weights(0.5, 0.8), 0.4, 0.1);
    assert_split_weights(source_control_diff_split_weights(1.0, f32::NAN), 0.5, 0.5);
    assert_split_weights(source_control_diff_split_weights(1.0, 0.0), 0.01, 0.99);
    assert_split_weights(source_control_diff_split_weights(0.0, 0.25), 0.25, 0.75);
}

#[test]
fn source_control_diff_skips_side_by_side_payload_builder_when_disabled() {
    let mut called = false;

    let open: Option<()> = source_control_side_by_side_open_if_needed(false, || {
        called = true;
        Err("side-by-side failed".to_owned())
    });

    assert!(open.is_none());
    assert!(!called);
}

#[test]
fn source_control_diff_falls_back_inline_when_side_by_side_builder_fails() {
    let open: Option<()> = source_control_side_by_side_open_if_needed(true, || {
        Err::<(), _>("side-by-side failed".to_owned())
    });

    assert!(open.is_none());
}

fn assert_split_weights(actual: (f32, f32), expected_left: f32, expected_right: f32) {
    assert!((actual.0 - expected_left).abs() < 0.0001, "{actual:?}");
    assert!((actual.1 - expected_right).abs() < 0.0001, "{actual:?}");
}

fn app_for_test(root: PathBuf) -> KuroyaApp {
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

fn diff_request(path: &str) -> SourceControlDiffOpenRequest {
    SourceControlDiffOpenRequest::Worktree {
        path: PathBuf::from(path),
        focus_hunk: None,
    }
}

fn diff_open_outcome(label: &str) -> Result<SourceControlDiffOpenOutcome, String> {
    Ok(SourceControlDiffOpenOutcome::Open(Box::new(
        SourceControlDiffOpen {
            open: VirtualDiffOpen {
                label: label.to_owned(),
                diff: format!("diff --git a/{label} b/{label}\n"),
                target: label.to_owned(),
                kind: "changes",
                source: None,
            },
            focus_hunk: None,
            side_by_side: None,
        },
    )))
}

#[test]
fn stale_source_control_diff_open_finished_after_newer_request_is_ignored() {
    let root = PathBuf::from("workspace");
    let mut app = app_for_test(root.clone());
    app.status = "newer request pending".to_owned();
    app.source_control_diff_open_active_request_id = 2;

    app.apply_source_control_diff_open_finished(
        root.clone(),
        root,
        app.workspace_event_generation,
        1,
        diff_request("src/main.rs"),
        Ok(SourceControlDiffOpenOutcome::Status(
            "stale diff completed".to_owned(),
        )),
    );

    assert_eq!(app.status, "newer request pending");
}

#[test]
fn stale_source_control_diff_open_finished_after_generation_change_is_ignored() {
    let root = PathBuf::from("workspace");
    let mut app = app_for_test(root.clone());
    let stale_generation = app.workspace_event_generation;
    app.workspace_event_generation = stale_generation + 1;
    app.status = "current workspace".to_owned();
    app.source_control_diff_open_active_request_id = 1;

    app.apply_source_control_diff_open_finished(
        root.clone(),
        root,
        stale_generation,
        1,
        diff_request("src/main.rs"),
        Ok(SourceControlDiffOpenOutcome::Status(
            "stale generation".to_owned(),
        )),
    );

    assert_eq!(app.status, "current workspace");
}

#[test]
fn stale_source_control_diff_open_finished_after_operation_root_change_is_ignored() {
    let root = PathBuf::from("workspace");
    let stale_operation_root = root.join("old-repo");
    let mut app = app_for_test(root.clone());
    app.status = "current operation root".to_owned();
    app.source_control_diff_open_active_request_id = 1;

    app.apply_source_control_diff_open_finished(
        root,
        stale_operation_root,
        app.workspace_event_generation,
        1,
        diff_request("src/main.rs"),
        Ok(SourceControlDiffOpenOutcome::Status(
            "stale operation root".to_owned(),
        )),
    );

    assert_eq!(app.status, "current operation root");
}

#[test]
fn stale_source_control_diff_open_finished_does_not_open_buffer() {
    let root = PathBuf::from("workspace");
    let mut app = app_for_test(root.clone());
    app.status = "current diff".to_owned();
    app.source_control_diff_open_active_request_id = 2;

    app.apply_source_control_diff_open_finished(
        root.clone(),
        root,
        app.workspace_event_generation,
        1,
        diff_request("src/main.rs"),
        diff_open_outcome("src/main.rs"),
    );

    assert!(app.buffers.is_empty());
    assert!(app.virtual_buffer_labels.is_empty());
    assert_eq!(app.status, "current diff");
}

#[test]
fn current_source_control_diff_open_finished_applies_status() {
    let root = PathBuf::from("workspace");
    let mut app = app_for_test(root.clone());
    app.source_control_diff_open_active_request_id = 7;

    app.apply_source_control_diff_open_finished(
        root.clone(),
        root,
        app.workspace_event_generation,
        7,
        diff_request("src/main.rs"),
        Ok(SourceControlDiffOpenOutcome::Status(
            "diff ready".to_owned(),
        )),
    );

    assert_eq!(app.status, "diff ready");
}

#[test]
fn source_control_diff_request_ids_wrap_to_nonzero_and_skip_active_id() {
    let mut next_request_id = u64::MAX;
    let mut active_request_id = 0;

    let request_id = super::reserve_source_control_diff_open_request_id_state(
        &mut next_request_id,
        &mut active_request_id,
    );

    assert_eq!(request_id, 1);
    assert_eq!(next_request_id, 1);
    assert_eq!(active_request_id, 1);

    next_request_id = u64::MAX;
    active_request_id = 1;
    let request_id = super::reserve_source_control_diff_open_request_id_state(
        &mut next_request_id,
        &mut active_request_id,
    );

    assert_eq!(request_id, 2);
    assert_eq!(next_request_id, 2);
    assert_eq!(active_request_id, 2);
}

#[test]
fn shared_source_control_diff_open_request_allows_batch_completions() {
    let root = PathBuf::from("workspace");
    let mut app = app_for_test(root.clone());
    app.source_control_diff_open_active_request_id = 9;

    app.apply_source_control_diff_open_finished(
        root.clone(),
        root.clone(),
        app.workspace_event_generation,
        9,
        diff_request("src/a.rs"),
        diff_open_outcome("src/a.rs"),
    );
    app.apply_source_control_diff_open_finished(
        root.clone(),
        root,
        app.workspace_event_generation,
        9,
        diff_request("src/b.rs"),
        diff_open_outcome("src/b.rs"),
    );

    assert_eq!(app.virtual_buffer_labels.len(), 2);
}

#[test]
fn source_control_diff_jobs_preserve_request_kind() {
    let worktree = TextBuffer::from_text(1, Some(PathBuf::from("a.rs")), "a".to_owned());
    let head = TextBuffer::from_text(2, Some(PathBuf::from("c.rs")), "c".to_owned());
    let worktree_job = SourceControlDiffOpenJob::worktree(
        PathBuf::from("a.rs"),
        SourceControlDiffText::open_buffer(&worktree, 99),
        Some(1),
    );
    let staged_job = SourceControlDiffOpenJob::staged(PathBuf::from("b.rs"), None);
    let head_job = SourceControlDiffOpenJob::head(
        PathBuf::from("c.rs"),
        SourceControlDiffText::open_buffer(&head, 99),
    );

    assert!(matches!(
        worktree_job.request,
        SourceControlDiffOpenRequest::Worktree {
            focus_hunk: Some(1),
            ..
        }
    ));
    assert!(matches!(
        staged_job.request,
        SourceControlDiffOpenRequest::Staged { .. }
    ));
    assert!(matches!(
        head_job.request,
        SourceControlDiffOpenRequest::Head { .. }
    ));
    assert!(worktree_job.prepare_side_by_side);
    assert!(staged_job.prepare_side_by_side);
    assert!(head_job.prepare_side_by_side);
    assert!(DiffOptions::default().max_file_size_bytes > 0);
}

#[test]
fn source_control_diff_detail_sanitizes_hunk_path_labels() {
    let path = hostile_display_path();
    let request = SourceControlDiffOpenRequest::Worktree {
        path: path.clone(),
        focus_hunk: Some(2),
    };

    let detail = source_control_diff_open_detail(&request);
    let pending = source_control_diff_open_pending_status(&request);
    let path_label = source_control_diff_path_label(&path);

    assert_display_safe(&detail);
    assert_display_safe(&pending);
    assert_display_safe(&path_label);
    assert!(detail.starts_with("worktree "));
    assert!(detail.ends_with(" hunk 3"));
    assert!(detail.contains("..."), "{detail}");
    assert!(path_label.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);
    let raw_file_name = format!("bad\n{}\u{202e}tail.rs", "very-long-".repeat(32));
    assert_eq!(
        path.file_name().and_then(|name| name.to_str()),
        Some(raw_file_name.as_str())
    );
}

#[test]
fn source_control_diff_error_statuses_sanitize_path_and_error_text() {
    let path = hostile_display_path();
    let error = hostile_error();
    let head_error = format!("HEAD failed\r\u{2066}{}", "x".repeat(256));

    let statuses = [
        source_control_diff_read_failure_status(&path, &error),
        source_control_head_compare_failure_status(&path, &error),
        source_control_head_side_open_failure_status(&path, &error),
        source_control_index_and_head_side_open_failure_status(&path, &error, &head_error),
    ];

    for status in statuses {
        assert_display_safe(&status);
        assert!(status.contains("..."), "{status}");
    }

    let error_label = source_control_diff_error_label(&error);
    assert_display_safe(&error_label);
    assert!(error_label.contains("..."), "{error_label}");
    assert!(error_label.chars().count() <= DISPLAY_ERROR_LABEL_MAX_CHARS);
    assert!(error.contains('\n'));
    assert!(error.contains('\u{202e}'));
}

#[test]
fn source_control_diff_target_statuses_sanitize_raw_targets() {
    let raw_target = format!(
        "target\n{}\u{202e}\u{2069}.rs",
        "segment-".repeat(DISPLAY_PATH_LABEL_MAX_CHARS)
    );

    let status = source_control_side_by_side_open_status("changes", &raw_target);
    let path_target = source_control_diff_target_label(&hostile_display_path());
    let cached_status = source_control_side_by_side_open_status("changes", &path_target);

    assert_display_safe(&status);
    assert_display_safe(&path_target);
    assert_display_safe(&cached_status);
    assert!(status.starts_with("Opened side-by-side changes for "));
    assert!(cached_status.starts_with("Opened side-by-side changes for "));
    assert!(status.contains("..."), "{status}");
    assert!(cached_status.contains("..."), "{cached_status}");
    assert!(path_target.contains("..."), "{path_target}");
    assert!(path_target.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);
    assert!(raw_target.contains('\n'));
    assert!(raw_target.contains('\u{202e}'));
}

#[test]
fn source_control_diff_path_labels_bound_titles_without_losing_suffix() {
    let path = hostile_display_path();
    let labels = SourceControlDiffPathLabels::new(&path);

    let title = labels.diff_title("Staged Changes");
    let target = labels.target_label();

    assert_display_safe(&title);
    assert_display_safe(&target);
    assert!(title.ends_with(" (Staged Changes)"), "{title}");
    assert!(title.contains("..."), "{title}");
    assert!(title.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);
    assert!(target.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);
    assert!(path.to_string_lossy().contains('\n'));
    assert!(path.to_string_lossy().contains('\u{202e}'));
}

#[test]
fn side_by_side_buffers_reuse_cached_target_labels_and_keep_raw_paths() {
    let path = hostile_display_path();
    let labels = SourceControlDiffPathLabels::new(&path);
    let target = labels.target_label();
    let open = staged_side_by_side_open_with_labels(
        &path,
        Some("head text".to_owned()),
        Some("index text".to_owned()),
        &labels,
    );

    assert_eq!(open.target, target);
    assert_display_safe(&open.target);
    assert!(open.target.contains("..."), "{:?}", open.target);

    for buffer in [&open.base, &open.source] {
        match buffer {
            SourceControlSideBySideBuffer::Virtual {
                label,
                path: buffer_path,
                target: buffer_target,
                ..
            } => {
                assert_display_safe(label);
                assert_eq!(buffer_path, &path);
                assert_eq!(buffer_target, &target);
                assert!(buffer_path.to_string_lossy().contains('\n'));
                assert!(buffer_path.to_string_lossy().contains('\u{202e}'));
            }
            other => panic!("expected virtual side-by-side buffer, got {other:?}"),
        }
    }
}

#[test]
fn source_control_diff_jobs_preserve_raw_request_paths() {
    let path = hostile_display_path();
    let job =
        SourceControlDiffOpenJob::worktree(path.clone(), SourceControlDiffText::Deleted, Some(4));

    match &job.request {
        SourceControlDiffOpenRequest::Worktree {
            path: request_path,
            focus_hunk,
        } => {
            assert_eq!(request_path, &path);
            assert_eq!(*focus_hunk, Some(4));
            assert!(request_path.to_string_lossy().contains('\n'));
            assert!(request_path.to_string_lossy().contains('\u{202e}'));
        }
        other => panic!("expected worktree request, got {other:?}"),
    }
}
