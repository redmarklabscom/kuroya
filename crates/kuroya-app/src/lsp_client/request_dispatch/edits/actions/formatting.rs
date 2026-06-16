mod dispatch;
mod pending;

use crate::lsp_client::{commands::LspClientCommand, pending::PendingLspRequest};
use dispatch::dispatch_formatting;
use std::collections::HashMap;
use tokio::process::ChildStdin;

pub(super) async fn handle_formatting_request_command(
    command: LspClientCommand,
    writer: &mut ChildStdin,
    next_request_id: &mut u64,
    pending_requests: &mut HashMap<u64, PendingLspRequest>,
) -> bool {
    let LspClientCommand::Formatting {
        request_id: formatting_request_id,
        id,
        path,
        version,
        tab_size,
        insert_spaces,
    } = command
    else {
        return true;
    };

    dispatch_formatting(
        formatting_request_id,
        id,
        path,
        version,
        tab_size,
        insert_spaces,
        writer,
        next_request_id,
        pending_requests,
    )
    .await
}
