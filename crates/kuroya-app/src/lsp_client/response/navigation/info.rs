mod definition;
mod document_highlights;
mod hover;

use crate::ui_event_channel::Sender;
use crate::{lsp_client::pending::PendingLspRequest, ui_events::UiEvent};
use serde_json::Value;

pub(super) fn handle_info_navigation_response(
    pending: PendingLspRequest,
    value: Value,
    ui_tx: &Sender<UiEvent>,
) {
    match pending {
        PendingLspRequest::Hover {
            id,
            path,
            version,
            line,
            character,
        } => {
            hover::handle_hover_response(id, path, version, line, character, &value, ui_tx);
        }
        PendingLspRequest::DocumentHighlights {
            id,
            path,
            version,
            line,
            character,
        } => {
            document_highlights::handle_document_highlights_response(
                id, path, version, line, character, &value, ui_tx,
            );
        }
        PendingLspRequest::Definition {
            id,
            path,
            version,
            line,
            character,
        } => {
            definition::handle_definition_response(
                id, path, version, line, character, &value, ui_tx,
            );
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::handle_info_navigation_response;
    use crate::lsp_client::pending::PendingLspRequest;
    use crossbeam_channel::TryRecvError;
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn info_navigation_response_handler_ignores_misrouted_pending_request() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        handle_info_navigation_response(
            PendingLspRequest::Formatting {
                request_id: 13,
                id: 13,
                path: PathBuf::from("src/main.rs"),
                version: 8,
            },
            json!({ "result": null }),
            &tx,
        );

        assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
    }
}
