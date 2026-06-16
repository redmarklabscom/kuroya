use crate::{
    persistence::RecoveredBuffer,
    recovery::{
        recovery_path_key, recovery_snapshot_draft_for_buffers, recovery_snapshot_for_buffers,
    },
};
use kuroya_core::TextBuffer;
use std::path::PathBuf;

#[test]
fn recovery_snapshots_skip_oversized_dirty_buffers() {
    let small_path = PathBuf::from("workspace/src/main.rs");
    let large_path = PathBuf::from("workspace/src/large.rs");
    let mut small = TextBuffer::from_text(1, Some(small_path.clone()), "small".to_owned());
    small.mark_dirty();
    let mut large = TextBuffer::from_text(2, Some(large_path.clone()), "too-large".to_owned());
    large.mark_dirty();
    let mut untitled = TextBuffer::from_text(3, None, "scratch".to_owned());
    untitled.mark_dirty();
    let clean = TextBuffer::from_text(
        4,
        Some(PathBuf::from("workspace/src/clean.rs")),
        "clean".to_owned(),
    );

    let snapshot = recovery_snapshot_for_buffers(&[small, large, untitled, clean], 7, 32);

    assert_eq!(
        snapshot.recovered,
        vec![
            RecoveredBuffer {
                path: Some(small_path),
                display_name: "main.rs".to_owned(),
                text: "small".to_owned(),
            },
            RecoveredBuffer {
                path: None,
                display_name: "Untitled".to_owned(),
                text: "scratch".to_owned(),
            },
        ]
    );
    assert_eq!(snapshot.skipped.len(), 1);
    assert_eq!(snapshot.skipped[0].path, Some(large_path));
    assert_eq!(snapshot.skipped[0].display_name, "large.rs");
    assert_eq!(snapshot.skipped[0].bytes, "too-large".len());
    assert!(snapshot.skipped[0].reason.contains("per-buffer"));
}

#[test]
fn recovery_snapshots_respect_total_session_budget() {
    let first_path = PathBuf::from("workspace/src/first.rs");
    let second_path = PathBuf::from("workspace/src/second.rs");
    let mut first = TextBuffer::from_text(1, Some(first_path.clone()), "first".to_owned());
    first.mark_dirty();
    let mut second = TextBuffer::from_text(2, Some(second_path.clone()), "second".to_owned());
    second.mark_dirty();

    let snapshot = recovery_snapshot_for_buffers(&[first, second], 16, 10);

    assert_eq!(
        snapshot.recovered,
        vec![RecoveredBuffer {
            path: Some(first_path),
            display_name: "first.rs".to_owned(),
            text: "first".to_owned(),
        }]
    );
    assert_eq!(snapshot.skipped.len(), 1);
    assert_eq!(snapshot.skipped[0].path, Some(second_path));
    assert_eq!(snapshot.skipped[0].bytes, "second".len());
    assert!(snapshot.skipped[0].reason.contains("total recovery"));
}

#[test]
fn recovery_snapshots_dedupe_dirty_buffers_by_path_using_newest_version() {
    let path = PathBuf::from("workspace/src/main.rs");
    let mut older = TextBuffer::from_text(1, Some(path.clone()), "older".to_owned());
    older.mark_dirty();
    let mut untitled = TextBuffer::from_text(2, None, "scratch".to_owned());
    untitled.mark_dirty();
    let mut newer = TextBuffer::from_text(3, Some(path.clone()), "new".to_owned());
    assert!(newer.replace_range(3..3, "er"));
    let mut second_untitled = TextBuffer::from_text(4, None, "scratch 2".to_owned());
    second_untitled.mark_dirty();

    let snapshot =
        recovery_snapshot_for_buffers(&[older, untitled, newer, second_untitled], 32, 128);

    assert_eq!(
        snapshot.recovered,
        vec![
            RecoveredBuffer {
                path: None,
                display_name: "Untitled".to_owned(),
                text: "scratch".to_owned(),
            },
            RecoveredBuffer {
                path: Some(path.clone()),
                display_name: "main.rs".to_owned(),
                text: "newer".to_owned(),
            },
            RecoveredBuffer {
                path: None,
                display_name: "Untitled".to_owned(),
                text: "scratch 2".to_owned(),
            },
        ]
    );
    assert_eq!(snapshot.skipped.len(), 1);
    assert_eq!(snapshot.skipped[0].path, Some(path));
    assert_eq!(snapshot.skipped[0].display_name, "main.rs");
    assert_eq!(snapshot.skipped[0].bytes, "older".len());
    assert!(
        snapshot.skipped[0]
            .reason
            .contains("duplicate recovery path")
    );
}

#[test]
fn recovery_snapshots_recover_persistable_duplicate_when_newest_exceeds_per_buffer_limit() {
    let path = PathBuf::from("workspace/src/main.rs");
    let mut older = TextBuffer::from_text(1, Some(path.clone()), "fit".to_owned());
    older.mark_dirty();
    let mut newer = TextBuffer::from_text(2, Some(path.clone()), "too".to_owned());
    assert!(newer.replace_range(3..3, "-large"));

    let snapshot = recovery_snapshot_for_buffers(&[older, newer], 4, 64);

    assert_eq!(
        snapshot.recovered,
        vec![RecoveredBuffer {
            path: Some(path.clone()),
            display_name: "main.rs".to_owned(),
            text: "fit".to_owned(),
        }]
    );
    assert_eq!(snapshot.skipped.len(), 1);
    assert_eq!(snapshot.skipped[0].path, Some(path));
    assert_eq!(snapshot.skipped[0].display_name, "main.rs");
    assert!(snapshot.skipped[0].reason.contains("per-buffer"));
}

#[test]
fn recovery_snapshots_recover_persistable_duplicate_when_newest_exceeds_total_session_limit() {
    let path = PathBuf::from("workspace/src/main.rs");
    let mut older = TextBuffer::from_text(1, Some(path.clone()), "fit".to_owned());
    older.mark_dirty();
    let mut newer = TextBuffer::from_text(2, Some(path.clone()), "too".to_owned());
    assert!(newer.replace_range(3..3, "-large"));

    let snapshot = recovery_snapshot_for_buffers(&[older, newer], 64, 4);

    assert_eq!(
        snapshot.recovered,
        vec![RecoveredBuffer {
            path: Some(path.clone()),
            display_name: "main.rs".to_owned(),
            text: "fit".to_owned(),
        }]
    );
    assert_eq!(snapshot.skipped.len(), 1);
    assert_eq!(snapshot.skipped[0].path, Some(path));
    assert_eq!(snapshot.skipped[0].display_name, "main.rs");
    assert!(snapshot.skipped[0].reason.contains("total recovery"));
}

#[test]
fn recovery_snapshots_dedupe_lexically_equivalent_dirty_paths() {
    let direct_path = PathBuf::from("workspace/src/main.rs");
    let equivalent_path = PathBuf::from("workspace/src/../src/main.rs");
    let mut older = TextBuffer::from_text(1, Some(equivalent_path.clone()), "older".to_owned());
    older.mark_dirty();
    let mut newer = TextBuffer::from_text(2, Some(direct_path.clone()), "new".to_owned());
    assert!(newer.replace_range(3..3, "er"));

    let snapshot = recovery_snapshot_for_buffers(&[older, newer], 32, 128);

    assert_eq!(
        snapshot.recovered,
        vec![RecoveredBuffer {
            path: Some(direct_path),
            display_name: "main.rs".to_owned(),
            text: "newer".to_owned(),
        }]
    );
    assert_eq!(snapshot.skipped.len(), 1);
    assert_eq!(snapshot.skipped[0].path, Some(equivalent_path));
    assert_eq!(snapshot.skipped[0].display_name, "main.rs");
    assert_eq!(snapshot.skipped[0].bytes, "older".len());
    assert!(
        snapshot.skipped[0]
            .reason
            .contains("duplicate recovery path")
    );
}

#[test]
fn recovery_path_key_preserves_stacked_leading_parent_dirs() {
    assert_eq!(
        recovery_path_key(&PathBuf::from("../../src/main.rs")),
        PathBuf::from("../../src/main.rs")
    );
    assert_eq!(
        recovery_path_key(&PathBuf::from("workspace/src/../../../main.rs")),
        PathBuf::from("../main.rs")
    );
}

#[test]
fn recovery_path_key_collapses_empty_normalized_paths_to_dot() {
    assert_eq!(recovery_path_key(&PathBuf::new()), PathBuf::from("."));
    assert_eq!(recovery_path_key(&PathBuf::from(".")), PathBuf::from("."));
    assert_eq!(
        recovery_path_key(&PathBuf::from("workspace/..")),
        PathBuf::from(".")
    );
}

#[cfg(windows)]
#[test]
fn recovery_path_key_folds_windows_case() {
    assert_eq!(
        recovery_path_key(&PathBuf::from(r"C:\Workspace\MAIN.rs")),
        PathBuf::from(r"c:\workspace\main.rs")
    );
}

#[cfg(windows)]
#[test]
fn recovery_snapshots_dedupe_windows_paths_case_insensitively() {
    let older_path = PathBuf::from(r"C:\Workspace\main.rs");
    let newer_path = PathBuf::from(r"c:\workspace\MAIN.rs");
    let mut older = TextBuffer::from_text(1, Some(older_path.clone()), "older".to_owned());
    older.mark_dirty();
    let mut newer = TextBuffer::from_text(2, Some(newer_path.clone()), "new".to_owned());
    assert!(newer.replace_range(3..3, "er"));

    let snapshot = recovery_snapshot_for_buffers(&[older, newer], 32, 128);

    assert_eq!(
        snapshot.recovered,
        vec![RecoveredBuffer {
            path: Some(newer_path),
            display_name: "MAIN.rs".to_owned(),
            text: "newer".to_owned(),
        }]
    );
    assert_eq!(snapshot.skipped.len(), 1);
    assert_eq!(snapshot.skipped[0].path, Some(older_path));
    assert!(
        snapshot.skipped[0]
            .reason
            .contains("duplicate recovery path")
    );
}

#[test]
fn recovery_snapshots_do_not_dedupe_stacked_parent_paths() {
    let escaped_path = PathBuf::from("../../src/main.rs");
    let local_path = PathBuf::from("src/main.rs");
    let mut escaped = TextBuffer::from_text(1, Some(escaped_path.clone()), "escaped".to_owned());
    escaped.mark_dirty();
    let mut local = TextBuffer::from_text(2, Some(local_path.clone()), "local".to_owned());
    local.mark_dirty();

    let snapshot = recovery_snapshot_for_buffers(&[escaped, local], 32, 128);

    assert_eq!(
        snapshot.recovered,
        vec![
            RecoveredBuffer {
                path: Some(escaped_path),
                display_name: "main.rs".to_owned(),
                text: "escaped".to_owned(),
            },
            RecoveredBuffer {
                path: Some(local_path),
                display_name: "main.rs".to_owned(),
                text: "local".to_owned(),
            },
        ]
    );
    assert!(snapshot.skipped.is_empty());
}

#[test]
fn recovery_snapshot_draft_preserves_text_without_materializing_immediately() {
    let path = PathBuf::from("workspace/src/main.rs");
    let mut buffer = TextBuffer::from_text(1, Some(path.clone()), "first".to_owned());
    buffer.mark_dirty();

    let draft = recovery_snapshot_draft_for_buffers(&[buffer.clone()], 32, 32);
    assert!(buffer.replace_range(0..5, "second"));

    let snapshot = draft.into_recovery_snapshot();

    assert_eq!(
        snapshot.recovered,
        vec![RecoveredBuffer {
            path: Some(path),
            display_name: "main.rs".to_owned(),
            text: "first".to_owned(),
        }]
    );
}
