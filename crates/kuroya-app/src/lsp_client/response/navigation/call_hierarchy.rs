use super::super::response_error;
use crate::ui_event_channel::Sender;
use crate::{
    lsp_client::pending::PendingLspRequest, lsp_ui_events::LspUiEvent, ui_events::UiEvent,
};
use kuroya_core::{
    LspCallHierarchyCall, LspCallHierarchyItem, lsp::file_uri_to_path,
    parse_call_hierarchy_incoming_response, parse_call_hierarchy_outgoing_response,
    parse_call_hierarchy_prepare_response,
};
use serde_json::Value;
use std::io::{self, Write};

const INVALID_PREPARE_CALL_HIERARCHY_RESPONSE: &str =
    "invalid textDocument/prepareCallHierarchy response";
const INVALID_CALL_HIERARCHY_INCOMING_RESPONSE: &str =
    "invalid callHierarchy/incomingCalls response";
const INVALID_CALL_HIERARCHY_OUTGOING_RESPONSE: &str =
    "invalid callHierarchy/outgoingCalls response";
const MAX_EMITTED_CALL_HIERARCHY_ITEMS: usize = 100;
const MAX_EMITTED_CALL_HIERARCHY_CALLS: usize = 500;
const MAX_EMITTED_CALL_HIERARCHY_RANGES: usize = 100;
const MAX_VALIDATED_CALL_HIERARCHY_ITEM_PAYLOAD_BYTES: usize = 64 * 1024;
const MAX_VALIDATED_CALL_HIERARCHY_POSITION_COMPONENT: u64 = i32::MAX as u64;

pub(super) fn handle_call_hierarchy_response(
    pending: PendingLspRequest,
    value: Value,
    ui_tx: &Sender<UiEvent>,
) {
    let mut error = response_error(&value);
    match pending {
        PendingLspRequest::PrepareCallHierarchy {
            id,
            path,
            version,
            line,
            character,
        } => {
            let items = if error.is_none() {
                match parse_call_hierarchy_prepare_success_response(&value) {
                    Some(items) => Some(items),
                    None => {
                        error = Some(INVALID_PREPARE_CALL_HIERARCHY_RESPONSE.to_owned());
                        None
                    }
                }
            } else {
                None
            };
            let _ = crate::lsp_client::response::emit_critical_lsp_response_event(
                ui_tx,
                LspUiEvent::CallHierarchyPrepared {
                    id,
                    path,
                    version,
                    line,
                    column: character,
                    items,
                    error,
                },
            );
        }
        PendingLspRequest::CallHierarchyIncoming {
            id,
            path,
            version,
            item,
        } => {
            let calls = if error.is_none() {
                match parse_call_hierarchy_incoming_success_response(&value) {
                    Some(calls) => Some(calls),
                    None => {
                        error = Some(INVALID_CALL_HIERARCHY_INCOMING_RESPONSE.to_owned());
                        None
                    }
                }
            } else {
                None
            };
            let _ = crate::lsp_client::response::emit_critical_lsp_response_event(
                ui_tx,
                LspUiEvent::CallHierarchyIncomingResult {
                    id,
                    path,
                    version,
                    item,
                    calls,
                    error,
                },
            );
        }
        PendingLspRequest::CallHierarchyOutgoing {
            id,
            path,
            version,
            item,
        } => {
            let calls = if error.is_none() {
                match parse_call_hierarchy_outgoing_success_response(&value) {
                    Some(calls) => Some(calls),
                    None => {
                        error = Some(INVALID_CALL_HIERARCHY_OUTGOING_RESPONSE.to_owned());
                        None
                    }
                }
            } else {
                None
            };
            let _ = crate::lsp_client::response::emit_critical_lsp_response_event(
                ui_tx,
                LspUiEvent::CallHierarchyOutgoingResult {
                    id,
                    path,
                    version,
                    item,
                    calls,
                    error,
                },
            );
        }
        _ => {}
    }
}

fn parse_call_hierarchy_prepare_success_response(
    value: &Value,
) -> Option<Vec<LspCallHierarchyItem>> {
    let result = value.get("result")?;
    if result.is_null() {
        return Some(Vec::new());
    }

    let items = result.as_array()?;
    if items
        .iter()
        .take(MAX_EMITTED_CALL_HIERARCHY_ITEMS)
        .any(|item| !call_hierarchy_item_is_parseable(item))
    {
        return None;
    }

    let mut items = parse_call_hierarchy_prepare_response(value)?;
    items.truncate(MAX_EMITTED_CALL_HIERARCHY_ITEMS);
    Some(items)
}

fn parse_call_hierarchy_incoming_success_response(
    value: &Value,
) -> Option<Vec<LspCallHierarchyCall>> {
    parse_call_hierarchy_calls_success_response(
        value,
        "from",
        parse_call_hierarchy_incoming_response,
    )
}

fn parse_call_hierarchy_outgoing_success_response(
    value: &Value,
) -> Option<Vec<LspCallHierarchyCall>> {
    parse_call_hierarchy_calls_success_response(value, "to", parse_call_hierarchy_outgoing_response)
}

fn parse_call_hierarchy_calls_success_response(
    value: &Value,
    item_key: &str,
    parser: fn(&Value) -> Option<Vec<LspCallHierarchyCall>>,
) -> Option<Vec<LspCallHierarchyCall>> {
    let result = value.get("result")?;
    if result.is_null() {
        return Some(Vec::new());
    }

    let calls = result.as_array()?;
    if calls
        .iter()
        .take(MAX_EMITTED_CALL_HIERARCHY_CALLS)
        .any(|call| !call_hierarchy_call_is_parseable(call, item_key))
    {
        return None;
    }

    let mut calls = parser(value)?;
    calls.truncate(MAX_EMITTED_CALL_HIERARCHY_CALLS);
    for call in &mut calls {
        call.ranges.truncate(MAX_EMITTED_CALL_HIERARCHY_RANGES);
    }
    Some(calls)
}

fn call_hierarchy_item_is_parseable(item: &Value) -> bool {
    call_hierarchy_prepare_item_shape_is_valid(item)
}

fn call_hierarchy_call_is_parseable(call: &Value, item_key: &str) -> bool {
    call_hierarchy_call_shape_is_valid(call, item_key)
}

fn call_hierarchy_call_shape_is_valid(call: &Value, item_key: &str) -> bool {
    let Some(item) = call.get(item_key) else {
        return false;
    };
    if !call_hierarchy_item_shape_is_valid(item) {
        return false;
    }

    let Some(ranges) = call.get("fromRanges").and_then(Value::as_array) else {
        return false;
    };
    ranges
        .iter()
        .take(MAX_EMITTED_CALL_HIERARCHY_RANGES)
        .all(lsp_range_shape_is_valid)
}

fn call_hierarchy_prepare_item_shape_is_valid(item: &Value) -> bool {
    item.get("name")
        .and_then(Value::as_str)
        .is_some_and(|name| !name.trim().is_empty())
        && item
            .get("kind")
            .and_then(Value::as_u64)
            .is_some_and(|kind| u8::try_from(kind).is_ok())
        && item
            .get("uri")
            .and_then(Value::as_str)
            .and_then(file_uri_to_path)
            .is_some()
        && item.get("range").is_none_or(lsp_range_shape_is_valid)
        && item
            .get("selectionRange")
            .or_else(|| item.get("range"))
            .is_some_and(lsp_range_shape_is_valid)
        && item.get("detail").is_none_or(Value::is_string)
        && call_hierarchy_item_payload_is_bounded(item)
}

fn call_hierarchy_item_shape_is_valid(item: &Value) -> bool {
    item.get("name")
        .and_then(Value::as_str)
        .is_some_and(|name| !name.trim().is_empty())
        && item
            .get("kind")
            .and_then(Value::as_u64)
            .is_some_and(|kind| u8::try_from(kind).is_ok())
        && item
            .get("uri")
            .and_then(Value::as_str)
            .and_then(file_uri_to_path)
            .is_some()
        && item.get("range").is_some_and(lsp_range_shape_is_valid)
        && item
            .get("selectionRange")
            .is_some_and(lsp_range_shape_is_valid)
        && item.get("detail").is_none_or(Value::is_string)
        && call_hierarchy_item_payload_is_bounded(item)
}

fn lsp_range_shape_is_valid(range: &Value) -> bool {
    let Some((start_line, start_character)) = range.get("start").and_then(lsp_position_shape)
    else {
        return false;
    };
    let Some((end_line, end_character)) = range.get("end").and_then(lsp_position_shape) else {
        return false;
    };

    end_line > start_line || (end_line == start_line && end_character >= start_character)
}

fn lsp_position_shape(position: &Value) -> Option<(usize, usize)> {
    Some((
        lsp_position_component(position.get("line")?)?,
        lsp_position_component(position.get("character")?)?,
    ))
}

fn lsp_position_component(value: &Value) -> Option<usize> {
    let component = value.as_u64()?;
    if component > MAX_VALIDATED_CALL_HIERARCHY_POSITION_COMPONENT {
        return None;
    }

    let component = usize::try_from(component).ok()?;
    component.checked_add(1)?;
    Some(component)
}

fn call_hierarchy_item_payload_is_bounded(value: &Value) -> bool {
    let mut counter = CountingWriter::new(MAX_VALIDATED_CALL_HIERARCHY_ITEM_PAYLOAD_BYTES);
    serde_json::to_writer(&mut counter, value).is_ok()
}

struct CountingWriter {
    bytes: usize,
    max_bytes: usize,
}

impl CountingWriter {
    fn new(max_bytes: usize) -> Self {
        Self {
            bytes: 0,
            max_bytes,
        }
    }
}

impl Write for CountingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let next = self.bytes.saturating_add(buf.len());
        if next > self.max_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "call hierarchy item payload too large",
            ));
        }
        self.bytes = next;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CountingWriter, INVALID_CALL_HIERARCHY_INCOMING_RESPONSE,
        INVALID_CALL_HIERARCHY_OUTGOING_RESPONSE, INVALID_PREPARE_CALL_HIERARCHY_RESPONSE,
        MAX_EMITTED_CALL_HIERARCHY_CALLS, MAX_EMITTED_CALL_HIERARCHY_ITEMS,
        MAX_EMITTED_CALL_HIERARCHY_RANGES, MAX_VALIDATED_CALL_HIERARCHY_ITEM_PAYLOAD_BYTES,
        MAX_VALIDATED_CALL_HIERARCHY_POSITION_COMPONENT, call_hierarchy_item_payload_is_bounded,
        handle_call_hierarchy_response,
    };
    use crate::lsp_client::pending::PendingLspRequest;
    use crate::{lsp_ui_events::LspUiEvent, ui_events::UiEvent};
    use crossbeam_channel::TryRecvError;
    use kuroya_core::LspCallHierarchyItem;
    use serde_json::json;
    use std::{io::Write, path::PathBuf};

    fn item(name: &str) -> LspCallHierarchyItem {
        LspCallHierarchyItem {
            name: name.to_owned(),
            detail: None,
            kind: 12,
            path: PathBuf::from("src/main.rs"),
            line: 2,
            column: 4,
            end_line: 2,
            end_column: 8,
            raw: json!({ "name": name }),
        }
    }

    fn response_item(name: &str) -> serde_json::Value {
        json!({
            "name": name,
            "detail": "fn handler()",
            "kind": 12,
            "uri": "file:///src/main.rs",
            "range": {
                "start": { "line": 1, "character": 0 },
                "end": { "line": 3, "character": 1 }
            },
            "selectionRange": {
                "start": { "line": 1, "character": 4 },
                "end": { "line": 1, "character": 11 }
            }
        })
    }

    fn response_range() -> serde_json::Value {
        json!({
            "start": { "line": 5, "character": 8 },
            "end": { "line": 5, "character": 14 }
        })
    }

    fn over_bound_range() -> serde_json::Value {
        let over_bound_component = MAX_VALIDATED_CALL_HIERARCHY_POSITION_COMPONENT + 1;
        json!({
            "start": { "line": over_bound_component, "character": 0 },
            "end": { "line": over_bound_component, "character": 0 }
        })
    }

    #[test]
    fn call_hierarchy_item_payload_accepts_exact_bound_serialized_json() {
        let value = serde_json::Value::String(
            "x".repeat(MAX_VALIDATED_CALL_HIERARCHY_ITEM_PAYLOAD_BYTES - 2),
        );

        assert!(call_hierarchy_item_payload_is_bounded(&value));
    }

    #[test]
    fn call_hierarchy_item_payload_rejects_over_bound_serialized_json() {
        let value = serde_json::Value::String(
            "x".repeat(MAX_VALIDATED_CALL_HIERARCHY_ITEM_PAYLOAD_BYTES - 1),
        );

        assert!(!call_hierarchy_item_payload_is_bounded(&value));
    }

    #[test]
    fn call_hierarchy_payload_counter_errors_after_limit() {
        let mut writer = CountingWriter::new(4);

        writer.write_all(b"1234").expect("exact-limit write");

        assert!(writer.write_all(b"5").is_err());
    }

    #[test]
    fn call_hierarchy_response_handler_ignores_misrouted_pending_request() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        handle_call_hierarchy_response(
            PendingLspRequest::Formatting {
                request_id: 23,
                id: 23,
                path: PathBuf::from("src/main.rs"),
                version: 6,
            },
            json!({ "result": [] }),
            &tx,
        );

        assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
    }

    #[test]
    fn call_hierarchy_prepare_reports_invalid_success_payload() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        handle_call_hierarchy_response(
            PendingLspRequest::PrepareCallHierarchy {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                line: 3,
                character: 12,
            },
            json!({ "result": { "not": "a list" } }),
            &tx,
        );

        match rx.recv().expect("call hierarchy prepare event") {
            UiEvent::Lsp(LspUiEvent::CallHierarchyPrepared {
                id,
                path,
                version,
                line,
                column,
                items,
                error,
            }) => {
                assert_eq!(id, 7);
                assert_eq!(path, PathBuf::from("src/main.rs"));
                assert_eq!(version, 11);
                assert_eq!(line, 3);
                assert_eq!(column, 12);
                assert_eq!(items, None);
                assert_eq!(
                    error.as_deref(),
                    Some(INVALID_PREPARE_CALL_HIERARCHY_RESPONSE)
                );
            }
            other => panic!("expected call hierarchy prepare event, got {other:?}"),
        }
    }

    #[test]
    fn call_hierarchy_prepare_reports_invalid_success_item() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let mut invalid_item = response_item("handler");
        invalid_item["selectionRange"] = json!({
            "start": { "line": 1 },
            "end": { "line": 1, "character": 11 }
        });

        handle_call_hierarchy_response(
            PendingLspRequest::PrepareCallHierarchy {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                line: 3,
                character: 12,
            },
            json!({ "result": [invalid_item] }),
            &tx,
        );

        match rx.recv().expect("call hierarchy prepare event") {
            UiEvent::Lsp(LspUiEvent::CallHierarchyPrepared { items, error, .. }) => {
                assert_eq!(items, None);
                assert_eq!(
                    error.as_deref(),
                    Some(INVALID_PREPARE_CALL_HIERARCHY_RESPONSE)
                );
            }
            other => panic!("expected call hierarchy prepare event, got {other:?}"),
        }
    }

    #[test]
    fn call_hierarchy_prepare_reports_invalid_present_range_even_with_selection_range() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let mut invalid_item = response_item("handler");
        invalid_item["range"] = json!({
            "start": { "line": 3, "character": 5 },
            "end": { "line": 3, "character": 4 }
        });

        handle_call_hierarchy_response(
            PendingLspRequest::PrepareCallHierarchy {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                line: 3,
                character: 12,
            },
            json!({ "result": [invalid_item] }),
            &tx,
        );

        match rx.recv().expect("call hierarchy prepare event") {
            UiEvent::Lsp(LspUiEvent::CallHierarchyPrepared { items, error, .. }) => {
                assert_eq!(items, None);
                assert_eq!(
                    error.as_deref(),
                    Some(INVALID_PREPARE_CALL_HIERARCHY_RESPONSE)
                );
            }
            other => panic!("expected call hierarchy prepare event, got {other:?}"),
        }
    }

    #[test]
    fn call_hierarchy_prepare_reports_over_bound_item_range() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let mut invalid_item = response_item("handler");
        invalid_item["range"] = over_bound_range();

        handle_call_hierarchy_response(
            PendingLspRequest::PrepareCallHierarchy {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                line: 3,
                character: 12,
            },
            json!({ "result": [invalid_item] }),
            &tx,
        );

        match rx.recv().expect("call hierarchy prepare event") {
            UiEvent::Lsp(LspUiEvent::CallHierarchyPrepared { items, error, .. }) => {
                assert_eq!(items, None);
                assert_eq!(
                    error.as_deref(),
                    Some(INVALID_PREPARE_CALL_HIERARCHY_RESPONSE)
                );
            }
            other => panic!("expected call hierarchy prepare event, got {other:?}"),
        }
    }

    #[test]
    fn call_hierarchy_prepare_reports_out_of_range_kind() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let mut invalid_item = response_item("handler");
        invalid_item["kind"] = json!(u16::MAX);

        handle_call_hierarchy_response(
            PendingLspRequest::PrepareCallHierarchy {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                line: 3,
                character: 12,
            },
            json!({ "result": [invalid_item] }),
            &tx,
        );

        match rx.recv().expect("call hierarchy prepare event") {
            UiEvent::Lsp(LspUiEvent::CallHierarchyPrepared { items, error, .. }) => {
                assert_eq!(items, None);
                assert_eq!(
                    error.as_deref(),
                    Some(INVALID_PREPARE_CALL_HIERARCHY_RESPONSE)
                );
            }
            other => panic!("expected call hierarchy prepare event, got {other:?}"),
        }
    }

    #[test]
    fn call_hierarchy_prepare_reports_oversized_raw_payload() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let mut invalid_item = response_item("handler");
        invalid_item["data"] = json!({
            "blob": "x".repeat(MAX_VALIDATED_CALL_HIERARCHY_ITEM_PAYLOAD_BYTES + 1)
        });

        handle_call_hierarchy_response(
            PendingLspRequest::PrepareCallHierarchy {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                line: 3,
                character: 12,
            },
            json!({ "result": [invalid_item] }),
            &tx,
        );

        match rx.recv().expect("call hierarchy prepare event") {
            UiEvent::Lsp(LspUiEvent::CallHierarchyPrepared { items, error, .. }) => {
                assert_eq!(items, None);
                assert_eq!(
                    error.as_deref(),
                    Some(INVALID_PREPARE_CALL_HIERARCHY_RESPONSE)
                );
            }
            other => panic!("expected call hierarchy prepare event, got {other:?}"),
        }
    }

    #[test]
    fn call_hierarchy_prepare_sends_empty_list_for_null_success() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        handle_call_hierarchy_response(
            PendingLspRequest::PrepareCallHierarchy {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                line: 3,
                character: 12,
            },
            json!({ "result": null }),
            &tx,
        );

        match rx.recv().expect("call hierarchy prepare event") {
            UiEvent::Lsp(LspUiEvent::CallHierarchyPrepared { items, error, .. }) => {
                assert_eq!(items, Some(Vec::new()));
                assert_eq!(error, None);
            }
            other => panic!("expected call hierarchy prepare event, got {other:?}"),
        }
    }

    #[test]
    fn call_hierarchy_prepare_sends_parsed_items() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        handle_call_hierarchy_response(
            PendingLspRequest::PrepareCallHierarchy {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                line: 3,
                character: 12,
            },
            json!({ "result": [response_item("handler")] }),
            &tx,
        );

        match rx.recv().expect("call hierarchy prepare event") {
            UiEvent::Lsp(LspUiEvent::CallHierarchyPrepared { items, error, .. }) => {
                let items = items.expect("parsed call hierarchy items");
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].name, "handler");
                assert_eq!(items[0].line, 2);
                assert_eq!(items[0].column, 5);
                assert_eq!(error, None);
            }
            other => panic!("expected call hierarchy prepare event, got {other:?}"),
        }
    }

    #[test]
    fn call_hierarchy_prepare_caps_emitted_items() {
        let mut result = (0..=MAX_EMITTED_CALL_HIERARCHY_ITEMS)
            .map(|index| response_item(&format!("handler{index:03}")))
            .collect::<Vec<_>>();
        result[MAX_EMITTED_CALL_HIERARCHY_ITEMS]["selectionRange"] = over_bound_range();

        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        handle_call_hierarchy_response(
            PendingLspRequest::PrepareCallHierarchy {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                line: 3,
                character: 12,
            },
            json!({ "result": result }),
            &tx,
        );

        match rx.recv().expect("call hierarchy prepare event") {
            UiEvent::Lsp(LspUiEvent::CallHierarchyPrepared { items, error, .. }) => {
                let items = items.expect("capped call hierarchy items");
                assert_eq!(items.len(), MAX_EMITTED_CALL_HIERARCHY_ITEMS);
                assert_eq!(error, None);
            }
            other => panic!("expected call hierarchy prepare event, got {other:?}"),
        }
    }

    #[test]
    fn call_hierarchy_incoming_reports_invalid_success_payload() {
        let root = item("root");
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        handle_call_hierarchy_response(
            PendingLspRequest::CallHierarchyIncoming {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                item: root.clone(),
            },
            json!({ "result": { "not": "a list" } }),
            &tx,
        );

        match rx.recv().expect("call hierarchy incoming event") {
            UiEvent::Lsp(LspUiEvent::CallHierarchyIncomingResult {
                id,
                path,
                version,
                item,
                calls,
                error,
            }) => {
                assert_eq!(id, 7);
                assert_eq!(path, PathBuf::from("src/main.rs"));
                assert_eq!(version, 11);
                assert_eq!(item, root);
                assert_eq!(calls, None);
                assert_eq!(
                    error.as_deref(),
                    Some(INVALID_CALL_HIERARCHY_INCOMING_RESPONSE)
                );
            }
            other => panic!("expected call hierarchy incoming event, got {other:?}"),
        }
    }

    #[test]
    fn call_hierarchy_incoming_reports_invalid_success_range() {
        let root = item("root");
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        handle_call_hierarchy_response(
            PendingLspRequest::CallHierarchyIncoming {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                item: root,
            },
            json!({
                "result": [{
                    "from": response_item("caller"),
                    "fromRanges": [{
                        "start": { "line": 5, "character": 8 },
                        "end": { "line": 5 }
                    }]
                }]
            }),
            &tx,
        );

        match rx.recv().expect("call hierarchy incoming event") {
            UiEvent::Lsp(LspUiEvent::CallHierarchyIncomingResult { calls, error, .. }) => {
                assert_eq!(calls, None);
                assert_eq!(
                    error.as_deref(),
                    Some(INVALID_CALL_HIERARCHY_INCOMING_RESPONSE)
                );
            }
            other => panic!("expected call hierarchy incoming event, got {other:?}"),
        }
    }

    #[test]
    fn call_hierarchy_incoming_reports_reversed_success_range() {
        let root = item("root");
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        handle_call_hierarchy_response(
            PendingLspRequest::CallHierarchyIncoming {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                item: root,
            },
            json!({
                "result": [{
                    "from": response_item("caller"),
                    "fromRanges": [{
                        "start": { "line": 5, "character": 14 },
                        "end": { "line": 5, "character": 8 }
                    }]
                }]
            }),
            &tx,
        );

        match rx.recv().expect("call hierarchy incoming event") {
            UiEvent::Lsp(LspUiEvent::CallHierarchyIncomingResult { calls, error, .. }) => {
                assert_eq!(calls, None);
                assert_eq!(
                    error.as_deref(),
                    Some(INVALID_CALL_HIERARCHY_INCOMING_RESPONSE)
                );
            }
            other => panic!("expected call hierarchy incoming event, got {other:?}"),
        }
    }

    #[test]
    fn call_hierarchy_incoming_reports_over_bound_success_range() {
        let root = item("root");
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        handle_call_hierarchy_response(
            PendingLspRequest::CallHierarchyIncoming {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                item: root,
            },
            json!({
                "result": [{
                    "from": response_item("caller"),
                    "fromRanges": [over_bound_range()]
                }]
            }),
            &tx,
        );

        match rx.recv().expect("call hierarchy incoming event") {
            UiEvent::Lsp(LspUiEvent::CallHierarchyIncomingResult { calls, error, .. }) => {
                assert_eq!(calls, None);
                assert_eq!(
                    error.as_deref(),
                    Some(INVALID_CALL_HIERARCHY_INCOMING_RESPONSE)
                );
            }
            other => panic!("expected call hierarchy incoming event, got {other:?}"),
        }
    }

    #[test]
    fn call_hierarchy_incoming_reports_oversized_call_item_payload() {
        let root = item("root");
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let mut invalid_item = response_item("caller");
        invalid_item["data"] = json!({
            "blob": "x".repeat(MAX_VALIDATED_CALL_HIERARCHY_ITEM_PAYLOAD_BYTES + 1)
        });

        handle_call_hierarchy_response(
            PendingLspRequest::CallHierarchyIncoming {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                item: root,
            },
            json!({
                "result": [{
                    "from": invalid_item,
                    "fromRanges": [response_range()]
                }]
            }),
            &tx,
        );

        match rx.recv().expect("call hierarchy incoming event") {
            UiEvent::Lsp(LspUiEvent::CallHierarchyIncomingResult { calls, error, .. }) => {
                assert_eq!(calls, None);
                assert_eq!(
                    error.as_deref(),
                    Some(INVALID_CALL_HIERARCHY_INCOMING_RESPONSE)
                );
            }
            other => panic!("expected call hierarchy incoming event, got {other:?}"),
        }
    }

    #[test]
    fn call_hierarchy_incoming_sends_empty_list_for_null_success() {
        let root = item("root");
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        handle_call_hierarchy_response(
            PendingLspRequest::CallHierarchyIncoming {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                item: root,
            },
            json!({ "result": null }),
            &tx,
        );

        match rx.recv().expect("call hierarchy incoming event") {
            UiEvent::Lsp(LspUiEvent::CallHierarchyIncomingResult { calls, error, .. }) => {
                assert_eq!(calls, Some(Vec::new()));
                assert_eq!(error, None);
            }
            other => panic!("expected call hierarchy incoming event, got {other:?}"),
        }
    }

    #[test]
    fn call_hierarchy_incoming_sends_parsed_calls() {
        let root = item("root");
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        handle_call_hierarchy_response(
            PendingLspRequest::CallHierarchyIncoming {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                item: root,
            },
            json!({
                "result": [{
                    "from": response_item("caller"),
                    "fromRanges": [response_range()]
                }]
            }),
            &tx,
        );

        match rx.recv().expect("call hierarchy incoming event") {
            UiEvent::Lsp(LspUiEvent::CallHierarchyIncomingResult { calls, error, .. }) => {
                let calls = calls.expect("parsed incoming calls");
                assert_eq!(calls.len(), 1);
                assert_eq!(calls[0].item.name, "caller");
                assert_eq!(calls[0].ranges.len(), 1);
                assert_eq!(calls[0].ranges[0].line, 6);
                assert_eq!(calls[0].ranges[0].column, 9);
                assert_eq!(error, None);
            }
            other => panic!("expected call hierarchy incoming event, got {other:?}"),
        }
    }

    #[test]
    fn call_hierarchy_incoming_caps_emitted_calls() {
        let root = item("root");
        let mut result = (0..=MAX_EMITTED_CALL_HIERARCHY_CALLS)
            .map(|index| {
                json!({
                "from": response_item(&format!("caller{index:03}")),
                "fromRanges": [response_range()]
                })
            })
            .collect::<Vec<_>>();
        result[MAX_EMITTED_CALL_HIERARCHY_CALLS]["fromRanges"] = json!([over_bound_range()]);

        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        handle_call_hierarchy_response(
            PendingLspRequest::CallHierarchyIncoming {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                item: root,
            },
            json!({ "result": result }),
            &tx,
        );

        match rx.recv().expect("call hierarchy incoming event") {
            UiEvent::Lsp(LspUiEvent::CallHierarchyIncomingResult { calls, error, .. }) => {
                let calls = calls.expect("capped incoming calls");
                assert_eq!(calls.len(), MAX_EMITTED_CALL_HIERARCHY_CALLS);
                assert_eq!(error, None);
            }
            other => panic!("expected call hierarchy incoming event, got {other:?}"),
        }
    }

    #[test]
    fn call_hierarchy_incoming_caps_emitted_ranges() {
        let root = item("root");
        let mut ranges = (0..=MAX_EMITTED_CALL_HIERARCHY_RANGES)
            .map(|_| response_range())
            .collect::<Vec<_>>();
        ranges[MAX_EMITTED_CALL_HIERARCHY_RANGES] = over_bound_range();

        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        handle_call_hierarchy_response(
            PendingLspRequest::CallHierarchyIncoming {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                item: root,
            },
            json!({
                "result": [{
                    "from": response_item("caller"),
                    "fromRanges": ranges
                }]
            }),
            &tx,
        );

        match rx.recv().expect("call hierarchy incoming event") {
            UiEvent::Lsp(LspUiEvent::CallHierarchyIncomingResult { calls, error, .. }) => {
                let calls = calls.expect("incoming calls with capped ranges");
                assert_eq!(calls.len(), 1);
                assert_eq!(calls[0].ranges.len(), MAX_EMITTED_CALL_HIERARCHY_RANGES);
                assert_eq!(error, None);
            }
            other => panic!("expected call hierarchy incoming event, got {other:?}"),
        }
    }

    #[test]
    fn call_hierarchy_outgoing_reports_invalid_success_payload() {
        let root = item("root");
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        handle_call_hierarchy_response(
            PendingLspRequest::CallHierarchyOutgoing {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                item: root.clone(),
            },
            json!({ "result": { "not": "a list" } }),
            &tx,
        );

        match rx.recv().expect("call hierarchy outgoing event") {
            UiEvent::Lsp(LspUiEvent::CallHierarchyOutgoingResult {
                id,
                path,
                version,
                item,
                calls,
                error,
            }) => {
                assert_eq!(id, 7);
                assert_eq!(path, PathBuf::from("src/main.rs"));
                assert_eq!(version, 11);
                assert_eq!(item, root);
                assert_eq!(calls, None);
                assert_eq!(
                    error.as_deref(),
                    Some(INVALID_CALL_HIERARCHY_OUTGOING_RESPONSE)
                );
            }
            other => panic!("expected call hierarchy outgoing event, got {other:?}"),
        }
    }

    #[test]
    fn call_hierarchy_outgoing_reports_invalid_success_call_item() {
        let root = item("root");
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let mut invalid_item = response_item("callee");
        invalid_item["kind"] = json!("method");

        handle_call_hierarchy_response(
            PendingLspRequest::CallHierarchyOutgoing {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                item: root,
            },
            json!({
                "result": [{
                    "to": invalid_item,
                    "fromRanges": [response_range()]
                }]
            }),
            &tx,
        );

        match rx.recv().expect("call hierarchy outgoing event") {
            UiEvent::Lsp(LspUiEvent::CallHierarchyOutgoingResult { calls, error, .. }) => {
                assert_eq!(calls, None);
                assert_eq!(
                    error.as_deref(),
                    Some(INVALID_CALL_HIERARCHY_OUTGOING_RESPONSE)
                );
            }
            other => panic!("expected call hierarchy outgoing event, got {other:?}"),
        }
    }

    #[test]
    fn call_hierarchy_outgoing_sends_empty_list_for_null_success() {
        let root = item("root");
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        handle_call_hierarchy_response(
            PendingLspRequest::CallHierarchyOutgoing {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                item: root,
            },
            json!({ "result": null }),
            &tx,
        );

        match rx.recv().expect("call hierarchy outgoing event") {
            UiEvent::Lsp(LspUiEvent::CallHierarchyOutgoingResult { calls, error, .. }) => {
                assert_eq!(calls, Some(Vec::new()));
                assert_eq!(error, None);
            }
            other => panic!("expected call hierarchy outgoing event, got {other:?}"),
        }
    }
}
