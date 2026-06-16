use super::{
    SOURCE_CONTROL_COMMIT_MESSAGE_DISPLAY_MAX_CHARS,
    SOURCE_CONTROL_PROTECTED_BRANCH_PATTERN_MAX_CHARS,
    SOURCE_CONTROL_SAVE_PROMPT_FILE_LABEL_MAX_CHARS, source_control_commit_message_display,
    source_control_commit_message_display_cow, source_control_commit_save_prompt_body,
    source_control_commit_save_prompt_labels, source_control_dirty_buffer_ids,
    source_control_empty_commit_confirmation_body,
    source_control_protected_branch_new_branch_required_status_display,
    source_control_protected_branch_pattern_display,
    source_control_protected_branch_pattern_display_cow,
    source_control_protected_branch_prompt_body_display,
    source_control_protected_branch_prompt_title_display, source_control_save_prompt_file_label,
    source_control_save_prompt_file_label_cow, source_control_save_remaining_count,
    source_control_saving_prompt_body, source_control_smart_commit_settings_save_failure_status,
    source_control_stash_save_prompt_body, source_control_stash_save_prompt_labels,
};
use crate::{
    app_state::{PendingFileReload, QueuedFileReload},
    source_control_runtime::source_control_app_for_test,
    transient_state::{PendingSourceControlCommitSave, PendingSourceControlStashSave},
};
use kuroya_core::{GitSmartCommitChanges, TextBuffer};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    path::PathBuf,
};

type DialogDisplayCowHelper = for<'a> fn(&'a str) -> Cow<'a, str>;

#[test]
fn protected_branch_prompt_display_strings_sanitize_branch_and_pattern() {
    let branch = format!("main\n{}\u{202e}tail", "branch-".repeat(80));
    let pattern = format!("release/*\r\n{}\u{2066}tail", "pattern-".repeat(80));

    let values = [
        source_control_protected_branch_prompt_title_display(&branch),
        source_control_protected_branch_prompt_body_display(&branch, &pattern),
        source_control_protected_branch_new_branch_required_status_display(&branch, &pattern),
    ];

    for value in values {
        assert_display_text_is_safe(&value);
        assert!(value.contains("main branch-"));
        assert!(value.contains("..."));
    }
}

#[test]
fn dialog_display_label_cows_borrow_clean_ascii_and_unicode() {
    let helpers: [(&str, DialogDisplayCowHelper); 3] = [
        (
            "commit message",
            source_control_commit_message_display_cow as DialogDisplayCowHelper,
        ),
        (
            "save prompt file",
            source_control_save_prompt_file_label_cow as DialogDisplayCowHelper,
        ),
        (
            "protected branch pattern",
            source_control_protected_branch_pattern_display_cow as DialogDisplayCowHelper,
        ),
    ];

    for (context, helper) in helpers {
        for value in ["clean-label", "clean-\u{03bb}-label"] {
            assert_borrowed_display_label(context, helper(value), value);
        }
    }
}

#[test]
fn dialog_display_label_cows_own_dirty_truncated_and_fallback_values() {
    let helpers: [(&str, DialogDisplayCowHelper, &str, usize); 3] = [
        (
            "commit message",
            source_control_commit_message_display_cow as DialogDisplayCowHelper,
            "commit message",
            SOURCE_CONTROL_COMMIT_MESSAGE_DISPLAY_MAX_CHARS,
        ),
        (
            "save prompt file",
            source_control_save_prompt_file_label_cow as DialogDisplayCowHelper,
            "file",
            SOURCE_CONTROL_SAVE_PROMPT_FILE_LABEL_MAX_CHARS,
        ),
        (
            "protected branch pattern",
            source_control_protected_branch_pattern_display_cow as DialogDisplayCowHelper,
            "protected branch pattern",
            SOURCE_CONTROL_PROTECTED_BRANCH_PATTERN_MAX_CHARS,
        ),
    ];

    for (context, helper, fallback, max_chars) in helpers {
        let dirty = assert_owned_display_label(context, helper("alpha\nbeta\u{202e}\u{2066}"));
        assert_eq!(dirty, "alpha beta");

        let raw_long = format!("alpha-{}", "very-long-".repeat(40));
        let truncated = assert_owned_display_label(context, helper(&raw_long));
        assert!(truncated.contains("..."));
        assert!(truncated.chars().count() <= max_chars);

        let fallback_label = assert_owned_display_label(context, helper("\n\r\u{202e}\u{2066}"));
        assert_eq!(fallback_label, fallback);
    }
}

#[test]
fn dialog_display_label_string_wrappers_match_cow_output() {
    let raw_long = format!("display-{}", "wrapper-".repeat(40));
    let cases = [
        "clean-label",
        "clean-\u{03bb}-label",
        "  clean-label  ",
        "alpha\nbeta\u{202e}",
        "\n\r\u{202e}\u{2066}",
        raw_long.as_str(),
    ];

    for value in cases {
        assert_eq!(
            source_control_commit_message_display(value),
            source_control_commit_message_display_cow(value).into_owned()
        );
        assert_eq!(
            source_control_save_prompt_file_label(value),
            source_control_save_prompt_file_label_cow(value).into_owned()
        );
        assert_eq!(
            source_control_protected_branch_pattern_display(value),
            source_control_protected_branch_pattern_display_cow(value).into_owned()
        );
    }
}

#[test]
fn commit_message_display_cow_preserves_existing_trim_semantics() {
    let label = source_control_commit_message_display_cow("  ship it  ");

    assert_eq!(label.as_ref(), "ship it");
    assert!(matches!(label, Cow::Owned(_)));
    assert_eq!(
        source_control_commit_message_display("  ship it  "),
        "ship it"
    );
}

#[test]
fn smart_commit_settings_save_failure_status_sanitizes_error_details() {
    let error = format!("first line\nsecond line \u{202e}{}", "x".repeat(400));

    let status =
        source_control_smart_commit_settings_save_failure_status("Smart commit enabled", &error);

    assert!(
        status
            .starts_with("Smart commit enabled, but settings save failed: first line second line ")
    );
    assert_display_text_is_safe(&status);
    assert!(status.contains("..."));
}

#[test]
fn source_control_saving_prompt_body_uses_file_count_labels() {
    assert_eq!(
        source_control_saving_prompt_body("committing", 1),
        "Saving 1 file before committing."
    );
    assert_eq!(
        source_control_saving_prompt_body("stashing", 2),
        "Saving 2 files before stashing."
    );
}

#[test]
fn source_control_save_prompt_bodies_reuse_action_specific_text() {
    let buffers = vec![
        TextBuffer::from_text(
            7,
            Some(PathBuf::from("workspace/src/main.rs")),
            "dirty\n".to_owned(),
        ),
        TextBuffer::from_text(
            8,
            Some(PathBuf::from("workspace/src/lib.rs")),
            "dirty\n".to_owned(),
        ),
    ];

    assert_eq!(
        source_control_commit_save_prompt_body(&buffers, &[7, 8]),
        "Save main.rs and 1 other file before committing, commit anyway, or cancel."
    );
    assert_eq!(
        source_control_stash_save_prompt_body(&buffers, &[7]),
        "Save main.rs before stashing, stash anyway, or cancel."
    );
    assert_eq!(
        source_control_commit_save_prompt_body(&buffers, &[99]),
        "Save files before committing, commit anyway, or cancel."
    );
}

#[test]
fn source_control_save_prompt_body_keeps_pending_id_order_with_indexed_lookup() {
    let buffers = vec![
        TextBuffer::from_text(
            8,
            Some(PathBuf::from("workspace/src/lib.rs")),
            "dirty\n".to_owned(),
        ),
        TextBuffer::from_text(
            7,
            Some(PathBuf::from("workspace/src/main.rs")),
            "dirty\n".to_owned(),
        ),
    ];

    assert_eq!(
        source_control_commit_save_prompt_body(&buffers, &[7, 8]),
        "Save main.rs and 1 other file before committing, commit anyway, or cancel."
    );
    assert_eq!(
        source_control_commit_save_prompt_body(&buffers, &[8, 7]),
        "Save lib.rs and 1 other file before committing, commit anyway, or cancel."
    );
}

#[test]
fn empty_commit_confirmation_body_sanitizes_and_bounds_display_message() {
    let message = format!("subject\n{}\u{202e}tail", "body-".repeat(80));

    let body = source_control_empty_commit_confirmation_body(&message);

    assert_display_text_is_safe(&body);
    assert!(body.contains("subject body-"));
    assert!(body.contains("..."));
    assert!(
        body.chars().count()
            <= "Create an empty commit with message \"\"?".chars().count()
                + SOURCE_CONTROL_COMMIT_MESSAGE_DISPLAY_MAX_CHARS
    );
}

#[test]
fn source_control_save_prompt_body_bounds_first_file_label() {
    let buffers = vec![TextBuffer::from_text(
        7,
        Some(PathBuf::from(format!(
            "workspace/src/bad\n{}\u{202e}.rs",
            "very-long-name-".repeat(24)
        ))),
        "dirty\n".to_owned(),
    )];

    let body = source_control_commit_save_prompt_body(&buffers, &[7]);

    assert_display_text_is_safe(&body);
    assert!(body.contains("..."));
    assert!(
        body.chars().count()
            <= "Save  before committing, commit anyway, or cancel."
                .chars()
                .count()
                + SOURCE_CONTROL_SAVE_PROMPT_FILE_LABEL_MAX_CHARS
    );
}

#[test]
fn source_control_save_prompt_labels_are_prepared_once_per_dialog() {
    let buffers = vec![
        TextBuffer::from_text(
            7,
            Some(PathBuf::from("workspace/src/main.rs")),
            "dirty\n".to_owned(),
        ),
        TextBuffer::from_text(
            8,
            Some(PathBuf::from("workspace/src/lib.rs")),
            "dirty\n".to_owned(),
        ),
    ];

    let commit = source_control_commit_save_prompt_labels(&buffers, &[7, 8]);
    assert_eq!(commit.title, "2 files have unsaved changes");
    assert_eq!(
        commit.body,
        "Save main.rs and 1 other file before committing, commit anyway, or cancel."
    );
    assert_eq!(commit.primary_label, "Save All and Commit");

    let stash = source_control_stash_save_prompt_labels(&buffers, &[7]);
    assert_eq!(stash.title, "1 file has unsaved changes");
    assert_eq!(
        stash.body,
        "Save main.rs before stashing, stash anyway, or cancel."
    );
    assert_eq!(stash.primary_label, "Save and Stash");
}

#[test]
fn source_control_save_remaining_count_includes_pending_format_and_dirty_buffers() {
    let root = PathBuf::from("workspace");
    let mut dirty = TextBuffer::from_text(
        4,
        Some(root.join("src/dirty.rs")),
        "fn dirty() {}\n".to_owned(),
    );
    dirty.mark_dirty();
    let clean = TextBuffer::from_text(
        5,
        Some(root.join("src/clean.rs")),
        "fn clean() {}\n".to_owned(),
    );
    let buffers = vec![dirty, clean];
    let in_flight = HashSet::from([1]);
    let queued = HashMap::from([(2, root.join("src/queued.rs"))]);
    let pending_format = HashMap::from([(3, ())]);

    assert_eq!(
        source_control_save_remaining_count(
            &[1, 2, 3, 4, 5],
            &buffers,
            &in_flight,
            &queued,
            &pending_format,
        ),
        4
    );
}

#[test]
fn source_control_dirty_buffer_ids_are_indexed_once_for_save_counts() {
    let root = PathBuf::from("workspace");
    let mut dirty = TextBuffer::from_text(
        4,
        Some(root.join("src/dirty.rs")),
        "fn dirty() {}\n".to_owned(),
    );
    dirty.mark_dirty();
    let clean = TextBuffer::from_text(
        5,
        Some(root.join("src/clean.rs")),
        "fn clean() {}\n".to_owned(),
    );
    let mut other_dirty = TextBuffer::from_text(
        6,
        Some(root.join("src/other.rs")),
        "fn other() {}\n".to_owned(),
    );
    other_dirty.mark_dirty();

    let dirty_ids = source_control_dirty_buffer_ids(&[dirty, clean, other_dirty]);

    assert!(dirty_ids.contains(&4));
    assert!(!dirty_ids.contains(&5));
    assert!(dirty_ids.contains(&6));
}

#[test]
fn commit_save_prompt_begin_dedupes_stale_and_clean_buffer_targets() {
    let root = PathBuf::from("workspace");
    let mut app = source_control_app_for_test(root.clone(), true);
    let mut dirty = TextBuffer::from_text(
        7,
        Some(root.join("src/dirty.rs")),
        "fn dirty() {}\n".to_owned(),
    );
    dirty.mark_dirty();
    let clean = TextBuffer::from_text(
        8,
        Some(root.join("src/clean.rs")),
        "fn clean() {}\n".to_owned(),
    );
    app.buffers.push(dirty);
    app.buffers.push(clean);
    let request_id = app.reserve_source_control_commit_request();

    app.begin_source_control_commit_save_prompt(
        request_id,
        "ship it".to_owned(),
        Some(GitSmartCommitChanges::Tracked),
        false,
        vec![99, 7, 8, 7, 99],
    );

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
    assert!(app.source_control_commit_in_flight_request_ids.is_empty());
    assert!(app.active_async_tasks.is_empty());
}

#[test]
fn stale_commit_prompt_begins_do_not_replace_current_request_state() {
    let mut app = source_control_app_for_test(PathBuf::from("workspace"), true);
    let older = app.reserve_source_control_commit_request();
    let newer = app.reserve_source_control_commit_request();

    app.begin_source_control_smart_commit_suggestion(
        older,
        "older".to_owned(),
        GitSmartCommitChanges::All,
        2,
    );
    app.begin_source_control_empty_commit_confirmation(older, "older".to_owned());
    app.begin_source_control_protected_branch_commit_prompt(
        older,
        "older".to_owned(),
        None,
        false,
        "main".to_owned(),
        "main".to_owned(),
    );
    app.begin_source_control_commit_save_prompt(older, "older".to_owned(), None, false, vec![7]);

    assert_eq!(app.source_control_commit_active_request_id, newer);
    assert!(app.pending_source_control_smart_commit.is_none());
    assert!(app.pending_source_control_empty_commit.is_none());
    assert!(app.pending_source_control_protected_branch_commit.is_none());
    assert!(app.pending_source_control_commit_save.is_none());
}

#[test]
fn stale_commit_save_confirmation_does_not_cancel_current_request() {
    let mut app = source_control_app_for_test(PathBuf::from("workspace"), true);
    let older = app.reserve_source_control_commit_request();
    let newer = app.reserve_source_control_commit_request();
    app.pending_source_control_commit_save = Some(PendingSourceControlCommitSave::Confirm {
        request_id: older,
        message: "older".to_owned(),
        smart_commit_changes: Some(GitSmartCommitChanges::Tracked),
        allow_empty: false,
        ids: vec![7],
    });

    app.confirm_source_control_commit_without_saving();

    assert_eq!(app.source_control_commit_active_request_id, newer);
    assert!(app.source_control_commit_in_flight_request_ids.is_empty());
    assert!(app.pending_source_control_commit_save.is_none());
    assert!(app.active_async_tasks.is_empty());
}

#[test]
fn source_control_commit_save_pauses_for_pending_clean_reload_without_raw_marker() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = source_control_app_for_test(root, true);
    let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "dirty\n".to_owned());
    let version = buffer.version();
    buffer.mark_dirty();
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
    app.pending_source_control_commit_save = Some(PendingSourceControlCommitSave::Confirm {
        request_id: 1,
        message: "ship it".to_owned(),
        smart_commit_changes: Some(GitSmartCommitChanges::Tracked),
        allow_empty: false,
        ids: vec![7],
    });

    app.save_source_control_commit_files();

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
    assert!(!app.in_flight_saves.contains(&7));
    assert!(app.status.contains("changed on disk"));
    assert!(app.active_async_tasks.is_empty());
}

#[test]
fn source_control_commit_save_pauses_for_queued_clean_reload_without_raw_marker() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = source_control_app_for_test(root, true);
    let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "dirty\n".to_owned());
    buffer.mark_dirty();
    app.buffers.push(buffer);
    app.queued_file_reloads.insert(
        7,
        QueuedFileReload {
            path,
            force_dirty: false,
        },
    );
    app.pending_source_control_commit_save = Some(PendingSourceControlCommitSave::Confirm {
        request_id: 1,
        message: "ship it".to_owned(),
        smart_commit_changes: Some(GitSmartCommitChanges::Tracked),
        allow_empty: false,
        ids: vec![7],
    });

    app.save_source_control_commit_files();

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
    assert!(!app.in_flight_saves.contains(&7));
    assert!(app.status.contains("changed on disk"));
    assert!(app.active_async_tasks.is_empty());
}

#[test]
fn source_control_stash_save_pauses_for_pending_clean_reload_without_raw_marker() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = source_control_app_for_test(root, true);
    let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "dirty\n".to_owned());
    let version = buffer.version();
    buffer.mark_dirty();
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
    app.pending_source_control_stash_save = Some(PendingSourceControlStashSave::Confirm {
        message: "work in progress".to_owned(),
        ids: vec![7],
    });

    app.save_source_control_stash_files();

    assert!(app.external_change_buffers.is_empty());
    assert!(matches!(
        app.pending_source_control_stash_save,
        Some(PendingSourceControlStashSave::Confirm {
            ref message,
            ref ids,
        }) if message == "work in progress" && ids == &vec![7]
    ));
    assert!(!app.in_flight_saves.contains(&7));
    assert!(app.status.contains("changed on disk"));
    assert!(app.active_async_tasks.is_empty());
}

#[test]
fn source_control_stash_save_pauses_for_queued_clean_reload_without_raw_marker() {
    let root = PathBuf::from("workspace");
    let path = root.join("src/main.rs");
    let mut app = source_control_app_for_test(root, true);
    let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "dirty\n".to_owned());
    buffer.mark_dirty();
    app.buffers.push(buffer);
    app.queued_file_reloads.insert(
        7,
        QueuedFileReload {
            path,
            force_dirty: false,
        },
    );
    app.pending_source_control_stash_save = Some(PendingSourceControlStashSave::Confirm {
        message: "work in progress".to_owned(),
        ids: vec![7],
    });

    app.save_source_control_stash_files();

    assert!(app.external_change_buffers.is_empty());
    assert!(matches!(
        app.pending_source_control_stash_save,
        Some(PendingSourceControlStashSave::Confirm {
            ref message,
            ref ids,
        }) if message == "work in progress" && ids == &vec![7]
    ));
    assert!(!app.in_flight_saves.contains(&7));
    assert!(app.status.contains("changed on disk"));
    assert!(app.active_async_tasks.is_empty());
}

#[test]
fn protected_branch_prompt_keeps_raw_branch_and_pattern_in_pending_state() {
    let mut app = source_control_app_for_test(PathBuf::from("workspace"), true);
    let raw_branch = "main\n\u{202e}raw".to_owned();
    let raw_pattern = "main*\u{2066}\nraw".to_owned();

    app.begin_source_control_protected_branch_commit_prompt(
        1,
        "commit message".to_owned(),
        Some(GitSmartCommitChanges::All),
        false,
        raw_branch.clone(),
        raw_pattern.clone(),
    );

    let pending = app
        .pending_source_control_protected_branch_commit
        .expect("protected branch prompt should be pending");
    assert_eq!(pending.branch, raw_branch);
    assert_eq!(pending.pattern, raw_pattern);
}

fn assert_display_text_is_safe(value: &str) {
    assert!(
        !value.chars().any(is_unsafe_display_char),
        "display text contains unsafe characters: {value:?}"
    );
    assert!(
        value.chars().count() <= SOURCE_CONTROL_PROTECTED_BRANCH_PATTERN_MAX_CHARS * 3,
        "display text should be bounded: {} chars",
        value.chars().count()
    );
}

fn assert_borrowed_display_label(context: &str, label: Cow<'_, str>, expected: &str) {
    match label {
        Cow::Borrowed(label) => assert_eq!(label, expected),
        Cow::Owned(label) => {
            panic!("expected {context} display label to borrow, got owned {label:?}")
        }
    }
}

fn assert_owned_display_label(context: &str, label: Cow<'_, str>) -> String {
    match label {
        Cow::Owned(label) => label,
        Cow::Borrowed(label) => {
            panic!("expected {context} display label to own, got borrowed {label:?}")
        }
    }
}

fn is_unsafe_display_char(ch: char) -> bool {
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
