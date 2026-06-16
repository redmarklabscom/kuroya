mod result;

use crate::ui_event_channel::Sender;
use crate::ui_events::UiEvent;
use kuroya_core::BufferId;
use result::send_workspace_symbols_result;
use serde_json::Value;
use std::path::PathBuf;

pub(super) fn handle_workspace_symbols_response(
    id: BufferId,
    path: PathBuf,
    query: String,
    value: &Value,
    ui_tx: &Sender<UiEvent>,
) {
    send_workspace_symbols_result(id, path, query, value, ui_tx);
}
