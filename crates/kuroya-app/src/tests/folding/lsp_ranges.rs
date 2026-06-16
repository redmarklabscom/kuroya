use crate::folding::{
    FoldedRange, best_folding_range_starting_at, fallback_folding_ranges,
    fold_import_ranges_by_default,
};
use kuroya_core::{LspFoldingRange, TextBuffer};

#[test]
fn best_folding_range_prefers_smallest_starting_at_cursor() {
    let ranges = vec![
        LspFoldingRange {
            start_line: 4,
            start_column: None,
            end_line: 20,
            end_column: None,
            kind: Some("region".to_owned()),
        },
        LspFoldingRange {
            start_line: 4,
            start_column: None,
            end_line: 8,
            end_column: None,
            kind: None,
        },
        LspFoldingRange {
            start_line: 5,
            start_column: None,
            end_line: 9,
            end_column: None,
            kind: None,
        },
    ];

    assert_eq!(
        best_folding_range_starting_at(&ranges, 4),
        Some(FoldedRange {
            start_line: 4,
            end_line: 8,
        })
    );
    assert_eq!(best_folding_range_starting_at(&ranges, 2), None);
}

#[test]
fn best_folding_range_ignores_invalid_ranges() {
    let ranges = vec![
        LspFoldingRange {
            start_line: 0,
            start_column: None,
            end_line: 5,
            end_column: None,
            kind: None,
        },
        LspFoldingRange {
            start_line: 4,
            start_column: None,
            end_line: 4,
            end_column: None,
            kind: None,
        },
        LspFoldingRange {
            start_line: 4,
            start_column: None,
            end_line: 8,
            end_column: None,
            kind: None,
        },
    ];

    assert_eq!(best_folding_range_starting_at(&ranges, 0), None);
    assert_eq!(
        best_folding_range_starting_at(&ranges, 4),
        Some(FoldedRange {
            start_line: 4,
            end_line: 8,
        })
    );
}

#[test]
fn fallback_folding_ranges_use_multiline_bracket_pairs() {
    let buffer = TextBuffer::from_text(
        1,
        None,
        "fn main() {\nlet ready = true;\nif ready {\nprintln!();\n}\n}\n".to_owned(),
    );

    let ranges = fallback_folding_ranges(&buffer);

    assert_eq!(
        best_folding_range_starting_at(&ranges, 1),
        Some(FoldedRange {
            start_line: 1,
            end_line: 6,
        })
    );
    assert_eq!(
        best_folding_range_starting_at(&ranges, 3),
        Some(FoldedRange {
            start_line: 3,
            end_line: 5,
        })
    );
}

#[test]
fn fallback_folding_ranges_ignore_brackets_inside_strings_and_line_comments() {
    let buffer = TextBuffer::from_text(
        1,
        None,
        "let text = \"{\";\nlet end = \"}\";\nif ready {\nwork();\n}\n// not a fold {\n// still not }\ntail();\n"
            .to_owned(),
    );

    let ranges = fallback_folding_ranges(&buffer);

    assert!(has_folding_range(&ranges, 3, 5), "{ranges:?}");
    assert!(!has_folding_range(&ranges, 1, 2), "{ranges:?}");
    assert!(!has_folding_range(&ranges, 6, 7), "{ranges:?}");
}

#[test]
fn fallback_folding_ranges_ignore_brackets_inside_block_comments() {
    let buffer = TextBuffer::from_text(
        1,
        None,
        "/*\n{\n}\n*/\nfn main() {\nwork();\n}\n".to_owned(),
    );

    let ranges = fallback_folding_ranges(&buffer);

    assert!(has_folding_range(&ranges, 5, 7), "{ranges:?}");
    assert!(!has_folding_range(&ranges, 2, 3), "{ranges:?}");
}

#[test]
fn fallback_folding_ranges_use_indentation_blocks() {
    let buffer = TextBuffer::from_text(
        1,
        None,
        "def run():\n    if ready:\n        work()\n    done()\ntail()\n".to_owned(),
    );

    let ranges = fallback_folding_ranges(&buffer);

    assert_eq!(
        best_folding_range_starting_at(&ranges, 1),
        Some(FoldedRange {
            start_line: 1,
            end_line: 4,
        })
    );
    assert_eq!(
        best_folding_range_starting_at(&ranges, 2),
        Some(FoldedRange {
            start_line: 2,
            end_line: 3,
        })
    );
}

fn has_folding_range(ranges: &[LspFoldingRange], start_line: usize, end_line: usize) -> bool {
    ranges
        .iter()
        .any(|range| range.start_line == start_line && range.end_line == end_line)
}

#[test]
fn import_folding_defaults_add_only_import_ranges_once() {
    let ranges = vec![
        LspFoldingRange {
            start_line: 1,
            start_column: None,
            end_line: 3,
            end_column: None,
            kind: Some("imports".to_owned()),
        },
        LspFoldingRange {
            start_line: 5,
            start_column: None,
            end_line: 9,
            end_column: None,
            kind: Some("region".to_owned()),
        },
        LspFoldingRange {
            start_line: 12,
            start_column: None,
            end_line: 15,
            end_column: None,
            kind: Some("Imports".to_owned()),
        },
    ];
    let mut folded = vec![FoldedRange {
        start_line: 20,
        end_line: 25,
    }];

    assert_eq!(fold_import_ranges_by_default(&mut folded, &ranges, true), 2);
    assert_eq!(
        folded,
        vec![
            FoldedRange {
                start_line: 1,
                end_line: 3,
            },
            FoldedRange {
                start_line: 12,
                end_line: 15,
            },
            FoldedRange {
                start_line: 20,
                end_line: 25,
            },
        ]
    );
    assert_eq!(fold_import_ranges_by_default(&mut folded, &ranges, true), 0);
    assert_eq!(
        fold_import_ranges_by_default(&mut folded, &ranges, false),
        0
    );
}

#[test]
fn import_folding_defaults_count_only_valid_ranges() {
    let ranges = vec![
        LspFoldingRange {
            start_line: 0,
            start_column: None,
            end_line: 3,
            end_column: None,
            kind: Some("imports".to_owned()),
        },
        LspFoldingRange {
            start_line: 5,
            start_column: None,
            end_line: 5,
            end_column: None,
            kind: Some("imports".to_owned()),
        },
        LspFoldingRange {
            start_line: 8,
            start_column: None,
            end_line: 9,
            end_column: None,
            kind: Some("imports".to_owned()),
        },
    ];
    let mut folded = Vec::new();

    assert_eq!(fold_import_ranges_by_default(&mut folded, &ranges, true), 1);
    assert_eq!(
        folded,
        vec![FoldedRange {
            start_line: 8,
            end_line: 9,
        }]
    );
}

#[test]
fn import_folding_defaults_skip_nested_import_ranges() {
    let ranges = vec![
        LspFoldingRange {
            start_line: 1,
            start_column: None,
            end_line: 3,
            end_column: None,
            kind: Some("imports".to_owned()),
        },
        LspFoldingRange {
            start_line: 1,
            start_column: None,
            end_line: 6,
            end_column: None,
            kind: Some("imports".to_owned()),
        },
        LspFoldingRange {
            start_line: 4,
            start_column: None,
            end_line: 6,
            end_column: None,
            kind: Some("imports".to_owned()),
        },
    ];
    let mut folded = Vec::new();

    assert_eq!(fold_import_ranges_by_default(&mut folded, &ranges, true), 1);
    assert_eq!(
        folded,
        vec![FoldedRange {
            start_line: 1,
            end_line: 6,
        }]
    );
}
