mod code_actions;
mod completion;
mod formatting;
mod signature;

use crate::ui_event_channel::Sender;
use crate::{lsp_client::pending::PendingLspRequest, ui_events::UiEvent};
use serde_json::Value;

pub(super) fn handle_edit_response(
    pending: PendingLspRequest,
    value: Value,
    ui_tx: &Sender<UiEvent>,
) {
    match pending {
        PendingLspRequest::Completion {
            id,
            path,
            version,
            line,
            character,
        } => {
            completion::handle_completion_response(
                id, path, version, line, character, &value, ui_tx,
            );
        }
        PendingLspRequest::ResolveCompletionItem {
            id,
            path,
            version,
            line,
            character,
            item,
            intent,
        } => {
            completion::handle_completion_item_resolve_response(
                id, path, version, line, character, *item, intent, &value, ui_tx,
            );
        }
        PendingLspRequest::SignatureHelp {
            id,
            path,
            version,
            line,
            character,
        } => {
            signature::handle_signature_help_response(
                id, path, version, line, character, &value, ui_tx,
            );
        }
        PendingLspRequest::Formatting {
            request_id,
            id,
            path,
            version,
        } => {
            formatting::handle_formatting_response(request_id, id, path, version, &value, ui_tx);
        }
        PendingLspRequest::CodeActions {
            id,
            path,
            version,
            line,
            character,
        } => {
            code_actions::handle_code_actions_response(
                id, path, version, line, character, &value, ui_tx,
            );
        }
        PendingLspRequest::ResolveCodeAction {
            id,
            path,
            version,
            line,
            character,
        } => {
            code_actions::handle_code_action_resolve_response(
                id, path, version, line, character, &value, ui_tx,
            );
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::handle_edit_response;
    use crate::lsp_client::pending::PendingLspRequest;
    use crossbeam_channel::TryRecvError;
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn edit_response_handler_ignores_misrouted_pending_request() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        handle_edit_response(
            PendingLspRequest::DocumentSymbols {
                id: 11,
                path: PathBuf::from("src/main.rs"),
                version: 5,
            },
            json!({ "result": [] }),
            &tx,
        );

        assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
    }
}
