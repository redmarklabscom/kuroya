use crate::history::{
    CLOSED_FILE_HISTORY_LIMIT, ClosedFileEntry, NAVIGATION_HISTORY_LIMIT, NavigationLocation,
    closed_file_entry_for_buffer, collect_navigation_locations, normalize_closed_file_history,
    normalize_navigation_history, push_closed_file_entry, push_navigation_location,
    take_navigation_history_target,
};
use kuroya_core::TextBuffer;
use std::{collections::VecDeque, path::PathBuf};

#[test]
fn closed_file_entry_captures_named_buffer_cursor() {
    let path = PathBuf::from("workspace/src/main.rs");
    let mut buffer = TextBuffer::from_text(
        7,
        Some(path.clone()),
        "fn main() {}\nlet value = 1;\n".to_owned(),
    );
    buffer.set_single_cursor(buffer.line_column_to_char(1, 4));

    assert_eq!(
        closed_file_entry_for_buffer(&buffer),
        Some(ClosedFileEntry::new(path, 2, 5))
    );
    assert_eq!(
        closed_file_entry_for_buffer(&TextBuffer::new_untitled(8)),
        None
    );
}

#[test]
fn closed_file_history_deduplicates_paths_and_stays_bounded() {
    let mut history = VecDeque::new();
    let path = PathBuf::from("workspace/src/main.rs");

    push_closed_file_entry(&mut history, ClosedFileEntry::new(path.clone(), 1, 1));
    push_closed_file_entry(&mut history, ClosedFileEntry::new(path.clone(), 9, 3));

    assert_eq!(
        history,
        VecDeque::from([ClosedFileEntry::new(path.clone(), 9, 3)])
    );

    for index in 0..(CLOSED_FILE_HISTORY_LIMIT + 2) {
        push_closed_file_entry(
            &mut history,
            ClosedFileEntry::new(PathBuf::from(format!("workspace/src/{index}.rs")), 1, 1),
        );
    }

    assert_eq!(history.len(), CLOSED_FILE_HISTORY_LIMIT);
    assert!(!history.iter().any(|entry| entry.path == path));
    assert_eq!(
        history.front().map(|entry| entry.path.clone()),
        Some(PathBuf::from("workspace/src/2.rs"))
    );
}

#[test]
fn closed_file_history_normalizes_persisted_entries() {
    let empty = PathBuf::new();
    let first = PathBuf::from("workspace/src/main.rs");
    let second = PathBuf::from("workspace/src/lib.rs");

    let history = normalize_closed_file_history(
        vec![
            ClosedFileEntry {
                path: empty,
                line: 0,
                column: 0,
            },
            ClosedFileEntry::new(first.clone(), 1, 1),
            ClosedFileEntry::new(second.clone(), 4, 2),
            ClosedFileEntry::new(first.clone(), 9, 3),
        ],
        2,
    );

    assert_eq!(
        history,
        VecDeque::from([
            ClosedFileEntry::new(second, 4, 2),
            ClosedFileEntry::new(first, 9, 3)
        ])
    );
}

#[test]
fn closed_file_history_normalizes_lexical_paths() {
    let first = PathBuf::from("workspace/src/main.rs");
    let second = PathBuf::from("workspace/src/lib.rs");

    let history = normalize_closed_file_history(
        vec![
            ClosedFileEntry::new(PathBuf::from("workspace/src/../src/main.rs"), 1, 1),
            ClosedFileEntry::new(second.clone(), 2, 1),
            ClosedFileEntry::new(first.clone(), 9, 4),
        ],
        8,
    );

    assert_eq!(
        history,
        VecDeque::from([
            ClosedFileEntry::new(second, 2, 1),
            ClosedFileEntry::new(first, 9, 4),
        ])
    );
}

#[test]
fn closed_file_history_preserves_stacked_leading_parent_paths() {
    let escaped = PathBuf::from("..").join("..").join("workspace/src/main.rs");
    let local = PathBuf::from("workspace/src/main.rs");
    let mut history = VecDeque::new();

    push_closed_file_entry(&mut history, ClosedFileEntry::new(escaped.clone(), 4, 1));
    push_closed_file_entry(&mut history, ClosedFileEntry::new(local.clone(), 4, 9));

    assert_eq!(
        history,
        VecDeque::from([
            ClosedFileEntry::new(escaped, 4, 1),
            ClosedFileEntry::new(local, 4, 9),
        ])
    );
}

#[test]
fn closed_file_history_skips_paths_that_normalize_empty() {
    let mut history = VecDeque::new();

    push_closed_file_entry(&mut history, ClosedFileEntry::new(PathBuf::from("."), 3, 2));
    push_closed_file_entry(
        &mut history,
        ClosedFileEntry::new(PathBuf::from("workspace/.."), 4, 1),
    );

    assert!(history.is_empty());

    let valid = PathBuf::from("workspace/src/lib.rs");
    let normalized = normalize_closed_file_history(
        vec![
            ClosedFileEntry {
                path: PathBuf::from("."),
                line: 0,
                column: 0,
            },
            ClosedFileEntry {
                path: PathBuf::from("workspace/.."),
                line: 4,
                column: 1,
            },
            ClosedFileEntry::new(valid.clone(), 5, 2),
        ],
        8,
    );

    assert_eq!(
        normalized,
        VecDeque::from([ClosedFileEntry::new(valid, 5, 2)])
    );
}

#[test]
fn navigation_history_deduplicates_and_stays_bounded() {
    let mut history = VecDeque::new();
    let first = NavigationLocation::new(PathBuf::from("workspace/src/main.rs"), 1, 1);

    push_navigation_location(&mut history, first.clone());
    push_navigation_location(&mut history, first);
    assert_eq!(history.len(), 1);

    history.clear();
    for index in 0..(NAVIGATION_HISTORY_LIMIT + 2) {
        push_navigation_location(
            &mut history,
            NavigationLocation::new(
                PathBuf::from(format!("workspace/src/{index}.rs")),
                index + 1,
                1,
            ),
        );
    }

    assert_eq!(history.len(), NAVIGATION_HISTORY_LIMIT);
    assert_eq!(history.front().map(|entry| entry.line), Some(3));
}

#[test]
fn navigation_history_coalesces_same_line_column_noise() {
    let mut history = VecDeque::new();
    let path = PathBuf::from("workspace/src/main.rs");

    push_navigation_location(&mut history, NavigationLocation::new(path.clone(), 8, 1));
    push_navigation_location(&mut history, NavigationLocation::new(path.clone(), 8, 24));

    assert_eq!(
        history,
        VecDeque::from([NavigationLocation::new(path, 8, 24)])
    );
}

#[test]
fn navigation_history_moves_duplicate_location_to_most_recent_entry() {
    let mut history = VecDeque::new();
    let first = PathBuf::from("workspace/src/main.rs");
    let second = PathBuf::from("workspace/src/lib.rs");

    push_navigation_location(&mut history, NavigationLocation::new(first.clone(), 8, 1));
    push_navigation_location(&mut history, NavigationLocation::new(second.clone(), 4, 1));
    push_navigation_location(&mut history, NavigationLocation::new(first.clone(), 8, 24));

    assert_eq!(
        history,
        VecDeque::from([
            NavigationLocation::new(second, 4, 1),
            NavigationLocation::new(first, 8, 24),
        ])
    );
}

#[test]
fn navigation_history_normalizes_persisted_entries() {
    let empty = PathBuf::new();
    let first = NavigationLocation::new(PathBuf::from("workspace/src/main.rs"), 0, 0);
    let second = NavigationLocation::new(PathBuf::from("workspace/src/lib.rs"), 5, 2);
    let third = NavigationLocation::new(PathBuf::from("workspace/src/other.rs"), 8, 3);

    let history = normalize_navigation_history(
        vec![
            NavigationLocation {
                path: empty,
                line: 0,
                column: 0,
            },
            first.clone(),
            first,
            second.clone(),
            third.clone(),
        ],
        2,
    );

    assert_eq!(history, VecDeque::from([second, third]));
}

#[test]
fn navigation_history_normalization_coalesces_same_line_entries() {
    let path = PathBuf::from("workspace/src/main.rs");

    let history = normalize_navigation_history(
        vec![
            NavigationLocation::new(path.clone(), 3, 1),
            NavigationLocation::new(path.clone(), 3, 9),
        ],
        8,
    );

    assert_eq!(
        history,
        VecDeque::from([NavigationLocation::new(path, 3, 9)])
    );
}

#[test]
fn navigation_history_normalizes_lexical_paths() {
    let mut history = VecDeque::new();
    let main = PathBuf::from("workspace/src/main.rs");
    let lib = PathBuf::from("workspace/src/lib.rs");

    push_navigation_location(
        &mut history,
        NavigationLocation::new(PathBuf::from("workspace/src/../src/main.rs"), 4, 1),
    );
    push_navigation_location(&mut history, NavigationLocation::new(lib.clone(), 6, 2));
    push_navigation_location(&mut history, NavigationLocation::new(main.clone(), 4, 9));

    assert_eq!(
        history,
        VecDeque::from([
            NavigationLocation::new(lib, 6, 2),
            NavigationLocation::new(main, 4, 9),
        ])
    );
}

#[test]
fn navigation_history_preserves_stacked_leading_parent_paths() {
    let escaped = PathBuf::from("..").join("..").join("workspace/src/main.rs");
    let local = PathBuf::from("workspace/src/main.rs");
    let mut history = VecDeque::new();

    push_navigation_location(&mut history, NavigationLocation::new(escaped.clone(), 4, 1));
    push_navigation_location(&mut history, NavigationLocation::new(local.clone(), 4, 9));

    assert_eq!(
        history,
        VecDeque::from([
            NavigationLocation::new(escaped, 4, 1),
            NavigationLocation::new(local, 4, 9),
        ])
    );
}

#[test]
fn navigation_history_skips_paths_that_normalize_empty() {
    let mut history = VecDeque::new();

    push_navigation_location(
        &mut history,
        NavigationLocation::new(PathBuf::from("."), 3, 2),
    );
    push_navigation_location(
        &mut history,
        NavigationLocation::new(PathBuf::from("workspace/.."), 4, 1),
    );

    assert!(history.is_empty());

    let valid = PathBuf::from("workspace/src/lib.rs");
    let normalized = normalize_navigation_history(
        vec![
            NavigationLocation {
                path: PathBuf::from("."),
                line: 0,
                column: 0,
            },
            NavigationLocation {
                path: PathBuf::from("workspace/.."),
                line: 4,
                column: 1,
            },
            NavigationLocation::new(valid.clone(), 5, 2),
        ],
        8,
    );

    assert_eq!(
        normalized,
        VecDeque::from([NavigationLocation::new(valid, 5, 2)])
    );
}

#[test]
fn navigation_history_normalization_removes_non_adjacent_duplicate_locations() {
    let first = PathBuf::from("workspace/src/main.rs");
    let second = PathBuf::from("workspace/src/lib.rs");
    let third = PathBuf::from("workspace/src/other.rs");

    let history = normalize_navigation_history(
        vec![
            NavigationLocation::new(first.clone(), 3, 1),
            NavigationLocation::new(second.clone(), 4, 1),
            NavigationLocation::new(third.clone(), 5, 1),
            NavigationLocation::new(first.clone(), 3, 9),
        ],
        8,
    );

    assert_eq!(
        history,
        VecDeque::from([
            NavigationLocation::new(second, 4, 1),
            NavigationLocation::new(third, 5, 1),
            NavigationLocation::new(first, 3, 9),
        ])
    );
}

#[test]
fn navigation_locations_collect_back_forward_and_current() {
    let back_a = NavigationLocation::new(PathBuf::from("workspace/src/a.rs"), 1, 1);
    let back_b = NavigationLocation::new(PathBuf::from("workspace/src/b.rs"), 2, 1);
    let forward = NavigationLocation::new(PathBuf::from("workspace/src/c.rs"), 3, 1);
    let current = NavigationLocation::new(PathBuf::from("workspace/src/d.rs"), 4, 1);
    let back = VecDeque::from([back_a.clone(), back_b.clone()]);
    let forward_stack = VecDeque::from([forward.clone()]);

    assert_eq!(
        collect_navigation_locations(&back, &forward_stack, Some(current.clone())),
        vec![back_a.clone(), back_b.clone(), forward.clone(), current]
    );
    assert_eq!(
        collect_navigation_locations(&back, &forward_stack, None),
        vec![back_a, back_b, forward]
    );
}

#[test]
fn navigation_history_moves_current_location_between_stacks() {
    let a = NavigationLocation::new(PathBuf::from("workspace/src/a.rs"), 1, 1);
    let b = NavigationLocation::new(PathBuf::from("workspace/src/b.rs"), 2, 1);
    let c = NavigationLocation::new(PathBuf::from("workspace/src/c.rs"), 3, 1);
    let mut back = VecDeque::from([a.clone(), b.clone()]);
    let mut forward = VecDeque::new();

    assert_eq!(
        take_navigation_history_target(&mut back, &mut forward, Some(c.clone()), -1),
        Some(b.clone())
    );
    assert_eq!(back, VecDeque::from([a.clone()]));
    assert_eq!(forward, VecDeque::from([c.clone()]));

    assert_eq!(
        take_navigation_history_target(&mut back, &mut forward, Some(b.clone()), -1),
        Some(a.clone())
    );
    assert_eq!(back, VecDeque::new());
    assert_eq!(forward, VecDeque::from([c, b.clone()]));

    assert_eq!(
        take_navigation_history_target(&mut back, &mut forward, Some(a.clone()), 1),
        Some(b)
    );
    assert_eq!(back, VecDeque::from([a]));
}
