use super::super::wire::write_message;
use kuroya_core::LspWireMessage;
use tokio::process::{Child, ChildStdin};

pub(super) async fn handle_shutdown_command(writer: &mut ChildStdin, child: &mut Child) {
    let _ = write_message(writer, &LspWireMessage::shutdown(2).to_json()).await;
    let _ = write_message(writer, &LspWireMessage::exit().to_json()).await;
    let _ = child.kill().await;
}
