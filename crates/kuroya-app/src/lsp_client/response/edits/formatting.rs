mod result;

use crate::ui_event_channel::Sender;
use crate::ui_events::UiEvent;
use kuroya_core::BufferId;
use result::send_formatting_result;
use serde_json::Value;
use std::path::PathBuf;

pub(super) fn handle_formatting_response(
    request_id: u64,
    id: BufferId,
    path: PathBuf,
    version: u64,
    value: &Value,
    ui_tx: &Sender<UiEvent>,
) {
    send_formatting_result(request_id, id, path, version, value, ui_tx);
}
