use super::send_buffer_synced;
use crate::ui_event_channel::Sender;
use crate::{
    lsp_client::wire::{lsp_version, write_message},
    ui_events::UiEvent,
};
use kuroya_core::{BufferId, LspWireMessage, TextSnapshot};
use std::path::PathBuf;
use tokio::process::ChildStdin;

pub(in crate::lsp_client::command_dispatch::document_sync::open_change) async fn dispatch_did_change(
    id: BufferId,
    path: PathBuf,
    version: u64,
    text: TextSnapshot,
    writer: &mut ChildStdin,
    ui_tx: &Sender<UiEvent>,
) -> bool {
    let wire_version = lsp_version(version);
    let text = text.text();
    let write_result = write_message(
        writer,
        &LspWireMessage::did_change(&path, wire_version, &text).to_json(),
    )
    .await;
    if write_result.is_ok() {
        send_buffer_synced(id, path, version, ui_tx);
        true
    } else {
        false
    }
}
