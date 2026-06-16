mod code_lenses;
mod document;
mod folding;
mod inlay_hints;
mod semantic_tokens;
mod workspace;

use crate::ui_event_channel::Sender;
use crate::{lsp_client::pending::PendingLspRequest, ui_events::UiEvent};
use serde_json::Value;

pub(super) fn handle_symbol_response(
    pending: PendingLspRequest,
    value: Value,
    ui_tx: &Sender<UiEvent>,
) {
    match pending {
        PendingLspRequest::DocumentSymbols { id, path, version } => {
            document::handle_document_symbols_response(id, path, version, &value, ui_tx);
        }
        PendingLspRequest::FoldingRanges { id, path, version } => {
            folding::handle_folding_ranges_response(id, path, version, &value, ui_tx);
        }
        PendingLspRequest::InlayHints {
            id, path, version, ..
        } => {
            inlay_hints::handle_inlay_hints_response(id, path, version, &value, ui_tx);
        }
        PendingLspRequest::CodeLenses { id, path, version } => {
            code_lenses::handle_code_lenses_response(id, path, version, &value, ui_tx);
        }
        PendingLspRequest::ResolveCodeLens { id, path, version } => {
            code_lenses::handle_code_lens_resolve_response(id, path, version, &value, ui_tx);
        }
        PendingLspRequest::ExecuteCommand {
            id,
            path,
            version,
            title,
            command,
        } => {
            code_lenses::handle_execute_command_response(
                id, path, version, title, command, &value, ui_tx,
            );
        }
        PendingLspRequest::SemanticTokens { id, path, version } => {
            semantic_tokens::handle_semantic_tokens_response(id, path, version, &value, ui_tx);
        }
        PendingLspRequest::WorkspaceSymbols { id, path, query } => {
            workspace::handle_workspace_symbols_response(id, path, query, &value, ui_tx);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::handle_symbol_response;
    use crate::lsp_client::pending::PendingLspRequest;
    use crossbeam_channel::TryRecvError;
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn symbol_response_handler_ignores_misrouted_pending_request() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        handle_symbol_response(
            PendingLspRequest::Hover {
                id: 7,
                path: PathBuf::from("src/main.rs"),
                version: 3,
                line: 4,
                character: 2,
            },
            json!({ "result": [] }),
            &tx,
        );

        assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
    }
}
