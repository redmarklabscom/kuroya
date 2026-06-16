use crate::folding::{
    FoldedRange, folded_range_starting_at, remove_fold_containing_line, remove_folds_hiding_line,
    retain_folded_ranges_matching_folding_ranges, toggle_folded_range, visible_line_indices,
    visible_row_for_line,
};
use kuroya_core::LspFoldingRange;

#[test]
fn visible_line_indices_hide_folded_children() {
    let folded = vec![
        FoldedRange {
            start_line: 2,
            end_line: 4,
        },
        FoldedRange {
            start_line: 6,
            end_line: 7,
        },
    ];

    assert_eq!(visible_line_indices(8, &folded), vec![0, 1, 4, 5, 7]);
    assert_eq!(visible_row_for_line(&[0, 1, 4, 5, 7], 3), 1);
    assert_eq!(visible_row_for_line(&[0, 1, 4, 5, 7], 4), 2);
}

#[test]
fn visible_line_indices_uses_largest_fold_when_ranges_share_start() {
    let folded = vec![
        FoldedRange {
            start_line: 2,
            end_line: 4,
        },
        FoldedRange {
            start_line: 2,
            end_line: 7,
        },
        FoldedRange {
            start_line: 4,
            end_line: 5,
        },
        FoldedRange {
            start_line: 8,
            end_line: 9,
        },
    ];

    assert_eq!(visible_line_indices(10, &folded), vec![0, 1, 7, 9]);
}

#[test]
fn visible_line_indices_ignore_invalid_and_hidden_start_ranges() {
    let folded = vec![
        FoldedRange {
            start_line: 0,
            end_line: 8,
        },
        FoldedRange {
            start_line: 2,
            end_line: 8,
        },
        FoldedRange {
            start_line: 4,
            end_line: 12,
        },
        FoldedRange {
            start_line: 10,
            end_line: 99,
        },
        FoldedRange {
            start_line: 11,
            end_line: 10,
        },
    ];

    assert_eq!(visible_line_indices(12, &folded), vec![0, 1, 8, 9]);
}

#[test]
fn toggle_folded_range_adds_and_removes_exact_range() {
    let mut folded = Vec::new();
    let range = FoldedRange {
        start_line: 3,
        end_line: 8,
    };

    assert!(toggle_folded_range(&mut folded, range));
    assert_eq!(folded, vec![range]);
    assert!(!toggle_folded_range(&mut folded, range));
    assert!(folded.is_empty());
}

#[test]
fn toggle_folded_range_rejects_invalid_ranges() {
    let mut folded = vec![
        FoldedRange {
            start_line: 0,
            end_line: 8,
        },
        FoldedRange {
            start_line: 4,
            end_line: 4,
        },
    ];

    assert!(!toggle_folded_range(
        &mut folded,
        FoldedRange {
            start_line: 0,
            end_line: 10,
        }
    ));
    assert!(folded.is_empty());
}

#[test]
fn folded_range_starting_at_ignores_invalid_ranges() {
    let folded = vec![
        FoldedRange {
            start_line: 0,
            end_line: 3,
        },
        FoldedRange {
            start_line: 3,
            end_line: 3,
        },
        FoldedRange {
            start_line: 3,
            end_line: 6,
        },
    ];

    assert_eq!(folded_range_starting_at(&folded, 0), None);
    assert_eq!(
        folded_range_starting_at(&folded, 3),
        Some(FoldedRange {
            start_line: 3,
            end_line: 6,
        })
    );
}

#[test]
fn retain_folded_ranges_matching_folding_ranges_drops_stale_folds() {
    let mut folded = vec![
        FoldedRange {
            start_line: 4,
            end_line: 8,
        },
        FoldedRange {
            start_line: 0,
            end_line: 4,
        },
        FoldedRange {
            start_line: 1,
            end_line: 3,
        },
        FoldedRange {
            start_line: 1,
            end_line: 3,
        },
    ];
    let ranges = vec![folding_range(1, 3), folding_range(6, 8)];

    retain_folded_ranges_matching_folding_ranges(&mut folded, &ranges);

    assert_eq!(
        folded,
        vec![FoldedRange {
            start_line: 1,
            end_line: 3,
        }]
    );

    retain_folded_ranges_matching_folding_ranges(&mut folded, &[]);
    assert!(folded.is_empty());
}

#[test]
fn retain_folded_ranges_matching_folding_ranges_remaps_unambiguous_start_lines() {
    let mut folded = vec![
        FoldedRange {
            start_line: 2,
            end_line: 5,
        },
        FoldedRange {
            start_line: 8,
            end_line: 10,
        },
        FoldedRange {
            start_line: 12,
            end_line: 18,
        },
    ];
    let ranges = vec![
        folding_range(2, 6),
        folding_range(8, 10),
        folding_range(12, 14),
        folding_range(12, 20),
    ];

    retain_folded_ranges_matching_folding_ranges(&mut folded, &ranges);

    assert_eq!(
        folded,
        vec![
            FoldedRange {
                start_line: 2,
                end_line: 6,
            },
            FoldedRange {
                start_line: 8,
                end_line: 10,
            },
        ]
    );
}

#[test]
fn remove_fold_containing_line_drops_invalid_ranges_before_matching() {
    let mut folded = vec![
        FoldedRange {
            start_line: 0,
            end_line: 9,
        },
        FoldedRange {
            start_line: 5,
            end_line: 5,
        },
        FoldedRange {
            start_line: 2,
            end_line: 4,
        },
    ];

    assert!(remove_fold_containing_line(&mut folded, 3));
    assert!(folded.is_empty());
}

#[test]
fn remove_folds_hiding_line_removes_only_folds_covering_hidden_target() {
    let mut folded = vec![
        FoldedRange {
            start_line: 1,
            end_line: 8,
        },
        FoldedRange {
            start_line: 3,
            end_line: 5,
        },
        FoldedRange {
            start_line: 4,
            end_line: 6,
        },
        FoldedRange {
            start_line: 7,
            end_line: 8,
        },
    ];

    assert!(remove_folds_hiding_line(&mut folded, 4));
    assert_eq!(
        folded,
        vec![
            FoldedRange {
                start_line: 4,
                end_line: 6,
            },
            FoldedRange {
                start_line: 7,
                end_line: 8,
            },
        ]
    );
}

fn folding_range(start_line: usize, end_line: usize) -> LspFoldingRange {
    LspFoldingRange {
        start_line,
        start_column: None,
        end_line,
        end_column: None,
        kind: None,
    }
}
