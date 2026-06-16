use super::super::super::response_error;
use crate::ui_event_channel::Sender;
use crate::{
    lsp_completion_resolve::CompletionResolveIntent, lsp_ui_events::LspUiEvent, ui_events::UiEvent,
};
use kuroya_core::{
    BufferId, LspCompletionItem, parse_completion_item_resolve_response, parse_completion_response,
};
use serde_json::{Map, Value};
use std::{
    io::{self, Write},
    path::{Path, PathBuf},
};

const INVALID_COMPLETION_RESPONSE: &str = "invalid textDocument/completion response";
const INVALID_COMPLETION_ITEM_RESOLVE_RESPONSE: &str = "invalid completionItem/resolve response";
const MAX_VALIDATED_COMPLETION_ITEMS: usize = 200;
const MAX_VALIDATED_COMPLETION_LABEL_CHARS: usize = 512;
const MAX_VALIDATED_COMPLETION_DETAIL_CHARS: usize = 1_024;
const MAX_VALIDATED_COMPLETION_DOCUMENTATION_CHARS: usize = 16_000;
const MAX_VALIDATED_COMPLETION_DOCUMENTATION_KIND_CHARS: usize = 64;
const MAX_VALIDATED_COMPLETION_SORT_TEXT_CHARS: usize = 512;
const MAX_VALIDATED_COMPLETION_FILTER_TEXT_CHARS: usize = 512;
const MAX_VALIDATED_COMPLETION_COMMIT_CHARACTERS: usize = 16;
const MAX_VALIDATED_COMPLETION_COMMIT_CHARACTER_CHARS: usize = 16;
const MAX_VALIDATED_COMPLETION_TAGS: usize = 8;
const MAX_VALIDATED_COMPLETION_LABEL_DETAIL_CHARS: usize = 1_024;
const MAX_VALIDATED_COMPLETION_ADDITIONAL_TEXT_EDITS: usize = 2_000;
const MAX_VALIDATED_TEXT_EDIT_NEW_TEXT_BYTES: usize = 2 * 1024 * 1024;
const MAX_VALIDATED_SNIPPET_INSERT_TEXT_BYTES: usize = 64 * 1024;
const MAX_VALIDATED_COMPLETION_COMMAND_TITLE_CHARS: usize = 512;
const MAX_VALIDATED_COMPLETION_COMMAND_ID_CHARS: usize = 512;
const MAX_VALIDATED_COMPLETION_RESOLVE_PAYLOAD_BYTES: usize = 64 * 1024;
const MAX_COMPLETION_COMMAND_ARGUMENTS_PAYLOAD_BYTES: usize = 64 * 1024;
const MAX_VALIDATED_COMPLETION_POSITION_COMPONENT: u64 = i32::MAX as u64;

pub(super) fn send_completion_result(
    id: BufferId,
    path: PathBuf,
    version: u64,
    line: usize,
    character: usize,
    value: &Value,
    ui_tx: &Sender<UiEvent>,
) {
    let mut error = response_error(value);
    let items = if error.is_none() {
        match parse_completion_success_response(value, &path) {
            Some(items) => Some(items),
            None => {
                error = Some(INVALID_COMPLETION_RESPONSE.to_owned());
                None
            }
        }
    } else {
        None
    };
    let _ = crate::lsp_client::response::emit_critical_lsp_response_event(
        ui_tx,
        LspUiEvent::CompletionResult {
            id,
            path,
            version,
            line,
            column: completion_event_column(character),
            items,
            error,
        },
    );
}

pub(super) fn send_completion_item_resolve_result(
    id: BufferId,
    path: PathBuf,
    version: u64,
    line: usize,
    character: usize,
    original_item: LspCompletionItem,
    intent: CompletionResolveIntent,
    value: &Value,
    ui_tx: &Sender<UiEvent>,
) {
    let mut error = response_error(value);
    let item = if error.is_none() {
        match parse_completion_item_resolve_success_response(value, &path, &original_item) {
            Some(item) => Some(item),
            None => {
                error = Some(INVALID_COMPLETION_ITEM_RESOLVE_RESPONSE.to_owned());
                None
            }
        }
    } else {
        None
    };
    let is_apply_intent = matches!(&intent, CompletionResolveIntent::Apply { .. });
    let event = LspUiEvent::CompletionItemResolveResult {
        id,
        path,
        version,
        line,
        column: completion_event_column(character),
        item: item.map(Box::new),
        fallback_item: Box::new(original_item),
        intent,
        error,
    };
    let _ = if is_apply_intent {
        crate::lsp_client::response::emit_critical_lsp_response_event(ui_tx, event)
    } else {
        crate::lsp_client::response::emit_lsp_response_event(ui_tx, event)
    };
}

fn parse_completion_success_response(value: &Value, path: &Path) -> Option<Vec<LspCompletionItem>> {
    let result = value.get("result")?;
    if result.is_null() {
        return Some(Vec::new());
    }

    let items = completion_result_items(result)?;
    let defaults = CompletionValidationDefaults::from_completion_result(result);
    let expected_items = items.len().min(MAX_VALIDATED_COMPLETION_ITEMS);
    if items
        .iter()
        .take(MAX_VALIDATED_COMPLETION_ITEMS)
        .any(|item| !completion_item_shape_is_valid(item, defaults))
    {
        return None;
    }

    let parsed = parse_completion_response(value, path)?;
    (parsed.len() == expected_items).then_some(parsed)
}

fn parse_completion_item_resolve_success_response(
    value: &Value,
    path: &Path,
    original_item: &LspCompletionItem,
) -> Option<LspCompletionItem> {
    let result = value.get("result")?;
    if result.is_null()
        || !completion_item_shape_is_valid(result, CompletionValidationDefaults::default())
    {
        return None;
    }

    parse_completion_item_resolve_response(value, path, original_item)
}

fn completion_result_items(result: &Value) -> Option<&[Value]> {
    if let Some(items) = result.as_array() {
        return Some(items.as_slice());
    }

    let object = result.as_object()?;
    if object
        .get("isIncomplete")
        .is_some_and(|is_incomplete| is_incomplete.as_bool().is_none())
        || object
            .get("itemDefaults")
            .is_some_and(|defaults| !completion_item_defaults_are_valid(defaults))
    {
        return None;
    }

    object.get("items")?.as_array().map(Vec::as_slice)
}

fn completion_item_defaults_are_valid(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };

    optional_string_array(
        object,
        "commitCharacters",
        MAX_VALIDATED_COMPLETION_COMMIT_CHARACTERS,
    ) && optional_u64(object, "insertTextFormat")
        && optional_u64(object, "insertTextMode")
        && object
            .get("editRange")
            .is_none_or(completion_default_edit_range_is_valid)
}

fn completion_default_edit_range_is_valid(value: &Value) -> bool {
    is_lsp_range(value)
        || (value.get("insert").is_some_and(is_lsp_range)
            && value.get("replace").is_some_and(is_lsp_range))
}

#[derive(Clone, Copy, Default)]
struct CompletionValidationDefaults {
    is_snippet: bool,
}

impl CompletionValidationDefaults {
    fn from_completion_result(value: &Value) -> Self {
        let Some(object) = value.as_object() else {
            return Self::default();
        };
        let is_snippet = object
            .get("itemDefaults")
            .and_then(Value::as_object)
            .and_then(|defaults| defaults.get("insertTextFormat"))
            .and_then(Value::as_u64)
            .is_some_and(|format| format == 2);
        Self { is_snippet }
    }
}

fn completion_item_shape_is_valid(value: &Value, defaults: CompletionValidationDefaults) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    if !required_bounded_non_empty_string(object, "label", MAX_VALIDATED_COMPLETION_LABEL_CHARS) {
        return false;
    }
    let is_snippet = object
        .get("insertTextFormat")
        .and_then(Value::as_u64)
        .is_some_and(|format| format == 2)
        || defaults.is_snippet;
    let insert_text_max_bytes = completion_insert_text_max_bytes(is_snippet);

    optional_bounded_string_chars(object, "detail", MAX_VALIDATED_COMPLETION_DETAIL_CHARS)
        && optional_bounded_string_chars(
            object,
            "sortText",
            MAX_VALIDATED_COMPLETION_SORT_TEXT_CHARS,
        )
        && optional_bounded_string_chars(
            object,
            "filterText",
            MAX_VALIDATED_COMPLETION_FILTER_TEXT_CHARS,
        )
        && optional_bounded_string_bytes(object, "insertText", insert_text_max_bytes)
        && optional_bounded_string_bytes(object, "textEditText", insert_text_max_bytes)
        && optional_u64(object, "kind")
        && optional_u64(object, "insertTextFormat")
        && optional_u64(object, "insertTextMode")
        && optional_bool(object, "deprecated")
        && optional_bool(object, "preselect")
        && optional_u64_array(object, "tags", MAX_VALIDATED_COMPLETION_TAGS)
        && optional_string_array(
            object,
            "commitCharacters",
            MAX_VALIDATED_COMPLETION_COMMIT_CHARACTERS,
        )
        && object
            .get("documentation")
            .is_none_or(completion_documentation_is_valid)
        && object
            .get("labelDetails")
            .is_none_or(completion_label_details_are_valid)
        && object
            .get("textEdit")
            .is_none_or(|edit| completion_text_edit_is_valid(edit, is_snippet))
        && object
            .get("additionalTextEdits")
            .is_none_or(text_edit_array_is_valid)
        && object
            .get("command")
            .is_none_or(completion_command_is_valid)
        && object.get("data").is_none_or(|_| {
            json_payload_is_bounded(value, MAX_VALIDATED_COMPLETION_RESOLVE_PAYLOAD_BYTES)
        })
}

fn completion_documentation_is_valid(value: &Value) -> bool {
    if let Some(text) = value.as_str() {
        return bounded_string_chars(text, MAX_VALIDATED_COMPLETION_DOCUMENTATION_CHARS);
    }

    let Some(object) = value.as_object() else {
        return false;
    };
    object
        .get("kind")
        .and_then(Value::as_str)
        .is_some_and(|kind| {
            bounded_string_chars(kind, MAX_VALIDATED_COMPLETION_DOCUMENTATION_KIND_CHARS)
        })
        && object
            .get("value")
            .and_then(Value::as_str)
            .is_some_and(|text| {
                bounded_string_chars(text, MAX_VALIDATED_COMPLETION_DOCUMENTATION_CHARS)
            })
}

fn completion_label_details_are_valid(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    optional_bounded_string_chars(
        object,
        "detail",
        MAX_VALIDATED_COMPLETION_LABEL_DETAIL_CHARS,
    ) && optional_bounded_string_chars(
        object,
        "description",
        MAX_VALIDATED_COMPLETION_LABEL_DETAIL_CHARS,
    )
}

fn completion_command_is_valid(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    required_bounded_non_empty_string(
        object,
        "title",
        MAX_VALIDATED_COMPLETION_COMMAND_TITLE_CHARS,
    ) && required_bounded_non_empty_string(
        object,
        "command",
        MAX_VALIDATED_COMPLETION_COMMAND_ID_CHARS,
    ) && object.get("arguments").is_none_or(|arguments| {
        arguments.is_array()
            && json_payload_is_bounded(arguments, MAX_COMPLETION_COMMAND_ARGUMENTS_PAYLOAD_BYTES)
    })
}

fn completion_text_edit_is_valid(value: &Value, is_snippet: bool) -> bool {
    value
        .get("newText")
        .and_then(Value::as_str)
        .is_some_and(|text| text.len() <= completion_insert_text_max_bytes(is_snippet))
        && (value.get("range").is_some_and(is_lsp_range)
            || (value.get("insert").is_some_and(is_lsp_range)
                && value.get("replace").is_some_and(is_lsp_range)))
}

fn text_edit_array_is_valid(value: &Value) -> bool {
    value.as_array().is_some_and(|edits| {
        edits.len() <= MAX_VALIDATED_COMPLETION_ADDITIONAL_TEXT_EDITS
            && edits.iter().all(text_edit_is_valid)
    })
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
    (line <= MAX_VALIDATED_COMPLETION_POSITION_COMPONENT
        && character <= MAX_VALIDATED_COMPLETION_POSITION_COMPONENT)
        .then_some((line, character))
}

fn completion_insert_text_max_bytes(is_snippet: bool) -> usize {
    if is_snippet {
        MAX_VALIDATED_SNIPPET_INSERT_TEXT_BYTES
    } else {
        MAX_VALIDATED_TEXT_EDIT_NEW_TEXT_BYTES
    }
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

fn optional_bounded_string_chars(object: &Map<String, Value>, key: &str, max_chars: usize) -> bool {
    object.get(key).is_none_or(|value| {
        value
            .as_str()
            .is_some_and(|text| bounded_string_chars(text, max_chars))
    })
}

fn optional_bounded_string_bytes(object: &Map<String, Value>, key: &str, max_bytes: usize) -> bool {
    object
        .get(key)
        .is_none_or(|value| value.as_str().is_some_and(|text| text.len() <= max_bytes))
}

fn bounded_string_chars(text: &str, max_chars: usize) -> bool {
    if text.len() > max_chars.saturating_mul(4) {
        return false;
    }
    text.chars().take(max_chars.saturating_add(1)).count() <= max_chars
}

fn optional_u64(object: &Map<String, Value>, key: &str) -> bool {
    object.get(key).is_none_or(Value::is_u64)
}

fn optional_bool(object: &Map<String, Value>, key: &str) -> bool {
    object.get(key).is_none_or(Value::is_boolean)
}

fn optional_string_array(object: &Map<String, Value>, key: &str, max_validated: usize) -> bool {
    object.get(key).is_none_or(|items| {
        items.as_array().is_some_and(|items| {
            items.iter().take(max_validated).all(|item| {
                item.as_str().is_some_and(|text| {
                    bounded_string_chars(text, MAX_VALIDATED_COMPLETION_COMMIT_CHARACTER_CHARS)
                })
            })
        })
    })
}

fn optional_u64_array(object: &Map<String, Value>, key: &str, max_validated: usize) -> bool {
    object.get(key).is_none_or(|items| {
        items
            .as_array()
            .is_some_and(|items| items.iter().take(max_validated).all(Value::is_u64))
    })
}

fn json_payload_is_bounded(value: &Value, max_bytes: usize) -> bool {
    let mut counter = CountingWriter::new(max_bytes);
    serde_json::to_writer(&mut counter, value).is_ok()
}

fn completion_event_column(character: usize) -> usize {
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
                "completion command arguments too large",
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
        INVALID_COMPLETION_ITEM_RESOLVE_RESPONSE, INVALID_COMPLETION_RESPONSE,
        MAX_COMPLETION_COMMAND_ARGUMENTS_PAYLOAD_BYTES,
        MAX_VALIDATED_COMPLETION_ADDITIONAL_TEXT_EDITS,
        MAX_VALIDATED_COMPLETION_COMMAND_TITLE_CHARS, MAX_VALIDATED_COMPLETION_COMMIT_CHARACTERS,
        MAX_VALIDATED_COMPLETION_DETAIL_CHARS, MAX_VALIDATED_COMPLETION_DOCUMENTATION_CHARS,
        MAX_VALIDATED_COMPLETION_ITEMS, MAX_VALIDATED_COMPLETION_LABEL_CHARS,
        MAX_VALIDATED_COMPLETION_POSITION_COMPONENT,
        MAX_VALIDATED_COMPLETION_RESOLVE_PAYLOAD_BYTES, MAX_VALIDATED_COMPLETION_TAGS,
        MAX_VALIDATED_SNIPPET_INSERT_TEXT_BYTES, MAX_VALIDATED_TEXT_EDIT_NEW_TEXT_BYTES,
        send_completion_item_resolve_result, send_completion_result,
    };
    use crate::{
        lsp_completion_resolve::CompletionResolveIntent, lsp_ui_events::LspUiEvent,
        ui_events::UiEvent,
    };
    use kuroya_core::LspCompletionItem;
    use serde_json::{Value, json};
    use std::{path::PathBuf, sync::Arc};

    #[test]
    fn completion_result_reports_invalid_success_payload() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_completion_result(
            7,
            PathBuf::from("src/main.rs"),
            3,
            2,
            4,
            &json!({
                "result": [{
                    "label": "HashMap",
                    "textEdit": {
                        "range": {
                            "start": { "line": 1 },
                            "end": { "line": 1, "character": 8 }
                        },
                        "newText": "HashMap"
                    }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("completion result event") {
            UiEvent::Lsp(LspUiEvent::CompletionResult { items, error, .. }) => {
                assert_eq!(items, None);
                assert_eq!(error.as_deref(), Some(INVALID_COMPLETION_RESPONSE));
            }
            other => panic!("expected completion result event, got {other:?}"),
        }
    }

    #[test]
    fn completion_result_reports_malformed_completion_list_item() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_completion_result(
            7,
            PathBuf::from("src/main.rs"),
            3,
            2,
            4,
            &json!({
                "result": {
                    "isIncomplete": false,
                    "items": [{ "detail": "missing label" }]
                }
            }),
            &tx,
        );

        match rx.recv().expect("completion result event") {
            UiEvent::Lsp(LspUiEvent::CompletionResult { items, error, .. }) => {
                assert_eq!(items, None);
                assert_eq!(error.as_deref(), Some(INVALID_COMPLETION_RESPONSE));
            }
            other => panic!("expected completion result event, got {other:?}"),
        }
    }

    #[test]
    fn completion_result_treats_null_success_as_empty() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_completion_result(
            7,
            PathBuf::from("src/main.rs"),
            3,
            2,
            4,
            &json!({ "result": null }),
            &tx,
        );

        match rx.recv().expect("completion result event") {
            UiEvent::Lsp(LspUiEvent::CompletionResult { items, error, .. }) => {
                assert_eq!(items, Some(Vec::new()));
                assert_eq!(error, None);
            }
            other => panic!("expected completion result event, got {other:?}"),
        }
    }

    #[test]
    fn completion_result_bounds_overflowing_event_column() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_completion_result(
            7,
            PathBuf::from("src/main.rs"),
            3,
            2,
            usize::MAX,
            &json!({ "result": null }),
            &tx,
        );

        match rx.recv().expect("completion result event") {
            UiEvent::Lsp(LspUiEvent::CompletionResult { column, error, .. }) => {
                assert_eq!(column, usize::MAX);
                assert_eq!(error, None);
            }
            other => panic!("expected completion result event, got {other:?}"),
        }
    }

    #[test]
    fn completion_result_preserves_valid_snippet_items() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_completion_result(
            7,
            PathBuf::from("src/main.rs"),
            3,
            2,
            4,
            &json!({
                "result": [{
                    "label": "println!",
                    "insertTextFormat": 2,
                    "insertText": "println!(\"${1:value}\");$0"
                }]
            }),
            &tx,
        );

        match rx.recv().expect("completion result event") {
            UiEvent::Lsp(LspUiEvent::CompletionResult {
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
                assert_eq!(version, 3);
                assert_eq!(line, 2);
                assert_eq!(column, 5);
                let items = items.expect("completion items");
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].label, "println!");
                assert_eq!(items[0].insert_text, "println!(\"value\");");
                assert!(items[0].is_snippet);
                assert_eq!(items[0].snippet_selection, Some(10..15));
                assert_eq!(items[0].snippet_tabstops, vec![10..15, 18..18]);
                assert_eq!(error, None);
            }
            other => panic!("expected completion result event, got {other:?}"),
        }
    }

    #[test]
    fn completion_result_preserves_raw_resolve_payload_and_bounds_commit_character_validation() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let mut commit_characters = (0..MAX_VALIDATED_COMPLETION_COMMIT_CHARACTERS)
            .map(|idx| json!(format!(".{idx}")))
            .collect::<Vec<_>>();
        commit_characters.push(json!(false));

        send_completion_result(
            7,
            PathBuf::from("src/main.rs"),
            3,
            2,
            4,
            &json!({
                "result": [{
                    "label": "HashMap",
                    "commitCharacters": commit_characters,
                    "data": {
                        "token": "raw-completion-item"
                    }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("completion result event") {
            UiEvent::Lsp(LspUiEvent::CompletionResult { items, error, .. }) => {
                assert_eq!(error, None);
                let items = items.expect("completion items");
                assert_eq!(items.len(), 1);
                assert_eq!(
                    items[0].commit_characters.len(),
                    MAX_VALIDATED_COMPLETION_COMMIT_CHARACTERS
                );
                assert_eq!(
                    items[0]
                        .resolve_payload
                        .as_ref()
                        .expect("raw resolve payload")["data"]["token"],
                    "raw-completion-item"
                );
            }
            other => panic!("expected completion result event, got {other:?}"),
        }
    }

    #[test]
    fn completion_result_bounds_tag_validation_to_prefix() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let mut tags = (0..MAX_VALIDATED_COMPLETION_TAGS)
            .map(|idx| json!(idx + 1))
            .collect::<Vec<_>>();
        tags.push(json!("malformed tail"));

        send_completion_result(
            7,
            PathBuf::from("src/main.rs"),
            3,
            2,
            4,
            &json!({
                "result": [{
                    "label": "HashMap",
                    "tags": tags
                }]
            }),
            &tx,
        );

        match rx.recv().expect("completion result event") {
            UiEvent::Lsp(LspUiEvent::CompletionResult { items, error, .. }) => {
                assert_eq!(error, None);
                let items = items.expect("completion items");
                assert_eq!(items.len(), 1);
                assert!(items[0].deprecated);
            }
            other => panic!("expected completion result event, got {other:?}"),
        }
    }

    #[test]
    fn completion_result_rejects_oversized_resolve_payload() {
        assert_invalid_completion_item(json!({
            "label": "HashMap",
            "data": {
                "blob": "x".repeat(MAX_VALIDATED_COMPLETION_RESOLVE_PAYLOAD_BYTES + 1)
            }
        }));
    }

    #[test]
    fn completion_result_validation_bounds_item_shape_scan() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let mut items = (0..MAX_VALIDATED_COMPLETION_ITEMS)
            .map(|idx| {
                json!({
                    "label": format!("Item{idx}")
                })
            })
            .collect::<Vec<_>>();
        items.push(json!({
            "detail": "overflow item without a label"
        }));

        send_completion_result(
            7,
            PathBuf::from("src/main.rs"),
            3,
            2,
            4,
            &json!({ "result": items }),
            &tx,
        );

        match rx.recv().expect("completion result event") {
            UiEvent::Lsp(LspUiEvent::CompletionResult { items, error, .. }) => {
                assert_eq!(error, None);
                let items = items.expect("completion items");
                assert_eq!(items.len(), MAX_VALIDATED_COMPLETION_ITEMS);
                assert_eq!(items[0].label, "Item0");
                assert_eq!(
                    items.last().expect("last bounded item").label,
                    format!("Item{}", MAX_VALIDATED_COMPLETION_ITEMS - 1)
                );
            }
            other => panic!("expected completion result event, got {other:?}"),
        }
    }

    #[test]
    fn completion_result_rejects_oversized_additional_text_edit_arrays() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let edits = (0..=MAX_VALIDATED_COMPLETION_ADDITIONAL_TEXT_EDITS)
            .map(|idx| {
                json!({
                    "range": {
                        "start": { "line": idx, "character": 0 },
                        "end": { "line": idx, "character": 0 }
                    },
                    "newText": ""
                })
            })
            .collect::<Vec<_>>();

        send_completion_result(
            7,
            PathBuf::from("src/main.rs"),
            3,
            2,
            4,
            &json!({
                "result": [{
                    "label": "HashMap",
                    "additionalTextEdits": edits
                }]
            }),
            &tx,
        );

        match rx.recv().expect("completion result event") {
            UiEvent::Lsp(LspUiEvent::CompletionResult { items, error, .. }) => {
                assert_eq!(items, None);
                assert_eq!(error.as_deref(), Some(INVALID_COMPLETION_RESPONSE));
            }
            other => panic!("expected completion result event, got {other:?}"),
        }
    }

    #[test]
    fn completion_result_rejects_reversed_text_edit_ranges() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_completion_result(
            7,
            PathBuf::from("src/main.rs"),
            3,
            2,
            4,
            &json!({
                "result": [{
                    "label": "HashMap",
                    "textEdit": {
                        "range": {
                            "start": { "line": 3, "character": 0 },
                            "end": { "line": 2, "character": 4 }
                        },
                        "newText": "HashMap"
                    }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("completion result event") {
            UiEvent::Lsp(LspUiEvent::CompletionResult { items, error, .. }) => {
                assert_eq!(items, None);
                assert_eq!(error.as_deref(), Some(INVALID_COMPLETION_RESPONSE));
            }
            other => panic!("expected completion result event, got {other:?}"),
        }
    }

    #[test]
    fn completion_result_rejects_oversized_text_edit_position_component() {
        assert_invalid_completion_item(json!({
            "label": "HashMap",
            "textEdit": {
                "range": {
                    "start": {
                        "line": MAX_VALIDATED_COMPLETION_POSITION_COMPONENT + 1,
                        "character": 0
                    },
                    "end": {
                        "line": MAX_VALIDATED_COMPLETION_POSITION_COMPONENT + 1,
                        "character": 0
                    }
                },
                "newText": "HashMap"
            }
        }));
    }

    #[test]
    fn completion_result_rejects_oversized_additional_text_edit_position_component() {
        assert_invalid_completion_item(json!({
            "label": "HashMap",
            "additionalTextEdits": [{
                "range": {
                    "start": {
                        "line": 0,
                        "character": MAX_VALIDATED_COMPLETION_POSITION_COMPONENT + 1
                    },
                    "end": {
                        "line": 0,
                        "character": MAX_VALIDATED_COMPLETION_POSITION_COMPONENT + 1
                    }
                },
                "newText": "use std::collections::HashMap;\n"
            }]
        }));
    }

    #[test]
    fn completion_result_rejects_oversized_insert_text() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_completion_result(
            7,
            PathBuf::from("src/main.rs"),
            3,
            2,
            4,
            &json!({
                "result": [{
                    "label": "Huge",
                    "insertText": "x".repeat(MAX_VALIDATED_TEXT_EDIT_NEW_TEXT_BYTES + 1)
                }]
            }),
            &tx,
        );

        match rx.recv().expect("completion result event") {
            UiEvent::Lsp(LspUiEvent::CompletionResult { items, error, .. }) => {
                assert_eq!(items, None);
                assert_eq!(error.as_deref(), Some(INVALID_COMPLETION_RESPONSE));
            }
            other => panic!("expected completion result event, got {other:?}"),
        }
    }

    #[test]
    fn completion_result_rejects_oversized_display_fields() {
        assert_invalid_completion_item(json!({
            "label": "x".repeat(MAX_VALIDATED_COMPLETION_LABEL_CHARS + 1)
        }));
        assert_invalid_completion_item(json!({
            "label": "HashMap",
            "detail": "x".repeat(MAX_VALIDATED_COMPLETION_DETAIL_CHARS + 1)
        }));
        assert_invalid_completion_item(json!({
            "label": "HashMap",
            "documentation": {
                "kind": "markdown",
                "value": "x".repeat(MAX_VALIDATED_COMPLETION_DOCUMENTATION_CHARS + 1)
            }
        }));
        assert_invalid_completion_item(json!({
            "label": "HashMap",
            "labelDetails": {
                "detail": "x".repeat(MAX_VALIDATED_COMPLETION_DETAIL_CHARS + 1)
            }
        }));
    }

    #[test]
    fn completion_result_rejects_oversized_snippet_insert_text() {
        assert_invalid_completion_item(json!({
            "label": "Huge snippet",
            "insertTextFormat": 2,
            "insertText": "x".repeat(MAX_VALIDATED_SNIPPET_INSERT_TEXT_BYTES + 1)
        }));

        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_completion_result(
            7,
            PathBuf::from("src/main.rs"),
            3,
            2,
            4,
            &json!({
                "result": {
                    "itemDefaults": {
                        "insertTextFormat": 2
                    },
                    "items": [{
                        "label": "Huge default snippet",
                        "insertText": "x".repeat(MAX_VALIDATED_SNIPPET_INSERT_TEXT_BYTES + 1)
                    }]
                }
            }),
            &tx,
        );

        match rx.recv().expect("completion result event") {
            UiEvent::Lsp(LspUiEvent::CompletionResult { items, error, .. }) => {
                assert_eq!(items, None);
                assert_eq!(error.as_deref(), Some(INVALID_COMPLETION_RESPONSE));
            }
            other => panic!("expected completion result event, got {other:?}"),
        }
    }

    #[test]
    fn completion_result_rejects_oversized_command_arguments() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_completion_result(
            7,
            PathBuf::from("src/main.rs"),
            3,
            2,
            4,
            &json!({
                "result": [{
                    "label": "HashMap",
                    "command": {
                        "title": "Fix",
                        "command": "rust.fix",
                        "arguments": ["x".repeat(MAX_COMPLETION_COMMAND_ARGUMENTS_PAYLOAD_BYTES)]
                    }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("completion result event") {
            UiEvent::Lsp(LspUiEvent::CompletionResult { items, error, .. }) => {
                assert_eq!(items, None);
                assert_eq!(error.as_deref(), Some(INVALID_COMPLETION_RESPONSE));
            }
            other => panic!("expected completion result event, got {other:?}"),
        }
    }

    #[test]
    fn completion_result_rejects_oversized_command_metadata() {
        assert_invalid_completion_item(json!({
            "label": "HashMap",
            "command": {
                "title": "x".repeat(MAX_VALIDATED_COMPLETION_COMMAND_TITLE_CHARS + 1),
                "command": "rust.fix"
            }
        }));
    }

    #[test]
    fn completion_item_resolve_result_reports_invalid_success_payload() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_completion_item_resolve_result(
            7,
            PathBuf::from("src/main.rs"),
            3,
            2,
            4,
            completion_item(),
            CompletionResolveIntent::Apply { commit_text: None },
            &json!({
                "result": {
                    "label": "HashMap",
                    "textEdit": {
                        "range": {
                            "start": { "line": 1, "character": 4 },
                            "end": { "line": "bad", "character": 8 }
                        },
                        "newText": "HashMap"
                    }
                }
            }),
            &tx,
        );

        match rx.recv().expect("completion item resolve result event") {
            UiEvent::Lsp(LspUiEvent::CompletionItemResolveResult { item, error, .. }) => {
                assert_eq!(item, None);
                assert_eq!(
                    error.as_deref(),
                    Some(INVALID_COMPLETION_ITEM_RESOLVE_RESPONSE)
                );
            }
            other => panic!("expected completion item resolve result event, got {other:?}"),
        }
    }

    #[test]
    fn completion_item_resolve_result_reports_null_success_as_invalid() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_completion_item_resolve_result(
            7,
            PathBuf::from("src/main.rs"),
            3,
            2,
            4,
            completion_item(),
            CompletionResolveIntent::Apply { commit_text: None },
            &json!({ "result": null }),
            &tx,
        );

        match rx.recv().expect("completion item resolve result event") {
            UiEvent::Lsp(LspUiEvent::CompletionItemResolveResult { item, error, .. }) => {
                assert_eq!(item, None);
                assert_eq!(
                    error.as_deref(),
                    Some(INVALID_COMPLETION_ITEM_RESOLVE_RESPONSE)
                );
            }
            other => panic!("expected completion item resolve result event, got {other:?}"),
        }
    }

    #[test]
    fn completion_item_resolve_result_rejects_oversized_resolve_payload() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_completion_item_resolve_result(
            7,
            PathBuf::from("src/main.rs"),
            3,
            2,
            4,
            completion_item(),
            CompletionResolveIntent::Apply { commit_text: None },
            &json!({
                "result": {
                    "label": "HashMap",
                    "data": {
                        "blob": "x".repeat(MAX_VALIDATED_COMPLETION_RESOLVE_PAYLOAD_BYTES + 1)
                    }
                }
            }),
            &tx,
        );

        match rx.recv().expect("completion item resolve result event") {
            UiEvent::Lsp(LspUiEvent::CompletionItemResolveResult { item, error, .. }) => {
                assert_eq!(item, None);
                assert_eq!(
                    error.as_deref(),
                    Some(INVALID_COMPLETION_ITEM_RESOLVE_RESPONSE)
                );
            }
            other => panic!("expected completion item resolve result event, got {other:?}"),
        }
    }

    #[test]
    fn completion_item_resolve_result_reports_origin_and_commit_text() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let path = PathBuf::from("src/main.rs");

        send_completion_item_resolve_result(
            7,
            path.clone(),
            3,
            2,
            4,
            completion_item(),
            CompletionResolveIntent::Apply {
                commit_text: Some(".".to_owned()),
            },
            &json!({
                "jsonrpc": "2.0",
                "id": 12,
                "result": {
                    "label": "HashMap",
                    "detail": "struct HashMap",
                    "data": { "id": 7 }
                }
            }),
            &tx,
        );

        match rx.recv().expect("event") {
            UiEvent::Lsp(LspUiEvent::CompletionItemResolveResult {
                id,
                path: event_path,
                version,
                line,
                column,
                item,
                fallback_item,
                intent,
                error,
            }) => {
                assert_eq!(id, 7);
                assert_eq!(event_path, path);
                assert_eq!(version, 3);
                assert_eq!(line, 2);
                assert_eq!(column, 5);
                assert_eq!(item.unwrap().detail.as_deref(), Some("struct HashMap"));
                assert_eq!(fallback_item.label, "HashMap");
                assert_eq!(
                    intent,
                    CompletionResolveIntent::Apply {
                        commit_text: Some(".".to_owned())
                    }
                );
                assert!(error.is_none());
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn completion_item_resolve_result_preserves_raw_fallback_payload() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let path = PathBuf::from("src/main.rs");
        let mut original_item = completion_item();
        original_item.detail = Some("raw fallback detail".to_owned());
        original_item.resolve_payload = Some(Arc::new(json!({
            "label": "HashMap",
            "data": {
                "token": "raw-fallback-item"
            }
        })));

        send_completion_item_resolve_result(
            7,
            path,
            3,
            2,
            4,
            original_item,
            CompletionResolveIntent::Preview { selected: 1 },
            &json!({
                "jsonrpc": "2.0",
                "id": 12,
                "result": {
                    "label": "HashMap",
                    "detail": "resolved detail",
                    "data": {
                        "token": "resolved-item"
                    }
                }
            }),
            &tx,
        );

        match rx.recv().expect("completion item resolve result event") {
            UiEvent::Lsp(LspUiEvent::CompletionItemResolveResult {
                item,
                fallback_item,
                error,
                ..
            }) => {
                assert_eq!(error, None);
                assert_eq!(item.expect("resolved item").resolve_payload, None);
                assert_eq!(fallback_item.detail.as_deref(), Some("raw fallback detail"));
                assert_eq!(
                    fallback_item
                        .resolve_payload
                        .as_ref()
                        .expect("raw fallback payload")["data"]["token"],
                    "raw-fallback-item"
                );
            }
            other => panic!("expected completion item resolve result event, got {other:?}"),
        }
    }

    fn completion_item() -> LspCompletionItem {
        LspCompletionItem {
            label: "HashMap".to_owned(),
            detail: None,
            documentation: None,
            kind: None,
            deprecated: false,
            is_snippet: false,
            sort_text: None,
            filter_text: None,
            preselect: false,
            commit_characters: Vec::new(),
            insert_text: "HashMap".to_owned(),
            snippet_selection: None,
            snippet_tabstops: Vec::new(),
            snippet_tabstop_groups: Vec::new(),
            text_edit: None,
            insert_text_edit: None,
            additional_text_edits: Vec::new(),
            resolve_payload: None,
        }
    }

    fn assert_invalid_completion_item(item: Value) {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_completion_result(
            7,
            PathBuf::from("src/main.rs"),
            3,
            2,
            4,
            &json!({ "result": [item] }),
            &tx,
        );

        match rx.recv().expect("completion result event") {
            UiEvent::Lsp(LspUiEvent::CompletionResult { items, error, .. }) => {
                assert_eq!(items, None);
                assert_eq!(error.as_deref(), Some(INVALID_COMPLETION_RESPONSE));
            }
            other => panic!("expected completion result event, got {other:?}"),
        }
    }
}
