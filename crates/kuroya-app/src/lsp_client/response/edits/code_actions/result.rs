use super::super::super::response_error;
use crate::ui_event_channel::Sender;
use crate::{lsp_ui_events::LspUiEvent, ui_events::UiEvent};
use kuroya_core::{
    BufferId, LspCodeAction, parse_code_action_resolve_response, parse_code_action_response,
};
use serde_json::{Map, Value};
use std::io::{self, Write};
use std::path::PathBuf;

const INVALID_CODE_ACTIONS_RESPONSE: &str = "invalid textDocument/codeAction response";
const INVALID_CODE_ACTION_RESOLVE_RESPONSE: &str = "invalid codeAction/resolve response";
const MAX_VALIDATED_CODE_ACTIONS: usize = 100;
const MAX_VALIDATED_TEXT_EDITS: usize = 2_000;
const MAX_VALIDATED_TEXT_EDIT_NEW_TEXT_BYTES: usize = 2 * 1024 * 1024;
const MAX_VALIDATED_CODE_ACTION_COMMAND_TITLE_CHARS: usize = 512;
const MAX_VALIDATED_CODE_ACTION_COMMAND_ID_CHARS: usize = 512;
const MAX_CODE_ACTION_COMMAND_ARGUMENTS_PAYLOAD_BYTES: usize = 64 * 1024;
const MAX_CODE_ACTION_RESOLVE_PAYLOAD_BYTES: usize = 64 * 1024;
const MAX_CODE_ACTION_POSITION_COMPONENT: u64 = i32::MAX as u64;

pub(super) fn send_code_actions_result(
    id: BufferId,
    path: PathBuf,
    version: u64,
    line: usize,
    character: usize,
    value: &Value,
    ui_tx: &Sender<UiEvent>,
) {
    let mut error = response_error(value);
    let actions = if error.is_none() {
        match parse_code_actions_success_response(value) {
            Some(actions) => Some(actions),
            None => {
                error = Some(INVALID_CODE_ACTIONS_RESPONSE.to_owned());
                None
            }
        }
    } else {
        None
    };
    let _ = crate::lsp_client::response::emit_critical_lsp_response_event(
        ui_tx,
        LspUiEvent::CodeActionsResult {
            id,
            path,
            version,
            line,
            column: code_action_origin_column(character),
            actions,
            error,
        },
    );
}

pub(super) fn send_code_action_resolve_result(
    id: BufferId,
    path: PathBuf,
    version: u64,
    line: usize,
    character: usize,
    value: &Value,
    ui_tx: &Sender<UiEvent>,
) {
    let mut error = response_error(value);
    let action = if error.is_none() {
        match parse_code_action_resolve_success_response(value) {
            Some(action) => Some(action),
            None => {
                error = Some(INVALID_CODE_ACTION_RESOLVE_RESPONSE.to_owned());
                None
            }
        }
    } else {
        None
    };
    let _ = crate::lsp_client::response::emit_critical_lsp_response_event(
        ui_tx,
        LspUiEvent::CodeActionResolveResult {
            id,
            path,
            version,
            line,
            column: code_action_origin_column(character),
            action,
            error,
        },
    );
}

fn parse_code_actions_success_response(value: &Value) -> Option<Vec<LspCodeAction>> {
    let result = value.get("result")?;
    if result.is_null() {
        return Some(Vec::new());
    }

    let items = result.as_array()?;
    if items.len() > MAX_VALIDATED_CODE_ACTIONS
        || items
            .iter()
            .any(|item| !code_action_item_shape_is_valid(item))
    {
        return None;
    }

    parse_code_action_response(value)
}

fn parse_code_action_resolve_success_response(value: &Value) -> Option<LspCodeAction> {
    let result = value.get("result")?;
    if result.is_null() || !code_action_item_shape_is_valid(result) {
        return None;
    }

    parse_code_action_resolve_response(value)
}

fn code_action_item_shape_is_valid(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    let Some(title) = object.get("title").and_then(Value::as_str) else {
        return false;
    };
    if title.trim().is_empty() {
        return false;
    }

    if object.get("kind").is_some_and(|kind| !kind.is_string())
        || object
            .get("diagnostics")
            .is_some_and(|diagnostics| !diagnostics.is_array())
        || object
            .get("isPreferred")
            .is_some_and(|is_preferred| is_preferred.as_bool().is_none())
        || object
            .get("disabled")
            .is_some_and(|disabled| !code_action_disabled_is_valid(disabled))
        || object
            .get("command")
            .is_some_and(|command| !code_action_command_is_valid(command))
        || object.get("data").is_some_and(|data| {
            !json_payload_is_bounded(data, MAX_CODE_ACTION_RESOLVE_PAYLOAD_BYTES)
        })
    {
        return false;
    }

    if let Some(edit) = object.get("edit")
        && !workspace_edit_shape_is_valid(edit)
    {
        return false;
    }

    if object.get("edit").is_none()
        && object.get("data").is_some()
        && object.get("disabled").is_none()
        && !resolvable_code_action_shape_is_valid(value)
    {
        return false;
    }

    true
}

fn code_action_disabled_is_valid(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    object.get("reason").and_then(Value::as_str).is_some()
}

fn code_action_command_is_valid(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    required_bounded_non_empty_string(
        object,
        "title",
        MAX_VALIDATED_CODE_ACTION_COMMAND_TITLE_CHARS,
    ) && required_bounded_non_empty_string(
        object,
        "command",
        MAX_VALIDATED_CODE_ACTION_COMMAND_ID_CHARS,
    ) && object.get("arguments").is_none_or(|arguments| {
        arguments.is_array()
            && json_payload_is_bounded(arguments, MAX_CODE_ACTION_COMMAND_ARGUMENTS_PAYLOAD_BYTES)
    })
}

fn workspace_edit_shape_is_valid(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };

    let mut text_edit_count = 0usize;
    object
        .get("changes")
        .is_none_or(|changes| workspace_changes_are_valid(changes, &mut text_edit_count))
        && object
            .get("documentChanges")
            .is_none_or(|document_changes| {
                workspace_document_changes_are_valid(document_changes, &mut text_edit_count)
            })
}

fn workspace_changes_are_valid(value: &Value, text_edit_count: &mut usize) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };

    object
        .values()
        .all(|edits| text_edit_array_is_valid(edits, text_edit_count))
}

fn workspace_document_changes_are_valid(value: &Value, text_edit_count: &mut usize) -> bool {
    value.as_array().is_some_and(|changes| {
        changes.iter().all(|change| {
            resource_operation_is_valid(change)
                || text_document_edit_is_valid(change, text_edit_count)
        })
    })
}

fn text_document_edit_is_valid(value: &Value, text_edit_count: &mut usize) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };

    object
        .get("textDocument")
        .is_some_and(versioned_text_document_identifier_is_valid)
        && object
            .get("edits")
            .is_some_and(|edits| text_edit_array_is_valid(edits, text_edit_count))
}

fn versioned_text_document_identifier_is_valid(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };

    object.get("uri").and_then(Value::as_str).is_some()
        && object.get("version").is_none_or(|version| {
            version.is_null() || version.as_i64().is_some() || version.as_u64().is_some()
        })
}

fn resource_operation_is_valid(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };

    match object.get("kind").and_then(Value::as_str) {
        Some("create" | "delete") => object.get("uri").and_then(Value::as_str).is_some(),
        Some("rename") => {
            object.get("oldUri").and_then(Value::as_str).is_some()
                && object.get("newUri").and_then(Value::as_str).is_some()
        }
        _ => false,
    }
}

fn text_edit_is_valid(value: &Value) -> bool {
    value
        .get("newText")
        .and_then(Value::as_str)
        .is_some_and(|text| text.len() <= MAX_VALIDATED_TEXT_EDIT_NEW_TEXT_BYTES)
        && value.get("range").is_some_and(is_lsp_range)
}

fn text_edit_array_is_valid(value: &Value, text_edit_count: &mut usize) -> bool {
    let Some(edits) = value.as_array() else {
        return false;
    };
    let Some(next_count) = text_edit_count.checked_add(edits.len()) else {
        return false;
    };
    if next_count > MAX_VALIDATED_TEXT_EDITS {
        return false;
    }
    *text_edit_count = next_count;
    edits.iter().all(text_edit_is_valid)
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
    Some((
        lsp_position_component(value.get("line")?)?,
        lsp_position_component(value.get("character")?)?,
    ))
}

fn lsp_position_component(value: &Value) -> Option<u64> {
    let component = value.as_u64()?;
    (component <= MAX_CODE_ACTION_POSITION_COMPONENT).then_some(component)
}

fn resolvable_code_action_shape_is_valid(value: &Value) -> bool {
    json_payload_is_bounded(value, MAX_CODE_ACTION_RESOLVE_PAYLOAD_BYTES)
}

fn json_payload_is_bounded(value: &Value, max_bytes: usize) -> bool {
    let mut counter = CountingWriter::new(max_bytes);
    serde_json::to_writer(&mut counter, value).is_ok()
}

fn required_bounded_non_empty_string(
    object: &Map<String, Value>,
    key: &str,
    max_chars: usize,
) -> bool {
    object
        .get(key)
        .and_then(Value::as_str)
        .is_some_and(|text| bounded_string_chars(text, max_chars) && !text.trim().is_empty())
}

fn bounded_string_chars(text: &str, max_chars: usize) -> bool {
    if text.len() > max_chars.saturating_mul(4) {
        return false;
    }
    text.chars().take(max_chars.saturating_add(1)).count() <= max_chars
}

fn code_action_origin_column(character: usize) -> usize {
    character.saturating_add(1)
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
                "code action resolve payload too large",
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
        CountingWriter, INVALID_CODE_ACTION_RESOLVE_RESPONSE, INVALID_CODE_ACTIONS_RESPONSE,
        MAX_CODE_ACTION_COMMAND_ARGUMENTS_PAYLOAD_BYTES, MAX_CODE_ACTION_POSITION_COMPONENT,
        MAX_CODE_ACTION_RESOLVE_PAYLOAD_BYTES, MAX_VALIDATED_CODE_ACTION_COMMAND_TITLE_CHARS,
        MAX_VALIDATED_CODE_ACTIONS, MAX_VALIDATED_TEXT_EDIT_NEW_TEXT_BYTES,
        resolvable_code_action_shape_is_valid, send_code_action_resolve_result,
        send_code_actions_result,
    };
    use crate::{lsp_ui_events::LspUiEvent, ui_events::UiEvent};
    use kuroya_core::{LspWorkspaceDocumentChange, LspWorkspaceResourceOperation};
    use serde_json::{Value, json};
    use std::{io::Write, path::PathBuf};

    fn valid_edit_action(title: impl Into<String>) -> Value {
        let title = title.into();
        json!({
            "title": title,
            "kind": "quickfix",
            "edit": {
                "changes": {
                    "file:///src/main.rs": [{
                        "range": {
                            "start": { "line": 0, "character": 0 },
                            "end": { "line": 0, "character": 0 }
                        },
                        "newText": ""
                    }]
                }
            }
        })
    }

    fn assert_invalid_code_actions_response(value: Value) {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_code_actions_result(7, PathBuf::from("src/main.rs"), 11, 3, 12, &value, &tx);

        match rx.recv().expect("code actions result event") {
            UiEvent::Lsp(LspUiEvent::CodeActionsResult { actions, error, .. }) => {
                assert_eq!(actions, None);
                assert_eq!(error.as_deref(), Some(INVALID_CODE_ACTIONS_RESPONSE));
            }
            other => panic!("expected code actions result event, got {other:?}"),
        }
    }

    fn assert_invalid_code_action_resolve_response(value: Value) {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_code_action_resolve_result(7, PathBuf::from("src/main.rs"), 11, 3, 12, &value, &tx);

        match rx.recv().expect("code action resolve result event") {
            UiEvent::Lsp(LspUiEvent::CodeActionResolveResult { action, error, .. }) => {
                assert_eq!(action, None);
                assert_eq!(error.as_deref(), Some(INVALID_CODE_ACTION_RESOLVE_RESPONSE));
            }
            other => panic!("expected code action resolve result event, got {other:?}"),
        }
    }

    #[test]
    fn code_actions_result_reports_invalid_success_payload() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_code_actions_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            12,
            &json!({ "result": { "not": "an array" } }),
            &tx,
        );

        match rx.recv().expect("code actions result event") {
            UiEvent::Lsp(LspUiEvent::CodeActionsResult { actions, error, .. }) => {
                assert_eq!(actions, None);
                assert_eq!(error.as_deref(), Some(INVALID_CODE_ACTIONS_RESPONSE));
            }
            other => panic!("expected code actions result event, got {other:?}"),
        }
    }

    #[test]
    fn code_actions_result_reports_malformed_action_item() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_code_actions_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            12,
            &json!({
                "result": [{
                    "title": "Import HashMap",
                    "kind": "quickfix",
                    "edit": {
                        "changes": {
                            "file:///src/main.rs": [{
                                "range": {
                                    "start": { "line": 0 },
                                    "end": { "line": 0, "character": 0 }
                                },
                                "newText": "use std::collections::HashMap;\n"
                            }]
                        }
                    }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("code actions result event") {
            UiEvent::Lsp(LspUiEvent::CodeActionsResult { actions, error, .. }) => {
                assert_eq!(actions, None);
                assert_eq!(error.as_deref(), Some(INVALID_CODE_ACTIONS_RESPONSE));
            }
            other => panic!("expected code actions result event, got {other:?}"),
        }
    }

    #[test]
    fn code_actions_result_rejects_malformed_action_tail_after_emitted_cap() {
        let mut actions = (0..MAX_VALIDATED_CODE_ACTIONS)
            .map(|idx| valid_edit_action(format!("Action {idx}")))
            .collect::<Vec<_>>();
        actions.push(json!({
            "kind": "quickfix",
            "edit": {
                "changes": {
                    "file:///src/main.rs": [{
                        "range": {
                            "start": { "line": "bad", "character": 0 },
                            "end": { "line": 0, "character": 0 }
                        },
                        "newText": ""
                    }]
                }
            }
        }));

        assert_invalid_code_actions_response(json!({ "result": actions }));
    }

    #[test]
    fn code_actions_result_rejects_oversized_action_tail_after_emitted_cap() {
        let actions = (0..=MAX_VALIDATED_CODE_ACTIONS)
            .map(|idx| valid_edit_action(format!("Action {idx}")))
            .collect::<Vec<_>>();

        assert_invalid_code_actions_response(json!({ "result": actions }));
    }

    #[test]
    fn code_actions_result_rejects_reversed_edit_ranges() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_code_actions_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            12,
            &json!({
                "result": [{
                    "title": "Import HashMap",
                    "kind": "quickfix",
                    "edit": {
                        "changes": {
                            "file:///src/main.rs": [{
                                "range": {
                                    "start": { "line": 2, "character": 0 },
                                    "end": { "line": 1, "character": 0 }
                                },
                                "newText": "use std::collections::HashMap;\n"
                            }]
                        }
                    }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("code actions result event") {
            UiEvent::Lsp(LspUiEvent::CodeActionsResult { actions, error, .. }) => {
                assert_eq!(actions, None);
                assert_eq!(error.as_deref(), Some(INVALID_CODE_ACTIONS_RESPONSE));
            }
            other => panic!("expected code actions result event, got {other:?}"),
        }
    }

    #[test]
    fn code_actions_result_rejects_position_components_above_core_bound() {
        let too_large = MAX_CODE_ACTION_POSITION_COMPONENT + 1;
        assert_invalid_code_actions_response(json!({
            "result": [{
                "title": "Import HashMap",
                "kind": "quickfix",
                "edit": {
                    "changes": {
                        "file:///src/main.rs": [{
                            "range": {
                                "start": { "line": too_large, "character": 0 },
                                "end": { "line": too_large, "character": 0 }
                            },
                            "newText": "use std::collections::HashMap;\n"
                        }]
                    }
                }
            }]
        }));
    }

    #[test]
    fn code_actions_result_rejects_oversized_workspace_edit_new_text() {
        assert_invalid_code_actions_response(json!({
            "result": [{
                "title": "Import HashMap",
                "kind": "quickfix",
                "edit": {
                    "changes": {
                        "file:///src/main.rs": [{
                            "range": {
                                "start": { "line": 0, "character": 0 },
                                "end": { "line": 0, "character": 0 }
                            },
                            "newText": "x".repeat(MAX_VALIDATED_TEXT_EDIT_NEW_TEXT_BYTES + 1)
                        }]
                    }
                }
            }]
        }));
    }

    #[test]
    fn code_actions_result_rejects_oversized_command_arguments() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_code_actions_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            12,
            &json!({
                "result": [{
                    "title": "Import HashMap",
                    "kind": "quickfix",
                    "command": {
                        "title": "Fix",
                        "command": "rust.fix",
                        "arguments": ["x".repeat(MAX_CODE_ACTION_COMMAND_ARGUMENTS_PAYLOAD_BYTES)]
                    },
                    "edit": {
                        "changes": {
                            "file:///src/main.rs": [{
                                "range": {
                                    "start": { "line": 0, "character": 0 },
                                    "end": { "line": 0, "character": 0 }
                                },
                                "newText": ""
                            }]
                        }
                    }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("code actions result event") {
            UiEvent::Lsp(LspUiEvent::CodeActionsResult { actions, error, .. }) => {
                assert_eq!(actions, None);
                assert_eq!(error.as_deref(), Some(INVALID_CODE_ACTIONS_RESPONSE));
            }
            other => panic!("expected code actions result event, got {other:?}"),
        }
    }

    #[test]
    fn code_actions_result_rejects_malformed_command_payload() {
        assert_invalid_code_actions_response(json!({
            "result": [{
                "title": "Import HashMap",
                "kind": "quickfix",
                "command": "rust.fix",
                "edit": {
                    "changes": {
                        "file:///src/main.rs": [{
                            "range": {
                                "start": { "line": 0, "character": 0 },
                                "end": { "line": 0, "character": 0 }
                            },
                            "newText": ""
                        }]
                    }
                }
            }]
        }));
    }

    #[test]
    fn code_actions_result_rejects_oversized_command_metadata() {
        assert_invalid_code_actions_response(json!({
            "result": [{
                "title": "Import HashMap",
                "kind": "quickfix",
                "command": {
                    "title": "x".repeat(MAX_VALIDATED_CODE_ACTION_COMMAND_TITLE_CHARS + 1),
                    "command": "rust.fix"
                },
                "edit": {
                    "changes": {
                        "file:///src/main.rs": [{
                            "range": {
                                "start": { "line": 0, "character": 0 },
                                "end": { "line": 0, "character": 0 }
                            },
                            "newText": ""
                        }]
                    }
                }
            }]
        }));
    }

    #[test]
    fn code_actions_result_rejects_oversized_action_data_payload() {
        let mut action = valid_edit_action("Import HashMap");
        action["data"] = json!({
            "request": "x".repeat(MAX_CODE_ACTION_RESOLVE_PAYLOAD_BYTES)
        });

        assert_invalid_code_actions_response(json!({ "result": [action] }));
    }

    #[test]
    fn code_actions_result_treats_null_success_as_empty_actions() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_code_actions_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            12,
            &json!({ "result": null }),
            &tx,
        );

        match rx.recv().expect("code actions result event") {
            UiEvent::Lsp(LspUiEvent::CodeActionsResult { actions, error, .. }) => {
                assert_eq!(actions, Some(Vec::new()));
                assert_eq!(error, None);
            }
            other => panic!("expected code actions result event, got {other:?}"),
        }
    }

    #[test]
    fn code_actions_result_reports_the_origin_cursor_column() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_code_actions_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            12,
            &json!({ "result": [] }),
            &tx,
        );

        match rx.recv().expect("code actions result event") {
            UiEvent::Lsp(LspUiEvent::CodeActionsResult {
                id,
                path,
                version,
                line,
                column,
                actions,
                error,
            }) => {
                assert_eq!(id, 7);
                assert_eq!(path, PathBuf::from("src/main.rs"));
                assert_eq!(version, 11);
                assert_eq!(line, 3);
                assert_eq!(column, 13);
                assert_eq!(actions, Some(Vec::new()));
                assert_eq!(error, None);
            }
            other => panic!("expected code actions result event, got {other:?}"),
        }
    }

    #[test]
    fn code_actions_result_saturates_unrepresentable_origin_cursor_column() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_code_actions_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            usize::MAX,
            &json!({ "result": [] }),
            &tx,
        );

        match rx.recv().expect("code actions result event") {
            UiEvent::Lsp(LspUiEvent::CodeActionsResult {
                column,
                actions,
                error,
                ..
            }) => {
                assert_eq!(column, usize::MAX);
                assert_eq!(actions, Some(Vec::new()));
                assert_eq!(error, None);
            }
            other => panic!("expected code actions result event, got {other:?}"),
        }
    }

    #[test]
    fn code_actions_result_preserves_actions_with_resource_document_changes() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_code_actions_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            12,
            &json!({
                "result": [{
                    "title": "Create module and import it",
                    "kind": "quickfix",
                    "edit": {
                        "documentChanges": [
                            {
                                "textDocument": {
                                    "uri": "file:///src/main.rs",
                                    "version": 3
                                },
                                "edits": [{
                                    "range": {
                                        "start": { "line": 0, "character": 0 },
                                        "end": { "line": 0, "character": 0 }
                                    },
                                    "newText": "mod generated;\n"
                                }]
                            },
                            {
                                "kind": "create",
                                "uri": "file:///src/generated.rs"
                            }
                        ]
                    }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("code actions result event") {
            UiEvent::Lsp(LspUiEvent::CodeActionsResult {
                id,
                path,
                version,
                line,
                column,
                actions,
                error,
            }) => {
                assert_eq!(id, 7);
                assert_eq!(path, PathBuf::from("src/main.rs"));
                assert_eq!(version, 11);
                assert_eq!(line, 3);
                assert_eq!(column, 13);
                let actions = actions.expect("actions");
                assert_eq!(actions.len(), 1);
                assert_eq!(actions[0].title, "Create module and import it");
                assert_eq!(actions[0].edits.len(), 1);
                assert_eq!(actions[0].document_changes.len(), 2);
                assert!(matches!(
                    &actions[0].document_changes[1],
                    LspWorkspaceDocumentChange::Resource(
                        LspWorkspaceResourceOperation::CreateFile { .. }
                    )
                ));
                assert_eq!(error, None);
            }
            other => panic!("expected code actions result event, got {other:?}"),
        }
    }

    #[test]
    fn code_action_resolve_result_reports_invalid_success_payload() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_code_action_resolve_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            12,
            &json!({
                "result": {
                    "title": "Import HashMap",
                    "kind": "quickfix",
                    "edit": {
                        "changes": {
                            "file:///src/main.rs": [{
                                "range": {
                                    "start": { "line": 0, "character": 0 },
                                    "end": { "line": "bad", "character": 0 }
                                },
                                "newText": "use std::collections::HashMap;\n"
                            }]
                        }
                    }
                }
            }),
            &tx,
        );

        match rx.recv().expect("code action resolve result event") {
            UiEvent::Lsp(LspUiEvent::CodeActionResolveResult { action, error, .. }) => {
                assert_eq!(action, None);
                assert_eq!(error.as_deref(), Some(INVALID_CODE_ACTION_RESOLVE_RESPONSE));
            }
            other => panic!("expected code action resolve result event, got {other:?}"),
        }
    }

    #[test]
    fn code_action_resolve_result_rejects_position_components_above_core_bound() {
        let too_large = MAX_CODE_ACTION_POSITION_COMPONENT + 1;
        assert_invalid_code_action_resolve_response(json!({
            "result": {
                "title": "Import HashMap",
                "kind": "quickfix",
                "edit": {
                    "changes": {
                        "file:///src/main.rs": [{
                            "range": {
                                "start": { "line": 0, "character": too_large },
                                "end": { "line": 0, "character": too_large }
                            },
                            "newText": "use std::collections::HashMap;\n"
                        }]
                    }
                }
            }
        }));
    }

    #[test]
    fn code_action_resolve_result_rejects_oversized_workspace_edit_new_text() {
        assert_invalid_code_action_resolve_response(json!({
            "result": {
                "title": "Import HashMap",
                "kind": "quickfix",
                "edit": {
                    "changes": {
                        "file:///src/main.rs": [{
                            "range": {
                                "start": { "line": 0, "character": 0 },
                                "end": { "line": 0, "character": 0 }
                            },
                            "newText": "x".repeat(MAX_VALIDATED_TEXT_EDIT_NEW_TEXT_BYTES + 1)
                        }]
                    }
                }
            }
        }));
    }

    #[test]
    fn code_action_resolve_result_rejects_oversized_action_data_payload() {
        let mut action = valid_edit_action("Import HashMap");
        action["data"] = json!({
            "request": "x".repeat(MAX_CODE_ACTION_RESOLVE_PAYLOAD_BYTES)
        });

        assert_invalid_code_action_resolve_response(json!({ "result": action }));
    }

    #[test]
    fn code_action_resolve_result_reports_null_success_as_invalid() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_code_action_resolve_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            12,
            &json!({ "result": null }),
            &tx,
        );

        match rx.recv().expect("code action resolve result event") {
            UiEvent::Lsp(LspUiEvent::CodeActionResolveResult { action, error, .. }) => {
                assert_eq!(action, None);
                assert_eq!(error.as_deref(), Some(INVALID_CODE_ACTION_RESOLVE_RESPONSE));
            }
            other => panic!("expected code action resolve result event, got {other:?}"),
        }
    }

    #[test]
    fn code_action_resolve_result_reports_the_origin_cursor_column() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_code_action_resolve_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            12,
            &json!({
                "result": {
                    "title": "Import HashMap",
                    "kind": "quickfix",
                    "edit": {
                        "changes": {
                            "file:///src/main.rs": [{
                                "range": {
                                    "start": { "line": 0, "character": 0 },
                                    "end": { "line": 0, "character": 0 }
                                },
                                "newText": "use std::collections::HashMap;\n"
                            }]
                        }
                    }
                }
            }),
            &tx,
        );

        match rx.recv().expect("code action resolve result event") {
            UiEvent::Lsp(LspUiEvent::CodeActionResolveResult {
                id,
                path,
                version,
                line,
                column,
                action,
                error,
            }) => {
                assert_eq!(id, 7);
                assert_eq!(path, PathBuf::from("src/main.rs"));
                assert_eq!(version, 11);
                assert_eq!(line, 3);
                assert_eq!(column, 13);
                assert_eq!(
                    action.as_ref().map(|action| action.title.as_str()),
                    Some("Import HashMap")
                );
                assert_eq!(error, None);
            }
            other => panic!("expected code action resolve result event, got {other:?}"),
        }
    }

    #[test]
    fn code_action_resolve_result_saturates_unrepresentable_origin_cursor_column() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_code_action_resolve_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            usize::MAX,
            &json!({
                "result": {
                    "title": "Import HashMap",
                    "kind": "quickfix",
                    "edit": {
                        "changes": {
                            "file:///src/main.rs": [{
                                "range": {
                                    "start": { "line": 0, "character": 0 },
                                    "end": { "line": 0, "character": 0 }
                                },
                                "newText": "use std::collections::HashMap;\n"
                            }]
                        }
                    }
                }
            }),
            &tx,
        );

        match rx.recv().expect("code action resolve result event") {
            UiEvent::Lsp(LspUiEvent::CodeActionResolveResult {
                column,
                action,
                error,
                ..
            }) => {
                assert_eq!(column, usize::MAX);
                assert_eq!(
                    action.as_ref().map(|action| action.title.as_str()),
                    Some("Import HashMap")
                );
                assert_eq!(error, None);
            }
            other => panic!("expected code action resolve result event, got {other:?}"),
        }
    }

    #[test]
    fn code_action_resolve_result_preserves_resource_document_changes_without_error() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_code_action_resolve_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            12,
            &json!({
                "result": {
                    "title": "Create module and import it",
                    "kind": "quickfix",
                    "data": { "id": 7 },
                    "edit": {
                        "documentChanges": [
                            {
                                "textDocument": {
                                    "uri": "file:///src/main.rs",
                                    "version": 3
                                },
                                "edits": [{
                                    "range": {
                                        "start": { "line": 0, "character": 0 },
                                        "end": { "line": 0, "character": 0 }
                                    },
                                    "newText": "mod generated;\n"
                                }]
                            },
                            {
                                "kind": "create",
                                "uri": "file:///src/generated.rs"
                            }
                        ]
                    }
                }
            }),
            &tx,
        );

        match rx.recv().expect("code action resolve result event") {
            UiEvent::Lsp(LspUiEvent::CodeActionResolveResult {
                id,
                path,
                version,
                line,
                column,
                action,
                error,
            }) => {
                assert_eq!(id, 7);
                assert_eq!(path, PathBuf::from("src/main.rs"));
                assert_eq!(version, 11);
                assert_eq!(line, 3);
                assert_eq!(column, 13);
                let action = action.expect("action");
                assert_eq!(action.title, "Create module and import it");
                assert_eq!(action.edits.len(), 1);
                assert_eq!(action.document_changes.len(), 2);
                assert!(matches!(
                    &action.document_changes[1],
                    LspWorkspaceDocumentChange::Resource(
                        LspWorkspaceResourceOperation::CreateFile { .. }
                    )
                ));
                assert_eq!(error, None);
            }
            other => panic!("expected code action resolve result event, got {other:?}"),
        }
    }

    #[test]
    fn oversized_resolvable_code_action_payload_is_rejected() {
        assert!(resolvable_code_action_shape_is_valid(&json!({
            "title": "Import HashMap",
            "data": { "request": "small" }
        })));
        assert!(!resolvable_code_action_shape_is_valid(&json!({
            "title": "Import HashMap",
            "data": { "request": "x".repeat(MAX_CODE_ACTION_RESOLVE_PAYLOAD_BYTES) }
        })));
    }

    #[test]
    fn counting_writer_stops_at_payload_limit() {
        let mut writer = CountingWriter::new(4);

        writer
            .write_all(b"rust")
            .expect("exactly bounded write should succeed");
        assert!(writer.write_all(b"!").is_err());
        assert_eq!(writer.bytes, 4);
    }
}
