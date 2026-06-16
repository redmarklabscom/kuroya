use crate::{
    KuroyaApp,
    folding::{
        fallback_folding_ranges, fold_import_ranges_by_default,
        retain_folded_ranges_matching_folding_ranges,
    },
    lsp_lifecycle::background_language_block_reason,
    lsp_runtime::lsp_command_queue_failed_status,
    path_display::{compact_path, display_error_label_cow, display_path_label_cow},
    syntax_tree_cache::{
        SyntaxTreeUnavailableReason, TreeSitterSyntaxCache, syntax_tree_unavailable_reason,
    },
    ui_events::UiEvent,
};
use kuroya_core::{BufferId, LspFoldingRange, TextBuffer, clamp_editor_folding_maximum_regions};
use std::path::{Path, PathBuf};

mod apply;
mod toggle;

impl KuroyaApp {
    pub(crate) fn request_lsp_folding_ranges(&mut self) {
        let Some((id, path, _, _, _)) = self.active_lsp_position() else {
            self.status = "No LSP folding target".to_owned();
            return;
        };
        self.request_lsp_folding_ranges_for(id, path);
    }

    pub(crate) fn request_lsp_folding_ranges_for(&mut self, id: BufferId, path: PathBuf) -> bool {
        let Some(buffer) = self
            .buffer(id)
            .filter(|buffer| buffer.path() == Some(&path))
        else {
            self.status = "No matching buffer for LSP folds".to_owned();
            return false;
        };
        if let Some(reason) = background_language_block_reason(
            id,
            buffer,
            &self.lossy_decoded_buffers,
            &self.binary_preview_buffers,
        ) {
            self.clear_pending_fold_line_for_path(&path);
            self.status = reason.folding_status().to_owned();
            return false;
        }
        let version = buffer.version();
        let Some(client) = self.ensure_lsp_for_buffer(id) else {
            return self.load_fallback_folding_ranges_for(id, path);
        };

        if !client.folding_ranges(id, path.clone(), version) {
            self.status = lsp_command_queue_failed_status("textDocument/foldingRange");
            return self.load_fallback_folding_ranges_for(id, path);
        }
        self.status = loading_folding_ranges_status(&path);
        true
    }

    pub(crate) fn load_fallback_folding_ranges_for(&mut self, id: BufferId, path: PathBuf) -> bool {
        let Some(buffer_index) = self
            .buffers
            .iter()
            .position(|buffer| buffer.id() == id && buffer.path() == Some(&path))
        else {
            self.status = "No matching buffer for fallback folds".to_owned();
            return false;
        };
        {
            let buffer = &self.buffers[buffer_index];
            if let Some(reason) = background_language_block_reason(
                id,
                buffer,
                &self.lossy_decoded_buffers,
                &self.binary_preview_buffers,
            ) {
                self.clear_pending_fold_line_for_path(&path);
                self.status = reason.folding_status().to_owned();
                return false;
            }
        }
        let buffer = self.buffers[buffer_index].clone();
        let version = buffer.version();
        let tx = self.tx.clone();
        self.status = loading_fallback_folding_ranges_status(&path);
        self.record_async_task_started("Fallback Folding", compact_path(&path));
        self.runtime.spawn_blocking(move || {
            let ranges = compute_fallback_folding_ranges(buffer);
            let _ = crate::ui_event_channel::send_ui_event(
                &tx,
                UiEvent::FallbackFoldingRangesLoaded {
                    id,
                    path,
                    version,
                    ranges,
                },
            );
        });
        true
    }

    pub(crate) fn apply_fallback_folding_ranges_loaded(
        &mut self,
        id: BufferId,
        path: PathBuf,
        version: u64,
        ranges: Vec<LspFoldingRange>,
    ) {
        let Some(buffer) = self.buffers.iter().find(|buffer| {
            buffer.id() == id
                && buffer.path().is_some_and(|candidate| candidate == &path)
                && buffer.version() == version
        }) else {
            return;
        };
        let source = FoldingRangeSource::fallback_for_buffer(buffer);
        let ranges = valid_folding_ranges_for_buffer(buffer, ranges);
        self.apply_folding_ranges_for_path(path, ranges, source);
    }

    pub(crate) fn apply_folding_ranges_for_path(
        &mut self,
        path: PathBuf,
        mut ranges: Vec<LspFoldingRange>,
        source: FoldingRangeSource,
    ) {
        normalize_loaded_folding_ranges(&mut ranges);
        ranges.truncate(clamp_editor_folding_maximum_regions(
            self.settings.folding_maximum_regions,
        ));
        let count = ranges.len();
        let remove_folded_entry = if let Some(folded) = self.folded_ranges.get_mut(&path) {
            retain_folded_ranges_matching_folding_ranges(folded, &ranges);
            folded.is_empty()
        } else {
            false
        };
        if remove_folded_entry {
            self.folded_ranges.remove(&path);
        }
        self.folding_ranges.insert(path.clone(), ranges);

        let handled_pending = if let Some(line) = self
            .pending_fold_line
            .as_ref()
            .and_then(|(pending_path, line)| (pending_path == &path).then_some(*line))
        {
            self.pending_fold_line = None;
            self.apply_fold_at_line(&path, line);
            true
        } else {
            false
        };
        self.fold_imports_by_default_for_path(&path);

        if handled_pending {
            return;
        }

        self.status = source.status(&path, count);
    }

    fn clear_pending_fold_line_for_path(&mut self, path: &Path) {
        if self
            .pending_fold_line
            .as_ref()
            .is_some_and(|(pending_path, _)| pending_path == path)
        {
            self.pending_fold_line = None;
        }
    }

    pub(crate) fn clear_folding_state_for_path(&mut self, path: &Path) {
        self.folding_ranges.remove(path);
        self.folded_ranges.remove(path);
        self.clear_pending_fold_line_for_path(path);
    }

    pub(crate) fn fold_imports_by_default_for_path(&mut self, path: &Path) -> usize {
        if !self.settings.folding_imports_by_default {
            return 0;
        }
        let Some(ranges) = self.folding_ranges.get(path) else {
            return 0;
        };
        if self.folded_ranges.contains_key(path) {
            let Some(folded) = self.folded_ranges.get_mut(path) else {
                return 0;
            };
            let added = fold_import_ranges_by_default(folded, ranges, true);
            let remove_entry = folded.is_empty();
            if remove_entry {
                self.folded_ranges.remove(path);
            }
            return added;
        }

        let mut folded = Vec::new();
        let added = fold_import_ranges_by_default(&mut folded, ranges, true);
        if !folded.is_empty() {
            self.folded_ranges.insert(path.to_path_buf(), folded);
        }
        added
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FoldingRangeSource {
    Lsp,
    Fallback,
    FallbackSyntaxUnavailable(SyntaxTreeUnavailableReason),
}

impl FoldingRangeSource {
    fn fallback_for_buffer(buffer: &TextBuffer) -> Self {
        syntax_tree_unavailable_reason(buffer)
            .map(Self::FallbackSyntaxUnavailable)
            .unwrap_or(Self::Fallback)
    }

    fn status(self, path: &Path, count: usize) -> String {
        let path = display_path_label_cow(path);
        match (self, count) {
            (Self::Lsp, 0) => format!("No folding ranges in {path}"),
            (Self::Lsp, count) => format!("{count} folding ranges in {path}"),
            (Self::Fallback, 0) => {
                format!("No fallback folding ranges in {path}")
            }
            (Self::Fallback, count) => {
                format!("{count} fallback folding ranges in {path}")
            }
            (Self::FallbackSyntaxUnavailable(reason), 0) => format!(
                "No fallback folding ranges in {path} ({})",
                syntax_tree_unavailable_status(reason)
            ),
            (Self::FallbackSyntaxUnavailable(reason), count) => format!(
                "{count} fallback folding ranges in {path} ({})",
                syntax_tree_unavailable_status(reason)
            ),
        }
    }
}

fn loading_folding_ranges_status(path: &Path) -> String {
    format!(
        "Loading folding ranges for {}",
        display_path_label_cow(path)
    )
}

fn loading_fallback_folding_ranges_status(path: &Path) -> String {
    format!(
        "Loading fallback folding ranges for {}",
        display_path_label_cow(path)
    )
}

fn syntax_tree_unavailable_status(reason: SyntaxTreeUnavailableReason) -> String {
    let status = match reason {
        SyntaxTreeUnavailableReason::UnsupportedLanguage(language) => {
            format!("tree-sitter unavailable: {language:?} unsupported")
        }
        SyntaxTreeUnavailableReason::LargeFileMode => {
            "tree-sitter skipped in large file mode".to_owned()
        }
        SyntaxTreeUnavailableReason::ParseByteBudget { bytes, max_bytes } => {
            format!("tree-sitter skipped: parse byte budget exceeded ({bytes}/{max_bytes} bytes)")
        }
        SyntaxTreeUnavailableReason::ParseLineBudget { lines, max_lines } => {
            format!("tree-sitter skipped: parse line budget exceeded ({lines}/{max_lines} lines)")
        }
    };
    folding_provider_status_label(&status)
}

fn folding_provider_status_label(status: &str) -> String {
    display_error_label_cow(status).into_owned()
}

fn compute_fallback_folding_ranges(buffer: TextBuffer) -> Vec<LspFoldingRange> {
    TreeSitterSyntaxCache::default()
        .folding_ranges_for_buffer(&buffer)
        .unwrap_or_else(|| fallback_folding_ranges(&buffer))
}

pub(crate) fn valid_folding_ranges_for_buffer(
    buffer: &TextBuffer,
    mut ranges: Vec<LspFoldingRange>,
) -> Vec<LspFoldingRange> {
    let line_count = buffer.len_lines();
    ranges.retain(|range| folding_range_within_buffer(range, line_count));
    normalize_loaded_folding_ranges(&mut ranges);
    ranges
}

fn folding_range_within_buffer(range: &LspFoldingRange, line_count: usize) -> bool {
    range.start_line > 0 && range.end_line > range.start_line && range.end_line <= line_count
}

fn normalize_loaded_folding_ranges(ranges: &mut Vec<LspFoldingRange>) {
    ranges.retain(|range| range.start_line > 0 && range.end_line > range.start_line);
    ranges.sort_unstable_by(|left, right| {
        left.start_line
            .cmp(&right.start_line)
            .then(left.end_line.cmp(&right.end_line))
            .then(left.start_column.cmp(&right.start_column))
            .then(left.end_column.cmp(&right.end_column))
            .then(left.kind.cmp(&right.kind))
    });
    ranges.dedup();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app_startup_context::AppStartupContext, folding::FoldedRange, terminal::TerminalPane,
    };
    use crate::{
        path_display::{DISPLAY_ERROR_LABEL_MAX_CHARS, DISPLAY_PATH_LABEL_MAX_CHARS},
        syntax_tree_cache::SYNTAX_TREE_MAX_BYTES,
    };
    use kuroya_core::{EditorSettings, LanguageId, Workspace};
    use std::{
        path::{Path, PathBuf},
        time::Instant,
    };
    use tokio::runtime::Runtime;

    #[test]
    fn fallback_folding_worker_uses_tree_sitter_ranges_for_rust() {
        let buffer = TextBuffer::from_text_with_language(
            1,
            None,
            "use crate::{\n    first,\n    second,\n};\nfn main() {}\n".to_owned(),
            LanguageId::Rust,
        );

        let ranges = compute_fallback_folding_ranges(buffer);

        assert!(ranges.iter().any(|range| {
            range.start_line == 1 && range.end_line == 4 && range.kind.as_deref() == Some("imports")
        }));
    }

    #[test]
    fn fallback_folding_worker_groups_single_line_rust_imports() {
        let buffer = TextBuffer::from_text_with_language(
            2,
            None,
            "use std::fs;\nuse std::path::Path;\n\nfn main() {}\n".to_owned(),
            LanguageId::Rust,
        );

        let ranges = compute_fallback_folding_ranges(buffer);

        assert!(ranges.iter().any(|range| {
            range.start_line == 1 && range.end_line == 2 && range.kind.as_deref() == Some("imports")
        }));
    }

    #[test]
    fn fallback_folding_status_names_unsupported_tree_sitter_language() {
        let root = std::env::temp_dir().join("kuroya-unsupported-tree-sitter-folding-status");
        let path = root.join("src/main.py");
        let mut app = app_for_test(root);
        let buffer = TextBuffer::from_text_with_language(
            7,
            Some(path.clone()),
            "if ready:\n    call()\n".to_owned(),
            LanguageId::Python,
        );
        let version = buffer.version();
        app.buffers.push(buffer);

        app.apply_fallback_folding_ranges_loaded(
            7,
            path.clone(),
            version,
            vec![folding_range(1, 2)],
        );

        assert_eq!(
            app.status,
            "1 fallback folding ranges in main.py (tree-sitter unavailable: Python unsupported)"
        );
    }

    #[test]
    fn fallback_folding_status_names_tree_sitter_parse_budget() {
        let root = std::env::temp_dir().join("kuroya-tree-sitter-budget-folding-status");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        let text = format!("{}\n", "x".repeat(SYNTAX_TREE_MAX_BYTES + 1));
        let text_len = text.len();
        let buffer =
            TextBuffer::from_text_with_language(7, Some(path.clone()), text, LanguageId::Rust);
        let version = buffer.version();
        app.buffers.push(buffer);

        app.apply_fallback_folding_ranges_loaded(
            7,
            path.clone(),
            version,
            vec![folding_range(1, 2)],
        );

        assert_eq!(
            app.status,
            format!(
                "1 fallback folding ranges in main.rs (tree-sitter skipped: parse byte budget exceeded ({text_len}/{SYNTAX_TREE_MAX_BYTES} bytes))"
            )
        );
    }

    #[test]
    fn folding_path_statuses_sanitize_and_bound_display_labels() {
        let root = std::env::temp_dir().join("kuroya-folding-status-path-safety");
        let path = unsafe_path_under(&root);
        let mut app = app_for_test(root.clone());
        app.folding_ranges
            .insert(path.clone(), vec![folding_range(1, 2)]);

        assert_raw_path_keeps_unsafe_text(&path);

        let loading = loading_folding_ranges_status(&path);
        assert_safe_status_text(&loading);
        assert!(loading.contains("..."));
        assert!(
            loading.chars().count()
                <= "Loading folding ranges for ".chars().count() + DISPLAY_PATH_LABEL_MAX_CHARS
        );

        let fallback_loading = loading_fallback_folding_ranges_status(&path);
        assert_safe_status_text(&fallback_loading);
        assert!(fallback_loading.contains("..."));
        assert!(
            fallback_loading.chars().count()
                <= "Loading fallback folding ranges for ".chars().count()
                    + DISPLAY_PATH_LABEL_MAX_CHARS
        );

        let loaded = FoldingRangeSource::Lsp.status(&path, 2);
        assert_safe_status_text(&loaded);
        assert!(loaded.contains("..."));
        assert!(
            loaded.chars().count()
                <= "2 folding ranges in ".chars().count() + DISPLAY_PATH_LABEL_MAX_CHARS
        );

        assert!(app.apply_fold_at_line(&path, 1));
        assert_safe_status_text(&app.status);
        assert!(app.status.contains("..."));
        assert!(app.status.ends_with(":1"));
        assert!(
            app.status.chars().count()
                <= "Folded 1 lines at ".chars().count() + DISPLAY_PATH_LABEL_MAX_CHARS + 2
        );

        let mut app = app_for_test(root);
        app.buffers.push(TextBuffer::from_text(
            9,
            Some(path.clone()),
            "alpha\nbeta\n".to_owned(),
        ));
        app.folded_ranges.insert(
            path.clone(),
            vec![FoldedRange {
                start_line: 1,
                end_line: 2,
            }],
        );

        app.toggle_fold_at_line(9, 1);

        assert_safe_status_text(&app.status);
        assert!(app.status.contains("..."));
        assert!(app.status.ends_with(":1"));
        assert!(
            app.status.chars().count()
                <= "Expanded fold at ".chars().count() + DISPLAY_PATH_LABEL_MAX_CHARS + 2
        );
    }

    #[test]
    fn folding_provider_status_sanitizes_and_bounds_error_labels() {
        let error = format!(
            "provider failed\nwhile folding \u{202e}{}tail",
            "very-long-error-".repeat(DISPLAY_ERROR_LABEL_MAX_CHARS)
        );

        let status = folding_provider_status_label(&error);

        assert!(error.contains('\n'));
        assert!(error.contains('\u{202e}'));
        assert_safe_status_text(&status);
        assert!(status.contains("..."));
        assert!(status.chars().count() <= DISPLAY_ERROR_LABEL_MAX_CHARS);
    }

    #[test]
    fn stale_fallback_folding_result_is_ignored() {
        let root = std::env::temp_dir().join("kuroya-stale-fallback-folding-test");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        app.buffers.push(TextBuffer::from_text_with_language(
            7,
            Some(path.clone()),
            "fn main() {\n}\n".to_owned(),
            LanguageId::Rust,
        ));

        app.apply_fallback_folding_ranges_loaded(7, path.clone(), 1, vec![folding_range(1, 2)]);

        assert!(!app.folding_ranges.contains_key(&path));
    }

    #[test]
    fn fallback_folding_result_applies_pending_fold() {
        let root = std::env::temp_dir().join("kuroya-pending-fallback-folding-test");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        app.buffers.push(TextBuffer::from_text_with_language(
            9,
            Some(path.clone()),
            "fn main() {\n    call();\n}\n".to_owned(),
            LanguageId::Rust,
        ));
        app.pending_fold_line = Some((path.clone(), 1));

        app.apply_fallback_folding_ranges_loaded(9, path.clone(), 0, vec![folding_range(1, 3)]);

        assert!(app.pending_fold_line.is_none());
        assert_eq!(
            app.folded_ranges.get(&path).map(Vec::as_slice),
            Some(
                &[FoldedRange {
                    start_line: 1,
                    end_line: 3,
                }][..]
            )
        );
    }

    #[test]
    fn loaded_folding_ranges_are_capped_by_settings_before_storage() {
        let root = std::env::temp_dir().join("kuroya-capped-folding-ranges-test");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        app.settings.folding_maximum_regions = 2;

        app.apply_folding_ranges_for_path(
            path.clone(),
            vec![
                folding_range(1, 3),
                folding_range(4, 6),
                folding_range(7, 9),
            ],
            FoldingRangeSource::Lsp,
        );

        assert_eq!(
            app.folding_ranges.get(&path).map(Vec::as_slice),
            Some(&[folding_range(1, 3), folding_range(4, 6)][..])
        );
        assert_eq!(app.status, "2 folding ranges in main.rs");
    }

    #[test]
    fn loaded_folding_ranges_prune_stale_folded_ranges_after_capping() {
        let root = std::env::temp_dir().join("kuroya-pruned-folded-ranges-test");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        app.settings.folding_maximum_regions = 2;
        app.folded_ranges.insert(
            path.clone(),
            vec![
                FoldedRange {
                    start_line: 1,
                    end_line: 3,
                },
                FoldedRange {
                    start_line: 4,
                    end_line: 6,
                },
                FoldedRange {
                    start_line: 7,
                    end_line: 9,
                },
            ],
        );

        app.apply_folding_ranges_for_path(
            path.clone(),
            vec![
                folding_range(1, 3),
                folding_range(4, 6),
                folding_range(7, 9),
            ],
            FoldingRangeSource::Lsp,
        );

        assert_eq!(
            app.folded_ranges.get(&path).map(Vec::as_slice),
            Some(
                &[
                    FoldedRange {
                        start_line: 1,
                        end_line: 3,
                    },
                    FoldedRange {
                        start_line: 4,
                        end_line: 6,
                    },
                ][..]
            )
        );

        app.apply_folding_ranges_for_path(path.clone(), Vec::new(), FoldingRangeSource::Lsp);

        assert!(!app.folded_ranges.contains_key(&path));
    }

    #[test]
    fn loaded_folding_ranges_remap_folded_state_for_unambiguous_changed_range() {
        let root = std::env::temp_dir().join("kuroya-remapped-folded-ranges-test");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        app.folded_ranges.insert(
            path.clone(),
            vec![
                FoldedRange {
                    start_line: 1,
                    end_line: 3,
                },
                FoldedRange {
                    start_line: 6,
                    end_line: 8,
                },
            ],
        );

        app.apply_folding_ranges_for_path(
            path.clone(),
            vec![
                folding_range(1, 4),
                folding_range(6, 9),
                folding_range(6, 10),
            ],
            FoldingRangeSource::Lsp,
        );

        assert_eq!(
            app.folded_ranges.get(&path).map(Vec::as_slice),
            Some(
                &[FoldedRange {
                    start_line: 1,
                    end_line: 4,
                }][..]
            )
        );
    }

    #[test]
    fn valid_folding_ranges_filter_out_of_buffer_ranges() {
        let buffer = TextBuffer::from_text(7, None, "alpha\nbeta".to_owned());

        let ranges = valid_folding_ranges_for_buffer(
            &buffer,
            vec![
                folding_range(2, 4),
                folding_range(1, 2),
                folding_range(0, 1),
                folding_range(2, 2),
                folding_range(1, 2),
            ],
        );

        assert_eq!(ranges, vec![folding_range(1, 2)]);
    }

    #[test]
    fn fallback_folding_result_filters_invalid_ranges_before_storage() {
        let root = std::env::temp_dir().join("kuroya-invalid-fallback-folding-test");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        let buffer = TextBuffer::from_text_with_language(
            7,
            Some(path.clone()),
            "alpha\nbeta".to_owned(),
            LanguageId::Rust,
        );
        let version = buffer.version();
        app.buffers.push(buffer);

        app.apply_fallback_folding_ranges_loaded(
            7,
            path.clone(),
            version,
            vec![folding_range(1, 3), folding_range(1, 2)],
        );

        assert_eq!(
            app.folding_ranges.get(&path).map(Vec::as_slice),
            Some(&[folding_range(1, 2)][..])
        );
        assert_eq!(app.status, "1 fallback folding ranges in main.rs");
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

    fn folding_range(start_line: usize, end_line: usize) -> LspFoldingRange {
        LspFoldingRange {
            start_line,
            start_column: None,
            end_line,
            end_column: None,
            kind: None,
        }
    }

    fn unsafe_path_under(root: &Path) -> PathBuf {
        root.join(format!(
            "fold\n{}\u{202e}tail.rs",
            "very-long-path-".repeat(DISPLAY_PATH_LABEL_MAX_CHARS)
        ))
    }

    fn assert_raw_path_keeps_unsafe_text(path: &Path) {
        let raw = path.to_string_lossy();
        assert!(raw.contains('\n'));
        assert!(raw.contains('\u{202e}'));
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
}
