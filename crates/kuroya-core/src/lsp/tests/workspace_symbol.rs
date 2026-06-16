use super::*;

#[test]
fn workspace_symbols_skip_stale_location_link_fields() {
    let valid_uri = path_to_file_uri(Path::new("src/lib.rs"));
    let stale_uri = path_to_file_uri(Path::new("src/stale.rs"));
    let value = json!({
        "jsonrpc": "2.0",
        "id": 27,
        "result": [
            {
                "name": "StaleUriOnly",
                "kind": 12,
                "location": {
                    "uri": stale_uri,
                    "targetRange": range_json(2)
                }
            },
            {
                "name": "StaleRange",
                "kind": 12,
                "location": {
                    "uri": stale_uri,
                    "range": range_json(4),
                    "targetUri": stale_uri,
                    "targetSelectionRange": range_json(4)
                }
            },
            {
                "name": "Valid",
                "kind": 12,
                "location": {
                    "uri": valid_uri,
                    "range": range_json(6)
                }
            }
        ]
    });

    let symbols = parse_workspace_symbols_response(&value).unwrap();

    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].name, "Valid");
    assert!(symbols[0].path.ends_with(Path::new("src").join("lib.rs")));
    assert_eq!(symbols[0].line, 7);
    assert_eq!(symbols[0].column, 1);
}

fn range_json(line: usize) -> serde_json::Value {
    json!({
        "start": { "line": line, "character": 0 },
        "end": { "line": line, "character": 4 }
    })
}
