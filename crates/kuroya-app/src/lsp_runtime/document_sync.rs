use crate::{
    KuroyaApp,
    lsp_lifecycle::{
        background_language_block_reason, lsp_lifecycle_target_for_buffer,
        lsp_lifecycle_targets_for_buffers,
    },
    lsp_runtime::{
        LSP_SYMBOL_REFRESH_DEBOUNCE, lsp_command_queue_failed_status,
        take_due_lsp_symbol_refresh_ids,
    },
    lsp_text_positions::lsp_line_content_utf16_len,
    path_display::display_path_label_cow,
};
use kuroya_core::{BufferId, LanguageId, TextBuffer, TextSnapshot};
use std::{
    borrow::Cow,
    collections::HashSet,
    path::{Path, PathBuf},
    time::Instant,
};

impl KuroyaApp {
    pub(crate) fn notify_lsp_open(&mut self, id: BufferId) {
        let Some((path, language, version)) = self.lsp_document_sync_target(id) else {
            return;
        };

        if let Some(client) = self.ensure_lsp_for_buffer(id) {
            let Some(text) = self.lsp_text_snapshot_for_version(id, version) else {
                return;
            };
            if !client.did_open(id, path.clone(), language, version, text) {
                self.status = lsp_command_queue_failed_status("textDocument/didOpen");
                return;
            }
            self.record_lsp_client_trace(
                "textDocument/didOpen",
                lsp_document_version_trace_label(&path, version),
            );
            self.pending_lsp_symbol_refreshes.remove(&id);
            self.request_lsp_symbol_refreshes(&client, id, &path, version);
        }
    }

    pub(crate) fn notify_lsp_change(&mut self, id: BufferId) {
        let Some((path, _language, version)) = self.lsp_document_sync_target(id) else {
            return;
        };

        if let Some(client) = self.ensure_lsp_for_buffer(id) {
            let Some(text) = self.lsp_text_snapshot_for_version(id, version) else {
                return;
            };
            if !client.did_change(id, path.clone(), version, text) {
                self.status = lsp_command_queue_failed_status("textDocument/didChange");
                return;
            }
            self.record_lsp_client_trace(
                "textDocument/didChange",
                lsp_document_version_trace_label(&path, version),
            );
            self.schedule_lsp_symbol_refresh(id);
        }
    }

    pub(crate) fn notify_lsp_save(&mut self, id: BufferId) {
        let Some(buffer) = self.buffer(id) else {
            return;
        };
        if background_language_block_reason(
            id,
            buffer,
            &self.lossy_decoded_buffers,
            &self.binary_preview_buffers,
        )
        .is_some()
        {
            return;
        }
        let Some(path) = buffer.path().cloned() else {
            return;
        };

        if let Some(client) = self.ensure_lsp_for_buffer(id) {
            if client.did_save(path.clone()) {
                self.record_lsp_client_trace(
                    "textDocument/didSave",
                    lsp_document_trace_path_label(&path),
                );
            } else {
                self.status = lsp_command_queue_failed_status("textDocument/didSave");
            }
        }
    }

    pub(crate) fn notify_lsp_close(&mut self, id: BufferId) {
        let Some(buffer) = self.buffer(id) else {
            return;
        };
        let Some((key, path)) = lsp_lifecycle_target_for_buffer(
            buffer,
            &self.lossy_decoded_buffers,
            &self.binary_preview_buffers,
        ) else {
            return;
        };

        if let Some(client) = self.lsp_clients.get(&key).cloned() {
            if client.did_close(path.clone()) {
                self.record_lsp_client_trace(
                    "textDocument/didClose",
                    lsp_document_trace_path_label(&path),
                );
            } else {
                self.status = lsp_command_queue_failed_status("textDocument/didClose");
            }
        }
        self.pending_lsp_symbol_refreshes.remove(&id);
    }

    pub(crate) fn notify_lsp_close_all(&mut self) {
        for (key, path) in lsp_lifecycle_targets_for_buffers(
            &self.buffers,
            &self.lossy_decoded_buffers,
            &self.binary_preview_buffers,
        ) {
            if let Some(client) = self.lsp_clients.get(&key).cloned() {
                if client.did_close(path.clone()) {
                    self.record_lsp_client_trace(
                        "textDocument/didClose",
                        lsp_document_trace_path_label(&path),
                    );
                }
            }
        }
    }

    pub(crate) fn flush_pending_lsp_symbol_refreshes(&mut self) -> usize {
        let ids = take_due_lsp_symbol_refresh_ids(
            &mut self.pending_lsp_symbol_refreshes,
            Instant::now(),
            LSP_SYMBOL_REFRESH_DEBOUNCE,
        );
        let mut count = 0usize;
        for id in ids {
            if self.request_lsp_symbol_refresh_for_buffer(id) {
                count = count.saturating_add(1);
            }
        }
        count
    }

    pub(crate) fn schedule_lsp_symbol_refreshes_for_open_buffers(&mut self) -> usize {
        let now = Instant::now();
        let scheduled_at = now.checked_sub(LSP_SYMBOL_REFRESH_DEBOUNCE).unwrap_or(now);
        let mut count = 0usize;
        let mut eligible_ids = HashSet::new();

        for buffer in &self.buffers {
            if lsp_symbol_refresh_buffer_is_eligible(
                buffer,
                &self.lossy_decoded_buffers,
                &self.binary_preview_buffers,
            ) {
                let id = buffer.id();
                eligible_ids.insert(id);
                self.pending_lsp_symbol_refreshes.insert(id, scheduled_at);
                count = count.saturating_add(1);
            }
        }

        self.pending_lsp_symbol_refreshes
            .retain(|id, _| eligible_ids.contains(id));
        count
    }

    fn schedule_lsp_symbol_refresh(&mut self, id: BufferId) {
        self.pending_lsp_symbol_refreshes.insert(id, Instant::now());
    }

    fn request_lsp_symbol_refresh_for_buffer(&mut self, id: BufferId) -> bool {
        let Some((path, version)) = self.lsp_symbol_refresh_target(id) else {
            return false;
        };
        let Some(client) = self.ensure_lsp_for_buffer(id) else {
            return false;
        };

        self.request_lsp_symbol_refreshes(&client, id, &path, version)
    }

    fn lsp_symbol_refresh_target(&self, id: BufferId) -> Option<(PathBuf, u64)> {
        let buffer = self.buffer(id)?;
        lsp_symbol_refresh_target_for_buffer(
            buffer,
            &self.lossy_decoded_buffers,
            &self.binary_preview_buffers,
        )
    }

    fn request_lsp_symbol_refreshes(
        &mut self,
        client: &crate::lsp_client::LspClientHandle,
        id: BufferId,
        path: &Path,
        version: u64,
    ) -> bool {
        let mut queued = false;
        let trace_label = lsp_document_trace_path_label(path);
        let path_buf = path.to_path_buf();
        let inlay_hint_range = if self.settings.inlay_hints {
            self.lsp_inlay_hint_range_end(id)
        } else {
            None
        };

        if self.settings.inlay_hints {
            if let Some((end_line, end_character)) = inlay_hint_range {
                if client.inlay_hints(id, path_buf.clone(), version, end_line, end_character) {
                    self.record_lsp_client_trace("textDocument/inlayHint", trace_label.clone());
                    queued = true;
                } else {
                    self.status = lsp_command_queue_failed_status("textDocument/inlayHint");
                }
            }
        } else {
            self.inlay_hints.remove(path);
        }

        if self.settings.code_lens {
            if client.code_lenses(id, path_buf.clone(), version) {
                self.record_lsp_client_trace("textDocument/codeLens", trace_label.clone());
                queued = true;
            } else {
                self.status = lsp_command_queue_failed_status("textDocument/codeLens");
            }
        } else {
            self.code_lenses.remove(path);
        }

        if client.semantic_tokens(id, path_buf, version) {
            self.record_lsp_client_trace("textDocument/semanticTokens/full", trace_label);
            queued = true;
        } else {
            self.status = lsp_command_queue_failed_status("textDocument/semanticTokens/full");
        }
        queued
    }

    fn lsp_document_sync_target(&self, id: BufferId) -> Option<(PathBuf, LanguageId, u64)> {
        let buffer = self.buffer(id)?;
        if background_language_block_reason(
            id,
            buffer,
            &self.lossy_decoded_buffers,
            &self.binary_preview_buffers,
        )
        .is_some()
        {
            return None;
        }
        Some((buffer.path()?.clone(), buffer.language(), buffer.version()))
    }

    fn lsp_text_snapshot_for_version(&self, id: BufferId, version: u64) -> Option<TextSnapshot> {
        let buffer = self.buffer(id)?;
        (buffer.version() == version).then(|| buffer.text_snapshot())
    }

    fn lsp_inlay_hint_range_end(&self, id: BufferId) -> Option<(usize, usize)> {
        self.buffer(id).map(inlay_hint_range_end)
    }
}

fn lsp_symbol_refresh_target_for_buffer(
    buffer: &TextBuffer,
    lossy_buffers: &HashSet<BufferId>,
    binary_buffers: &HashSet<BufferId>,
) -> Option<(PathBuf, u64)> {
    if !lsp_symbol_refresh_buffer_is_eligible(buffer, lossy_buffers, binary_buffers) {
        return None;
    }

    Some((buffer.path()?.clone(), buffer.version()))
}

fn lsp_symbol_refresh_buffer_is_eligible(
    buffer: &TextBuffer,
    lossy_buffers: &HashSet<BufferId>,
    binary_buffers: &HashSet<BufferId>,
) -> bool {
    let id = buffer.id();
    if background_language_block_reason(id, buffer, lossy_buffers, binary_buffers).is_some() {
        return false;
    }

    buffer.path().is_some()
}

fn lsp_document_version_trace_label(path: &Path, version: u64) -> String {
    format!("{} v{version}", lsp_document_trace_path_label(path))
}

fn lsp_document_trace_path_label(path: &Path) -> Cow<'_, str> {
    display_path_label_cow(path)
}

fn inlay_hint_range_end(buffer: &kuroya_core::TextBuffer) -> (usize, usize) {
    let end_line = buffer.len_lines().saturating_sub(1);
    let end_character = lsp_line_content_utf16_len(buffer, end_line).unwrap_or_default();
    (end_line, end_character)
}

#[cfg(test)]
mod tests {
    use super::{
        inlay_hint_range_end, lsp_document_trace_path_label, lsp_document_version_trace_label,
        lsp_symbol_refresh_buffer_is_eligible, lsp_symbol_refresh_target_for_buffer,
    };
    use crate::{
        KuroyaApp, app_startup_context::AppStartupContext,
        lsp_runtime::LSP_SYMBOL_REFRESH_DEBOUNCE, lsp_runtime::due_lsp_symbol_refresh_ids,
        path_display::DISPLAY_PATH_LABEL_MAX_CHARS, terminal::TerminalPane,
    };
    use kuroya_core::{EditorSettings, TextBuffer, Workspace};
    use std::{
        fs,
        path::PathBuf,
        time::{Instant, SystemTime, UNIX_EPOCH},
    };
    use tokio::runtime::Runtime;

    #[test]
    fn inlay_hint_range_end_uses_utf16_line_length() {
        let buffer = TextBuffer::from_text(1, None, "alpha\n\u{1f600}x".to_owned());

        assert_eq!(inlay_hint_range_end(&buffer), (1, 3));
    }

    #[test]
    fn lsp_document_trace_labels_are_display_safe_and_bounded() {
        let path = PathBuf::from("workspace").join(format!(
            "sync\n{}\u{202e}.rs",
            "path-".repeat(DISPLAY_PATH_LABEL_MAX_CHARS)
        ));

        let path_label = lsp_document_trace_path_label(&path);
        let version_label = lsp_document_version_trace_label(&path, 42);

        for label in [path_label.as_ref(), version_label.trim_end_matches(" v42")] {
            assert!(!label.contains('\n'));
            assert!(!label.contains('\u{202e}'));
            assert!(label.contains("..."));
            assert!(label.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);
        }
        assert!(version_label.ends_with(" v42"));
    }

    #[test]
    fn open_buffer_symbol_refreshes_are_scheduled_due_immediately() {
        let root = temp_root("open-buffer-symbol-refreshes");
        let source = root.join("src").join("main.rs");
        let mut app = app_for_test(root.clone());
        app.buffers.push(TextBuffer::from_text(
            7,
            Some(source),
            "fn main() {}\n".to_owned(),
        ));
        app.buffers
            .push(TextBuffer::from_text(8, None, "fn helper() {}".to_owned()));

        assert_eq!(app.schedule_lsp_symbol_refreshes_for_open_buffers(), 1);
        assert_eq!(
            due_lsp_symbol_refresh_ids(
                &app.pending_lsp_symbol_refreshes,
                Instant::now(),
                LSP_SYMBOL_REFRESH_DEBOUNCE,
            ),
            vec![7]
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn open_buffer_symbol_refresh_scheduling_prunes_stale_pending_ids() {
        let root = temp_root("symbol-refresh-prune-stale");
        let source = root.join("src").join("main.rs");
        let mut app = app_for_test(root.clone());
        app.buffers.push(TextBuffer::from_text(
            7,
            Some(source),
            "fn main() {}\n".to_owned(),
        ));
        app.buffers
            .push(TextBuffer::from_text(8, None, "fn helper() {}".to_owned()));
        app.pending_lsp_symbol_refreshes.insert(99, Instant::now());

        assert_eq!(app.schedule_lsp_symbol_refreshes_for_open_buffers(), 1);

        assert!(app.pending_lsp_symbol_refreshes.contains_key(&7));
        assert!(!app.pending_lsp_symbol_refreshes.contains_key(&8));
        assert!(!app.pending_lsp_symbol_refreshes.contains_key(&99));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn lsp_symbol_refresh_target_keeps_raw_path_while_filtering_blocked_buffers() {
        let raw_path = PathBuf::from("workspace/src/raw\n\u{202e}.rs");
        let buffer = TextBuffer::from_text(7, Some(raw_path.clone()), "fn main() {}".to_owned());
        let lossy = std::collections::HashSet::new();
        let binary = std::collections::HashSet::new();

        assert!(lsp_symbol_refresh_buffer_is_eligible(
            &buffer, &lossy, &binary
        ));

        let (path, version) = lsp_symbol_refresh_target_for_buffer(&buffer, &lossy, &binary)
            .expect("path-backed text buffer is eligible");

        assert_eq!(path, raw_path);
        assert_eq!(version, buffer.version());

        let lossy = std::collections::HashSet::from([7]);
        assert!(!lsp_symbol_refresh_buffer_is_eligible(
            &buffer, &lossy, &binary
        ));
        assert!(lsp_symbol_refresh_target_for_buffer(&buffer, &lossy, &binary).is_none());

        let unbacked_buffer = TextBuffer::from_text(8, None, "fn helper() {}".to_owned());
        let lossy = std::collections::HashSet::new();
        assert!(!lsp_symbol_refresh_buffer_is_eligible(
            &unbacked_buffer,
            &lossy,
            &binary
        ));
        assert!(lsp_symbol_refresh_target_for_buffer(&unbacked_buffer, &lossy, &binary).is_none());
    }

    #[test]
    fn text_snapshot_for_version_rejects_stale_buffers() {
        let root = temp_root("stale-text-snapshot");
        let source = root.join("src").join("main.rs");
        let mut app = app_for_test(root.clone());
        app.buffers
            .push(TextBuffer::from_text(7, Some(source), "alpha".to_owned()));
        let version = app.buffer(7).expect("buffer").version();

        let snapshot = app
            .lsp_text_snapshot_for_version(7, version)
            .expect("current version snapshot");
        assert_eq!(snapshot.text(), "alpha");

        app.buffer_mut(7).expect("buffer").insert_at_cursor(" beta");

        assert!(app.lsp_text_snapshot_for_version(7, version).is_none());

        let _ = fs::remove_dir_all(root);
    }

    fn app_for_test(root: PathBuf) -> KuroyaApp {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let settings = EditorSettings::default();
        KuroyaApp::from_startup_context(AppStartupContext {
            runtime: Runtime::new().expect("test runtime"),
            tx,
            rx,
            workspace: Workspace::new(root.clone()),
            settings: settings.clone(),
            settings_panel_draft: settings,
            settings_editor_font_path: String::new(),
            settings_ui_font_path: String::new(),
            theme_picker_selected: 0,
            saved_session: None,
            terminal: TerminalPane::new(root.clone(), 100, 12.0, 1.2),
            watcher: None,
            recent_projects: Vec::new(),
            trusted_workspaces: vec![root],
            now: Instant::now(),
            startup_timings: Vec::new(),
        })
    }

    fn temp_root(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        std::env::temp_dir().join(format!(
            "kuroya-document-sync-{name}-{}-{nanos}",
            std::process::id()
        ))
    }
}
