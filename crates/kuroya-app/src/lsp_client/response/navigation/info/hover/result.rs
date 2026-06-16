use super::super::super::super::response_error;
use crate::ui_event_channel::Sender;
use crate::{lsp_ui_events::LspUiEvent, ui_events::UiEvent};
use kuroya_core::{BufferId, parse_hover_response};
use serde_json::Value;
use std::path::PathBuf;

const INVALID_HOVER_RESPONSE: &str = "invalid textDocument/hover response";
const MAX_VALIDATED_HOVER_CONTENT_CHARS: usize = 64 * 1024;
const MAX_VALIDATED_HOVER_PARTS: usize = 64;
const MAX_VALIDATED_HOVER_LANGUAGE_CHARS: usize = 64;

pub(super) fn send_hover_result(
    id: BufferId,
    path: PathBuf,
    version: u64,
    line: usize,
    character: usize,
    value: &Value,
    ui_tx: &Sender<UiEvent>,
) {
    let contents = if let Some(error) = response_error(value) {
        Some(error)
    } else {
        match parse_hover_success_response(value) {
            Ok(contents) => contents,
            Err(()) => Some(INVALID_HOVER_RESPONSE.to_owned()),
        }
    };
    let _ = crate::lsp_client::response::emit_lsp_response_event(
        ui_tx,
        LspUiEvent::HoverResult {
            id,
            path,
            version,
            line,
            column: character.saturating_add(1),
            contents,
        },
    );
}

fn parse_hover_success_response(value: &Value) -> Result<Option<String>, ()> {
    let result = value.get("result").ok_or(())?;
    if result.is_null() {
        return Ok(None);
    }

    let result = result.as_object().ok_or(())?;
    if !hover_range_is_valid(result.get("range")) {
        return Err(());
    }

    let contents = result.get("contents").ok_or(())?;
    if !hover_contents_shape_is_valid(contents) {
        return Err(());
    }

    if let Some(text) = contents.as_str() {
        let text = text.trim();
        return Ok((!text.is_empty()).then(|| text.to_owned()));
    }

    Ok(parse_hover_response(value).map(|hover| hover.text))
}

fn hover_contents_shape_is_valid(value: &Value) -> bool {
    if let Some(text) = value.as_str() {
        return bounded_string_chars(text, MAX_VALIDATED_HOVER_CONTENT_CHARS);
    }

    if let Some(items) = value.as_array() {
        return items.len() <= MAX_VALIDATED_HOVER_PARTS
            && items.iter().all(marked_string_is_valid);
    }

    markup_content_is_valid(value) || marked_string_object_is_valid(value)
}

fn marked_string_is_valid(value: &Value) -> bool {
    value
        .as_str()
        .is_some_and(|text| bounded_string_chars(text, MAX_VALIDATED_HOVER_CONTENT_CHARS))
        || marked_string_object_is_valid(value)
}

fn marked_string_object_is_valid(value: &Value) -> bool {
    value
        .get("language")
        .and_then(Value::as_str)
        .is_some_and(|language| bounded_string_chars(language, MAX_VALIDATED_HOVER_LANGUAGE_CHARS))
        && value
            .get("value")
            .and_then(Value::as_str)
            .is_some_and(|text| bounded_string_chars(text, MAX_VALIDATED_HOVER_CONTENT_CHARS))
}

fn markup_content_is_valid(value: &Value) -> bool {
    let Some(kind) = value.get("kind").and_then(Value::as_str) else {
        return false;
    };
    if !matches!(kind, "markdown" | "plaintext") {
        return false;
    }
    value
        .get("value")
        .and_then(Value::as_str)
        .is_some_and(|text| bounded_string_chars(text, MAX_VALIDATED_HOVER_CONTENT_CHARS))
}

fn hover_range_is_valid(value: Option<&Value>) -> bool {
    let Some(value) = value else {
        return true;
    };
    let Some(start) = value.get("start") else {
        return false;
    };
    let Some(end) = value.get("end") else {
        return false;
    };
    let Some((start_line, start_character)) = hover_position(start) else {
        return false;
    };
    let Some((end_line, end_character)) = hover_position(end) else {
        return false;
    };

    start_line < end_line || (start_line == end_line && start_character <= end_character)
}

fn hover_position(value: &Value) -> Option<(u64, u64)> {
    Some((
        value.get("line")?.as_u64()?,
        value.get("character")?.as_u64()?,
    ))
}

fn bounded_string_chars(text: &str, max_chars: usize) -> bool {
    text.chars().take(max_chars.saturating_add(1)).count() <= max_chars
}

#[cfg(test)]
mod tests {
    use super::{
        INVALID_HOVER_RESPONSE, MAX_VALIDATED_HOVER_CONTENT_CHARS,
        MAX_VALIDATED_HOVER_LANGUAGE_CHARS, MAX_VALIDATED_HOVER_PARTS, send_hover_result,
    };
    use crate::{lsp_ui_events::LspUiEvent, ui_events::UiEvent};
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn hover_result_reports_invalid_success_payload() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_hover_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({ "result": { "contents": 42 } }),
            &tx,
        );

        match rx.recv().expect("hover result event") {
            UiEvent::Lsp(LspUiEvent::HoverResult {
                id,
                path,
                version,
                line,
                column,
                contents,
            }) => {
                assert_eq!(id, 7);
                assert_eq!(path, PathBuf::from("src/main.rs"));
                assert_eq!(version, 11);
                assert_eq!(line, 3);
                assert_eq!(column, 5);
                assert_eq!(contents.as_deref(), Some(INVALID_HOVER_RESPONSE));
            }
            other => panic!("expected hover result event, got {other:?}"),
        }
    }

    #[test]
    fn hover_result_reports_invalid_marked_string_array_item() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_hover_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({
                "result": {
                    "contents": [
                        "String",
                        { "language": "rust" }
                    ]
                }
            }),
            &tx,
        );

        match rx.recv().expect("hover result event") {
            UiEvent::Lsp(LspUiEvent::HoverResult { contents, .. }) => {
                assert_eq!(contents.as_deref(), Some(INVALID_HOVER_RESPONSE));
            }
            other => panic!("expected hover result event, got {other:?}"),
        }
    }

    #[test]
    fn hover_result_reports_invalid_optional_range() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_hover_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({
                "result": {
                    "contents": "String",
                    "range": {
                        "start": { "line": 3, "character": 1 },
                        "end": { "line": 2, "character": 1 }
                    }
                }
            }),
            &tx,
        );

        match rx.recv().expect("hover result event") {
            UiEvent::Lsp(LspUiEvent::HoverResult { contents, .. }) => {
                assert_eq!(contents.as_deref(), Some(INVALID_HOVER_RESPONSE));
            }
            other => panic!("expected hover result event, got {other:?}"),
        }
    }

    #[test]
    fn hover_result_reports_oversized_plain_string_contents() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_hover_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({
                "result": {
                    "contents": "x".repeat(MAX_VALIDATED_HOVER_CONTENT_CHARS + 1)
                }
            }),
            &tx,
        );

        match rx.recv().expect("hover result event") {
            UiEvent::Lsp(LspUiEvent::HoverResult { contents, .. }) => {
                assert_eq!(contents.as_deref(), Some(INVALID_HOVER_RESPONSE));
            }
            other => panic!("expected hover result event, got {other:?}"),
        }
    }

    #[test]
    fn hover_result_reports_oversized_structured_contents() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_hover_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({
                "result": {
                    "contents": {
                        "kind": "markdown",
                        "value": "x".repeat(MAX_VALIDATED_HOVER_CONTENT_CHARS + 1)
                    }
                }
            }),
            &tx,
        );

        match rx.recv().expect("hover result event") {
            UiEvent::Lsp(LspUiEvent::HoverResult { contents, .. }) => {
                assert_eq!(contents.as_deref(), Some(INVALID_HOVER_RESPONSE));
            }
            other => panic!("expected hover result event, got {other:?}"),
        }
    }

    #[test]
    fn hover_result_reports_oversized_marked_string_arrays() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let contents = (0..=MAX_VALIDATED_HOVER_PARTS)
            .map(|_| json!("String"))
            .collect::<Vec<_>>();
        send_hover_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({ "result": { "contents": contents } }),
            &tx,
        );

        match rx.recv().expect("hover result event") {
            UiEvent::Lsp(LspUiEvent::HoverResult { contents, .. }) => {
                assert_eq!(contents.as_deref(), Some(INVALID_HOVER_RESPONSE));
            }
            other => panic!("expected hover result event, got {other:?}"),
        }

        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_hover_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({
                "result": {
                    "contents": [{
                        "language": "x".repeat(MAX_VALIDATED_HOVER_LANGUAGE_CHARS + 1),
                        "value": "String"
                    }]
                }
            }),
            &tx,
        );

        match rx.recv().expect("hover result event") {
            UiEvent::Lsp(LspUiEvent::HoverResult { contents, .. }) => {
                assert_eq!(contents.as_deref(), Some(INVALID_HOVER_RESPONSE));
            }
            other => panic!("expected hover result event, got {other:?}"),
        }
    }

    #[test]
    fn hover_result_treats_null_success_as_empty() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_hover_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({ "result": null }),
            &tx,
        );

        match rx.recv().expect("hover result event") {
            UiEvent::Lsp(LspUiEvent::HoverResult { contents, .. }) => {
                assert_eq!(contents, None);
            }
            other => panic!("expected hover result event, got {other:?}"),
        }
    }

    #[test]
    fn hover_result_bounds_unrepresentable_origin_column() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_hover_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            usize::MAX,
            &json!({ "result": null }),
            &tx,
        );

        match rx.recv().expect("hover result event") {
            UiEvent::Lsp(LspUiEvent::HoverResult {
                column, contents, ..
            }) => {
                assert_eq!(column, usize::MAX);
                assert_eq!(contents, None);
            }
            other => panic!("expected hover result event, got {other:?}"),
        }
    }

    #[test]
    fn hover_result_sends_parsed_markup_and_marked_strings() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_hover_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({
                "result": {
                    "contents": {
                        "kind": "markdown",
                        "value": "```rust\nfn main()\n```"
                    }
                }
            }),
            &tx,
        );

        match rx.recv().expect("hover result event") {
            UiEvent::Lsp(LspUiEvent::HoverResult { contents, .. }) => {
                assert_eq!(contents.as_deref(), Some("```rust\nfn main()\n```"));
            }
            other => panic!("expected hover result event, got {other:?}"),
        }

        send_hover_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({
                "result": {
                    "contents": [
                        "String",
                        { "language": "rust", "value": "struct String" }
                    ]
                }
            }),
            &tx,
        );

        match rx.recv().expect("hover result event") {
            UiEvent::Lsp(LspUiEvent::HoverResult { contents, .. }) => {
                assert_eq!(
                    contents.as_deref(),
                    Some("String\n\n```rust\nstruct String\n```")
                );
            }
            other => panic!("expected hover result event, got {other:?}"),
        }
    }
}
