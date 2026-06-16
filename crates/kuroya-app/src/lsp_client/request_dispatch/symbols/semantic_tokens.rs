mod dispatch;
mod pending;

use crate::lsp_client::pending::PendingLspRequest;
use dispatch::dispatch_semantic_tokens;
use kuroya_core::BufferId;
use std::{collections::HashMap, path::PathBuf};
use tokio::process::ChildStdin;

pub(super) async fn dispatch_semantic_tokens_request(
    id: BufferId,
    path: PathBuf,
    version: u64,
    writer: &mut ChildStdin,
    next_request_id: &mut u64,
    pending_requests: &mut HashMap<u64, PendingLspRequest>,
) -> bool {
    dispatch_semantic_tokens(id, path, version, writer, next_request_id, pending_requests).await
}
