mod actions;
mod family;
mod position;

use crate::lsp_client::{commands::LspClientCommand, pending::PendingLspRequest};
use family::{EditRequestFamily, edit_request_family};
use std::collections::HashMap;
use tokio::process::ChildStdin;

pub(super) async fn handle_edit_request_command(
    command: LspClientCommand,
    writer: &mut ChildStdin,
    next_request_id: &mut u64,
    pending_requests: &mut HashMap<u64, PendingLspRequest>,
) -> bool {
    let Some(family) = edit_request_family(&command) else {
        return true;
    };

    match family {
        EditRequestFamily::Position => {
            position::handle_position_edit_request_command(
                command,
                writer,
                next_request_id,
                pending_requests,
            )
            .await
        }
        EditRequestFamily::Actions => {
            actions::handle_action_edit_request_command(
                command,
                writer,
                next_request_id,
                pending_requests,
            )
            .await
        }
    }
}
