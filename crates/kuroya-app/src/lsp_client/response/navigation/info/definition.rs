mod result;

use crate::ui_event_channel::Sender;
use crate::ui_events::UiEvent;
use kuroya_core::BufferId;
use result::send_definition_result;
use serde_json::Value;
use std::path::PathBuf;

pub(super) fn handle_definition_response(
    id: BufferId,
    path: PathBuf,
    version: u64,
    line: usize,
    character: usize,
    value: &Value,
    ui_tx: &Sender<UiEvent>,
) {
    send_definition_result(id, path, version, line, character, value, ui_tx);
}
