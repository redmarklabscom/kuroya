use super::super::super::response_error;
use crate::ui_event_channel::Sender;
use crate::{lsp_ui_events::LspUiEvent, ui_events::UiEvent};
use kuroya_core::{BufferId, LspTextEdit, parse_formatting_response};
use serde_json::Value;
use std::path::{Path, PathBuf};

const INVALID_FORMATTING_RESPONSE: &str = "invalid textDocument/formatting response";
const MAX_VALIDATED_FORMATTING_TEXT_EDITS: usize = 2_000;
const MAX_VALIDATED_TEXT_EDIT_NEW_TEXT_BYTES: usize = 2 * 1024 * 1024;
const MAX_VALIDATED_FORMATTING_POSITION_COMPONENT: u64 = i32::MAX as u64;

pub(super) fn send_formatting_result(
    request_id: u64,
    id: BufferId,
    path: PathBuf,
    version: u64,
    value: &Value,
    ui_tx: &Sender<UiEvent>,
) {
    let mut error = response_error(value);
    let edits = if error.is_none() {
        match parse_formatting_success_response(value, &path) {
            Some(edits) => Some(edits),
            None => {
                error = Some(INVALID_FORMATTING_RESPONSE.to_owned());
                None
            }
        }
    } else {
        None
    };
    let _ = crate::lsp_client::response::emit_critical_lsp_response_event(
        ui_tx,
        LspUiEvent::FormattingResult {
            request_id,
            id,
            path,
            version,
            edits,
            error,
        },
    );
}

fn parse_formatting_success_response(value: &Value, path: &Path) -> Option<Vec<LspTextEdit>> {
    let result = value.get("result")?;
    if result.is_null() {
        return Some(Vec::new());
    }

    let edits = result.as_array()?;
    if edits.len() > MAX_VALIDATED_FORMATTING_TEXT_EDITS
        || edits.iter().any(|edit| !text_edit_is_valid(edit))
    {
        return None;
    }

    parse_formatting_response(value, path)
}

fn text_edit_is_valid(value: &Value) -> bool {
    value
        .get("newText")
        .and_then(Value::as_str)
        .is_some_and(|text| text.len() <= MAX_VALIDATED_TEXT_EDIT_NEW_TEXT_BYTES)
        && value.get("range").is_some_and(is_lsp_range)
}

fn is_lsp_range(value: &Value) -> bool {
    let Some(start) = value.get("start").and_then(lsp_position) else {
        return false;
    };
    let Some(end) = value.get("end").and_then(lsp_position) else {
        return false;
    };
    start <= end
}

fn lsp_position(value: &Value) -> Option<(u64, u64)> {
    let line = value.get("line").and_then(Value::as_u64)?;
    let character = value.get("character").and_then(Value::as_u64)?;
    if line > MAX_VALIDATED_FORMATTING_POSITION_COMPONENT
        || character > MAX_VALIDATED_FORMATTING_POSITION_COMPONENT
    {
        return None;
    }
    Some((line, character))
}

#[cfg(test)]
mod tests {
    use super::{
        INVALID_FORMATTING_RESPONSE, MAX_VALIDATED_TEXT_EDIT_NEW_TEXT_BYTES, send_formatting_result,
    };
    use crate::{lsp_ui_events::LspUiEvent, ui_events::UiEvent};
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn formatting_result_reports_invalid_success_payload() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_formatting_result(
            13,
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({ "result": { "not": "an array" } }),
            &tx,
        );

        match rx.recv().expect("formatting result event") {
            UiEvent::Lsp(LspUiEvent::FormattingResult {
                id,
                request_id,
                path,
                version,
                edits,
                error,
            }) => {
                assert_eq!(id, 7);
                assert_eq!(request_id, 13);
                assert_eq!(path, PathBuf::from("src/main.rs"));
                assert_eq!(version, 11);
                assert_eq!(edits, None);
                assert_eq!(error.as_deref(), Some(INVALID_FORMATTING_RESPONSE));
            }
            other => panic!("expected formatting result event, got {other:?}"),
        }
    }

    #[test]
    fn formatting_result_reports_malformed_text_edit() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_formatting_result(
            13,
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({
                "result": [{
                    "range": {
                        "start": { "line": 0 },
                        "end": { "line": 0, "character": 0 }
                    },
                    "newText": "fn main() {}\n"
                }]
            }),
            &tx,
        );

        match rx.recv().expect("formatting result event") {
            UiEvent::Lsp(LspUiEvent::FormattingResult { edits, error, .. }) => {
                assert_eq!(edits, None);
                assert_eq!(error.as_deref(), Some(INVALID_FORMATTING_RESPONSE));
            }
            other => panic!("expected formatting result event, got {other:?}"),
        }
    }

    #[test]
    fn formatting_result_reports_reversed_text_edit_range() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_formatting_result(
            13,
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({
                "result": [{
                    "range": {
                        "start": { "line": 2, "character": 0 },
                        "end": { "line": 1, "character": 0 }
                    },
                    "newText": "fn main() {}\n"
                }]
            }),
            &tx,
        );

        match rx.recv().expect("formatting result event") {
            UiEvent::Lsp(LspUiEvent::FormattingResult { edits, error, .. }) => {
                assert_eq!(edits, None);
                assert_eq!(error.as_deref(), Some(INVALID_FORMATTING_RESPONSE));
            }
            other => panic!("expected formatting result event, got {other:?}"),
        }
    }

    #[test]
    fn formatting_result_reports_overflowing_position_component() {
        let overflowing_component = (i32::MAX as u64) + 1;
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_formatting_result(
            13,
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({
                "result": [{
                    "range": {
                        "start": { "line": overflowing_component, "character": 0 },
                        "end": { "line": overflowing_component, "character": 1 }
                    },
                    "newText": "fn main() {}\n"
                }]
            }),
            &tx,
        );

        match rx.recv().expect("formatting result event") {
            UiEvent::Lsp(LspUiEvent::FormattingResult { edits, error, .. }) => {
                assert_eq!(edits, None);
                assert_eq!(error.as_deref(), Some(INVALID_FORMATTING_RESPONSE));
            }
            other => panic!("expected formatting result event, got {other:?}"),
        }
    }

    #[test]
    fn formatting_result_reports_oversized_text_edit() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_formatting_result(
            13,
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({
                "result": [{
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 0, "character": 0 }
                    },
                    "newText": "x".repeat(MAX_VALIDATED_TEXT_EDIT_NEW_TEXT_BYTES + 1)
                }]
            }),
            &tx,
        );

        match rx.recv().expect("formatting result event") {
            UiEvent::Lsp(LspUiEvent::FormattingResult { edits, error, .. }) => {
                assert_eq!(edits, None);
                assert_eq!(error.as_deref(), Some(INVALID_FORMATTING_RESPONSE));
            }
            other => panic!("expected formatting result event, got {other:?}"),
        }
    }

    #[test]
    fn formatting_result_treats_null_success_as_empty_edits() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_formatting_result(
            13,
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({ "result": null }),
            &tx,
        );

        match rx.recv().expect("formatting result event") {
            UiEvent::Lsp(LspUiEvent::FormattingResult { edits, error, .. }) => {
                assert_eq!(edits, Some(Vec::new()));
                assert_eq!(error, None);
            }
            other => panic!("expected formatting result event, got {other:?}"),
        }
    }

    #[test]
    fn formatting_result_sends_parsed_edits_event() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_formatting_result(
            13,
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({
                "result": [{
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 2, "character": 1 }
                    },
                    "newText": "fn main() {\n    println!(\"hi\");\n}\n"
                }]
            }),
            &tx,
        );

        match rx.recv().expect("formatting result event") {
            UiEvent::Lsp(LspUiEvent::FormattingResult {
                id,
                request_id,
                path,
                version,
                edits,
                error,
            }) => {
                assert_eq!(id, 7);
                assert_eq!(request_id, 13);
                assert_eq!(path, PathBuf::from("src/main.rs"));
                assert_eq!(version, 11);
                let edits = edits.expect("parsed formatting edits");
                assert_eq!(edits.len(), 1);
                assert_eq!(edits[0].path, PathBuf::from("src/main.rs"));
                assert_eq!(edits[0].start_line, 1);
                assert_eq!(edits[0].start_column, 1);
                assert_eq!(edits[0].end_line, 3);
                assert_eq!(edits[0].end_column, 2);
                assert!(edits[0].new_text.contains("println!"));
                assert_eq!(error, None);
            }
            other => panic!("expected formatting result event, got {other:?}"),
        }
    }
}
