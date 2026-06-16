mod result;

use crate::ui_event_channel::Sender;
use crate::ui_events::UiEvent;
use kuroya_core::BufferId;
use result::send_folding_ranges_result;
use serde_json::Value;
use std::path::PathBuf;

pub(super) fn handle_folding_ranges_response(
    id: BufferId,
    path: PathBuf,
    version: u64,
    value: &Value,
    ui_tx: &Sender<UiEvent>,
) {
    send_folding_ranges_result(id, path, version, value, ui_tx);
}
