use super::super::super::response_error;
use crate::ui_event_channel::Sender;
use crate::{lsp_ui_events::LspUiEvent, ui_events::UiEvent};
use kuroya_core::{BufferId, LspInlayHint, parse_inlay_hints_response};
use serde_json::Value;
use std::path::PathBuf;

const INVALID_INLAY_HINTS_RESPONSE: &str = "invalid textDocument/inlayHint response";
const MAX_VALIDATED_INLAY_HINTS: usize = 500;
const MAX_VALIDATED_INLAY_HINT_POSITION_COMPONENT: u64 = i32::MAX as u64;
const MAX_VALIDATED_INLAY_HINT_LABEL_CHARS: usize = 10_000;
const MAX_VALIDATED_INLAY_HINT_LABEL_PARTS: usize = 256;
const MAX_INLAY_HINT_RESULT_EVENT_LABEL_CHARS: usize = 100_000;

pub(super) fn send_inlay_hints_result(
    id: BufferId,
    path: PathBuf,
    version: u64,
    value: &Value,
    ui_tx: &Sender<UiEvent>,
) {
    let mut error = response_error(value);
    let hints = if error.is_none() {
        match parse_inlay_hints_success_response(value) {
            Some(hints) => Some(hints),
            None => {
                error = Some(INVALID_INLAY_HINTS_RESPONSE.to_owned());
                None
            }
        }
    } else {
        None
    };
    let _ = crate::lsp_client::response::emit_lsp_response_event(
        ui_tx,
        LspUiEvent::InlayHintsResult {
            id,
            path,
            version,
            hints,
            error,
        },
    );
}

fn parse_inlay_hints_success_response(value: &Value) -> Option<Vec<LspInlayHint>> {
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
        .take(MAX_VALIDATED_INLAY_HINTS)
        .any(|item| !is_valid_inlay_hint_item(item))
    {
        return None;
    }

    let mut hints = parse_inlay_hints_response(value)?;
    truncate_inlay_hints_for_result_event(&mut hints);
    Some(hints)
}

fn truncate_inlay_hints_for_result_event(hints: &mut Vec<LspInlayHint>) {
    hints.truncate(MAX_VALIDATED_INLAY_HINTS);

    let mut label_chars = 0usize;
    for (index, hint) in hints.iter().enumerate() {
        let next_label_chars = label_chars.saturating_add(hint.label.chars().count());
        if next_label_chars > MAX_INLAY_HINT_RESULT_EVENT_LABEL_CHARS {
            hints.truncate(index);
            return;
        }
        label_chars = next_label_chars;
    }
}

fn is_valid_inlay_hint_item(value: &Value) -> bool {
    let Some(position) = value.get("position") else {
        return false;
    };
    if !position
        .get("line")
        .is_some_and(lsp_zero_based_coordinate_fits_one_based_usize)
        || !position
            .get("character")
            .is_some_and(lsp_zero_based_coordinate_fits_one_based_usize)
    {
        return false;
    }

    is_valid_inlay_hint_label(value.get("label")) && value.get("kind").is_none_or(lsp_u64_fits_u8)
}

fn is_valid_inlay_hint_label(value: Option<&Value>) -> bool {
    match value {
        Some(Value::String(label)) => bounded_inlay_hint_label_text_has_visible_value(label),
        Some(Value::Array(parts)) => {
            if parts.is_empty() {
                return false;
            }
            if parts.len() > MAX_VALIDATED_INLAY_HINT_LABEL_PARTS {
                return false;
            }

            let mut has_visible_value = false;
            let mut inspected_chars = 0usize;
            for part in parts {
                let Some(value) = part.get("value").and_then(Value::as_str) else {
                    return false;
                };
                if !accumulate_inlay_hint_label_text(
                    value,
                    &mut inspected_chars,
                    &mut has_visible_value,
                ) {
                    return false;
                }
            }
            has_visible_value
        }
        _ => false,
    }
}

fn bounded_inlay_hint_label_text_has_visible_value(label: &str) -> bool {
    let mut has_visible_value = false;
    let mut inspected_chars = 0usize;
    accumulate_inlay_hint_label_text(label, &mut inspected_chars, &mut has_visible_value)
        && has_visible_value
}

fn accumulate_inlay_hint_label_text(
    label: &str,
    inspected_chars: &mut usize,
    has_visible_value: &mut bool,
) -> bool {
    for ch in label.chars() {
        if *inspected_chars >= MAX_VALIDATED_INLAY_HINT_LABEL_CHARS {
            return false;
        }
        *inspected_chars += 1;
        *has_visible_value |= !ch.is_whitespace();
    }
    true
}

fn lsp_u64_fits_u8(value: &Value) -> bool {
    value
        .as_u64()
        .is_some_and(|value| u8::try_from(value).is_ok())
}

fn lsp_zero_based_coordinate_fits_one_based_usize(value: &Value) -> bool {
    value
        .as_u64()
        .is_some_and(|value| value <= MAX_VALIDATED_INLAY_HINT_POSITION_COMPONENT)
}

#[cfg(test)]
mod tests {
    use super::{
        INVALID_INLAY_HINTS_RESPONSE, MAX_INLAY_HINT_RESULT_EVENT_LABEL_CHARS,
        MAX_VALIDATED_INLAY_HINT_LABEL_CHARS, MAX_VALIDATED_INLAY_HINT_LABEL_PARTS,
        MAX_VALIDATED_INLAY_HINT_POSITION_COMPONENT, MAX_VALIDATED_INLAY_HINTS,
        send_inlay_hints_result,
    };
    use crate::{lsp_ui_events::LspUiEvent, ui_events::UiEvent};
    use serde_json::{Value, json};
    use std::path::PathBuf;

    #[test]
    fn inlay_hints_result_reports_invalid_success_payload() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_inlay_hints_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({ "result": [{ "position": { "line": 1 }, "label": ": usize" }] }),
            &tx,
        );

        match rx.recv().expect("inlay hints result event") {
            UiEvent::Lsp(LspUiEvent::InlayHintsResult {
                id,
                path,
                version,
                hints,
                error,
            }) => {
                assert_eq!(id, 7);
                assert_eq!(path, PathBuf::from("src/main.rs"));
                assert_eq!(version, 11);
                assert_eq!(hints, None);
                assert_eq!(error.as_deref(), Some(INVALID_INLAY_HINTS_RESPONSE));
            }
            other => panic!("expected inlay hints result event, got {other:?}"),
        }
    }

    #[test]
    fn inlay_hints_result_reports_malformed_label_parts() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_inlay_hints_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({
                "result": [{
                    "position": { "line": 1, "character": 2 },
                    "label": [{ "value": ": " }, { "tooltip": "missing value" }]
                }]
            }),
            &tx,
        );

        match rx.recv().expect("inlay hints result event") {
            UiEvent::Lsp(LspUiEvent::InlayHintsResult { hints, error, .. }) => {
                assert_eq!(hints, None);
                assert_eq!(error.as_deref(), Some(INVALID_INLAY_HINTS_RESPONSE));
            }
            other => panic!("expected inlay hints result event, got {other:?}"),
        }
    }

    #[test]
    fn inlay_hints_result_reports_kind_that_cannot_fit_wire_type() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_inlay_hints_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({
                "result": [{
                    "position": { "line": 1, "character": 2 },
                    "label": ": usize",
                    "kind": 256
                }]
            }),
            &tx,
        );

        match rx.recv().expect("inlay hints result event") {
            UiEvent::Lsp(LspUiEvent::InlayHintsResult { hints, error, .. }) => {
                assert_eq!(hints, None);
                assert_eq!(error.as_deref(), Some(INVALID_INLAY_HINTS_RESPONSE));
            }
            other => panic!("expected inlay hints result event, got {other:?}"),
        }
    }

    #[test]
    fn inlay_hints_result_reports_coordinate_outside_validated_position_domain() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let too_large = MAX_VALIDATED_INLAY_HINT_POSITION_COMPONENT + 1;
        send_inlay_hints_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({
                "result": [{
                    "position": { "line": too_large, "character": 0 },
                    "label": ": usize"
                }]
            }),
            &tx,
        );

        match rx.recv().expect("inlay hints result event") {
            UiEvent::Lsp(LspUiEvent::InlayHintsResult { hints, error, .. }) => {
                assert_eq!(hints, None);
                assert_eq!(error.as_deref(), Some(INVALID_INLAY_HINTS_RESPONSE));
            }
            other => panic!("expected inlay hints result event, got {other:?}"),
        }
    }

    #[test]
    fn inlay_hints_result_reports_pathological_label_payloads() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_inlay_hints_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({
                "result": [{
                    "position": { "line": 1, "character": 2 },
                    "label": "x".repeat(MAX_VALIDATED_INLAY_HINT_LABEL_CHARS + 1)
                }]
            }),
            &tx,
        );

        match rx.recv().expect("inlay hints result event") {
            UiEvent::Lsp(LspUiEvent::InlayHintsResult { hints, error, .. }) => {
                assert_eq!(hints, None);
                assert_eq!(error.as_deref(), Some(INVALID_INLAY_HINTS_RESPONSE));
            }
            other => panic!("expected inlay hints result event, got {other:?}"),
        }

        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let parts = (0..=MAX_VALIDATED_INLAY_HINT_LABEL_PARTS)
            .map(|_| json!({ "value": "x" }))
            .collect::<Vec<Value>>();
        send_inlay_hints_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({
                "result": [{
                    "position": { "line": 1, "character": 2 },
                    "label": parts
                }]
            }),
            &tx,
        );

        match rx.recv().expect("inlay hints result event") {
            UiEvent::Lsp(LspUiEvent::InlayHintsResult { hints, error, .. }) => {
                assert_eq!(hints, None);
                assert_eq!(error.as_deref(), Some(INVALID_INLAY_HINTS_RESPONSE));
            }
            other => panic!("expected inlay hints result event, got {other:?}"),
        }
    }

    #[test]
    fn inlay_hints_result_only_validates_the_bounded_result_prefix() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let too_large = MAX_VALIDATED_INLAY_HINT_POSITION_COMPONENT + 1;
        let mut items = (0..MAX_VALIDATED_INLAY_HINTS)
            .map(|column| {
                json!({
                    "position": { "line": 0, "character": column },
                    "label": format!("hint {column}")
                })
            })
            .collect::<Vec<Value>>();
        items.push(json!({
            "position": { "line": too_large, "character": 0 },
            "label": "tail"
        }));

        send_inlay_hints_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({ "result": items }),
            &tx,
        );

        match rx.recv().expect("inlay hints result event") {
            UiEvent::Lsp(LspUiEvent::InlayHintsResult { hints, error, .. }) => {
                assert_eq!(error, None);
                assert_eq!(
                    hints.expect("parsed inlay hints").len(),
                    MAX_VALIDATED_INLAY_HINTS
                );
            }
            other => panic!("expected inlay hints result event, got {other:?}"),
        }
    }

    #[test]
    fn inlay_hints_result_caps_large_labels_before_ui_event() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let max_large_labels =
            MAX_INLAY_HINT_RESULT_EVENT_LABEL_CHARS / MAX_VALIDATED_INLAY_HINT_LABEL_CHARS;
        let items = (0..=max_large_labels)
            .map(|line| {
                json!({
                    "position": { "line": line, "character": 0 },
                    "label": "x".repeat(MAX_VALIDATED_INLAY_HINT_LABEL_CHARS)
                })
            })
            .collect::<Vec<Value>>();

        send_inlay_hints_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({ "result": items }),
            &tx,
        );

        match rx.recv().expect("inlay hints result event") {
            UiEvent::Lsp(LspUiEvent::InlayHintsResult { hints, error, .. }) => {
                let hints = hints.expect("bounded inlay hints");
                assert_eq!(hints.len(), max_large_labels);
                assert_eq!(error, None);
            }
            other => panic!("expected inlay hints result event, got {other:?}"),
        }
    }

    #[test]
    fn inlay_hints_result_treats_null_success_as_empty() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_inlay_hints_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({ "result": null }),
            &tx,
        );

        match rx.recv().expect("inlay hints result event") {
            UiEvent::Lsp(LspUiEvent::InlayHintsResult { hints, error, .. }) => {
                assert_eq!(hints, Some(Vec::new()));
                assert_eq!(error, None);
            }
            other => panic!("expected inlay hints result event, got {other:?}"),
        }
    }

    #[test]
    fn inlay_hints_result_sends_parsed_hints_event() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_inlay_hints_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({
                "result": [
                    {
                        "position": { "line": 2, "character": 16 },
                        "label": ": usize",
                        "kind": 1
                    },
                    {
                        "position": { "line": 1, "character": 8 },
                        "label": [
                            { "value": "name" },
                            { "value": ": " },
                            { "value": "&str" }
                        ],
                        "kind": 2
                    }
                ]
            }),
            &tx,
        );

        match rx.recv().expect("inlay hints result event") {
            UiEvent::Lsp(LspUiEvent::InlayHintsResult {
                id,
                path,
                version,
                hints,
                error,
            }) => {
                assert_eq!(id, 7);
                assert_eq!(path, PathBuf::from("src/main.rs"));
                assert_eq!(version, 11);
                let hints = hints.expect("parsed inlay hints");
                assert_eq!(hints.len(), 2);
                assert_eq!(hints[0].line, 2);
                assert_eq!(hints[0].column, 9);
                assert_eq!(hints[0].label, "name: &str");
                assert_eq!(hints[0].kind, Some(2));
                assert_eq!(hints[1].line, 3);
                assert_eq!(hints[1].column, 17);
                assert_eq!(hints[1].label, ": usize");
                assert_eq!(hints[1].kind, Some(1));
                assert_eq!(error, None);
            }
            other => panic!("expected inlay hints result event, got {other:?}"),
        }
    }
}
