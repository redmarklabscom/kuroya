mod result;

use crate::{
    lsp_completion_resolve::CompletionResolveIntent, ui_event_channel::Sender, ui_events::UiEvent,
};
use kuroya_core::{BufferId, LspCompletionItem};
use result::{send_completion_item_resolve_result, send_completion_result};
use serde_json::Value;
use std::path::PathBuf;

pub(super) fn handle_completion_response(
    id: BufferId,
    path: PathBuf,
    version: u64,
    line: usize,
    character: usize,
    value: &Value,
    ui_tx: &Sender<UiEvent>,
) {
    send_completion_result(id, path, version, line, character, value, ui_tx);
}

pub(super) fn handle_completion_item_resolve_response(
    id: BufferId,
    path: PathBuf,
    version: u64,
    line: usize,
    character: usize,
    item: LspCompletionItem,
    intent: CompletionResolveIntent,
    value: &Value,
    ui_tx: &Sender<UiEvent>,
) {
    send_completion_item_resolve_result(
        id, path, version, line, character, item, intent, value, ui_tx,
    );
}
