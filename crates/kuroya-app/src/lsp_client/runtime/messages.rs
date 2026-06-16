mod diagnostics;
mod progress;
mod requests;
mod responses;

use super::super::pending::PendingLspRequest;
use crate::ui_event_channel::Sender;
use crate::ui_events::UiEvent;
use diagnostics::send_publish_diagnostics;
pub(super) use progress::forget_created_work_done_progress_tokens_for_server;
use progress::{acknowledge_work_done_progress_create, send_work_done_progress};
use requests::handle_server_request;
use responses::handle_response_message;
use serde_json::Value;
use std::{collections::HashMap, path::Path};
use tokio::process::ChildStdin;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LspServerMessageOutcome {
    Continue,
    FatalWriteFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LspServerMessageHandlerOutcome {
    Unhandled,
    Handled,
    FatalWriteFailure,
}

pub(super) async fn handle_lsp_server_message(
    value: Value,
    language: &str,
    root: &Path,
    generation: u64,
    pending_requests: &mut HashMap<u64, PendingLspRequest>,
    ui_tx: &Sender<UiEvent>,
    writer: &mut ChildStdin,
) -> LspServerMessageOutcome {
    if value.get("method").is_none() {
        handle_response_message(value, language, root, generation, pending_requests, ui_tx);
        return LspServerMessageOutcome::Continue;
    }

    if send_publish_diagnostics(&value, language, root, generation, ui_tx) {
        return LspServerMessageOutcome::Continue;
    }
    if send_work_done_progress(&value, language, root, generation, ui_tx) {
        return LspServerMessageOutcome::Continue;
    }
    match acknowledge_work_done_progress_create(&value, language, root, generation, ui_tx, writer)
        .await
    {
        LspServerMessageHandlerOutcome::Handled => return LspServerMessageOutcome::Continue,
        LspServerMessageHandlerOutcome::FatalWriteFailure => {
            return LspServerMessageOutcome::FatalWriteFailure;
        }
        LspServerMessageHandlerOutcome::Unhandled => {}
    }
    match handle_server_request(&value, language, root, generation, ui_tx, writer).await {
        LspServerMessageHandlerOutcome::Handled => return LspServerMessageOutcome::Continue,
        LspServerMessageHandlerOutcome::FatalWriteFailure => {
            return LspServerMessageOutcome::FatalWriteFailure;
        }
        LspServerMessageHandlerOutcome::Unhandled => {}
    }

    handle_response_message(value, language, root, generation, pending_requests, ui_tx);
    LspServerMessageOutcome::Continue
}
