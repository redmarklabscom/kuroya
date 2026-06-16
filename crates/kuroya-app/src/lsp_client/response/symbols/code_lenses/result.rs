use super::super::super::response_error;
use crate::lsp_client::pending::{
    MAX_LSP_OUTBOUND_JSON_PAYLOAD_BYTES, MAX_LSP_OUTBOUND_TEXT_PAYLOAD_CHARS,
    lsp_json_payload_is_bounded,
};
use crate::ui_event_channel::Sender;
use crate::{lsp_ui_events::LspUiEvent, ui_events::UiEvent};
use kuroya_core::{
    BufferId, LspCodeLens, parse_code_lens_resolve_response, parse_code_lenses_response,
};
use serde_json::Value;
use std::path::PathBuf;

const INVALID_CODE_LENSES_RESPONSE: &str = "invalid textDocument/codeLens response";
const INVALID_CODE_LENS_RESOLVE_RESPONSE: &str = "invalid codeLens/resolve response";
const MAX_VALIDATED_CODE_LENSES: usize = 500;
const MAX_LSP_POSITION_COMPONENT: usize = i32::MAX as usize;

pub(super) fn send_code_lenses_result(
    id: BufferId,
    path: PathBuf,
    version: u64,
    value: &Value,
    ui_tx: &Sender<UiEvent>,
) {
    let mut error = response_error(value);
    let lenses = if error.is_none() {
        match parse_code_lenses_success_response(value) {
            Some(lenses) => Some(lenses),
            None => {
                error = Some(INVALID_CODE_LENSES_RESPONSE.to_owned());
                None
            }
        }
    } else {
        None
    };
    let _ = crate::lsp_client::response::emit_lsp_response_event(
        ui_tx,
        LspUiEvent::CodeLensesResult {
            id,
            path,
            version,
            lenses,
            error,
        },
    );
}

fn parse_code_lenses_success_response(value: &Value) -> Option<Vec<LspCodeLens>> {
    if value.get("error").is_some() {
        return None;
    }

    let result = value.get("result")?;
    if result.is_null() {
        return Some(Vec::new());
    }

    let items = result.as_array()?;
    if items
        .iter()
        .any(|item| !is_valid_code_lens_item_shape(item))
    {
        return None;
    }

    let mut lenses = parse_code_lenses_response(value)?;
    lenses.truncate(MAX_VALIDATED_CODE_LENSES);
    Some(lenses)
}

fn is_valid_code_lens_item_shape(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    let Some(range) = object.get("range") else {
        return false;
    };
    if !is_valid_lsp_range(range) {
        return false;
    }

    match object.get("command") {
        Some(command) => is_valid_code_lens_command(command),
        None => is_valid_code_lens_resolve_payload(value),
    }
}

fn is_valid_code_lens_command(value: &Value) -> bool {
    let Some(command) = value.as_object() else {
        return false;
    };

    command.get("title").is_some_and(required_bounded_lsp_text)
        && command
            .get("command")
            .is_some_and(required_bounded_lsp_command_id)
        && command.get("arguments").is_none_or(|arguments| {
            arguments.is_array()
                && lsp_json_payload_is_bounded(arguments, MAX_LSP_OUTBOUND_JSON_PAYLOAD_BYTES)
        })
}

fn is_valid_code_lens_resolve_payload(value: &Value) -> bool {
    value.get("data").is_some()
        && lsp_json_payload_is_bounded(value, MAX_LSP_OUTBOUND_JSON_PAYLOAD_BYTES)
}

fn is_valid_lsp_range(value: &Value) -> bool {
    let Some(start) = lsp_position(value.get("start")) else {
        return false;
    };
    let Some(end) = lsp_position(value.get("end")) else {
        return false;
    };
    start <= end
}

fn lsp_position(value: Option<&Value>) -> Option<(usize, usize)> {
    let value = value?;
    Some((
        lsp_zero_based_coordinate_to_usize(value.get("line")?)?,
        lsp_zero_based_coordinate_to_usize(value.get("character")?)?,
    ))
}

fn lsp_zero_based_coordinate_to_usize(value: &Value) -> Option<usize> {
    usize::try_from(value.as_u64()?)
        .ok()
        .filter(|value| *value <= MAX_LSP_POSITION_COMPONENT)
}

fn required_bounded_lsp_text(value: &Value) -> bool {
    bounded_lsp_text_has_visible_value(value).unwrap_or(false)
}

fn required_bounded_lsp_command_id(value: &Value) -> bool {
    let Some(text) = value.as_str() else {
        return false;
    };
    text.trim() == text
        && !text.chars().any(char::is_control)
        && bounded_lsp_text_has_visible_value(value).unwrap_or(false)
}

fn bounded_lsp_text_has_visible_value(value: &Value) -> Option<bool> {
    let text = value.as_str()?;
    let mut has_visible_value = false;
    for (index, ch) in text.chars().enumerate() {
        if index >= MAX_LSP_OUTBOUND_TEXT_PAYLOAD_CHARS {
            return Some(false);
        }
        has_visible_value |= !ch.is_whitespace();
    }
    Some(has_visible_value)
}

pub(super) fn send_code_lens_resolve_result(
    id: BufferId,
    path: PathBuf,
    version: u64,
    value: &Value,
    ui_tx: &Sender<UiEvent>,
) {
    let mut error = response_error(value);
    let lens = if error.is_none() {
        match parse_code_lens_resolve_success_response(value) {
            Some(lens) => Some(lens),
            None => {
                error = Some(INVALID_CODE_LENS_RESOLVE_RESPONSE.to_owned());
                None
            }
        }
    } else {
        None
    };
    let _ = crate::lsp_client::response::emit_lsp_response_event(
        ui_tx,
        LspUiEvent::CodeLensResolveResult {
            id,
            path,
            version,
            lens,
            error,
        },
    );
}

fn parse_code_lens_resolve_success_response(value: &Value) -> Option<LspCodeLens> {
    if value.get("error").is_some() {
        return None;
    }

    let result = value.get("result")?;
    if result.is_null() || !is_valid_code_lens_item_shape(result) {
        return None;
    }

    parse_code_lens_resolve_response(value)
}

pub(super) fn send_execute_command_result(
    id: BufferId,
    path: PathBuf,
    version: u64,
    title: String,
    command: String,
    value: &Value,
    ui_tx: &Sender<UiEvent>,
) {
    let _ = crate::lsp_client::response::emit_critical_lsp_response_event(
        ui_tx,
        LspUiEvent::CodeLensCommandResult {
            id,
            path,
            version,
            title,
            command,
            error: response_error(value),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::{
        INVALID_CODE_LENS_RESOLVE_RESPONSE, INVALID_CODE_LENSES_RESPONSE,
        MAX_LSP_POSITION_COMPONENT, MAX_VALIDATED_CODE_LENSES, send_code_lens_resolve_result,
        send_code_lenses_result, send_execute_command_result,
    };
    use crate::{lsp_ui_events::LspUiEvent, ui_events::UiEvent};
    use serde_json::{Value, json};
    use std::path::PathBuf;

    fn code_lens_item(title: &str) -> serde_json::Value {
        json!({
            "range": {
                "start": { "line": 2, "character": 4 },
                "end": { "line": 2, "character": 4 }
            },
            "command": {
                "title": title,
                "command": "rust-analyzer.runSingle"
            }
        })
    }

    fn assert_invalid_code_lenses_result(value: Value) {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_code_lenses_result(7, PathBuf::from("src/main.rs"), 11, &value, &tx);

        match rx.recv().expect("code lenses result event") {
            UiEvent::Lsp(LspUiEvent::CodeLensesResult { lenses, error, .. }) => {
                assert_eq!(lenses, None);
                assert_eq!(error.as_deref(), Some(INVALID_CODE_LENSES_RESPONSE));
            }
            other => panic!("expected code lenses result event, got {other:?}"),
        }
    }

    fn assert_invalid_code_lens_resolve_result(value: Value) {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_code_lens_resolve_result(7, PathBuf::from("src/main.rs"), 11, &value, &tx);

        match rx.recv().expect("code lens resolve result event") {
            UiEvent::Lsp(LspUiEvent::CodeLensResolveResult { lens, error, .. }) => {
                assert_eq!(lens, None);
                assert_eq!(error.as_deref(), Some(INVALID_CODE_LENS_RESOLVE_RESPONSE));
            }
            other => panic!("expected code lens resolve result event, got {other:?}"),
        }
    }

    #[test]
    fn code_lenses_result_reports_invalid_success_payload() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_code_lenses_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({ "result": { "not": "an array" } }),
            &tx,
        );

        match rx.recv().expect("code lenses result event") {
            UiEvent::Lsp(LspUiEvent::CodeLensesResult {
                id,
                path,
                version,
                lenses,
                error,
            }) => {
                assert_eq!(id, 7);
                assert_eq!(path, PathBuf::from("src/main.rs"));
                assert_eq!(version, 11);
                assert_eq!(lenses, None);
                assert_eq!(error.as_deref(), Some(INVALID_CODE_LENSES_RESPONSE));
            }
            other => panic!("expected code lenses result event, got {other:?}"),
        }
    }

    #[test]
    fn code_lenses_result_reports_invalid_malformed_lens_item() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_code_lenses_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({
                "result": [{
                    "range": {
                        "start": { "line": 2 }
                    },
                    "command": {
                        "title": "Run Test",
                        "command": "rust-analyzer.runSingle"
                    }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("code lenses result event") {
            UiEvent::Lsp(LspUiEvent::CodeLensesResult {
                id,
                path,
                version,
                lenses,
                error,
            }) => {
                assert_eq!(id, 7);
                assert_eq!(path, PathBuf::from("src/main.rs"));
                assert_eq!(version, 11);
                assert_eq!(lenses, None);
                assert_eq!(error.as_deref(), Some(INVALID_CODE_LENSES_RESPONSE));
            }
            other => panic!("expected code lenses result event, got {other:?}"),
        }
    }

    #[test]
    fn code_lenses_result_reports_malformed_command_arguments() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_code_lenses_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({
                "result": [{
                    "range": {
                        "start": { "line": 2, "character": 4 },
                        "end": { "line": 2, "character": 4 }
                    },
                    "command": {
                        "title": "Run Test",
                        "command": "rust-analyzer.runSingle",
                        "arguments": { "not": "an array" }
                    }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("code lenses result event") {
            UiEvent::Lsp(LspUiEvent::CodeLensesResult { lenses, error, .. }) => {
                assert_eq!(lenses, None);
                assert_eq!(error.as_deref(), Some(INVALID_CODE_LENSES_RESPONSE));
            }
            other => panic!("expected code lenses result event, got {other:?}"),
        }
    }

    #[test]
    fn code_lenses_result_reports_invalid_ranges_and_command_targets() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_code_lenses_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({
                "result": [{
                    "range": {
                        "start": { "line": 2, "character": 4 },
                        "end": { "line": 1, "character": 4 }
                    },
                    "command": {
                        "title": "Run Test",
                        "command": ""
                    }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("code lenses result event") {
            UiEvent::Lsp(LspUiEvent::CodeLensesResult { lenses, error, .. }) => {
                assert_eq!(lenses, None);
                assert_eq!(error.as_deref(), Some(INVALID_CODE_LENSES_RESPONSE));
            }
            other => panic!("expected code lenses result event, got {other:?}"),
        }
    }

    #[test]
    fn code_lenses_result_reports_oversized_command_arguments() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_code_lenses_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({
                "result": [{
                    "range": {
                        "start": { "line": 2, "character": 4 },
                        "end": { "line": 2, "character": 4 }
                    },
                    "command": {
                        "title": "Run Test",
                        "command": "rust-analyzer.runSingle",
                        "arguments": ["x".repeat(crate::lsp_client::pending::MAX_LSP_OUTBOUND_JSON_PAYLOAD_BYTES)]
                    }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("code lenses result event") {
            UiEvent::Lsp(LspUiEvent::CodeLensesResult { lenses, error, .. }) => {
                assert_eq!(lenses, None);
                assert_eq!(error.as_deref(), Some(INVALID_CODE_LENSES_RESPONSE));
            }
            other => panic!("expected code lenses result event, got {other:?}"),
        }
    }

    #[test]
    fn code_lenses_result_reports_invalid_command_payload_shape() {
        assert_invalid_code_lenses_result(json!({
            "result": [{
                "range": {
                    "start": { "line": 2, "character": 4 },
                    "end": { "line": 2, "character": 4 }
                },
                "command": {
                    "title": "Run Test"
                }
            }]
        }));
        assert_invalid_code_lenses_result(json!({
            "result": [{
                "range": {
                    "start": { "line": 2, "character": 4 },
                    "end": { "line": 2, "character": 4 }
                },
                "command": {
                    "title": "Run Test",
                    "command": "rust-analyzer.run\nSingle"
                }
            }]
        }));
    }

    #[test]
    fn code_lenses_result_reports_oversized_resolve_payload() {
        assert_invalid_code_lenses_result(json!({
            "result": [{
                "range": {
                    "start": { "line": 2, "character": 4 },
                    "end": { "line": 2, "character": 4 }
                },
                "data": "x".repeat(crate::lsp_client::pending::MAX_LSP_OUTBOUND_JSON_PAYLOAD_BYTES)
            }]
        }));
    }

    #[test]
    fn code_lenses_result_reports_coordinates_above_core_supported_bound() {
        let too_large = MAX_LSP_POSITION_COMPONENT as u64 + 1;
        assert_invalid_code_lenses_result(json!({
            "result": [{
                "range": {
                    "start": { "line": too_large, "character": 0 },
                    "end": { "line": too_large, "character": 0 }
                },
                "command": {
                    "title": "Run Test",
                    "command": "rust-analyzer.runSingle"
                }
            }]
        }));
    }

    #[test]
    fn code_lenses_result_reports_error_payload_without_message_as_invalid() {
        assert_invalid_code_lenses_result(json!({
            "error": { "code": -32000 },
            "result": null
        }));
    }

    #[test]
    fn code_lenses_result_preserves_lsp_error_message_without_parsing_result() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_code_lenses_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({
                "error": { "message": "server failed" },
                "result": [{
                    "range": {
                        "start": { "line": 2 }
                    },
                    "command": {
                        "title": "Run Test",
                        "command": "rust-analyzer.runSingle"
                    }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("code lenses result event") {
            UiEvent::Lsp(LspUiEvent::CodeLensesResult { lenses, error, .. }) => {
                assert_eq!(lenses, None);
                assert_eq!(error.as_deref(), Some("server failed"));
            }
            other => panic!("expected code lenses result event, got {other:?}"),
        }
    }

    #[test]
    fn code_lenses_result_sends_empty_list_for_null_success() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_code_lenses_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({ "result": null }),
            &tx,
        );

        match rx.recv().expect("code lenses result event") {
            UiEvent::Lsp(LspUiEvent::CodeLensesResult {
                id,
                path,
                version,
                lenses,
                error,
            }) => {
                assert_eq!(id, 7);
                assert_eq!(path, PathBuf::from("src/main.rs"));
                assert_eq!(version, 11);
                assert_eq!(lenses, Some(Vec::new()));
                assert_eq!(error, None);
            }
            other => panic!("expected code lenses result event, got {other:?}"),
        }
    }

    #[test]
    fn code_lenses_result_sends_parsed_lenses_event() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_code_lenses_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({
                "result": [{
                    "range": {
                        "start": { "line": 2, "character": 4 },
                        "end": { "line": 2, "character": 4 }
                    },
                    "command": {
                        "title": "Run Test",
                        "command": "rust-analyzer.runSingle"
                    }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("code lenses result event") {
            UiEvent::Lsp(LspUiEvent::CodeLensesResult {
                id,
                path,
                version,
                lenses,
                error,
            }) => {
                assert_eq!(id, 7);
                assert_eq!(path, PathBuf::from("src/main.rs"));
                assert_eq!(version, 11);
                let lenses = lenses.expect("parsed code lenses");
                assert_eq!(lenses.len(), 1);
                assert_eq!(lenses[0].line, 3);
                assert_eq!(lenses[0].column, 5);
                assert_eq!(lenses[0].title, "Run Test");
                assert_eq!(
                    lenses[0].command.as_deref(),
                    Some("rust-analyzer.runSingle")
                );
                assert_eq!(error, None);
            }
            other => panic!("expected code lenses result event, got {other:?}"),
        }
    }

    #[test]
    fn code_lenses_result_caps_emitted_lenses() {
        let result = (0..=MAX_VALIDATED_CODE_LENSES)
            .map(|index| code_lens_item(&format!("Run Test {index:03}")))
            .collect::<Vec<_>>();

        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_code_lenses_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({ "result": result }),
            &tx,
        );

        match rx.recv().expect("code lenses result event") {
            UiEvent::Lsp(LspUiEvent::CodeLensesResult { lenses, error, .. }) => {
                let lenses = lenses.expect("capped code lenses");
                assert_eq!(lenses.len(), MAX_VALIDATED_CODE_LENSES);
                assert_eq!(error, None);
            }
            other => panic!("expected code lenses result event, got {other:?}"),
        }
    }

    #[test]
    fn code_lenses_result_reports_malformed_tail_after_cap() {
        let mut result = (0..MAX_VALIDATED_CODE_LENSES)
            .map(|index| code_lens_item(&format!("Run Test {index:03}")))
            .collect::<Vec<_>>();
        result.push(json!({
            "range": {
                "start": { "line": 2, "character": 4 },
                "end": { "line": 1, "character": 4 }
            },
            "command": {
                "title": "Run Tail",
                "command": "rust-analyzer.runSingle"
            }
        }));

        assert_invalid_code_lenses_result(json!({ "result": result }));
    }

    #[test]
    fn code_lenses_result_reports_oversized_tail_after_cap() {
        let mut result = (0..MAX_VALIDATED_CODE_LENSES)
            .map(|index| code_lens_item(&format!("Run Test {index:03}")))
            .collect::<Vec<_>>();
        result.push(json!({
            "range": {
                "start": { "line": 2, "character": 4 },
                "end": { "line": 2, "character": 4 }
            },
            "data": "x".repeat(crate::lsp_client::pending::MAX_LSP_OUTBOUND_JSON_PAYLOAD_BYTES)
        }));

        assert_invalid_code_lenses_result(json!({ "result": result }));
    }

    #[test]
    fn code_lens_resolve_result_sends_resolved_lens_event() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_code_lens_resolve_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({
                "result": {
                    "range": {
                        "start": { "line": 2, "character": 4 },
                        "end": { "line": 2, "character": 4 }
                    },
                    "command": {
                        "title": "Run Test",
                        "command": "rust-analyzer.runSingle"
                    }
                }
            }),
            &tx,
        );

        match rx.recv().expect("code lens resolve result event") {
            UiEvent::Lsp(LspUiEvent::CodeLensResolveResult {
                id,
                path,
                version,
                lens,
                error,
            }) => {
                assert_eq!(id, 7);
                assert_eq!(path, PathBuf::from("src/main.rs"));
                assert_eq!(version, 11);
                assert_eq!(
                    lens.as_ref().map(|lens| lens.title.as_str()),
                    Some("Run Test")
                );
                assert_eq!(error, None);
            }
            other => panic!("expected code lens resolve result event, got {other:?}"),
        }
    }

    #[test]
    fn code_lens_resolve_result_reports_invalid_success_payload() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_code_lens_resolve_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({ "result": null }),
            &tx,
        );

        match rx.recv().expect("code lens resolve result event") {
            UiEvent::Lsp(LspUiEvent::CodeLensResolveResult {
                id,
                path,
                version,
                lens,
                error,
            }) => {
                assert_eq!(id, 7);
                assert_eq!(path, PathBuf::from("src/main.rs"));
                assert_eq!(version, 11);
                assert_eq!(lens, None);
                assert_eq!(error.as_deref(), Some(INVALID_CODE_LENS_RESOLVE_RESPONSE));
            }
            other => panic!("expected code lens resolve result event, got {other:?}"),
        }
    }

    #[test]
    fn code_lens_resolve_result_reports_invalid_command_payload_shape() {
        assert_invalid_code_lens_resolve_result(json!({
            "result": {
                "range": {
                    "start": { "line": 2, "character": 4 },
                    "end": { "line": 2, "character": 4 }
                },
                "command": {
                    "title": "Run Test"
                }
            }
        }));
    }

    #[test]
    fn code_lens_resolve_result_reports_error_payload_without_message_as_invalid() {
        assert_invalid_code_lens_resolve_result(json!({
            "error": { "code": -32000 },
            "result": null
        }));
    }

    #[test]
    fn execute_command_result_sends_status_event() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_execute_command_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            "Run Test".to_owned(),
            "rust-analyzer.runSingle".to_owned(),
            &json!({ "result": null }),
            &tx,
        );

        match rx.recv().expect("execute command result event") {
            UiEvent::Lsp(LspUiEvent::CodeLensCommandResult {
                id,
                path,
                version,
                title,
                command,
                error,
            }) => {
                assert_eq!(id, 7);
                assert_eq!(path, PathBuf::from("src/main.rs"));
                assert_eq!(version, 11);
                assert_eq!(title, "Run Test");
                assert_eq!(command, "rust-analyzer.runSingle");
                assert_eq!(error, None);
            }
            other => panic!("expected execute command result event, got {other:?}"),
        }
    }
}
