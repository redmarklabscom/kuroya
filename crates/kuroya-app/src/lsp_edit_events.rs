mod code_actions;
mod formatting;
pub(crate) mod workspace_files;

use crate::{KuroyaApp, lsp_ui_events::LspUiEvent};
mod popup_results;

impl KuroyaApp {
    pub(crate) fn handle_lsp_edit_event(&mut self, event: LspUiEvent) {
        match event {
            LspUiEvent::CompletionResult {
                id,
                path,
                version,
                line,
                column,
                items,
                error,
            } => {
                self.handle_lsp_completion_result(id, path, version, line, column, items, error);
            }
            LspUiEvent::CompletionItemResolveResult {
                id,
                path,
                version,
                line,
                column,
                item,
                fallback_item,
                intent,
                error,
            } => {
                self.handle_lsp_completion_item_resolve_result(
                    id,
                    path,
                    version,
                    line,
                    column,
                    item.map(|item| *item),
                    *fallback_item,
                    intent,
                    error,
                );
            }
            LspUiEvent::SignatureHelpResult {
                id,
                path,
                version,
                line,
                column,
                help,
                error,
            } => {
                self.handle_lsp_signature_help_result(id, path, version, line, column, help, error);
            }
            LspUiEvent::FormattingResult {
                request_id,
                id,
                path,
                version,
                edits,
                error,
            } => {
                self.handle_lsp_formatting_result(request_id, id, path, version, edits, error);
            }
            LspUiEvent::CodeActionsResult {
                id,
                path,
                version,
                line,
                column,
                actions,
                error,
            } => {
                self.handle_lsp_code_actions_result(
                    id, path, version, line, column, actions, error,
                );
            }
            LspUiEvent::CodeActionResolveResult {
                id,
                path,
                version,
                line,
                column,
                action,
                error,
            } => {
                self.handle_lsp_code_action_resolve_result(
                    id, path, version, line, column, action, error,
                );
            }
            LspUiEvent::WorkspaceApplyEditRequest {
                language,
                root,
                generation,
                request_id,
                label,
                edits,
                document_changes,
                document_versions,
                error,
            } => {
                self.handle_lsp_workspace_apply_edit_request(
                    language,
                    root,
                    generation,
                    request_id,
                    label,
                    edits,
                    document_changes,
                    document_versions,
                    error,
                );
            }
            LspUiEvent::WorkspaceEditFilesApplied {
                root,
                generation,
                changed,
                failed,
                apply_edit_response,
            } => {
                if !self.workspace_event_is_current(&root, generation) {
                    return;
                }
                self.handle_lsp_workspace_edit_files_applied(
                    changed,
                    failed.len(),
                    apply_edit_response,
                );
            }
            _ => {}
        }
    }
}
