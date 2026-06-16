use crate::{
    KuroyaApp,
    lsp_rename_requests::lsp_rename_request_target,
    path_display::{display_error_label_cow, display_path_label_cow},
    workspace_state::{
        active_buffer_lsp_position_matches, lsp_event_path_is_current, paths_match_lexically,
    },
};
use kuroya_core::{BufferId, LspReference, LspTextEdit};
use std::path::{Path, PathBuf};

impl KuroyaApp {
    pub(crate) fn handle_lsp_references_result(
        &mut self,
        id: BufferId,
        path: PathBuf,
        version: u64,
        line: usize,
        column: usize,
        references: Option<Vec<LspReference>>,
        error: Option<String>,
    ) {
        if self.active != Some(id)
            || !lsp_event_path_is_current(&self.workspace.root, &path)
            || !active_buffer_lsp_position_matches(
                self.active_buffer(),
                &path,
                version,
                line,
                column,
            )
            || !pending_lsp_references_request_matches(
                self.references_open,
                self.references_path.as_deref(),
                self.references_line,
                self.references_column,
                &path,
                line,
                column,
            )
        {
            return;
        }
        if let Some(error) = error {
            self.references_open = false;
            self.references.clear();
            self.status = references_failed_status(&error);
        } else if let Some(mut references) = references {
            retain_workspace_references(&self.workspace.root, &mut references);
            retain_valid_references(&mut references);
            let count = references.len();
            let display_line = line.saturating_add(1);
            let same_target = self.references_path.as_ref() == Some(&path)
                && self.references_line == display_line
                && self.references_column == column;
            self.references_open = true;
            if self.references_path.as_ref() != Some(&path) {
                self.references_path = Some(path);
            }
            self.references_line = display_line;
            self.references_column = column;
            if !same_target || self.references != references {
                self.references = references;
                self.references_selected = 0;
            } else if self.references_selected >= count {
                self.references_selected = count.saturating_sub(1);
            }
            self.status = if count == 0 {
                format!("No references at {display_line}:{column}")
            } else {
                format!("{count} references at {display_line}:{column}")
            };
        } else {
            self.references_open = false;
            self.references.clear();
            self.status = references_load_failed_status(&path);
        }
    }

    pub(crate) fn handle_lsp_rename_result(
        &mut self,
        id: BufferId,
        origin_path: PathBuf,
        version: u64,
        origin_line: usize,
        origin_column: usize,
        new_name: String,
        edits: Option<Vec<LspTextEdit>>,
        error: Option<String>,
    ) {
        if self.active != Some(id)
            || !lsp_event_path_is_current(&self.workspace.root, &origin_path)
            || !active_buffer_lsp_position_matches(
                self.active_buffer(),
                &origin_path,
                version,
                origin_line,
                origin_column,
            )
            || !pending_lsp_rename_request_matches(
                self.lsp_rename_open,
                &self.lsp_rename_input,
                &new_name,
            )
        {
            return;
        }
        self.lsp_rename_input.clear();
        if let Some(error) = error {
            self.clear_lsp_rename_preview_state();
            self.status = rename_failed_status(&error);
        } else if let Some(edits) = edits {
            self.lsp_hover = None;
            self.open_lsp_rename_preview(new_name, edits);
        } else {
            self.clear_lsp_rename_preview_state();
            self.status = rename_location_failed_status(&origin_path, origin_line, origin_column);
        }
    }
}

fn retain_workspace_references(workspace_root: &Path, references: &mut Vec<LspReference>) {
    if references
        .iter()
        .all(|reference| lsp_event_path_is_current(workspace_root, &reference.path))
    {
        return;
    }
    references.retain(|reference| lsp_event_path_is_current(workspace_root, &reference.path));
}

fn retain_valid_references(references: &mut Vec<LspReference>) {
    if references.iter().all(lsp_reference_range_is_valid) {
        return;
    }
    references.retain(lsp_reference_range_is_valid);
}

fn lsp_reference_range_is_valid(reference: &LspReference) -> bool {
    reference.line > 0
        && reference.column > 0
        && reference.end_line > 0
        && reference.end_column > 0
        && (reference.end_line > reference.line
            || (reference.end_line == reference.line && reference.end_column >= reference.column))
}

fn pending_lsp_references_request_matches(
    references_open: bool,
    references_path: Option<&Path>,
    references_line: usize,
    references_column: usize,
    path: &Path,
    line: usize,
    column: usize,
) -> bool {
    references_open
        && references_line == line.saturating_add(1)
        && references_column == column
        && references_path
            .is_some_and(|references_path| paths_match_lexically(references_path, path))
}

fn pending_lsp_rename_request_matches(
    lsp_rename_open: bool,
    lsp_rename_input: &str,
    new_name: &str,
) -> bool {
    if lsp_rename_open {
        return false;
    }

    lsp_rename_request_target(lsp_rename_input).is_ok_and(|target| target == new_name)
}

fn references_failed_status(error: &str) -> String {
    format!("References failed: {}", display_error_label_cow(error))
}

fn references_load_failed_status(path: &Path) -> String {
    format!(
        "Could not load references for {}",
        display_path_label_cow(path)
    )
}

fn rename_failed_status(error: &str) -> String {
    format!("Rename failed: {}", display_error_label_cow(error))
}

fn rename_location_failed_status(path: &Path, line: usize, column: usize) -> String {
    format!(
        "Rename failed at {}:{}:{}",
        display_path_label_cow(path),
        line.saturating_add(1),
        column
    )
}

#[cfg(test)]
mod tests {
    use super::{
        pending_lsp_references_request_matches, pending_lsp_rename_request_matches,
        references_failed_status, references_load_failed_status, rename_failed_status,
        rename_location_failed_status, retain_valid_references, retain_workspace_references,
    };
    use crate::{
        KuroyaApp,
        app_startup_context::AppStartupContext,
        path_display::{DISPLAY_ERROR_LABEL_MAX_CHARS, DISPLAY_PATH_LABEL_MAX_CHARS},
        terminal::TerminalPane,
    };
    use kuroya_core::{EditorSettings, LspReference, LspTextEdit, TextBuffer, Workspace};
    use std::{
        path::{Path, PathBuf},
        time::Instant,
    };
    use tokio::runtime::Runtime;

    #[test]
    fn references_failed_status_sanitizes_and_bounds_lsp_error_display_text() {
        let error = format!(
            "server failed\nwhile searching \u{202e}{}",
            "details-".repeat(DISPLAY_ERROR_LABEL_MAX_CHARS)
        );

        let status = references_failed_status(&error);

        assert!(error.contains('\n'));
        assert!(error.contains('\u{202e}'));
        assert_clean_display_status(&status);
        assert!(status.contains("..."));
        assert!(
            status.chars().count()
                <= "References failed: ".chars().count() + DISPLAY_ERROR_LABEL_MAX_CHARS
        );
    }

    #[test]
    fn references_load_failed_status_sanitizes_and_bounds_path_display_text() {
        let path = hostile_path("references", DISPLAY_PATH_LABEL_MAX_CHARS);

        let status = references_load_failed_status(&path);

        assert_raw_path_keeps_hostile_text(&path);
        assert_clean_display_status(&status);
        assert!(status.contains("..."));
        assert!(
            status.chars().count()
                <= "Could not load references for ".chars().count() + DISPLAY_PATH_LABEL_MAX_CHARS
        );
    }

    #[test]
    fn rename_failed_status_sanitizes_and_bounds_lsp_error_display_text() {
        let error = format!(
            "rename rejected\r\nbecause \u{202d}{}",
            "reason-".repeat(DISPLAY_ERROR_LABEL_MAX_CHARS)
        );

        let status = rename_failed_status(&error);

        assert!(error.contains('\r'));
        assert!(error.contains('\n'));
        assert!(error.contains('\u{202d}'));
        assert_clean_display_status(&status);
        assert!(status.contains("..."));
        assert!(
            status.chars().count()
                <= "Rename failed: ".chars().count() + DISPLAY_ERROR_LABEL_MAX_CHARS
        );
    }

    #[test]
    fn rename_location_failed_status_sanitizes_and_bounds_path_display_text() {
        let path = hostile_path("rename", DISPLAY_PATH_LABEL_MAX_CHARS);
        let suffix = format!(":{}:{}", 42, 7);

        let status = rename_location_failed_status(&path, 41, 7);

        assert_raw_path_keeps_hostile_text(&path);
        assert_clean_display_status(&status);
        assert!(status.contains("..."));
        assert!(status.ends_with(&suffix));
        assert!(
            status.chars().count()
                <= "Rename failed at ".chars().count()
                    + DISPLAY_PATH_LABEL_MAX_CHARS
                    + suffix.chars().count()
        );
    }

    #[test]
    fn references_result_filters_destinations_outside_workspace() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let inside = root.join("src/lib.rs");
        let outside = PathBuf::from("outside/lib.rs");
        let (mut app, version) = app_with_active_buffer(root.clone(), path.clone());
        seed_pending_references_request(&mut app, path.as_path(), 0, 1);

        app.handle_lsp_references_result(
            7,
            path,
            version,
            0,
            1,
            Some(vec![reference(outside), reference(inside.clone())]),
            None,
        );

        assert!(app.references_open);
        assert_eq!(app.references.len(), 1);
        assert_eq!(app.references[0].path, inside);
        assert_eq!(app.status, "1 references at 1:1");
    }

    #[test]
    fn references_result_filters_invalid_reference_ranges() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let valid_path = root.join("src/lib.rs");
        let invalid_zero = root.join("src/zero.rs");
        let invalid_reversed = root.join("src/reversed.rs");
        let (mut app, version) = app_with_active_buffer(root.clone(), path.clone());
        seed_pending_references_request(&mut app, path.as_path(), 0, 1);

        app.handle_lsp_references_result(
            7,
            path,
            version,
            0,
            1,
            Some(vec![
                reference_with_range(invalid_zero, 0, 1, 1, 4),
                reference_with_range(invalid_reversed, 3, 5, 3, 2),
                reference(valid_path.clone()),
            ]),
            None,
        );

        assert!(app.references_open);
        assert_eq!(app.references.len(), 1);
        assert_eq!(app.references[0].path, valid_path);
        assert_eq!(app.status, "1 references at 1:1");
    }

    #[test]
    fn references_result_preserves_selection_for_duplicate_results() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let first = root.join("src/lib.rs");
        let second = root.join("src/service.rs");
        let (mut app, version) = app_with_active_buffer(root.clone(), path.clone());
        let references = vec![reference(first), reference(second)];
        seed_pending_references_request(&mut app, path.as_path(), 0, 1);

        app.handle_lsp_references_result(
            7,
            path.clone(),
            version,
            0,
            1,
            Some(references.clone()),
            None,
        );
        app.references_selected = 1;

        app.handle_lsp_references_result(7, path, version, 0, 1, Some(references), None);

        assert_eq!(app.references_selected, 1);
        assert_eq!(app.status, "2 references at 1:1");
    }

    #[test]
    fn references_result_resets_selection_when_results_change() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let first = root.join("src/lib.rs");
        let second = root.join("src/service.rs");
        let replacement = root.join("src/replacement.rs");
        let (mut app, version) = app_with_active_buffer(root.clone(), path.clone());
        seed_pending_references_request(&mut app, path.as_path(), 0, 1);

        app.handle_lsp_references_result(
            7,
            path.clone(),
            version,
            0,
            1,
            Some(vec![reference(first), reference(second)]),
            None,
        );
        app.references_selected = 1;

        app.handle_lsp_references_result(
            7,
            path,
            version,
            0,
            1,
            Some(vec![reference(replacement)]),
            None,
        );

        assert_eq!(app.references_selected, 0);
        assert_eq!(app.references.len(), 1);
        assert_eq!(app.status, "1 references at 1:1");
    }

    #[test]
    fn references_result_ignores_result_after_target_changes() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let first = root.join("src/lib.rs");
        let second = root.join("src/service.rs");
        let (mut app, version) = app_with_active_buffer(root.clone(), path.clone());
        let references = vec![reference(first), reference(second)];
        seed_pending_references_request(&mut app, path.as_path(), 0, 1);

        app.handle_lsp_references_result(
            7,
            path.clone(),
            version,
            0,
            1,
            Some(references.clone()),
            None,
        );
        app.references_selected = 1;
        app.references_column = 2;
        app.status = "unchanged".to_owned();

        app.handle_lsp_references_result(7, path, version, 0, 1, Some(references), None);

        assert!(app.references_open);
        assert_eq!(app.references_selected, 1);
        assert_eq!(app.status, "unchanged");
    }

    #[test]
    fn references_result_ignores_closed_pending_request() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let target = root.join("src/lib.rs");
        let (mut app, version) = app_with_active_buffer(root.clone(), path.clone());
        seed_pending_references_request(&mut app, path.as_path(), 0, 1);
        app.references_open = false;
        app.references = vec![reference(target.clone())];
        app.status = "closed".to_owned();

        app.handle_lsp_references_result(
            7,
            path,
            version,
            0,
            1,
            Some(vec![reference(PathBuf::from("workspace/src/other.rs"))]),
            None,
        );

        assert!(!app.references_open);
        assert_eq!(app.references, vec![reference(target)]);
        assert_eq!(app.status, "closed");
    }

    #[test]
    fn references_result_ignores_stale_buffer_id() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let target = root.join("src/lib.rs");
        let (mut app, version) = app_with_active_buffer(root.clone(), path.clone());
        app.references_open = true;
        app.references = vec![reference(target.clone())];
        app.references_path = Some(path.clone());
        app.references_line = 1;
        app.references_column = 1;
        app.references_selected = 0;
        app.status = "unchanged".to_owned();

        app.handle_lsp_references_result(
            8,
            path,
            version,
            0,
            1,
            Some(vec![reference(target.clone())]),
            None,
        );

        assert!(app.references_open);
        assert_eq!(app.references, vec![reference(target)]);
        assert_eq!(app.status, "unchanged");
    }

    #[test]
    fn references_result_ignores_stale_version() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let target = root.join("src/lib.rs");
        let (mut app, version) = app_with_active_buffer(root.clone(), path.clone());
        app.references_open = true;
        app.references = vec![reference(target.clone())];
        app.status = "unchanged".to_owned();

        app.handle_lsp_references_result(
            7,
            path,
            version + 1,
            0,
            1,
            Some(vec![reference(root.join("src/other.rs"))]),
            None,
        );

        assert!(app.references_open);
        assert_eq!(app.references, vec![reference(target)]);
        assert_eq!(app.status, "unchanged");
    }

    #[test]
    fn references_result_ignores_stale_cursor_position() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let target = root.join("src/lib.rs");
        let (mut app, version) = app_with_active_buffer(root.clone(), path.clone());
        app.references_open = true;
        app.references = vec![reference(target.clone())];
        app.status = "unchanged".to_owned();

        app.handle_lsp_references_result(
            7,
            path,
            version,
            0,
            2,
            Some(vec![reference(root.join("src/other.rs"))]),
            None,
        );

        assert!(app.references_open);
        assert_eq!(app.references, vec![reference(target)]);
        assert_eq!(app.status, "unchanged");
    }

    #[test]
    fn rename_result_ignores_stale_buffer_id() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let (mut app, version) = app_with_active_buffer(root, path.clone());
        app.status = "unchanged".to_owned();

        app.handle_lsp_rename_result(
            8,
            path,
            version,
            0,
            1,
            "renamed".to_owned(),
            None,
            Some("server error".to_owned()),
        );

        assert_eq!(app.status, "unchanged");
    }

    #[test]
    fn rename_result_ignores_stale_version() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let (mut app, version) = app_with_active_buffer(root.clone(), path.clone());
        app.status = "unchanged".to_owned();

        app.handle_lsp_rename_result(
            7,
            path,
            version + 1,
            0,
            1,
            "renamed".to_owned(),
            Some(vec![text_edit(root.join("src/lib.rs"), "renamed")]),
            None,
        );

        assert!(!app.lsp_rename_preview_open);
        assert!(app.lsp_rename_preview_edits.is_empty());
        assert_eq!(app.status, "unchanged");
    }

    #[test]
    fn rename_result_preserves_raw_preview_edits() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let edit_path = root.join("src").join(".").join("lib.rs");
        let raw_text = "raw\n\u{202e}replacement";
        let (mut app, version) = app_with_active_buffer(root, path.clone());
        app.lsp_rename_input = "renamed".to_owned();

        app.handle_lsp_rename_result(
            7,
            path,
            version,
            0,
            1,
            "renamed".to_owned(),
            Some(vec![text_edit(edit_path.clone(), raw_text)]),
            None,
        );

        assert!(app.lsp_rename_preview_open);
        assert!(app.lsp_rename_input.is_empty());
        assert_eq!(app.lsp_rename_preview_edits.len(), 1);
        assert_eq!(app.lsp_rename_preview_edits[0].path, edit_path);
        assert_eq!(app.lsp_rename_preview_edits[0].new_text, raw_text);
        assert!(app.lsp_rename_preview_edits[0].new_text.contains('\n'));
        assert!(
            app.lsp_rename_preview_edits[0]
                .new_text
                .contains('\u{202e}')
        );
    }

    #[test]
    fn rename_result_ignores_replaced_pending_name() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let target = root.join("src/lib.rs");
        let (mut app, version) = app_with_active_buffer(root, path.clone());
        app.lsp_rename_input = "newer_name".to_owned();
        app.status = "unchanged".to_owned();

        app.handle_lsp_rename_result(
            7,
            path,
            version,
            0,
            1,
            "older_name".to_owned(),
            Some(vec![text_edit(target, "older_name")]),
            None,
        );

        assert!(!app.lsp_rename_preview_open);
        assert!(app.lsp_rename_preview_edits.is_empty());
        assert_eq!(app.lsp_rename_input, "newer_name");
        assert_eq!(app.status, "unchanged");
    }

    #[test]
    fn pending_reference_request_requires_open_matching_target() {
        let path = Path::new("workspace/src/main.rs");
        let equivalent_path = Path::new("workspace/src/./main.rs");

        assert!(pending_lsp_references_request_matches(
            true,
            Some(path),
            1,
            2,
            equivalent_path,
            0,
            2
        ));
        assert!(!pending_lsp_references_request_matches(
            false,
            Some(path),
            1,
            2,
            equivalent_path,
            0,
            2
        ));
        assert!(!pending_lsp_references_request_matches(
            true,
            Some(path),
            1,
            3,
            equivalent_path,
            0,
            2
        ));
        assert!(!pending_lsp_references_request_matches(
            true,
            None,
            1,
            2,
            equivalent_path,
            0,
            2
        ));
    }

    #[test]
    fn pending_rename_request_requires_closed_popup_and_matching_target() {
        assert!(pending_lsp_rename_request_matches(
            false,
            "  renamed_symbol  ",
            "renamed_symbol"
        ));
        assert!(!pending_lsp_rename_request_matches(
            true,
            "renamed_symbol",
            "renamed_symbol"
        ));
        assert!(!pending_lsp_rename_request_matches(
            false,
            "other_symbol",
            "renamed_symbol"
        ));
        assert!(!pending_lsp_rename_request_matches(
            false,
            "bad\nsymbol",
            "bad\nsymbol"
        ));
    }

    #[test]
    fn retain_workspace_references_leaves_all_workspace_results_unchanged() {
        let root = PathBuf::from("workspace");
        let mut references = vec![
            reference(root.join("src/lib.rs")),
            reference(root.join("src/main.rs")),
        ];
        let expected = references.clone();

        retain_workspace_references(&root, &mut references);

        assert_eq!(references, expected);
    }

    #[test]
    fn retain_valid_references_keeps_raw_valid_ranges_and_drops_impossible_ranges() {
        let raw_path = PathBuf::from("workspace/src/ref\n\u{202e}.rs");
        let valid = reference_with_range(raw_path.clone(), 2, 3, 2, 7);
        let zero = reference_with_range(PathBuf::from("workspace/src/zero.rs"), 0, 3, 2, 7);
        let reversed = reference_with_range(PathBuf::from("workspace/src/reversed.rs"), 4, 8, 4, 2);
        let mut references = vec![zero, valid.clone(), reversed];

        retain_valid_references(&mut references);

        assert_eq!(references, vec![valid]);
        assert_eq!(references[0].path, raw_path);
        assert!(references[0].path.to_string_lossy().contains('\n'));
        assert!(references[0].path.to_string_lossy().contains('\u{202e}'));
    }

    fn hostile_path(prefix: &str, repeat: usize) -> PathBuf {
        PathBuf::from("workspace").join(format!(
            "{prefix}\n\u{202e}{}tail.rs",
            "segment-".repeat(repeat)
        ))
    }

    fn assert_raw_path_keeps_hostile_text(path: &Path) {
        let raw = path.to_string_lossy();
        assert!(raw.contains('\n'));
        assert!(raw.contains('\u{202e}'));
    }

    fn assert_clean_display_status(status: &str) {
        assert!(!status.contains('\n'));
        assert!(!status.contains('\r'));
        assert!(!status.contains('\u{202d}'));
        assert!(!status.contains('\u{202e}'));
    }

    fn reference(path: PathBuf) -> LspReference {
        reference_with_range(path, 1, 1, 1, 4)
    }

    fn reference_with_range(
        path: PathBuf,
        line: usize,
        column: usize,
        end_line: usize,
        end_column: usize,
    ) -> LspReference {
        LspReference {
            path,
            line,
            column,
            end_line,
            end_column,
        }
    }

    fn text_edit(path: PathBuf, new_text: &str) -> LspTextEdit {
        LspTextEdit {
            path,
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 4,
            new_text: new_text.to_owned(),
        }
    }

    fn seed_pending_references_request(
        app: &mut KuroyaApp,
        path: &Path,
        line: usize,
        column: usize,
    ) {
        app.references_open = true;
        app.references_path = Some(path.to_path_buf());
        app.references_line = line.saturating_add(1);
        app.references_column = column;
    }

    fn app_with_active_buffer(root: PathBuf, path: PathBuf) -> (KuroyaApp, u64) {
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text(7, Some(path), "fn main() {}\n".to_owned());
        buffer.set_single_cursor(0);
        let version = buffer.version();
        app.buffers.push(buffer);
        app.panes[0].active = Some(7);
        app.active = Some(7);
        (app, version)
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
