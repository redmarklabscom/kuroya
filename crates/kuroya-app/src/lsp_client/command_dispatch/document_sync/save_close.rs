use crate::lsp_client::{commands::LspClientCommand, wire::write_message};
use kuroya_core::LspWireMessage;
use tokio::process::ChildStdin;

pub(super) async fn handle_save_close_command(
    command: LspClientCommand,
    writer: &mut ChildStdin,
) -> bool {
    let message = match command {
        LspClientCommand::DidSave { path } => LspWireMessage::did_save(&path).to_json(),
        LspClientCommand::DidClose { path } => LspWireMessage::did_close(&path).to_json(),
        _ => return true,
    };

    write_message(writer, &message).await.is_ok()
}
