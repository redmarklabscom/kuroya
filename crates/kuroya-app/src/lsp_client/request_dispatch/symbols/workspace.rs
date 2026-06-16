mod dispatch;
mod pending;

use crate::lsp_client::pending::PendingLspRequest;
use dispatch::dispatch_workspace_symbols;
use kuroya_core::BufferId;
use std::{collections::HashMap, path::PathBuf};
use tokio::process::ChildStdin;

pub(super) async fn dispatch_workspace_symbols_request(
    id: BufferId,
    path: PathBuf,
    query: String,
    writer: &mut ChildStdin,
    next_request_id: &mut u64,
    pending_requests: &mut HashMap<u64, PendingLspRequest>,
) -> bool {
    dispatch_workspace_symbols(id, path, query, writer, next_request_id, pending_requests).await
}
