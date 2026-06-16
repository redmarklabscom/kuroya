use super::*;

#[test]
fn initialize_advertises_completion_snippet_support() {
    let value = LspWireMessage::initialize(1, Path::new("workspace")).to_json();

    assert_eq!(
        value["params"]["capabilities"]["textDocument"]["completion"]["completionItem"]["snippetSupport"],
        true
    );
    assert_eq!(
        value["params"]["capabilities"]["textDocument"]["completion"]["completionItem"]["documentationFormat"],
        json!(["markdown", "plaintext"])
    );
    assert_eq!(
        value["params"]["capabilities"]["textDocument"]["completion"]["completionItem"]["commitCharactersSupport"],
        true
    );
    assert_eq!(
        value["params"]["capabilities"]["textDocument"]["completion"]["completionItem"]["insertReplaceSupport"],
        true
    );
    assert_eq!(
        value["params"]["capabilities"]["textDocument"]["completion"]["completionItem"]["dataSupport"],
        true
    );
    assert_eq!(
        value["params"]["capabilities"]["textDocument"]["completion"]["completionItem"]["resolveSupport"]
            ["properties"],
        json!(["detail", "documentation", "additionalTextEdits"])
    );
    assert_eq!(
        value["params"]["capabilities"]["textDocument"]["completion"]["completionList"]["itemDefaults"],
        json!(["commitCharacters", "editRange", "insertTextFormat"])
    );
    assert_eq!(
        value["params"]["capabilities"]["textDocument"]["hover"]["contentFormat"],
        json!(["markdown", "plaintext"])
    );
}

#[test]
fn completion_request_uses_text_document_position_params() {
    let value = LspWireMessage::completion(19, Path::new("src/lib.rs"), 6, 4).to_json();

    assert_eq!(value["id"], 19);
    assert_eq!(value["method"], "textDocument/completion");
    assert_eq!(value["params"]["position"]["line"], 6);
    assert_eq!(value["params"]["position"]["character"], 4);
    assert_eq!(value["params"]["context"]["triggerKind"], 1);
}

#[test]
fn completion_item_resolve_request_uses_original_item_as_params() {
    let item = LspCompletionItem {
        label: "HashMap".to_owned(),
        detail: None,
        documentation: None,
        kind: None,
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
        additional_text_edits: Vec::new(),
        resolve_payload: Some(std::sync::Arc::new(json!({
            "label": "HashMap",
            "data": { "id": 7 }
        }))),
    };

    let value = LspWireMessage::completion_item_resolve(26, &item)
        .expect("resolve payload")
        .to_json();

    assert_eq!(value["id"], 26);
    assert_eq!(value["method"], "completionItem/resolve");
    assert_eq!(value["params"]["label"], "HashMap");
    assert_eq!(value["params"]["data"]["id"], 7);
}

#[test]
fn parses_completion_lists_and_text_edits() {
    let document_path = PathBuf::from("src").join("main.rs");
    let value = json!({
        "jsonrpc": "2.0",
        "id": 20,
        "result": {
            "isIncomplete": false,
            "items": [
                {
                    "label": "println!",
                    "kind": 3,
                    "detail": "macro",
                    "sortText": "0002",
                    "filterText": "println",
                    "preselect": true,
                    "commitCharacters": [".", "("],
                    "tags": [1],
                    "documentation": {
                        "kind": "markdown",
                        "value": "Prints to stdout."
                    },
                    "insertText": "println!"
                },
                {
                    "label": "print",
                    "kind": 2,
                    "documentation": "Print without newline.",
                    "textEdit": {
                        "range": {
                            "start": { "line": 4, "character": 8 },
                            "end": { "line": 4, "character": 11 }
                        },
                        "newText": "print"
                    },
                    "additionalTextEdits": [
                        {
                            "range": {
                                "start": { "line": 0, "character": 0 },
                                "end": { "line": 0, "character": 0 }
                            },
                            "newText": "use std::fmt;\n"
                        }
                    ]
                }
            ]
        }
    });

    let items = parse_completion_response(&value, &document_path).unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].label, "println!");
    assert_eq!(items[0].detail.as_deref(), Some("macro"));
    assert_eq!(items[0].documentation.as_deref(), Some("Prints to stdout."));
    assert_eq!(items[0].sort_text.as_deref(), Some("0002"));
    assert_eq!(items[0].filter_text.as_deref(), Some("println"));
    assert!(items[0].preselect);
    assert!(items[0].deprecated);
    assert!(!items[0].is_snippet);
    assert_eq!(items[0].commit_characters, vec![".", "("]);
    assert_eq!(items[0].insert_text, "println!");
    assert!(items[0].text_edit.is_none());
    assert!(items[0].insert_text_edit.is_none());
    assert_eq!(items[1].insert_text, "print");
    assert_eq!(
        items[1].documentation.as_deref(),
        Some("Print without newline.")
    );
    let edit = items[1].text_edit.as_ref().unwrap();
    assert_eq!(edit.start_line, 5);
    assert_eq!(edit.start_column, 9);
    assert_eq!(edit.end_column, 12);
    assert_eq!(edit.new_text, "print");
    assert_eq!(items[1].additional_text_edits.len(), 1);
    assert_eq!(items[1].additional_text_edits[0].start_line, 1);
    assert_eq!(items[1].additional_text_edits[0].start_column, 1);
    assert_eq!(
        items[1].additional_text_edits[0].new_text,
        "use std::fmt;\n"
    );
}

#[test]
fn completion_label_details_fill_bounded_detail_when_detail_is_absent() {
    let document_path = PathBuf::from("src").join("main.rs");
    let value = json!({
        "jsonrpc": "2.0",
        "id": 20,
        "result": [
            {
                "label": "collect",
                "labelDetails": {
                    "detail": "::<Vec<_>>()",
                    "description": "std::iter::Iterator"
                }
            },
            {
                "label": "explicit",
                "detail": "explicit detail",
                "labelDetails": {
                    "detail": "ignored detail",
                    "description": "ignored description"
                }
            },
            {
                "label": "bounded",
                "labelDetails": {
                    "detail": format!(
                        "{}tail",
                        "\u{03b4}".repeat(MAX_LSP_COMPLETION_DETAIL_CHARS + 4)
                    ),
                    "description": "ignored"
                }
            }
        ]
    });

    let items = parse_completion_response(&value, &document_path).unwrap();

    assert_eq!(items.len(), 3);
    assert_eq!(
        items[0].detail.as_deref(),
        Some("::<Vec<_>>() std::iter::Iterator")
    );
    assert_eq!(items[1].detail.as_deref(), Some("explicit detail"));
    let detail = items[2].detail.as_deref().unwrap();
    assert_eq!(
        detail.matches('\u{03b4}').count(),
        MAX_LSP_COMPLETION_DETAIL_CHARS
    );
    assert!(!detail.contains("tail"));
    assert!(!detail.contains("ignored"));
}

#[test]
fn completion_items_with_data_retain_bounded_resolve_payload() {
    let document_path = PathBuf::from("src").join("main.rs");
    let value = json!({
        "jsonrpc": "2.0",
        "id": 20,
        "result": [{
            "label": "HashMap",
            "insertText": "HashMap",
            "data": { "id": 7 }
        }]
    });

    let items = parse_completion_response(&value, &document_path).unwrap();

    assert_eq!(items.len(), 1);
    assert!(items[0].needs_resolve());
    assert_eq!(
        items[0]
            .resolve_payload
            .as_ref()
            .unwrap()
            .get("data")
            .and_then(|data| data.get("id")),
        Some(&json!(7))
    );
}

#[test]
fn oversized_completion_resolve_payload_keeps_item_without_resolve() {
    let document_path = PathBuf::from("src").join("main.rs");
    let value = json!({
        "jsonrpc": "2.0",
        "id": 20,
        "result": [{
            "label": "HashMap",
            "insertText": "HashMap",
            "data": { "blob": "x".repeat(MAX_LSP_COMPLETION_RESOLVE_PAYLOAD_BYTES + 1) }
        }]
    });

    let items = parse_completion_response(&value, &document_path).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "HashMap");
    assert!(!items[0].needs_resolve());
}

#[test]
fn completion_item_resolve_response_uses_label_details_over_original_detail() {
    let document_path = PathBuf::from("src").join("main.rs");
    let original_value = json!({
        "jsonrpc": "2.0",
        "id": 20,
        "result": [{
            "label": "HashMap",
            "detail": "old detail",
            "insertText": "HashMap",
            "data": { "id": 7 }
        }]
    });
    let mut original_items = parse_completion_response(&original_value, &document_path).unwrap();
    let original = original_items.pop().unwrap();
    let value = json!({
        "jsonrpc": "2.0",
        "id": 21,
        "result": {
            "label": "HashMap",
            "labelDetails": {
                "detail": "<K, V>",
                "description": "std::collections"
            },
            "data": { "id": 7 }
        }
    });

    let item = parse_completion_item_resolve_response(&value, &document_path, &original)
        .expect("resolved completion item");

    assert_eq!(item.detail.as_deref(), Some("<K, V> std::collections"));
    assert_eq!(item.insert_text, "HashMap");
    assert!(!item.needs_resolve());
}

#[test]
fn completion_item_resolve_response_merges_lazy_fields_and_preserves_insert_edit() {
    let document_path = PathBuf::from("src").join("main.rs");
    let original_value = json!({
        "jsonrpc": "2.0",
        "id": 20,
        "result": [{
            "label": "HashMap",
            "textEdit": {
                "range": {
                    "start": { "line": 4, "character": 8 },
                    "end": { "line": 4, "character": 12 }
                },
                "newText": "HashMap"
            },
            "data": { "id": 7 }
        }]
    });
    let mut original_items = parse_completion_response(&original_value, &document_path).unwrap();
    let original = original_items.pop().unwrap();
    let value = json!({
        "jsonrpc": "2.0",
        "id": 21,
        "result": {
            "label": "HashMap",
            "detail": "struct HashMap",
            "documentation": {
                "kind": "markdown",
                "value": "A hash map."
            },
            "additionalTextEdits": [{
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 0, "character": 0 }
                },
                "newText": "use std::collections::HashMap;\n"
            }],
            "data": { "id": 7 }
        }
    });

    let item = parse_completion_item_resolve_response(&value, &document_path, &original)
        .expect("resolved completion item");

    assert_eq!(item.label, "HashMap");
    assert_eq!(item.detail.as_deref(), Some("struct HashMap"));
    assert_eq!(item.documentation.as_deref(), Some("A hash map."));
    assert_eq!(item.insert_text, "HashMap");
    let edit = item.text_edit.as_ref().expect("original text edit");
    assert_eq!(edit.start_line, 5);
    assert_eq!(edit.start_column, 9);
    assert_eq!(edit.end_column, 13);
    assert_eq!(item.additional_text_edits.len(), 1);
    assert_eq!(
        item.additional_text_edits[0].new_text,
        "use std::collections::HashMap;\n"
    );
    assert!(!item.needs_resolve());
}

#[test]
fn completion_items_with_invalid_or_oversized_additional_text_edits_are_rejected() {
    let document_path = PathBuf::from("src").join("main.rs");
    let huge = "x".repeat(MAX_LSP_TEXT_EDIT_NEW_TEXT_BYTES + 1);
    let value = json!({
        "jsonrpc": "2.0",
        "id": 20,
        "result": [
            {
                "label": "side-edits-not-array",
                "insertText": "bad",
                "additionalTextEdits": {}
            },
            {
                "label": "malformed-side-edit",
                "insertText": "bad",
                "additionalTextEdits": [{
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 0, "character": 0 }
                    }
                }]
            },
            {
                "label": "huge-side-edit",
                "insertText": "bad",
                "additionalTextEdits": [{
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 0, "character": 0 }
                    },
                    "newText": huge
                }]
            },
            {
                "label": "valid",
                "insertText": "ok",
                "additionalTextEdits": [{
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 0, "character": 0 }
                    },
                    "newText": "use std::fmt;\n"
                }]
            }
        ]
    });

    let items = parse_completion_response(&value, &document_path).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "valid");
    assert_eq!(items[0].insert_text, "ok");
    assert_eq!(items[0].additional_text_edits.len(), 1);
    assert_eq!(
        items[0].additional_text_edits[0].new_text,
        "use std::fmt;\n"
    );
}

#[test]
fn completion_response_is_bounded() {
    let document_path = PathBuf::from("src").join("main.rs");
    let items = (0..MAX_LSP_COMPLETION_ITEMS + 12)
        .map(|idx| {
            json!({
                "label": format!("item{idx}"),
                "insertText": format!("item{idx}")
            })
        })
        .collect::<Vec<_>>();
    let value = json!({
        "jsonrpc": "2.0",
        "id": 20,
        "result": items
    });

    let items = parse_completion_response(&value, &document_path).unwrap();

    assert_eq!(items.len(), MAX_LSP_COMPLETION_ITEMS);
    assert_eq!(
        items.last().map(|item| item.label.as_str()),
        Some("item199")
    );
}

#[test]
fn completion_response_does_not_search_past_item_bound() {
    let document_path = PathBuf::from("src").join("main.rs");
    let mut items = (0..MAX_LSP_COMPLETION_ITEMS)
        .map(|_| json!({ "label": " \n\t " }))
        .collect::<Vec<_>>();
    items.push(json!({
        "label": "valid-after-bound",
        "insertText": "valid-after-bound"
    }));
    let value = json!({
        "jsonrpc": "2.0",
        "id": 20,
        "result": items
    });

    let items = parse_completion_response(&value, &document_path).unwrap();

    assert!(items.is_empty());
}

#[test]
fn completion_metadata_is_sanitized_and_bounded() {
    let document_path = PathBuf::from("src").join("main.rs");
    let value = json!({
        "jsonrpc": "2.0",
        "id": 20,
        "result": [{
            "label": format!(
                "  {}\ntail",
                "\u{03bb}".repeat(MAX_LSP_COMPLETION_LABEL_CHARS + 4)
            ),
            "detail": format!(
                "{}tail",
                "\u{03b4}".repeat(MAX_LSP_COMPLETION_DETAIL_CHARS + 4)
            ),
            "documentation": format!(
                "Summary\n\n{}tail",
                "\u{03bc}".repeat(MAX_LSP_COMPLETION_DOCUMENTATION_CHARS + 4)
            ),
            "sortText": format!(
                "{}tail",
                "\u{03c3}".repeat(MAX_LSP_COMPLETION_SORT_TEXT_CHARS + 4)
            ),
            "filterText": format!(
                "{}tail",
                "\u{03c6}".repeat(MAX_LSP_COMPLETION_FILTER_TEXT_CHARS + 4)
            ),
            "commitCharacters": [
                format!("{}tail", "\u{03c7}".repeat(MAX_LSP_COMPLETION_COMMIT_CHARACTER_CHARS + 4)),
                " \n\t ",
                ".", "(", ")", "[", "]", "{", "}", ";", ":", ",", "<", ">", "/", "\\", "extra"
            ],
            "insertText": "value"
        }]
    });

    let items = parse_completion_response(&value, &document_path).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].label.matches('\u{03bb}').count(),
        MAX_LSP_COMPLETION_LABEL_CHARS
    );
    assert!(!items[0].label.contains("tail"));
    assert!(!items[0].label.contains('\n'));
    assert_eq!(
        items[0]
            .detail
            .as_deref()
            .unwrap()
            .matches('\u{03b4}')
            .count(),
        MAX_LSP_COMPLETION_DETAIL_CHARS
    );
    let documentation = items[0].documentation.as_deref().unwrap();
    assert!(documentation.starts_with("Summary\n\n"));
    assert!(documentation.chars().count() <= MAX_LSP_COMPLETION_DOCUMENTATION_CHARS);
    assert!(!documentation.contains("tail"));
    assert_eq!(
        items[0]
            .sort_text
            .as_deref()
            .unwrap()
            .matches('\u{03c3}')
            .count(),
        MAX_LSP_COMPLETION_SORT_TEXT_CHARS
    );
    assert_eq!(
        items[0]
            .filter_text
            .as_deref()
            .unwrap()
            .matches('\u{03c6}')
            .count(),
        MAX_LSP_COMPLETION_FILTER_TEXT_CHARS
    );
    assert_eq!(
        items[0].commit_characters.len(),
        MAX_LSP_COMPLETION_COMMIT_CHARACTERS
    );
    assert_eq!(
        items[0].commit_characters[0].matches('\u{03c7}').count(),
        MAX_LSP_COMPLETION_COMMIT_CHARACTER_CHARS
    );
    assert!(!items[0].commit_characters[0].contains("tail"));
    assert!(
        !items[0]
            .commit_characters
            .iter()
            .any(|text| text.is_empty())
    );
}

#[test]
fn oversized_completion_insert_payloads_are_rejected() {
    let document_path = PathBuf::from("src").join("main.rs");
    let huge = "x".repeat(MAX_LSP_TEXT_EDIT_NEW_TEXT_BYTES + 1);
    let value = json!({
        "jsonrpc": "2.0",
        "id": 20,
        "result": [
            {
                "label": "huge-insert-text",
                "insertText": huge
            },
            {
                "label": "huge-text-edit",
                "textEdit": {
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 0, "character": 0 }
                    },
                    "newText": "x".repeat(MAX_LSP_TEXT_EDIT_NEW_TEXT_BYTES + 1)
                }
            },
            {
                "label": "small",
                "insertText": "small"
            }
        ]
    });

    let items = parse_completion_response(&value, &document_path).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "small");
    assert_eq!(items[0].insert_text, "small");
}

#[test]
fn parses_completion_insert_replace_text_edits() {
    let document_path = PathBuf::from("src").join("main.rs");
    let value = json!({
        "jsonrpc": "2.0",
        "id": 20,
        "result": [{
            "label": "println!",
            "textEdit": {
                "newText": "println!",
                "insert": {
                    "start": { "line": 4, "character": 8 },
                    "end": { "line": 4, "character": 11 }
                },
                "replace": {
                    "start": { "line": 4, "character": 8 },
                    "end": { "line": 4, "character": 13 }
                }
            }
        }]
    });

    let items = parse_completion_response(&value, &document_path).unwrap();

    let replace_edit = items[0].text_edit.as_ref().unwrap();
    assert_eq!(replace_edit.start_line, 5);
    assert_eq!(replace_edit.start_column, 9);
    assert_eq!(replace_edit.end_column, 14);
    assert_eq!(replace_edit.new_text, "println!");

    let insert_edit = items[0].insert_text_edit.as_ref().unwrap();
    assert_eq!(insert_edit.start_line, 5);
    assert_eq!(insert_edit.start_column, 9);
    assert_eq!(insert_edit.end_column, 12);
    assert_eq!(insert_edit.new_text, "println!");
}

#[test]
fn parses_completion_item_defaults_for_edit_range_and_snippets() {
    let document_path = PathBuf::from("src").join("main.rs");
    let value = json!({
        "jsonrpc": "2.0",
        "id": 20,
        "result": {
            "isIncomplete": false,
            "itemDefaults": {
                "commitCharacters": ["."],
                "insertTextFormat": 2,
                "editRange": {
                    "insert": {
                        "start": { "line": 4, "character": 8 },
                        "end": { "line": 4, "character": 11 }
                    },
                    "replace": {
                        "start": { "line": 4, "character": 8 },
                        "end": { "line": 4, "character": 13 }
                    }
                }
            },
            "items": [{
                "label": "Some",
                "textEditText": "Some(${1:value})$0"
            }]
        }
    });

    let items = parse_completion_response(&value, &document_path).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Some");
    assert_eq!(items[0].insert_text, "Some(value)");
    assert_eq!(items[0].commit_characters, vec!["."]);
    assert!(items[0].is_snippet);
    assert_eq!(items[0].snippet_selection, Some(5..10));
    assert_eq!(items[0].snippet_tabstops, vec![5..10, 11..11]);

    let replace_edit = items[0].text_edit.as_ref().unwrap();
    assert_eq!(replace_edit.start_line, 5);
    assert_eq!(replace_edit.start_column, 9);
    assert_eq!(replace_edit.end_column, 14);
    assert_eq!(replace_edit.new_text, "Some(value)");

    let insert_edit = items[0].insert_text_edit.as_ref().unwrap();
    assert_eq!(insert_edit.start_line, 5);
    assert_eq!(insert_edit.start_column, 9);
    assert_eq!(insert_edit.end_column, 12);
    assert_eq!(insert_edit.new_text, "Some(value)");
}

#[test]
fn completion_item_default_edit_range_uses_label_when_text_edit_text_is_absent() {
    let document_path = PathBuf::from("src").join("main.rs");
    let value = json!({
        "jsonrpc": "2.0",
        "id": 20,
        "result": {
            "isIncomplete": false,
            "itemDefaults": {
                "editRange": {
                    "start": { "line": 4, "character": 8 },
                    "end": { "line": 4, "character": 13 }
                }
            },
            "items": [{
                "label": "display",
                "insertText": "actual"
            }]
        }
    });

    let items = parse_completion_response(&value, &document_path).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].insert_text, "display");
    let edit = items[0].text_edit.as_ref().unwrap();
    assert_eq!(edit.new_text, "display");
    assert_eq!(edit.start_line, 5);
    assert_eq!(edit.start_column, 9);
    assert_eq!(edit.end_column, 14);
}

#[test]
fn parses_snippet_completion_insert_text_as_plain_text() {
    let document_path = PathBuf::from("src").join("main.rs");
    let value = json!({
        "jsonrpc": "2.0",
        "id": 20,
        "result": [{
            "label": "println!",
            "insertTextFormat": 2,
            "insertText": "println!(\"${1:value}\");$0"
        }]
    });

    let items = parse_completion_response(&value, &document_path).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].insert_text, "println!(\"value\");");
    assert_eq!(items[0].snippet_selection, Some(10..15));
    assert_eq!(items[0].snippet_tabstops, vec![10..15, 18..18]);
    assert!(items[0].is_snippet);
    assert!(items[0].text_edit.is_none());
}

#[test]
fn parses_snippet_completion_text_edits_as_plain_text() {
    let document_path = PathBuf::from("src").join("main.rs");
    let value = json!({
        "jsonrpc": "2.0",
        "id": 20,
        "result": [{
            "label": "Some",
            "insertTextFormat": 2,
            "textEdit": {
                "range": {
                    "start": { "line": 4, "character": 8 },
                    "end": { "line": 4, "character": 12 }
                },
                "newText": "Some(${1:inner})$0"
            },
            "additionalTextEdits": [{
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 0, "character": 0 }
                },
                "newText": "${1:not_a_snippet}\n"
            }]
        }]
    });

    let items = parse_completion_response(&value, &document_path).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].insert_text, "Some(inner)");
    assert_eq!(items[0].snippet_selection, Some(5..10));
    assert_eq!(items[0].snippet_tabstops, vec![5..10, 11..11]);
    let edit = items[0].text_edit.as_ref().unwrap();
    assert_eq!(edit.new_text, "Some(inner)");
    assert_eq!(
        items[0].additional_text_edits[0].new_text,
        "${1:not_a_snippet}\n"
    );
}

#[test]
fn expands_nested_choices_and_escaped_snippet_syntax() {
    let nested = expand_lsp_completion_snippet("${1:Vec<${2:T}>}::${3|new,default|}($0)").unwrap();
    assert_eq!(nested.text, "Vec<T>::new()");
    assert_eq!(nested.selection, Some(0..6));
    assert_eq!(nested.tabstops, vec![0..6, 4..5, 8..11, 12..12]);
    assert_eq!(
        nested.tabstop_groups,
        vec![vec![0..6], vec![4..5], vec![8..11], vec![12..12]]
    );

    let escaped = expand_lsp_completion_snippet(r"\$${1:value}\} \\").unwrap();
    assert_eq!(escaped.text, "$value} \\");
    assert_eq!(escaped.selection, Some(1..6));
    assert_eq!(escaped.tabstops, vec![1..6]);
    assert_eq!(escaped.tabstop_groups, vec![vec![1..6]]);
}

#[test]
fn expands_regex_and_path_snippets_with_non_special_escapes() {
    let regex = expand_lsp_completion_snippet(r#"${1:^\d+\.\w+$}"#).unwrap();
    assert_eq!(regex.text, r#"^\d+\.\w+$"#);
    assert_eq!(regex.selection, Some(0..10));
    assert_eq!(regex.tabstops, vec![0..10]);

    let path =
        expand_lsp_completion_snippet(r"${1|C:\tmp\logs,D:\src\app|}\\${2:file.txt}").unwrap();
    assert_eq!(path.text, r"C:\tmp\logs\file.txt");
    assert_eq!(path.selection, Some(0..11));
    assert_eq!(path.tabstops, vec![0..11, 12..20]);
}

#[test]
fn oversized_snippet_expansions_are_rejected() {
    let document_path = PathBuf::from("src").join("main.rs");
    let repeated_value = "x".repeat(MAX_SNIPPET_EXPANSION_BYTES / 2 + 1);
    let repeated = format!("${{1:{}}}${{1}}", repeated_value);
    let plain = "x".repeat(MAX_SNIPPET_EXPANSION_BYTES + 1);
    let value = json!({
        "jsonrpc": "2.0",
        "id": 20,
        "result": [
            {
                "label": "huge-plain-snippet",
                "insertTextFormat": 2,
                "insertText": plain
            },
            {
                "label": "huge-expanded-snippet",
                "insertTextFormat": 2,
                "insertText": repeated
            },
            {
                "label": "small",
                "insertTextFormat": 2,
                "insertText": "${1:ok}$0"
            }
        ]
    });

    let items = parse_completion_response(&value, &document_path).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "small");
    assert_eq!(items[0].insert_text, "ok");
}

#[test]
fn oversized_snippet_sources_are_rejected_even_with_small_expansions() {
    let snippet = "${TM_FILENAME}".repeat(MAX_SNIPPET_SOURCE_BYTES / "${TM_FILENAME}".len() + 1);

    assert!(expand_lsp_completion_snippet(&snippet).is_none());
}

#[test]
fn excessive_snippet_tabstops_are_rejected() {
    let snippet = (1..=MAX_SNIPPET_TABSTOPS + 1)
        .map(|tabstop| format!("${{{tabstop}:x}}"))
        .collect::<String>();

    assert!(expand_lsp_completion_snippet(&snippet).is_none());
}

#[test]
fn overflowing_repeated_placeholder_defaults_reject_snippet_expansion() {
    let default = "x".repeat(MAX_SNIPPET_EXPANSION_BYTES + 1);
    let snippet = format!("${{1:ok}}${{1:{default}}}$0");

    assert!(expand_lsp_completion_snippet(&snippet).is_none());
}

#[test]
fn snippet_expansion_groups_linked_tabstops_with_initial_values() {
    let expansion =
        expand_lsp_completion_snippet("${1:value} = ${1}; ${2|Some,None|} ${2} $0").unwrap();

    assert_eq!(expansion.text, "value = value; Some Some ");
    assert_eq!(expansion.selection, Some(0..5));
    assert_eq!(expansion.tabstops, vec![0..5, 15..19, 25..25]);
    assert_eq!(
        expansion.tabstop_groups,
        vec![vec![0..5, 8..13], vec![15..19, 20..24], vec![25..25]]
    );

    let repeated_placeholder = expand_lsp_completion_snippet("${1:foo}/${1:bar}").unwrap();
    assert_eq!(repeated_placeholder.text, "foo/foo");
    assert_eq!(repeated_placeholder.tabstop_groups, vec![vec![0..3, 4..7]]);
}

#[test]
fn snippet_expansion_uses_final_tabstop_when_no_placeholder_selection() {
    let expansion = expand_lsp_completion_snippet("println!($0);").unwrap();
    assert_eq!(expansion.text, "println!();");
    assert_eq!(expansion.selection, Some(9..9));
    assert_eq!(expansion.tabstops, vec![9..9]);
}
