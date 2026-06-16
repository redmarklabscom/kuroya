mod dispatch;
mod pending;

use crate::lsp_client::pending::PendingLspRequest;
use dispatch::dispatch_inlay_hints;
use kuroya_core::BufferId;
use std::{collections::HashMap, path::PathBuf};
use tokio::process::ChildStdin;

pub(super) async fn dispatch_inlay_hints_request(
    id: BufferId,
    path: PathBuf,
    version: u64,
    end_line: usize,
    end_character: usize,
    writer: &mut ChildStdin,
    next_request_id: &mut u64,
    pending_requests: &mut HashMap<u64, PendingLspRequest>,
) -> bool {
    dispatch_inlay_hints(
        id,
        path,
        version,
        end_line,
        end_character,
        writer,
        next_request_id,
        pending_requests,
    )
    .await
}
