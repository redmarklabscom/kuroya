use std::path::PathBuf;

mod path_helpers;

pub(crate) use path_helpers::{
    explorer_kind_for_path, path_matches_kind, retarget_path_prefix, workspace_child_path,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExplorerEntryKind {
    File,
    Folder,
}

#[derive(Debug, Clone)]
pub(crate) enum ExplorerFileAction {
    Rename {
        path: PathBuf,
        kind: ExplorerEntryKind,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct ExplorerDeleteTarget {
    pub(crate) path: PathBuf,
    pub(crate) kind: ExplorerEntryKind,
}

#[derive(Debug, Clone)]
pub(crate) enum ExplorerOperationResult {
    Created {
        path: PathBuf,
        kind: ExplorerEntryKind,
    },
    Renamed {
        old_path: PathBuf,
        new_path: PathBuf,
        kind: ExplorerEntryKind,
    },
    Deleted {
        path: PathBuf,
        kind: ExplorerEntryKind,
    },
}
