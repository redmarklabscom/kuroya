use crate::ui_event_channel::Sender;
use crate::{lsp_ui_events::LspUiEvent, ui_events::UiEvent};
use kuroya_core::parse_publish_diagnostics;
use serde_json::Value;
use std::path::Path;

pub(super) fn send_publish_diagnostics(
    value: &Value,
    language: &str,
    root: &Path,
    generation: u64,
    ui_tx: &Sender<UiEvent>,
) -> bool {
    let Some((path, version, diagnostics)) = parse_publish_diagnostics(value) else {
        return false;
    };

    let _ = crate::ui_event_channel::send_ui_event(
        ui_tx,
        UiEvent::Lsp(LspUiEvent::Diagnostics {
            language: language.to_owned(),
            root: root.to_path_buf(),
            generation,
            path,
            version,
            diagnostics,
        }),
    );
    true
}
