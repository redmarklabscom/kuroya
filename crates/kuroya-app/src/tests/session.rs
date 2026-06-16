use crate::{
    app_session::{
        persisted_explorer_expanded_paths, persisted_source_control_sort_mode,
        persisted_source_control_view_mode, restored_explorer_expanded_paths,
        source_control_sort_mode_from_persisted, source_control_view_mode_from_persisted,
        workspace_descendant_path_for_session,
    },
    app_session_restore::terminal_visible_after_startup,
    persistence::{
        BufferSelectionState, BufferViewState, PaneBufferViewState, PersistedSourceControlSortMode,
        PersistedSourceControlViewMode,
    },
    session_state::{
        EditorPane, apply_buffer_history_state, apply_buffer_view_state, merged_recent_projects,
        recent_projects_with_recorded, session_history_states, session_pane_view_states,
        session_view_states,
    },
    source_control_panel::{SourceControlSortMode, SourceControlViewMode},
};

use kuroya_core::{Selection, TerminalHideOnStartup, TextBuffer};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

#[test]
fn session_view_states_capture_cursor_and_scroll_lines() {
    let path = PathBuf::from("workspace/src/main.rs");
    let mut buffer =
        TextBuffer::from_text(7, Some(path.clone()), "one\ntwo\nthree\nfour".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(2, 3));
    let panes = vec![EditorPane {
        id: 2,
        active: Some(7),
        weight: 1.0,
    }];
    let scroll_offsets = HashMap::from([((2, 7), 40.0)]);

    assert_eq!(
        session_view_states(&[buffer], &panes, &scroll_offsets, &HashMap::new(), 2, 20.0),
        vec![BufferViewState {
            path,
            cursor_line: 3,
            cursor_column: 4,
            scroll_line: 3,
            horizontal_scroll_offset: 0.0,
            selections: vec![BufferSelectionState {
                anchor_line: 3,
                anchor_column: 4,
                cursor_line: 3,
                cursor_column: 4,
            }],
        }]
    );
}

#[test]
fn session_view_states_capture_inactive_buffer_scroll_offsets() {
    let first_path = PathBuf::from("workspace/src/first.rs");
    let second_path = PathBuf::from("workspace/src/second.rs");
    let mut first = TextBuffer::from_text(
        7,
        Some(first_path.clone()),
        "one\ntwo\nthree\nfour\nfive".to_owned(),
    );
    first.set_single_cursor(first.line_column_to_char(0, 1));
    let mut second = TextBuffer::from_text(8, Some(second_path.clone()), "active".to_owned());
    second.set_single_cursor(second.len_chars());
    let panes = vec![EditorPane {
        id: 2,
        active: Some(8),
        weight: 1.0,
    }];
    let scroll_offsets = HashMap::from([((2, 7), 60.0), ((2, 8), 0.0)]);

    assert_eq!(
        session_view_states(
            &[first, second],
            &panes,
            &scroll_offsets,
            &HashMap::new(),
            2,
            20.0,
        ),
        vec![
            BufferViewState {
                path: first_path,
                cursor_line: 1,
                cursor_column: 2,
                scroll_line: 4,
                horizontal_scroll_offset: 0.0,
                selections: vec![BufferSelectionState {
                    anchor_line: 1,
                    anchor_column: 2,
                    cursor_line: 1,
                    cursor_column: 2,
                }],
            },
            BufferViewState {
                path: second_path,
                cursor_line: 1,
                cursor_column: 7,
                scroll_line: 1,
                horizontal_scroll_offset: 0.0,
                selections: vec![BufferSelectionState {
                    anchor_line: 1,
                    anchor_column: 7,
                    cursor_line: 1,
                    cursor_column: 7,
                }],
            },
        ]
    );
}

#[test]
fn session_view_states_prefer_active_pane_scroll_offsets() {
    let path = PathBuf::from("workspace/src/main.rs");
    let buffer = TextBuffer::from_text(7, Some(path.clone()), "one\ntwo\nthree".to_owned());
    let panes = vec![
        EditorPane {
            id: 2,
            active: Some(7),
            weight: 0.5,
        },
        EditorPane {
            id: 3,
            active: Some(7),
            weight: 0.5,
        },
    ];
    let scroll_offsets = HashMap::from([((2, 7), 20.0), ((3, 7), 40.0)]);

    assert_eq!(
        session_view_states(&[buffer], &panes, &scroll_offsets, &HashMap::new(), 3, 20.0),
        vec![BufferViewState {
            path,
            cursor_line: 1,
            cursor_column: 1,
            scroll_line: 3,
            horizontal_scroll_offset: 0.0,
            selections: vec![BufferSelectionState {
                anchor_line: 1,
                anchor_column: 1,
                cursor_line: 1,
                cursor_column: 1,
            }],
        }]
    );
}

#[test]
fn session_view_states_capture_horizontal_scroll_offsets() {
    let path = PathBuf::from("workspace/src/main.rs");
    let buffer = TextBuffer::from_text(7, Some(path.clone()), "let value = 1;".to_owned());
    let panes = vec![EditorPane {
        id: 2,
        active: Some(7),
        weight: 1.0,
    }];
    let vertical_offsets = HashMap::from([((2, 7), 0.0)]);
    let horizontal_offsets = HashMap::from([((2, 7), 144.0)]);

    assert_eq!(
        session_view_states(
            &[buffer],
            &panes,
            &vertical_offsets,
            &horizontal_offsets,
            2,
            20.0,
        ),
        vec![BufferViewState {
            path,
            cursor_line: 1,
            cursor_column: 1,
            scroll_line: 1,
            horizontal_scroll_offset: 144.0,
            selections: vec![BufferSelectionState {
                anchor_line: 1,
                anchor_column: 1,
                cursor_line: 1,
                cursor_column: 1,
            }],
        }]
    );
}

#[test]
fn session_pane_view_states_capture_same_path_per_pane_viewports() {
    let path = PathBuf::from("workspace/src/main.rs");
    let buffer = TextBuffer::from_text(
        7,
        Some(path.clone()),
        "one\ntwo\nthree\nfour\nfive".to_owned(),
    );
    let panes = vec![
        EditorPane {
            id: 2,
            active: Some(7),
            weight: 0.5,
        },
        EditorPane {
            id: 3,
            active: Some(7),
            weight: 0.5,
        },
    ];
    let vertical_offsets = HashMap::from([((2, 7), 20.0), ((3, 7), 80.0)]);
    let horizontal_offsets = HashMap::from([((2, 7), 24.0), ((3, 7), 144.0)]);

    assert_eq!(
        session_pane_view_states(
            &[buffer],
            &panes,
            &vertical_offsets,
            &horizontal_offsets,
            20.0,
        ),
        vec![
            PaneBufferViewState {
                pane_index: 0,
                path: path.clone(),
                scroll_line: 2,
                horizontal_scroll_offset: 24.0,
            },
            PaneBufferViewState {
                pane_index: 1,
                path,
                scroll_line: 5,
                horizontal_scroll_offset: 144.0,
            },
        ]
    );
}

#[test]
fn session_pane_view_states_sanitize_invalid_horizontal_offsets() {
    let first_path = PathBuf::from("workspace/src/first.rs");
    let second_path = PathBuf::from("workspace/src/second.rs");
    let third_path = PathBuf::from("workspace/src/third.rs");
    let first = TextBuffer::from_text(7, Some(first_path.clone()), "first".to_owned());
    let second = TextBuffer::from_text(8, Some(second_path.clone()), "second".to_owned());
    let third = TextBuffer::from_text(9, Some(third_path.clone()), "third".to_owned());
    let panes = vec![
        EditorPane {
            id: 2,
            active: Some(7),
            weight: 1.0,
        },
        EditorPane {
            id: 3,
            active: Some(8),
            weight: 1.0,
        },
        EditorPane {
            id: 4,
            active: Some(9),
            weight: 1.0,
        },
    ];
    let vertical_offsets = HashMap::from([((2, 7), 0.0), ((3, 8), 20.0), ((4, 9), 40.0)]);
    let horizontal_offsets =
        HashMap::from([((2, 7), -12.0), ((3, 8), f32::NAN), ((4, 9), f32::INFINITY)]);

    assert_eq!(
        session_pane_view_states(
            &[first, second, third],
            &panes,
            &vertical_offsets,
            &horizontal_offsets,
            20.0,
        ),
        vec![
            PaneBufferViewState {
                pane_index: 0,
                path: first_path,
                scroll_line: 1,
                horizontal_scroll_offset: 0.0,
            },
            PaneBufferViewState {
                pane_index: 1,
                path: second_path,
                scroll_line: 2,
                horizontal_scroll_offset: 0.0,
            },
            PaneBufferViewState {
                pane_index: 2,
                path: third_path,
                scroll_line: 3,
                horizontal_scroll_offset: 0.0,
            },
        ]
    );
}

#[test]
fn session_view_states_capture_multiple_selections() {
    let path = PathBuf::from("workspace/src/main.rs");
    let mut buffer = TextBuffer::from_text(
        7,
        Some(path.clone()),
        "alpha\nbeta\ngamma\ndelta".to_owned(),
    );
    let first = Selection {
        anchor: buffer.line_column_to_char(0, 1),
        cursor: buffer.line_column_to_char(0, 4),
    };
    let second = Selection {
        anchor: buffer.line_column_to_char(2, 5),
        cursor: buffer.line_column_to_char(2, 1),
    };
    buffer.set_selections([first, second]);
    let panes = vec![EditorPane {
        id: 2,
        active: Some(7),
        weight: 1.0,
    }];

    assert_eq!(
        session_view_states(&[buffer], &panes, &HashMap::new(), &HashMap::new(), 2, 20.0,),
        vec![BufferViewState {
            path,
            cursor_line: 3,
            cursor_column: 2,
            scroll_line: 3,
            horizontal_scroll_offset: 0.0,
            selections: vec![
                BufferSelectionState {
                    anchor_line: 1,
                    anchor_column: 2,
                    cursor_line: 1,
                    cursor_column: 5,
                },
                BufferSelectionState {
                    anchor_line: 3,
                    anchor_column: 6,
                    cursor_line: 3,
                    cursor_column: 2,
                },
            ],
        }]
    );
}

#[test]
fn buffer_view_state_restore_clamps_to_existing_text() {
    let path = PathBuf::from("workspace/src/main.rs");
    let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "one\ntwo".to_owned());
    let state = BufferViewState {
        path,
        cursor_line: 99,
        cursor_column: 99,
        scroll_line: 99,
        horizontal_scroll_offset: 0.0,
        selections: Vec::new(),
    };

    let scroll_line = apply_buffer_view_state(&mut buffer, &state);

    assert_eq!(buffer.cursor_position().line, 1);
    assert_eq!(buffer.cursor_position().column, 3);
    assert_eq!(scroll_line, 1);
}

#[test]
fn buffer_view_state_restore_applies_multiple_selections() {
    let path = PathBuf::from("workspace/src/main.rs");
    let mut buffer = TextBuffer::from_text(
        7,
        Some(path.clone()),
        "alpha\nbeta\ngamma\ndelta".to_owned(),
    );
    let state = BufferViewState {
        path,
        cursor_line: 1,
        cursor_column: 1,
        scroll_line: 2,
        horizontal_scroll_offset: 0.0,
        selections: vec![
            BufferSelectionState {
                anchor_line: 1,
                anchor_column: 2,
                cursor_line: 1,
                cursor_column: 5,
            },
            BufferSelectionState {
                anchor_line: 3,
                anchor_column: 6,
                cursor_line: 3,
                cursor_column: 2,
            },
        ],
    };

    let scroll_line = apply_buffer_view_state(&mut buffer, &state);

    assert_eq!(
        buffer.selections(),
        &[
            Selection {
                anchor: buffer.line_column_to_char(0, 1),
                cursor: buffer.line_column_to_char(0, 4),
            },
            Selection {
                anchor: buffer.line_column_to_char(2, 5),
                cursor: buffer.line_column_to_char(2, 1),
            },
        ]
    );
    assert_eq!(scroll_line, 1);
}

#[test]
fn session_history_states_restore_matching_buffer_undo_stack() {
    let path = PathBuf::from("workspace/src/main.rs");
    let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "one".to_owned());
    buffer.set_single_cursor(buffer.len_chars());
    buffer.insert_at_cursor("\ntwo");

    let mut states = session_history_states(&[buffer], Some(7));
    assert_eq!(states.len(), 1);

    let mut restored = TextBuffer::from_text(8, Some(path), "one\ntwo".to_owned());
    assert!(apply_buffer_history_state(&mut restored, states.remove(0)));
    assert!(restored.undo());
    assert_eq!(restored.text(), "one");
}

#[test]
fn session_history_state_rejects_mismatched_buffer_text() {
    let path = PathBuf::from("workspace/src/main.rs");
    let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "one".to_owned());
    buffer.insert_at_cursor("\ntwo");

    let mut states = session_history_states(&[buffer], Some(7));
    let mut restored = TextBuffer::from_text(8, Some(path), "changed".to_owned());

    assert!(!apply_buffer_history_state(&mut restored, states.remove(0)));
    assert!(!restored.undo());
}

#[test]
fn restored_session_recents_merge_without_overwriting_global_list() {
    let global_first = PathBuf::from("workspace/global-first");
    let current = PathBuf::from("workspace/current");
    let legacy = PathBuf::from("workspace/legacy");
    let global = vec![global_first.clone(), current.clone()];
    let restored = vec![
        legacy.clone(),
        global_first.clone(),
        current.join("src").join(".."),
    ];

    assert_eq!(
        recent_projects_with_recorded(&merged_recent_projects(&global, &restored), current.clone()),
        vec![current, global_first, legacy]
    );
}

#[test]
fn explorer_session_paths_are_workspace_scoped_and_deterministic() {
    let root = PathBuf::from("workspace");
    let outside = PathBuf::from("other");
    let expanded = HashSet::from([
        root.clone(),
        root.join("z"),
        root.join("a"),
        root.join("src").join(".."),
        root.join("src").join("..").join("a"),
        root.join("a").join("nested").join(".."),
        root.join("src").join("..").join("..").join("outside"),
        root.join("..").join("workspace").join("reentered"),
        outside.join("ignored"),
    ]);

    assert_eq!(
        persisted_explorer_expanded_paths(&root, &expanded),
        vec![root.join("a"), root.join("z")]
    );

    assert_eq!(
        restored_explorer_expanded_paths(
            &root,
            vec![
                root.clone(),
                root.join("a"),
                root.join("src").join(".."),
                root.join("src").join("..").join("a"),
                root.join("..").join("workspace").join("reentered"),
                outside.join("ignored")
            ]
        ),
        HashSet::from([root.join("a")])
    );

    assert_eq!(
        workspace_descendant_path_for_session(
            &root,
            &root.join("src").join("..").join("README.md")
        ),
        Some(root.join("README.md"))
    );
    assert_eq!(
        workspace_descendant_path_for_session(&root, &root.join("src").join("..")),
        None
    );
    assert_eq!(
        workspace_descendant_path_for_session(&root, &root.join("src").join("..").join("..")),
        None
    );
    assert_eq!(
        workspace_descendant_path_for_session(
            &root,
            &root.join("..").join("workspace").join("README.md")
        ),
        None
    );
    assert_eq!(
        workspace_descendant_path_for_session(
            &root,
            &root
                .join("src")
                .join("..")
                .join("..")
                .join("workspace")
                .join("README.md")
        ),
        None
    );
}

#[test]
fn restored_explorer_expanded_paths_dedupes_raw_path_aliases() {
    let root = PathBuf::from("workspace");
    let restored = restored_explorer_expanded_paths(
        &root,
        vec![
            root.join("src"),
            root.join("src").join("nested").join(".."),
            root.join(".").join("src"),
            root.join("src").join("..").join("src"),
        ],
    );

    assert_eq!(restored, HashSet::from([root.join("src")]));
}

#[test]
fn terminal_startup_visibility_follows_hide_on_startup_setting() {
    assert!(terminal_visible_after_startup(
        TerminalHideOnStartup::Never,
        true,
        false
    ));
    assert!(!terminal_visible_after_startup(
        TerminalHideOnStartup::Never,
        false,
        true
    ));
    assert!(terminal_visible_after_startup(
        TerminalHideOnStartup::WhenEmpty,
        true,
        true
    ));
    assert!(!terminal_visible_after_startup(
        TerminalHideOnStartup::WhenEmpty,
        true,
        false
    ));
    assert!(!terminal_visible_after_startup(
        TerminalHideOnStartup::Always,
        true,
        true
    ));
}

#[test]
fn source_control_session_modes_round_trip() {
    assert_eq!(
        persisted_source_control_view_mode(SourceControlViewMode::List),
        PersistedSourceControlViewMode::List
    );
    assert_eq!(
        persisted_source_control_view_mode(SourceControlViewMode::Tree),
        PersistedSourceControlViewMode::Tree
    );
    assert_eq!(
        source_control_view_mode_from_persisted(PersistedSourceControlViewMode::Tree),
        SourceControlViewMode::Tree
    );

    assert_eq!(
        persisted_source_control_sort_mode(SourceControlSortMode::Path),
        PersistedSourceControlSortMode::Path
    );
    assert_eq!(
        persisted_source_control_sort_mode(SourceControlSortMode::Name),
        PersistedSourceControlSortMode::Name
    );
    assert_eq!(
        persisted_source_control_sort_mode(SourceControlSortMode::Status),
        PersistedSourceControlSortMode::Status
    );
    assert_eq!(
        source_control_sort_mode_from_persisted(PersistedSourceControlSortMode::Status),
        SourceControlSortMode::Status
    );
}
