use crate::{
    KuroyaApp,
    lsp_runtime::{
        LSP_MAX_RESTART_ATTEMPTS, LspRestartDecision, lsp_buffer_synced_status,
        lsp_restart_buffer_ids, lsp_restart_decision, lsp_server_config_for_language,
        lsp_server_configs_for_settings, lsp_server_ready_status, lsp_status_display_message,
        lsp_stopped_disabled_status, lsp_stopped_no_buffers_status,
        lsp_stopped_restart_scheduled_status, lsp_stopped_workspace_symbol_reason,
        schedule_lsp_restart_at,
    },
    lsp_ui_events::LspUiEvent,
    ui_events::UiEvent,
    workspace_state::{
        buffer_id_path_version_matches, lsp_event_path_is_current, workspace_event_matches,
    },
};
use std::{path::Path, time::Instant};

impl KuroyaApp {
    pub(crate) fn handle_lsp_event(&mut self, event: UiEvent) -> Option<UiEvent> {
        match event {
            UiEvent::Lsp(event) => {
                self.record_lsp_ui_event_trace(&event);
                if !self.workspace_trusted {
                    return None;
                }
                self.handle_lsp_ui_event(event)
            }
            other => Some(other),
        }
    }

    fn handle_lsp_ui_event(&mut self, event: LspUiEvent) -> Option<UiEvent> {
        match event {
            LspUiEvent::ServerResult { target, event } => {
                if !self.lsp_lifecycle_event_matches(
                    &target.language,
                    &target.root,
                    target.generation,
                ) {
                    return None;
                }
                self.handle_lsp_ui_event(*event)
            }
            LspUiEvent::Diagnostics {
                language,
                root,
                generation,
                path,
                version,
                diagnostics,
            } => {
                if !self.lsp_lifecycle_event_matches(&language, &root, generation) {
                    return None;
                }
                if !lsp_event_path_is_current(&self.workspace.root, &path) {
                    return None;
                }
                self.pending_lsp_diagnostics.queue_for_server(
                    crate::lsp_diagnostics_batch::PendingLspDiagnosticsSource {
                        language,
                        root,
                        generation,
                    },
                    path,
                    version,
                    diagnostics,
                    Instant::now(),
                );
                None
            }
            LspUiEvent::BufferSynced { id, path, version } => {
                if !lsp_event_path_is_current(&self.workspace.root, &path)
                    || !buffer_id_path_version_matches(&self.buffers, id, &path, version)
                {
                    return None;
                }
                self.status = lsp_buffer_synced_status(&path, version);
                None
            }
            event @ (LspUiEvent::HoverResult { .. }
            | LspUiEvent::DocumentHighlightsResult { .. }
            | LspUiEvent::DefinitionResult { .. }
            | LspUiEvent::CallHierarchyPrepared { .. }
            | LspUiEvent::CallHierarchyIncomingResult { .. }
            | LspUiEvent::CallHierarchyOutgoingResult { .. }
            | LspUiEvent::TypeHierarchyPrepared { .. }
            | LspUiEvent::TypeHierarchySupertypesResult { .. }
            | LspUiEvent::TypeHierarchySubtypesResult { .. }
            | LspUiEvent::ReferencesResult { .. }
            | LspUiEvent::RenameResult { .. }) => {
                self.handle_lsp_navigation_event(event);
                None
            }
            event @ (LspUiEvent::DocumentSymbolsResult { .. }
            | LspUiEvent::FoldingRangesResult { .. }
            | LspUiEvent::InlayHintsResult { .. }
            | LspUiEvent::CodeLensesResult { .. }
            | LspUiEvent::CodeLensResolveResult { .. }
            | LspUiEvent::CodeLensCommandResult { .. }
            | LspUiEvent::SemanticTokensResult { .. }
            | LspUiEvent::WorkspaceSymbolsResult { .. }) => {
                self.handle_lsp_symbol_event(event);
                None
            }
            event @ (LspUiEvent::CompletionResult { .. }
            | LspUiEvent::CompletionItemResolveResult { .. }
            | LspUiEvent::SignatureHelpResult { .. }
            | LspUiEvent::FormattingResult { .. }
            | LspUiEvent::CodeActionsResult { .. }
            | LspUiEvent::CodeActionResolveResult { .. }
            | LspUiEvent::WorkspaceApplyEditRequest { .. }
            | LspUiEvent::WorkspaceEditFilesApplied { .. }) => {
                self.handle_lsp_edit_event(event);
                None
            }
            LspUiEvent::WorkDoneProgressCreated { .. } => None,
            LspUiEvent::WorkDoneProgress {
                language,
                root,
                generation,
                progress,
            } => {
                if !self.lsp_lifecycle_event_matches(&language, &root, generation) {
                    return None;
                }
                self.handle_lsp_work_done_progress(language, root, generation, progress);
                None
            }
            LspUiEvent::ServerReady {
                language,
                root,
                generation,
            } => {
                if !self.lsp_lifecycle_event_matches(&language, &root, generation) {
                    return None;
                }
                self.lsp_restart_attempts.remove(&language);
                self.pending_lsp_restarts.remove(&language);
                self.status = lsp_server_ready_status(&language);
                None
            }
            LspUiEvent::ServerStopped {
                language,
                root,
                generation,
            } => {
                if !self.lsp_lifecycle_event_matches(&language, &root, generation) {
                    return None;
                }
                self.clear_lsp_progress_for_server(&language, &root, generation);
                self.lsp_clients.remove(&language);
                if self.lsp_unavailable.contains(&language) {
                    self.continue_pending_format_on_save_for_lsp(&language);
                    return None;
                }

                let lsp_configs = lsp_server_configs_for_settings(&self.settings);
                let restart_targets = lsp_restart_buffer_ids(
                    &language,
                    &self.buffers,
                    &lsp_configs,
                    &self.plugin_languages,
                    &self.workspace.root,
                    &self.lossy_decoded_buffers,
                    &self.binary_preview_buffers,
                );
                match lsp_restart_decision(
                    self.lsp_restart_attempts.get(&language).copied(),
                    restart_targets.len(),
                    LSP_MAX_RESTART_ATTEMPTS,
                ) {
                    LspRestartDecision::NoEligibleBuffers => {
                        self.lsp_restart_attempts.remove(&language);
                        self.status = lsp_stopped_no_buffers_status(&language);
                    }
                    LspRestartDecision::Disable => {
                        self.lsp_unavailable.insert(language.clone());
                        self.status = lsp_stopped_disabled_status(&language);
                    }
                    LspRestartDecision::Restart { attempt } => {
                        self.lsp_restart_attempts.insert(language.clone(), attempt);
                        let reopened = restart_targets.len();
                        self.pending_lsp_restarts.insert(
                            language.clone(),
                            schedule_lsp_restart_at(Instant::now(), attempt),
                        );
                        self.status = lsp_stopped_restart_scheduled_status(&language, reopened);
                    }
                }
                self.fallback_pending_workspace_symbols_for_stopped_lsp(&language);
                self.continue_pending_format_on_save_for_lsp(&language);
                None
            }
            LspUiEvent::Status {
                language,
                root,
                generation,
                message,
            } => {
                if !self.lsp_lifecycle_event_matches(&language, &root, generation) {
                    return None;
                }
                let unavailable =
                    unavailable_lsp_status_language(&message) == Some(language.as_str());
                if unavailable {
                    self.lsp_clients.remove(&language);
                    self.lsp_restart_attempts.remove(&language);
                    self.pending_lsp_restarts.remove(&language);
                    self.lsp_unavailable.insert(language.clone());
                }
                self.status = lsp_status_display_message(&message);
                if unavailable {
                    self.continue_pending_format_on_save_for_lsp(&language);
                }
                None
            }
        }
    }

    pub(crate) fn lsp_lifecycle_event_matches(
        &self,
        language: &str,
        root: &Path,
        generation: u64,
    ) -> bool {
        workspace_event_matches(&self.workspace.root, root)
            && self
                .lsp_clients
                .get(language)
                .is_some_and(|handle| handle.generation() == generation)
    }

    fn fallback_pending_workspace_symbols_for_stopped_lsp(&mut self, language: &str) {
        if !self.workspace_symbols_open {
            return;
        }
        let query = self.workspace_symbol_submitted_query.as_str();
        if query.is_empty() || self.workspace_symbol_query.trim() != query {
            return;
        }
        let Some(path) = self.workspace_symbol_submitted_path.as_deref() else {
            return;
        };
        if !lsp_event_path_is_current(&self.workspace.root, path) {
            return;
        }
        let lsp_configs = lsp_server_configs_for_settings(&self.settings);
        let submitted_for_language = self.buffers.iter().any(|buffer| {
            buffer.path().is_some_and(|buffer_path| buffer_path == path)
                && lsp_server_config_for_language(&lsp_configs, buffer.language())
                    .is_some_and(|config| config.language == language)
        });
        if submitted_for_language {
            let query = self.workspace_symbol_submitted_query.clone();
            let reason = lsp_stopped_workspace_symbol_reason(language);
            self.load_index_workspace_symbols(query, &reason);
        }
    }

    fn continue_pending_format_on_save_for_lsp(&mut self, language: &str) -> usize {
        let lsp_configs = lsp_server_configs_for_settings(&self.settings);
        let mut pending_save_ids = Vec::with_capacity(self.pending_format_on_save.len());
        for id in self.pending_format_on_save.keys().copied() {
            if self
                .buffer(id)
                .and_then(|buffer| lsp_server_config_for_language(&lsp_configs, buffer.language()))
                .is_some_and(|server| server.language == language)
            {
                pending_save_ids.push(id);
            }
        }

        let mut count = 0;
        for id in pending_save_ids {
            let Some(pending) = self.finish_pending_format_on_save(id) else {
                continue;
            };
            self.cancel_lsp_formatting_request(pending.request_id);
            let overwrite_external_change = self.take_format_on_save_overwrite_external_change(id);
            self.format_on_save_bypass.insert(id);
            if overwrite_external_change {
                self.spawn_save_to_over_external_change(id, pending.save_path);
            } else {
                self.spawn_save_to(id, pending.save_path);
            }
            count += 1;
        }

        count
    }
}

#[cfg(test)]
pub(crate) fn lsp_status_event_matches(current_root: &Path, event_root: &Path) -> bool {
    workspace_event_matches(current_root, event_root)
}

pub(crate) fn unavailable_lsp_status_language(message: &str) -> Option<&str> {
    let mut parts = message.split_whitespace();
    let language = parts.next()?;
    (parts.next() == Some("LSP") && parts.next() == Some("unavailable:")).then_some(language)
}

#[cfg(test)]
mod tests;
