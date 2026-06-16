use super::super::super::response_error;
use crate::ui_event_channel::Sender;
use crate::{lsp_ui_events::LspUiEvent, ui_events::UiEvent};
use kuroya_core::{BufferId, LspSignatureHelp, parse_signature_help_response};
use serde_json::Value;
use std::path::PathBuf;

const INVALID_SIGNATURE_HELP_RESPONSE: &str = "invalid textDocument/signatureHelp response";
const MAX_VALIDATED_SIGNATURES: usize = 20;
const MAX_VALIDATED_SIGNATURE_PARAMETERS: usize = 30;
const MAX_VALIDATED_SIGNATURE_LABEL_CHARS: usize = 16_000;
const MAX_VALIDATED_SIGNATURE_PARAMETER_LABEL_CHARS: usize = 4_000;
const MAX_VALIDATED_SIGNATURE_DOCUMENTATION_CHARS: usize = 16_000;
const MAX_VALIDATED_SIGNATURE_PARAMETER_DOCUMENTATION_CHARS: usize = 8_000;
const MAX_VALIDATED_SIGNATURE_DOCUMENTATION_KIND_CHARS: usize = 64;
const MAX_VALIDATED_SIGNATURE_DOCUMENTATION_PARTS: usize = 64;
const MAX_VALIDATED_SIGNATURE_DOCUMENTATION_DEPTH: usize = 4;

pub(super) fn send_signature_help_result(
    id: BufferId,
    path: PathBuf,
    version: u64,
    line: usize,
    character: usize,
    value: &Value,
    ui_tx: &Sender<UiEvent>,
) {
    let mut error = response_error(value);
    let help = if error.is_none() {
        match parse_signature_help_success_response(value) {
            Some(help) => Some(help),
            None => {
                error = Some(INVALID_SIGNATURE_HELP_RESPONSE.to_owned());
                None
            }
        }
    } else {
        None
    };
    let _ = crate::lsp_client::response::emit_lsp_response_event(
        ui_tx,
        LspUiEvent::SignatureHelpResult {
            id,
            path,
            version,
            line,
            column: signature_help_event_column(character),
            help,
            error,
        },
    );
}

fn signature_help_event_column(character: usize) -> usize {
    character.saturating_add(1)
}

fn parse_signature_help_success_response(value: &Value) -> Option<LspSignatureHelp> {
    let result = value.get("result")?;
    if result.is_null() {
        return Some(empty_signature_help());
    }

    let signatures = result.get("signatures")?.as_array()?;
    let active_signature = optional_lsp_index(result.get("activeSignature"))?;
    let active_parameter = optional_lsp_index(result.get("activeParameter"))?;

    if signatures.len() > MAX_VALIDATED_SIGNATURES {
        return None;
    }

    if signatures
        .iter()
        .any(|signature| !signature_information_shape_is_valid(signature))
    {
        return None;
    }

    if signatures.is_empty() {
        return Some(empty_signature_help());
    }

    if !active_signature_selection_is_valid(signatures, active_signature, active_parameter) {
        return None;
    }

    parse_signature_help_response(value)
}

fn empty_signature_help() -> LspSignatureHelp {
    LspSignatureHelp {
        signatures: Vec::new(),
        active_signature: 0,
        active_parameter: None,
    }
}

fn signature_information_shape_is_valid(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    let Some(label) = object.get("label").and_then(Value::as_str) else {
        return false;
    };

    if !bounded_non_empty_string_chars(label, MAX_VALIDATED_SIGNATURE_LABEL_CHARS) {
        return false;
    }

    if object.get("documentation").is_some_and(|documentation| {
        !signature_documentation_shape_is_valid(
            documentation,
            MAX_VALIDATED_SIGNATURE_DOCUMENTATION_CHARS,
        )
    }) {
        return false;
    }

    let active_parameter = match object.get("activeParameter") {
        Some(active_parameter) => {
            let Some(active_parameter) = lsp_index(active_parameter) else {
                return false;
            };
            Some(active_parameter)
        }
        None => None,
    };

    let Some(parameters) = object.get("parameters") else {
        return active_parameter.is_none();
    };
    let Some(parameters) = parameters.as_array() else {
        return false;
    };
    if parameters.len() > MAX_VALIDATED_SIGNATURE_PARAMETERS {
        return false;
    }

    parameters
        .iter()
        .all(|parameter| parameter_information_shape_is_valid(parameter, label))
        && active_parameter.is_none_or(|active_parameter| active_parameter < parameters.len())
}

fn active_signature_selection_is_valid(
    signatures: &[Value],
    active_signature: Option<usize>,
    active_parameter: Option<usize>,
) -> bool {
    let active_signature = active_signature.unwrap_or(0);
    let Some(signature) = signatures.get(active_signature) else {
        return false;
    };

    if let Some(active_parameter) = active_parameter {
        return signature_parameter_index_is_valid(signature, active_parameter);
    }

    let Some(signature_active_parameter) = signature.get("activeParameter") else {
        return true;
    };
    let Some(signature_active_parameter) = lsp_index(signature_active_parameter) else {
        return false;
    };
    signature_parameter_index_is_valid(signature, signature_active_parameter)
}

fn signature_parameter_index_is_valid(signature: &Value, active_parameter: usize) -> bool {
    signature
        .get("parameters")
        .and_then(Value::as_array)
        .is_some_and(|parameters| {
            active_parameter < parameters.len().min(MAX_VALIDATED_SIGNATURE_PARAMETERS)
        })
}

fn optional_lsp_index(value: Option<&Value>) -> Option<Option<usize>> {
    match value {
        Some(value) => lsp_index(value).map(Some),
        None => Some(None),
    }
}

fn lsp_index(value: &Value) -> Option<usize> {
    usize::try_from(value.as_u64()?).ok()
}

fn bounded_string_chars(text: &str, max_chars: usize) -> bool {
    if text.len() > max_chars.saturating_mul(4) {
        return false;
    }
    text.chars().take(max_chars.saturating_add(1)).count() <= max_chars
}

fn bounded_non_empty_string_chars(text: &str, max_chars: usize) -> bool {
    !text.trim().is_empty() && bounded_string_chars(text, max_chars)
}

fn parameter_information_shape_is_valid(value: &Value, signature_label: &str) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    let Some(label) = object.get("label") else {
        return false;
    };

    if !parameter_label_shape_is_valid(label, signature_label) {
        return false;
    }

    object.get("documentation").is_none_or(|documentation| {
        signature_documentation_shape_is_valid(
            documentation,
            MAX_VALIDATED_SIGNATURE_PARAMETER_DOCUMENTATION_CHARS,
        )
    })
}

fn parameter_label_shape_is_valid(value: &Value, signature_label: &str) -> bool {
    if let Some(label) = value.as_str() {
        return bounded_non_empty_string_chars(
            label,
            MAX_VALIDATED_SIGNATURE_PARAMETER_LABEL_CHARS,
        );
    }

    let Some(range) = value.as_array() else {
        return false;
    };
    if range.len() != 2 {
        return false;
    }

    let Some(start) = range.first().and_then(lsp_index) else {
        return false;
    };
    let Some(end) = range.get(1).and_then(lsp_index) else {
        return false;
    };

    start < end
        && end <= signature_label.chars().count()
        && end - start <= MAX_VALIDATED_SIGNATURE_PARAMETER_LABEL_CHARS
        && signature_label
            .chars()
            .skip(start)
            .take(end - start)
            .any(|ch| !ch.is_whitespace())
}

fn signature_documentation_shape_is_valid(value: &Value, max_chars: usize) -> bool {
    let mut remaining_parts = MAX_VALIDATED_SIGNATURE_DOCUMENTATION_PARTS;
    signature_documentation_shape_is_valid_at_depth(value, max_chars, 0, &mut remaining_parts)
}

fn signature_documentation_shape_is_valid_at_depth(
    value: &Value,
    max_chars: usize,
    depth: usize,
    remaining_parts: &mut usize,
) -> bool {
    if let Some(text) = value.as_str() {
        return bounded_string_chars(text, max_chars);
    }

    if let Some(items) = value.as_array() {
        if depth >= MAX_VALIDATED_SIGNATURE_DOCUMENTATION_DEPTH
            || items.len() > MAX_VALIDATED_SIGNATURE_DOCUMENTATION_PARTS
            || items.len() > *remaining_parts
        {
            return false;
        }

        *remaining_parts -= items.len();
        return items.iter().all(|item| {
            signature_documentation_shape_is_valid_at_depth(
                item,
                max_chars,
                depth + 1,
                remaining_parts,
            )
        });
    }

    value.as_object().is_some_and(|object| {
        object.get("kind").is_none_or(|kind| {
            kind.as_str().is_some_and(|kind| {
                bounded_string_chars(kind, MAX_VALIDATED_SIGNATURE_DOCUMENTATION_KIND_CHARS)
            })
        }) && object.get("language").is_none_or(|language| {
            language.as_str().is_some_and(|language| {
                bounded_string_chars(language, MAX_VALIDATED_SIGNATURE_DOCUMENTATION_KIND_CHARS)
            })
        }) && object
            .get("value")
            .and_then(Value::as_str)
            .is_some_and(|text| bounded_string_chars(text, max_chars))
    })
}

#[cfg(test)]
mod tests {
    use super::{
        INVALID_SIGNATURE_HELP_RESPONSE, MAX_VALIDATED_SIGNATURE_DOCUMENTATION_CHARS,
        MAX_VALIDATED_SIGNATURE_DOCUMENTATION_PARTS, MAX_VALIDATED_SIGNATURE_LABEL_CHARS,
        MAX_VALIDATED_SIGNATURE_PARAMETER_DOCUMENTATION_CHARS,
        MAX_VALIDATED_SIGNATURE_PARAMETER_LABEL_CHARS, MAX_VALIDATED_SIGNATURE_PARAMETERS,
        MAX_VALIDATED_SIGNATURES, send_signature_help_result,
    };
    use crate::{lsp_ui_events::LspUiEvent, ui_events::UiEvent};
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn signature_help_result_reports_invalid_success_payload() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_signature_help_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            12,
            &json!({ "result": { "not": "signature help" } }),
            &tx,
        );

        match rx.recv().expect("signature help result event") {
            UiEvent::Lsp(LspUiEvent::SignatureHelpResult { help, error, .. }) => {
                assert_eq!(help, None);
                assert_eq!(error.as_deref(), Some(INVALID_SIGNATURE_HELP_RESPONSE));
            }
            other => panic!("expected signature help result event, got {other:?}"),
        }
    }

    #[test]
    fn signature_help_result_saturates_unrepresentable_request_column() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_signature_help_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            usize::MAX,
            &json!({ "result": null }),
            &tx,
        );

        match rx.recv().expect("signature help result event") {
            UiEvent::Lsp(LspUiEvent::SignatureHelpResult {
                column,
                help,
                error,
                ..
            }) => {
                assert_eq!(column, usize::MAX);
                let help = help.expect("empty signature help");
                assert_eq!(help.signatures, Vec::new());
                assert_eq!(error, None);
            }
            other => panic!("expected signature help result event, got {other:?}"),
        }
    }

    #[test]
    fn signature_help_result_reports_malformed_signature_item() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_signature_help_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            12,
            &json!({
                "result": {
                    "signatures": [{
                        "label": "push(value: T)",
                        "parameters": [{
                            "label": [5, 500]
                        }]
                    }]
                }
            }),
            &tx,
        );

        match rx.recv().expect("signature help result event") {
            UiEvent::Lsp(LspUiEvent::SignatureHelpResult { help, error, .. }) => {
                assert_eq!(help, None);
                assert_eq!(error.as_deref(), Some(INVALID_SIGNATURE_HELP_RESPONSE));
            }
            other => panic!("expected signature help result event, got {other:?}"),
        }
    }

    #[test]
    fn signature_help_result_reports_oversized_signature_label() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_signature_help_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            12,
            &json!({
                "result": {
                    "signatures": [{
                        "label": "x".repeat(MAX_VALIDATED_SIGNATURE_LABEL_CHARS + 1)
                    }]
                }
            }),
            &tx,
        );

        match rx.recv().expect("signature help result event") {
            UiEvent::Lsp(LspUiEvent::SignatureHelpResult { help, error, .. }) => {
                assert_eq!(help, None);
                assert_eq!(error.as_deref(), Some(INVALID_SIGNATURE_HELP_RESPONSE));
            }
            other => panic!("expected signature help result event, got {other:?}"),
        }
    }

    #[test]
    fn signature_help_result_reports_oversized_parameter_label_string() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_signature_help_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            12,
            &json!({
                "result": {
                    "activeParameter": 0,
                    "signatures": [{
                        "label": "push(value: T)",
                        "parameters": [{
                            "label": "x".repeat(
                                MAX_VALIDATED_SIGNATURE_PARAMETER_LABEL_CHARS + 1
                            )
                        }]
                    }]
                }
            }),
            &tx,
        );

        match rx.recv().expect("signature help result event") {
            UiEvent::Lsp(LspUiEvent::SignatureHelpResult { help, error, .. }) => {
                assert_eq!(help, None);
                assert_eq!(error.as_deref(), Some(INVALID_SIGNATURE_HELP_RESPONSE));
            }
            other => panic!("expected signature help result event, got {other:?}"),
        }
    }

    #[test]
    fn signature_help_result_reports_oversized_parameter_label_range() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_signature_help_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            12,
            &json!({
                "result": {
                    "activeParameter": 0,
                    "signatures": [{
                        "label": "x".repeat(
                            MAX_VALIDATED_SIGNATURE_PARAMETER_LABEL_CHARS + 1
                        ),
                        "parameters": [{
                            "label": [0, MAX_VALIDATED_SIGNATURE_PARAMETER_LABEL_CHARS + 1]
                        }]
                    }]
                }
            }),
            &tx,
        );

        match rx.recv().expect("signature help result event") {
            UiEvent::Lsp(LspUiEvent::SignatureHelpResult { help, error, .. }) => {
                assert_eq!(help, None);
                assert_eq!(error.as_deref(), Some(INVALID_SIGNATURE_HELP_RESPONSE));
            }
            other => panic!("expected signature help result event, got {other:?}"),
        }
    }

    #[test]
    fn signature_help_result_reports_empty_parameter_label_range() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_signature_help_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            12,
            &json!({
                "result": {
                    "activeParameter": 0,
                    "signatures": [{
                        "label": "push(value: T)",
                        "parameters": [{
                            "label": [5, 5]
                        }]
                    }]
                }
            }),
            &tx,
        );

        match rx.recv().expect("signature help result event") {
            UiEvent::Lsp(LspUiEvent::SignatureHelpResult { help, error, .. }) => {
                assert_eq!(help, None);
                assert_eq!(error.as_deref(), Some(INVALID_SIGNATURE_HELP_RESPONSE));
            }
            other => panic!("expected signature help result event, got {other:?}"),
        }
    }

    #[test]
    fn signature_help_result_reports_parameter_tail_after_cap() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let mut parameters = (0..MAX_VALIDATED_SIGNATURE_PARAMETERS)
            .map(|idx| json!({ "label": format!("arg{idx}") }))
            .collect::<Vec<_>>();
        parameters.push(json!({ "label": [5, 4] }));

        send_signature_help_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            12,
            &json!({
                "result": {
                    "signatures": [{
                        "label": "call(arg0)",
                        "parameters": parameters
                    }]
                }
            }),
            &tx,
        );

        match rx.recv().expect("signature help result event") {
            UiEvent::Lsp(LspUiEvent::SignatureHelpResult { help, error, .. }) => {
                assert_eq!(help, None);
                assert_eq!(error.as_deref(), Some(INVALID_SIGNATURE_HELP_RESPONSE));
            }
            other => panic!("expected signature help result event, got {other:?}"),
        }
    }

    #[test]
    fn signature_help_result_reports_oversized_signature_documentation() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_signature_help_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            12,
            &json!({
                "result": {
                    "signatures": [{
                        "label": "push(value: T)",
                        "documentation": {
                            "kind": "markdown",
                            "value": "x".repeat(MAX_VALIDATED_SIGNATURE_DOCUMENTATION_CHARS + 1)
                        }
                    }]
                }
            }),
            &tx,
        );

        match rx.recv().expect("signature help result event") {
            UiEvent::Lsp(LspUiEvent::SignatureHelpResult { help, error, .. }) => {
                assert_eq!(help, None);
                assert_eq!(error.as_deref(), Some(INVALID_SIGNATURE_HELP_RESPONSE));
            }
            other => panic!("expected signature help result event, got {other:?}"),
        }
    }

    #[test]
    fn signature_help_result_reports_too_many_signature_documentation_parts() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let documentation = (0..=MAX_VALIDATED_SIGNATURE_DOCUMENTATION_PARTS)
            .map(|idx| json!(format!("part {idx}")))
            .collect::<Vec<_>>();

        send_signature_help_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            12,
            &json!({
                "result": {
                    "signatures": [{
                        "label": "push(value: T)",
                        "documentation": documentation
                    }]
                }
            }),
            &tx,
        );

        match rx.recv().expect("signature help result event") {
            UiEvent::Lsp(LspUiEvent::SignatureHelpResult { help, error, .. }) => {
                assert_eq!(help, None);
                assert_eq!(error.as_deref(), Some(INVALID_SIGNATURE_HELP_RESPONSE));
            }
            other => panic!("expected signature help result event, got {other:?}"),
        }
    }

    #[test]
    fn signature_help_result_reports_too_many_nested_documentation_parts() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let first = (0..32)
            .map(|idx| json!(format!("first {idx}")))
            .collect::<Vec<_>>();
        let second = (0..31)
            .map(|idx| json!(format!("second {idx}")))
            .collect::<Vec<_>>();

        send_signature_help_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            12,
            &json!({
                "result": {
                    "signatures": [{
                        "label": "push(value: T)",
                        "documentation": [first, second]
                    }]
                }
            }),
            &tx,
        );

        match rx.recv().expect("signature help result event") {
            UiEvent::Lsp(LspUiEvent::SignatureHelpResult { help, error, .. }) => {
                assert_eq!(help, None);
                assert_eq!(error.as_deref(), Some(INVALID_SIGNATURE_HELP_RESPONSE));
            }
            other => panic!("expected signature help result event, got {other:?}"),
        }
    }

    #[test]
    fn signature_help_result_reports_too_deep_signature_documentation() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_signature_help_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            12,
            &json!({
                "result": {
                    "signatures": [{
                        "label": "push(value: T)",
                        "documentation": [[[[["too deep"]]]]]
                    }]
                }
            }),
            &tx,
        );

        match rx.recv().expect("signature help result event") {
            UiEvent::Lsp(LspUiEvent::SignatureHelpResult { help, error, .. }) => {
                assert_eq!(help, None);
                assert_eq!(error.as_deref(), Some(INVALID_SIGNATURE_HELP_RESPONSE));
            }
            other => panic!("expected signature help result event, got {other:?}"),
        }
    }

    #[test]
    fn signature_help_result_reports_oversized_parameter_documentation() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_signature_help_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            12,
            &json!({
                "result": {
                    "signatures": [{
                        "label": "push(value: T)",
                        "parameters": [{
                            "label": "value: T",
                            "documentation": "x".repeat(
                                MAX_VALIDATED_SIGNATURE_PARAMETER_DOCUMENTATION_CHARS + 1
                            )
                        }]
                    }]
                }
            }),
            &tx,
        );

        match rx.recv().expect("signature help result event") {
            UiEvent::Lsp(LspUiEvent::SignatureHelpResult { help, error, .. }) => {
                assert_eq!(help, None);
                assert_eq!(error.as_deref(), Some(INVALID_SIGNATURE_HELP_RESPONSE));
            }
            other => panic!("expected signature help result event, got {other:?}"),
        }
    }

    #[test]
    fn signature_help_result_reports_active_signature_outside_signature_count() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_signature_help_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            12,
            &json!({
                "result": {
                    "activeSignature": 1,
                    "signatures": [{
                        "label": "push(value: T)",
                        "parameters": [{ "label": "value: T" }]
                    }]
                }
            }),
            &tx,
        );

        match rx.recv().expect("signature help result event") {
            UiEvent::Lsp(LspUiEvent::SignatureHelpResult { help, error, .. }) => {
                assert_eq!(help, None);
                assert_eq!(error.as_deref(), Some(INVALID_SIGNATURE_HELP_RESPONSE));
            }
            other => panic!("expected signature help result event, got {other:?}"),
        }
    }

    #[test]
    fn signature_help_result_reports_active_parameter_outside_signature_parameters() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_signature_help_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            12,
            &json!({
                "result": {
                    "activeParameter": 1,
                    "signatures": [{
                        "label": "push(value: T)",
                        "parameters": [{ "label": "value: T" }]
                    }]
                }
            }),
            &tx,
        );

        match rx.recv().expect("signature help result event") {
            UiEvent::Lsp(LspUiEvent::SignatureHelpResult { help, error, .. }) => {
                assert_eq!(help, None);
                assert_eq!(error.as_deref(), Some(INVALID_SIGNATURE_HELP_RESPONSE));
            }
            other => panic!("expected signature help result event, got {other:?}"),
        }
    }

    #[test]
    fn signature_help_result_reports_inactive_signature_active_parameter_outside_parameters() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_signature_help_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            12,
            &json!({
                "result": {
                    "activeSignature": 0,
                    "signatures": [
                        {
                            "label": "push(value: T)",
                            "parameters": [{ "label": "value: T" }]
                        },
                        {
                            "label": "insert(index: usize)",
                            "activeParameter": 1,
                            "parameters": [{ "label": "index: usize" }]
                        }
                    ]
                }
            }),
            &tx,
        );

        match rx.recv().expect("signature help result event") {
            UiEvent::Lsp(LspUiEvent::SignatureHelpResult { help, error, .. }) => {
                assert_eq!(help, None);
                assert_eq!(error.as_deref(), Some(INVALID_SIGNATURE_HELP_RESPONSE));
            }
            other => panic!("expected signature help result event, got {other:?}"),
        }
    }

    #[test]
    fn signature_help_result_reports_signature_tail_after_cap() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let mut signatures = (0..MAX_VALIDATED_SIGNATURES)
            .map(|idx| json!({ "label": format!("signature{idx}()") }))
            .collect::<Vec<_>>();
        signatures.push(json!({ "parameters": [{ "label": [0, 1] }] }));

        send_signature_help_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            12,
            &json!({
                "result": {
                    "signatures": signatures
                }
            }),
            &tx,
        );

        match rx.recv().expect("signature help result event") {
            UiEvent::Lsp(LspUiEvent::SignatureHelpResult { help, error, .. }) => {
                assert_eq!(help, None);
                assert_eq!(error.as_deref(), Some(INVALID_SIGNATURE_HELP_RESPONSE));
            }
            other => panic!("expected signature help result event, got {other:?}"),
        }
    }

    #[test]
    fn signature_help_result_sends_empty_help_for_null_success() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_signature_help_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            12,
            &json!({ "result": null }),
            &tx,
        );

        match rx.recv().expect("signature help result event") {
            UiEvent::Lsp(LspUiEvent::SignatureHelpResult { help, error, .. }) => {
                let help = help.expect("empty signature help");
                assert_eq!(help.signatures, Vec::new());
                assert_eq!(help.active_signature, 0);
                assert_eq!(help.active_parameter, None);
                assert_eq!(error, None);
            }
            other => panic!("expected signature help result event, got {other:?}"),
        }
    }

    #[test]
    fn signature_help_result_preserves_active_signature_and_parameter() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        send_signature_help_result(
            7,
            PathBuf::from("src/main.rs"),
            11,
            3,
            12,
            &json!({
                "result": {
                    "activeSignature": 1,
                    "signatures": [
                        {
                            "label": "push(value: T)",
                            "parameters": [{ "label": "value: T" }]
                        },
                        {
                            "label": "insert(index: usize, value: T)",
                            "activeParameter": 1,
                            "parameters": [
                                {
                                    "label": [7, 19],
                                    "documentation": "Target index"
                                },
                                {
                                    "label": "value: T"
                                }
                            ]
                        }
                    ]
                }
            }),
            &tx,
        );

        match rx.recv().expect("signature help result event") {
            UiEvent::Lsp(LspUiEvent::SignatureHelpResult {
                id,
                path,
                version,
                line,
                column,
                help,
                error,
            }) => {
                assert_eq!(id, 7);
                assert_eq!(path, PathBuf::from("src/main.rs"));
                assert_eq!(version, 11);
                assert_eq!(line, 3);
                assert_eq!(column, 13);
                let help = help.expect("signature help");
                assert_eq!(help.active_signature, 1);
                assert_eq!(help.active_parameter, Some(1));
                assert_eq!(help.signatures.len(), 2);
                assert_eq!(help.signatures[1].parameters[0].label, "index: usize");
                assert_eq!(
                    help.signatures[1].parameters[0].documentation.as_deref(),
                    Some("Target index")
                );
                assert_eq!(help.signatures[1].parameters[1].label, "value: T");
                assert_eq!(error, None);
            }
            other => panic!("expected signature help result event, got {other:?}"),
        }
    }
}
