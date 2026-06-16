use kuroya_core::{LspCompletionItem, LspTextEdit};

const MAX_COMPLETION_IMPORT_EDIT_BYTES: usize = 64 * 1024;
const MAX_COMPLETION_IMPORT_EDITS: usize = 64;
const MAX_COMPLETION_IMPORT_TOTAL_BYTES: usize = 128 * 1024;
const MAX_COMPLETION_IMPORT_LINES: usize = 256;
const MAX_COMPLETION_IMPORT_LINE_BYTES: usize = 4 * 1024;

pub(crate) fn completion_has_auto_import_edit(item: &LspCompletionItem) -> bool {
    if item.additional_text_edits.is_empty() {
        return false;
    }
    if item.additional_text_edits.len() > MAX_COMPLETION_IMPORT_EDITS {
        return false;
    }

    let mut has_import = false;
    let mut total_new_text_bytes = 0usize;
    for edit in &item.additional_text_edits {
        let Some(total) = total_new_text_bytes.checked_add(edit.new_text.len()) else {
            return false;
        };
        total_new_text_bytes = total;
        if total_new_text_bytes > MAX_COMPLETION_IMPORT_TOTAL_BYTES {
            return false;
        }
        if text_edit_adds_import(edit) {
            has_import = true;
        } else if text_edit_has_effect(edit) {
            return false;
        }
    }
    has_import
}

fn text_edit_adds_import(edit: &LspTextEdit) -> bool {
    if !text_edit_is_insertion(edit) {
        return false;
    }
    if edit.new_text.len() > MAX_COMPLETION_IMPORT_EDIT_BYTES {
        return false;
    }

    let mut has_import = false;
    let mut lines = 0usize;
    for line in edit.new_text.lines() {
        lines += 1;
        if lines > MAX_COMPLETION_IMPORT_LINES {
            return false;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.len() > MAX_COMPLETION_IMPORT_LINE_BYTES {
            return false;
        }
        if !likely_import_line(line) {
            return false;
        }
        has_import = true;
    }
    has_import
}

fn text_edit_is_insertion(edit: &LspTextEdit) -> bool {
    edit.start_line == edit.end_line && edit.start_column == edit.end_column
}

fn text_edit_has_effect(edit: &LspTextEdit) -> bool {
    edit.start_line != edit.end_line
        || edit.start_column != edit.end_column
        || !edit.new_text.is_empty()
}

fn likely_import_line(line: &str) -> bool {
    starts_with_ascii_case_insensitive(line, "use ")
        || starts_with_ascii_case_insensitive(line, "pub use ")
        || starts_with_ascii_case_insensitive(line, "import ")
        || starts_with_ascii_case_insensitive(line, "from ")
}

fn starts_with_ascii_case_insensitive(text: &str, prefix: &str) -> bool {
    text.get(..prefix.len())
        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(prefix))
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_COMPLETION_IMPORT_EDIT_BYTES, MAX_COMPLETION_IMPORT_EDITS, MAX_COMPLETION_IMPORT_LINES,
        MAX_COMPLETION_IMPORT_TOTAL_BYTES, completion_has_auto_import_edit,
    };
    use kuroya_core::{LspCompletionItem, LspTextEdit};
    use std::path::PathBuf;

    fn item(additional_text: &str) -> LspCompletionItem {
        LspCompletionItem {
            label: "HashMap".to_owned(),
            detail: None,
            documentation: None,
            kind: Some(7),
            deprecated: false,
            is_snippet: false,
            sort_text: None,
            filter_text: None,
            preselect: false,
            commit_characters: Vec::new(),
            insert_text: "HashMap".to_owned(),
            snippet_selection: None,
            snippet_tabstops: Vec::new(),
            snippet_tabstop_groups: Vec::new(),
            text_edit: None,
            insert_text_edit: None,
            additional_text_edits: vec![LspTextEdit {
                path: PathBuf::from("src/main.rs"),
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 1,
                new_text: additional_text.to_owned(),
            }],
            resolve_payload: None,
        }
    }

    #[test]
    fn completion_import_detection_recognizes_common_import_edits() {
        assert!(completion_has_auto_import_edit(&item(
            "use std::collections::HashMap;\n",
        )));
        assert!(completion_has_auto_import_edit(&item(
            "import { useMemo } from 'react';\n",
        )));
        assert!(completion_has_auto_import_edit(&item(
            "from pathlib import Path\n",
        )));
    }

    #[test]
    fn completion_import_detection_ignores_regular_side_edits() {
        assert!(!completion_has_auto_import_edit(&item(
            "let value = HashMap::new();\n",
        )));
    }

    #[test]
    fn completion_import_detection_ignores_mixed_side_effect_edits() {
        assert!(!completion_has_auto_import_edit(&item(
            "use std::collections::HashMap;\nlet value = HashMap::new();\n",
        )));
    }

    #[test]
    fn completion_import_detection_ignores_imports_with_separate_side_effect_edits() {
        let mut item = item("use std::collections::HashMap;\n");
        item.additional_text_edits.push(LspTextEdit {
            path: PathBuf::from("src/main.rs"),
            start_line: 3,
            start_column: 1,
            end_line: 3,
            end_column: 1,
            new_text: "let value = HashMap::new();\n".to_owned(),
        });

        assert!(!completion_has_auto_import_edit(&item));
    }

    #[test]
    fn completion_import_detection_ignores_replacement_import_edits() {
        let mut item = item("use std::collections::HashMap;\n");
        item.additional_text_edits[0].end_column = 8;

        assert!(!completion_has_auto_import_edit(&item));
    }

    #[test]
    fn completion_import_detection_ignores_oversized_import_text() {
        let huge_import = format!(
            "use std::collections::{};\n",
            "HashMap".repeat(MAX_COMPLETION_IMPORT_EDIT_BYTES)
        );

        assert!(!completion_has_auto_import_edit(&item(&huge_import)));
    }

    #[test]
    fn completion_import_detection_ignores_too_many_import_lines() {
        let many_imports = "use std::fmt::Debug;\n".repeat(MAX_COMPLETION_IMPORT_LINES + 1);

        assert!(!completion_has_auto_import_edit(&item(&many_imports)));
    }

    #[test]
    fn completion_import_detection_ignores_too_many_import_edits() {
        let mut completion = item("use std::fmt::Debug;\n");
        completion.additional_text_edits = (0..=MAX_COMPLETION_IMPORT_EDITS)
            .map(|idx| LspTextEdit {
                path: PathBuf::from("src/main.rs"),
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 1,
                new_text: format!("use std::fmt::Debug{idx};\n"),
            })
            .collect();

        assert!(!completion_has_auto_import_edit(&completion));
    }

    #[test]
    fn completion_import_detection_ignores_aggregate_oversized_import_text() {
        let long_import = format!("use std::{};\n", "x".repeat(4_000));
        let edit_count = (MAX_COMPLETION_IMPORT_TOTAL_BYTES / long_import.len()) + 1;
        assert!(edit_count <= MAX_COMPLETION_IMPORT_EDITS);
        let mut completion = item("use std::fmt::Debug;\n");
        completion.additional_text_edits = (0..edit_count)
            .map(|_| LspTextEdit {
                path: PathBuf::from("src/main.rs"),
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 1,
                new_text: long_import.clone(),
            })
            .collect();

        assert!(!completion_has_auto_import_edit(&completion));
    }
}
