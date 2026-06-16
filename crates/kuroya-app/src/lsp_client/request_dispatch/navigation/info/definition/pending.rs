use crate::lsp_client::pending::{PendingLspRequest, register_pending_request};
use kuroya_core::BufferId;
use std::{collections::HashMap, path::PathBuf};

pub(super) fn register_definition_request(
    request_id: u64,
    id: BufferId,
    path: PathBuf,
    version: u64,
    line: usize,
    character: usize,
    pending_requests: &mut HashMap<u64, PendingLspRequest>,
) {
    register_pending_request(
        pending_requests,
        request_id,
        PendingLspRequest::Definition {
            id,
            path,
            version,
            line,
            character,
        },
    );
}
