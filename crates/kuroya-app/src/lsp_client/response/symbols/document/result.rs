use super::super::super::response_error;
use crate::ui_event_channel::Sender;
use crate::{lsp_ui_events::LspUiEvent, ui_events::UiEvent};
use kuroya_core::{
    BufferId, LspDocumentSymbol, lsp::file_uri_to_path, parse_document_symbols_response,
};
use serde_json::{Map, Value};
use std::path::PathBuf;

const INVALID_DOCUMENT_SYMBOLS_RESPONSE: &str = "invalid textDocument/documentSymbol response";
const MAX_VALIDATED_DOCUMENT_SYMBOLS: usize = 5_000;
const MAX_VALIDATED_DOCUMENT_SYMBOL_DEPTH: usize = 64;
const MAX_VALIDATED_DOCUMENT_SYMBOL_POSITION_COMPONENT: usize = i32::MAX as usize;

type LspPosition = (usize, usize);
type LspRangeBounds = (LspPosition, LspPosition);

pub(super) fn send_document_symbols_result(
    id: BufferId,
    path: PathBuf,
    version: u64,
    value: &Value,
    ui_tx: &Sender<UiEvent>,
) {
    let mut error = response_error(value);
    let symbols = if error.is_none() {
        match parse_document_symbols_success_response(value, &path) {
            Some(symbols) => Some(symbols),
            None => {
                error = Some(INVALID_DOCUMENT_SYMBOLS_RESPONSE.to_owned());
                None
            }
        }
    } else {
        None
    };
    let _ = crate::lsp_client::response::emit_critical_lsp_response_event(
        ui_tx,
        LspUiEvent::DocumentSymbolsResult {
            id,
            path,
            version,
            symbols,
            error,
        },
    );
}

fn parse_document_symbols_success_response(
    value: &Value,
    path: &std::path::Path,
) -> Option<Vec<LspDocumentSymbol>> {
    let result = value.get("result")?;
    if result.is_null() {
        return Some(Vec::new());
    }

    let items = result.as_array()?;
    if items.is_empty() {
        return Some(Vec::new());
    }

    let bounded_items = bounded_document_symbol_items(items)?;
    let bounded_value = document_symbols_response_value(bounded_items);
    parse_document_symbols_response(&bounded_value, path)
}

fn bounded_document_symbol_items(items: &[Value]) -> Option<Vec<Value>> {
    let mut remaining = MAX_VALIDATED_DOCUMENT_SYMBOLS;
    let mut bounded_items = Vec::with_capacity(items.len().min(MAX_VALIDATED_DOCUMENT_SYMBOLS));
    for item in items {
        if remaining == 0 {
            break;
        }
        if let Some(item) = bounded_document_symbol_item(item, 0, &mut remaining)? {
            bounded_items.push(item);
        }
    }
    Some(bounded_items)
}

fn bounded_document_symbol_item(
    item: &Value,
    depth: usize,
    remaining: &mut usize,
) -> Option<Option<Value>> {
    if *remaining == 0 || depth > MAX_VALIDATED_DOCUMENT_SYMBOL_DEPTH {
        return Some(None);
    }

    if item.get("selectionRange").is_some() {
        bounded_hierarchical_document_symbol(item, depth, remaining)
    } else {
        bounded_flat_document_symbol(item, remaining)
    }
}

fn bounded_hierarchical_document_symbol(
    item: &Value,
    depth: usize,
    remaining: &mut usize,
) -> Option<Option<Value>> {
    let name = item.get("name").and_then(Value::as_str)?;
    let kind = item.get("kind").and_then(lsp_u64_as_u8)?;
    let detail = item.get("detail");
    if !detail.is_none_or(Value::is_string) {
        return None;
    }

    let range = lsp_range_bounds(item.get("range"))?;
    let selection_range = lsp_range_bounds(item.get("selectionRange"))?;
    if !lsp_range_contains_range(range, selection_range) {
        return None;
    }

    *remaining = remaining.saturating_sub(1);
    let children = match item.get("children") {
        None => None,
        Some(Value::Array(children)) => Some(bounded_document_symbol_children(
            children,
            depth + 1,
            remaining,
        )?),
        Some(_) => return None,
    };

    let mut bounded_item = Map::new();
    bounded_item.insert("name".to_owned(), Value::String(name.to_owned()));
    if let Some(detail) = detail.and_then(Value::as_str) {
        bounded_item.insert("detail".to_owned(), Value::String(detail.to_owned()));
    }
    bounded_item.insert("kind".to_owned(), Value::from(kind));
    bounded_item.insert("range".to_owned(), lsp_range_value(range));
    bounded_item.insert(
        "selectionRange".to_owned(),
        lsp_range_value(selection_range),
    );
    if let Some(children) = children {
        bounded_item.insert("children".to_owned(), Value::Array(children));
    }

    Some(Some(Value::Object(bounded_item)))
}

fn bounded_document_symbol_children(
    children: &[Value],
    depth: usize,
    remaining: &mut usize,
) -> Option<Vec<Value>> {
    let mut bounded_children = Vec::with_capacity(children.len().min(*remaining));
    for child in children {
        if *remaining == 0 {
            break;
        }
        if let Some(child) = bounded_document_symbol_item(child, depth, remaining)? {
            bounded_children.push(child);
        }
    }
    Some(bounded_children)
}

fn bounded_flat_document_symbol(item: &Value, remaining: &mut usize) -> Option<Option<Value>> {
    let name = item.get("name").and_then(Value::as_str)?;
    let kind = item.get("kind").and_then(lsp_u64_as_u8)?;
    let container_name = item.get("containerName");
    if !container_name.is_none_or(Value::is_string) {
        return None;
    }
    let (uri, range) = lsp_location_parts(item.get("location"))?;

    *remaining = remaining.saturating_sub(1);
    let mut location = Map::new();
    location.insert("uri".to_owned(), Value::String(uri.to_owned()));
    location.insert("range".to_owned(), lsp_range_value(range));

    let mut bounded_item = Map::new();
    bounded_item.insert("name".to_owned(), Value::String(name.to_owned()));
    bounded_item.insert("kind".to_owned(), Value::from(kind));
    if let Some(container_name) = container_name.and_then(Value::as_str) {
        bounded_item.insert(
            "containerName".to_owned(),
            Value::String(container_name.to_owned()),
        );
    }
    bounded_item.insert("location".to_owned(), Value::Object(location));

    Some(Some(Value::Object(bounded_item)))
}

fn lsp_location_parts(location: Option<&Value>) -> Option<(&str, LspRangeBounds)> {
    let location = location?;
    let uri = location.get("uri").and_then(Value::as_str)?;
    file_uri_to_path(uri)?;
    Some((uri, lsp_range_bounds(location.get("range"))?))
}

fn document_symbols_response_value(items: Vec<Value>) -> Value {
    let mut response = Map::new();
    response.insert("result".to_owned(), Value::Array(items));
    Value::Object(response)
}

fn lsp_range_contains_range(outer: LspRangeBounds, inner: LspRangeBounds) -> bool {
    let (outer_start, outer_end) = outer;
    let (inner_start, inner_end) = inner;
    outer_start <= inner_start && inner_end <= outer_end
}

fn lsp_range_bounds(range: Option<&Value>) -> Option<LspRangeBounds> {
    let range = range?;
    let start = lsp_position(range.get("start"))?;
    let end = lsp_position(range.get("end"))?;
    (start <= end).then_some((start, end))
}

fn lsp_range_value(range: LspRangeBounds) -> Value {
    let (start, end) = range;
    let mut value = Map::new();
    value.insert("start".to_owned(), lsp_position_value(start));
    value.insert("end".to_owned(), lsp_position_value(end));
    Value::Object(value)
}

fn lsp_position_value(position: LspPosition) -> Value {
    let (line, character) = position;
    let mut value = Map::new();
    value.insert("line".to_owned(), Value::from(line as u64));
    value.insert("character".to_owned(), Value::from(character as u64));
    Value::Object(value)
}

fn lsp_position(position: Option<&Value>) -> Option<LspPosition> {
    let position = position?;
    Some((
        lsp_zero_based_coordinate_to_usize(position.get("line")?)?,
        lsp_zero_based_coordinate_to_usize(position.get("character")?)?,
    ))
}

fn lsp_zero_based_coordinate_to_usize(value: &Value) -> Option<usize> {
    usize::try_from(value.as_u64()?)
        .ok()
        .filter(|value| *value <= MAX_VALIDATED_DOCUMENT_SYMBOL_POSITION_COMPONENT)
}

fn lsp_u64_as_u8(value: &Value) -> Option<u8> {
    u8::try_from(value.as_u64()?).ok()
}

#[cfg(test)]
mod tests {
    use super::{
        INVALID_DOCUMENT_SYMBOLS_RESPONSE, MAX_VALIDATED_DOCUMENT_SYMBOL_DEPTH,
        MAX_VALIDATED_DOCUMENT_SYMBOL_POSITION_COMPONENT, MAX_VALIDATED_DOCUMENT_SYMBOLS,
        send_document_symbols_result,
    };
    use crate::{lsp_ui_events::LspUiEvent, ui_events::UiEvent};
    use serde_json::json;
    use std::path::{Path, PathBuf};

    #[test]
    fn document_symbols_result_reports_invalid_success_payload() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_document_symbols_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({ "result": { "not": "an array" } }),
            &tx,
        );

        match rx.recv().expect("document symbols result event") {
            UiEvent::Lsp(LspUiEvent::DocumentSymbolsResult {
                id,
                path,
                version,
                symbols,
                error,
            }) => {
                assert_eq!(id, 7);
                assert_eq!(path, PathBuf::from("src/main.rs"));
                assert_eq!(version, 11);
                assert_eq!(symbols, None);
                assert_eq!(error.as_deref(), Some(INVALID_DOCUMENT_SYMBOLS_RESPONSE));
            }
            other => panic!("expected document symbols result event, got {other:?}"),
        }
    }

    #[test]
    fn document_symbols_result_reports_malformed_hierarchical_item() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_document_symbols_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({
                "result": [{
                    "name": "module",
                    "kind": 2,
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 8, "character": 1 }
                    },
                    "selectionRange": {
                        "start": { "line": 0, "character": 4 },
                        "end": { "line": 0, "character": 10 }
                    },
                    "children": { "not": "an array" }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("document symbols result event") {
            UiEvent::Lsp(LspUiEvent::DocumentSymbolsResult { symbols, error, .. }) => {
                assert_eq!(symbols, None);
                assert_eq!(error.as_deref(), Some(INVALID_DOCUMENT_SYMBOLS_RESPONSE));
            }
            other => panic!("expected document symbols result event, got {other:?}"),
        }
    }

    #[test]
    fn document_symbols_result_reports_invalid_ranges_and_kind_values() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_document_symbols_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({
                "result": [{
                    "name": "module",
                    "kind": 999,
                    "range": {
                        "start": { "line": 2, "character": 0 },
                        "end": { "line": 1, "character": 1 }
                    },
                    "selectionRange": {
                        "start": { "line": usize::MAX as u64, "character": 0 },
                        "end": { "line": usize::MAX as u64, "character": 1 }
                    }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("document symbols result event") {
            UiEvent::Lsp(LspUiEvent::DocumentSymbolsResult { symbols, error, .. }) => {
                assert_eq!(symbols, None);
                assert_eq!(error.as_deref(), Some(INVALID_DOCUMENT_SYMBOLS_RESPONSE));
            }
            other => panic!("expected document symbols result event, got {other:?}"),
        }
    }

    #[test]
    fn document_symbols_result_reports_coordinates_above_core_supported_bound() {
        let too_large = (MAX_VALIDATED_DOCUMENT_SYMBOL_POSITION_COMPONENT as u64) + 1;
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_document_symbols_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({
                "result": [{
                    "name": "module",
                    "kind": 2,
                    "range": {
                        "start": { "line": too_large, "character": 0 },
                        "end": { "line": too_large, "character": 1 }
                    },
                    "selectionRange": {
                        "start": { "line": too_large, "character": 0 },
                        "end": { "line": too_large, "character": 1 }
                    }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("document symbols result event") {
            UiEvent::Lsp(LspUiEvent::DocumentSymbolsResult { symbols, error, .. }) => {
                assert_eq!(symbols, None);
                assert_eq!(error.as_deref(), Some(INVALID_DOCUMENT_SYMBOLS_RESPONSE));
            }
            other => panic!("expected document symbols result event, got {other:?}"),
        }
    }

    #[test]
    fn document_symbols_result_reports_selection_range_outside_symbol_range() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_document_symbols_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({
                "result": [{
                    "name": "module",
                    "kind": 2,
                    "range": {
                        "start": { "line": 2, "character": 0 },
                        "end": { "line": 4, "character": 1 }
                    },
                    "selectionRange": {
                        "start": { "line": 1, "character": 0 },
                        "end": { "line": 1, "character": 6 }
                    }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("document symbols result event") {
            UiEvent::Lsp(LspUiEvent::DocumentSymbolsResult { symbols, error, .. }) => {
                assert_eq!(symbols, None);
                assert_eq!(error.as_deref(), Some(INVALID_DOCUMENT_SYMBOLS_RESPONSE));
            }
            other => panic!("expected document symbols result event, got {other:?}"),
        }
    }

    #[test]
    fn document_symbols_result_reports_malformed_flat_item() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_document_symbols_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({
                "result": [{
                    "name": "main",
                    "kind": 12,
                    "location": {
                        "uri": "not a file uri",
                        "range": {
                            "start": { "line": 0, "character": 0 },
                            "end": { "line": 1, "character": 1 }
                        }
                    }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("document symbols result event") {
            UiEvent::Lsp(LspUiEvent::DocumentSymbolsResult { symbols, error, .. }) => {
                assert_eq!(symbols, None);
                assert_eq!(error.as_deref(), Some(INVALID_DOCUMENT_SYMBOLS_RESPONSE));
            }
            other => panic!("expected document symbols result event, got {other:?}"),
        }
    }

    #[test]
    fn document_symbols_result_reports_location_link_shape_for_flat_item() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let symbol_path = PathBuf::from("src/main.rs");
        send_document_symbols_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({
                "result": [{
                    "name": "main",
                    "kind": 12,
                    "location": {
                        "targetUri": file_uri(&symbol_path),
                        "targetSelectionRange": {
                            "start": { "line": 0, "character": 0 },
                            "end": { "line": 1, "character": 1 }
                        }
                    }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("document symbols result event") {
            UiEvent::Lsp(LspUiEvent::DocumentSymbolsResult { symbols, error, .. }) => {
                assert_eq!(symbols, None);
                assert_eq!(error.as_deref(), Some(INVALID_DOCUMENT_SYMBOLS_RESPONSE));
            }
            other => panic!("expected document symbols result event, got {other:?}"),
        }
    }

    #[test]
    fn document_symbols_result_reports_invalid_percent_encoded_location_uri() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_document_symbols_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({
                "result": [{
                    "name": "main",
                    "kind": 12,
                    "location": {
                        "uri": "file:///C:/workspace/src/lib%GG.rs",
                        "range": {
                            "start": { "line": 0, "character": 0 },
                            "end": { "line": 1, "character": 1 }
                        }
                    }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("document symbols result event") {
            UiEvent::Lsp(LspUiEvent::DocumentSymbolsResult { symbols, error, .. }) => {
                assert_eq!(symbols, None);
                assert_eq!(error.as_deref(), Some(INVALID_DOCUMENT_SYMBOLS_RESPONSE));
            }
            other => panic!("expected document symbols result event, got {other:?}"),
        }
    }

    #[test]
    fn document_symbols_result_treats_null_success_as_empty() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_document_symbols_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({ "result": null }),
            &tx,
        );

        match rx.recv().expect("document symbols result event") {
            UiEvent::Lsp(LspUiEvent::DocumentSymbolsResult { symbols, error, .. }) => {
                assert_eq!(symbols, Some(Vec::new()));
                assert_eq!(error, None);
            }
            other => panic!("expected document symbols result event, got {other:?}"),
        }
    }

    #[test]
    fn document_symbols_result_sends_parsed_hierarchical_symbols_event() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_document_symbols_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({
                "result": [{
                    "name": "module",
                    "detail": "mod",
                    "kind": 2,
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 8, "character": 1 }
                    },
                    "selectionRange": {
                        "start": { "line": 0, "character": 4 },
                        "end": { "line": 0, "character": 10 }
                    },
                    "children": [{
                        "name": "run",
                        "kind": 12,
                        "range": {
                            "start": { "line": 2, "character": 0 },
                            "end": { "line": 4, "character": 1 }
                        },
                        "selectionRange": {
                            "start": { "line": 2, "character": 3 },
                            "end": { "line": 2, "character": 6 }
                        }
                    }]
                }]
            }),
            &tx,
        );

        match rx.recv().expect("document symbols result event") {
            UiEvent::Lsp(LspUiEvent::DocumentSymbolsResult { symbols, error, .. }) => {
                let symbols = symbols.expect("parsed document symbols");
                assert_eq!(symbols.len(), 2);
                assert_eq!(symbols[0].name, "module");
                assert_eq!(symbols[0].detail.as_deref(), Some("mod"));
                assert_eq!(symbols[0].line, 1);
                assert_eq!(symbols[0].column, 5);
                assert_eq!(symbols[1].name, "run");
                assert_eq!(symbols[1].depth, 1);
                assert_eq!(error, None);
            }
            other => panic!("expected document symbols result event, got {other:?}"),
        }
    }

    #[test]
    fn document_symbols_result_sends_parsed_flat_symbols_event() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let symbol_path = PathBuf::from("src/main.rs");
        send_document_symbols_result(
            7,
            PathBuf::from("src/lib.rs"),
            11,
            &json!({
                "result": [{
                    "name": "main",
                    "kind": 12,
                    "containerName": "crate",
                    "location": {
                        "uri": file_uri(&symbol_path),
                        "range": {
                            "start": { "line": 0, "character": 3 },
                            "end": { "line": 1, "character": 1 }
                        }
                    }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("document symbols result event") {
            UiEvent::Lsp(LspUiEvent::DocumentSymbolsResult { symbols, error, .. }) => {
                let symbols = symbols.expect("parsed document symbols");
                assert_eq!(symbols.len(), 1);
                assert_eq!(symbols[0].name, "main");
                assert_eq!(symbols[0].detail.as_deref(), Some("crate"));
                assert_eq!(symbols[0].line, 1);
                assert_eq!(symbols[0].column, 4);
                assert!(symbols[0].path.ends_with(Path::new("src").join("main.rs")));
                assert_eq!(error, None);
            }
            other => panic!("expected document symbols result event, got {other:?}"),
        }
    }

    #[test]
    fn document_symbols_result_caps_validated_flat_symbols() {
        let symbol_path = PathBuf::from("src/main.rs");
        let mut result = (0..=MAX_VALIDATED_DOCUMENT_SYMBOLS)
            .map(|index| {
                json!({
                    "name": format!("symbol{index:04}"),
                    "kind": 12,
                    "location": {
                        "uri": file_uri(&symbol_path),
                        "range": {
                            "start": { "line": index, "character": 0 },
                            "end": { "line": index, "character": 1 }
                        }
                    }
                })
            })
            .collect::<Vec<_>>();
        result[MAX_VALIDATED_DOCUMENT_SYMBOLS]["name"] = json!("x".repeat(1024 * 1024));
        result[MAX_VALIDATED_DOCUMENT_SYMBOLS]["location"] = json!({
            "uri": "not a file uri",
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": 1 }
            }
        });

        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_document_symbols_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({ "result": result }),
            &tx,
        );

        match rx.recv().expect("document symbols result event") {
            UiEvent::Lsp(LspUiEvent::DocumentSymbolsResult { symbols, error, .. }) => {
                let symbols = symbols.expect("bounded document symbols");
                assert_eq!(symbols.len(), MAX_VALIDATED_DOCUMENT_SYMBOLS);
                assert_eq!(error, None);
            }
            other => panic!("expected document symbols result event, got {other:?}"),
        }
    }

    #[test]
    fn document_symbols_result_ignores_malformed_children_past_depth_cap() {
        let mut nested = json!({
            "name": "too-deep",
            "kind": "bad",
            "children": { "not": "an array" }
        });
        for depth in (0..=MAX_VALIDATED_DOCUMENT_SYMBOL_DEPTH).rev() {
            nested = hierarchical_symbol_json(depth, vec![nested]);
        }

        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_document_symbols_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({ "result": [nested] }),
            &tx,
        );

        match rx.recv().expect("document symbols result event") {
            UiEvent::Lsp(LspUiEvent::DocumentSymbolsResult { symbols, error, .. }) => {
                let symbols = symbols.expect("depth-bounded document symbols");
                assert_eq!(symbols.len(), MAX_VALIDATED_DOCUMENT_SYMBOL_DEPTH + 1);
                assert_eq!(error, None);
            }
            other => panic!("expected document symbols result event, got {other:?}"),
        }
    }

    #[test]
    fn document_symbols_result_caps_nested_children_before_malformed_tail() {
        let mut children = (0..MAX_VALIDATED_DOCUMENT_SYMBOLS)
            .map(|index| hierarchical_symbol_json(index + 1, Vec::new()))
            .collect::<Vec<_>>();
        children[MAX_VALIDATED_DOCUMENT_SYMBOLS - 1] = json!({
            "name": "bad-tail",
            "kind": 12,
            "children": { "not": "an array" }
        });
        let root = hierarchical_symbol_json(0, children);

        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_document_symbols_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({ "result": [root] }),
            &tx,
        );

        match rx.recv().expect("document symbols result event") {
            UiEvent::Lsp(LspUiEvent::DocumentSymbolsResult { symbols, error, .. }) => {
                let symbols = symbols.expect("count-bounded document symbols");
                assert_eq!(symbols.len(), MAX_VALIDATED_DOCUMENT_SYMBOLS);
                assert_eq!(
                    symbols.last().map(|symbol| symbol.name.as_str()),
                    Some("symbol4999")
                );
                assert_eq!(error, None);
            }
            other => panic!("expected document symbols result event, got {other:?}"),
        }
    }

    fn hierarchical_symbol_json(
        index: usize,
        children: Vec<serde_json::Value>,
    ) -> serde_json::Value {
        let mut symbol = json!({
            "name": format!("symbol{index:04}"),
            "kind": 12,
            "range": {
                "start": { "line": index, "character": 0 },
                "end": { "line": index, "character": 6 }
            },
            "selectionRange": {
                "start": { "line": index, "character": 0 },
                "end": { "line": index, "character": 6 }
            }
        });
        if !children.is_empty() {
            symbol["children"] = json!(children);
        }
        symbol
    }

    fn file_uri(path: &Path) -> String {
        format!("file:///{}", path.display().to_string().replace('\\', "/"))
    }
}
