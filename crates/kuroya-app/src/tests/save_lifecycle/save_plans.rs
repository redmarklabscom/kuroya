use crate::save_lifecycle::{
    SaveAllBlocker, SaveAllPlan, autosave_buffer_ids, dirty_buffer_ids,
    plan_save_all_dirty_buffers, save_needs_external_change_confirmation,
};
use kuroya_core::TextBuffer;
use std::{collections::HashSet, path::PathBuf};

#[test]
fn save_confirmation_only_blocks_dirty_files_changed_on_disk() {
    let clean_path = PathBuf::from("workspace/src/main.rs");
    let dirty_path = PathBuf::from("workspace/src/lib.rs");
    let mut dirty = TextBuffer::from_text(2, Some(dirty_path), "dirty".to_owned());
    dirty.insert_at_cursor("!");
    let buffers = vec![
        TextBuffer::from_text(1, Some(clean_path), "clean".to_owned()),
        dirty,
        TextBuffer::new_untitled(3),
    ];
    let changed_on_disk = HashSet::from([1, 2, 3, 4]);

    assert!(!save_needs_external_change_confirmation(
        1,
        &changed_on_disk,
        &buffers
    ));
    assert!(save_needs_external_change_confirmation(
        2,
        &changed_on_disk,
        &buffers
    ));
    assert!(!save_needs_external_change_confirmation(
        3,
        &changed_on_disk,
        &buffers
    ));
    assert!(!save_needs_external_change_confirmation(
        4,
        &changed_on_disk,
        &buffers
    ));
}

#[test]
fn workspace_switch_detects_dirty_buffers_before_teardown() {
    let clean = TextBuffer::from_text(
        1,
        Some(PathBuf::from("workspace/src/main.rs")),
        "fn main() {}".to_owned(),
    );
    let mut dirty = TextBuffer::from_text(
        2,
        Some(PathBuf::from("workspace/src/lib.rs")),
        "pub fn lib() {}".to_owned(),
    );
    dirty.mark_dirty();

    assert_eq!(dirty_buffer_ids(&[clean, dirty]), vec![2]);
}

#[test]
fn save_all_plan_saves_only_dirty_named_buffers() {
    let clean = TextBuffer::from_text(
        1,
        Some(PathBuf::from("workspace/src/main.rs")),
        "fn main() {}".to_owned(),
    );
    let mut dirty = TextBuffer::from_text(
        2,
        Some(PathBuf::from("workspace/src/lib.rs")),
        "pub fn lib() {}".to_owned(),
    );
    dirty.mark_dirty();

    assert_eq!(
        plan_save_all_dirty_buffers(
            &[clean, dirty],
            &HashSet::new(),
            &HashSet::new(),
            &HashSet::new()
        ),
        SaveAllPlan {
            savable: vec![2],
            first_blocker: None,
        }
    );
}

#[test]
fn autosave_skips_blocked_conflicting_in_flight_and_untitled_buffers() {
    let mut safe = TextBuffer::from_text(
        1,
        Some(PathBuf::from("workspace/src/main.rs")),
        "safe".to_owned(),
    );
    safe.mark_dirty();
    let mut changed = TextBuffer::from_text(
        2,
        Some(PathBuf::from("workspace/src/changed.rs")),
        "changed".to_owned(),
    );
    changed.mark_dirty();
    let mut blocked = TextBuffer::from_text(
        3,
        Some(PathBuf::from("workspace/src/blocked.rs")),
        "blocked".to_owned(),
    );
    blocked.mark_dirty();
    let mut in_flight = TextBuffer::from_text(
        4,
        Some(PathBuf::from("workspace/src/in_flight.rs")),
        "saving".to_owned(),
    );
    in_flight.mark_dirty();
    let mut untitled = TextBuffer::new_untitled(5);
    untitled.mark_dirty();
    let clean = TextBuffer::from_text(
        6,
        Some(PathBuf::from("workspace/src/clean.rs")),
        "clean".to_owned(),
    );

    assert_eq!(
        autosave_buffer_ids(
            &[safe, changed, blocked, in_flight, untitled, clean],
            &HashSet::from([2]),
            &HashSet::from([3, 4]),
            &HashSet::new(),
            &HashSet::new(),
        ),
        vec![1],
    );
}

#[test]
fn autosave_skips_protected_preview_and_read_only_buffers() {
    let mut safe = TextBuffer::from_text(
        1,
        Some(PathBuf::from("workspace/src/main.rs")),
        "safe".to_owned(),
    );
    safe.mark_dirty();
    let mut lossy = TextBuffer::from_text(
        2,
        Some(PathBuf::from("workspace/src/lossy.dat")),
        "lossy".to_owned(),
    );
    lossy.mark_dirty();
    let mut binary = TextBuffer::from_text(
        3,
        Some(PathBuf::from("workspace/src/binary.dat")),
        "binary".to_owned(),
    );
    binary.mark_dirty();
    let mut read_only = TextBuffer::from_text(
        4,
        Some(PathBuf::from("workspace/src/read_only.rs")),
        "readonly".to_owned(),
    );
    read_only.mark_dirty();
    read_only.set_read_only(true);

    assert_eq!(
        autosave_buffer_ids(
            &[safe, lossy, binary, read_only],
            &HashSet::new(),
            &HashSet::new(),
            &HashSet::from([2]),
            &HashSet::from([3]),
        ),
        vec![1],
    );
}

#[test]
fn save_all_plan_keeps_savable_files_and_surfaces_first_blocker() {
    let mut safe = TextBuffer::from_text(
        1,
        Some(PathBuf::from("workspace/src/main.rs")),
        "safe".to_owned(),
    );
    safe.mark_dirty();
    let mut untitled = TextBuffer::new_untitled(2);
    untitled.mark_dirty();
    let mut changed = TextBuffer::from_text(
        3,
        Some(PathBuf::from("workspace/src/changed.rs")),
        "changed".to_owned(),
    );
    changed.mark_dirty();

    assert_eq!(
        plan_save_all_dirty_buffers(
            &[safe, untitled, changed],
            &HashSet::from([3]),
            &HashSet::new(),
            &HashSet::new(),
        ),
        SaveAllPlan {
            savable: vec![1],
            first_blocker: Some(SaveAllBlocker::Untitled(2)),
        }
    );
}

#[test]
fn save_all_plan_blocks_conflicts_and_protected_previews() {
    let mut changed = TextBuffer::from_text(
        1,
        Some(PathBuf::from("workspace/src/changed.rs")),
        "changed".to_owned(),
    );
    changed.mark_dirty();
    let mut lossy = TextBuffer::from_text(
        2,
        Some(PathBuf::from("workspace/src/lossy.dat")),
        "lossy".to_owned(),
    );
    lossy.mark_dirty();

    assert_eq!(
        plan_save_all_dirty_buffers(
            std::slice::from_ref(&changed),
            &HashSet::from([1]),
            &HashSet::new(),
            &HashSet::new(),
        ),
        SaveAllPlan {
            savable: Vec::new(),
            first_blocker: Some(SaveAllBlocker::ExternalChange(1)),
        }
    );
    assert_eq!(
        plan_save_all_dirty_buffers(
            &[lossy],
            &HashSet::new(),
            &HashSet::from([2]),
            &HashSet::new(),
        ),
        SaveAllPlan {
            savable: Vec::new(),
            first_blocker: Some(SaveAllBlocker::ProtectedPreview(
                2,
                "file was decoded with replacement characters"
            )),
        }
    );
}

#[test]
fn save_all_plan_blocks_read_only_untitled_buffers_before_save_as() {
    let mut read_only = TextBuffer::new_untitled(7);
    read_only.mark_dirty();
    read_only.set_read_only(true);

    assert_eq!(
        plan_save_all_dirty_buffers(
            &[read_only],
            &HashSet::new(),
            &HashSet::new(),
            &HashSet::new(),
        ),
        SaveAllPlan {
            savable: Vec::new(),
            first_blocker: Some(SaveAllBlocker::ProtectedPreview(7, "buffer is read-only")),
        }
    );
}
