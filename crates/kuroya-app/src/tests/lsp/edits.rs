use crate::{
    lsp_edits::apply_lsp_edits_to_text,
    lsp_rename_preview::{lsp_rename_preview_counts, lsp_rename_preview_edit_label},
};
use kuroya_core::LspTextEdit;
use std::path::PathBuf;

#[test]
fn lsp_text_edits_apply_to_text_by_line_column() {
    let path = PathBuf::from("src/lib.rs");
    let edits = vec![
        LspTextEdit {
            path: path.clone(),
            start_line: 1,
            start_column: 5,
            end_line: 1,
            end_column: 10,
            new_text: "renamed".to_owned(),
        },
        LspTextEdit {
            path: path.clone(),
            start_line: 2,
            start_column: 1,
            end_line: 2,
            end_column: 6,
            new_text: "renamed".to_owned(),
        },
    ];

    let text =
        apply_lsp_edits_to_text(&path, "let alpha = 1;\nalpha\n".to_owned(), &edits).unwrap();
    assert_eq!(text, "let renamed = 1;\nrenamed\n");
}

#[test]
fn lsp_text_edits_reject_out_of_range_positions() {
    let path = PathBuf::from("src/lib.rs");
    let text = "let alpha = 1;\n".to_owned();

    assert!(
        apply_lsp_edits_to_text(
            &path,
            text.clone(),
            &[LspTextEdit {
                path: path.clone(),
                start_line: 3,
                start_column: 1,
                end_line: 3,
                end_column: 1,
                new_text: "renamed".to_owned(),
            }],
        )
        .is_none()
    );
    assert!(
        apply_lsp_edits_to_text(
            &path,
            text,
            &[LspTextEdit {
                path: path.clone(),
                start_line: 1,
                start_column: 40,
                end_line: 1,
                end_column: 40,
                new_text: "renamed".to_owned(),
            }],
        )
        .is_none()
    );
}

#[test]
fn lsp_text_edits_allow_exact_line_end_insertions() {
    let path = PathBuf::from("src/lib.rs");
    let edits = vec![LspTextEdit {
        path: path.clone(),
        start_line: 1,
        start_column: 15,
        end_line: 1,
        end_column: 15,
        new_text: " // ok".to_owned(),
    }];

    let text = apply_lsp_edits_to_text(&path, "let alpha = 1;\n".to_owned(), &edits).unwrap();

    assert_eq!(text, "let alpha = 1; // ok\n");
}

#[test]
fn lsp_text_edits_interpret_columns_as_utf16_code_units() {
    let path = PathBuf::from("src/lib.rs");
    let edits = vec![LspTextEdit {
        path: path.clone(),
        start_line: 1,
        start_column: 3,
        end_line: 1,
        end_column: 8,
        new_text: "renamed".to_owned(),
    }];

    let text = apply_lsp_edits_to_text(&path, "😀alpha\n".to_owned(), &edits).unwrap();

    assert_eq!(text, "😀renamed\n");
}

#[test]
fn lsp_text_edits_reject_columns_inside_utf16_surrogates() {
    let path = PathBuf::from("src/lib.rs");
    let edits = vec![LspTextEdit {
        path: path.clone(),
        start_line: 1,
        start_column: 2,
        end_line: 1,
        end_column: 2,
        new_text: "invalid".to_owned(),
    }];

    assert!(apply_lsp_edits_to_text(&path, "😀alpha\n".to_owned(), &edits).is_none());
}

#[test]
fn lsp_text_edits_reject_overlapping_ranges() {
    let path = PathBuf::from("src/lib.rs");
    let edits = vec![
        LspTextEdit {
            path: path.clone(),
            start_line: 1,
            start_column: 5,
            end_line: 1,
            end_column: 10,
            new_text: "renamed".to_owned(),
        },
        LspTextEdit {
            path: path.clone(),
            start_line: 1,
            start_column: 8,
            end_line: 1,
            end_column: 13,
            new_text: "other".to_owned(),
        },
    ];

    assert!(apply_lsp_edits_to_text(&path, "let alpha = 1;\n".to_owned(), &edits).is_none());
}

#[test]
fn lsp_text_edits_allow_multiple_insertions_at_same_position() {
    let path = PathBuf::from("src/lib.rs");
    let edits = vec![
        LspTextEdit {
            path: path.clone(),
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 1,
            new_text: "first\n".to_owned(),
        },
        LspTextEdit {
            path: path.clone(),
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 1,
            new_text: "second\n".to_owned(),
        },
    ];

    let text = apply_lsp_edits_to_text(&path, "let alpha = 1;\n".to_owned(), &edits).unwrap();

    assert_eq!(text, "first\nsecond\nlet alpha = 1;\n");
}

#[test]
fn lsp_rename_preview_summarizes_files_and_edit_ranges() {
    let first = PathBuf::from("src/lib.rs");
    let second = PathBuf::from("src/main.rs");
    let edits = vec![
        LspTextEdit {
            path: first.clone(),
            start_line: 1,
            start_column: 5,
            end_line: 1,
            end_column: 10,
            new_text: "renamed".to_owned(),
        },
        LspTextEdit {
            path: first,
            start_line: 2,
            start_column: 1,
            end_line: 2,
            end_column: 6,
            new_text: String::new(),
        },
        LspTextEdit {
            path: second,
            start_line: 3,
            start_column: 2,
            end_line: 3,
            end_column: 7,
            new_text: "multi\nline".to_owned(),
        },
    ];

    assert_eq!(lsp_rename_preview_counts(&edits), (2, 3));
    assert_eq!(
        lsp_rename_preview_edit_label(&edits[0]),
        "1:5-1:10 -> renamed"
    );
    assert_eq!(
        lsp_rename_preview_edit_label(&edits[1]),
        "2:1-2:6 -> (delete)"
    );
    assert_eq!(
        lsp_rename_preview_edit_label(&edits[2]),
        "3:2-3:7 -> multi\\nline"
    );
}
