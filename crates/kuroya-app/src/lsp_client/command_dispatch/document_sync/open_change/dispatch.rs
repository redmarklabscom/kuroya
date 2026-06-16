mod change;
mod open;

use crate::ui_event_channel::Sender;
use crate::{lsp_ui_events::LspUiEvent, ui_events::UiEvent};
use kuroya_core::BufferId;
use std::path::PathBuf;

pub(super) use change::dispatch_did_change;
pub(super) use open::dispatch_did_open;

fn send_buffer_synced(id: BufferId, path: PathBuf, version: u64, ui_tx: &Sender<UiEvent>) {
    let _ = crate::ui_event_channel::send_ui_event(ui_tx, buffer_synced_event(id, path, version));
}

fn buffer_synced_event(id: BufferId, path: PathBuf, version: u64) -> UiEvent {
    UiEvent::Lsp(LspUiEvent::BufferSynced { id, path, version })
}

#[cfg(test)]
mod tests {
    use super::buffer_synced_event;
    use crate::{lsp_ui_events::LspUiEvent, ui_events::UiEvent};
    use std::path::PathBuf;

    #[test]
    fn buffer_synced_event_preserves_buffer_path_and_version() {
        let path = PathBuf::from("src/main.rs");

        match buffer_synced_event(7, path.clone(), 3) {
            UiEvent::Lsp(LspUiEvent::BufferSynced {
                id: 7,
                path: event_path,
                version: 3,
            }) => assert_eq!(event_path, path),
            other => panic!("expected BufferSynced event, got {other:?}"),
        }
    }

    #[test]
    fn buffer_synced_event_preserves_internal_u64_version() {
        let path = PathBuf::from("src/main.rs");
        let version = i32::MAX as u64 + 1;

        match buffer_synced_event(7, path.clone(), version) {
            UiEvent::Lsp(LspUiEvent::BufferSynced {
                id: 7,
                path: event_path,
                version: event_version,
            }) => {
                assert_eq!(event_path, path);
                assert_eq!(event_version, version);
            }
            other => panic!("expected BufferSynced event, got {other:?}"),
        }
    }
}
