use super::ensure_workspace_child;
use crate::{
    KuroyaApp,
    explorer::{ExplorerEntryKind, ExplorerOperationResult},
    explorer_runtime::{explorer_operation_error_detail, explorer_operation_path_label},
    ui_events::UiEvent,
    workspace_trust::workspace_path_stays_within_root_lexically,
};
use std::{
    io,
    path::{Path, PathBuf},
};

impl KuroyaApp {
    pub(crate) fn spawn_create_file(&mut self, path: PathBuf) {
        let tx = self.tx.clone();
        let root = self.workspace.root.clone();
        let generation = self.workspace_event_generation;
        self.status = format!("Creating {}", explorer_operation_path_label(&path));
        self.runtime.spawn(async move {
            let result = async {
                ensure_create_target(&root, &path)?;
                if let Some(parent) = path.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                create_new_file(&path).await
            }
            .await;

            match result {
                Ok(()) => {
                    let _ = crate::ui_event_channel::send_ui_event(
                        &tx,
                        UiEvent::ExplorerOperationFinished {
                            root,
                            generation,
                            operation: ExplorerOperationResult::Created {
                                path,
                                kind: ExplorerEntryKind::File,
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
                            action: "create file",
                            path,
                            error: explorer_operation_error_detail(&error.to_string()),
                        },
                    );
                }
            }
        });
    }

    pub(crate) fn spawn_create_folder(&mut self, path: PathBuf) {
        let tx = self.tx.clone();
        let root = self.workspace.root.clone();
        let generation = self.workspace_event_generation;
        self.status = format!("Creating {}", explorer_operation_path_label(&path));
        self.runtime.spawn(async move {
            let result = async {
                ensure_create_target(&root, &path)?;
                if let Some(parent) = path.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                create_new_dir(&path).await
            }
            .await;

            match result {
                Ok(()) => {
                    let _ = crate::ui_event_channel::send_ui_event(
                        &tx,
                        UiEvent::ExplorerOperationFinished {
                            root,
                            generation,
                            operation: ExplorerOperationResult::Created {
                                path,
                                kind: ExplorerEntryKind::Folder,
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
                            action: "create folder",
                            path,
                            error: explorer_operation_error_detail(&error.to_string()),
                        },
                    );
                }
            }
        });
    }
}

fn ensure_create_target(root: &Path, path: &Path) -> io::Result<()> {
    ensure_workspace_child(root, path)?;
    if workspace_path_stays_within_root_lexically(root, path) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "explorer create target must stay within the workspace path",
        ))
    }
}

async fn create_new_file(path: &Path) -> io::Result<()> {
    if tokio::fs::try_exists(path).await? {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "explorer create target already exists",
        ));
    }
    match tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .await
    {
        Ok(_file) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "path already exists",
        )),
        Err(error) => Err(error),
    }
}

async fn create_new_dir(path: &Path) -> io::Result<()> {
    match tokio::fs::create_dir(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "path already exists",
        )),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::{create_new_dir, create_new_file, ensure_create_target};
    use std::{
        fs, io,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn create_target_rejects_parent_reentry_paths() {
        let root = PathBuf::from("workspace").join("current");
        let path = root.join("..").join("current").join("new.rs");

        let error = ensure_create_target(&root, &path).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn create_target_preserves_raw_pathbuf_text() {
        let root = PathBuf::from("workspace");
        let path = root.join("src").join("..").join("src").join("raw\nname.rs");
        let original = path.clone();

        ensure_create_target(&root, &path).unwrap();

        assert_eq!(path, original);
        assert!(path.as_os_str().to_string_lossy().contains('\n'));
    }

    #[tokio::test]
    async fn create_file_rejects_existing_file_without_overwrite() {
        let root = temp_workspace("existing-file");
        fs::create_dir_all(&root).unwrap();
        let path = root.join("main.rs");
        fs::write(&path, "original\n").unwrap();

        let error = create_new_file(&path).await.unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read_to_string(&path).unwrap(), "original\n");
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn create_file_rejects_existing_folder_conflict() {
        let root = temp_workspace("file-folder-conflict");
        let path = root.join("src");
        fs::create_dir_all(&path).unwrap();

        let error = create_new_file(&path).await.unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert!(path.is_dir());
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn create_dir_rejects_existing_file_conflict() {
        let root = temp_workspace("dir-file-conflict");
        fs::create_dir_all(&root).unwrap();
        let path = root.join("src");
        fs::write(&path, "not a directory\n").unwrap();

        let error = create_new_dir(&path).await.unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert!(path.is_file());
        assert_eq!(fs::read_to_string(&path).unwrap(), "not a directory\n");
        fs::remove_dir_all(root).unwrap();
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "kuroya-explorer-create-{name}-{}-{nanos}",
            std::process::id()
        ))
    }
}
