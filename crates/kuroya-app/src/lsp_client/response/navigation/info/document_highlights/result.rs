use super::super::super::super::response_error;
use crate::ui_event_channel::Sender;
use crate::{lsp_ui_events::LspUiEvent, ui_events::UiEvent};
use kuroya_core::{BufferId, LspDocumentHighlight};
use serde_json::Value;
use std::path::PathBuf;

const INVALID_DOCUMENT_HIGHLIGHTS_RESPONSE: &str =
    "invalid textDocument/documentHighlight response";
const MAX_DOCUMENT_HIGHLIGHTS: usize = 500;
const MAX_DOCUMENT_HIGHLIGHT_POSITION_COMPONENT: u64 = i32::MAX as u64;

pub(super) fn send_document_highlights_result(
    id: BufferId,
    path: PathBuf,
    version: u64,
    line: usize,
    character: usize,
    value: &Value,
    ui_tx: &Sender<UiEvent>,
) {
    let mut error = response_error(value);
    let highlights = if error.is_none() {
        match parse_document_highlights_success_response(value) {
            Some(highlights) => Some(highlights),
            None => {
                error = Some(INVALID_DOCUMENT_HIGHLIGHTS_RESPONSE.to_owned());
                None
            }
        }
    } else {
        None
    };
    let _ = crate::lsp_client::response::emit_lsp_response_event(
        ui_tx,
        LspUiEvent::DocumentHighlightsResult {
            id,
            path,
            version,
            line,
            column: character.saturating_add(1),
            highlights,
            error,
        },
    );
}

fn parse_document_highlights_success_response(value: &Value) -> Option<Vec<LspDocumentHighlight>> {
    let result = value.get("result")?;
    if result.is_null() {
        return Some(Vec::new());
    }

    let items = result.as_array()?;
    let mut highlights = Vec::with_capacity(items.len().min(MAX_DOCUMENT_HIGHLIGHTS));
    for item in items.iter().take(MAX_DOCUMENT_HIGHLIGHTS) {
        let highlight = parse_document_highlight_item(item)?;
        highlights.push(highlight);
    }

    highlights.sort_unstable_by(|a, b| {
        a.line
            .cmp(&b.line)
            .then(a.column.cmp(&b.column))
            .then(a.end_line.cmp(&b.end_line))
            .then(a.end_column.cmp(&b.end_column))
            .then(a.kind.cmp(&b.kind))
    });
    highlights.dedup();
    Some(highlights)
}

fn parse_document_highlight_item(value: &Value) -> Option<LspDocumentHighlight> {
    let (line, column, end_line, end_column) = parse_lsp_range(value.get("range")?)?;
    let kind = match value.get("kind") {
        Some(kind) => Some(parse_document_highlight_kind(kind)?),
        None => None,
    };

    Some(LspDocumentHighlight {
        line,
        column,
        end_line,
        end_column,
        kind,
    })
}

fn parse_document_highlight_kind(value: &Value) -> Option<u8> {
    let kind = u8::try_from(value.as_u64()?).ok()?;
    matches!(kind, 1..=3).then_some(kind)
}

fn parse_lsp_range(value: &Value) -> Option<(usize, usize, usize, usize)> {
    let (line, column) = parse_lsp_position(value.get("start")?)?;
    let (end_line, end_column) = parse_lsp_position(value.get("end")?)?;
    (end_line > line || (end_line == line && end_column >= column))
        .then_some((line, column, end_line, end_column))
}

fn parse_lsp_position(value: &Value) -> Option<(usize, usize)> {
    Some((
        one_based_lsp_position_component(value.get("line")?.as_u64()?)?,
        one_based_lsp_position_component(value.get("character")?.as_u64()?)?,
    ))
}

fn one_based_lsp_position_component(value: u64) -> Option<usize> {
    if value > MAX_DOCUMENT_HIGHLIGHT_POSITION_COMPONENT {
        return None;
    }
    usize::try_from(value).ok()?.checked_add(1)
}

#[cfg(test)]
mod tests {
    use super::{
        INVALID_DOCUMENT_HIGHLIGHTS_RESPONSE, MAX_DOCUMENT_HIGHLIGHTS,
        send_document_highlights_result,
    };
    use crate::{lsp_ui_events::LspUiEvent, ui_events::UiEvent};
    use serde_json::json;
    use std::path::PathBuf;

    fn highlight_item(line: usize) -> serde_json::Value {
        json!({
            "range": {
                "start": { "line": line, "character": 2 },
                "end": { "line": line, "character": 5 }
            },
            "kind": 2
        })
    }

    #[test]
    fn document_highlights_result_reports_invalid_success_payload() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_document_highlights_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({ "result": [{ "range": { "start": { "line": 1 } } }] }),
            &tx,
        );

        match rx.recv().expect("document highlights result event") {
            UiEvent::Lsp(LspUiEvent::DocumentHighlightsResult {
                id,
                path,
                version,
                line,
                column,
                highlights,
                error,
            }) => {
                assert_eq!(id, 7);
                assert_eq!(path, PathBuf::from("src/main.rs"));
                assert_eq!(version, 11);
                assert_eq!(line, 3);
                assert_eq!(column, 5);
                assert_eq!(highlights, None);
                assert_eq!(error.as_deref(), Some(INVALID_DOCUMENT_HIGHLIGHTS_RESPONSE));
            }
            other => panic!("expected document highlights result event, got {other:?}"),
        }
    }

    #[test]
    fn document_highlights_result_reports_invalid_kind_type() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_document_highlights_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({
                "result": [{
                    "range": {
                        "start": { "line": 1, "character": 2 },
                        "end": { "line": 1, "character": 5 }
                    },
                    "kind": "read"
                }]
            }),
            &tx,
        );

        match rx.recv().expect("document highlights result event") {
            UiEvent::Lsp(LspUiEvent::DocumentHighlightsResult {
                highlights, error, ..
            }) => {
                assert_eq!(highlights, None);
                assert_eq!(error.as_deref(), Some(INVALID_DOCUMENT_HIGHLIGHTS_RESPONSE));
            }
            other => panic!("expected document highlights result event, got {other:?}"),
        }
    }

    #[test]
    fn document_highlights_result_reports_invalid_reversed_range() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_document_highlights_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({
                "result": [{
                    "range": {
                        "start": { "line": 1, "character": 5 },
                        "end": { "line": 1, "character": 2 }
                    }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("document highlights result event") {
            UiEvent::Lsp(LspUiEvent::DocumentHighlightsResult {
                highlights, error, ..
            }) => {
                assert_eq!(highlights, None);
                assert_eq!(error.as_deref(), Some(INVALID_DOCUMENT_HIGHLIGHTS_RESPONSE));
            }
            other => panic!("expected document highlights result event, got {other:?}"),
        }
    }

    #[test]
    fn document_highlights_result_reports_invalid_kind_overflow() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_document_highlights_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({
                "result": [{
                    "range": {
                        "start": { "line": 1, "character": 2 },
                        "end": { "line": 1, "character": 5 }
                    },
                    "kind": 999
                }]
            }),
            &tx,
        );

        match rx.recv().expect("document highlights result event") {
            UiEvent::Lsp(LspUiEvent::DocumentHighlightsResult {
                highlights, error, ..
            }) => {
                assert_eq!(highlights, None);
                assert_eq!(error.as_deref(), Some(INVALID_DOCUMENT_HIGHLIGHTS_RESPONSE));
            }
            other => panic!("expected document highlights result event, got {other:?}"),
        }
    }

    #[test]
    fn document_highlights_result_reports_unknown_kind_value() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_document_highlights_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({
                "result": [{
                    "range": {
                        "start": { "line": 1, "character": 2 },
                        "end": { "line": 1, "character": 5 }
                    },
                    "kind": 4
                }]
            }),
            &tx,
        );

        match rx.recv().expect("document highlights result event") {
            UiEvent::Lsp(LspUiEvent::DocumentHighlightsResult {
                highlights, error, ..
            }) => {
                assert_eq!(highlights, None);
                assert_eq!(error.as_deref(), Some(INVALID_DOCUMENT_HIGHLIGHTS_RESPONSE));
            }
            other => panic!("expected document highlights result event, got {other:?}"),
        }
    }

    #[test]
    fn document_highlights_result_reports_overflowing_position_component() {
        let overflowing_component = (i32::MAX as u64) + 1;
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_document_highlights_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({
                "result": [{
                    "range": {
                        "start": { "line": overflowing_component, "character": 2 },
                        "end": { "line": overflowing_component, "character": 5 }
                    }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("document highlights result event") {
            UiEvent::Lsp(LspUiEvent::DocumentHighlightsResult {
                highlights, error, ..
            }) => {
                assert_eq!(highlights, None);
                assert_eq!(error.as_deref(), Some(INVALID_DOCUMENT_HIGHLIGHTS_RESPONSE));
            }
            other => panic!("expected document highlights result event, got {other:?}"),
        }
    }

    #[test]
    fn document_highlights_result_bounds_unrepresentable_origin_column() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_document_highlights_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            usize::MAX,
            &json!({ "result": null }),
            &tx,
        );

        match rx.recv().expect("document highlights result event") {
            UiEvent::Lsp(LspUiEvent::DocumentHighlightsResult {
                column,
                highlights,
                error,
                ..
            }) => {
                assert_eq!(column, usize::MAX);
                assert_eq!(highlights, Some(Vec::new()));
                assert_eq!(error, None);
            }
            other => panic!("expected document highlights result event, got {other:?}"),
        }
    }

    #[test]
    fn document_highlights_result_treats_null_success_as_empty() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_document_highlights_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({ "result": null }),
            &tx,
        );

        match rx.recv().expect("document highlights result event") {
            UiEvent::Lsp(LspUiEvent::DocumentHighlightsResult {
                highlights, error, ..
            }) => {
                assert_eq!(highlights, Some(Vec::new()));
                assert_eq!(error, None);
            }
            other => panic!("expected document highlights result event, got {other:?}"),
        }
    }

    #[test]
    fn document_highlights_result_sends_parsed_highlights_event() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_document_highlights_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({
                "result": [{
                    "range": {
                        "start": { "line": 1, "character": 2 },
                        "end": { "line": 1, "character": 5 }
                    },
                    "kind": 2
                }]
            }),
            &tx,
        );

        match rx.recv().expect("document highlights result event") {
            UiEvent::Lsp(LspUiEvent::DocumentHighlightsResult {
                highlights, error, ..
            }) => {
                let highlights = highlights.expect("parsed document highlights");
                assert_eq!(highlights.len(), 1);
                assert_eq!(highlights[0].line, 2);
                assert_eq!(highlights[0].column, 3);
                assert_eq!(highlights[0].end_line, 2);
                assert_eq!(highlights[0].end_column, 6);
                assert_eq!(highlights[0].kind, Some(2));
                assert_eq!(error, None);
            }
            other => panic!("expected document highlights result event, got {other:?}"),
        }
    }

    #[test]
    fn document_highlights_result_caps_emitted_highlights() {
        let mut result = (0..=MAX_DOCUMENT_HIGHLIGHTS)
            .map(highlight_item)
            .collect::<Vec<_>>();
        result[MAX_DOCUMENT_HIGHLIGHTS]["range"] = json!({
            "start": { "line": 1, "character": 5 },
            "end": { "line": 1, "character": 2 }
        });

        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_document_highlights_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({ "result": result }),
            &tx,
        );

        match rx.recv().expect("document highlights result event") {
            UiEvent::Lsp(LspUiEvent::DocumentHighlightsResult {
                highlights, error, ..
            }) => {
                let highlights = highlights.expect("capped document highlights");
                assert_eq!(highlights.len(), MAX_DOCUMENT_HIGHLIGHTS);
                assert_eq!(error, None);
            }
            other => panic!("expected document highlights result event, got {other:?}"),
        }
    }
}
