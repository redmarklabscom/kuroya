mod dispatch;
mod pending;

use crate::lsp_client::{commands::LspClientCommand, pending::PendingLspRequest};
use dispatch::dispatch_references;
use std::collections::HashMap;
use tokio::process::ChildStdin;

pub(super) async fn handle_references_request_command(
    command: LspClientCommand,
    writer: &mut ChildStdin,
    next_request_id: &mut u64,
    pending_requests: &mut HashMap<u64, PendingLspRequest>,
) -> bool {
    let LspClientCommand::References {
        id,
        path,
        version,
        line,
        character,
        include_declaration,
    } = command
    else {
        return true;
    };

    dispatch_references(
        id,
        path,
        version,
        line,
        character,
        include_declaration,
        writer,
        next_request_id,
        pending_requests,
    )
    .await
}
