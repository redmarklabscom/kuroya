use super::*;

#[test]
fn initialize_advertises_code_action_kind_support() {
    let value = LspWireMessage::initialize(1, Path::new("workspace")).to_json();
    let code_action = &value["params"]["capabilities"]["textDocument"]["codeAction"];

    assert_eq!(code_action["dynamicRegistration"], false);
    assert_eq!(code_action["isPreferredSupport"], true);
    assert_eq!(code_action["dataSupport"], true);
    assert_eq!(code_action["resolveSupport"]["properties"], json!(["edit"]));
    assert!(
        code_action["codeActionLiteralSupport"]["codeActionKind"]["valueSet"]
            .as_array()
            .unwrap()
            .iter()
            .any(|kind| kind == "source.organizeImports")
    );
    assert!(
        code_action["codeActionLiteralSupport"]["codeActionKind"]["valueSet"]
            .as_array()
            .unwrap()
            .iter()
            .any(|kind| kind == "source.addMissingImports")
    );
}

#[test]
fn signature_help_request_uses_text_document_position_params() {
    let value = LspWireMessage::signature_help(20, Path::new("src/lib.rs"), 8, 12).to_json();

    assert_eq!(value["id"], 20);
    assert_eq!(value["method"], "textDocument/signatureHelp");
    assert_eq!(value["params"]["position"]["line"], 8);
    assert_eq!(value["params"]["position"]["character"], 12);
    assert_eq!(value["params"]["context"]["triggerKind"], 1);
}

#[test]
fn formatting_request_uses_document_uri_and_options() {
    let value = LspWireMessage::formatting(21, Path::new("src/lib.rs"), 4, true).to_json();

    assert_eq!(value["id"], 21);
    assert_eq!(value["method"], "textDocument/formatting");
    assert_eq!(value["params"]["options"]["tabSize"], 4);
    assert_eq!(value["params"]["options"]["insertSpaces"], true);
    assert!(
        value["params"]["textDocument"]["uri"]
            .as_str()
            .is_some_and(|uri| uri.starts_with("file://"))
    );
}

#[test]
fn code_action_request_uses_document_range_and_empty_diagnostics() {
    let value = LspWireMessage::code_action(23, Path::new("src/lib.rs"), 2, 4, 2, 9).to_json();

    assert_eq!(value["id"], 23);
    assert_eq!(value["method"], "textDocument/codeAction");
    assert_eq!(value["params"]["range"]["start"]["line"], 2);
    assert_eq!(value["params"]["range"]["start"]["character"], 4);
    assert_eq!(value["params"]["range"]["end"]["line"], 2);
    assert_eq!(value["params"]["range"]["end"]["character"], 9);
    assert_eq!(value["params"]["context"]["diagnostics"], json!([]));
    assert!(
        value["params"]["textDocument"]["uri"]
            .as_str()
            .is_some_and(|uri| uri.starts_with("file://"))
    );
}

#[test]
fn code_action_request_can_include_diagnostic_context() {
    let diagnostic = Diagnostic {
        path: PathBuf::from("src/lib.rs"),
        line: 3,
        column: 5,
        char_range: 8..14,
        severity: DiagnosticSeverity::Error,
        source: "rust-analyzer".to_owned(),
        message: "cannot find type `HashMap`".to_owned(),
        unused: false,
        deprecated: false,
    };
    let value = LspWireMessage::code_action_with_diagnostics(
        24,
        Path::new("src/lib.rs"),
        2,
        0,
        2,
        16,
        &[diagnostic],
    )
    .to_json();

    let diagnostic = &value["params"]["context"]["diagnostics"][0];
    assert_eq!(diagnostic["range"]["start"]["line"], 2);
    assert_eq!(diagnostic["range"]["start"]["character"], 4);
    assert_eq!(diagnostic["range"]["end"]["line"], 2);
    assert_eq!(diagnostic["range"]["end"]["character"], 10);
    assert_eq!(diagnostic["severity"], 1);
    assert_eq!(diagnostic["source"], "rust-analyzer");
    assert_eq!(diagnostic["message"], "cannot find type `HashMap`");
}

#[test]
fn code_action_resolve_request_uses_original_action_as_params() {
    let action = LspCodeAction {
        title: "Import HashMap".to_owned(),
        kind: Some("quickfix".to_owned()),
        edits: Vec::new(),
        document_changes: Vec::new(),
        resolve_payload: Some(std::sync::Arc::new(json!({
            "title": "Import HashMap",
            "kind": "quickfix",
            "data": { "id": 7 }
        }))),
    };

    let value = LspWireMessage::code_action_resolve(25, &action)
        .expect("resolve payload")
        .to_json();

    assert_eq!(value["id"], 25);
    assert_eq!(value["method"], "codeAction/resolve");
    assert_eq!(value["params"]["title"], "Import HashMap");
    assert_eq!(value["params"]["data"]["id"], 7);
}

#[test]
fn parses_signature_help_with_active_parameter_and_label_ranges() {
    let value = json!({
        "jsonrpc": "2.0",
        "id": 28,
        "result": {
            "activeSignature": 1,
            "activeParameter": 0,
            "signatures": [
                {
                    "label": "ignored()"
                },
                {
                    "label": "insert(index: usize, value: T)",
                    "documentation": {
                        "kind": "markdown",
                        "value": "Insert a value."
                    },
                    "parameters": [
                        {
                            "label": [7, 19],
                            "documentation": "Target index"
                        },
                        {
                            "label": "value: T"
                        }
                    ]
                }
            ]
        }
    });

    let help = parse_signature_help_response(&value).unwrap();
    assert_eq!(help.active_signature, 1);
    assert_eq!(help.active_parameter, Some(0));
    assert_eq!(help.signatures.len(), 2);
    let active = &help.signatures[1];
    assert_eq!(active.label, "insert(index: usize, value: T)");
    assert_eq!(active.documentation.as_deref(), Some("Insert a value."));
    assert_eq!(active.parameters[0].label, "index: usize");
    assert_eq!(
        active.parameters[0].documentation.as_deref(),
        Some("Target index")
    );
    assert_eq!(active.parameters[1].label, "value: T");
}

#[test]
fn signature_help_text_fields_are_bounded_and_bad_parameter_ranges_are_skipped() {
    let value = json!({
        "jsonrpc": "2.0",
        "id": 28,
        "result": {
            "signatures": [{
                "label": format!(
                    "{}tail",
                    "\u{03c3}".repeat(MAX_LSP_SIGNATURE_LABEL_CHARS + 4)
                ),
                "documentation": format!(
                    "{}tail",
                    "\u{03b4}".repeat(MAX_LSP_SIGNATURE_DOCUMENTATION_CHARS + 4)
                ),
                "parameters": [
                    {
                        "label": [9, 3],
                        "documentation": "bad range"
                    },
                    {
                        "label": format!(
                            "{}tail",
                            "\u{03c0}".repeat(MAX_LSP_SIGNATURE_PARAMETER_LABEL_CHARS + 4)
                        ),
                        "documentation": format!(
                            "{}tail",
                            "\u{03bc}".repeat(
                                MAX_LSP_SIGNATURE_PARAMETER_DOCUMENTATION_CHARS + 4
                            )
                        )
                    }
                ]
            }]
        }
    });

    let help = parse_signature_help_response(&value).unwrap();

    assert_eq!(help.signatures.len(), 1);
    let signature = &help.signatures[0];
    assert_eq!(
        signature.label.matches('\u{03c3}').count(),
        MAX_LSP_SIGNATURE_LABEL_CHARS
    );
    assert!(!signature.label.contains("tail"));
    let documentation = signature.documentation.as_deref().unwrap();
    assert_eq!(
        documentation.matches('\u{03b4}').count(),
        MAX_LSP_SIGNATURE_DOCUMENTATION_CHARS
    );
    assert!(!documentation.contains("tail"));
    assert_eq!(signature.parameters.len(), 1);
    assert_eq!(
        signature.parameters[0].label.matches('\u{03c0}').count(),
        MAX_LSP_SIGNATURE_PARAMETER_LABEL_CHARS
    );
    assert!(!signature.parameters[0].label.contains("tail"));
    let parameter_documentation = signature.parameters[0].documentation.as_deref().unwrap();
    assert_eq!(
        parameter_documentation.matches('\u{03bc}').count(),
        MAX_LSP_SIGNATURE_PARAMETER_DOCUMENTATION_CHARS
    );
    assert!(!parameter_documentation.contains("tail"));
}

#[test]
fn parses_formatting_text_edits() {
    let document_path = PathBuf::from("src").join("main.rs");
    let value = json!({
        "jsonrpc": "2.0",
        "id": 22,
        "result": [{
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 2, "character": 1 }
            },
            "newText": "fn main() {\n    println!(\"hi\");\n}\n"
        }]
    });

    let edits = parse_formatting_response(&value, &document_path).unwrap();
    assert_eq!(edits.len(), 1);
    assert!(edits[0].path.ends_with(Path::new("src").join("main.rs")));
    assert_eq!(edits[0].start_line, 1);
    assert_eq!(edits[0].start_column, 1);
    assert_eq!(edits[0].end_line, 3);
    assert_eq!(edits[0].end_column, 2);
    assert!(edits[0].new_text.contains("println!"));
}

#[test]
fn formatting_text_edits_reject_reversed_ranges() {
    let document_path = PathBuf::from("src").join("main.rs");
    let value = json!({
        "jsonrpc": "2.0",
        "id": 22,
        "result": [{
            "range": {
                "start": { "line": 2, "character": 4 },
                "end": { "line": 2, "character": 1 }
            },
            "newText": "x"
        }]
    });

    assert!(parse_formatting_response(&value, &document_path).is_none());
}

#[test]
fn parses_edit_backed_and_resolvable_code_actions() {
    let uri = path_to_file_uri(Path::new("src/main.rs"));
    let value = json!({
        "jsonrpc": "2.0",
        "id": 24,
        "result": [
            {
                "title": "Import HashMap",
                "kind": "quickfix",
                "edit": {
                    "changes": {
                        uri.clone(): [{
                            "range": {
                                "start": { "line": 0, "character": 0 },
                                "end": { "line": 0, "character": 0 }
                            },
                            "newText": "use std::collections::HashMap;\n"
                        }]
                    }
                }
            },
            {
                "title": "Resolve import",
                "kind": "quickfix",
                "data": { "id": 7 }
            },
            {
                "title": "Run command",
                "command": { "command": "example.command", "title": "Run command" }
            },
            {
                "title": "Disabled",
                "disabled": { "reason": "not available" },
                "edit": {
                    "changes": {
                        uri: []
                    }
                }
            }
        ]
    });

    let actions = parse_code_action_response(&value).unwrap();
    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0].title, "Import HashMap");
    assert_eq!(actions[0].kind.as_deref(), Some("quickfix"));
    assert_eq!(actions[0].edits.len(), 1);
    assert!(!actions[0].needs_resolve());
    assert!(
        actions[0].edits[0]
            .path
            .ends_with(Path::new("src").join("main.rs"))
    );
    assert_eq!(actions[0].edits[0].start_line, 1);
    assert!(actions[0].edits[0].new_text.contains("HashMap"));
    assert_eq!(actions[1].title, "Resolve import");
    assert!(actions[1].edits.is_empty());
    assert!(actions[1].needs_resolve());
    assert_eq!(
        actions[1]
            .resolve_payload
            .as_deref()
            .and_then(|payload| payload.get("data"))
            .and_then(|data| data.get("id")),
        Some(&json!(7))
    );
}

#[test]
fn code_action_response_prefers_document_changes_over_changes() {
    let ignored_uri = path_to_file_uri(Path::new("src/ignored.rs"));
    let used_uri = path_to_file_uri(Path::new("src/main.rs"));
    let value = json!({
        "jsonrpc": "2.0",
        "id": 24,
        "result": [{
            "title": "Apply edit",
            "kind": "quickfix",
            "edit": {
                "changes": {
                    ignored_uri: [{
                        "range": {
                            "start": { "line": 0, "character": 0 },
                            "end": { "line": 0, "character": 0 }
                        },
                        "newText": "ignored\n"
                    }]
                },
                "documentChanges": [{
                    "textDocument": { "uri": used_uri, "version": 3 },
                    "edits": [{
                        "range": {
                            "start": { "line": 1, "character": 0 },
                            "end": { "line": 1, "character": 0 }
                        },
                        "newText": "used\n"
                    }]
                }]
            }
        }]
    });

    let actions = parse_code_action_response(&value).unwrap();

    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].edits.len(), 1);
    assert!(
        actions[0].edits[0]
            .path
            .ends_with(Path::new("src").join("main.rs"))
    );
    assert_eq!(actions[0].edits[0].new_text, "used\n");
}

#[test]
fn code_action_response_preserves_resource_workspace_edits() {
    let uri = path_to_file_uri(Path::new("src/new.rs"));
    let value = json!({
        "jsonrpc": "2.0",
        "id": 24,
        "result": [{
            "title": "Create file",
            "kind": "quickfix",
            "edit": {
                "documentChanges": [{
                    "kind": "create",
                    "uri": uri
                }]
            }
        }]
    });

    let actions = parse_code_action_response(&value).unwrap();

    assert_eq!(actions.len(), 1);
    assert!(actions[0].edits.is_empty());
    assert!(matches!(
        &actions[0].document_changes[0],
        LspWorkspaceDocumentChange::Resource(LspWorkspaceResourceOperation::CreateFile { .. })
    ));
    assert!(!actions[0].needs_resolve());
}

#[test]
fn code_action_response_is_bounded() {
    let uri = path_to_file_uri(Path::new("src/main.rs"));
    let actions = (0..MAX_LSP_CODE_ACTIONS + 12)
        .map(|idx| {
            json!({
                "title": format!("Action {idx}"),
                "kind": "quickfix",
                "edit": {
                    "changes": {
                        uri.clone(): [{
                            "range": {
                                "start": { "line": idx, "character": 0 },
                                "end": { "line": idx, "character": 0 }
                            },
                            "newText": format!("// action {idx}\n")
                        }]
                    }
                }
            })
        })
        .collect::<Vec<_>>();
    let value = json!({
        "jsonrpc": "2.0",
        "id": 24,
        "result": actions
    });

    let actions = parse_code_action_response(&value).unwrap();

    assert_eq!(actions.len(), MAX_LSP_CODE_ACTIONS);
    assert_eq!(
        actions.last().map(|action| action.title.as_str()),
        Some("Action 99")
    );
}

#[test]
fn code_action_resolve_response_parses_single_resolved_edit() {
    let uri = path_to_file_uri(Path::new("src/main.rs"));
    let value = json!({
        "jsonrpc": "2.0",
        "id": 24,
        "result": {
            "title": "Import HashMap",
            "kind": "quickfix",
            "data": { "id": 7 },
            "edit": {
                "changes": {
                    uri: [{
                        "range": {
                            "start": { "line": 0, "character": 0 },
                            "end": { "line": 0, "character": 0 }
                        },
                        "newText": "use std::collections::HashMap;\n"
                    }]
                }
            }
        }
    });

    let action = parse_code_action_resolve_response(&value).unwrap();

    assert_eq!(action.title, "Import HashMap");
    assert_eq!(action.edits.len(), 1);
    assert!(!action.needs_resolve());
    assert!(action.edits[0].new_text.contains("HashMap"));
}

#[test]
fn code_action_resolve_response_preserves_resource_workspace_edits() {
    let uri = path_to_file_uri(Path::new("src/new.rs"));
    let value = json!({
        "jsonrpc": "2.0",
        "id": 24,
        "result": {
            "title": "Create file",
            "kind": "quickfix",
            "data": { "id": 7 },
            "edit": {
                "documentChanges": [{
                    "kind": "create",
                    "uri": uri
                }]
            }
        }
    });

    let action = parse_code_action_resolve_response(&value).unwrap();

    assert!(action.edits.is_empty());
    assert_eq!(action.document_changes.len(), 1);
    assert!(matches!(
        &action.document_changes[0],
        LspWorkspaceDocumentChange::Resource(LspWorkspaceResourceOperation::CreateFile { .. })
    ));
    assert!(!action.needs_resolve());
}

#[test]
fn code_action_resolve_response_rejects_command_only_result() {
    let value = json!({
        "jsonrpc": "2.0",
        "id": 24,
        "result": {
            "title": "Run command",
            "command": { "command": "example.command", "title": "Run command" },
            "data": { "id": 7 }
        }
    });

    assert!(parse_code_action_resolve_response(&value).is_none());
}

#[test]
fn code_action_title_and_kind_are_sanitized_and_bounded() {
    let uri = path_to_file_uri(Path::new("src/main.rs"));
    let value = json!({
        "jsonrpc": "2.0",
        "id": 24,
        "result": [
            {
                "title": format!(
                    "  {}\ntail",
                    "\u{03b1}".repeat(MAX_LSP_CODE_ACTION_TITLE_CHARS + 4)
                ),
                "kind": format!(
                    "{}tail",
                    "\u{03b2}".repeat(MAX_LSP_CODE_ACTION_KIND_CHARS + 4)
                ),
                "edit": {
                    "changes": {
                        uri.clone(): [{
                            "range": {
                                "start": { "line": 0, "character": 0 },
                                "end": { "line": 0, "character": 0 }
                            },
                            "newText": "use std::collections::HashMap;\n"
                        }]
                    }
                }
            },
            {
                "title": " \n\t ",
                "kind": "quickfix",
                "edit": {
                    "changes": {
                        uri: [{
                            "range": {
                                "start": { "line": 0, "character": 0 },
                                "end": { "line": 0, "character": 0 }
                            },
                            "newText": "use std::collections::BTreeMap;\n"
                        }]
                    }
                }
            }
        ]
    });

    let actions = parse_code_action_response(&value).unwrap();

    assert_eq!(actions.len(), 1);
    assert_eq!(
        actions[0].title.matches('\u{03b1}').count(),
        MAX_LSP_CODE_ACTION_TITLE_CHARS
    );
    assert!(!actions[0].title.contains("tail"));
    assert!(!actions[0].title.contains('\n'));
    let kind = actions[0].kind.as_deref().unwrap();
    assert_eq!(
        kind.matches('\u{03b2}').count(),
        MAX_LSP_CODE_ACTION_KIND_CHARS
    );
    assert!(!kind.contains("tail"));
}
