use crate::{
    KuroyaApp,
    explorer::{ExplorerEntryKind, ExplorerOperationResult},
    explorer_runtime::{explorer_operation_error_detail, explorer_operation_path_label},
    ui_events::UiEvent,
    workspace_trust::{trusted_workspace_paths_match, workspace_path_contains_lexically},
};
use std::{
    io,
    path::{Path, PathBuf},
};

mod create;

impl KuroyaApp {
    pub(crate) fn spawn_rename_path(
        &mut self,
        old_path: PathBuf,
        new_path: PathBuf,
        kind: ExplorerEntryKind,
    ) {
        let tx = self.tx.clone();
        let root = self.workspace.root.clone();
        let generation = self.workspace_event_generation;
        self.status = format!("Renaming {}", explorer_operation_path_label(&old_path));
        self.runtime.spawn(async move {
            let result = async {
                ensure_workspace_child(&root, &old_path)?;
                ensure_workspace_child(&root, &new_path)?;
                ensure_existing_kind(&old_path, kind).await?;
                if tokio::fs::try_exists(&new_path).await? {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::AlreadyExists,
                        "target already exists",
                    ));
                }
                if let Some(parent) = new_path.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                tokio::fs::rename(&old_path, &new_path).await
            }
            .await;

            match result {
                Ok(()) => {
                    let _ = crate::ui_event_channel::send_ui_event(
                        &tx,
                        UiEvent::ExplorerOperationFinished {
                            root,
                            generation,
                            operation: ExplorerOperationResult::Renamed {
                                old_path,
                                new_path,
                                kind,
                            },
                        },
                    );
                }
                Err(error) => {
                    let _ = crate::ui_event_channel::send_ui_event(
                        &tx,
                        UiEvent::ExplorerOperationFailed {
                            root,
                            generation,
                            action: "rename",
                            path: old_path,
                            error: explorer_operation_error_detail(&error.to_string()),
                        },
                    );
                }
            }
        });
    }

    pub(crate) fn spawn_delete_path(&mut self, path: PathBuf, kind: ExplorerEntryKind) {
        let tx = self.tx.clone();
        let root = self.workspace.root.clone();
        let generation = self.workspace_event_generation;
        self.status = format!("Deleting {}", explorer_operation_path_label(&path));
        self.runtime.spawn(async move {
            let result = async {
                ensure_workspace_child(&root, &path)?;
                ensure_existing_kind(&path, kind).await?;
                match kind {
                    ExplorerEntryKind::File => tokio::fs::remove_file(&path).await,
                    ExplorerEntryKind::Folder => tokio::fs::remove_dir_all(&path).await,
                }
            }
            .await;

            match result {
                Ok(()) => {
                    let _ = crate::ui_event_channel::send_ui_event(
                        &tx,
                        UiEvent::ExplorerOperationFinished {
                            root,
                            generation,
                            operation: ExplorerOperationResult::Deleted { path, kind },
                        },
                    );
                }
                Err(error) => {
                    let _ = crate::ui_event_channel::send_ui_event(
                        &tx,
                        UiEvent::ExplorerOperationFailed {
                            root,
                            generation,
                            action: "delete",
                            path,
                            error: explorer_operation_error_detail(&error.to_string()),
                        },
                    );
                }
            }
        });
    }
}

fn ensure_workspace_child(root: &Path, path: &Path) -> io::Result<()> {
    if explorer_fs_path_is_workspace_child(root, path) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "explorer operation must stay inside the workspace",
        ))
    }
}

async fn ensure_existing_kind(path: &Path, kind: ExplorerEntryKind) -> io::Result<()> {
    let metadata = tokio::fs::metadata(path).await?;
    if explorer_fs_metadata_matches_kind(&metadata, kind) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            match kind {
                ExplorerEntryKind::File => "explorer target is no longer a file",
                ExplorerEntryKind::Folder => "explorer target is no longer a folder",
            },
        ))
    }
}

pub(crate) fn explorer_fs_metadata_matches_kind(
    metadata: &std::fs::Metadata,
    kind: ExplorerEntryKind,
) -> bool {
    match kind {
        ExplorerEntryKind::File => metadata.is_file(),
        ExplorerEntryKind::Folder => metadata.is_dir(),
    }
}

pub(crate) fn explorer_fs_path_is_workspace_child(root: &Path, path: &Path) -> bool {
    workspace_path_contains_lexically(root, path) && !trusted_workspace_paths_match(root, path)
}

#[cfg(test)]
mod tests {
    use super::explorer_fs_path_is_workspace_child;
    use std::path::PathBuf;

    #[test]
    fn explorer_fs_path_checks_preserve_raw_pathbuf_text() {
        let root = PathBuf::from("workspace");
        let path = root.join("raw\n\u{202e}name.rs");
        let original = path.clone();

        assert!(explorer_fs_path_is_workspace_child(&root, &path));
        assert_eq!(path, original);
        assert!(path.as_os_str().to_string_lossy().contains('\n'));
        assert!(path.as_os_str().to_string_lossy().contains('\u{202e}'));
    }

    #[test]
    fn explorer_fs_path_checks_reject_equivalent_root_and_escape_paths() {
        let root = PathBuf::from("workspace");

        assert!(!explorer_fs_path_is_workspace_child(
            &root,
            &root.join("src").join("..")
        ));
        assert!(!explorer_fs_path_is_workspace_child(
            &root,
            &root.join("..").join("outside")
        ));
        assert!(explorer_fs_path_is_workspace_child(
            &root,
            &root.join("src").join("..").join("src").join("main.rs")
        ));
    }
}
