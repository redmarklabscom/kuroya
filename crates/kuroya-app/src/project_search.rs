use crate::{
    KuroyaApp,
    devtools_async_tasks::MAX_ASYNC_TASK_DETAIL_CHARS,
    path_display::display_path_label_cow,
    project_search_state::{
        MAX_PROJECT_SEARCH_RECENT_QUERIES, next_project_search_request_id,
        normalize_project_search_request_query, parse_project_globs,
        project_search_globs_match_current, project_search_query_matches_current,
        project_search_status_query_label, quoted_project_search_query_label,
        record_recent_project_search_from_parsed_globs,
    },
    ui_events::UiEvent,
    ui_state::{clamp_selection, move_selection},
};
use kuroya_core::{SearchOptions, SearchResult, search::search_project_index_with_cancel};
use std::{
    fmt::Write as _,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

impl KuroyaApp {
    pub(crate) fn spawn_project_search(&mut self) {
        let Some(query) = normalize_project_search_request_query(&self.project_search_query) else {
            self.invalidate_project_search_requests();
            self.project_search_result = SearchResult::default();
            self.project_search_result_query.clear();
            self.project_search_result_index_generation = self.project_search_index_generation;
            self.project_search_selected = 0;
            return;
        };

        let request_id = self.reserve_project_search_request_id();
        let index = self.project_search_index.clone();
        let index_generation = self.project_search_index_generation;
        let workspace_root = self.workspace.root.clone();
        let tx = self.tx.clone();
        let cancel_generation = self.project_search_cancel_generation.clone();
        let case_sensitive = self.project_search_case_sensitive;
        let whole_word = self.project_search_whole_word;
        let include_globs = parse_project_globs(&self.project_search_include);
        let exclude_globs = parse_project_globs(&self.project_search_exclude);
        record_recent_project_search_from_parsed_globs(
            &mut self.project_search_recent,
            &query,
            case_sensitive,
            whole_word,
            &include_globs,
            &exclude_globs,
            MAX_PROJECT_SEARCH_RECENT_QUERIES,
        );
        self.status = project_search_start_status(&query);
        self.record_async_task_started("Project Search", project_search_start_detail(&query));
        self.runtime.spawn_blocking(move || {
            let options = SearchOptions {
                query,
                case_sensitive,
                whole_word,
                include_globs,
                exclude_globs,
                ..SearchOptions::default()
            };
            let result = search_project_index_with_cancel(&index, &options, || {
                project_search_request_is_cancelled(&cancel_generation, request_id)
            })
            .unwrap_or_default();
            let SearchOptions {
                query,
                include_globs,
                exclude_globs,
                ..
            } = options;
            let _ = crate::ui_event_channel::send_critical_ui_event(
                &tx,
                UiEvent::SearchFinished {
                    request_id,
                    index_generation,
                    workspace_root,
                    query,
                    case_sensitive,
                    whole_word,
                    include_globs,
                    exclude_globs,
                    result,
                },
            );
        });
    }

    fn reserve_project_search_request_id(&mut self) -> u64 {
        self.project_search_next_request_id =
            next_project_search_request_id(self.project_search_next_request_id);
        self.project_search_active_request_id = self.project_search_next_request_id;
        self.project_search_cancel_generation
            .store(self.project_search_active_request_id, Ordering::Relaxed);
        self.project_search_active_request_id
    }

    pub(crate) fn invalidate_project_search_requests(&mut self) {
        self.reserve_project_search_request_id();
    }

    pub(crate) fn project_search_results_match_current_query(&self) -> bool {
        if self.project_search_result_index_generation != self.project_search_index_generation
            || !project_search_query_matches_current(
                &self.project_search_result_query,
                &self.project_search_query,
            )
            || self.project_search_result_case_sensitive != self.project_search_case_sensitive
            || self.project_search_result_whole_word != self.project_search_whole_word
        {
            return false;
        }

        project_search_globs_match_current(
            &self.project_search_result_include_globs,
            &self.project_search_include,
        ) && project_search_globs_match_current(
            &self.project_search_result_exclude_globs,
            &self.project_search_exclude,
        )
    }

    pub(crate) fn goto_project_search_result(&mut self, direction: isize) {
        if !self.project_search_results_match_current_query() {
            self.status =
                if normalize_project_search_request_query(&self.project_search_query).is_none() {
                    "No project search query".to_owned()
                } else {
                    "Run project search to refresh results".to_owned()
                };
            return;
        }

        let len = self.project_search_result.matches.len();
        if len == 0 {
            self.status = "No project search matches".to_owned();
            return;
        }

        move_project_search_selection(&mut self.project_search_selected, len, direction);
        let Some(jump) =
            project_search_result_jump(&self.project_search_result, self.project_search_selected)
        else {
            return;
        };

        let status = project_search_match_status(
            self.project_search_selected + 1,
            len,
            &jump.path,
            jump.line,
            jump.column,
        );
        self.open_file_at_known_openable(jump.path, jump.line, jump.column);
        self.status = status;
    }
}

fn move_project_search_selection(selection: &mut usize, len: usize, direction: isize) {
    clamp_selection(selection, len);
    move_selection(selection, len, direction);
}

#[derive(Debug, PartialEq, Eq)]
struct ProjectSearchResultJump {
    path: PathBuf,
    line: usize,
    column: usize,
}

fn project_search_result_jump(
    result: &SearchResult,
    selected_index: usize,
) -> Option<ProjectSearchResultJump> {
    let result_match = result.matches.get(selected_index)?;
    Some(ProjectSearchResultJump {
        path: result_match.path.clone(),
        line: result_match.line,
        column: result_match.column,
    })
}

fn project_search_start_status(query: &str) -> String {
    let label = project_search_status_query_label(query);
    let mut status = String::with_capacity("Searching for `".len() + label.len() + 1);
    status.push_str("Searching for `");
    status.push_str(&label);
    status.push('`');
    status
}

fn project_search_start_detail(query: &str) -> String {
    quoted_project_search_query_label(query, MAX_ASYNC_TASK_DETAIL_CHARS)
}

fn project_search_request_is_cancelled(cancel_generation: &AtomicU64, request_id: u64) -> bool {
    cancel_generation.load(Ordering::Relaxed) != request_id
}

fn project_search_match_status(
    selected_index: usize,
    result_count: usize,
    path: &Path,
    line: usize,
    column: usize,
) -> String {
    let path = display_path_label_cow(path);
    let mut status = String::with_capacity(path.len().saturating_add(48));
    let _ = write!(
        status,
        "Project match {selected_index}/{result_count} at {}:{line}:{column}",
        path.as_ref()
    );
    status
}

#[cfg(test)]
mod tests {
    use super::{
        ProjectSearchResultJump, move_project_search_selection, project_search_match_status,
        project_search_request_is_cancelled, project_search_result_jump,
        project_search_start_status,
    };
    use crate::{
        KuroyaApp,
        app_startup_context::AppStartupContext,
        devtools_async_tasks::{MAX_ASYNC_TASK_DETAIL_CHARS, async_task_event_label},
        path_display::DISPLAY_PATH_LABEL_MAX_CHARS,
        project_search_state::{
            MAX_PROJECT_SEARCH_QUERY_CHARS, MAX_PROJECT_SEARCH_STATUS_QUERY_CHARS,
            normalize_project_search_request_query,
        },
        terminal::TerminalPane,
        ui_event_channel::ui_event_channel,
        ui_events::UiEvent,
    };
    use kuroya_core::{EditorSettings, SearchMatch, SearchResult, TextBuffer, Workspace};
    use std::{
        path::PathBuf,
        sync::atomic::Ordering,
        time::{Duration, Instant},
    };
    use tokio::runtime::Runtime;

    #[test]
    fn project_search_start_status_sanitizes_and_bounds_query_label() {
        let query = format!(
            "alpha\nbeta\u{202e}{}",
            "query-fragment-".repeat(MAX_PROJECT_SEARCH_STATUS_QUERY_CHARS)
        );

        let status = project_search_start_status(&query);

        assert!(status.starts_with("Searching for `alpha beta"));
        assert!(!status.contains('\n'));
        assert!(!status.contains('\u{202e}'));
        assert!(status.contains("..."));
        assert!(
            status.chars().count()
                <= "Searching for ``".chars().count() + MAX_PROJECT_SEARCH_STATUS_QUERY_CHARS
        );
    }

    #[test]
    fn project_search_match_status_sanitizes_and_bounds_path_label() {
        let path = PathBuf::from(format!(
            "workspace/src/bad\n{}\u{202e}tail.rs",
            "path-fragment-".repeat(DISPLAY_PATH_LABEL_MAX_CHARS)
        ));

        let status = project_search_match_status(1, 2, &path, 3, 5);

        assert!(status.starts_with("Project match 1/2 at "));
        assert!(!status.contains('\n'));
        assert!(!status.contains('\u{202e}'));
        assert!(status.contains("..."));
        assert!(
            status.chars().count()
                <= "Project match 1/2 at :3:5".chars().count() + DISPLAY_PATH_LABEL_MAX_CHARS
        );
    }

    #[test]
    fn project_search_result_jump_captures_only_open_target_fields() {
        let path = PathBuf::from("workspace/src/main.rs");
        let result = SearchResult {
            matches: vec![SearchMatch {
                path: path.clone(),
                line: 3,
                column: 5,
                preview: "needle preview that is not needed for opening".repeat(32),
            }],
            ..SearchResult::default()
        };

        assert_eq!(
            project_search_result_jump(&result, 0),
            Some(ProjectSearchResultJump {
                path,
                line: 3,
                column: 5,
            })
        );
        assert_eq!(project_search_result_jump(&result, 1), None);
    }

    #[test]
    fn project_search_selection_clamps_before_keyboard_wrap() {
        let mut selected = usize::MAX;

        move_project_search_selection(&mut selected, 3, 1);

        assert_eq!(selected, 0);

        selected = usize::MAX;
        move_project_search_selection(&mut selected, 3, -1);

        assert_eq!(selected, 1);
    }

    #[test]
    fn spawn_project_search_sanitizes_visible_labels_and_preserves_raw_request_query() {
        let root = PathBuf::from("workspace");
        let mut app = app_for_project_search_test(root);
        let raw_query = format!(
            "  alpha\n  beta\u{202e} {}  ",
            "query-fragment-".repeat(MAX_PROJECT_SEARCH_QUERY_CHARS)
        );
        let request_query =
            normalize_project_search_request_query(&raw_query).expect("request query");
        app.project_search_query = raw_query.clone();

        app.spawn_project_search();

        assert_eq!(app.project_search_query, raw_query);
        assert!(!app.status.contains('\n'));
        assert!(!app.status.contains('\u{202e}'));
        assert!(app.status.contains("..."));

        let active_detail = app
            .active_async_tasks
            .front()
            .expect("project search active task")
            .detail
            .clone();
        assert!(active_detail.starts_with('`'));
        assert!(active_detail.ends_with('`'));
        assert!(!active_detail.contains('\n'));
        assert!(!active_detail.contains('\u{202e}'));
        assert!(active_detail.contains("..."));
        assert!(active_detail.chars().count() <= MAX_ASYNC_TASK_DETAIL_CHARS);

        let event = app
            .rx
            .recv_timeout(Duration::from_secs(1))
            .expect("project search completion event");
        let label = async_task_event_label(&event).expect("search event label");
        assert_eq!(label.detail, active_detail);
        match event {
            UiEvent::SearchFinished { query, .. } => {
                assert_eq!(query, request_query);
                assert!(!query.contains('\n'));
                assert!(!query.contains('\u{202e}'));
                assert!(query.chars().count() <= MAX_PROJECT_SEARCH_QUERY_CHARS);
            }
            _ => panic!("expected project search completion event"),
        }
    }

    #[test]
    fn project_search_request_reservation_updates_cancellation_generation() {
        let root = PathBuf::from("workspace");
        let mut app = app_for_project_search_test(root);

        let first_request = app.reserve_project_search_request_id();
        assert_eq!(first_request, 1);
        assert_eq!(app.project_search_active_request_id, 1);
        assert_eq!(
            app.project_search_cancel_generation.load(Ordering::Relaxed),
            1
        );

        app.invalidate_project_search_requests();
        assert_eq!(app.project_search_active_request_id, 2);
        assert_eq!(
            app.project_search_cancel_generation.load(Ordering::Relaxed),
            2
        );
    }

    #[test]
    fn project_search_request_cancellation_tracks_generation_mismatches() {
        let cancel_generation = std::sync::atomic::AtomicU64::new(7);

        assert!(!project_search_request_is_cancelled(&cancel_generation, 7));

        cancel_generation.store(8, Ordering::Relaxed);

        assert!(project_search_request_is_cancelled(&cancel_generation, 7));
    }

    #[test]
    fn project_search_results_match_current_query_checks_options_before_globs() {
        let root = PathBuf::from("workspace");
        let mut app = app_for_project_search_test(root);
        app.project_search_query = " needle ".to_owned();
        app.project_search_result_query = "needle".to_owned();
        app.project_search_result_index_generation = app.project_search_index_generation;
        app.project_search_result_include_globs = vec!["src/**/*.rs".to_owned()];
        app.project_search_result_exclude_globs = vec!["target/**".to_owned()];
        app.project_search_include = " src/**/*.rs, src/**/*.rs ".to_owned();
        app.project_search_exclude = "target/**".to_owned();

        assert!(app.project_search_results_match_current_query());

        app.project_search_case_sensitive = true;

        assert!(!app.project_search_results_match_current_query());
    }

    #[test]
    fn project_search_results_match_current_query_normalizes_current_text() {
        let root = PathBuf::from("workspace");
        let mut app = app_for_project_search_test(root);
        app.project_search_query = " needle\n  value ".to_owned();
        app.project_search_result_query = "needle value".to_owned();
        app.project_search_result_index_generation = app.project_search_index_generation;

        assert!(app.project_search_results_match_current_query());

        app.project_search_result_query = "needle  value".to_owned();

        assert!(!app.project_search_results_match_current_query());
    }

    #[test]
    fn project_search_result_jump_records_history_without_filesystem_precheck() {
        let root = PathBuf::from("workspace");
        let mut app = app_for_project_search_test(root.clone());
        let origin_path = root.join("src").join("origin.rs");
        app.buffers.push(TextBuffer::from_text(
            1,
            Some(origin_path.clone()),
            "origin\n".to_owned(),
        ));
        app.set_active_buffer(1);
        app.project_search_query = "needle".to_owned();
        app.project_search_result_query = "needle".to_owned();
        app.project_search_result = SearchResult {
            matches: vec![SearchMatch {
                path: root.join("src").join("indexed-result.rs"),
                line: 3,
                column: 5,
                preview: "needle".to_owned(),
            }],
            ..SearchResult::default()
        };

        app.goto_project_search_result(1);

        assert_eq!(
            app.navigation_back.back().map(|location| &location.path),
            Some(&origin_path)
        );
        assert!(
            app.pending_open_paths
                .contains(&root.join("src").join("indexed-result.rs"))
        );
    }

    #[test]
    fn project_search_result_jump_reuses_lexically_equivalent_open_buffer() {
        let root = PathBuf::from("workspace");
        let mut app = app_for_project_search_test(root.clone());
        let origin_path = root.join("src").join("origin.rs");
        let open_path = root.join("src").join("main.rs");
        let search_path = root.join("src").join("..").join("src").join("main.rs");

        app.buffers.push(TextBuffer::from_text(
            1,
            Some(origin_path.clone()),
            "origin\n".to_owned(),
        ));
        app.buffers.push(TextBuffer::from_text(
            2,
            Some(open_path.clone()),
            "first\nsecond needle\nthird\n".to_owned(),
        ));
        app.set_active_buffer(1);
        app.project_search_query = "needle".to_owned();
        app.project_search_result_query = "needle".to_owned();
        app.project_search_result = SearchResult {
            matches: vec![SearchMatch {
                path: search_path,
                line: 2,
                column: 8,
                preview: "second needle".to_owned(),
            }],
            ..SearchResult::default()
        };

        app.goto_project_search_result(1);

        assert_eq!(app.active, Some(2));
        assert!(app.pending_open_paths.is_empty());
        assert!(app.pending_file_jump.is_none());
        assert_eq!(app.buffer(2).and_then(TextBuffer::path), Some(&open_path));
        assert_eq!(app.buffer(2).unwrap().cursor_position().line, 1);
        assert_eq!(app.buffer(2).unwrap().cursor_position().column, 7);
        assert_eq!(
            app.navigation_back.back().map(|location| &location.path),
            Some(&origin_path)
        );
        assert_eq!(app.status, "Project match 1/1 at main.rs:2:8");
    }

    fn app_for_project_search_test(root: PathBuf) -> KuroyaApp {
        let (tx, rx) = ui_event_channel();
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
