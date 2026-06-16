use crate::save_lifecycle::{
    dirty_buffer_save_block_reason, protected_preview_save_block_reason,
    workspace_switch_save_block_reason,
};
use crate::workspace_guard_runtime::{
    workspace_guard_display_path, workspace_guard_status_message,
};
use kuroya_core::TextBuffer;
use std::{collections::HashSet, path::PathBuf};

#[test]
fn workspace_switch_save_guard_blocks_unsafe_dirty_buffers() {
    let mut untitled = TextBuffer::new_untitled(1);
    untitled.mark_dirty();
    let mut changed = TextBuffer::from_text(
        2,
        Some(PathBuf::from("workspace/src/changed.rs")),
        "changed".to_owned(),
    );
    changed.mark_dirty();
    let mut lossy = TextBuffer::from_text(
        3,
        Some(PathBuf::from("workspace/src/lossy.dat")),
        "lossy".to_owned(),
    );
    lossy.mark_dirty();

    assert!(
        workspace_switch_save_block_reason(
            &[1],
            &[untitled],
            &HashSet::new(),
            &HashSet::new(),
            &HashSet::new(),
        )
        .unwrap()
        .contains("Save As")
    );

    assert!(
        workspace_switch_save_block_reason(
            &[2],
            &[changed],
            &HashSet::from([2]),
            &HashSet::new(),
            &HashSet::new(),
        )
        .unwrap()
        .contains("changed on disk")
    );

    assert!(
        workspace_switch_save_block_reason(
            &[3],
            &[lossy],
            &HashSet::new(),
            &HashSet::from([3]),
            &HashSet::new(),
        )
        .unwrap()
        .contains("replacement characters")
    );
}

#[test]
fn dirty_buffer_save_guard_mentions_requested_action() {
    let mut untitled = TextBuffer::new_untitled(1);
    untitled.mark_dirty();
    let mut changed = TextBuffer::from_text(
        2,
        Some(PathBuf::from("workspace/src/changed.rs")),
        "changed".to_owned(),
    );
    changed.mark_dirty();

    assert!(
        dirty_buffer_save_block_reason(
            &[1],
            &[untitled],
            &HashSet::new(),
            &HashSet::new(),
            &HashSet::new(),
            "exiting",
        )
        .unwrap()
        .contains("before exiting")
    );

    assert!(
        dirty_buffer_save_block_reason(
            &[2],
            &[changed],
            &HashSet::from([2]),
            &HashSet::new(),
            &HashSet::new(),
            "exiting",
        )
        .unwrap()
        .contains("before exiting")
    );
}

#[test]
fn save_is_blocked_for_protected_preview_named_buffers() {
    let lossy_named = TextBuffer::from_text(
        1,
        Some(PathBuf::from("workspace/lossy.dat")),
        "ok\u{FFFD}".to_owned(),
    );
    let binary_named = TextBuffer::from_text(
        2,
        Some(PathBuf::from("workspace/binary.dat")),
        "ok\0".to_owned(),
    );
    let untitled = TextBuffer::new_untitled(3);
    let buffers = vec![lossy_named, binary_named, untitled];
    let lossy = HashSet::from([1, 3, 4]);
    let binary = HashSet::from([2, 3, 5]);

    assert_eq!(
        protected_preview_save_block_reason(1, &lossy, &binary, &buffers),
        Some("file was decoded with replacement characters")
    );
    assert_eq!(
        protected_preview_save_block_reason(2, &lossy, &binary, &buffers),
        Some("binary previews are read-only")
    );
    assert_eq!(
        protected_preview_save_block_reason(3, &lossy, &binary, &buffers),
        None
    );
    assert_eq!(
        protected_preview_save_block_reason(4, &lossy, &binary, &buffers),
        None
    );
    assert_eq!(
        protected_preview_save_block_reason(5, &lossy, &binary, &buffers),
        None
    );
}

#[test]
fn save_is_blocked_for_named_read_only_buffers() {
    let mut read_only = TextBuffer::from_text(
        1,
        Some(PathBuf::from("workspace/readonly.rs")),
        "readonly".to_owned(),
    );
    read_only.set_read_only(true);
    let buffers = vec![read_only];

    assert_eq!(
        protected_preview_save_block_reason(1, &HashSet::new(), &HashSet::new(), &buffers),
        Some("buffer is read-only")
    );
}

#[test]
fn save_is_blocked_for_untitled_read_only_buffers_before_save_as() {
    let mut read_only = TextBuffer::new_untitled(1);
    read_only.mark_dirty();
    read_only.set_read_only(true);
    let buffers = vec![read_only];

    assert_eq!(
        protected_preview_save_block_reason(1, &HashSet::new(), &HashSet::new(), &buffers),
        Some("buffer is read-only")
    );
    assert_eq!(
        dirty_buffer_save_block_reason(
            &[1],
            &buffers,
            &HashSet::new(),
            &HashSet::new(),
            &HashSet::new(),
            "exiting",
        ),
        Some("Cannot save Untitled; buffer is read-only".to_owned())
    );
}

#[test]
fn workspace_guard_status_text_sanitizes_newline_buffer_labels() {
    let mut changed = TextBuffer::from_text(
        1,
        Some(PathBuf::from("workspace/src/evil\n\u{202e}name.rs")),
        "changed".to_owned(),
    );
    changed.mark_dirty();

    let reason = dirty_buffer_save_block_reason(
        &[1],
        &[changed],
        &HashSet::from([1]),
        &HashSet::new(),
        &HashSet::new(),
        "exiting",
    )
    .unwrap();
    let display = workspace_guard_status_message(&reason);

    assert!(display.contains("evil name.rs changed on disk"));
    assert!(!display.chars().any(|ch| ch.is_control()));
    assert!(!display.contains('\u{202e}'));
}

#[test]
fn workspace_guard_path_text_bounds_long_display_paths() {
    let long_name = format!("{}.rs", "a".repeat(240));
    let path = PathBuf::from("workspace/src").join(long_name);
    let display = workspace_guard_display_path(&path);

    assert!(display.chars().count() <= 120);
    assert!(display.contains("..."));
    assert!(!display.chars().any(|ch| ch.is_control()));
}

#[test]
fn workspace_guard_display_text_falls_back_for_blank_values() {
    assert_eq!(workspace_guard_display_path(PathBuf::new().as_path()), ".");
    assert_eq!(
        workspace_guard_status_message("\n\u{202e}\u{0007}"),
        "Save blocked"
    );
}
