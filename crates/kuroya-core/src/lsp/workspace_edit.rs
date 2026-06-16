use serde_json::Value;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use super::{
    LspTextEdit, LspWorkspaceDocumentChange, LspWorkspaceResourceOperation, MAX_LSP_TEXT_EDITS,
    collect_lsp_text_edits, file_uri_to_path,
};

pub fn parse_workspace_edit_response(value: &Value) -> Option<Vec<LspTextEdit>> {
    let result = value.get("result")?;
    if result.is_null() {
        return Some(Vec::new());
    }

    parse_workspace_edit(result)
}

fn parse_workspace_edit(result: &Value) -> Option<Vec<LspTextEdit>> {
    let parsed =
        parse_workspace_edit_with_resource_mode(result, WorkspaceEditResourceMode::Reject)?;
    Some(parsed.edits)
}

pub(super) struct ParsedWorkspaceEdit {
    pub(super) edits: Vec<LspTextEdit>,
    pub(super) document_changes: Vec<LspWorkspaceDocumentChange>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum WorkspaceEditResourceMode {
    Preserve,
    Reject,
}

pub(super) fn parse_workspace_edit_with_resource_mode(
    result: &Value,
    resource_mode: WorkspaceEditResourceMode,
) -> Option<ParsedWorkspaceEdit> {
    let mut edits = Vec::with_capacity(workspace_edit_initial_text_edit_capacity(result));
    let mut document_change_items =
        Vec::with_capacity(if resource_mode == WorkspaceEditResourceMode::Preserve {
            workspace_edit_initial_document_change_capacity(result)
        } else {
            0
        });
    if let Some(document_changes) = result.get("documentChanges") {
        let document_changes = document_changes.as_array()?;
        for document_change in document_changes {
            document_change.as_object()?;
            if let Some(resource_operation) = parse_workspace_resource_operation(document_change)? {
                if resource_mode == WorkspaceEditResourceMode::Reject {
                    return None;
                }
                document_change_items
                    .push(LspWorkspaceDocumentChange::Resource(resource_operation));
                continue;
            }
            let uri = document_change
                .get("textDocument")
                .and_then(|text_document| text_document.get("uri"))
                .and_then(Value::as_str)?;
            let path = file_uri_to_path(uri)?;
            let version = parse_text_document_edit_version(document_change.get("textDocument")?)?;
            let document_edits = document_change.get("edits")?.as_array()?;
            let mut text_edits = Vec::with_capacity(document_edits.len().min(MAX_LSP_TEXT_EDITS));
            collect_lsp_text_edits(&mut text_edits, &path, document_edits)?;
            if edits.len().saturating_add(text_edits.len()) > MAX_LSP_TEXT_EDITS {
                return None;
            }
            if resource_mode == WorkspaceEditResourceMode::Preserve {
                edits.extend(text_edits.clone());
                document_change_items.push(LspWorkspaceDocumentChange::TextEdit {
                    path,
                    version,
                    edits: text_edits,
                });
            } else {
                edits.extend(text_edits);
            }
        }
    } else if let Some(changes) = result.get("changes") {
        let changes = changes.as_object()?;
        for (uri, uri_edits) in changes {
            let path = file_uri_to_path(uri)?;
            let uri_edits = uri_edits.as_array()?;
            collect_lsp_text_edits(&mut edits, &path, uri_edits)?;
        }
    }

    Some(ParsedWorkspaceEdit {
        edits,
        document_changes: document_change_items,
    })
}

pub(super) struct ParsedWorkspaceApplyEdit {
    pub(super) edits: Vec<LspTextEdit>,
    pub(super) document_changes: Vec<LspWorkspaceDocumentChange>,
    pub(super) document_versions: BTreeMap<PathBuf, i32>,
}

pub(super) fn parse_workspace_apply_edit(result: &Value) -> Option<ParsedWorkspaceApplyEdit> {
    result.as_object()?;
    let mut edits = Vec::with_capacity(workspace_edit_initial_text_edit_capacity(result));
    let mut document_change_items =
        Vec::with_capacity(workspace_edit_initial_document_change_capacity(result));
    let mut document_versions = BTreeMap::new();

    if let Some(document_changes) = result.get("documentChanges") {
        let document_changes = document_changes.as_array()?;
        for document_change in document_changes {
            document_change.as_object()?;
            if let Some(resource_operation) = parse_workspace_resource_operation(document_change)? {
                document_change_items
                    .push(LspWorkspaceDocumentChange::Resource(resource_operation));
                continue;
            }
            let text_document = document_change.get("textDocument")?;
            text_document.as_object()?;
            let uri = text_document.get("uri").and_then(Value::as_str)?;
            let path = file_uri_to_path(uri)?;
            let version = parse_text_document_edit_version(text_document)?;
            if let Some(version) = version {
                collect_lsp_document_version(&mut document_versions, &path, version)?;
            }
            let document_edits = document_change.get("edits")?.as_array()?;
            let mut text_edits = Vec::with_capacity(document_edits.len().min(MAX_LSP_TEXT_EDITS));
            collect_lsp_text_edits(&mut text_edits, &path, document_edits)?;
            if edits.len().saturating_add(text_edits.len()) > MAX_LSP_TEXT_EDITS {
                return None;
            }
            edits.extend(text_edits.clone());
            document_change_items.push(LspWorkspaceDocumentChange::TextEdit {
                path,
                version,
                edits: text_edits,
            });
        }
    } else if let Some(changes) = result.get("changes") {
        let changes = changes.as_object()?;
        for (uri, uri_edits) in changes {
            let path = file_uri_to_path(uri)?;
            let uri_edits = uri_edits.as_array()?;
            collect_lsp_text_edits(&mut edits, &path, uri_edits)?;
        }
    }

    Some(ParsedWorkspaceApplyEdit {
        edits,
        document_changes: document_change_items,
        document_versions,
    })
}

fn workspace_edit_initial_text_edit_capacity(result: &Value) -> usize {
    match result.get("documentChanges") {
        Some(document_changes) => document_changes
            .as_array()
            .map(|document_changes| document_changes.len().min(MAX_LSP_TEXT_EDITS))
            .unwrap_or(0),
        None => result
            .get("changes")
            .and_then(Value::as_object)
            .map(|changes| changes.len().min(MAX_LSP_TEXT_EDITS))
            .unwrap_or(0),
    }
}

fn workspace_edit_initial_document_change_capacity(result: &Value) -> usize {
    result
        .get("documentChanges")
        .and_then(Value::as_array)
        .map(|document_changes| document_changes.len().min(MAX_LSP_TEXT_EDITS))
        .unwrap_or(0)
}

fn parse_workspace_resource_operation(
    document_change: &Value,
) -> Option<Option<LspWorkspaceResourceOperation>> {
    let Some(kind) = document_change.get("kind") else {
        return Some(None);
    };
    if document_change.get("textDocument").is_some() {
        return None;
    }
    let kind = kind.as_str()?;
    match kind {
        "create" => {
            let options = parse_create_or_rename_file_options(document_change.get("options"))?;
            let path = file_uri_to_path(document_change.get("uri")?.as_str()?)?;
            Some(Some(LspWorkspaceResourceOperation::CreateFile {
                path,
                overwrite: options.overwrite,
                ignore_if_exists: options.ignore_if_exists,
            }))
        }
        "rename" => {
            let options = parse_create_or_rename_file_options(document_change.get("options"))?;
            let old_path = file_uri_to_path(document_change.get("oldUri")?.as_str()?)?;
            let new_path = file_uri_to_path(document_change.get("newUri")?.as_str()?)?;
            Some(Some(LspWorkspaceResourceOperation::RenameFile {
                old_path,
                new_path,
                overwrite: options.overwrite,
                ignore_if_exists: options.ignore_if_exists,
            }))
        }
        "delete" => {
            let options = parse_delete_file_options(document_change.get("options"))?;
            let path = file_uri_to_path(document_change.get("uri")?.as_str()?)?;
            Some(Some(LspWorkspaceResourceOperation::DeleteFile {
                path,
                recursive: options.recursive,
                ignore_if_not_exists: options.ignore_if_not_exists,
            }))
        }
        _ => None,
    }
}

struct CreateOrRenameFileOptions {
    overwrite: bool,
    ignore_if_exists: bool,
}

fn parse_create_or_rename_file_options(
    options: Option<&Value>,
) -> Option<CreateOrRenameFileOptions> {
    Some(CreateOrRenameFileOptions {
        overwrite: parse_optional_bool_option(options, "overwrite")?.unwrap_or(false),
        ignore_if_exists: parse_optional_bool_option(options, "ignoreIfExists")?.unwrap_or(false),
    })
}

struct DeleteFileOptions {
    recursive: bool,
    ignore_if_not_exists: bool,
}

fn parse_delete_file_options(options: Option<&Value>) -> Option<DeleteFileOptions> {
    Some(DeleteFileOptions {
        recursive: parse_optional_bool_option(options, "recursive")?.unwrap_or(false),
        ignore_if_not_exists: parse_optional_bool_option(options, "ignoreIfNotExists")?
            .unwrap_or(false),
    })
}

fn parse_optional_bool_option(options: Option<&Value>, field: &str) -> Option<Option<bool>> {
    let Some(options) = options else {
        return Some(None);
    };
    if options.is_null() {
        return Some(None);
    }
    let options = options.as_object()?;
    match options.get(field) {
        None => Some(None),
        Some(value) => value.as_bool().map(Some),
    }
}

fn parse_text_document_edit_version(text_document: &Value) -> Option<Option<i32>> {
    match text_document.get("version") {
        None | Some(Value::Null) => Some(None),
        Some(Value::Number(version)) => {
            let version = version.as_i64()?;
            (0..=i32::MAX as i64)
                .contains(&version)
                .then_some(Some(version as i32))
        }
        Some(_) => None,
    }
}

fn collect_lsp_document_version(
    document_versions: &mut BTreeMap<PathBuf, i32>,
    path: &Path,
    version: i32,
) -> Option<()> {
    if let Some(previous) = document_versions.get(path) {
        return (*previous == version).then_some(());
    }
    document_versions.insert(path.to_path_buf(), version);
    Some(())
}
