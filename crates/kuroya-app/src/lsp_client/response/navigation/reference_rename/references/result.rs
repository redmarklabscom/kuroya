use super::super::super::super::response_error;
use crate::ui_event_channel::Sender;
use crate::{lsp_ui_events::LspUiEvent, ui_events::UiEvent};
use kuroya_core::{BufferId, LspReference, lsp::file_uri_to_path};
use serde_json::Value;
use std::path::PathBuf;

const INVALID_REFERENCES_RESPONSE: &str = "invalid textDocument/references response";
const MAX_EMITTED_REFERENCES: usize = 5_000;
const MAX_REFERENCE_POSITION_COMPONENT: usize = i32::MAX as usize;

pub(super) fn send_references_result(
    id: BufferId,
    path: PathBuf,
    version: u64,
    line: usize,
    character: usize,
    value: &Value,
    ui_tx: &Sender<UiEvent>,
) {
    let mut error = response_error(value);
    let references = if error.is_none() {
        match parse_references_success_response(value) {
            Some(references) => Some(references),
            None => {
                error = Some(INVALID_REFERENCES_RESPONSE.to_owned());
                None
            }
        }
    } else {
        None
    };
    let _ = crate::lsp_client::response::emit_critical_lsp_response_event(
        ui_tx,
        LspUiEvent::ReferencesResult {
            id,
            path,
            version,
            line,
            column: character.saturating_add(1),
            references,
            error,
        },
    );
}

fn parse_references_success_response(value: &Value) -> Option<Vec<LspReference>> {
    let result = value.get("result")?;
    if result.is_null() {
        return Some(Vec::new());
    }

    let items = result.as_array()?;
    let mut references = Vec::with_capacity(items.len().min(MAX_EMITTED_REFERENCES));
    for item in items.iter().take(MAX_EMITTED_REFERENCES) {
        references.push(parse_reference_item(item)?);
    }

    references.sort_unstable_by(|a, b| {
        a.path
            .cmp(&b.path)
            .then(a.line.cmp(&b.line))
            .then(a.column.cmp(&b.column))
            .then(a.end_line.cmp(&b.end_line))
            .then(a.end_column.cmp(&b.end_column))
    });
    references.dedup();
    Some(references)
}

fn parse_reference_item(value: &Value) -> Option<LspReference> {
    let (uri, range) = if let Some(uri) = value.get("uri").and_then(Value::as_str) {
        (uri, value.get("range")?)
    } else {
        (
            value.get("targetUri")?.as_str()?,
            value
                .get("targetSelectionRange")
                .or_else(|| value.get("targetRange"))?,
        )
    };
    let path = file_uri_to_path(uri)?;
    let ((line, column), (end_line, end_column)) = reference_range(range)?;

    Some(LspReference {
        path,
        line: one_based_reference_position_component(line)?,
        column: one_based_reference_position_component(column)?,
        end_line: one_based_reference_position_component(end_line)?,
        end_column: one_based_reference_position_component(end_column)?,
    })
}

fn reference_range(value: &Value) -> Option<((usize, usize), (usize, usize))> {
    let (start_line, start_character) = value.get("start").and_then(reference_position)?;
    let (end_line, end_character) = value.get("end").and_then(reference_position)?;

    (end_line > start_line || (end_line == start_line && end_character >= start_character))
        .then_some(((start_line, start_character), (end_line, end_character)))
}

fn reference_position(value: &Value) -> Option<(usize, usize)> {
    Some((
        reference_position_component(value.get("line")?)?,
        reference_position_component(value.get("character")?)?,
    ))
}

fn reference_position_component(value: &Value) -> Option<usize> {
    let component = usize::try_from(value.as_u64()?).ok()?;
    if component > MAX_REFERENCE_POSITION_COMPONENT {
        return None;
    }
    Some(component)
}

fn one_based_reference_position_component(component: usize) -> Option<usize> {
    component.checked_add(1)
}

#[cfg(test)]
mod tests {
    use super::{INVALID_REFERENCES_RESPONSE, MAX_EMITTED_REFERENCES, send_references_result};
    use crate::{lsp_ui_events::LspUiEvent, ui_events::UiEvent};
    use kuroya_core::lsp::path_to_file_uri;
    use serde_json::json;
    use std::path::{Path, PathBuf};

    fn location_item(uri: &str) -> serde_json::Value {
        json!({
            "uri": uri,
            "range": {
                "start": { "line": 4, "character": 8 },
                "end": { "line": 4, "character": 12 }
            }
        })
    }

    fn indexed_location_item(index: usize) -> serde_json::Value {
        location_item(&format!("file:///src/reference_{index:04}.rs"))
    }

    #[test]
    fn references_result_reports_invalid_success_payload() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_references_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({ "result": { "not": "a list" } }),
            &tx,
        );

        match rx.recv().expect("references result event") {
            UiEvent::Lsp(LspUiEvent::ReferencesResult {
                id,
                path,
                version,
                line,
                column,
                references,
                error,
            }) => {
                assert_eq!(id, 7);
                assert_eq!(path, PathBuf::from("src/main.rs"));
                assert_eq!(version, 11);
                assert_eq!(line, 3);
                assert_eq!(column, 5);
                assert_eq!(references, None);
                assert_eq!(error.as_deref(), Some(INVALID_REFERENCES_RESPONSE));
            }
            other => panic!("expected references result event, got {other:?}"),
        }
    }

    #[test]
    fn references_result_reports_malformed_location_item() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_references_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({
                "result": [{
                    "uri": "file:///src/main.rs",
                    "range": {
                        "start": { "line": 1, "character": 2 },
                        "end": { "line": 1 }
                    }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("references result event") {
            UiEvent::Lsp(LspUiEvent::ReferencesResult {
                references, error, ..
            }) => {
                assert_eq!(references, None);
                assert_eq!(error.as_deref(), Some(INVALID_REFERENCES_RESPONSE));
            }
            other => panic!("expected references result event, got {other:?}"),
        }
    }

    #[test]
    fn references_result_reports_malformed_location_link_item() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_references_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({
                "result": [{
                    "targetUri": "file:///src/lib.rs",
                    "targetRange": {
                        "start": { "line": 1, "character": 2 },
                        "end": { "line": 1, "character": "bad" }
                    }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("references result event") {
            UiEvent::Lsp(LspUiEvent::ReferencesResult {
                references, error, ..
            }) => {
                assert_eq!(references, None);
                assert_eq!(error.as_deref(), Some(INVALID_REFERENCES_RESPONSE));
            }
            other => panic!("expected references result event, got {other:?}"),
        }
    }

    #[test]
    fn references_result_reports_invalid_uri_before_parsing() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_references_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({
                "result": [location_item("file:///src/lib%GG.rs")]
            }),
            &tx,
        );

        match rx.recv().expect("references result event") {
            UiEvent::Lsp(LspUiEvent::ReferencesResult {
                references, error, ..
            }) => {
                assert_eq!(references, None);
                assert_eq!(error.as_deref(), Some(INVALID_REFERENCES_RESPONSE));
            }
            other => panic!("expected references result event, got {other:?}"),
        }
    }

    #[test]
    fn references_result_reports_overflowing_position_component() {
        let overflowing_component = (i32::MAX as u64) + 1;
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_references_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({
                "result": [{
                    "uri": "file:///src/lib.rs",
                    "range": {
                        "start": { "line": overflowing_component, "character": 0 },
                        "end": { "line": overflowing_component, "character": 1 }
                    }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("references result event") {
            UiEvent::Lsp(LspUiEvent::ReferencesResult {
                references, error, ..
            }) => {
                assert_eq!(references, None);
                assert_eq!(error.as_deref(), Some(INVALID_REFERENCES_RESPONSE));
            }
            other => panic!("expected references result event, got {other:?}"),
        }
    }

    #[test]
    fn references_result_treats_null_success_as_empty() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_references_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({ "result": null }),
            &tx,
        );

        match rx.recv().expect("references result event") {
            UiEvent::Lsp(LspUiEvent::ReferencesResult {
                references, error, ..
            }) => {
                assert_eq!(references, Some(Vec::new()));
                assert_eq!(error, None);
            }
            other => panic!("expected references result event, got {other:?}"),
        }
    }

    #[test]
    fn references_result_bounds_unrepresentable_origin_column() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_references_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            usize::MAX,
            &json!({ "result": null }),
            &tx,
        );

        match rx.recv().expect("references result event") {
            UiEvent::Lsp(LspUiEvent::ReferencesResult {
                column,
                references,
                error,
                ..
            }) => {
                assert_eq!(column, usize::MAX);
                assert_eq!(references, Some(Vec::new()));
                assert_eq!(error, None);
            }
            other => panic!("expected references result event, got {other:?}"),
        }
    }

    #[test]
    fn references_result_sends_parsed_locations_and_location_links_event() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_references_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({
                "result": [
                    {
                        "uri": "file:///src/main.rs",
                        "range": {
                            "start": { "line": 4, "character": 8 },
                            "end": { "line": 4, "character": 12 }
                        }
                    },
                    {
                        "targetUri": "file:///src/lib.rs",
                        "targetSelectionRange": {
                            "start": { "line": 1, "character": 2 },
                            "end": { "line": 1, "character": 6 }
                        }
                    }
                ]
            }),
            &tx,
        );

        match rx.recv().expect("references result event") {
            UiEvent::Lsp(LspUiEvent::ReferencesResult {
                references, error, ..
            }) => {
                let references = references.expect("parsed references");
                assert_eq!(references.len(), 2);
                let lib = references
                    .iter()
                    .find(|reference| reference.path.ends_with(Path::new("src").join("lib.rs")))
                    .expect("location link reference");
                assert_eq!(lib.line, 2);
                assert_eq!(lib.column, 3);
                assert_eq!(lib.end_column, 7);
                let main = references
                    .iter()
                    .find(|reference| reference.path.ends_with(Path::new("src").join("main.rs")))
                    .expect("location reference");
                assert_eq!(main.line, 5);
                assert_eq!(main.column, 9);
                assert_eq!(main.end_column, 13);
                assert_eq!(error, None);
            }
            other => panic!("expected references result event, got {other:?}"),
        }
    }

    #[test]
    fn references_result_preserves_raw_request_and_reference_paths() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let request_path = PathBuf::from("workspace/src/request\n\u{202e}.rs");
        let reference_path = PathBuf::from("workspace/src/ref\n\u{202e}.rs");
        let reference_uri = path_to_file_uri(&reference_path);

        send_references_result(
            7,
            request_path.clone(),
            11,
            3,
            4,
            &json!({
                "result": [{
                    "uri": reference_uri,
                    "range": {
                        "start": { "line": 4, "character": 8 },
                        "end": { "line": 4, "character": 12 }
                    }
                }]
            }),
            &tx,
        );

        match rx.recv().expect("references result event") {
            UiEvent::Lsp(LspUiEvent::ReferencesResult {
                path,
                references,
                error,
                ..
            }) => {
                let references = references.expect("parsed references");
                assert_eq!(path, request_path);
                assert_eq!(references.len(), 1);
                assert!(references[0].path.ends_with(&reference_path));
                assert!(references[0].path.to_string_lossy().contains('\n'));
                assert!(references[0].path.to_string_lossy().contains('\u{202e}'));
                assert_eq!(references[0].line, 5);
                assert_eq!(references[0].column, 9);
                assert_eq!(references[0].end_column, 13);
                assert_eq!(error, None);
            }
            other => panic!("expected references result event, got {other:?}"),
        }
    }

    #[test]
    fn references_result_allows_duplicate_valid_locations() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_references_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({
                "result": [
                    location_item("file:///src/main.rs"),
                    location_item("file:///src/main.rs")
                ]
            }),
            &tx,
        );

        match rx.recv().expect("references result event") {
            UiEvent::Lsp(LspUiEvent::ReferencesResult {
                references, error, ..
            }) => {
                let references = references.expect("deduped references");
                assert_eq!(references.len(), 1);
                assert_eq!(error, None);
            }
            other => panic!("expected references result event, got {other:?}"),
        }
    }

    #[test]
    fn references_result_caps_emitted_locations() {
        let mut result = (0..=MAX_EMITTED_REFERENCES)
            .map(indexed_location_item)
            .collect::<Vec<_>>();
        result[MAX_EMITTED_REFERENCES]["range"] = json!({
            "start": { "line": 4, "character": 12 },
            "end": { "line": 4, "character": 8 }
        });

        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_references_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            4,
            &json!({ "result": result }),
            &tx,
        );

        match rx.recv().expect("references result event") {
            UiEvent::Lsp(LspUiEvent::ReferencesResult {
                references, error, ..
            }) => {
                let references = references.expect("capped references");
                assert_eq!(references.len(), MAX_EMITTED_REFERENCES);
                assert_eq!(error, None);
            }
            other => panic!("expected references result event, got {other:?}"),
        }
    }
}
