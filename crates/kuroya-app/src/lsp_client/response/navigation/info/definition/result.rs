use super::super::super::super::response_error;
use crate::ui_event_channel::Sender;
use crate::{lsp_ui_events::LspUiEvent, ui_events::UiEvent};
use kuroya_core::{BufferId, LspDefinition, lsp::file_uri_to_path, parse_definition_response};
use serde_json::Value;
use std::path::PathBuf;

const INVALID_DEFINITION_RESPONSE: &str = "invalid textDocument/definition response";
const MAX_DEFINITION_LOCATION_ITEMS: usize = 512;
const MAX_DEFINITION_POSITION_COMPONENT: usize = i32::MAX as usize;

pub(super) fn send_definition_result(
    id: BufferId,
    path: PathBuf,
    version: u64,
    line: usize,
    character: usize,
    value: &Value,
    ui_tx: &Sender<UiEvent>,
) {
    let mut error = response_error(value);
    let definition = if error.is_none() {
        match parse_definition_success_response(value) {
            Some(definition) => definition,
            None => {
                error = Some(INVALID_DEFINITION_RESPONSE.to_owned());
                None
            }
        }
    } else {
        None
    };

    let _ = crate::lsp_client::response::emit_critical_lsp_response_event(
        ui_tx,
        LspUiEvent::DefinitionResult {
            id,
            origin_path: path,
            version,
            origin_line: line,
            origin_column: character.saturating_add(1),
            definition,
            error,
        },
    );
}

fn parse_definition_success_response(value: &Value) -> Option<Option<LspDefinition>> {
    let result = value.get("result")?;
    if result.is_null() {
        return Some(None);
    }

    if let Some(items) = result.as_array() {
        let mut first_definition = None;
        for item in items.iter().take(MAX_DEFINITION_LOCATION_ITEMS) {
            if !definition_location_shape_is_valid(item) {
                return None;
            }
            let definition = parse_definition_location(item)?;
            if first_definition.is_none() {
                first_definition = Some(definition);
            }
        }
        return Some(first_definition);
    }

    if !definition_location_shape_is_valid(result) {
        return None;
    }
    parse_definition_location(result).map(Some)
}

fn parse_definition_location(value: &Value) -> Option<LspDefinition> {
    parse_definition_response(&serde_json::json!({ "result": value }))
}

fn definition_location_shape_is_valid(value: &Value) -> bool {
    if value
        .get("uri")
        .and_then(Value::as_str)
        .is_some_and(|uri| file_uri_to_path(uri).is_some())
    {
        return definition_range_is_valid(value.get("range"));
    }

    value
        .get("targetUri")
        .and_then(Value::as_str)
        .is_some_and(|uri| file_uri_to_path(uri).is_some())
        && definition_range_is_valid(
            value
                .get("targetSelectionRange")
                .or_else(|| value.get("targetRange")),
        )
}

fn definition_range_is_valid(value: Option<&Value>) -> bool {
    let Some(value) = value else {
        return false;
    };
    let Some((start_line, start_character)) = definition_position(value.get("start")) else {
        return false;
    };
    let Some((end_line, end_character)) = definition_position(value.get("end")) else {
        return false;
    };

    end_line > start_line || (end_line == start_line && end_character >= start_character)
}

fn definition_position(value: Option<&Value>) -> Option<(usize, usize)> {
    let value = value?;
    Some((
        definition_position_component(value.get("line")?)?,
        definition_position_component(value.get("character")?)?,
    ))
}

fn definition_position_component(value: &Value) -> Option<usize> {
    let value = usize::try_from(value.as_u64()?).ok()?;
    (value <= MAX_DEFINITION_POSITION_COMPONENT).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::{
        INVALID_DEFINITION_RESPONSE, MAX_DEFINITION_LOCATION_ITEMS, send_definition_result,
    };
    use crate::{lsp_ui_events::LspUiEvent, ui_events::UiEvent};
    use serde_json::json;
    use std::path::{Path, PathBuf};

    #[test]
    fn definition_result_reports_invalid_success_payload() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_definition_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({ "result": { "uri": "file:///C:/workspace/src/lib.rs" } }),
            &tx,
        );

        match rx.recv().expect("definition result event") {
            UiEvent::Lsp(LspUiEvent::DefinitionResult {
                id,
                origin_path,
                version,
                origin_line,
                origin_column,
                definition,
                error,
            }) => {
                assert_eq!(id, 7);
                assert_eq!(origin_path, PathBuf::from("src/main.rs"));
                assert_eq!(version, 11);
                assert_eq!(origin_line, 3);
                assert_eq!(origin_column, 5);
                assert_eq!(definition, None);
                assert_eq!(error.as_deref(), Some(INVALID_DEFINITION_RESPONSE));
            }
            other => panic!("expected definition result event, got {other:?}"),
        }
    }

    #[test]
    fn definition_result_reports_invalid_item_in_success_array() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_definition_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({
                "result": [
                    location("file:///C:/workspace/src/valid.rs", 1, 2, 1, 8),
                    { "uri": "file:///C:/workspace/src/bad.rs", "range": { "start": { "line": 1 } } }
                ]
            }),
            &tx,
        );

        match rx.recv().expect("definition result event") {
            UiEvent::Lsp(LspUiEvent::DefinitionResult {
                definition, error, ..
            }) => {
                assert_eq!(definition, None);
                assert_eq!(error.as_deref(), Some(INVALID_DEFINITION_RESPONSE));
            }
            other => panic!("expected definition result event, got {other:?}"),
        }
    }

    #[test]
    fn definition_result_reports_invalid_reversed_location_range() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_definition_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({
                "result": location("file:///C:/workspace/src/lib.rs", 2, 8, 2, 3)
            }),
            &tx,
        );

        match rx.recv().expect("definition result event") {
            UiEvent::Lsp(LspUiEvent::DefinitionResult {
                definition, error, ..
            }) => {
                assert_eq!(definition, None);
                assert_eq!(error.as_deref(), Some(INVALID_DEFINITION_RESPONSE));
            }
            other => panic!("expected definition result event, got {other:?}"),
        }
    }

    #[test]
    fn definition_result_reports_invalid_uri_before_parsing() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_definition_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({
                "result": location("file:///C:/workspace/src/lib%GG.rs", 1, 2, 1, 8)
            }),
            &tx,
        );

        match rx.recv().expect("definition result event") {
            UiEvent::Lsp(LspUiEvent::DefinitionResult {
                definition, error, ..
            }) => {
                assert_eq!(definition, None);
                assert_eq!(error.as_deref(), Some(INVALID_DEFINITION_RESPONSE));
            }
            other => panic!("expected definition result event, got {other:?}"),
        }
    }

    #[test]
    fn definition_result_reports_overflowing_position_component() {
        let overflowing_component = (i32::MAX as u64) + 1;
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_definition_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({
                "result": {
                    "uri": "file:///C:/workspace/src/lib.rs",
                    "range": {
                        "start": { "line": overflowing_component, "character": 0 },
                        "end": { "line": overflowing_component, "character": 1 }
                    }
                }
            }),
            &tx,
        );

        match rx.recv().expect("definition result event") {
            UiEvent::Lsp(LspUiEvent::DefinitionResult {
                definition, error, ..
            }) => {
                assert_eq!(definition, None);
                assert_eq!(error.as_deref(), Some(INVALID_DEFINITION_RESPONSE));
            }
            other => panic!("expected definition result event, got {other:?}"),
        }
    }

    #[test]
    fn definition_result_bounds_unrepresentable_origin_column() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_definition_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            usize::MAX,
            &json!({ "result": null }),
            &tx,
        );

        match rx.recv().expect("definition result event") {
            UiEvent::Lsp(LspUiEvent::DefinitionResult {
                origin_column,
                definition,
                error,
                ..
            }) => {
                assert_eq!(origin_column, usize::MAX);
                assert_eq!(definition, None);
                assert_eq!(error, None);
            }
            other => panic!("expected definition result event, got {other:?}"),
        }
    }

    #[test]
    fn definition_result_caps_oversized_location_arrays() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let mut items = (0..=MAX_DEFINITION_LOCATION_ITEMS)
            .map(|idx| location("file:///C:/workspace/src/lib.rs", idx, 0, idx, 1))
            .collect::<Vec<_>>();
        items[MAX_DEFINITION_LOCATION_ITEMS] =
            location("file:///C:/workspace/src/tail.rs", 2, 8, 2, 3);
        send_definition_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({ "result": items }),
            &tx,
        );

        match rx.recv().expect("definition result event") {
            UiEvent::Lsp(LspUiEvent::DefinitionResult {
                definition, error, ..
            }) => {
                let definition = definition.expect("first bounded definition");
                assert!(definition.path.ends_with(Path::new("src").join("lib.rs")));
                assert_eq!(definition.line, 1);
                assert_eq!(definition.column, 1);
                assert_eq!(error, None);
            }
            other => panic!("expected definition result event, got {other:?}"),
        }
    }

    #[test]
    fn definition_result_treats_null_success_as_no_definition() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_definition_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({ "result": null }),
            &tx,
        );

        match rx.recv().expect("definition result event") {
            UiEvent::Lsp(LspUiEvent::DefinitionResult {
                definition, error, ..
            }) => {
                assert_eq!(definition, None);
                assert_eq!(error, None);
            }
            other => panic!("expected definition result event, got {other:?}"),
        }
    }

    #[test]
    fn definition_result_sends_parsed_location_event() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_definition_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({ "result": location("file:///C:/workspace/src/lib.rs", 1, 2, 1, 8) }),
            &tx,
        );

        match rx.recv().expect("definition result event") {
            UiEvent::Lsp(LspUiEvent::DefinitionResult {
                definition, error, ..
            }) => {
                let definition = definition.expect("parsed definition");
                assert!(definition.path.ends_with(Path::new("src").join("lib.rs")));
                assert_eq!(definition.line, 2);
                assert_eq!(definition.column, 3);
                assert_eq!(error, None);
            }
            other => panic!("expected definition result event, got {other:?}"),
        }
    }

    #[test]
    fn definition_result_sends_parsed_location_link_event() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_definition_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({
                "result": [{
                    "targetUri": "file:///C:/workspace/src/target.rs",
                    "targetSelectionRange": {
                        "start": { "line": 4, "character": 6 },
                        "end": { "line": 4, "character": 12 }
                    }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("definition result event") {
            UiEvent::Lsp(LspUiEvent::DefinitionResult {
                definition, error, ..
            }) => {
                let definition = definition.expect("parsed definition link");
                assert!(
                    definition
                        .path
                        .ends_with(Path::new("src").join("target.rs"))
                );
                assert_eq!(definition.line, 5);
                assert_eq!(definition.column, 7);
                assert_eq!(error, None);
            }
            other => panic!("expected definition result event, got {other:?}"),
        }
    }

    fn location(
        uri: &str,
        line: usize,
        character: usize,
        end_line: usize,
        end_character: usize,
    ) -> serde_json::Value {
        json!({
            "uri": uri,
            "range": {
                "start": { "line": line, "character": character },
                "end": { "line": end_line, "character": end_character }
            }
        })
    }
}
