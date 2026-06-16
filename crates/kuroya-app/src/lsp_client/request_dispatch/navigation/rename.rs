mod dispatch;
mod pending;

use crate::lsp_client::{commands::LspClientCommand, pending::PendingLspRequest};
use dispatch::dispatch_rename;
use std::collections::HashMap;
use tokio::process::ChildStdin;

pub(super) async fn handle_rename_request_command(
    command: LspClientCommand,
    writer: &mut ChildStdin,
    next_request_id: &mut u64,
    pending_requests: &mut HashMap<u64, PendingLspRequest>,
) -> bool {
    let LspClientCommand::Rename {
        id,
        path,
        version,
        line,
        character,
        new_name,
    } = command
    else {
        return true;
    };

    dispatch_rename(
        id,
        path,
        version,
        line,
        character,
        new_name,
        writer,
        next_request_id,
        pending_requests,
    )
    .await
}
