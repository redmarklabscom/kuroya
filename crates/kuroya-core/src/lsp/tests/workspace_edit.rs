use super::*;

#[test]
fn initialize_advertises_workspace_apply_edit() {
    let value = LspWireMessage::initialize(1, Path::new("workspace")).to_json();

    assert_eq!(
        value["params"]["capabilities"]["workspace"]["applyEdit"],
        true
    );
}

#[test]
fn workspace_apply_edit_response_serializes_apply_result() {
    let success =
        LspWireMessage::apply_workspace_edit_response(17, true, Some("ignored"), Some(2)).to_json();

    assert_eq!(success["jsonrpc"], "2.0");
    assert_eq!(success["id"], 17);
    assert_eq!(success["result"]["applied"], true);
    assert!(success["result"].get("failureReason").is_none());
    assert!(success["result"].get("failedChange").is_none());

    let failure =
        LspWireMessage::apply_workspace_edit_response(18, false, Some("buffer changed"), Some(1))
            .to_json();

    assert_eq!(failure["id"], 18);
    assert_eq!(failure["result"]["applied"], false);
    assert_eq!(failure["result"]["failureReason"], "buffer changed");
    assert_eq!(failure["result"]["failedChange"], 1);

    let string_id = LspWireMessage::apply_workspace_edit_response(
        LspRequestId::String("request-19".to_owned()),
        true,
        None,
        None,
    )
    .to_json();
    assert_eq!(string_id["id"], "request-19");
}

#[test]
fn workspace_apply_edit_request_prefers_document_changes_over_changes() {
    let first_uri = path_to_file_uri(Path::new("src/lib.rs"));
    let second_uri = path_to_file_uri(Path::new("src/main.rs"));
    let value = json!({
        "jsonrpc": "2.0",
        "id": 19,
        "method": "workspace/applyEdit",
        "params": {
            "label": "Apply rename",
            "edit": {
                "changes": {
                    first_uri: [{
                        "range": {
                            "start": { "line": 2, "character": 4 },
                            "end": { "line": 2, "character": 10 }
                        },
                        "newText": "renamed"
                    }]
                },
                "documentChanges": [{
                    "textDocument": { "uri": second_uri, "version": 3 },
                    "edits": [{
                        "range": {
                            "start": { "line": 0, "character": 0 },
                            "end": { "line": 0, "character": 4 }
                        },
                        "newText": "main"
                    }]
                }]
            }
        }
    });

    let request = parse_apply_workspace_edit_request(&value).unwrap();

    assert_eq!(request.id, LspRequestId::Number(19));
    assert_eq!(request.label.as_deref(), Some("Apply rename"));
    assert_eq!(request.edits.len(), 1);
    assert_eq!(request.document_changes.len(), 1);
    assert!(
        request.edits[0]
            .path
            .ends_with(Path::new("src").join("main.rs"))
    );
    assert_eq!(request.edits[0].start_line, 1);
    assert_eq!(request.edits[0].new_text, "main");
    assert_eq!(
        request
            .document_versions
            .get(&request.edits[0].path)
            .copied(),
        Some(3)
    );
    assert!(matches!(
        &request.document_changes[0],
        LspWorkspaceDocumentChange::TextEdit {
            version: Some(3),
            edits,
            ..
        } if edits.len() == 1
    ));
}

#[test]
fn parses_workspace_apply_edit_request_string_id() {
    let uri = path_to_file_uri(Path::new("src/lib.rs"));
    let value = json!({
        "jsonrpc": "2.0",
        "id": "request-19",
        "method": "workspace/applyEdit",
        "params": {
            "edit": {
                "changes": {
                    uri: []
                }
            }
        }
    });

    let request = parse_apply_workspace_edit_request(&value).unwrap();

    assert_eq!(request.id, LspRequestId::String("request-19".to_owned()));
    assert!(request.document_changes.is_empty());
}

#[test]
fn workspace_apply_edit_request_collects_document_versions() {
    let checked_uri = path_to_file_uri(Path::new("src/lib.rs"));
    let unchecked_uri = path_to_file_uri(Path::new("src/main.rs"));
    let missing_uri = path_to_file_uri(Path::new("src/mod.rs"));
    let value = json!({
        "jsonrpc": "2.0",
        "id": 25,
        "method": "workspace/applyEdit",
        "params": {
            "edit": {
                "documentChanges": [
                    {
                        "textDocument": { "uri": checked_uri.clone(), "version": 7 },
                        "edits": []
                    },
                    {
                        "textDocument": { "uri": checked_uri.clone(), "version": 7 },
                        "edits": []
                    },
                    {
                        "textDocument": { "uri": unchecked_uri.clone(), "version": null },
                        "edits": []
                    },
                    {
                        "textDocument": { "uri": missing_uri.clone() },
                        "edits": []
                    }
                ]
            }
        }
    });

    let request = parse_apply_workspace_edit_request(&value).unwrap();
    let checked_path = file_uri_to_path(&checked_uri).unwrap();
    let unchecked_path = file_uri_to_path(&unchecked_uri).unwrap();
    let missing_path = file_uri_to_path(&missing_uri).unwrap();

    assert_eq!(request.document_versions.get(&checked_path), Some(&7));
    assert!(!request.document_versions.contains_key(&unchecked_path));
    assert!(!request.document_versions.contains_key(&missing_path));
}

#[test]
fn workspace_apply_edit_request_rejects_invalid_edit_payloads() {
    let uri = path_to_file_uri(Path::new("src/lib.rs"));
    let value = json!({
        "jsonrpc": "2.0",
        "id": 20,
        "method": "workspace/applyEdit",
        "params": {
            "edit": {
                "changes": {
                    uri: [{
                        "range": {
                            "start": { "line": 0, "character": 0 },
                            "end": { "line": 0, "character": 1 }
                        },
                        "newText": "x".repeat(MAX_LSP_TEXT_EDIT_NEW_TEXT_BYTES + 1)
                    }]
                }
            }
        }
    });

    assert!(parse_apply_workspace_edit_request(&value).is_none());
}

#[test]
fn workspace_apply_edit_request_rejects_document_changes_over_total_edit_limit() {
    let first_uri = path_to_file_uri(Path::new("src/first.rs"));
    let second_uri = path_to_file_uri(Path::new("src/second.rs"));
    let edit = || {
        json!({
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": 0 }
            },
            "newText": "x"
        })
    };
    let first_edits = (0..MAX_LSP_TEXT_EDITS).map(|_| edit()).collect::<Vec<_>>();
    let value = json!({
        "jsonrpc": "2.0",
        "id": 34,
        "method": "workspace/applyEdit",
        "params": {
            "edit": {
                "documentChanges": [
                    {
                        "textDocument": { "uri": first_uri },
                        "edits": first_edits
                    },
                    {
                        "textDocument": { "uri": second_uri },
                        "edits": [edit()]
                    }
                ]
            }
        }
    });

    assert!(parse_apply_workspace_edit_request(&value).is_none());
}

#[test]
fn workspace_apply_edit_request_rejects_wrong_workspace_edit_shapes() {
    let uri = path_to_file_uri(Path::new("src/lib.rs"));
    let changes = json!({
        "jsonrpc": "2.0",
        "id": 22,
        "method": "workspace/applyEdit",
        "params": {
            "edit": {
                "changes": []
            }
        }
    });
    assert!(parse_apply_workspace_edit_request(&changes).is_none());

    let document_changes = json!({
        "jsonrpc": "2.0",
        "id": 23,
        "method": "workspace/applyEdit",
        "params": {
            "edit": {
                "documentChanges": {}
            }
        }
    });
    assert!(parse_apply_workspace_edit_request(&document_changes).is_none());

    let change_edits = json!({
        "jsonrpc": "2.0",
        "id": 26,
        "method": "workspace/applyEdit",
        "params": {
            "edit": {
                "changes": {
                    uri.clone(): {}
                }
            }
        }
    });
    assert!(parse_apply_workspace_edit_request(&change_edits).is_none());

    let text_document = json!({
        "jsonrpc": "2.0",
        "id": 27,
        "method": "workspace/applyEdit",
        "params": {
            "edit": {
                "documentChanges": [{
                    "textDocument": uri.clone(),
                    "edits": []
                }]
            }
        }
    });
    assert!(parse_apply_workspace_edit_request(&text_document).is_none());

    let document_edits = json!({
        "jsonrpc": "2.0",
        "id": 28,
        "method": "workspace/applyEdit",
        "params": {
            "edit": {
                "documentChanges": [{
                    "textDocument": { "uri": uri.clone() },
                    "edits": {}
                }]
            }
        }
    });
    assert!(parse_apply_workspace_edit_request(&document_edits).is_none());
}

#[test]
fn workspace_apply_edit_request_rejects_invalid_document_versions() {
    let uri = path_to_file_uri(Path::new("src/lib.rs"));
    for version in [json!("1"), json!(-1), json!(i32::MAX as u64 + 1)] {
        let value = json!({
            "jsonrpc": "2.0",
            "id": 24,
            "method": "workspace/applyEdit",
            "params": {
                "edit": {
                    "documentChanges": [{
                        "textDocument": { "uri": uri.clone(), "version": version },
                        "edits": []
                    }]
                }
            }
        });

        assert!(parse_apply_workspace_edit_request(&value).is_none());
    }

    let conflicting = json!({
        "jsonrpc": "2.0",
        "id": 24,
        "method": "workspace/applyEdit",
        "params": {
            "edit": {
                "documentChanges": [
                    {
                        "textDocument": { "uri": uri.clone(), "version": 1 },
                        "edits": []
                    },
                    {
                        "textDocument": { "uri": uri.clone(), "version": 2 },
                        "edits": []
                    }
                ]
            }
        }
    });

    assert!(parse_apply_workspace_edit_request(&conflicting).is_none());
}

#[test]
fn workspace_apply_edit_request_parses_resource_operations() {
    let create_uri = path_to_file_uri(Path::new("src/new.rs"));
    let old_uri = path_to_file_uri(Path::new("src/old.rs"));
    let renamed_uri = path_to_file_uri(Path::new("src/renamed.rs"));
    let delete_uri = path_to_file_uri(Path::new("src/delete.rs"));
    let value = json!({
        "jsonrpc": "2.0",
        "id": 21,
        "method": "workspace/applyEdit",
        "params": {
            "edit": {
                "documentChanges": [
                    {
                        "kind": "create",
                        "uri": create_uri,
                        "options": { "overwrite": true, "ignoreIfExists": true }
                    },
                    {
                        "kind": "rename",
                        "oldUri": old_uri,
                        "newUri": renamed_uri,
                        "options": { "overwrite": false, "ignoreIfExists": true }
                    },
                    {
                        "kind": "delete",
                        "uri": delete_uri,
                        "options": { "recursive": true, "ignoreIfNotExists": true }
                    }
                ]
            }
        }
    });

    let request = parse_apply_workspace_edit_request(&value).unwrap();

    assert!(request.edits.is_empty());
    assert_eq!(request.document_changes.len(), 3);
    assert!(matches!(
        &request.document_changes[0],
        LspWorkspaceDocumentChange::Resource(LspWorkspaceResourceOperation::CreateFile {
            overwrite: true,
            ignore_if_exists: true,
            ..
        })
    ));
    assert!(matches!(
        &request.document_changes[1],
        LspWorkspaceDocumentChange::Resource(LspWorkspaceResourceOperation::RenameFile {
            overwrite: false,
            ignore_if_exists: true,
            ..
        })
    ));
    assert!(matches!(
        &request.document_changes[2],
        LspWorkspaceDocumentChange::Resource(LspWorkspaceResourceOperation::DeleteFile {
            recursive: true,
            ignore_if_not_exists: true,
            ..
        })
    ));
}

#[test]
fn workspace_apply_edit_request_prefers_resource_document_changes_over_top_level_changes() {
    let ignored_uri = path_to_file_uri(Path::new("src/ignored.rs"));
    let created_uri = path_to_file_uri(Path::new("src/created.rs"));
    let value = json!({
        "jsonrpc": "2.0",
        "id": 31,
        "method": "workspace/applyEdit",
        "params": {
            "edit": {
                "changes": {
                    ignored_uri: [{
                        "range": {
                            "start": { "line": 0, "character": 0 },
                            "end": { "line": 0, "character": 0 }
                        },
                        "newText": "ignored"
                    }]
                },
                "documentChanges": [
                    {
                        "kind": "create",
                        "uri": created_uri.clone()
                    },
                    {
                        "textDocument": { "uri": created_uri, "version": null },
                        "edits": [{
                            "range": {
                                "start": { "line": 0, "character": 0 },
                                "end": { "line": 0, "character": 0 }
                            },
                            "newText": "created"
                        }]
                    }
                ]
            }
        }
    });

    let request = parse_apply_workspace_edit_request(&value).unwrap();

    assert_eq!(request.edits.len(), 1);
    assert_eq!(request.edits[0].new_text, "created");
    assert!(
        request.edits[0]
            .path
            .ends_with(Path::new("src").join("created.rs"))
    );
    assert_eq!(request.document_changes.len(), 2);
    assert!(matches!(
        &request.document_changes[0],
        LspWorkspaceDocumentChange::Resource(LspWorkspaceResourceOperation::CreateFile { .. })
    ));
    assert!(matches!(
        &request.document_changes[1],
        LspWorkspaceDocumentChange::TextEdit { edits, .. } if edits[0].new_text == "created"
    ));
}

#[test]
fn workspace_apply_edit_request_preserves_mixed_document_change_order() {
    let first_uri = path_to_file_uri(Path::new("src/first.rs"));
    let second_uri = path_to_file_uri(Path::new("src/second.rs"));
    let created_uri = path_to_file_uri(Path::new("src/created.rs"));
    let value = json!({
        "jsonrpc": "2.0",
        "id": 32,
        "method": "workspace/applyEdit",
        "params": {
            "edit": {
                "documentChanges": [
                    {
                        "textDocument": { "uri": first_uri, "version": 1 },
                        "edits": [{
                            "range": {
                                "start": { "line": 0, "character": 0 },
                                "end": { "line": 0, "character": 0 }
                            },
                            "newText": "first"
                        }]
                    },
                    {
                        "kind": "create",
                        "uri": created_uri
                    },
                    {
                        "textDocument": { "uri": second_uri, "version": 2 },
                        "edits": [{
                            "range": {
                                "start": { "line": 1, "character": 0 },
                                "end": { "line": 1, "character": 0 }
                            },
                            "newText": "second"
                        }]
                    }
                ]
            }
        }
    });

    let request = parse_apply_workspace_edit_request(&value).unwrap();

    assert_eq!(request.edits.len(), 2);
    assert!(matches!(
        &request.document_changes[0],
        LspWorkspaceDocumentChange::TextEdit {
            version: Some(1),
            edits,
            ..
        } if edits[0].new_text == "first"
    ));
    assert!(matches!(
        &request.document_changes[1],
        LspWorkspaceDocumentChange::Resource(LspWorkspaceResourceOperation::CreateFile { .. })
    ));
    assert!(matches!(
        &request.document_changes[2],
        LspWorkspaceDocumentChange::TextEdit {
            version: Some(2),
            edits,
            ..
        } if edits[0].new_text == "second"
    ));
}

#[test]
fn workspace_apply_edit_request_rejects_malformed_resource_operations() {
    let uri = path_to_file_uri(Path::new("src/new.rs"));
    for document_change in [
        json!({ "kind": "create", "uri": uri.clone(), "options": [] }),
        json!({ "kind": "create", "uri": uri.clone(), "options": { "overwrite": "yes" } }),
        json!({ "kind": "rename", "oldUri": uri.clone() }),
        json!({ "kind": "delete", "uri": uri.clone(), "options": { "recursive": "yes" } }),
        json!({ "kind": "unknown", "uri": uri.clone() }),
        json!({ "kind": "create", "textDocument": { "uri": uri.clone() }, "uri": uri.clone() }),
    ] {
        let value = json!({
            "jsonrpc": "2.0",
            "id": 21,
            "method": "workspace/applyEdit",
            "params": {
                "edit": {
                    "documentChanges": [document_change]
                }
            }
        });

        assert!(parse_apply_workspace_edit_request(&value).is_none());
    }
}

#[test]
fn workspace_apply_edit_request_rejects_malformed_file_uris() {
    let edits = [json!({
        "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 0 }
        },
        "newText": "x"
    })];
    for edit in [
        json!({
            "changes": {
                "file:///src/lib.rs?version=1": edits.clone()
            }
        }),
        json!({
            "documentChanges": [{
                "textDocument": { "uri": "file:///src/lib.rs#L10" },
                "edits": edits.clone()
            }]
        }),
        json!({
            "documentChanges": [{
                "kind": "create",
                "uri": "file:///src/new%GG.rs"
            }]
        }),
        json!({
            "documentChanges": [{
                "kind": "rename",
                "oldUri": "file:///src/old.rs",
                "newUri": "file:///src/new%FF.rs"
            }]
        }),
        json!({
            "documentChanges": [{
                "kind": "delete",
                "uri": "file:///src/delete%00.rs"
            }]
        }),
    ] {
        let value = json!({
            "jsonrpc": "2.0",
            "id": 21,
            "method": "workspace/applyEdit",
            "params": { "edit": edit }
        });

        assert!(parse_apply_workspace_edit_request(&value).is_none());
    }
}

#[test]
fn workspace_edit_response_prefers_document_changes_over_changes() {
    let first_uri = path_to_file_uri(Path::new("src/lib.rs"));
    let second_uri = path_to_file_uri(Path::new("src/main.rs"));
    let value = json!({
        "jsonrpc": "2.0",
        "id": 15,
        "result": {
            "changes": {
                first_uri: [{
                    "range": {
                        "start": { "line": 1, "character": 2 },
                        "end": { "line": 1, "character": 5 }
                    },
                    "newText": "renamed"
                }]
            },
            "documentChanges": [{
                "textDocument": { "uri": second_uri, "version": 3 },
                "edits": [{
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 0, "character": 4 }
                    },
                    "newText": "main"
                }]
            }]
        }
    });

    let edits = parse_workspace_edit_response(&value).unwrap();
    assert_eq!(edits.len(), 1);
    assert!(edits[0].path.ends_with(Path::new("src").join("main.rs")));
    assert_eq!(edits[0].start_line, 1);
    assert_eq!(edits[0].new_text, "main");
}

#[test]
fn workspace_edit_response_parses_changes_when_document_changes_are_absent() {
    let uri = path_to_file_uri(Path::new("src/lib.rs"));
    let value = json!({
        "jsonrpc": "2.0",
        "id": 15,
        "result": {
            "changes": {
                uri: [{
                    "range": {
                        "start": { "line": 1, "character": 2 },
                        "end": { "line": 1, "character": 5 }
                    },
                    "newText": "renamed"
                }]
            }
        }
    });

    let edits = parse_workspace_edit_response(&value).unwrap();

    assert_eq!(edits.len(), 1);
    assert!(edits[0].path.ends_with(Path::new("src").join("lib.rs")));
    assert_eq!(edits[0].start_line, 2);
    assert_eq!(edits[0].start_column, 3);
    assert_eq!(edits[0].end_column, 6);
    assert_eq!(edits[0].new_text, "renamed");
}

#[test]
fn workspace_edit_response_rejects_resource_document_changes() {
    let ignored_uri = path_to_file_uri(Path::new("src/ignored.rs"));
    let created_uri = path_to_file_uri(Path::new("src/created.rs"));
    let value = json!({
        "jsonrpc": "2.0",
        "id": 16,
        "result": {
            "changes": {
                ignored_uri: [{
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 0, "character": 0 }
                    },
                    "newText": "ignored"
                }]
            },
            "documentChanges": [
                {
                    "kind": "create",
                    "uri": created_uri.clone()
                },
                {
                    "textDocument": { "uri": created_uri, "version": null },
                    "edits": [{
                        "range": {
                            "start": { "line": 0, "character": 0 },
                            "end": { "line": 0, "character": 0 }
                        },
                        "newText": "created"
                    }]
                }
            ]
        }
    });

    assert!(parse_workspace_edit_response(&value).is_none());
}

#[test]
fn workspace_edit_response_rejects_malformed_document_changes() {
    let value = json!({
        "jsonrpc": "2.0",
        "id": 16,
        "result": {
            "documentChanges": [{
                "edits": []
            }]
        }
    });

    assert!(parse_workspace_edit_response(&value).is_none());
}

#[test]
fn workspace_edit_response_rejects_malformed_file_uris() {
    let edits = [json!({
        "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 0 }
        },
        "newText": "x"
    })];
    let changes = json!({
        "jsonrpc": "2.0",
        "id": 16,
        "result": {
            "changes": {
                "file:///src/lib.rs?version=1": edits.clone()
            }
        }
    });
    assert!(parse_workspace_edit_response(&changes).is_none());

    let document_changes = json!({
        "jsonrpc": "2.0",
        "id": 16,
        "result": {
            "documentChanges": [{
                "textDocument": { "uri": "file:///src/lib.rs#L10" },
                "edits": edits
            }]
        }
    });
    assert!(parse_workspace_edit_response(&document_changes).is_none());
}

#[test]
fn workspace_edits_over_the_safety_limit_are_rejected() {
    let uri = path_to_file_uri(Path::new("src/lib.rs"));
    let edits = (0..MAX_LSP_TEXT_EDITS + 1)
        .map(|idx| {
            json!({
                "range": {
                    "start": { "line": idx, "character": 0 },
                    "end": { "line": idx, "character": 1 }
                },
                "newText": "x"
            })
        })
        .collect::<Vec<_>>();
    let value = json!({
        "jsonrpc": "2.0",
        "id": 15,
        "result": {
            "changes": {
                uri: edits
            }
        }
    });

    assert!(parse_workspace_edit_response(&value).is_none());
}

#[test]
fn workspace_edits_with_huge_replacement_text_are_rejected() {
    let uri = path_to_file_uri(Path::new("src/lib.rs"));
    let value = json!({
        "jsonrpc": "2.0",
        "id": 15,
        "result": {
            "changes": {
                uri: [{
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 0, "character": 1 }
                    },
                    "newText": "x".repeat(MAX_LSP_TEXT_EDIT_NEW_TEXT_BYTES + 1)
                }]
            }
        }
    });

    assert!(parse_workspace_edit_response(&value).is_none());
}
