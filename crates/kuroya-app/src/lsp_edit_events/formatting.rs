use crate::{
    KuroyaApp,
    app_state::PendingFormatOnSave,
    path_display::{display_error_label_cow, display_path_label_cow},
    workspace_state::{
        buffer_id_path_version_matches, lsp_event_path_is_current, paths_match_lexically,
    },
};
use kuroya_core::{BufferId, LspTextEdit};
use std::path::{Path, PathBuf};

impl KuroyaApp {
    pub(super) fn handle_lsp_formatting_result(
        &mut self,
        request_id: u64,
        id: BufferId,
        path: PathBuf,
        version: u64,
        edits: Option<Vec<LspTextEdit>>,
        error: Option<String>,
    ) {
        if self.take_canceled_lsp_formatting_request(request_id) {
            return;
        }
        let pending_save = self
            .pending_format_on_save
            .get(&id)
            .filter(|pending| {
                pending.request_id == request_id
                    && paths_match_lexically(&pending.format_path, &path)
                    && pending.version == version
            })
            .cloned();
        let path_is_current = lsp_event_path_is_current(&self.workspace.root, &path);
        if !path_is_current || !buffer_id_path_version_matches(&self.buffers, id, &path, version) {
            if let Some(pending_save) = pending_save {
                let buffer_still_targets_format_path = self.buffer(id).is_some_and(|buffer| {
                    path_is_current
                        && buffer
                            .path()
                            .is_some_and(|buffer_path| paths_match_lexically(buffer_path, &path))
                });
                if buffer_still_targets_format_path {
                    self.continue_stale_format_on_save(id, pending_save);
                    return;
                }
                self.cancel_pending_format_on_save(id);
            }
            return;
        }
        if let Some(error) = error {
            self.status = formatting_failed_status(&error);
        } else if let Some(edits) = edits {
            self.apply_lsp_workspace_edits(edits, &formatted_status_label(&path));
        } else {
            self.status = formatting_load_failed_status(&path);
        }
        if let Some(pending_save) = pending_save {
            let overwrite_external_change = self.take_format_on_save_overwrite_external_change(id);
            self.finish_pending_format_on_save(id);
            if !overwrite_external_change
                && paths_match_lexically(&pending_save.save_path, &path)
                && self
                    .save_needs_observed_external_change_confirmation(id, &pending_save.save_path)
            {
                self.open_save_conflict_for_buffer(id);
                return;
            }
            self.format_on_save_bypass.insert(id);
            if overwrite_external_change {
                self.spawn_save_to_over_external_change(id, pending_save.save_path);
            } else {
                self.spawn_save_to(id, pending_save.save_path);
            }
        }
    }

    fn continue_stale_format_on_save(&mut self, id: BufferId, pending_save: PendingFormatOnSave) {
        let Some(buffer) = self.buffer(id) else {
            self.cancel_pending_format_on_save(id);
            return;
        };
        let current_version = buffer.version();
        let current_format_path = buffer.path().cloned();

        if let Some(request_id) =
            self.request_lsp_formatting_for_buffer(id, Some("Formatting before save"), false)
        {
            self.replace_pending_format_on_save(
                id,
                PendingFormatOnSave {
                    save_path: pending_save.save_path,
                    format_path: current_format_path.unwrap_or(pending_save.format_path),
                    version: current_version,
                    request_id,
                },
                0,
            );
            return;
        }

        let overwrite_external_change = self.take_format_on_save_overwrite_external_change(id);
        self.finish_pending_format_on_save(id);
        self.format_on_save_bypass.insert(id);
        if overwrite_external_change {
            self.spawn_save_to_over_external_change(id, pending_save.save_path);
        } else {
            self.spawn_save_to(id, pending_save.save_path);
        }
    }
}

fn formatting_failed_status(error: &str) -> String {
    format!("Format failed: {}", display_error_label_cow(error))
}

fn formatted_status_label(path: &Path) -> String {
    format!("Formatted {}", display_path_label_cow(path))
}

fn formatting_load_failed_status(path: &Path) -> String {
    format!("Format failed for {}", display_path_label_cow(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app_startup_context::AppStartupContext,
        path_display::{DISPLAY_ERROR_LABEL_MAX_CHARS, DISPLAY_PATH_LABEL_MAX_CHARS},
        terminal::TerminalPane,
    };
    use kuroya_core::{EditorSettings, TextBuffer, TextEdit, Workspace};
    use std::{path::PathBuf, time::Instant};
    use tokio::runtime::Runtime;

    #[test]
    fn formatting_failed_status_sanitizes_and_bounds_lsp_error_text() {
        let status = formatting_failed_status(&unsafe_error_text());

        assert_safe_status_text(&status);
        assert!(status.starts_with("Format failed: "));
        assert!(status.contains("..."));
        assert!(
            status.chars().count()
                <= "Format failed: ".chars().count() + DISPLAY_ERROR_LABEL_MAX_CHARS
        );
    }

    #[test]
    fn formatting_path_statuses_sanitize_and_bound_file_labels() {
        let path = unsafe_path();

        let formatted = formatted_status_label(&path);
        let failed = formatting_load_failed_status(&path);

        for (prefix, status) in [("Formatted ", formatted), ("Format failed for ", failed)] {
            assert_safe_status_text(&status);
            assert!(status.starts_with(prefix));
            assert!(status.contains("..."));
            assert!(
                status.chars().count() <= prefix.chars().count() + DISPLAY_PATH_LABEL_MAX_CHARS
            );
        }
    }

    #[test]
    fn formatting_result_applies_raw_edit_while_status_uses_display_path() {
        let root = PathBuf::from("workspace");
        let path = unsafe_path_under(&root);
        let mut app = app_for_test(root);
        let buffer = TextBuffer::from_text(7, Some(path.clone()), "fn main() {}\n".to_owned());
        let version = buffer.version();
        app.buffers.push(buffer);

        app.handle_lsp_formatting_result(
            13,
            7,
            path.clone(),
            version,
            Some(vec![LspTextEdit {
                path,
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 1,
                new_text: "// formatted\n".to_owned(),
            }]),
            None,
        );

        assert_eq!(
            app.buffer(7).expect("buffer").text(),
            "// formatted\nfn main() {}\n"
        );
        assert_safe_status_text(&app.status);
        assert!(app.status.starts_with("Formatted "));
        assert!(app.status.contains("..."));
        assert!(app.status.ends_with(": changed 1 open buffers"));
    }

    #[test]
    fn stale_formatting_result_continues_format_on_save_without_hanging() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "fn main() {}\n".to_owned());
        let stale_version = buffer.version();
        buffer.apply_edit(TextEdit {
            range: 0..0,
            inserted: "// newer\n".to_owned(),
        });
        app.buffers.push(buffer);
        app.pending_format_on_save.insert(
            7,
            PendingFormatOnSave {
                save_path: path.clone(),
                format_path: path.clone(),
                version: stale_version,
                request_id: 21,
            },
        );
        app.status = "current status".to_owned();

        app.handle_lsp_formatting_result(
            21,
            7,
            path.clone(),
            stale_version,
            Some(vec![LspTextEdit {
                path: path.clone(),
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 1,
                new_text: "// stale\n".to_owned(),
            }]),
            None,
        );

        assert_eq!(
            app.buffer(7).expect("buffer").text(),
            "// newer\nfn main() {}\n"
        );
        assert!(!app.pending_format_on_save.contains_key(&7));
        assert!(!app.format_on_save_bypass.contains(&7));
        assert!(app.in_flight_saves.contains(&7));
        assert!(app.status.starts_with("Saving "));
    }

    #[test]
    fn format_on_save_result_accepts_equivalent_format_path() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let equivalent_path = root.join("src").join(".").join("main.rs");
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "fn main() {}\n".to_owned());
        let version = buffer.version();
        buffer.mark_dirty();
        app.buffers.push(buffer);
        app.pending_format_on_save.insert(
            7,
            PendingFormatOnSave {
                save_path: path.clone(),
                format_path: path.clone(),
                version,
                request_id: 21,
            },
        );

        app.handle_lsp_formatting_result(21, 7, equivalent_path, version, Some(Vec::new()), None);

        assert!(!app.pending_format_on_save.contains_key(&7));
        assert!(!app.format_on_save_bypass.contains(&7));
        assert!(app.in_flight_saves.contains(&7));
        assert!(app.status.starts_with("Saving "));
    }

    #[test]
    fn stale_format_on_save_result_continues_for_equivalent_current_buffer_path() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let equivalent_path = root.join("src").join(".").join("main.rs");
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "fn main() {}\n".to_owned());
        let stale_version = buffer.version();
        buffer.apply_edit(TextEdit {
            range: 0..0,
            inserted: "// newer\n".to_owned(),
        });
        app.buffers.push(buffer);
        app.pending_format_on_save.insert(
            7,
            PendingFormatOnSave {
                save_path: path.clone(),
                format_path: path.clone(),
                version: stale_version,
                request_id: 21,
            },
        );

        app.handle_lsp_formatting_result(
            21,
            7,
            equivalent_path.clone(),
            stale_version,
            Some(vec![LspTextEdit {
                path: equivalent_path,
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 1,
                new_text: "// stale\n".to_owned(),
            }]),
            None,
        );

        assert_eq!(
            app.buffer(7).expect("buffer").text(),
            "// newer\nfn main() {}\n"
        );
        assert!(!app.pending_format_on_save.contains_key(&7));
        assert!(!app.format_on_save_bypass.contains(&7));
        assert!(app.in_flight_saves.contains(&7));
        assert!(app.status.starts_with("Saving "));
    }

    #[test]
    fn unrelated_formatting_result_does_not_consume_format_on_save() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        let buffer = TextBuffer::from_text(7, Some(path.clone()), "fn main() {}\n".to_owned());
        let version = buffer.version();
        app.buffers.push(buffer);
        app.pending_format_on_save.insert(
            7,
            PendingFormatOnSave {
                save_path: path.clone(),
                format_path: path.clone(),
                version,
                request_id: 21,
            },
        );

        app.handle_lsp_formatting_result(20, 7, path.clone(), version, Some(Vec::new()), None);

        assert_eq!(
            app.pending_format_on_save.get(&7),
            Some(&PendingFormatOnSave {
                save_path: path.clone(),
                format_path: path,
                version,
                request_id: 21,
            })
        );
        assert!(!app.format_on_save_bypass.contains(&7));
    }

    #[test]
    fn format_on_save_result_rechecks_external_change_before_delayed_save() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "fn main() {}\n".to_owned());
        let version = buffer.version();
        buffer.mark_dirty();
        app.buffers.push(buffer);
        app.external_change_buffers.insert(7);
        app.pending_format_on_save.insert(
            7,
            PendingFormatOnSave {
                save_path: path.clone(),
                format_path: path.clone(),
                version,
                request_id: 21,
            },
        );

        app.handle_lsp_formatting_result(21, 7, path, version, Some(Vec::new()), None);

        assert_eq!(app.save_conflict_buffer, Some(7));
        assert!(!app.pending_format_on_save.contains_key(&7));
        assert!(!app.format_on_save_bypass.contains(&7));
        assert!(!app.in_flight_saves.contains(&7));
        assert!(app.status.ends_with(" changed on disk"));
    }

    #[test]
    fn format_on_save_result_rechecks_external_change_for_equivalent_save_path() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let equivalent_path = root.join("src").join(".").join("main.rs");
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "fn main() {}\n".to_owned());
        let version = buffer.version();
        buffer.mark_dirty();
        app.buffers.push(buffer);
        app.external_change_buffers.insert(7);
        app.pending_format_on_save.insert(
            7,
            PendingFormatOnSave {
                save_path: path.clone(),
                format_path: path,
                version,
                request_id: 21,
            },
        );

        app.handle_lsp_formatting_result(21, 7, equivalent_path, version, Some(Vec::new()), None);

        assert_eq!(app.save_conflict_buffer, Some(7));
        assert!(!app.pending_format_on_save.contains_key(&7));
        assert!(!app.format_on_save_bypass.contains(&7));
        assert!(!app.in_flight_saves.contains(&7));
        assert!(app.status.ends_with(" changed on disk"));
    }

    #[test]
    fn canceled_format_on_save_result_does_not_apply_edits() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "fn main() {}\n".to_owned());
        buffer.mark_dirty();
        let version = buffer.version();
        app.buffers.push(buffer);
        app.pending_format_on_save.insert(
            7,
            PendingFormatOnSave {
                save_path: path.clone(),
                format_path: path.clone(),
                version,
                request_id: 21,
            },
        );

        app.clear_deferred_save_work(7);
        app.handle_lsp_formatting_result(
            21,
            7,
            path.clone(),
            version,
            Some(vec![LspTextEdit {
                path,
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 1,
                new_text: "// formatted\n".to_owned(),
            }]),
            None,
        );

        assert_eq!(app.buffer(7).expect("buffer").text(), "fn main() {}\n");
        assert!(!app.pending_format_on_save.contains_key(&7));
        assert!(!app.in_flight_saves.contains(&7));
    }

    #[test]
    fn force_overwrite_format_on_save_result_does_not_reopen_conflict() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text(7, Some(path.clone()), "fn main() {}\n".to_owned());
        buffer.mark_dirty();
        let version = buffer.version();
        app.buffers.push(buffer);
        app.external_change_buffers.insert(7);
        app.mark_format_on_save_overwrite_external_change(7);
        app.pending_format_on_save.insert(
            7,
            PendingFormatOnSave {
                save_path: path.clone(),
                format_path: path.clone(),
                version,
                request_id: 21,
            },
        );

        app.handle_lsp_formatting_result(21, 7, path, version, Some(Vec::new()), None);

        assert_eq!(app.save_conflict_buffer, None);
        assert!(!app.pending_format_on_save.contains_key(&7));
        assert!(!app.format_on_save_overwrites_external_change(7));
        assert!(app.in_flight_saves.contains(&7));
        assert!(app.status.starts_with("Saving "));
    }

    fn unsafe_path() -> PathBuf {
        unsafe_path_under(Path::new("workspace"))
    }

    fn unsafe_path_under(root: &Path) -> PathBuf {
        root.join(format!(
            "bad\n{}\u{202e}tail.rs",
            "very-long-path-".repeat(DISPLAY_PATH_LABEL_MAX_CHARS)
        ))
    }

    fn unsafe_error_text() -> String {
        format!(
            "first\nsecond\u{202e}{}tail",
            "very-long-error-".repeat(DISPLAY_ERROR_LABEL_MAX_CHARS)
        )
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
