use crate::{
    KuroyaApp,
    file_io::write_text_atomic,
    lsp_edits::apply_lsp_edits_to_text,
    lsp_lifecycle::LSP_DISK_EDIT_MAX_BYTES,
    lsp_ui_events::{LspUiEvent, LspWorkspaceApplyEditDiskResponse},
    path_display::{display_error_label_cow, display_path_label_cow},
    ui_events::UiEvent,
    workspace_trust::{
        trusted_workspace_paths_match, workspace_path_contains_lexically,
        workspace_path_stays_within_root_lexically,
    },
};
use kuroya_core::{LspTextEdit, LspWorkspaceDocumentChange, LspWorkspaceResourceOperation};
use std::{
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub(crate) struct LspDiskTextEditPlan {
    path: PathBuf,
    edits: Vec<LspTextEdit>,
    expected: Option<LspDiskEditFileSignature>,
}

impl LspDiskTextEditPlan {
    pub(crate) fn capture(root: &Path, path: PathBuf, edits: Vec<LspTextEdit>) -> Self {
        let expected = lsp_disk_edit_file_signature(root, &path).ok();
        Self {
            path,
            edits,
            expected,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LspDiskEditFileSignature {
    len: u64,
    modified_nanos: Option<u128>,
}

impl LspDiskEditFileSignature {
    fn from_metadata(metadata: &std::fs::Metadata) -> Self {
        Self {
            len: metadata.len(),
            modified_nanos: metadata_modified_nanos(metadata),
        }
    }
}

impl KuroyaApp {
    pub(crate) fn spawn_lsp_disk_edits(
        &mut self,
        edits: Vec<LspDiskTextEditPlan>,
        apply_edit_response: Option<LspWorkspaceApplyEditDiskResponse>,
    ) {
        let tx = self.tx.clone();
        let root = self.workspace.root.clone();
        let generation = self.workspace_event_generation;
        self.runtime.spawn_blocking(move || {
            let mut changed = 0;
            let mut failed = Vec::new();

            {
                let mut validator = LspDiskEditPathValidator::new(&root);
                for plan in edits {
                    match apply_lsp_disk_text_edit_plan_with_validator(&mut validator, &plan) {
                        Ok(()) => changed += 1,
                        Err(error) => {
                            failed.push((plan.path, error));
                        }
                    }
                }
            }

            let _ = crate::ui_event_channel::send_ui_event(
                &tx,
                UiEvent::Lsp(LspUiEvent::WorkspaceEditFilesApplied {
                    root,
                    generation,
                    changed,
                    failed,
                    apply_edit_response,
                }),
            );
        });
    }

    pub(crate) fn spawn_lsp_workspace_document_changes(
        &mut self,
        changes: Vec<LspWorkspaceDocumentChange>,
        apply_edit_response: Option<LspWorkspaceApplyEditDiskResponse>,
    ) {
        let tx = self.tx.clone();
        let root = self.workspace.root.clone();
        let generation = self.workspace_event_generation;
        self.runtime.spawn_blocking(move || {
            let mut changed = 0;
            let mut failed = Vec::new();

            {
                let mut validator = LspDiskEditPathValidator::new(&root);
                for change in changes {
                    let path = workspace_document_change_primary_path(&change);
                    match apply_lsp_workspace_document_change_with_validator(&mut validator, change)
                    {
                        Ok(true) => changed += 1,
                        Ok(false) => {}
                        Err(error) => {
                            failed.push((path, error));
                            break;
                        }
                    }
                }
            }

            let _ = crate::ui_event_channel::send_ui_event(
                &tx,
                UiEvent::Lsp(LspUiEvent::WorkspaceEditFilesApplied {
                    root,
                    generation,
                    changed,
                    failed,
                    apply_edit_response,
                }),
            );
        });
    }
}

#[cfg(test)]
fn apply_lsp_disk_text_edits(
    root: &Path,
    path: &Path,
    edits: &[LspTextEdit],
) -> Result<(), String> {
    let mut validator = LspDiskEditPathValidator::new(root);
    let expected = workspace_text_edit_file_signature_for_read(&mut validator, path)?;
    let text = read_lsp_disk_edit_text_with_known_len(path, expected.len)
        .map_err(|error| display_error_label_cow(&error).into_owned())?;
    let text = apply_lsp_edits_to_text(path, text, edits)
        .ok_or_else(|| "invalid LSP edit range".to_owned())?;
    validator.validate_lsp_disk_edit_file_signature(path, &expected)?;
    write_text_atomic(path, &text).map_err(display_lsp_disk_edit_error)
}

#[cfg(test)]
fn apply_lsp_disk_text_edit_plan(root: &Path, plan: LspDiskTextEditPlan) -> Result<(), String> {
    let mut validator = LspDiskEditPathValidator::new(root);
    apply_lsp_disk_text_edit_plan_with_validator(&mut validator, &plan)
}

fn apply_lsp_disk_text_edit_plan_with_validator(
    validator: &mut LspDiskEditPathValidator<'_>,
    plan: &LspDiskTextEditPlan,
) -> Result<(), String> {
    let expected = if let Some(expected) = plan.expected.as_ref() {
        validator.validate_lsp_disk_edit_file_signature(&plan.path, expected)?;
        expected.clone()
    } else {
        validator.lsp_disk_edit_file_signature(&plan.path)?
    };
    let text = read_lsp_disk_edit_text_with_known_len(&plan.path, expected.len)
        .map_err(|error| display_error_label_cow(&error).into_owned())?;
    let text = apply_lsp_edits_to_text(&plan.path, text, &plan.edits)
        .ok_or_else(|| "invalid LSP edit range".to_owned())?;
    validator.validate_lsp_disk_edit_file_signature(&plan.path, &expected)?;
    write_text_atomic(&plan.path, &text).map_err(display_lsp_disk_edit_error)
}

fn workspace_document_change_primary_path(change: &LspWorkspaceDocumentChange) -> PathBuf {
    match change {
        LspWorkspaceDocumentChange::TextEdit { path, .. } => path.clone(),
        LspWorkspaceDocumentChange::Resource(LspWorkspaceResourceOperation::CreateFile {
            path,
            ..
        }) => path.clone(),
        LspWorkspaceDocumentChange::Resource(LspWorkspaceResourceOperation::RenameFile {
            old_path,
            ..
        }) => old_path.clone(),
        LspWorkspaceDocumentChange::Resource(LspWorkspaceResourceOperation::DeleteFile {
            path,
            ..
        }) => path.clone(),
    }
}

#[cfg(test)]
fn apply_lsp_workspace_document_change(
    root: &Path,
    change: LspWorkspaceDocumentChange,
) -> Result<bool, String> {
    let mut validator = LspDiskEditPathValidator::new(root);
    apply_lsp_workspace_document_change_with_validator(&mut validator, change)
}

fn apply_lsp_workspace_document_change_with_validator(
    validator: &mut LspDiskEditPathValidator<'_>,
    change: LspWorkspaceDocumentChange,
) -> Result<bool, String> {
    match change {
        LspWorkspaceDocumentChange::TextEdit { path, edits, .. } => {
            if edits.is_empty() {
                validator.validate_existing_workspace_path(&path, "edit")?;
                return Ok(false);
            }
            let expected = workspace_text_edit_file_signature_for_read(validator, &path)?;
            let text = read_lsp_disk_edit_text_with_known_len(&path, expected.len)
                .map_err(|error| display_error_label_cow(&error).into_owned())?;
            let text = apply_lsp_edits_to_text(&path, text, &edits)
                .ok_or_else(|| "invalid LSP edit range".to_owned())?;
            validator.validate_lsp_disk_edit_file_signature(&path, &expected)?;
            write_text_atomic(&path, &text).map_err(display_lsp_disk_edit_error)?;
            Ok(true)
        }
        LspWorkspaceDocumentChange::Resource(LspWorkspaceResourceOperation::CreateFile {
            path,
            overwrite,
            ignore_if_exists,
        }) => create_lsp_workspace_file(validator, path, overwrite, ignore_if_exists),
        LspWorkspaceDocumentChange::Resource(LspWorkspaceResourceOperation::RenameFile {
            old_path,
            new_path,
            overwrite,
            ignore_if_exists,
        }) => rename_lsp_workspace_file(validator, old_path, new_path, overwrite, ignore_if_exists),
        LspWorkspaceDocumentChange::Resource(LspWorkspaceResourceOperation::DeleteFile {
            path,
            recursive,
            ignore_if_not_exists,
        }) => delete_lsp_workspace_file(validator, path, recursive, ignore_if_not_exists),
    }
}

fn workspace_text_edit_file_signature_for_read(
    validator: &mut LspDiskEditPathValidator<'_>,
    path: &Path,
) -> Result<LspDiskEditFileSignature, String> {
    validator.lsp_disk_edit_file_signature(path)
}

fn read_lsp_disk_edit_text_with_known_len(path: &Path, known_len: u64) -> Result<String, String> {
    read_lsp_disk_edit_text_after_metadata(path, known_len, LSP_DISK_EDIT_MAX_BYTES)
}

fn read_lsp_disk_edit_text_after_metadata(
    path: &Path,
    metadata_len: u64,
    max_bytes: u64,
) -> Result<String, String> {
    if metadata_len > max_bytes {
        return Err(format!(
            "file too large for LSP disk edit ({} bytes)",
            metadata_len
        ));
    }
    let mut file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut bytes = Vec::with_capacity(metadata_len.min(max_bytes).min(usize::MAX as u64) as usize);
    file.by_ref()
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    let byte_len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if byte_len > max_bytes {
        return Err(format!(
            "file too large for LSP disk edit ({byte_len} bytes)"
        ));
    }
    if bytes.contains(&0) {
        return Err("binary file skipped".to_owned());
    }
    String::from_utf8(bytes).map_err(|_| "invalid UTF-8 file skipped".to_owned())
}

fn lsp_disk_edit_file_signature(
    root: &Path,
    path: &Path,
) -> Result<LspDiskEditFileSignature, String> {
    let mut validator = LspDiskEditPathValidator::new(root);
    validator.lsp_disk_edit_file_signature(path)
}

struct LspDiskEditPathValidator<'a> {
    root: &'a Path,
    canonical_root: Option<Result<PathBuf, String>>,
}

impl<'a> LspDiskEditPathValidator<'a> {
    fn new(root: &'a Path) -> Self {
        Self {
            root,
            canonical_root: None,
        }
    }

    fn validate_existing_workspace_path(
        &mut self,
        path: &Path,
        action: &str,
    ) -> Result<(), String> {
        validate_workspace_path_lexically(self.root, path, action)?;
        let root = self.canonical_workspace_root()?;
        let path = path.canonicalize().map_err(display_lsp_disk_edit_error)?;
        if workspace_path_contains_lexically(root, &path) {
            Ok(())
        } else {
            Err(format!("workspace {action} resolved outside workspace"))
        }
    }

    fn validate_workspace_target_path(&mut self, path: &Path, action: &str) -> Result<(), String> {
        self.validate_workspace_target_path_with_optional_metadata(
            path,
            action,
            false,
            lsp_disk_edit_metadata,
        )
        .map(drop)
    }

    fn validate_workspace_target_path_with_metadata(
        &mut self,
        path: &Path,
        action: &str,
    ) -> Result<Option<std::fs::Metadata>, String> {
        self.validate_workspace_target_path_with_optional_metadata(
            path,
            action,
            true,
            lsp_disk_edit_metadata,
        )
    }

    fn validate_workspace_target_path_with_optional_metadata(
        &mut self,
        path: &Path,
        action: &str,
        load_metadata: bool,
        metadata: impl FnOnce(&Path) -> Result<std::fs::Metadata, std::io::Error>,
    ) -> Result<Option<std::fs::Metadata>, String> {
        validate_workspace_path_lexically(self.root, path, action)?;
        let root = self.canonical_workspace_root()?;
        let existing = match std::fs::symlink_metadata(path) {
            Ok(_) => {
                let canonical = path.canonicalize().map_err(display_lsp_disk_edit_error)?;
                if workspace_path_contains_lexically(root, &canonical) {
                    return if load_metadata {
                        metadata(path)
                            .map(Some)
                            .map_err(display_lsp_disk_edit_error)
                    } else {
                        Ok(None)
                    };
                }
                return Err(format!("workspace {action} target outside workspace"));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                nearest_existing_parent(path)?
            }
            Err(error) => return Err(display_lsp_disk_edit_error(error)),
        };
        let existing = existing
            .canonicalize()
            .map_err(display_lsp_disk_edit_error)?;
        if workspace_path_contains_lexically(root, &existing) {
            Ok(None)
        } else {
            Err(format!("workspace {action} target outside workspace"))
        }
    }

    fn validate_lsp_disk_edit_file_signature(
        &mut self,
        path: &Path,
        expected: &LspDiskEditFileSignature,
    ) -> Result<(), String> {
        let current = self.lsp_disk_edit_file_signature(path)?;
        if &current == expected {
            Ok(())
        } else {
            Err(stale_lsp_disk_edit_target_error(path))
        }
    }

    fn lsp_disk_edit_file_signature(
        &mut self,
        path: &Path,
    ) -> Result<LspDiskEditFileSignature, String> {
        self.validate_existing_workspace_path(path, "edit")?;
        let metadata = std::fs::metadata(path).map_err(display_lsp_disk_edit_error)?;
        if !metadata.is_file() {
            return Err(lsp_disk_edit_target_not_file_error(path));
        }
        Ok(LspDiskEditFileSignature::from_metadata(&metadata))
    }

    fn canonical_workspace_root(&mut self) -> Result<&Path, String> {
        if self.canonical_root.is_none() {
            self.canonical_root = Some(canonical_workspace_root(self.root));
        }
        match self
            .canonical_root
            .as_ref()
            .expect("canonical root cache initialized")
        {
            Ok(root) => Ok(root.as_path()),
            Err(error) => Err(error.clone()),
        }
    }
}

fn metadata_modified_nanos(metadata: &std::fs::Metadata) -> Option<u128> {
    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
}

fn stale_lsp_disk_edit_target_error(path: &Path) -> String {
    format!(
        "stale disk edit target changed: {}",
        display_path_label_cow(path)
    )
}

fn lsp_disk_edit_target_not_file_error(path: &Path) -> String {
    format!(
        "workspace edit target is not a file: {}",
        display_path_label_cow(path)
    )
}

fn display_lsp_disk_edit_error(error: impl std::fmt::Display) -> String {
    display_error_label_cow(&error.to_string()).into_owned()
}

fn lsp_disk_edit_metadata(path: &Path) -> Result<std::fs::Metadata, std::io::Error> {
    std::fs::metadata(path)
}

fn canonical_workspace_root(root: &Path) -> Result<PathBuf, String> {
    root.canonicalize().map_err(|error| {
        format!(
            "workspace root unavailable: {}",
            display_lsp_disk_edit_error(error)
        )
    })
}

fn nearest_existing_ancestor(path: &Path) -> Result<PathBuf, String> {
    let mut current = path;
    loop {
        if current.exists() {
            return Ok(current.to_path_buf());
        }
        current = current
            .parent()
            .ok_or_else(|| "workspace target has no existing ancestor".to_owned())?;
    }
}

fn nearest_existing_parent(path: &Path) -> Result<PathBuf, String> {
    let parent = path
        .parent()
        .ok_or_else(|| "workspace target has no existing ancestor".to_owned())?;
    nearest_existing_ancestor(parent)
}

fn validate_workspace_path_lexically(root: &Path, path: &Path, action: &str) -> Result<(), String> {
    if path_is_within_workspace_lexically(root, path) {
        Ok(())
    } else {
        Err(format!("workspace {action} outside workspace"))
    }
}

fn path_is_within_workspace_lexically(root: &Path, path: &Path) -> bool {
    workspace_path_stays_within_root_lexically(root, path)
        && workspace_path_contains_lexically(root, path)
}

fn path_same_as_workspace_root_lexically(root: &Path, path: &Path) -> bool {
    trusted_workspace_paths_match(root, path)
}

fn workspace_directory_rename_target_inside_source(old_path: &Path, new_path: &Path) -> bool {
    !trusted_workspace_paths_match(old_path, new_path)
        && workspace_path_contains_lexically(old_path, new_path)
}

fn create_lsp_workspace_file(
    validator: &mut LspDiskEditPathValidator<'_>,
    path: PathBuf,
    overwrite: bool,
    ignore_if_exists: bool,
) -> Result<bool, String> {
    if path_same_as_workspace_root_lexically(validator.root, &path) {
        return Err("cannot create over workspace root".to_owned());
    }
    if let Some(metadata) =
        validator.validate_workspace_target_path_with_metadata(&path, "create")?
    {
        if overwrite {
            if metadata.is_dir() {
                return Err("cannot overwrite directory".to_owned());
            }
            let expected = LspDiskEditFileSignature::from_metadata(&metadata);
            read_lsp_disk_edit_text_with_known_len(&path, metadata.len())
                .map_err(|error| display_error_label_cow(&error).into_owned())?;
            validator.validate_lsp_disk_edit_file_signature(&path, &expected)?;
        } else if ignore_if_exists {
            return Ok(false);
        } else {
            return Err("path already exists".to_owned());
        }
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(display_lsp_disk_edit_error)?;
    }
    validator.validate_workspace_target_path(&path, "create")?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true);
    if overwrite {
        options.create(true).truncate(true);
    } else {
        options.create_new(true);
    }
    options
        .open(&path)
        .map(drop)
        .map_err(display_lsp_disk_edit_error)?;
    Ok(true)
}

fn rename_lsp_workspace_file(
    validator: &mut LspDiskEditPathValidator<'_>,
    old_path: PathBuf,
    new_path: PathBuf,
    overwrite: bool,
    ignore_if_exists: bool,
) -> Result<bool, String> {
    if path_same_as_workspace_root_lexically(validator.root, &old_path) {
        return Err("cannot rename workspace root".to_owned());
    }
    if path_same_as_workspace_root_lexically(validator.root, &new_path) {
        return Err("cannot rename to workspace root".to_owned());
    }
    if trusted_workspace_paths_match(&old_path, &new_path) {
        return Err("resource rename source and target match".to_owned());
    }
    validator.validate_existing_workspace_path(&old_path, "rename")?;
    let new_metadata =
        validator.validate_workspace_target_path_with_metadata(&new_path, "rename")?;
    let old_metadata = std::fs::symlink_metadata(&old_path).map_err(display_lsp_disk_edit_error)?;
    if old_metadata.is_dir()
        && workspace_directory_rename_target_inside_source(&old_path, &new_path)
    {
        return Err("resource rename target is inside source".to_owned());
    }
    if let Some(new_metadata) = new_metadata {
        if overwrite {
            if new_metadata.is_dir() {
                return Err("cannot overwrite directory".to_owned());
            }
            let expected = LspDiskEditFileSignature::from_metadata(&new_metadata);
            read_lsp_disk_edit_text_with_known_len(&new_path, new_metadata.len())
                .map_err(|error| display_error_label_cow(&error).into_owned())?;
            validator.validate_lsp_disk_edit_file_signature(&new_path, &expected)?;
            remove_existing_lsp_workspace_file(&new_path)?;
        } else if ignore_if_exists {
            return Ok(false);
        } else {
            return Err("target already exists".to_owned());
        }
    }
    if let Some(parent) = new_path.parent() {
        std::fs::create_dir_all(parent).map_err(display_lsp_disk_edit_error)?;
    }
    validator.validate_existing_workspace_path(&old_path, "rename")?;
    validator.validate_workspace_target_path(&new_path, "rename")?;
    std::fs::rename(&old_path, &new_path).map_err(display_lsp_disk_edit_error)?;
    Ok(true)
}

fn delete_lsp_workspace_file(
    validator: &mut LspDiskEditPathValidator<'_>,
    path: PathBuf,
    recursive: bool,
    ignore_if_not_exists: bool,
) -> Result<bool, String> {
    if path_same_as_workspace_root_lexically(validator.root, &path) {
        return Err("cannot delete workspace root".to_owned());
    }
    validate_workspace_path_lexically(validator.root, &path, "delete")?;
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && ignore_if_not_exists => {
            validator.validate_workspace_target_path(&path, "delete")?;
            return Ok(false);
        }
        Err(error) => return Err(display_lsp_disk_edit_error(error)),
    };
    validator.validate_existing_workspace_path(&path, "delete")?;
    if metadata.is_dir() {
        if !recursive {
            return Err("recursive delete required for directories".to_owned());
        }
        std::fs::remove_dir_all(&path).map_err(display_lsp_disk_edit_error)?;
    } else {
        std::fs::remove_file(&path).map_err(display_lsp_disk_edit_error)?;
    }
    Ok(true)
}

fn remove_existing_lsp_workspace_file(path: &std::path::Path) -> Result<(), String> {
    std::fs::remove_file(path).map_err(display_lsp_disk_edit_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path_display::{DISPLAY_ERROR_LABEL_MAX_CHARS, DISPLAY_PATH_LABEL_MAX_CHARS};
    use kuroya_core::LspTextEdit;
    use std::{
        cell::Cell,
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn lsp_disk_text_edits_reject_workspace_escape_before_mutating_file() {
        let base = temp_workspace("disk-edit-escape");
        let root = base.join("workspace");
        fs::create_dir_all(&root).unwrap();
        let outside = base.join("outside.rs");
        fs::write(&outside, "old\n").unwrap();
        let escaped = root.join("..").join("outside.rs");

        let result = apply_lsp_disk_text_edits(&root, &escaped, &[edit(&escaped, "changed\n")]);

        assert!(result.unwrap_err().contains("outside workspace"));
        assert_eq!(fs::read_to_string(outside).unwrap(), "old\n");
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn lsp_workspace_create_rejects_workspace_escape_before_mutating_disk() {
        let base = temp_workspace("create-escape");
        let root = base.join("workspace");
        fs::create_dir_all(&root).unwrap();
        let outside = base.join("created.rs");
        let escaped = root.join("..").join("created.rs");

        let result = apply_lsp_workspace_document_change(
            &root,
            LspWorkspaceDocumentChange::Resource(LspWorkspaceResourceOperation::CreateFile {
                path: escaped,
                overwrite: false,
                ignore_if_exists: false,
            }),
        );

        assert!(result.unwrap_err().contains("outside workspace"));
        assert!(!outside.exists());
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn lsp_workspace_create_rejects_parent_reentry_before_mutating_disk() {
        let base = temp_workspace("create-parent-reentry");
        let root = base.join("workspace");
        fs::create_dir_all(&root).unwrap();
        let created = root.join("created.rs");
        let reentry = root.join("..").join("workspace").join("created.rs");

        let result = apply_lsp_workspace_document_change(
            &root,
            LspWorkspaceDocumentChange::Resource(LspWorkspaceResourceOperation::CreateFile {
                path: reentry,
                overwrite: false,
                ignore_if_exists: false,
            }),
        );

        assert!(result.unwrap_err().contains("outside workspace"));
        assert!(!created.exists());
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn lsp_workspace_rename_rejects_target_escape_before_moving_source() {
        let base = temp_workspace("rename-escape");
        let root = base.join("workspace");
        fs::create_dir_all(root.join("src")).unwrap();
        let old_path = root.join("src").join("old.rs");
        fs::write(&old_path, "old\n").unwrap();
        let outside = base.join("renamed.rs");
        let escaped = root.join("..").join("renamed.rs");

        let result = apply_lsp_workspace_document_change(
            &root,
            LspWorkspaceDocumentChange::Resource(LspWorkspaceResourceOperation::RenameFile {
                old_path: old_path.clone(),
                new_path: escaped,
                overwrite: false,
                ignore_if_exists: false,
            }),
        );

        assert!(result.unwrap_err().contains("outside workspace"));
        assert_eq!(fs::read_to_string(old_path).unwrap(), "old\n");
        assert!(!outside.exists());
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn lsp_workspace_rename_rejects_directory_target_inside_source_before_creating_parent() {
        let root = temp_workspace("rename-dir-inside-source");
        let old_path = root.join("src").join("dir");
        let child_path = old_path.join("file.rs");
        let new_path = old_path.join("nested").join("renamed");
        fs::create_dir_all(&old_path).unwrap();
        fs::write(&child_path, "old\n").unwrap();

        let result = apply_lsp_workspace_document_change(
            &root,
            LspWorkspaceDocumentChange::Resource(LspWorkspaceResourceOperation::RenameFile {
                old_path: old_path.clone(),
                new_path: new_path.clone(),
                overwrite: false,
                ignore_if_exists: false,
            }),
        );

        assert!(
            result
                .unwrap_err()
                .contains("resource rename target is inside source")
        );
        assert_eq!(fs::read_to_string(child_path).unwrap(), "old\n");
        assert!(!old_path.join("nested").exists());
        assert!(!new_path.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn lsp_workspace_create_overwrite_rejects_binary_target_before_truncating_it() {
        let root = temp_workspace("create-overwrite-binary");
        fs::create_dir_all(root.join("src")).unwrap();
        let path = root.join("src").join("binary.rs");
        let original = b"old\0bytes".to_vec();
        fs::write(&path, &original).unwrap();

        let result = apply_lsp_workspace_document_change(
            &root,
            LspWorkspaceDocumentChange::Resource(LspWorkspaceResourceOperation::CreateFile {
                path: path.clone(),
                overwrite: true,
                ignore_if_exists: false,
            }),
        );

        assert!(result.unwrap_err().contains("binary file skipped"));
        assert_eq!(fs::read(path).unwrap(), original);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn lsp_workspace_text_edit_rejects_directory_target_before_reading() {
        let root = temp_workspace("text-edit-directory-target");
        let path = root.join("src").join("dir.rs");
        fs::create_dir_all(&path).unwrap();

        let result = apply_lsp_workspace_document_change(
            &root,
            LspWorkspaceDocumentChange::TextEdit {
                path: path.clone(),
                version: None,
                edits: vec![edit(&path, "new\n")],
            },
        );

        assert!(
            result
                .unwrap_err()
                .contains("workspace edit target is not a file")
        );
        assert!(path.is_dir());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn lsp_disk_text_reader_with_known_len_rechecks_growth_with_bounded_read() {
        let root = temp_workspace("known-len-growth");
        fs::create_dir_all(&root).unwrap();
        let path = root.join("grown.rs");
        fs::write(&path, "123456789").unwrap();

        let error = read_lsp_disk_edit_text_after_metadata(&path, 1, 4).unwrap_err();

        assert_eq!(error, "file too large for LSP disk edit (5 bytes)");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn lsp_workspace_target_validation_skips_metadata_when_caller_discards_it() {
        let root = temp_workspace("target-validation-skip-metadata");
        fs::create_dir_all(root.join("src")).unwrap();
        let path = root.join("src").join("existing.rs");
        fs::write(&path, "old\n").unwrap();
        let metadata_calls = Cell::new(0);
        let mut validator = LspDiskEditPathValidator::new(&root);

        let result = validator.validate_workspace_target_path_with_optional_metadata(
            &path,
            "create",
            false,
            |_| {
                metadata_calls.set(metadata_calls.get() + 1);
                fs::metadata(&path)
            },
        );

        assert!(result.unwrap().is_none());
        assert_eq!(metadata_calls.get(), 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn lsp_workspace_target_validation_loads_metadata_once_when_requested() {
        let root = temp_workspace("target-validation-load-metadata");
        fs::create_dir_all(root.join("src")).unwrap();
        let path = root.join("src").join("existing.rs");
        fs::write(&path, "old\n").unwrap();
        let metadata_calls = Cell::new(0);
        let mut validator = LspDiskEditPathValidator::new(&root);

        let result = validator.validate_workspace_target_path_with_optional_metadata(
            &path,
            "create",
            true,
            |path| {
                metadata_calls.set(metadata_calls.get() + 1);
                fs::metadata(path)
            },
        );

        assert!(result.unwrap().unwrap().is_file());
        assert_eq!(metadata_calls.get(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn lsp_workspace_resource_ops_create_rename_delete_successfully() {
        let root = temp_workspace("resource-ops-success");
        let src = root.join("src");
        fs::create_dir_all(&src).unwrap();
        let created = src.join("created.rs");
        let renamed = src.join("renamed.rs");

        let created_result = apply_lsp_workspace_document_change(
            &root,
            LspWorkspaceDocumentChange::Resource(LspWorkspaceResourceOperation::CreateFile {
                path: created.clone(),
                overwrite: false,
                ignore_if_exists: false,
            }),
        );

        assert_eq!(created_result, Ok(true));
        assert!(created.exists());
        fs::write(&created, "created\n").unwrap();

        let renamed_result = apply_lsp_workspace_document_change(
            &root,
            LspWorkspaceDocumentChange::Resource(LspWorkspaceResourceOperation::RenameFile {
                old_path: created.clone(),
                new_path: renamed.clone(),
                overwrite: false,
                ignore_if_exists: false,
            }),
        );

        assert_eq!(renamed_result, Ok(true));
        assert!(!created.exists());
        assert_eq!(fs::read_to_string(&renamed).unwrap(), "created\n");

        let deleted_result = apply_lsp_workspace_document_change(
            &root,
            LspWorkspaceDocumentChange::Resource(LspWorkspaceResourceOperation::DeleteFile {
                path: renamed.clone(),
                recursive: false,
                ignore_if_not_exists: false,
            }),
        );

        assert_eq!(deleted_result, Ok(true));
        assert!(!renamed.exists());

        let ignored_delete = apply_lsp_workspace_document_change(
            &root,
            LspWorkspaceDocumentChange::Resource(LspWorkspaceResourceOperation::DeleteFile {
                path: renamed,
                recursive: false,
                ignore_if_not_exists: true,
            }),
        );

        assert_eq!(ignored_delete, Ok(false));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn lsp_workspace_resource_ops_keep_root_and_same_target_guards() {
        let root = temp_workspace("resource-op-root-guards");
        let src = root.join("src");
        fs::create_dir_all(&src).unwrap();
        let source = src.join("source.rs");
        let target = src.join("target.rs");
        fs::write(&source, "source\n").unwrap();

        let create_root = apply_lsp_workspace_document_change(
            &root,
            LspWorkspaceDocumentChange::Resource(LspWorkspaceResourceOperation::CreateFile {
                path: root.clone(),
                overwrite: true,
                ignore_if_exists: false,
            }),
        );
        assert_eq!(
            create_root.unwrap_err(),
            "cannot create over workspace root"
        );
        assert!(root.exists());

        let rename_root = apply_lsp_workspace_document_change(
            &root,
            LspWorkspaceDocumentChange::Resource(LspWorkspaceResourceOperation::RenameFile {
                old_path: root.clone(),
                new_path: target.clone(),
                overwrite: false,
                ignore_if_exists: false,
            }),
        );
        assert_eq!(rename_root.unwrap_err(), "cannot rename workspace root");
        assert!(root.exists());
        assert!(!target.exists());

        let rename_to_root = apply_lsp_workspace_document_change(
            &root,
            LspWorkspaceDocumentChange::Resource(LspWorkspaceResourceOperation::RenameFile {
                old_path: source.clone(),
                new_path: root.clone(),
                overwrite: true,
                ignore_if_exists: false,
            }),
        );
        assert_eq!(
            rename_to_root.unwrap_err(),
            "cannot rename to workspace root"
        );
        assert_eq!(fs::read_to_string(&source).unwrap(), "source\n");

        let rename_same_target = apply_lsp_workspace_document_change(
            &root,
            LspWorkspaceDocumentChange::Resource(LspWorkspaceResourceOperation::RenameFile {
                old_path: source.clone(),
                new_path: source.clone(),
                overwrite: true,
                ignore_if_exists: false,
            }),
        );
        assert_eq!(
            rename_same_target.unwrap_err(),
            "resource rename source and target match"
        );
        assert_eq!(fs::read_to_string(source).unwrap(), "source\n");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn lsp_disk_text_edit_plan_rejects_stale_file_before_write() {
        let root = temp_workspace("stale-disk-edit");
        fs::create_dir_all(root.join("src")).unwrap();
        let path = root.join("src").join("main.rs");
        fs::write(&path, "old\n").unwrap();
        let plan = LspDiskTextEditPlan::capture(&root, path.clone(), vec![edit(&path, "new\n")]);

        fs::write(&path, "changed after queue\n").unwrap();
        let result = apply_lsp_disk_text_edit_plan(&root, plan);

        assert!(result.unwrap_err().contains("stale disk edit target"));
        assert_eq!(fs::read_to_string(path).unwrap(), "changed after queue\n");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn lsp_disk_text_edit_plan_rejects_directory_target_before_reading() {
        let root = temp_workspace("plan-directory-target");
        let path = root.join("src").join("dir.rs");
        fs::create_dir_all(&path).unwrap();
        let plan = LspDiskTextEditPlan::capture(&root, path.clone(), vec![edit(&path, "new\n")]);

        let result = apply_lsp_disk_text_edit_plan(&root, plan);

        assert!(
            result
                .unwrap_err()
                .contains("workspace edit target is not a file")
        );
        assert!(path.is_dir());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn lsp_disk_edit_errors_sanitize_and_bound_path_and_error_labels() {
        let path = PathBuf::from("workspace/src").join(format!(
            "bad\n{}\u{202e}target.rs",
            "very-long-path-".repeat(DISPLAY_PATH_LABEL_MAX_CHARS)
        ));
        let error = format!(
            "first line\nsecond line \u{202e}{}",
            "x".repeat(DISPLAY_ERROR_LABEL_MAX_CHARS * 2)
        );

        let stale = stale_lsp_disk_edit_target_error(&path);
        let not_file = lsp_disk_edit_target_not_file_error(&path);
        let displayed_error = display_lsp_disk_edit_error(&error);

        for status in [&stale, &not_file, &displayed_error] {
            assert_safe_status_text(status);
            assert!(status.contains("..."), "{status}");
        }
        assert!(
            stale.chars().count()
                <= "stale disk edit target changed: ".chars().count()
                    + DISPLAY_PATH_LABEL_MAX_CHARS
        );
        assert!(
            not_file.chars().count()
                <= "workspace edit target is not a file: ".chars().count()
                    + DISPLAY_PATH_LABEL_MAX_CHARS
        );
        assert!(displayed_error.chars().count() <= DISPLAY_ERROR_LABEL_MAX_CHARS);
    }

    #[test]
    fn lsp_disk_text_edit_plan_capture_preserves_raw_path_and_edit_payload() {
        let root = PathBuf::from("workspace");
        let raw_path = root.join(format!(
            "raw\n{}\u{202e}.rs",
            "unsafe-path-".repeat(DISPLAY_PATH_LABEL_MAX_CHARS)
        ));
        let raw_text = "first\nsecond\u{202e}\n".to_owned();
        let raw_edit = LspTextEdit {
            path: raw_path.clone(),
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 4,
            new_text: raw_text.clone(),
        };

        let plan = LspDiskTextEditPlan::capture(&root, raw_path.clone(), vec![raw_edit]);

        assert_eq!(plan.path, raw_path);
        assert_eq!(plan.edits.len(), 1);
        assert_eq!(plan.edits[0].path, plan.path);
        assert_eq!(plan.edits[0].new_text, raw_text);
        assert_eq!(plan.expected, None);
    }

    #[test]
    fn lsp_workspace_rename_overwrite_rejects_binary_target_before_removing_it() {
        let root = temp_workspace("rename-overwrite-binary");
        fs::create_dir_all(root.join("src")).unwrap();
        let old_path = root.join("src").join("old.rs");
        let target = root.join("src").join("target.rs");
        let original_target = b"bin\0target".to_vec();
        fs::write(&old_path, "old\n").unwrap();
        fs::write(&target, &original_target).unwrap();

        let result = apply_lsp_workspace_document_change(
            &root,
            LspWorkspaceDocumentChange::Resource(LspWorkspaceResourceOperation::RenameFile {
                old_path: old_path.clone(),
                new_path: target.clone(),
                overwrite: true,
                ignore_if_exists: false,
            }),
        );

        assert!(result.unwrap_err().contains("binary file skipped"));
        assert_eq!(fs::read_to_string(old_path).unwrap(), "old\n");
        assert_eq!(fs::read(target).unwrap(), original_target);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn lsp_workspace_delete_rejects_workspace_root_even_when_called_directly() {
        let root = temp_workspace("delete-root");
        fs::create_dir_all(root.join("src")).unwrap();

        let result = apply_lsp_workspace_document_change(
            &root,
            LspWorkspaceDocumentChange::Resource(LspWorkspaceResourceOperation::DeleteFile {
                path: root.clone(),
                recursive: true,
                ignore_if_not_exists: false,
            }),
        );

        assert_eq!(result.unwrap_err(), "cannot delete workspace root");
        assert!(root.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn lsp_workspace_delete_rejects_equivalent_workspace_root() {
        let root = temp_workspace("delete-equivalent-root");
        fs::create_dir_all(root.join("src")).unwrap();
        let equivalent_root = root.join("src").join("..");

        let result = apply_lsp_workspace_document_change(
            &root,
            LspWorkspaceDocumentChange::Resource(LspWorkspaceResourceOperation::DeleteFile {
                path: equivalent_root,
                recursive: true,
                ignore_if_not_exists: false,
            }),
        );

        assert_eq!(result.unwrap_err(), "cannot delete workspace root");
        assert!(root.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn lsp_workspace_lexical_guards_match_windows_paths_case_insensitively() {
        assert!(path_is_within_workspace_lexically(
            Path::new(r"C:\Repo\Project"),
            Path::new(r"c:\repo\project\src\main.rs")
        ));
        assert!(path_same_as_workspace_root_lexically(
            Path::new(r"C:\Repo\Project"),
            Path::new(r"c:\repo\project\src\..")
        ));
    }

    fn edit(path: &Path, new_text: &str) -> LspTextEdit {
        LspTextEdit {
            path: path.to_path_buf(),
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 4,
            new_text: new_text.to_owned(),
        }
    }

    fn assert_safe_status_text(status: &str) {
        assert!(
            !status.chars().any(is_unsafe_status_char),
            "status contains unsafe display characters: {status:?}"
        );
    }

    fn is_unsafe_status_char(ch: char) -> bool {
        ch.is_control()
            || matches!(
                ch,
                '\u{061c}'
                    | '\u{200e}'
                    | '\u{200f}'
                    | '\u{2028}'..='\u{202e}'
                    | '\u{2066}'..='\u{2069}'
            )
    }

    fn temp_workspace(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("kuroya-{label}-{}-{nanos}", std::process::id()))
    }
}
