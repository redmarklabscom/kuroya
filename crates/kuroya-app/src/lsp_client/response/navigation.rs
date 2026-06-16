use crate::ui_event_channel::Sender;
use crate::{lsp_client::pending::PendingLspRequest, ui_events::UiEvent};
use call_hierarchy::handle_call_hierarchy_response;
use info::handle_info_navigation_response;
use reference_rename::handle_reference_rename_response;
use serde_json::Value;

mod call_hierarchy;
mod info;
mod reference_rename;
mod type_hierarchy;

pub(super) fn handle_navigation_response(
    pending: PendingLspRequest,
    value: Value,
    ui_tx: &Sender<UiEvent>,
) {
    match pending {
        pending @ (PendingLspRequest::Hover { .. }
        | PendingLspRequest::DocumentHighlights { .. }
        | PendingLspRequest::Definition { .. }) => {
            handle_info_navigation_response(pending, value, ui_tx);
        }
        pending @ (PendingLspRequest::PrepareCallHierarchy { .. }
        | PendingLspRequest::CallHierarchyIncoming { .. }
        | PendingLspRequest::CallHierarchyOutgoing { .. }) => {
            handle_call_hierarchy_response(pending, value, ui_tx);
        }
        pending @ (PendingLspRequest::PrepareTypeHierarchy { .. }
        | PendingLspRequest::TypeHierarchySupertypes { .. }
        | PendingLspRequest::TypeHierarchySubtypes { .. }) => {
            type_hierarchy::handle_type_hierarchy_response(pending, value, ui_tx);
        }
        pending @ (PendingLspRequest::References { .. } | PendingLspRequest::Rename { .. }) => {
            handle_reference_rename_response(pending, value, ui_tx);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::handle_navigation_response;
    use crate::lsp_client::pending::PendingLspRequest;
    use crossbeam_channel::TryRecvError;
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn navigation_response_handler_ignores_misrouted_pending_request() {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        handle_navigation_response(
            PendingLspRequest::Completion {
                id: 31,
                path: PathBuf::from("src/main.rs"),
                version: 9,
                line: 3,
                character: 5,
            },
            json!({ "result": [] }),
            &tx,
        );

        assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
    }
}
