mod open_change;
mod save_close;

use super::super::commands::LspClientCommand;
use crate::ui_event_channel::Sender;
use crate::ui_events::UiEvent;
use tokio::process::ChildStdin;

pub(super) async fn handle_document_sync_command(
    command: LspClientCommand,
    writer: &mut ChildStdin,
    ui_tx: &Sender<UiEvent>,
) -> bool {
    match command {
        command @ (LspClientCommand::DidOpen { .. } | LspClientCommand::DidChange { .. }) => {
            open_change::handle_open_change_command(command, writer, ui_tx).await
        }
        command @ (LspClientCommand::DidSave { .. } | LspClientCommand::DidClose { .. }) => {
            save_close::handle_save_close_command(command, writer).await
        }
        _ => true,
    }
}
