use super::super::super::response_error;
use crate::ui_event_channel::Sender;
use crate::{lsp_ui_events::LspUiEvent, ui_events::UiEvent};
use kuroya_core::{BufferId, LspFoldingRange, parse_folding_ranges_response};
use serde_json::Value;
use std::path::PathBuf;

const INVALID_FOLDING_RANGES_RESPONSE: &str = "invalid textDocument/foldingRange response";
const MAX_VALIDATED_FOLDING_RANGES: usize = 1_000;
const MAX_VALIDATED_FOLDING_RANGE_POSITION_COMPONENT: u64 = i32::MAX as u64;

pub(super) fn send_folding_ranges_result(
    id: BufferId,
    path: PathBuf,
    version: u64,
    value: &Value,
    ui_tx: &Sender<UiEvent>,
) {
    let mut error = response_error(value);
    let ranges = if error.is_none() {
        match parse_folding_ranges_success_response(value) {
            Some(ranges) => Some(ranges),
            None => {
                error = Some(INVALID_FOLDING_RANGES_RESPONSE.to_owned());
                None
            }
        }
    } else {
        None
    };
    let _ = crate::lsp_client::response::emit_critical_lsp_response_event(
        ui_tx,
        LspUiEvent::FoldingRangesResult {
            id,
            path,
            version,
            ranges,
            error,
        },
    );
}

fn parse_folding_ranges_success_response(value: &Value) -> Option<Vec<LspFoldingRange>> {
    let result = value.get("result")?;
    if result.is_null() {
        return Some(Vec::new());
    }

    let items = result.as_array()?;
    if items
        .iter()
        .take(MAX_VALIDATED_FOLDING_RANGES)
        .any(|item| !is_valid_folding_range_item(item))
    {
        return None;
    }

    parse_folding_ranges_response(value)
}

fn is_valid_folding_range_item(value: &Value) -> bool {
    let Some(start_line) = required_lsp_coordinate(value, "startLine") else {
        return false;
    };
    let Some(end_line) = required_lsp_coordinate(value, "endLine") else {
        return false;
    };
    if end_line <= start_line {
        return false;
    }

    is_optional_lsp_coordinate(value, "startCharacter")
        && is_optional_lsp_coordinate(value, "endCharacter")
        && is_optional_string(value, "kind")
}

fn required_lsp_coordinate(value: &Value, key: &str) -> Option<u64> {
    let coordinate = value.get(key)?.as_u64()?;
    lsp_coordinate_fits_validated_position(coordinate).then_some(coordinate)
}

fn is_optional_lsp_coordinate(value: &Value, key: &str) -> bool {
    value.get(key).is_none_or(|value| {
        value
            .as_u64()
            .is_some_and(lsp_coordinate_fits_validated_position)
    })
}

fn lsp_coordinate_fits_validated_position(value: u64) -> bool {
    value <= MAX_VALIDATED_FOLDING_RANGE_POSITION_COMPONENT
}

fn is_optional_string(value: &Value, key: &str) -> bool {
    value.get(key).is_none_or(Value::is_string)
}

#[cfg(test)]
mod tests {
    use super::{
        INVALID_FOLDING_RANGES_RESPONSE, MAX_VALIDATED_FOLDING_RANGE_POSITION_COMPONENT,
        MAX_VALIDATED_FOLDING_RANGES, send_folding_ranges_result,
    };
    use crate::{lsp_ui_events::LspUiEvent, ui_events::UiEvent};
    use serde_json::{Value, json};
    use std::path::PathBuf;

    #[test]
    fn folding_ranges_result_reports_invalid_success_payload() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_folding_ranges_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({ "result": [{ "startLine": 1 }] }),
            &tx,
        );

        match rx.recv().expect("folding ranges result event") {
            UiEvent::Lsp(LspUiEvent::FoldingRangesResult {
                id,
                path,
                version,
                ranges,
                error,
            }) => {
                assert_eq!(id, 7);
                assert_eq!(path, PathBuf::from("src/main.rs"));
                assert_eq!(version, 11);
                assert_eq!(ranges, None);
                assert_eq!(error.as_deref(), Some(INVALID_FOLDING_RANGES_RESPONSE));
            }
            other => panic!("expected folding ranges result event, got {other:?}"),
        }
    }

    #[test]
    fn folding_ranges_result_reports_inverted_success_payload() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_folding_ranges_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({ "result": [{ "startLine": 4, "endLine": 2 }] }),
            &tx,
        );

        match rx.recv().expect("folding ranges result event") {
            UiEvent::Lsp(LspUiEvent::FoldingRangesResult { ranges, error, .. }) => {
                assert_eq!(ranges, None);
                assert_eq!(error.as_deref(), Some(INVALID_FOLDING_RANGES_RESPONSE));
            }
            other => panic!("expected folding ranges result event, got {other:?}"),
        }
    }

    #[test]
    fn folding_ranges_result_reports_oversized_success_payload() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let overflowing_component = MAX_VALIDATED_FOLDING_RANGE_POSITION_COMPONENT + 1;
        send_folding_ranges_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({
                "result": [{
                    "startLine": 1,
                    "endLine": overflowing_component,
                    "startCharacter": 0
                }]
            }),
            &tx,
        );

        match rx.recv().expect("folding ranges result event") {
            UiEvent::Lsp(LspUiEvent::FoldingRangesResult { ranges, error, .. }) => {
                assert_eq!(ranges, None);
                assert_eq!(error.as_deref(), Some(INVALID_FOLDING_RANGES_RESPONSE));
            }
            other => panic!("expected folding ranges result event, got {other:?}"),
        }
    }

    #[test]
    fn folding_ranges_result_reports_oversized_optional_character() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let overflowing_component = MAX_VALIDATED_FOLDING_RANGE_POSITION_COMPONENT + 1;
        send_folding_ranges_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({
                "result": [{
                    "startLine": 1,
                    "endLine": 2,
                    "startCharacter": overflowing_component
                }]
            }),
            &tx,
        );

        match rx.recv().expect("folding ranges result event") {
            UiEvent::Lsp(LspUiEvent::FoldingRangesResult { ranges, error, .. }) => {
                assert_eq!(ranges, None);
                assert_eq!(error.as_deref(), Some(INVALID_FOLDING_RANGES_RESPONSE));
            }
            other => panic!("expected folding ranges result event, got {other:?}"),
        }
    }

    #[test]
    fn folding_ranges_result_treats_null_success_as_empty() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_folding_ranges_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({ "result": null }),
            &tx,
        );

        match rx.recv().expect("folding ranges result event") {
            UiEvent::Lsp(LspUiEvent::FoldingRangesResult { ranges, error, .. }) => {
                assert_eq!(ranges, Some(Vec::new()));
                assert_eq!(error, None);
            }
            other => panic!("expected folding ranges result event, got {other:?}"),
        }
    }

    #[test]
    fn folding_ranges_result_only_validates_the_bounded_result_prefix() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let mut items = (0..MAX_VALIDATED_FOLDING_RANGES)
            .map(|line| json!({ "startLine": line * 2, "endLine": line * 2 + 1 }))
            .collect::<Vec<Value>>();
        items.push(json!({ "startLine": 0 }));

        send_folding_ranges_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({ "result": items }),
            &tx,
        );

        match rx.recv().expect("folding ranges result event") {
            UiEvent::Lsp(LspUiEvent::FoldingRangesResult { ranges, error, .. }) => {
                assert_eq!(error, None);
                assert_eq!(
                    ranges.expect("parsed folding ranges").len(),
                    MAX_VALIDATED_FOLDING_RANGES
                );
            }
            other => panic!("expected folding ranges result event, got {other:?}"),
        }
    }

    #[test]
    fn folding_ranges_result_sends_parsed_ranges_event() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_folding_ranges_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            &json!({
                "result": [
                    {
                        "startLine": 1,
                        "startCharacter": 4,
                        "endLine": 3,
                        "endCharacter": 0,
                        "kind": "region"
                    }
                ]
            }),
            &tx,
        );

        match rx.recv().expect("folding ranges result event") {
            UiEvent::Lsp(LspUiEvent::FoldingRangesResult {
                id,
                path,
                version,
                ranges,
                error,
            }) => {
                assert_eq!(id, 7);
                assert_eq!(path, PathBuf::from("src/main.rs"));
                assert_eq!(version, 11);
                let ranges = ranges.expect("parsed folding ranges");
                assert_eq!(ranges.len(), 1);
                assert_eq!(ranges[0].start_line, 2);
                assert_eq!(ranges[0].start_column, Some(5));
                assert_eq!(ranges[0].end_line, 4);
                assert_eq!(ranges[0].end_column, Some(1));
                assert_eq!(ranges[0].kind.as_deref(), Some("region"));
                assert_eq!(error, None);
            }
            other => panic!("expected folding ranges result event, got {other:?}"),
        }
    }
}
