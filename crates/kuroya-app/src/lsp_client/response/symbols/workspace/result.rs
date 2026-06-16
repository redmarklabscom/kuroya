use super::super::super::response_error;
use crate::ui_event_channel::Sender;
use crate::{lsp_ui_events::LspUiEvent, ui_events::UiEvent};
use kuroya_core::{
    BufferId, LspWorkspaceSymbol, lsp::file_uri_to_path, parse_workspace_symbols_response,
};
use serde_json::Value;
use std::path::PathBuf;

const INVALID_WORKSPACE_SYMBOLS_RESPONSE: &str = "invalid workspace/symbol response";
const WORKSPACE_SYMBOL_RESULT_EVENT_MAX_ITEMS: usize = 200;
const LSP_WORKSPACE_SYMBOL_NAME_MAX_CHARS: usize = 512;
const LSP_WORKSPACE_SYMBOL_DETAIL_MAX_CHARS: usize = 1_024;
const LSP_URI_MAX_BYTES: usize = 16 * 1024;
const LSP_POSITION_COMPONENT_MAX: usize = i32::MAX as usize;

pub(super) fn send_workspace_symbols_result(
    id: BufferId,
    path: PathBuf,
    query: String,
    value: &Value,
    ui_tx: &Sender<UiEvent>,
) {
    let mut error = response_error(value);
    let symbols = if error.is_none() {
        match parse_workspace_symbols_success_response(value) {
            Some(symbols) => Some(symbols),
            None => {
                error = Some(INVALID_WORKSPACE_SYMBOLS_RESPONSE.to_owned());
                None
            }
        }
    } else {
        None
    };
    let _ = crate::lsp_client::response::emit_critical_lsp_response_event(
        ui_tx,
        LspUiEvent::WorkspaceSymbolsResult {
            id,
            path,
            query,
            symbols,
            error,
        },
    );
}

fn parse_workspace_symbols_success_response(value: &Value) -> Option<Vec<LspWorkspaceSymbol>> {
    let result = value.get("result")?;
    if result.is_null() {
        return Some(Vec::new());
    }

    let items = result.as_array()?;
    if items.is_empty() {
        return Some(Vec::new());
    }

    if items
        .iter()
        .take(WORKSPACE_SYMBOL_RESULT_EVENT_MAX_ITEMS)
        .any(|item| !workspace_symbol_item_is_parseable(item))
    {
        return None;
    }

    let capped_value = (items.len() > WORKSPACE_SYMBOL_RESULT_EVENT_MAX_ITEMS).then(|| {
        let mut capped_items =
            Vec::with_capacity(items.len().min(WORKSPACE_SYMBOL_RESULT_EVENT_MAX_ITEMS));
        capped_items.extend(
            items
                .iter()
                .take(WORKSPACE_SYMBOL_RESULT_EVENT_MAX_ITEMS)
                .cloned(),
        );

        let mut object = serde_json::Map::new();
        object.insert("result".to_owned(), Value::Array(capped_items));
        Value::Object(object)
    });

    let mut symbols = parse_workspace_symbols_response(capped_value.as_ref().unwrap_or(value))?;
    symbols.truncate(WORKSPACE_SYMBOL_RESULT_EVENT_MAX_ITEMS);
    Some(symbols)
}

fn workspace_symbol_item_is_parseable(item: &Value) -> bool {
    item.get("name")
        .and_then(Value::as_str)
        .is_some_and(|name| {
            lsp_required_text_payload_is_parseable(name, LSP_WORKSPACE_SYMBOL_NAME_MAX_CHARS)
        })
        && item.get("kind").is_some_and(lsp_u64_fits_u8)
        && lsp_optional_text_payload_is_parseable(
            item.get("containerName"),
            LSP_WORKSPACE_SYMBOL_DETAIL_MAX_CHARS,
        )
        && lsp_optional_text_payload_is_parseable(
            item.get("detail"),
            LSP_WORKSPACE_SYMBOL_DETAIL_MAX_CHARS,
        )
        && workspace_symbol_location_is_parseable(item.get("location"))
}

fn workspace_symbol_location_is_parseable(location: Option<&Value>) -> bool {
    let Some(location) = location else {
        return false;
    };
    if !location.is_object()
        || location.get("targetUri").is_some()
        || location.get("targetRange").is_some()
        || location.get("targetSelectionRange").is_some()
    {
        return false;
    }
    let Some(uri) = location.get("uri").and_then(Value::as_str) else {
        return false;
    };
    if !lsp_file_uri_is_parseable(uri) {
        return false;
    }

    location.get("range").is_none_or(lsp_range_is_parseable)
}

fn lsp_range_is_parseable(range: &Value) -> bool {
    lsp_range_bounds(range).is_some()
}

fn lsp_range_bounds(range: &Value) -> Option<((usize, usize), (usize, usize))> {
    let start = lsp_position(range.get("start")?)?;
    let end = lsp_position(range.get("end")?)?;
    (start <= end).then_some((start, end))
}

fn lsp_position(position: &Value) -> Option<(usize, usize)> {
    Some((
        lsp_zero_based_coordinate_to_usize(position.get("line")?)?,
        lsp_zero_based_coordinate_to_usize(position.get("character")?)?,
    ))
}

fn lsp_zero_based_coordinate_to_usize(value: &Value) -> Option<usize> {
    let value = value.as_u64()?;
    if value > LSP_POSITION_COMPONENT_MAX as u64 {
        return None;
    }
    usize::try_from(value).ok()
}

fn lsp_u64_fits_u8(value: &Value) -> bool {
    value
        .as_u64()
        .is_some_and(|value| u8::try_from(value).is_ok())
}

fn lsp_file_uri_is_parseable(uri: &str) -> bool {
    uri.len() <= LSP_URI_MAX_BYTES
        && uri.starts_with("file://")
        && !uri.contains('?')
        && !uri.contains('#')
        && file_uri_to_path(uri).is_some()
}

fn lsp_required_text_payload_is_parseable(text: &str, max_chars: usize) -> bool {
    let mut chars = 0;
    let mut has_non_whitespace = false;
    for ch in text.chars().take(max_chars.saturating_add(1)) {
        chars += 1;
        has_non_whitespace |= !ch.is_whitespace();
    }
    has_non_whitespace && chars <= max_chars
}

fn lsp_optional_text_payload_is_parseable(value: Option<&Value>, max_chars: usize) -> bool {
    value.is_none_or(|value| {
        value
            .as_str()
            .is_some_and(|text| text.chars().take(max_chars.saturating_add(1)).count() <= max_chars)
    })
}

#[cfg(test)]
mod tests {
    use super::{
        INVALID_WORKSPACE_SYMBOLS_RESPONSE, LSP_POSITION_COMPONENT_MAX, LSP_URI_MAX_BYTES,
        LSP_WORKSPACE_SYMBOL_DETAIL_MAX_CHARS, LSP_WORKSPACE_SYMBOL_NAME_MAX_CHARS,
        WORKSPACE_SYMBOL_RESULT_EVENT_MAX_ITEMS, send_workspace_symbols_result,
    };
    use crate::{lsp_ui_events::LspUiEvent, ui_events::UiEvent};
    use serde_json::json;
    use std::path::{Path, PathBuf};

    #[test]
    fn workspace_symbols_result_reports_invalid_success_payload() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_workspace_symbols_result(
            7,
            PathBuf::from("src/main.rs"),
            "main".to_owned(),
            &json!({ "result": { "not": "an array" } }),
            &tx,
        );

        match rx.recv().expect("workspace symbols result event") {
            UiEvent::Lsp(LspUiEvent::WorkspaceSymbolsResult {
                id,
                path,
                query,
                symbols,
                error,
            }) => {
                assert_eq!(id, 7);
                assert_eq!(path, PathBuf::from("src/main.rs"));
                assert_eq!(query, "main");
                assert_eq!(symbols, None);
                assert_eq!(error.as_deref(), Some(INVALID_WORKSPACE_SYMBOLS_RESPONSE));
            }
            other => panic!("expected workspace symbols result event, got {other:?}"),
        }
    }

    #[test]
    fn workspace_symbols_result_reports_malformed_symbol_item() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_workspace_symbols_result(
            7,
            PathBuf::from("src/main.rs"),
            "main".to_owned(),
            &json!({
                "result": [{
                    "name": "main",
                    "kind": 12,
                    "location": { "uri": "not a file uri" }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("workspace symbols result event") {
            UiEvent::Lsp(LspUiEvent::WorkspaceSymbolsResult { symbols, error, .. }) => {
                assert_eq!(symbols, None);
                assert_eq!(error.as_deref(), Some(INVALID_WORKSPACE_SYMBOLS_RESPONSE));
            }
            other => panic!("expected workspace symbols result event, got {other:?}"),
        }
    }

    #[test]
    fn workspace_symbols_result_reports_invalid_ranges_and_kind_values() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let uri = format!(
            "file:///{}",
            PathBuf::from("src/main.rs")
                .display()
                .to_string()
                .replace('\\', "/")
        );
        send_workspace_symbols_result(
            7,
            PathBuf::from("src/main.rs"),
            "main".to_owned(),
            &json!({
                "result": [{
                    "name": "main",
                    "kind": 999,
                    "location": {
                        "uri": uri,
                        "range": {
                            "start": { "line": 4, "character": 2 },
                            "end": { "line": 2, "character": 1 }
                        }
                    }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("workspace symbols result event") {
            UiEvent::Lsp(LspUiEvent::WorkspaceSymbolsResult { symbols, error, .. }) => {
                assert_eq!(symbols, None);
                assert_eq!(error.as_deref(), Some(INVALID_WORKSPACE_SYMBOLS_RESPONSE));
            }
            other => panic!("expected workspace symbols result event, got {other:?}"),
        }
    }

    #[test]
    fn workspace_symbols_result_reports_coordinates_that_cannot_become_one_based() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let uri = format!(
            "file:///{}",
            PathBuf::from("src/main.rs")
                .display()
                .to_string()
                .replace('\\', "/")
        );
        send_workspace_symbols_result(
            7,
            PathBuf::from("src/main.rs"),
            "main".to_owned(),
            &json!({
                "result": [{
                    "name": "main",
                    "kind": 12,
                    "location": {
                        "uri": uri,
                        "range": {
                            "start": { "line": usize::MAX as u64, "character": 0 },
                            "end": { "line": usize::MAX as u64, "character": 1 }
                        }
                    }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("workspace symbols result event") {
            UiEvent::Lsp(LspUiEvent::WorkspaceSymbolsResult { symbols, error, .. }) => {
                assert_eq!(symbols, None);
                assert_eq!(error.as_deref(), Some(INVALID_WORKSPACE_SYMBOLS_RESPONSE));
            }
            other => panic!("expected workspace symbols result event, got {other:?}"),
        }
    }

    #[test]
    fn workspace_symbols_result_reports_coordinates_above_core_supported_bound() {
        let uri = file_uri("src/main.rs");

        assert_invalid_workspace_symbols_response(json!({
            "result": [{
                "name": "main",
                "kind": 12,
                "location": {
                    "uri": uri,
                    "range": {
                        "start": {
                            "line": LSP_POSITION_COMPONENT_MAX + 1,
                            "character": 0
                        },
                        "end": {
                            "line": LSP_POSITION_COMPONENT_MAX + 1,
                            "character": 1
                        }
                    }
                }
            }]
        }));
    }

    #[test]
    fn workspace_symbols_result_reports_malformed_file_uri_payloads() {
        for uri in [
            "file:///src/main%GG.rs".to_owned(),
            "file:///src/main.rs?version=1".to_owned(),
            format!("file:///{}", "a".repeat(LSP_URI_MAX_BYTES)),
        ] {
            assert_invalid_workspace_symbols_response(json!({
                "result": [{
                    "name": "main",
                    "kind": 12,
                    "location": {
                        "uri": uri
                    }
                }]
            }));
        }
    }

    #[test]
    fn workspace_symbols_result_reports_oversized_label_payloads() {
        let uri = file_uri("src/main.rs");

        assert_invalid_workspace_symbols_response(json!({
            "result": [{
                "name": "n".repeat(LSP_WORKSPACE_SYMBOL_NAME_MAX_CHARS + 1),
                "kind": 12,
                "location": {
                    "uri": uri
                }
            }]
        }));

        let uri = file_uri("src/main.rs");
        assert_invalid_workspace_symbols_response(json!({
            "result": [{
                "name": "main",
                "kind": 12,
                "containerName": "d".repeat(LSP_WORKSPACE_SYMBOL_DETAIL_MAX_CHARS + 1),
                "location": {
                    "uri": uri
                }
            }]
        }));

        let uri = file_uri("src/main.rs");
        assert_invalid_workspace_symbols_response(json!({
            "result": [{
                "name": "main",
                "kind": 12,
                "detail": "d".repeat(LSP_WORKSPACE_SYMBOL_DETAIL_MAX_CHARS + 1),
                "location": {
                    "uri": uri
                }
            }]
        }));
    }

    #[test]
    fn workspace_symbols_result_rejects_stale_location_link_shapes() {
        let uri = file_uri("src/main.rs");

        assert_invalid_workspace_symbols_response(json!({
            "result": [{
                "name": "main",
                "kind": 12,
                "location": {
                    "targetUri": uri,
                    "range": range_json(2),
                    "targetSelectionRange": range_json(2)
                }
            }]
        }));

        let uri = file_uri("src/main.rs");
        assert_invalid_workspace_symbols_response(json!({
            "result": [{
                "name": "main",
                "kind": 12,
                "location": {
                    "uri": uri,
                    "targetRange": range_json(2)
                }
            }]
        }));
    }

    #[test]
    fn workspace_symbols_result_treats_null_success_as_empty() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_workspace_symbols_result(
            7,
            PathBuf::from("src/main.rs"),
            "main".to_owned(),
            &json!({ "result": null }),
            &tx,
        );

        match rx.recv().expect("workspace symbols result event") {
            UiEvent::Lsp(LspUiEvent::WorkspaceSymbolsResult { symbols, error, .. }) => {
                assert_eq!(symbols, Some(Vec::new()));
                assert_eq!(error, None);
            }
            other => panic!("expected workspace symbols result event, got {other:?}"),
        }
    }

    #[test]
    fn workspace_symbols_result_sends_parsed_symbols_event() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let uri = format!(
            "file:///{}",
            PathBuf::from("src/main.rs")
                .display()
                .to_string()
                .replace('\\', "/")
        );
        send_workspace_symbols_result(
            7,
            PathBuf::from("src/main.rs"),
            "main".to_owned(),
            &json!({
                "result": [{
                    "name": "main",
                    "kind": 12,
                    "containerName": "crate",
                    "location": {
                        "uri": uri,
                        "range": {
                            "start": { "line": 2, "character": 3 },
                            "end": { "line": 4, "character": 1 }
                        }
                    }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("workspace symbols result event") {
            UiEvent::Lsp(LspUiEvent::WorkspaceSymbolsResult {
                id,
                path,
                query,
                symbols,
                error,
            }) => {
                assert_eq!(id, 7);
                assert_eq!(path, PathBuf::from("src/main.rs"));
                assert_eq!(query, "main");
                let symbols = symbols.expect("parsed workspace symbols");
                assert_eq!(symbols.len(), 1);
                assert_eq!(symbols[0].name, "main");
                assert_eq!(symbols[0].detail.as_deref(), Some("crate"));
                assert_eq!(symbols[0].line, 3);
                assert_eq!(symbols[0].column, 4);
                assert_eq!(error, None);
            }
            other => panic!("expected workspace symbols result event, got {other:?}"),
        }
    }

    #[test]
    fn workspace_symbols_result_caps_symbols_before_ui_event() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let result = (0..WORKSPACE_SYMBOL_RESULT_EVENT_MAX_ITEMS + 25)
            .map(workspace_symbol_json)
            .collect::<Vec<_>>();
        send_workspace_symbols_result(
            7,
            PathBuf::from("src/main.rs"),
            "main".to_owned(),
            &json!({ "result": result }),
            &tx,
        );

        match rx.recv().expect("workspace symbols result event") {
            UiEvent::Lsp(LspUiEvent::WorkspaceSymbolsResult { symbols, error, .. }) => {
                assert_eq!(
                    symbols.expect("bounded workspace symbols").len(),
                    WORKSPACE_SYMBOL_RESULT_EVENT_MAX_ITEMS
                );
                assert_eq!(error, None);
            }
            other => panic!("expected workspace symbols result event, got {other:?}"),
        }
    }

    #[test]
    fn workspace_symbols_result_ignores_malformed_tail_past_event_cap() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let mut result = (0..WORKSPACE_SYMBOL_RESULT_EVENT_MAX_ITEMS)
            .map(workspace_symbol_json)
            .collect::<Vec<_>>();
        result.push(json!({
            "name": "bad-tail",
            "kind": 12,
            "location": { "uri": "not a file uri" }
        }));
        send_workspace_symbols_result(
            7,
            PathBuf::from("src/main.rs"),
            "main".to_owned(),
            &json!({ "result": result }),
            &tx,
        );

        match rx.recv().expect("workspace symbols result event") {
            UiEvent::Lsp(LspUiEvent::WorkspaceSymbolsResult { symbols, error, .. }) => {
                assert_eq!(
                    symbols.expect("bounded workspace symbols").len(),
                    WORKSPACE_SYMBOL_RESULT_EVENT_MAX_ITEMS
                );
                assert_eq!(error, None);
            }
            other => panic!("expected workspace symbols result event, got {other:?}"),
        }
    }

    #[test]
    fn workspace_symbols_result_ignores_oversized_tail_before_parsing() {
        let mut result = (0..WORKSPACE_SYMBOL_RESULT_EVENT_MAX_ITEMS)
            .map(workspace_symbol_json)
            .collect::<Vec<_>>();
        result.push(json!({
            "name": "tail",
            "kind": 12,
            "containerName": "d".repeat(LSP_WORKSPACE_SYMBOL_DETAIL_MAX_CHARS + 1),
            "location": {
                "uri": file_uri("aaa/tail.rs")
            }
        }));

        let (symbols, error) = workspace_symbols_response(json!({ "result": result }));

        assert_eq!(error, None);
        let symbols = symbols.expect("bounded workspace symbols");
        assert_eq!(symbols.len(), WORKSPACE_SYMBOL_RESULT_EVENT_MAX_ITEMS);
        assert!(symbols.iter().all(|symbol| symbol.name != "tail"));
    }

    fn assert_invalid_workspace_symbols_response(value: serde_json::Value) {
        let (symbols, error) = workspace_symbols_response(value);

        assert_eq!(symbols, None);
        assert_eq!(error.as_deref(), Some(INVALID_WORKSPACE_SYMBOLS_RESPONSE));
    }

    fn workspace_symbols_response(
        value: serde_json::Value,
    ) -> (Option<Vec<kuroya_core::LspWorkspaceSymbol>>, Option<String>) {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_workspace_symbols_result(
            7,
            PathBuf::from("src/main.rs"),
            "main".to_owned(),
            &value,
            &tx,
        );

        match rx.recv().expect("workspace symbols result event") {
            UiEvent::Lsp(LspUiEvent::WorkspaceSymbolsResult { symbols, error, .. }) => {
                (symbols, error)
            }
            other => panic!("expected workspace symbols result event, got {other:?}"),
        }
    }

    fn workspace_symbol_json(index: usize) -> serde_json::Value {
        let uri = file_uri(PathBuf::from(format!("src/{index}.rs")));
        json!({
            "name": format!("Symbol{index}"),
            "kind": 12,
            "location": {
                "uri": uri,
                "range": {
                    "start": { "line": index, "character": 0 },
                    "end": { "line": index, "character": 1 }
                }
            }
        })
    }

    fn range_json(line: usize) -> serde_json::Value {
        json!({
            "start": { "line": line, "character": 0 },
            "end": { "line": line, "character": 1 }
        })
    }

    fn file_uri(path: impl AsRef<Path>) -> String {
        format!(
            "file:///{}",
            path.as_ref().display().to_string().replace('\\', "/")
        )
    }
}
