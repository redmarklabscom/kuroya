use super::super::response_error;
use crate::ui_event_channel::Sender;
use crate::{
    lsp_client::pending::PendingLspRequest, lsp_ui_events::LspUiEvent, ui_events::UiEvent,
};
use kuroya_core::{
    LspTypeHierarchyItem, lsp::file_uri_to_path, parse_type_hierarchy_prepare_response,
    parse_type_hierarchy_subtypes_response, parse_type_hierarchy_supertypes_response,
};
use serde_json::Value;
use std::io::{self, Write};

const INVALID_PREPARE_TYPE_HIERARCHY_RESPONSE: &str =
    "invalid textDocument/prepareTypeHierarchy response";
const INVALID_TYPE_HIERARCHY_SUPERTYPES_RESPONSE: &str =
    "invalid typeHierarchy/supertypes response";
const INVALID_TYPE_HIERARCHY_SUBTYPES_RESPONSE: &str = "invalid typeHierarchy/subtypes response";
const MAX_EMITTED_TYPE_HIERARCHY_PREPARE_ITEMS: usize = 100;
const MAX_EMITTED_TYPE_HIERARCHY_RELATION_ITEMS: usize = 500;
const MAX_VALIDATED_TYPE_HIERARCHY_ITEM_PAYLOAD_BYTES: usize = 64 * 1024;
const MAX_VALIDATED_TYPE_HIERARCHY_POSITION_COMPONENT: u64 = i32::MAX as u64;

pub(super) fn handle_type_hierarchy_response(
    pending: PendingLspRequest,
    value: Value,
    ui_tx: &Sender<UiEvent>,
) {
    let mut error = response_error(&value);
    match pending {
        PendingLspRequest::PrepareTypeHierarchy {
            id,
            path,
            version,
            line,
            character,
        } => {
            let items = if error.is_none() {
                match parse_type_hierarchy_prepare_success_response(&value) {
                    Some(items) => Some(items),
                    None => {
                        error = Some(INVALID_PREPARE_TYPE_HIERARCHY_RESPONSE.to_owned());
                        None
                    }
                }
            } else {
                None
            };
            let _ = crate::lsp_client::response::emit_critical_lsp_response_event(
                ui_tx,
                LspUiEvent::TypeHierarchyPrepared {
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
        PendingLspRequest::TypeHierarchySupertypes {
            id,
            path,
            version,
            item,
        } => {
            let supertypes = if error.is_none() {
                match parse_type_hierarchy_supertypes_success_response(&value) {
                    Some(supertypes) => Some(supertypes),
                    None => {
                        error = Some(INVALID_TYPE_HIERARCHY_SUPERTYPES_RESPONSE.to_owned());
                        None
                    }
                }
            } else {
                None
            };
            let _ = crate::lsp_client::response::emit_critical_lsp_response_event(
                ui_tx,
                LspUiEvent::TypeHierarchySupertypesResult {
                    id,
                    path,
                    version,
                    item,
                    supertypes,
                    error,
                },
            );
        }
        PendingLspRequest::TypeHierarchySubtypes {
            id,
            path,
            version,
            item,
        } => {
            let subtypes = if error.is_none() {
                match parse_type_hierarchy_subtypes_success_response(&value) {
                    Some(subtypes) => Some(subtypes),
                    None => {
                        error = Some(INVALID_TYPE_HIERARCHY_SUBTYPES_RESPONSE.to_owned());
                        None
                    }
                }
            } else {
                None
            };
            let _ = crate::lsp_client::response::emit_critical_lsp_response_event(
                ui_tx,
                LspUiEvent::TypeHierarchySubtypesResult {
                    id,
                    path,
                    version,
                    item,
                    subtypes,
                    error,
                },
            );
        }
        _ => {}
    }
}

fn parse_type_hierarchy_prepare_success_response(
    value: &Value,
) -> Option<Vec<LspTypeHierarchyItem>> {
    parse_type_hierarchy_items_success_response(
        value,
        parse_type_hierarchy_prepare_response,
        MAX_EMITTED_TYPE_HIERARCHY_PREPARE_ITEMS,
    )
}

fn parse_type_hierarchy_supertypes_success_response(
    value: &Value,
) -> Option<Vec<LspTypeHierarchyItem>> {
    parse_type_hierarchy_items_success_response(
        value,
        parse_type_hierarchy_supertypes_response,
        MAX_EMITTED_TYPE_HIERARCHY_RELATION_ITEMS,
    )
}

fn parse_type_hierarchy_subtypes_success_response(
    value: &Value,
) -> Option<Vec<LspTypeHierarchyItem>> {
    parse_type_hierarchy_items_success_response(
        value,
        parse_type_hierarchy_subtypes_response,
        MAX_EMITTED_TYPE_HIERARCHY_RELATION_ITEMS,
    )
}

fn parse_type_hierarchy_items_success_response(
    value: &Value,
    parser: fn(&Value) -> Option<Vec<LspTypeHierarchyItem>>,
    max_items: usize,
) -> Option<Vec<LspTypeHierarchyItem>> {
    let result = value.get("result")?;
    if result.is_null() {
        return Some(Vec::new());
    }

    let items = result.as_array()?;
    if items
        .iter()
        .take(max_items)
        .any(|item| !type_hierarchy_item_is_parseable(item))
    {
        return None;
    }

    let mut items = parser(value)?;
    items.truncate(max_items);
    Some(items)
}

fn type_hierarchy_item_is_parseable(item: &Value) -> bool {
    is_valid_type_hierarchy_item_shape(item)
}

fn is_valid_type_hierarchy_item_shape(value: &Value) -> bool {
    value
        .get("name")
        .and_then(Value::as_str)
        .is_some_and(|name| !name.trim().is_empty())
        && value
            .get("kind")
            .and_then(Value::as_u64)
            .is_some_and(|kind| u8::try_from(kind).is_ok())
        && value
            .get("uri")
            .and_then(Value::as_str)
            .and_then(file_uri_to_path)
            .is_some()
        && is_valid_type_hierarchy_range(value.get("range"))
        && is_valid_type_hierarchy_range(value.get("selectionRange"))
        && value.get("detail").is_none_or(Value::is_string)
        && type_hierarchy_item_payload_is_bounded(value)
}

fn is_valid_type_hierarchy_range(value: Option<&Value>) -> bool {
    let Some(value) = value else {
        return false;
    };
    let Some(start) = value.get("start") else {
        return false;
    };
    let Some(end) = value.get("end") else {
        return false;
    };
    let Some(start_line) = start.get("line").and_then(lsp_position_component) else {
        return false;
    };
    let Some(start_character) = start.get("character").and_then(lsp_position_component) else {
        return false;
    };
    let Some(end_line) = end.get("line").and_then(lsp_position_component) else {
        return false;
    };
    let Some(end_character) = end.get("character").and_then(lsp_position_component) else {
        return false;
    };

    end_line > start_line || (end_line == start_line && end_character >= start_character)
}

fn lsp_position_component(value: &Value) -> Option<usize> {
    let component = value.as_u64()?;
    if component > MAX_VALIDATED_TYPE_HIERARCHY_POSITION_COMPONENT {
        return None;
    }
    usize::try_from(component).ok()
}

fn type_hierarchy_item_payload_is_bounded(value: &Value) -> bool {
    let mut counter = CountingWriter::new(MAX_VALIDATED_TYPE_HIERARCHY_ITEM_PAYLOAD_BYTES);
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
                "type hierarchy item payload too large",
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
        CountingWriter, INVALID_PREPARE_TYPE_HIERARCHY_RESPONSE,
        INVALID_TYPE_HIERARCHY_SUBTYPES_RESPONSE, INVALID_TYPE_HIERARCHY_SUPERTYPES_RESPONSE,
        MAX_EMITTED_TYPE_HIERARCHY_PREPARE_ITEMS, MAX_EMITTED_TYPE_HIERARCHY_RELATION_ITEMS,
        MAX_VALIDATED_TYPE_HIERARCHY_ITEM_PAYLOAD_BYTES,
        MAX_VALIDATED_TYPE_HIERARCHY_POSITION_COMPONENT, handle_type_hierarchy_response,
        type_hierarchy_item_payload_is_bounded,
    };
    use crate::lsp_client::pending::PendingLspRequest;
    use crate::{lsp_ui_events::LspUiEvent, ui_events::UiEvent};
    use crossbeam_channel::TryRecvError;
    use kuroya_core::LspTypeHierarchyItem;
    use serde_json::json;
    use std::{io::Write, path::PathBuf};

    fn item(name: &str) -> LspTypeHierarchyItem {
        LspTypeHierarchyItem {
            name: name.to_owned(),
            detail: None,
            kind: 5,
            path: PathBuf::from("src/main.rs"),
            line: 2,
            column: 4,
            end_line: 2,
            end_column: 8,
            raw: json!({ "name": name }),
        }
    }

    fn wire_item(name: &str, uri: &str) -> serde_json::Value {
        json!({
            "name": name,
            "kind": 5,
            "uri": uri,
            "range": {
                "start": { "line": 1, "character": 0 },
                "end": { "line": 4, "character": 1 }
            },
            "selectionRange": {
                "start": { "line": 2, "character": 4 },
                "end": { "line": 2, "character": 10 }
            },
            "data": { "token": name }
        })
    }

    #[test]
    fn type_hierarchy_item_payload_bound_accepts_valid_payload() {
        assert!(type_hierarchy_item_payload_is_bounded(&json!({
            "name": "Widget",
            "data": { "token": "small" }
        })));
    }

    #[test]
    fn type_hierarchy_item_payload_bound_accepts_exact_limit() {
        let value = serde_json::Value::String(
            "x".repeat(MAX_VALIDATED_TYPE_HIERARCHY_ITEM_PAYLOAD_BYTES - 2),
        );
        assert_eq!(
            serde_json::to_string(&value)
                .expect("exact-bound value serializes")
                .len(),
            MAX_VALIDATED_TYPE_HIERARCHY_ITEM_PAYLOAD_BYTES
        );

        assert!(type_hierarchy_item_payload_is_bounded(&value));
    }

    #[test]
    fn type_hierarchy_item_payload_bound_rejects_over_limit() {
        let value = serde_json::Value::String(
            "x".repeat(MAX_VALIDATED_TYPE_HIERARCHY_ITEM_PAYLOAD_BYTES - 1),
        );
        assert_eq!(
            serde_json::to_string(&value)
                .expect("over-bound value serializes")
                .len(),
            MAX_VALIDATED_TYPE_HIERARCHY_ITEM_PAYLOAD_BYTES + 1
        );

        assert!(!type_hierarchy_item_payload_is_bounded(&value));
    }

    #[test]
    fn counting_writer_stops_at_type_hierarchy_payload_limit() {
        let mut writer = CountingWriter::new(4);

        writer
            .write_all(b"rust")
            .expect("exactly bounded write should succeed");
        assert!(writer.write_all(b"!").is_err());
        assert_eq!(writer.bytes, 4);
    }

    #[test]
    fn type_hierarchy_response_handler_ignores_misrouted_pending_request() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        handle_type_hierarchy_response(
            PendingLspRequest::CodeActions {
                id: 29,
                path: PathBuf::from("src/main.rs"),
                version: 7,
                line: 2,
                character: 4,
            },
            json!({ "result": [] }),
            &tx,
        );

        assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
    }

    #[test]
    fn type_hierarchy_prepare_reports_invalid_success_payload() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        handle_type_hierarchy_response(
            PendingLspRequest::PrepareTypeHierarchy {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                line: 3,
                character: 12,
            },
            json!({ "result": { "not": "a list" } }),
            &tx,
        );

        match rx.recv().expect("type hierarchy prepare event") {
            UiEvent::Lsp(LspUiEvent::TypeHierarchyPrepared {
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
                    Some(INVALID_PREPARE_TYPE_HIERARCHY_RESPONSE)
                );
            }
            other => panic!("expected type hierarchy prepare event, got {other:?}"),
        }
    }

    #[test]
    fn type_hierarchy_prepare_reports_malformed_success_item() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        handle_type_hierarchy_response(
            PendingLspRequest::PrepareTypeHierarchy {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                line: 3,
                character: 12,
            },
            json!({
                "result": [{
                    "name": "Widget",
                    "kind": 5,
                    "uri": "file:///src/widget.rs",
                    "selectionRange": {
                        "start": { "line": 2, "character": 4 },
                        "end": { "line": 2, "character": 10 }
                    }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("type hierarchy prepare event") {
            UiEvent::Lsp(LspUiEvent::TypeHierarchyPrepared { items, error, .. }) => {
                assert_eq!(items, None);
                assert_eq!(
                    error.as_deref(),
                    Some(INVALID_PREPARE_TYPE_HIERARCHY_RESPONSE)
                );
            }
            other => panic!("expected type hierarchy prepare event, got {other:?}"),
        }
    }

    #[test]
    fn type_hierarchy_prepare_reports_out_of_range_kind() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let mut invalid_item = wire_item("Widget", "file:///src/widget.rs");
        invalid_item["kind"] = json!(u16::MAX);

        handle_type_hierarchy_response(
            PendingLspRequest::PrepareTypeHierarchy {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                line: 3,
                character: 12,
            },
            json!({ "result": [invalid_item] }),
            &tx,
        );

        match rx.recv().expect("type hierarchy prepare event") {
            UiEvent::Lsp(LspUiEvent::TypeHierarchyPrepared { items, error, .. }) => {
                assert_eq!(items, None);
                assert_eq!(
                    error.as_deref(),
                    Some(INVALID_PREPARE_TYPE_HIERARCHY_RESPONSE)
                );
            }
            other => panic!("expected type hierarchy prepare event, got {other:?}"),
        }
    }

    #[test]
    fn type_hierarchy_prepare_reports_coordinate_above_core_bound() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let mut invalid_item = wire_item("Widget", "file:///src/widget.rs");
        invalid_item["range"]["start"]["line"] =
            json!(MAX_VALIDATED_TYPE_HIERARCHY_POSITION_COMPONENT + 1);

        handle_type_hierarchy_response(
            PendingLspRequest::PrepareTypeHierarchy {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                line: 3,
                character: 12,
            },
            json!({ "result": [invalid_item] }),
            &tx,
        );

        match rx.recv().expect("type hierarchy prepare event") {
            UiEvent::Lsp(LspUiEvent::TypeHierarchyPrepared { items, error, .. }) => {
                assert_eq!(items, None);
                assert_eq!(
                    error.as_deref(),
                    Some(INVALID_PREPARE_TYPE_HIERARCHY_RESPONSE)
                );
            }
            other => panic!("expected type hierarchy prepare event, got {other:?}"),
        }
    }

    #[test]
    fn type_hierarchy_prepare_reports_oversized_raw_payload() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let mut invalid_item = wire_item("Widget", "file:///src/widget.rs");
        invalid_item["data"] = json!({
            "blob": "x".repeat(MAX_VALIDATED_TYPE_HIERARCHY_ITEM_PAYLOAD_BYTES + 1)
        });

        handle_type_hierarchy_response(
            PendingLspRequest::PrepareTypeHierarchy {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                line: 3,
                character: 12,
            },
            json!({ "result": [invalid_item] }),
            &tx,
        );

        match rx.recv().expect("type hierarchy prepare event") {
            UiEvent::Lsp(LspUiEvent::TypeHierarchyPrepared { items, error, .. }) => {
                assert_eq!(items, None);
                assert_eq!(
                    error.as_deref(),
                    Some(INVALID_PREPARE_TYPE_HIERARCHY_RESPONSE)
                );
            }
            other => panic!("expected type hierarchy prepare event, got {other:?}"),
        }
    }

    #[test]
    fn type_hierarchy_prepare_treats_null_success_as_empty() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        handle_type_hierarchy_response(
            PendingLspRequest::PrepareTypeHierarchy {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                line: 3,
                character: 12,
            },
            json!({ "result": null }),
            &tx,
        );

        match rx.recv().expect("type hierarchy prepare event") {
            UiEvent::Lsp(LspUiEvent::TypeHierarchyPrepared { items, error, .. }) => {
                assert_eq!(items, Some(Vec::new()));
                assert_eq!(error, None);
            }
            other => panic!("expected type hierarchy prepare event, got {other:?}"),
        }
    }

    #[test]
    fn type_hierarchy_prepare_sends_parsed_items_event() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        handle_type_hierarchy_response(
            PendingLspRequest::PrepareTypeHierarchy {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                line: 3,
                character: 12,
            },
            json!({ "result": [wire_item("Widget", "file:///src/widget.rs")] }),
            &tx,
        );

        match rx.recv().expect("type hierarchy prepare event") {
            UiEvent::Lsp(LspUiEvent::TypeHierarchyPrepared {
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
                let items = items.expect("parsed type hierarchy items");
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].name, "Widget");
                assert_eq!(items[0].line, 3);
                assert_eq!(items[0].column, 5);
                assert_eq!(items[0].raw["data"]["token"], "Widget");
                assert_eq!(error, None);
            }
            other => panic!("expected type hierarchy prepare event, got {other:?}"),
        }
    }

    #[test]
    fn type_hierarchy_prepare_caps_emitted_items() {
        let mut result = (0..=MAX_EMITTED_TYPE_HIERARCHY_PREPARE_ITEMS)
            .map(|index| wire_item(&format!("Widget{index:03}"), "file:///src/widget.rs"))
            .collect::<Vec<_>>();
        result[MAX_EMITTED_TYPE_HIERARCHY_PREPARE_ITEMS]["selectionRange"] = json!({
            "start": {
                "line": MAX_VALIDATED_TYPE_HIERARCHY_POSITION_COMPONENT + 1,
                "character": 4
            },
            "end": { "line": 2, "character": 10 }
        });

        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        handle_type_hierarchy_response(
            PendingLspRequest::PrepareTypeHierarchy {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                line: 3,
                character: 12,
            },
            json!({ "result": result }),
            &tx,
        );

        match rx.recv().expect("type hierarchy prepare event") {
            UiEvent::Lsp(LspUiEvent::TypeHierarchyPrepared { items, error, .. }) => {
                let items = items.expect("capped type hierarchy items");
                assert_eq!(items.len(), MAX_EMITTED_TYPE_HIERARCHY_PREPARE_ITEMS);
                assert_eq!(error, None);
            }
            other => panic!("expected type hierarchy prepare event, got {other:?}"),
        }
    }

    #[test]
    fn type_hierarchy_supertypes_reports_invalid_success_payload() {
        let root = item("Widget");
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        handle_type_hierarchy_response(
            PendingLspRequest::TypeHierarchySupertypes {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                item: root.clone(),
            },
            json!({ "result": { "not": "a list" } }),
            &tx,
        );

        match rx.recv().expect("type hierarchy supertypes event") {
            UiEvent::Lsp(LspUiEvent::TypeHierarchySupertypesResult {
                id,
                path,
                version,
                item,
                supertypes,
                error,
            }) => {
                assert_eq!(id, 7);
                assert_eq!(path, PathBuf::from("src/main.rs"));
                assert_eq!(version, 11);
                assert_eq!(item, root);
                assert_eq!(supertypes, None);
                assert_eq!(
                    error.as_deref(),
                    Some(INVALID_TYPE_HIERARCHY_SUPERTYPES_RESPONSE)
                );
            }
            other => panic!("expected type hierarchy supertypes event, got {other:?}"),
        }
    }

    #[test]
    fn type_hierarchy_supertypes_reports_malformed_success_item() {
        let root = item("Widget");
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        handle_type_hierarchy_response(
            PendingLspRequest::TypeHierarchySupertypes {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                item: root,
            },
            json!({
                "result": [{
                    "name": "Parent",
                    "kind": 5,
                    "uri": "file:///src/parent.rs",
                    "range": {
                        "start": { "line": 1, "character": 0 },
                        "end": { "line": 4, "character": 1 }
                    },
                    "selectionRange": {
                        "start": { "line": 2 },
                        "end": { "line": 2, "character": 10 }
                    }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("type hierarchy supertypes event") {
            UiEvent::Lsp(LspUiEvent::TypeHierarchySupertypesResult {
                supertypes, error, ..
            }) => {
                assert_eq!(supertypes, None);
                assert_eq!(
                    error.as_deref(),
                    Some(INVALID_TYPE_HIERARCHY_SUPERTYPES_RESPONSE)
                );
            }
            other => panic!("expected type hierarchy supertypes event, got {other:?}"),
        }
    }

    #[test]
    fn type_hierarchy_supertypes_reports_reversed_success_range() {
        let root = item("Widget");
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let mut invalid_item = wire_item("Parent", "file:///src/parent.rs");
        invalid_item["selectionRange"] = json!({
            "start": { "line": 2, "character": 10 },
            "end": { "line": 2, "character": 4 }
        });

        handle_type_hierarchy_response(
            PendingLspRequest::TypeHierarchySupertypes {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                item: root,
            },
            json!({ "result": [invalid_item] }),
            &tx,
        );

        match rx.recv().expect("type hierarchy supertypes event") {
            UiEvent::Lsp(LspUiEvent::TypeHierarchySupertypesResult {
                supertypes, error, ..
            }) => {
                assert_eq!(supertypes, None);
                assert_eq!(
                    error.as_deref(),
                    Some(INVALID_TYPE_HIERARCHY_SUPERTYPES_RESPONSE)
                );
            }
            other => panic!("expected type hierarchy supertypes event, got {other:?}"),
        }
    }

    #[test]
    fn type_hierarchy_supertypes_reports_coordinate_above_core_bound() {
        let root = item("Widget");
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let mut invalid_item = wire_item("Parent", "file:///src/parent.rs");
        invalid_item["range"]["start"]["line"] =
            json!(MAX_VALIDATED_TYPE_HIERARCHY_POSITION_COMPONENT + 1);

        handle_type_hierarchy_response(
            PendingLspRequest::TypeHierarchySupertypes {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                item: root,
            },
            json!({ "result": [invalid_item] }),
            &tx,
        );

        match rx.recv().expect("type hierarchy supertypes event") {
            UiEvent::Lsp(LspUiEvent::TypeHierarchySupertypesResult {
                supertypes, error, ..
            }) => {
                assert_eq!(supertypes, None);
                assert_eq!(
                    error.as_deref(),
                    Some(INVALID_TYPE_HIERARCHY_SUPERTYPES_RESPONSE)
                );
            }
            other => panic!("expected type hierarchy supertypes event, got {other:?}"),
        }
    }

    #[test]
    fn type_hierarchy_supertypes_treats_null_success_as_empty() {
        let root = item("Widget");
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        handle_type_hierarchy_response(
            PendingLspRequest::TypeHierarchySupertypes {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                item: root,
            },
            json!({ "result": null }),
            &tx,
        );

        match rx.recv().expect("type hierarchy supertypes event") {
            UiEvent::Lsp(LspUiEvent::TypeHierarchySupertypesResult {
                supertypes, error, ..
            }) => {
                assert_eq!(supertypes, Some(Vec::new()));
                assert_eq!(error, None);
            }
            other => panic!("expected type hierarchy supertypes event, got {other:?}"),
        }
    }

    #[test]
    fn type_hierarchy_supertypes_sends_parsed_items_event() {
        let root = item("Widget");
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        handle_type_hierarchy_response(
            PendingLspRequest::TypeHierarchySupertypes {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                item: root.clone(),
            },
            json!({ "result": [wire_item("Parent", "file:///src/parent.rs")] }),
            &tx,
        );

        match rx.recv().expect("type hierarchy supertypes event") {
            UiEvent::Lsp(LspUiEvent::TypeHierarchySupertypesResult {
                item,
                supertypes,
                error,
                ..
            }) => {
                assert_eq!(item, root);
                let supertypes = supertypes.expect("parsed type hierarchy supertypes");
                assert_eq!(supertypes.len(), 1);
                assert_eq!(supertypes[0].name, "Parent");
                assert_eq!(error, None);
            }
            other => panic!("expected type hierarchy supertypes event, got {other:?}"),
        }
    }

    #[test]
    fn type_hierarchy_supertypes_caps_emitted_items() {
        let root = item("Widget");
        let mut result = (0..=MAX_EMITTED_TYPE_HIERARCHY_RELATION_ITEMS)
            .map(|index| wire_item(&format!("Parent{index:03}"), "file:///src/parent.rs"))
            .collect::<Vec<_>>();
        result[MAX_EMITTED_TYPE_HIERARCHY_RELATION_ITEMS]["range"] = json!({
            "start": {
                "line": MAX_VALIDATED_TYPE_HIERARCHY_POSITION_COMPONENT + 1,
                "character": 1
            },
            "end": { "line": 4, "character": 2 }
        });

        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        handle_type_hierarchy_response(
            PendingLspRequest::TypeHierarchySupertypes {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                item: root,
            },
            json!({ "result": result }),
            &tx,
        );

        match rx.recv().expect("type hierarchy supertypes event") {
            UiEvent::Lsp(LspUiEvent::TypeHierarchySupertypesResult {
                supertypes, error, ..
            }) => {
                let supertypes = supertypes.expect("capped type hierarchy supertypes");
                assert_eq!(supertypes.len(), MAX_EMITTED_TYPE_HIERARCHY_RELATION_ITEMS);
                assert_eq!(error, None);
            }
            other => panic!("expected type hierarchy supertypes event, got {other:?}"),
        }
    }

    #[test]
    fn type_hierarchy_subtypes_reports_invalid_success_payload() {
        let root = item("Widget");
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        handle_type_hierarchy_response(
            PendingLspRequest::TypeHierarchySubtypes {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                item: root.clone(),
            },
            json!({ "result": { "not": "a list" } }),
            &tx,
        );

        match rx.recv().expect("type hierarchy subtypes event") {
            UiEvent::Lsp(LspUiEvent::TypeHierarchySubtypesResult {
                id,
                path,
                version,
                item,
                subtypes,
                error,
            }) => {
                assert_eq!(id, 7);
                assert_eq!(path, PathBuf::from("src/main.rs"));
                assert_eq!(version, 11);
                assert_eq!(item, root);
                assert_eq!(subtypes, None);
                assert_eq!(
                    error.as_deref(),
                    Some(INVALID_TYPE_HIERARCHY_SUBTYPES_RESPONSE)
                );
            }
            other => panic!("expected type hierarchy subtypes event, got {other:?}"),
        }
    }

    #[test]
    fn type_hierarchy_subtypes_reports_malformed_success_item() {
        let root = item("Widget");
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        handle_type_hierarchy_response(
            PendingLspRequest::TypeHierarchySubtypes {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                item: root,
            },
            json!({
                "result": [{
                    "name": "",
                    "kind": 5,
                    "uri": "file:///src/child.rs",
                    "range": {
                        "start": { "line": 1, "character": 0 },
                        "end": { "line": 4, "character": 1 }
                    },
                    "selectionRange": {
                        "start": { "line": 2, "character": 4 },
                        "end": { "line": 2, "character": 10 }
                    }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("type hierarchy subtypes event") {
            UiEvent::Lsp(LspUiEvent::TypeHierarchySubtypesResult {
                subtypes, error, ..
            }) => {
                assert_eq!(subtypes, None);
                assert_eq!(
                    error.as_deref(),
                    Some(INVALID_TYPE_HIERARCHY_SUBTYPES_RESPONSE)
                );
            }
            other => panic!("expected type hierarchy subtypes event, got {other:?}"),
        }
    }

    #[test]
    fn type_hierarchy_subtypes_treats_null_success_as_empty() {
        let root = item("Widget");
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        handle_type_hierarchy_response(
            PendingLspRequest::TypeHierarchySubtypes {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                item: root,
            },
            json!({ "result": null }),
            &tx,
        );

        match rx.recv().expect("type hierarchy subtypes event") {
            UiEvent::Lsp(LspUiEvent::TypeHierarchySubtypesResult {
                subtypes, error, ..
            }) => {
                assert_eq!(subtypes, Some(Vec::new()));
                assert_eq!(error, None);
            }
            other => panic!("expected type hierarchy subtypes event, got {other:?}"),
        }
    }

    #[test]
    fn type_hierarchy_subtypes_sends_parsed_items_event() {
        let root = item("Widget");
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        handle_type_hierarchy_response(
            PendingLspRequest::TypeHierarchySubtypes {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                item: root.clone(),
            },
            json!({ "result": [wire_item("Child", "file:///src/child.rs")] }),
            &tx,
        );

        match rx.recv().expect("type hierarchy subtypes event") {
            UiEvent::Lsp(LspUiEvent::TypeHierarchySubtypesResult {
                item,
                subtypes,
                error,
                ..
            }) => {
                assert_eq!(item, root);
                let subtypes = subtypes.expect("parsed type hierarchy subtypes");
                assert_eq!(subtypes.len(), 1);
                assert_eq!(subtypes[0].name, "Child");
                assert_eq!(error, None);
            }
            other => panic!("expected type hierarchy subtypes event, got {other:?}"),
        }
    }

    #[test]
    fn type_hierarchy_subtypes_caps_emitted_items() {
        let root = item("Widget");
        let mut result = (0..=MAX_EMITTED_TYPE_HIERARCHY_RELATION_ITEMS)
            .map(|index| wire_item(&format!("Child{index:03}"), "file:///src/child.rs"))
            .collect::<Vec<_>>();
        result[MAX_EMITTED_TYPE_HIERARCHY_RELATION_ITEMS]["selectionRange"]["end"]["character"] =
            json!(MAX_VALIDATED_TYPE_HIERARCHY_POSITION_COMPONENT + 1);

        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        handle_type_hierarchy_response(
            PendingLspRequest::TypeHierarchySubtypes {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 11,
                item: root,
            },
            json!({ "result": result }),
            &tx,
        );

        match rx.recv().expect("type hierarchy subtypes event") {
            UiEvent::Lsp(LspUiEvent::TypeHierarchySubtypesResult {
                subtypes, error, ..
            }) => {
                let subtypes = subtypes.expect("capped type hierarchy subtypes");
                assert_eq!(subtypes.len(), MAX_EMITTED_TYPE_HIERARCHY_RELATION_ITEMS);
                assert_eq!(error, None);
            }
            other => panic!("expected type hierarchy subtypes event, got {other:?}"),
        }
    }
}
