use crate::lsp_edits::{
    apply_completion_passthrough_events, apply_completion_passthrough_events_with_editor_keys,
    completion_buffer_edit_plan, completion_buffer_edits, completion_passthrough_edit_intent,
    completion_passthrough_edit_intent_with_acceptance,
};
use eframe::egui::{Event, ImeEvent, Key, Modifiers};
use kuroya_core::{
    AutoPairSettings, EditorSuggestInsertMode, LspCompletionItem, LspTextEdit, TextBuffer,
};
use std::path::PathBuf;

#[test]
fn completion_buffer_edits_apply_insert_text_and_additional_edits_together() {
    let path = PathBuf::from("src/lib.rs");
    let mut buffer = TextBuffer::from_text(
        1,
        Some(path.clone()),
        "fn main() {\n    Form\n}\n".to_owned(),
    );
    buffer.set_single_cursor(buffer.line_column_to_char(1, 8));
    let item = LspCompletionItem {
        label: "Formatter".to_owned(),
        detail: Some("struct".to_owned()),
        documentation: None,
        kind: Some(7),
        deprecated: false,
        is_snippet: false,
        sort_text: None,
        filter_text: None,
        preselect: false,
        commit_characters: Vec::new(),
        insert_text: "Formatter".to_owned(),
        snippet_selection: None,
        snippet_tabstops: Vec::new(),
        snippet_tabstop_groups: Vec::new(),
        text_edit: None,
        insert_text_edit: None,
        additional_text_edits: vec![LspTextEdit {
            path,
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 1,
            new_text: "use std::fmt::Formatter;\n".to_owned(),
        }],
        resolve_payload: None,
    };

    let edits = completion_buffer_edits(&buffer, &item, EditorSuggestInsertMode::Insert).unwrap();
    assert!(buffer.apply_edits(edits));
    assert_eq!(
        buffer.text(),
        "use std::fmt::Formatter;\nfn main() {\n    Formatter\n}\n"
    );
}

#[test]
fn completion_buffer_edits_accept_lexically_equivalent_paths() {
    let path = PathBuf::from("workspace/src/lib.rs");
    let edit_path = PathBuf::from("workspace/src/../src/lib.rs");
    let mut buffer = TextBuffer::from_text(1, Some(path), "fn main() {\n    Form\n}\n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(1, 8));
    let item = LspCompletionItem {
        label: "Formatter".to_owned(),
        detail: Some("struct".to_owned()),
        documentation: None,
        kind: Some(7),
        deprecated: false,
        is_snippet: false,
        sort_text: None,
        filter_text: None,
        preselect: false,
        commit_characters: Vec::new(),
        insert_text: "Formatter".to_owned(),
        snippet_selection: None,
        snippet_tabstops: Vec::new(),
        snippet_tabstop_groups: Vec::new(),
        text_edit: None,
        insert_text_edit: None,
        additional_text_edits: vec![LspTextEdit {
            path: edit_path,
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 1,
            new_text: "use std::fmt::Formatter;\n".to_owned(),
        }],
        resolve_payload: None,
    };

    let edits = completion_buffer_edits(&buffer, &item, EditorSuggestInsertMode::Insert).unwrap();
    assert!(buffer.apply_edits(edits));
    assert_eq!(
        buffer.text(),
        "use std::fmt::Formatter;\nfn main() {\n    Formatter\n}\n"
    );
}

#[test]
fn completion_buffer_edits_reject_overlapping_additional_and_primary_edits() {
    let path = PathBuf::from("src/lib.rs");
    let mut buffer = TextBuffer::from_text(1, Some(path.clone()), "Hash".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 4));
    let item = LspCompletionItem {
        label: "HashMap".to_owned(),
        detail: Some("struct".to_owned()),
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
        text_edit: Some(LspTextEdit {
            path: path.clone(),
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 5,
            new_text: "HashMap".to_owned(),
        }),
        insert_text_edit: None,
        additional_text_edits: vec![LspTextEdit {
            path,
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 3,
            new_text: "use std::collections::HashMap;\n".to_owned(),
        }],
        resolve_payload: None,
    };

    assert!(completion_buffer_edits(&buffer, &item, EditorSuggestInsertMode::Insert).is_none());
}

#[test]
fn completion_buffer_edits_reject_additional_edits_for_other_paths() {
    let path = PathBuf::from("src/lib.rs");
    let mut buffer = TextBuffer::from_text(1, Some(path.clone()), "Hash".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 4));
    let item = LspCompletionItem {
        label: "HashMap".to_owned(),
        detail: Some("struct".to_owned()),
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
            path: PathBuf::from("src/other.rs"),
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 1,
            new_text: "use std::collections::HashMap;\n".to_owned(),
        }],
        resolve_payload: None,
    };

    assert!(completion_buffer_edits(&buffer, &item, EditorSuggestInsertMode::Insert).is_none());
    assert_eq!(buffer.text(), "Hash");
}

#[test]
fn completion_buffer_edits_follow_insert_or_replace_mode() {
    let path = PathBuf::from("src/lib.rs");
    let mut buffer = TextBuffer::from_text(1, Some(path), "print suffix".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 3));
    let item = LspCompletionItem {
        label: "println!".to_owned(),
        detail: None,
        documentation: None,
        kind: Some(3),
        deprecated: false,
        is_snippet: false,
        sort_text: None,
        filter_text: None,
        preselect: false,
        commit_characters: Vec::new(),
        insert_text: "println!".to_owned(),
        snippet_selection: None,
        snippet_tabstops: Vec::new(),
        snippet_tabstop_groups: Vec::new(),
        text_edit: None,
        insert_text_edit: None,
        additional_text_edits: Vec::new(),
        resolve_payload: None,
    };

    let mut insert_buffer = buffer.clone();
    let insert_edits =
        completion_buffer_edits(&insert_buffer, &item, EditorSuggestInsertMode::Insert).unwrap();
    assert!(insert_buffer.apply_edits(insert_edits));
    assert_eq!(insert_buffer.text(), "println!nt suffix");

    let replace_edits =
        completion_buffer_edits(&buffer, &item, EditorSuggestInsertMode::Replace).unwrap();
    assert!(buffer.apply_edits(replace_edits));
    assert_eq!(buffer.text(), "println! suffix");
}

#[test]
fn completion_buffer_edits_use_insert_replace_text_edit_for_insert_mode() {
    let path = PathBuf::from("src/lib.rs");
    let mut buffer = TextBuffer::from_text(1, Some(path.clone()), "print".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 3));
    let item = LspCompletionItem {
        label: "println!".to_owned(),
        detail: None,
        documentation: None,
        kind: Some(3),
        deprecated: false,
        is_snippet: false,
        sort_text: None,
        filter_text: None,
        preselect: false,
        commit_characters: Vec::new(),
        insert_text: "println!".to_owned(),
        snippet_selection: None,
        snippet_tabstops: Vec::new(),
        snippet_tabstop_groups: Vec::new(),
        text_edit: Some(LspTextEdit {
            path: path.clone(),
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 6,
            new_text: "println!".to_owned(),
        }),
        insert_text_edit: Some(LspTextEdit {
            path,
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 4,
            new_text: "println!".to_owned(),
        }),
        additional_text_edits: Vec::new(),
        resolve_payload: None,
    };

    let mut insert_buffer = buffer.clone();
    let insert_edits =
        completion_buffer_edits(&insert_buffer, &item, EditorSuggestInsertMode::Insert).unwrap();
    assert!(insert_buffer.apply_edits(insert_edits));
    assert_eq!(insert_buffer.text(), "println!nt");

    let replace_edits =
        completion_buffer_edits(&buffer, &item, EditorSuggestInsertMode::Replace).unwrap();
    assert!(buffer.apply_edits(replace_edits));
    assert_eq!(buffer.text(), "println!");
}

#[test]
fn completion_buffer_edit_plan_preserves_snippet_selection_for_primary_edit() {
    let path = PathBuf::from("src/lib.rs");
    let mut buffer = TextBuffer::from_text(1, Some(path), "pri".to_owned());
    buffer.set_single_cursor(buffer.len_chars());
    let item = LspCompletionItem {
        label: "println!".to_owned(),
        detail: None,
        documentation: None,
        kind: Some(3),
        deprecated: false,
        is_snippet: true,
        sort_text: None,
        filter_text: None,
        preselect: false,
        commit_characters: Vec::new(),
        insert_text: "println!(value, other);".to_owned(),
        snippet_selection: Some(9..14),
        snippet_tabstops: vec![9..14, 16..21, 23..23],
        snippet_tabstop_groups: vec![vec![9..14], vec![16..21], vec![23..23]],
        text_edit: None,
        insert_text_edit: None,
        additional_text_edits: Vec::new(),
        resolve_payload: None,
    };

    let plan =
        completion_buffer_edit_plan(&buffer, &item, EditorSuggestInsertMode::Insert).unwrap();
    let primary = plan.primary_edit.clone().unwrap();
    assert_eq!(
        plan.snippet_tabstop_groups,
        vec![vec![9..14], vec![16..21], vec![23..23]]
    );

    let tabstops = buffer
        .apply_edits_with_inserted_selections(plan.edits, &primary, &plan.snippet_tabstops)
        .unwrap();
    assert_eq!(buffer.text(), "println!(value, other);");
    assert_eq!(tabstops, vec![9..14, 16..21, 23..23]);
    assert_eq!(
        buffer.selections(),
        &[kuroya_core::Selection {
            anchor: 9,
            cursor: 14
        }]
    );
}

#[test]
fn completion_passthrough_events_apply_text_and_close_stale_completions() {
    let mut buffer = TextBuffer::from_text(1, Some(PathBuf::from("src/lib.rs")), "pri".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 3));
    let events = vec![Event::Text("n".to_owned())];

    assert!(completion_passthrough_edit_intent(&events));
    assert!(apply_completion_passthrough_events(
        &mut buffer,
        &events,
        AutoPairSettings::default(),
    ));
    assert_eq!(buffer.text(), "prin");
}

#[test]
fn completion_passthrough_events_apply_ime_commit_only() {
    let mut buffer = TextBuffer::from_text(1, Some(PathBuf::from("src/lib.rs")), "pri".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 3));
    let events = vec![Event::Ime(ImeEvent::Commit("文".to_owned()))];

    assert!(completion_passthrough_edit_intent(&events));
    assert!(apply_completion_passthrough_events(
        &mut buffer,
        &events,
        AutoPairSettings::default(),
    ));
    assert_eq!(buffer.text(), "pri文");

    let preedit = vec![Event::Ime(ImeEvent::Preedit("wen".to_owned()))];
    assert!(!completion_passthrough_edit_intent(&preedit));
    assert!(!apply_completion_passthrough_events(
        &mut buffer,
        &preedit,
        AutoPairSettings::default(),
    ));
    assert_eq!(buffer.text(), "pri文");
}

#[test]
fn completion_passthrough_events_handle_backspace_and_ignore_popup_navigation() {
    let mut buffer =
        TextBuffer::from_text(1, Some(PathBuf::from("src/lib.rs")), "print".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 5));
    let navigation = vec![Event::Key {
        key: Key::ArrowDown,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }];
    let backspace = vec![Event::Key {
        key: Key::Backspace,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }];

    assert!(!completion_passthrough_edit_intent(&navigation));
    assert!(!apply_completion_passthrough_events(
        &mut buffer,
        &navigation,
        AutoPairSettings::default(),
    ));
    assert!(completion_passthrough_edit_intent(&backspace));
    assert!(apply_completion_passthrough_events(
        &mut buffer,
        &backspace,
        AutoPairSettings::default(),
    ));
    assert_eq!(buffer.text(), "prin");
}

#[test]
fn completion_passthrough_respects_acceptance_keys() {
    let enter = vec![Event::Key {
        key: Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }];
    let tab = vec![Event::Key {
        key: Key::Tab,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }];

    assert!(!completion_passthrough_edit_intent_with_acceptance(
        &enter, true, false
    ));
    assert!(completion_passthrough_edit_intent_with_acceptance(
        &enter, false, false
    ));
    assert!(!completion_passthrough_edit_intent_with_acceptance(
        &tab, false, true
    ));
    assert!(completion_passthrough_edit_intent_with_acceptance(
        &tab, false, false
    ));
}

#[test]
fn completion_passthrough_can_insert_enter_and_tab_when_not_accepting() {
    let mut buffer = TextBuffer::from_text(
        1,
        Some(PathBuf::from("src/lib.rs")),
        "fn main() {".to_owned(),
    );
    buffer.set_single_cursor(buffer.line_column_to_char(0, 11));
    let events = vec![
        Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        },
        Event::Key {
            key: Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        },
    ];

    assert!(apply_completion_passthrough_events_with_editor_keys(
        &mut buffer,
        &events,
        AutoPairSettings::default(),
        "  ",
        false,
    ));
    assert_eq!(buffer.text(), "fn main() {\n  ");
}

#[test]
fn completion_passthrough_ignores_enter_and_tab_without_editor_keys() {
    let mut buffer = TextBuffer::from_text(
        1,
        Some(PathBuf::from("src/lib.rs")),
        "fn main() {".to_owned(),
    );
    buffer.set_single_cursor(buffer.line_column_to_char(0, 11));
    let events = vec![
        Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        },
        Event::Key {
            key: Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        },
    ];

    assert!(!apply_completion_passthrough_events(
        &mut buffer,
        &events,
        AutoPairSettings::default(),
    ));
    assert_eq!(buffer.text(), "fn main() {");
}
