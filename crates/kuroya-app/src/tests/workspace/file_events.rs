use crate::workspace_state::{
    OpenFileRequest, classify_open_file_request, dirty_open_buffers_for_changes,
    path_set_contains_exact_or_lexically, reloadable_open_buffers_for_changes,
    remove_path_map_entry_exact_or_lexically, remove_path_set_entry_exact_or_lexically,
    should_activate_loaded_file, take_pending_panes_for_path, workspace_event_matches,
};
use kuroya_core::TextBuffer;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

#[test]
fn reloadable_open_buffers_only_include_clean_changed_files() {
    let changed_path = PathBuf::from("workspace/src/main.rs");
    let dirty_path = PathBuf::from("workspace/src/lib.rs");
    let mut dirty = TextBuffer::from_text(2, Some(dirty_path.clone()), "dirty".to_owned());
    dirty.insert_at_cursor("!");
    let buffers = vec![
        TextBuffer::from_text(1, Some(changed_path.clone()), "clean".to_owned()),
        dirty,
        TextBuffer::new_untitled(3),
    ];

    assert_eq!(
        reloadable_open_buffers_for_changes(&[changed_path.clone(), dirty_path], &buffers),
        vec![(1, changed_path)]
    );
}

#[test]
fn reloadable_open_buffers_include_descendants_of_changed_directories() {
    let changed_dir = PathBuf::from("workspace/src");
    let descendant = PathBuf::from("workspace/src/main.rs");
    let sibling = PathBuf::from("workspace/tests/main.rs");
    let buffers = vec![
        TextBuffer::from_text(1, Some(descendant.clone()), "clean".to_owned()),
        TextBuffer::from_text(2, Some(sibling), "other".to_owned()),
    ];

    assert_eq!(
        reloadable_open_buffers_for_changes(&[changed_dir], &buffers),
        vec![(1, descendant)]
    );
}

#[test]
fn reloadable_open_buffers_include_descendants_of_changed_workspace_root() {
    let workspace = PathBuf::from("workspace");
    let main = PathBuf::from("workspace/src/main.rs");
    let lib = PathBuf::from("workspace/src/lib.rs");
    let buffers = vec![
        TextBuffer::from_text(1, Some(main.clone()), "clean".to_owned()),
        TextBuffer::from_text(2, Some(lib.clone()), "clean".to_owned()),
    ];

    assert_eq!(
        reloadable_open_buffers_for_changes(&[workspace], &buffers),
        vec![(1, main), (2, lib)]
    );
}

#[test]
fn reloadable_open_buffers_include_descendants_of_equivalent_changed_roots() {
    let workspace = PathBuf::from("workspace");
    let changed = workspace.join("src").join("..");
    let main = PathBuf::from("workspace/src/main.rs");
    let readme = PathBuf::from("workspace/README.md");
    let outside = PathBuf::from("other/src/main.rs");
    let buffers = vec![
        TextBuffer::from_text(1, Some(main.clone()), "clean".to_owned()),
        TextBuffer::from_text(2, Some(readme.clone()), "clean".to_owned()),
        TextBuffer::from_text(3, Some(outside), "other".to_owned()),
    ];

    assert_eq!(
        reloadable_open_buffers_for_changes(&[changed], &buffers),
        vec![(1, main), (2, readme)]
    );
}

#[test]
fn dirty_open_buffers_report_external_disk_changes() {
    let clean_path = PathBuf::from("workspace/src/main.rs");
    let dirty_path = PathBuf::from("workspace/src/lib.rs");
    let mut dirty = TextBuffer::from_text(2, Some(dirty_path.clone()), "dirty".to_owned());
    dirty.insert_at_cursor("!");
    let buffers = vec![
        TextBuffer::from_text(1, Some(clean_path.clone()), "clean".to_owned()),
        dirty,
        TextBuffer::new_untitled(3),
    ];

    assert_eq!(
        dirty_open_buffers_for_changes(&[clean_path, dirty_path.clone()], &buffers),
        vec![(2, dirty_path)]
    );
}

#[test]
fn dirty_open_buffers_include_descendants_of_changed_directories() {
    let changed_dir = PathBuf::from("workspace/src");
    let dirty_path = PathBuf::from("workspace/src/lib.rs");
    let clean_path = PathBuf::from("workspace/src/main.rs");
    let mut dirty = TextBuffer::from_text(2, Some(dirty_path.clone()), "dirty".to_owned());
    dirty.insert_at_cursor("!");
    let buffers = vec![
        TextBuffer::from_text(1, Some(clean_path), "clean".to_owned()),
        dirty,
    ];

    assert_eq!(
        dirty_open_buffers_for_changes(&[changed_dir], &buffers),
        vec![(2, dirty_path)]
    );
}

#[test]
fn open_file_requests_coalesce_open_and_pending_paths() {
    let open_path = PathBuf::from("workspace/src/main.rs");
    let pending_path = PathBuf::from("workspace/src/lib.rs");
    let buffers = vec![TextBuffer::from_text(
        7,
        Some(open_path.clone()),
        "open".to_owned(),
    )];
    let pending = HashSet::from([pending_path.clone()]);

    assert_eq!(
        classify_open_file_request(&open_path, &buffers, &pending),
        OpenFileRequest::AlreadyOpen(7)
    );
    assert_eq!(
        classify_open_file_request(&pending_path, &buffers, &pending),
        OpenFileRequest::AlreadyPending
    );
    assert_eq!(
        classify_open_file_request(Path::new("workspace/src/new.rs"), &buffers, &pending),
        OpenFileRequest::Spawn
    );
}

#[test]
fn open_file_requests_coalesce_lexically_equivalent_paths() {
    let open_path = PathBuf::from("workspace/src/main.rs");
    let open_variant = PathBuf::from("workspace/src")
        .join("..")
        .join("src/main.rs");
    let pending_path = PathBuf::from("workspace/src/lib.rs");
    let pending_variant = PathBuf::from("workspace/src").join("..").join("src/lib.rs");
    let buffers = vec![TextBuffer::from_text(
        7,
        Some(open_path.clone()),
        "open".to_owned(),
    )];
    let pending = HashSet::from([pending_path]);

    assert_eq!(
        classify_open_file_request(&open_variant, &buffers, &pending),
        OpenFileRequest::AlreadyOpen(7)
    );
    assert_eq!(
        classify_open_file_request(&pending_variant, &buffers, &pending),
        OpenFileRequest::AlreadyPending
    );
}

#[test]
fn open_file_requests_prefer_exact_open_buffer_over_earlier_lexical_match() {
    let exact_path = PathBuf::from("workspace/src/main.rs");
    let lexical_path = PathBuf::from("workspace/src")
        .join("..")
        .join("src/main.rs");
    let buffers = vec![
        TextBuffer::from_text(7, Some(lexical_path), "lexical".to_owned()),
        TextBuffer::from_text(9, Some(exact_path.clone()), "exact".to_owned()),
    ];

    assert_eq!(
        classify_open_file_request(&exact_path, &buffers, &HashSet::new()),
        OpenFileRequest::AlreadyOpen(9)
    );
}

#[test]
fn pending_session_panes_resolve_by_loaded_path_once() {
    let main = PathBuf::from("workspace/src/main.rs");
    let lib = PathBuf::from("workspace/src/lib.rs");
    let mut pending = HashMap::from([(2, main.clone()), (3, lib.clone()), (4, main.clone())]);

    assert_eq!(take_pending_panes_for_path(&mut pending, &main), vec![2, 4]);
    assert!(!pending.contains_key(&2));
    assert!(!pending.contains_key(&4));
    assert_eq!(pending.get(&3), Some(&lib));
    assert!(take_pending_panes_for_path(&mut pending, &main).is_empty());
}

#[test]
fn pending_session_panes_resolve_lexically_equivalent_loaded_path_once() {
    let main = PathBuf::from("workspace/src/main.rs");
    let equivalent_main = PathBuf::from("workspace/src")
        .join("..")
        .join("src/main.rs");
    let lib = PathBuf::from("workspace/src/lib.rs");
    let mut pending = HashMap::from([(2, main.clone()), (3, lib.clone()), (4, main)]);

    assert_eq!(
        take_pending_panes_for_path(&mut pending, &equivalent_main),
        vec![2, 4]
    );
    assert!(!pending.contains_key(&2));
    assert!(!pending.contains_key(&4));
    assert_eq!(pending.get(&3), Some(&lib));
    assert!(take_pending_panes_for_path(&mut pending, &equivalent_main).is_empty());
}

#[test]
fn pending_path_sets_resolve_exact_and_lexical_entries_once() {
    let main = PathBuf::from("workspace/src/main.rs");
    let lib = PathBuf::from("workspace/src/lib.rs");
    let equivalent_lib = PathBuf::from("workspace/src").join("..").join("src/lib.rs");
    let mut pending = HashSet::from([main.clone(), lib.clone()]);

    assert!(path_set_contains_exact_or_lexically(&pending, &main));
    assert!(remove_path_set_entry_exact_or_lexically(
        &mut pending,
        &main
    ));
    assert!(!pending.contains(&main));
    assert!(pending.contains(&lib));

    assert!(path_set_contains_exact_or_lexically(
        &pending,
        &equivalent_lib
    ));
    assert!(remove_path_set_entry_exact_or_lexically(
        &mut pending,
        &equivalent_lib
    ));
    assert!(!pending.contains(&lib));
    assert!(!remove_path_set_entry_exact_or_lexically(
        &mut pending,
        Path::new("workspace/src/missing.rs")
    ));
}

#[test]
fn pending_path_maps_remove_exact_and_lexical_entries_once() {
    let main = PathBuf::from("workspace/src/main.rs");
    let lib = PathBuf::from("workspace/src/lib.rs");
    let equivalent_lib = PathBuf::from("workspace/src").join("..").join("src/lib.rs");
    let mut pending = HashMap::from([(main.clone(), 1), (lib.clone(), 2)]);

    assert_eq!(
        remove_path_map_entry_exact_or_lexically(&mut pending, &main),
        Some(1)
    );
    assert!(!pending.contains_key(&main));
    assert_eq!(pending.get(&lib), Some(&2));

    assert_eq!(
        remove_path_map_entry_exact_or_lexically(&mut pending, &equivalent_lib),
        Some(2)
    );
    assert!(!pending.contains_key(&lib));
    assert_eq!(
        remove_path_map_entry_exact_or_lexically(
            &mut pending,
            Path::new("workspace/src/missing.rs")
        ),
        None
    );
}

#[test]
fn restored_active_path_prevents_incidental_async_activation() {
    assert!(should_activate_loaded_file(false, false, false, false));
    assert!(!should_activate_loaded_file(false, false, false, true));
    assert!(should_activate_loaded_file(true, false, false, true));
    assert!(should_activate_loaded_file(false, true, false, true));
    assert!(!should_activate_loaded_file(false, false, true, false));
}

#[test]
fn workspace_scoped_events_reject_stale_workspace_roots() {
    let current = Path::new("workspace/current");
    let stale = Path::new("workspace/stale");

    assert!(workspace_event_matches(current, current));
    assert!(!workspace_event_matches(current, stale));
}
