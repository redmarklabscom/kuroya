mod result;

use crate::ui_event_channel::Sender;
use crate::ui_events::UiEvent;
use kuroya_core::BufferId;
use result::{send_code_action_resolve_result, send_code_actions_result};
use serde_json::Value;
use std::path::PathBuf;

pub(super) fn handle_code_actions_response(
    id: BufferId,
    path: PathBuf,
    version: u64,
    line: usize,
    character: usize,
    value: &Value,
    ui_tx: &Sender<UiEvent>,
) {
    send_code_actions_result(id, path, version, line, character, value, ui_tx);
}

pub(super) fn handle_code_action_resolve_response(
    id: BufferId,
    path: PathBuf,
    version: u64,
    line: usize,
    character: usize,
    value: &Value,
    ui_tx: &Sender<UiEvent>,
) {
    send_code_action_resolve_result(id, path, version, line, character, value, ui_tx);
}
