mod result;

use crate::ui_event_channel::Sender;
use crate::ui_events::UiEvent;
use kuroya_core::BufferId;
use result::{send_code_lens_resolve_result, send_code_lenses_result, send_execute_command_result};
use serde_json::Value;
use std::path::PathBuf;

pub(super) fn handle_code_lenses_response(
    id: BufferId,
    path: PathBuf,
    version: u64,
    value: &Value,
    ui_tx: &Sender<UiEvent>,
) {
    send_code_lenses_result(id, path, version, value, ui_tx);
}

pub(super) fn handle_code_lens_resolve_response(
    id: BufferId,
    path: PathBuf,
    version: u64,
    value: &Value,
    ui_tx: &Sender<UiEvent>,
) {
    send_code_lens_resolve_result(id, path, version, value, ui_tx);
}

pub(super) fn handle_execute_command_response(
    id: BufferId,
    path: PathBuf,
    version: u64,
    title: String,
    command: String,
    value: &Value,
    ui_tx: &Sender<UiEvent>,
) {
    send_execute_command_result(id, path, version, title, command, value, ui_tx);
}
