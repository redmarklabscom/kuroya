use crate::{
    KuroyaApp,
    lsp_disk_edit_actions::LspDiskTextEditPlan,
    lsp_edits::buffer_text_edits_from_lsp,
    lsp_lifecycle::open_lsp_workspace_edit_block_reason,
    lsp_ui_events::{LspWorkspaceApplyEditDiskResponse, LspWorkspaceApplyEditResponseTarget},
    path_display::{DISPLAY_PATH_LABEL_MAX_CHARS, sanitized_display_label_cow},
    workspace_trust::workspace_path_contains_lexically,
};
use kuroya_core::{BufferId, LspTextEdit, TextEdit as BufferTextEdit};
use std::{
    borrow::Cow,
    collections::BTreeMap,
    fmt::Write as _,
    path::{Path, PathBuf},
};

const WORKSPACE_EDIT_STATUS_LABEL_MAX_CHARS: usize = DISPLAY_PATH_LABEL_MAX_CHARS;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct LspWorkspaceEditOutcome {
    pub(crate) open_changed: usize,
    pub(crate) open_skipped: usize,
    pub(crate) open_failed: usize,
    pub(crate) disk_queued: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LspWorkspaceEditTarget {
    pub(crate) resolved_path: PathBuf,
    pub(crate) open_buffer_id: Option<BufferId>,
}

#[derive(Debug, Default)]
struct LspWorkspaceEditGroup {
    open_buffer_id: Option<BufferId>,
    edits: Vec<LspTextEdit>,
}

enum PreparedLspWorkspaceEdit {
    Open {
        id: BufferId,
        edits: Vec<BufferTextEdit>,
    },
    Disk {
        path: PathBuf,
        edits: Vec<LspTextEdit>,
    },
}

impl LspWorkspaceEditOutcome {
    pub(crate) fn applied(&self) -> bool {
        self.open_skipped == 0 && self.open_failed == 0
    }

    pub(crate) fn failure_reason(&self) -> Option<String> {
        if self.open_failed > 0 {
            Some(format!(
                "{} open buffer edit(s) could not be applied",
                self.open_failed
            ))
        } else if self.open_skipped > 0 {
            Some(format!(
                "{} unsafe open buffer(s) skipped",
                self.open_skipped
            ))
        } else {
            None
        }
    }
}

impl KuroyaApp {
    pub(crate) fn resolve_lsp_workspace_edit_target(&self, path: &Path) -> LspWorkspaceEditTarget {
        if let Some(buffer) = self.buffer_by_lexical_path(path) {
            if let Some(buffer_path) = buffer.path() {
                return LspWorkspaceEditTarget {
                    resolved_path: buffer_path.clone(),
                    open_buffer_id: Some(buffer.id()),
                };
            }
        }

        LspWorkspaceEditTarget {
            resolved_path: path.to_path_buf(),
            open_buffer_id: None,
        }
    }

    pub(crate) fn apply_lsp_workspace_edits(
        &mut self,
        edits: Vec<LspTextEdit>,
        label: &str,
    ) -> LspWorkspaceEditOutcome {
        self.apply_lsp_workspace_edits_inner(edits, label, None)
    }

    pub(crate) fn apply_lsp_workspace_edits_for_apply_edit(
        &mut self,
        edits: Vec<LspTextEdit>,
        label: &str,
        response_target: LspWorkspaceApplyEditResponseTarget,
    ) -> LspWorkspaceEditOutcome {
        self.apply_lsp_workspace_edits_inner(edits, label, Some(response_target))
    }

    fn apply_lsp_workspace_edits_inner(
        &mut self,
        edits: Vec<LspTextEdit>,
        label: &str,
        response_target: Option<LspWorkspaceApplyEditResponseTarget>,
    ) -> LspWorkspaceEditOutcome {
        if !self.workspace_trusted {
            self.status = lsp_workspace_edit_restricted_status(label);
            return LspWorkspaceEditOutcome::default();
        }

        if edits.is_empty() {
            self.status = lsp_workspace_edit_no_edits_status(label);
            return LspWorkspaceEditOutcome::default();
        }

        let mut edits_by_path: BTreeMap<PathBuf, LspWorkspaceEditGroup> = BTreeMap::new();
        for edit in edits {
            let target = self.resolve_lsp_workspace_edit_target(&edit.path);
            let group = edits_by_path.entry(target.resolved_path).or_default();
            group.open_buffer_id = target.open_buffer_id;
            group.edits.push(edit);
        }

        let (mut outcome, prepared_edits) = self.prepare_lsp_workspace_edits(edits_by_path);
        if outcome.open_skipped > 0 || outcome.open_failed > 0 {
            self.status = lsp_workspace_edit_applied_status(label, outcome);
            return outcome;
        }

        let mut disk_edits = Vec::with_capacity(prepared_edits.len());
        for prepared in prepared_edits {
            match prepared {
                PreparedLspWorkspaceEdit::Open { id, edits } => {
                    let Some(buffer) = self.buffer_mut(id) else {
                        outcome.open_failed += 1;
                        continue;
                    };
                    let changed = buffer.apply_edits(edits);
                    if changed {
                        self.mark_buffer_changed(id);
                        outcome.open_changed += 1;
                    }
                }
                PreparedLspWorkspaceEdit::Disk { path, edits } => {
                    disk_edits.push(LspDiskTextEditPlan::capture(
                        &self.workspace.root,
                        path,
                        edits,
                    ));
                }
            }
        }

        outcome.disk_queued = disk_edits.len();
        if !disk_edits.is_empty() {
            let response = response_target.map(|target| LspWorkspaceApplyEditDiskResponse {
                target,
                open_failed: outcome.open_failed,
                open_skipped: outcome.open_skipped,
            });
            self.spawn_lsp_disk_edits(disk_edits, response);
        }
        self.status = lsp_workspace_edit_applied_status(label, outcome);
        self.spawn_git_auto_refresh();
        outcome
    }

    fn prepare_lsp_workspace_edits(
        &self,
        edits_by_path: BTreeMap<PathBuf, LspWorkspaceEditGroup>,
    ) -> (LspWorkspaceEditOutcome, Vec<PreparedLspWorkspaceEdit>) {
        let mut outcome = LspWorkspaceEditOutcome::default();
        let has_open_buffer_edits = edits_by_path
            .values()
            .any(|group| group.open_buffer_id.is_some());
        let changed_on_disk = if has_open_buffer_edits {
            self.observed_external_change_buffer_ids()
        } else {
            Default::default()
        };
        let mut prepared_edits = Vec::with_capacity(edits_by_path.len());
        for (path, group) in edits_by_path {
            if !lsp_workspace_edit_group_ranges_are_valid(&group.edits) {
                outcome.open_failed += 1;
                continue;
            }
            if !path_is_within_workspace(&self.workspace.root, &path) {
                outcome.open_failed += 1;
                continue;
            }

            if let Some(id) = group.open_buffer_id {
                if open_lsp_workspace_edit_block_reason(
                    id,
                    &changed_on_disk,
                    &self.lossy_decoded_buffers,
                    &self.binary_preview_buffers,
                    &self.buffers,
                )
                .is_some()
                {
                    outcome.open_skipped += 1;
                    continue;
                }
                let Some(buffer) = self.buffer(id) else {
                    outcome.open_failed += 1;
                    continue;
                };
                let Some(buffer_edits) = buffer_text_edits_from_lsp(buffer, &group.edits) else {
                    outcome.open_failed += 1;
                    continue;
                };
                prepared_edits.push(PreparedLspWorkspaceEdit::Open {
                    id,
                    edits: buffer_edits,
                });
            } else {
                prepared_edits.push(PreparedLspWorkspaceEdit::Disk {
                    path,
                    edits: group.edits,
                });
            }
        }

        (outcome, prepared_edits)
    }
}

fn lsp_workspace_edit_no_edits_status(label: &str) -> String {
    lsp_workspace_edit_status_with_suffix(label, ": no edits")
}

fn lsp_workspace_edit_restricted_status(label: &str) -> String {
    lsp_workspace_edit_status_with_suffix(label, ": workspace is restricted")
}

fn lsp_workspace_edit_applied_status(label: &str, outcome: LspWorkspaceEditOutcome) -> String {
    let label = lsp_workspace_edit_status_label_cow(label);
    let mut status = String::with_capacity(label.len() + 96);
    status.push_str(label.as_ref());
    let _ = write!(status, ": changed {} open buffers", outcome.open_changed);
    if outcome.disk_queued > 0 {
        let _ = write!(status, ", queued {} files on disk", outcome.disk_queued);
    }
    if outcome.open_skipped > 0 {
        let _ = write!(
            status,
            ", skipped {} unsafe open buffers",
            outcome.open_skipped
        );
    }
    if outcome.open_failed > 0 {
        let _ = write!(status, ", failed {} open buffers", outcome.open_failed);
    }
    status
}

fn lsp_workspace_edit_status_with_suffix(label: &str, suffix: &str) -> String {
    let label = lsp_workspace_edit_status_label_cow(label);
    let mut status = String::with_capacity(label.len() + suffix.len());
    status.push_str(label.as_ref());
    status.push_str(suffix);
    status
}

fn lsp_workspace_edit_status_label_cow(label: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(
        label,
        WORKSPACE_EDIT_STATUS_LABEL_MAX_CHARS,
        "LSP workspace edit",
    )
}

fn path_is_within_workspace(root: &Path, path: &Path) -> bool {
    workspace_path_contains_lexically(root, path)
}

fn lsp_workspace_edit_group_ranges_are_valid(edits: &[LspTextEdit]) -> bool {
    edits.iter().all(lsp_workspace_edit_range_is_valid)
}

fn lsp_workspace_edit_range_is_valid(edit: &LspTextEdit) -> bool {
    edit.start_line > 0
        && edit.start_column > 0
        && edit.end_line > 0
        && edit.end_column > 0
        && (edit.end_line > edit.start_line
            || (edit.end_line == edit.start_line && edit.end_column >= edit.start_column))
}

#[cfg(test)]
mod tests {
    use super::{
        LspWorkspaceEditOutcome, WORKSPACE_EDIT_STATUS_LABEL_MAX_CHARS,
        lsp_workspace_edit_applied_status, lsp_workspace_edit_no_edits_status,
        lsp_workspace_edit_range_is_valid, lsp_workspace_edit_restricted_status,
        lsp_workspace_edit_status_label_cow,
    };
    use crate::{KuroyaApp, app_startup_context::AppStartupContext, terminal::TerminalPane};
    use kuroya_core::{EditorSettings, LspTextEdit, TextBuffer, Workspace};
    use std::{
        borrow::Cow,
        path::{Path, PathBuf},
        time::Instant,
    };
    use tokio::runtime::Runtime;

    #[test]
    fn lsp_workspace_edit_status_label_cow_borrows_clean_provider_labels() {
        assert!(matches!(
            lsp_workspace_edit_status_label_cow("rust-analyzer"),
            Cow::Borrowed("rust-analyzer")
        ));

        let unicode = "lsp-\u{03bb}-provider";
        match lsp_workspace_edit_status_label_cow(unicode) {
            Cow::Borrowed(label) => assert_eq!(label, unicode),
            Cow::Owned(label) => panic!("expected borrowed provider label, got {label:?}"),
        }
    }

    #[test]
    fn lsp_workspace_edit_status_label_cow_owns_dirty_truncated_and_fallback_labels() {
        let dirty = lsp_workspace_edit_status_label_cow("Rename\nprovider\u{202e}");
        assert_owned_cow_eq(dirty, "Rename provider");

        let long = format!(
            "very-long-provider-label-{}",
            "x".repeat(WORKSPACE_EDIT_STATUS_LABEL_MAX_CHARS * 2)
        );
        let truncated = lsp_workspace_edit_status_label_cow(&long);
        assert!(matches!(&truncated, Cow::Owned(_)));
        assert!(truncated.contains("..."), "{truncated}");
        assert!(truncated.chars().count() <= WORKSPACE_EDIT_STATUS_LABEL_MAX_CHARS);

        let fallback = lsp_workspace_edit_status_label_cow("\n\u{202e}\t");
        assert_owned_cow_eq(fallback, "LSP workspace edit");
    }

    #[test]
    fn lsp_workspace_edit_status_wording_is_unchanged() {
        let outcome = LspWorkspaceEditOutcome {
            open_changed: 2,
            open_skipped: 1,
            open_failed: 1,
            disk_queued: 3,
        };

        assert_eq!(
            lsp_workspace_edit_applied_status("Rename provider", outcome),
            "Rename provider: changed 2 open buffers, queued 3 files on disk, skipped 1 unsafe open buffers, failed 1 open buffers"
        );
        assert_eq!(
            lsp_workspace_edit_no_edits_status("Rename provider"),
            "Rename provider: no edits"
        );
        assert_eq!(
            lsp_workspace_edit_restricted_status("Rename provider"),
            "Rename provider: workspace is restricted"
        );
    }

    #[test]
    fn lsp_workspace_edit_status_sanitizes_and_bounds_provider_label() {
        let label = format!(
            "Rename\n{}\u{202e}target",
            "very-long-provider-label-".repeat(WORKSPACE_EDIT_STATUS_LABEL_MAX_CHARS)
        );
        let outcome = LspWorkspaceEditOutcome {
            open_changed: 2,
            open_skipped: 1,
            open_failed: 1,
            disk_queued: 3,
        };

        let applied = lsp_workspace_edit_applied_status(&label, outcome);
        let no_edits = lsp_workspace_edit_no_edits_status(&label);
        let restricted = lsp_workspace_edit_restricted_status(&label);

        for status in [applied, no_edits, restricted] {
            assert_safe_status_text(&status);
            assert!(status.contains("Rename"), "{status}");
            assert!(status.contains("..."), "{status}");
            assert!(
                status.chars().count()
                    <= WORKSPACE_EDIT_STATUS_LABEL_MAX_CHARS
                        + ": changed 2 open buffers, queued 3 files on disk, skipped 1 unsafe open buffers, failed 1 open buffers"
                            .chars()
                            .count()
            );
        }
    }

    #[test]
    fn lsp_workspace_edit_status_uses_fallback_for_blank_provider_label() {
        assert_eq!(
            lsp_workspace_edit_no_edits_status("\n\u{202e}\t"),
            "LSP workspace edit: no edits"
        );
        assert_eq!(
            lsp_workspace_edit_restricted_status("\n\u{202e}\t"),
            "LSP workspace edit: workspace is restricted"
        );
    }

    #[test]
    fn workspace_edit_prevalidates_open_ranges_before_mutating_any_buffer() {
        let root = PathBuf::from("workspace");
        let first_path = root.join("src/a.rs");
        let second_path = root.join("src/b.rs");
        let mut app = app_for_test(root);
        app.buffers.push(TextBuffer::from_text(
            7,
            Some(first_path.clone()),
            "first\n".to_owned(),
        ));
        app.buffers.push(TextBuffer::from_text(
            8,
            Some(second_path.clone()),
            "second\n".to_owned(),
        ));

        let outcome = app.apply_lsp_workspace_edits(
            vec![
                edit(&first_path, "changed first\n"),
                invalid_range_edit(&second_path),
            ],
            "Batch edit",
        );

        assert_eq!(
            outcome,
            LspWorkspaceEditOutcome {
                open_changed: 0,
                open_skipped: 0,
                open_failed: 1,
                disk_queued: 0,
            }
        );
        assert_eq!(app.buffer(7).expect("buffer").text(), "first\n");
        assert_eq!(app.buffer(8).expect("buffer").text(), "second\n");
        assert_eq!(
            app.status,
            "Batch edit: changed 0 open buffers, failed 1 open buffers"
        );
    }

    #[test]
    fn workspace_edit_prevalidation_failure_prevents_disk_queue() {
        let root = PathBuf::from("workspace");
        let open_path = root.join("src/open.rs");
        let disk_path = root.join("src/disk.rs");
        let mut app = app_for_test(root);
        app.buffers.push(TextBuffer::from_text(
            7,
            Some(open_path.clone()),
            "open\n".to_owned(),
        ));

        let outcome = app.apply_lsp_workspace_edits(
            vec![invalid_range_edit(&open_path), edit(&disk_path, "disk\n")],
            "Mixed edit",
        );

        assert_eq!(outcome.open_changed, 0);
        assert_eq!(outcome.open_failed, 1);
        assert_eq!(outcome.disk_queued, 0);
        assert_eq!(app.buffer(7).expect("buffer").text(), "open\n");
    }

    #[test]
    fn workspace_edit_prevalidates_disk_ranges_before_queueing() {
        let root = PathBuf::from("workspace");
        let disk_path = root.join("src/disk.rs");
        let mut app = app_for_test(root);

        let invalid = reversed_range_edit(&disk_path);
        assert!(!lsp_workspace_edit_range_is_valid(&invalid));

        let outcome = app.apply_lsp_workspace_edits(vec![invalid], "Disk edit");

        assert_eq!(
            outcome,
            LspWorkspaceEditOutcome {
                open_changed: 0,
                open_skipped: 0,
                open_failed: 1,
                disk_queued: 0,
            }
        );
        assert_eq!(
            app.status,
            "Disk edit: changed 0 open buffers, failed 1 open buffers"
        );
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

    fn assert_owned_cow_eq(value: Cow<'_, str>, expected: &str) {
        match value {
            Cow::Owned(label) => assert_eq!(label, expected),
            Cow::Borrowed(label) => panic!("expected owned provider label, got {label:?}"),
        }
    }

    fn edit(path: &Path, new_text: &str) -> LspTextEdit {
        LspTextEdit {
            path: path.to_path_buf(),
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 1,
            new_text: new_text.to_owned(),
        }
    }

    fn invalid_range_edit(path: &Path) -> LspTextEdit {
        LspTextEdit {
            path: path.to_path_buf(),
            start_line: 99,
            start_column: 1,
            end_line: 99,
            end_column: 1,
            new_text: "invalid\n".to_owned(),
        }
    }

    fn reversed_range_edit(path: &Path) -> LspTextEdit {
        LspTextEdit {
            path: path.to_path_buf(),
            start_line: 3,
            start_column: 5,
            end_line: 3,
            end_column: 2,
            new_text: "invalid\n".to_owned(),
        }
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
}
