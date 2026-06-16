use kuroya_core::{BufferId, GitChangeStage, GitLineChangeKind};
use std::{collections::BTreeMap, path::PathBuf};

pub(crate) const GIT_DIFF_MAX_BYTES: usize = 3 * 1024 * 1024;

#[derive(Debug, Clone)]
pub(crate) struct DiffCacheEntry {
    pub(crate) version: u64,
    pub(crate) ignore_trim_whitespace: bool,
    pub(crate) lines: BTreeMap<usize, GitLineChangeKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct DiffCacheRequestKey {
    pub(crate) root: PathBuf,
    pub(crate) buffer_id: BufferId,
    pub(crate) path: PathBuf,
    pub(crate) version: u64,
    pub(crate) ignore_trim_whitespace: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiffBufferSource {
    pub(crate) path: PathBuf,
    pub(crate) base_path: Option<PathBuf>,
    pub(crate) hunk_stage: Option<GitChangeStage>,
    pub(crate) saved_buffer_id: Option<BufferId>,
}
