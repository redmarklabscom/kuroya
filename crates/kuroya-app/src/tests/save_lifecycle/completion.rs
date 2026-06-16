use crate::save_lifecycle::{apply_save_completion, save_completion_status};
use kuroya_core::TextBuffer;
use std::path::{Path, PathBuf};

#[test]
fn save_completion_clears_dirty_only_for_matching_buffer_version() {
    let path = PathBuf::from("workspace/src/main.rs");
    let mut current = TextBuffer::from_text(1, None, "current".to_owned());
    current.mark_dirty();
    let current_version = current.version();

    assert!(!apply_save_completion(
        &mut current,
        path.clone(),
        current_version
    ));
    assert_eq!(current.path(), Some(&path));
    assert!(!current.is_dirty());

    let mut stale = TextBuffer::from_text(2, None, "stale".to_owned());
    stale.mark_dirty();
    let saved_version = stale.version();
    stale.insert_at_cursor("!");

    assert!(apply_save_completion(
        &mut stale,
        path.clone(),
        saved_version
    ));
    assert_eq!(stale.path(), Some(&path));
    assert!(stale.is_dirty());
}

#[test]
fn save_completion_status_reports_unsaved_newer_edits() {
    let path = Path::new("workspace/src/main.rs");

    assert_eq!(save_completion_status(path, false), "Saved main.rs");
    assert_eq!(
        save_completion_status(path, true),
        "Saved main.rs; newer edits remain unsaved"
    );
}
