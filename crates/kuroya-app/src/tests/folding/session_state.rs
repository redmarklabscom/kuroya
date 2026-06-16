use crate::{
    folding::{
        FoldedRange, clamp_folded_ranges_for_line_count, folded_ranges_from_session,
        session_fold_states,
    },
    persistence::{BufferFoldState, PersistedFoldRange},
};
use std::{collections::HashMap, path::PathBuf};

#[test]
fn session_fold_states_round_trip_normalized_ranges() {
    let first_path = PathBuf::from("workspace/src/main.rs");
    let second_path = PathBuf::from("workspace/src/lib.rs");
    let folded = HashMap::from([
        (
            first_path.clone(),
            vec![
                FoldedRange {
                    start_line: 8,
                    end_line: 12,
                },
                FoldedRange {
                    start_line: 8,
                    end_line: 12,
                },
                FoldedRange {
                    start_line: 0,
                    end_line: 10,
                },
            ],
        ),
        (
            second_path.clone(),
            vec![FoldedRange {
                start_line: 2,
                end_line: 4,
            }],
        ),
    ]);

    let states = session_fold_states(&folded);
    assert_eq!(
        states,
        vec![
            BufferFoldState {
                path: second_path.clone(),
                ranges: vec![PersistedFoldRange {
                    start_line: 2,
                    end_line: 4,
                }],
            },
            BufferFoldState {
                path: first_path.clone(),
                ranges: vec![PersistedFoldRange {
                    start_line: 8,
                    end_line: 12,
                }],
            },
        ]
    );

    let restored = folded_ranges_from_session(&states);
    assert_eq!(
        restored.get(&first_path),
        Some(&vec![FoldedRange {
            start_line: 8,
            end_line: 12,
        }])
    );
    assert_eq!(
        restored.get(&second_path),
        Some(&vec![FoldedRange {
            start_line: 2,
            end_line: 4,
        }])
    );
}

#[test]
fn restored_fold_ranges_are_clamped_to_loaded_file_length() {
    let path = PathBuf::from("workspace/src/main.rs");
    let mut folded = HashMap::from([(
        path.clone(),
        vec![
            FoldedRange {
                start_line: 2,
                end_line: 99,
            },
            FoldedRange {
                start_line: 7,
                end_line: 9,
            },
        ],
    )]);

    clamp_folded_ranges_for_line_count(&mut folded, &path, 4);

    assert_eq!(
        folded.get(&path),
        Some(&vec![FoldedRange {
            start_line: 2,
            end_line: 4,
        }])
    );
}

#[test]
fn session_fold_states_skip_pathless_entries() {
    let path = PathBuf::from("workspace/src/main.rs");
    let folded = HashMap::from([
        (
            PathBuf::new(),
            vec![FoldedRange {
                start_line: 1,
                end_line: 3,
            }],
        ),
        (
            path.clone(),
            vec![FoldedRange {
                start_line: 2,
                end_line: 4,
            }],
        ),
    ]);

    assert_eq!(
        session_fold_states(&folded),
        vec![BufferFoldState {
            path,
            ranges: vec![PersistedFoldRange {
                start_line: 2,
                end_line: 4,
            }],
        }]
    );
}

#[test]
fn restored_session_folds_skip_pathless_and_invalid_entries() {
    let path = PathBuf::from("workspace/src/main.rs");
    let states = vec![
        BufferFoldState {
            path: PathBuf::new(),
            ranges: vec![PersistedFoldRange {
                start_line: 1,
                end_line: 3,
            }],
        },
        BufferFoldState {
            path: path.clone(),
            ranges: vec![
                PersistedFoldRange {
                    start_line: 0,
                    end_line: 4,
                },
                PersistedFoldRange {
                    start_line: 5,
                    end_line: 5,
                },
                PersistedFoldRange {
                    start_line: 7,
                    end_line: 9,
                },
            ],
        },
        BufferFoldState {
            path: path.clone(),
            ranges: vec![PersistedFoldRange {
                start_line: 7,
                end_line: 9,
            }],
        },
    ];

    let restored = folded_ranges_from_session(&states);

    assert_eq!(
        restored.get(&path),
        Some(&vec![FoldedRange {
            start_line: 7,
            end_line: 9,
        }])
    );
    assert!(!restored.contains_key(&PathBuf::new()));
}

#[test]
fn clamping_loaded_single_line_file_removes_stale_session_folds() {
    let path = PathBuf::from("workspace/src/main.rs");
    let mut folded = HashMap::from([(
        path.clone(),
        vec![
            FoldedRange {
                start_line: 1,
                end_line: 3,
            },
            FoldedRange {
                start_line: 3,
                end_line: 5,
            },
        ],
    )]);

    clamp_folded_ranges_for_line_count(&mut folded, &path, 1);

    assert!(!folded.contains_key(&path));
}
