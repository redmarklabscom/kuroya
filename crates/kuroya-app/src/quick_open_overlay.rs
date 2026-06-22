use crate::{
    KuroyaApp,
    quick_open::{
        MAX_QUICK_OPEN_QUERY_MEMORY, QUICK_OPEN_RESULT_LIMIT, QuickOpenMatchQuery, QuickOpenQuery,
        QuickOpenResult, QuickOpenResultsCache, quick_open_index_file_identity,
        quick_open_latest_navigation_locations_from_history, quick_open_paths_match,
        quick_open_ranked_results_from_open_paths,
        quick_open_result_label_with_navigation_line_column,
        quick_open_target_with_navigation_line_column, record_quick_open_query_memory,
        sanitize_quick_open_query_input,
    },
    ui_state::{
        clamp_selection, handle_list_navigation_keys, selected_row_scroll_offset,
        selection_page_step,
    },
};
use eframe::egui::{self, Context, Key, ScrollArea, TextEdit};
use kuroya_core::Command;
use std::{ops::Range, time::Duration};

const QUICK_OPEN_ROW_HEIGHT: f32 = 24.0;
const QUICK_OPEN_WINDOW_PREFERRED_SIZE: [f32; 2] = [620.0, 420.0];
const QUICK_OPEN_WINDOW_MIN_SIZE: [f32; 2] = [280.0, 180.0];
const QUICK_OPEN_WINDOW_MARGIN: [f32; 2] = [32.0, 96.0];

impl KuroyaApp {
    pub(crate) fn render_quick_open(&mut self, ctx: &Context) {
        self.ensure_workspace_index_started();
        let mut open_target = None;

        egui::Window::new("Quick Open")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 72.0])
            .fixed_size(quick_open_window_size(ctx))
            .show(ctx, |ui| {
                let mut scroll_to_selection = false;
                let response = ui.add(
                    TextEdit::singleline(&mut self.quick_open_query)
                        .hint_text("Type a filename or file:line:column")
                        .desired_width(f32::INFINITY),
                );
                response.request_focus();
                if response.changed() {
                    let sanitized_query = sanitize_quick_open_query_input(&self.quick_open_query);
                    if sanitized_query != self.quick_open_query {
                        self.quick_open_query = sanitized_query;
                    }
                    self.quick_open_selected = 0;
                    scroll_to_selection = true;
                }

                if ui.input(|input| input.key_pressed(Key::Escape)) {
                    self.quick_open = false;
                }

                let row_count = self.refresh_quick_open_results_cache();
                let indexing_pending = quick_open_indexing_pending(
                    self.workspace_placeholder,
                    self.index.files().len(),
                    self.workspace_index_in_flight_request_id,
                );
                if indexing_pending {
                    ctx.request_repaint_after(Duration::from_millis(32));
                }
                let mut selected = self.quick_open_selected;
                let mut close_quick_open = false;
                if let Some(cache) = self.quick_open_results_cache.as_ref() {
                    clamp_selection(&mut selected, row_count);
                    let viewport_height = ui.available_height();

                    scroll_to_selection |= ui.input(|input| {
                        handle_list_navigation_keys(
                            input,
                            &mut selected,
                            row_count,
                            selection_page_step(QUICK_OPEN_ROW_HEIGHT, viewport_height),
                        )
                    });
                    if ui.input(|input| input.key_pressed(Key::Enter)) {
                        open_target = quick_open_open_target_at(
                            cache,
                            selected,
                            row_count,
                            &self.quick_open_query,
                        );
                    }

                    if row_count == 0 {
                        let message = quick_open_empty_state_message(indexing_pending);
                        quick_open_scroll_area().show(ui, |ui| {
                            ui.add_space(20.0);
                            ui.centered_and_justified(|ui| {
                                ui.label(message);
                            });
                        });
                    } else {
                        let mut scroll_area = quick_open_scroll_area();
                        if scroll_to_selection {
                            scroll_area =
                                scroll_area.vertical_scroll_offset(selected_row_scroll_offset(
                                    selected,
                                    row_count,
                                    QUICK_OPEN_ROW_HEIGHT,
                                    viewport_height,
                                ));
                        }
                        scroll_area.show_rows(ui, QUICK_OPEN_ROW_HEIGHT, row_count, |ui, rows| {
                            for row in quick_open_prepare_visible_rows(cache, rows, row_count) {
                                let is_selected = row.index == selected;
                                if ui.selectable_label(is_selected, row.label).clicked() {
                                    close_quick_open = true;
                                    open_target = quick_open_open_target_at(
                                        cache,
                                        row.index,
                                        row_count,
                                        &self.quick_open_query,
                                    );
                                }
                            }
                        });
                    }
                } else {
                    selected = 0;
                    let message = quick_open_empty_state_message(indexing_pending);
                    quick_open_scroll_area().show(ui, |ui| {
                        ui.add_space(20.0);
                        ui.centered_and_justified(|ui| {
                            ui.label(message);
                        });
                    });
                }
                self.quick_open_selected = selected;
                if close_quick_open {
                    self.quick_open = false;
                }
            });

        if let Some(target) = open_target {
            self.quick_open = false;
            record_quick_open_query_memory(
                &mut self.quick_open_query_memory,
                &self.workspace.root,
                &target.query_pattern,
                &target.path,
                MAX_QUICK_OPEN_QUERY_MEMORY,
            );
            if let Some((line, column)) = target.line_column {
                self.open_file_at_known_openable(target.path, line, column);
            } else {
                self.command_bus.push(Command::OpenFile(target.path));
            }
        }
    }

    fn refresh_quick_open_results_cache(&mut self) -> usize {
        let index_file_identity = quick_open_index_file_identity(self.index.files());
        let index_generation = self.project_index_generation;
        let current_navigation_location = self.current_navigation_location();
        if let Some(cache) = self.quick_open_results_cache.as_mut()
            && cache.matches(
                &self.quick_open_query,
                index_generation,
                &index_file_identity,
                &self.quick_open_recent_files,
                self.buffers
                    .iter()
                    .filter_map(|buffer| buffer.path().map(|path| path.as_path())),
                &self.quick_open_query_memory,
                &self.navigation_back,
                &self.navigation_forward,
                current_navigation_location.as_ref(),
            )
        {
            return quick_open_refresh_stale_display_metadata(cache);
        }

        let mut open_file_paths = Vec::with_capacity(self.buffers.len());
        open_file_paths.extend(
            self.buffers
                .iter()
                .filter_map(|buffer| buffer.path().cloned()),
        );
        let navigation_locations = quick_open_latest_navigation_locations_from_history(
            &self.navigation_back,
            &self.navigation_forward,
            current_navigation_location.as_ref(),
        );
        let parsed_query = crate::quick_open::parse_quick_open_query(&self.quick_open_query);
        if let Some(cache) = self.quick_open_results_cache.as_mut()
            && quick_open_cache_ranking_inputs_match_ignoring_query_target(
                cache,
                &parsed_query,
                index_generation,
                &index_file_identity,
                &self.quick_open_recent_files,
                open_file_paths.iter().map(|path| path.as_path()),
                &self.quick_open_query_memory,
                &navigation_locations,
            )
        {
            return quick_open_refresh_cached_query_metadata(
                cache,
                &self.quick_open_query,
                parsed_query,
                &self.navigation_back,
                &self.navigation_forward,
                current_navigation_location,
                &navigation_locations,
            );
        }
        let previous_cache = self.quick_open_results_cache.take();
        let match_query = QuickOpenMatchQuery::from_sanitized_query(parsed_query.pattern.clone());
        let results = quick_open_ranked_results_from_open_paths(
            &self.matcher,
            &self.workspace.root,
            self.index.files().iter().map(|path| path.as_path()),
            &self.quick_open_recent_files,
            &open_file_paths,
            &self.quick_open_query_memory,
            &navigation_locations,
            &match_query,
            QUICK_OPEN_RESULT_LIMIT,
        );
        let (results, result_labels) =
            quick_open_results_with_reused_display_metadata(results, &parsed_query, previous_cache);

        let visible_count = results.len();
        self.quick_open_results_cache = Some(QuickOpenResultsCache {
            query_input: self.quick_open_query.clone(),
            index_generation,
            index_file_identity,
            recent_files: self.quick_open_recent_files.clone(),
            open_files: open_file_paths,
            query_memory: self.quick_open_query_memory.clone(),
            navigation_back: self.navigation_back.clone(),
            navigation_forward: self.navigation_forward.clone(),
            current_navigation_location,
            parsed_query,
            result_labels,
            results,
        });
        visible_count
    }
}

fn quick_open_indexing_pending(
    workspace_placeholder: bool,
    index_file_count: usize,
    workspace_index_in_flight_request_id: Option<u64>,
) -> bool {
    !workspace_placeholder
        && index_file_count == 0
        && workspace_index_in_flight_request_id.is_some()
}

fn quick_open_empty_state_message(indexing_pending: bool) -> &'static str {
    if indexing_pending {
        "Indexing workspace..."
    } else {
        "No matching files"
    }
}

fn quick_open_scroll_area() -> ScrollArea {
    ScrollArea::vertical()
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
}

fn quick_open_window_size(ctx: &Context) -> [f32; 2] {
    let available = ctx.available_rect().size();
    quick_open_window_size_for_available(available.x, available.y)
}

fn quick_open_window_size_for_available(available_width: f32, available_height: f32) -> [f32; 2] {
    [
        quick_open_window_dimension(
            QUICK_OPEN_WINDOW_PREFERRED_SIZE[0],
            QUICK_OPEN_WINDOW_MIN_SIZE[0],
            available_width,
            QUICK_OPEN_WINDOW_MARGIN[0],
        ),
        quick_open_window_dimension(
            QUICK_OPEN_WINDOW_PREFERRED_SIZE[1],
            QUICK_OPEN_WINDOW_MIN_SIZE[1],
            available_height,
            QUICK_OPEN_WINDOW_MARGIN[1],
        ),
    ]
}

fn quick_open_window_dimension(preferred: f32, minimum: f32, available: f32, margin: f32) -> f32 {
    if !available.is_finite() || available <= 0.0 {
        return preferred;
    }

    let usable = (available - margin).max(1.0);
    preferred.min(usable).max(minimum.min(usable))
}

fn quick_open_refresh_stale_display_metadata(cache: &mut QuickOpenResultsCache) -> usize {
    let visible_count = quick_open_visible_result_count(cache);
    if visible_count != cache.results.len() || cache.result_labels.len() != cache.results.len() {
        cache.result_labels = quick_open_result_labels(&cache.results, &cache.parsed_query);
    }
    cache.results.len()
}

fn quick_open_visible_result_count(cache: &QuickOpenResultsCache) -> usize {
    cache
        .results
        .iter()
        .zip(&cache.result_labels)
        .take_while(|(result, label)| {
            quick_open_result_label_matches_result(result, label, &cache.parsed_query)
        })
        .count()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct QuickOpenPreparedVisibleRow<'a> {
    index: usize,
    label: &'a str,
}

fn quick_open_prepare_visible_rows<'a>(
    cache: &'a QuickOpenResultsCache,
    rows: Range<usize>,
    visible_count: usize,
) -> impl Iterator<Item = QuickOpenPreparedVisibleRow<'a>> + 'a {
    let end = rows
        .end
        .min(visible_count)
        .min(cache.results.len())
        .min(cache.result_labels.len());
    let start = rows.start.min(end);
    (start..end).map(move |index| QuickOpenPreparedVisibleRow {
        index,
        label: cache.result_labels[index].as_str(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct QuickOpenOpenTarget {
    path: std::path::PathBuf,
    line_column: Option<(usize, usize)>,
    query_pattern: String,
}

fn quick_open_open_target_at(
    cache: &QuickOpenResultsCache,
    index: usize,
    visible_count: usize,
    expected_query_input: &str,
) -> Option<QuickOpenOpenTarget> {
    if cache.query_input != expected_query_input {
        return None;
    }

    if index >= visible_count {
        return None;
    }

    cache.result_labels.get(index)?;
    cache.results.get(index).map(|result| {
        let (path, line_column) = quick_open_target_with_navigation_line_column(
            result.path.clone(),
            &cache.parsed_query,
            result.navigation_line_column,
        );
        QuickOpenOpenTarget {
            path,
            line_column,
            query_pattern: cache.parsed_query.pattern.clone(),
        }
    })
}

fn quick_open_cache_ranking_inputs_match_ignoring_query_target<'a>(
    cache: &QuickOpenResultsCache,
    parsed_query: &QuickOpenQuery,
    index_generation: u64,
    index_file_identity: &crate::quick_open::QuickOpenIndexFileIdentity,
    recent_files: &std::collections::VecDeque<std::path::PathBuf>,
    open_files: impl IntoIterator<Item = &'a std::path::Path>,
    query_memory: &std::collections::VecDeque<crate::quick_open::QuickOpenQueryMemoryEntry>,
    navigation_locations: &[crate::history::NavigationLocation],
) -> bool {
    cache.parsed_query.pattern == parsed_query.pattern
        && cache.ranking_inputs_match(
            &cache.query_input,
            index_generation,
            index_file_identity,
            recent_files,
            open_files,
            query_memory,
            navigation_locations,
        )
}

fn quick_open_refresh_cached_query_metadata(
    cache: &mut QuickOpenResultsCache,
    query_input: &str,
    parsed_query: QuickOpenQuery,
    navigation_back: &std::collections::VecDeque<crate::history::NavigationLocation>,
    navigation_forward: &std::collections::VecDeque<crate::history::NavigationLocation>,
    current_navigation_location: Option<crate::history::NavigationLocation>,
    navigation_locations: &[crate::history::NavigationLocation],
) -> usize {
    let navigation_metadata_matches = cache.navigation_back.iter().eq(navigation_back.iter())
        && cache
            .navigation_forward
            .iter()
            .eq(navigation_forward.iter())
        && cache.current_navigation_location.as_ref() == current_navigation_location.as_ref();

    cache.query_input.clear();
    cache.query_input.push_str(query_input);
    if navigation_metadata_matches {
        if !quick_open_result_label_query_metadata_matches(&cache.parsed_query, &parsed_query)
            || !quick_open_result_labels_match_results(
                &cache.results,
                &cache.result_labels,
                &parsed_query,
            )
        {
            cache.result_labels = quick_open_result_labels(&cache.results, &parsed_query);
        }
        cache.parsed_query = parsed_query;
    } else {
        cache.parsed_query = parsed_query;
        cache.refresh_navigation_metadata(
            navigation_back,
            navigation_forward,
            current_navigation_location,
            navigation_locations,
        );
    }
    cache.results.len()
}

fn quick_open_results_with_reused_display_metadata(
    results: Vec<QuickOpenResult>,
    parsed_query: &QuickOpenQuery,
    previous_cache: Option<QuickOpenResultsCache>,
) -> (Vec<QuickOpenResult>, Vec<String>) {
    let Some(previous_cache) = previous_cache else {
        let result_labels = quick_open_result_labels(&results, parsed_query);
        return (results, result_labels);
    };

    let QuickOpenResultsCache {
        parsed_query: previous_query,
        result_labels: previous_labels,
        results: previous_results,
        ..
    } = previous_cache;

    if quick_open_result_label_query_metadata_matches(&previous_query, parsed_query) {
        if previous_results == results
            && quick_open_result_labels_match_results(
                &previous_results,
                &previous_labels,
                parsed_query,
            )
        {
            return (previous_results, previous_labels);
        }

        let result_labels = quick_open_result_labels_reusing_previous(
            &results,
            parsed_query,
            &previous_results,
            previous_labels,
        );
        return (results, result_labels);
    }

    let result_labels = quick_open_result_labels(&results, parsed_query);
    (results, result_labels)
}

fn quick_open_result_label_query_metadata_matches(
    previous_query: &QuickOpenQuery,
    query: &QuickOpenQuery,
) -> bool {
    previous_query.line == query.line
        && (query.line.is_none() || previous_query.column == query.column)
}

fn quick_open_result_labels_reusing_previous(
    results: &[QuickOpenResult],
    parsed_query: &QuickOpenQuery,
    previous_results: &[QuickOpenResult],
    previous_labels: Vec<String>,
) -> Vec<String> {
    let mut previous_labels: Vec<Option<String>> = previous_labels.into_iter().map(Some).collect();
    results
        .iter()
        .enumerate()
        .map(|(index, result)| {
            if let Some(previous_label) = quick_open_take_previous_result_label(
                previous_results,
                &mut previous_labels,
                parsed_query,
                result,
                index,
            ) {
                return previous_label;
            }

            quick_open_result_label_with_navigation_line_column(
                &result.rel,
                parsed_query,
                result.navigation_line_column,
            )
        })
        .collect()
}

fn quick_open_take_previous_result_label(
    previous_results: &[QuickOpenResult],
    previous_labels: &mut [Option<String>],
    parsed_query: &QuickOpenQuery,
    result: &QuickOpenResult,
    preferred_index: usize,
) -> Option<String> {
    if let Some(label) = quick_open_take_previous_result_label_at(
        previous_results,
        previous_labels,
        parsed_query,
        result,
        preferred_index,
    ) {
        return Some(label);
    }

    previous_results
        .iter()
        .zip(previous_labels.iter_mut())
        .enumerate()
        .find_map(|(index, (previous_result, previous_label))| {
            let label = previous_label.as_ref()?;
            if index != preferred_index
                && quick_open_result_label_metadata_matches(previous_result, result, parsed_query)
                && quick_open_result_label_matches_result(previous_result, label, parsed_query)
            {
                return previous_label.take();
            }
            None
        })
}

fn quick_open_take_previous_result_label_at(
    previous_results: &[QuickOpenResult],
    previous_labels: &mut [Option<String>],
    parsed_query: &QuickOpenQuery,
    result: &QuickOpenResult,
    index: usize,
) -> Option<String> {
    let previous_result = previous_results.get(index)?;
    if !quick_open_result_label_metadata_matches(previous_result, result, parsed_query) {
        return None;
    }

    let previous_label = previous_labels.get_mut(index)?;
    if previous_label.as_ref().is_some_and(|label| {
        quick_open_result_label_matches_result(previous_result, label, parsed_query)
    }) {
        return previous_label.take();
    }

    None
}

fn quick_open_result_labels_match_results(
    results: &[QuickOpenResult],
    labels: &[String],
    parsed_query: &QuickOpenQuery,
) -> bool {
    labels.len() == results.len()
        && results.iter().zip(labels).all(|(result, label)| {
            quick_open_result_label_matches_result(result, label, parsed_query)
        })
}

fn quick_open_result_label_matches_result(
    result: &QuickOpenResult,
    label: &str,
    parsed_query: &QuickOpenQuery,
) -> bool {
    label
        == quick_open_result_label_with_navigation_line_column(
            &result.rel,
            parsed_query,
            result.navigation_line_column,
        )
}

fn quick_open_result_label_metadata_matches(
    previous_result: &QuickOpenResult,
    result: &QuickOpenResult,
    query: &QuickOpenQuery,
) -> bool {
    quick_open_paths_match(&previous_result.path, &result.path)
        && previous_result.rel == result.rel
        && (query.line.is_some()
            || previous_result.navigation_line_column == result.navigation_line_column)
}

fn quick_open_result_labels(
    results: &[QuickOpenResult],
    parsed_query: &QuickOpenQuery,
) -> Vec<String> {
    results
        .iter()
        .map(|result| {
            quick_open_result_label_with_navigation_line_column(
                &result.rel,
                parsed_query,
                result.navigation_line_column,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        quick_open_cache_ranking_inputs_match_ignoring_query_target,
        quick_open_empty_state_message, quick_open_indexing_pending, quick_open_open_target_at,
        quick_open_prepare_visible_rows, quick_open_refresh_cached_query_metadata,
        quick_open_refresh_stale_display_metadata, quick_open_results_with_reused_display_metadata,
        quick_open_visible_result_count, quick_open_window_size_for_available,
    };
    use crate::quick_open::{
        QUICK_OPEN_RESULT_LABEL_MAX_CHARS, QuickOpenQuery, QuickOpenResult, QuickOpenResultsCache,
        quick_open_index_file_identity,
    };
    use std::{collections::VecDeque, path::PathBuf};

    fn quick_open_result(
        rel: &str,
        navigation_line_column: Option<(usize, usize)>,
    ) -> QuickOpenResult {
        QuickOpenResult {
            rank_score: 120,
            fuzzy_score: 40,
            path: PathBuf::from("workspace").join(rel),
            rel: rel.to_owned(),
            navigation_line_column,
        }
    }

    fn quick_open_results_cache(
        parsed_query: QuickOpenQuery,
        results: Vec<QuickOpenResult>,
        result_labels: Vec<String>,
    ) -> QuickOpenResultsCache {
        let index_files = results
            .iter()
            .map(|result| result.path.clone())
            .collect::<Vec<_>>();
        QuickOpenResultsCache {
            query_input: parsed_query.pattern.clone(),
            index_generation: 7,
            index_file_identity: quick_open_index_file_identity(&index_files),
            recent_files: VecDeque::new(),
            open_files: Vec::new(),
            query_memory: VecDeque::new(),
            navigation_back: VecDeque::new(),
            navigation_forward: VecDeque::new(),
            current_navigation_location: None,
            parsed_query,
            result_labels,
            results,
        }
    }

    #[test]
    fn quick_open_window_size_uses_preferred_size_when_roomy() {
        assert_eq!(
            quick_open_window_size_for_available(1200.0, 900.0),
            [620.0, 420.0]
        );
    }

    #[test]
    fn quick_open_window_size_shrinks_to_available_viewport() {
        assert_eq!(
            quick_open_window_size_for_available(360.0, 260.0),
            [328.0, 164.0]
        );
    }

    #[test]
    fn quick_open_window_size_keeps_soft_minimum_when_possible() {
        assert_eq!(
            quick_open_window_size_for_available(300.0, 300.0),
            [268.0, 204.0]
        );
        assert_eq!(
            quick_open_window_size_for_available(f32::NAN, f32::INFINITY),
            [620.0, 420.0]
        );
    }

    #[test]
    fn quick_open_empty_state_reports_indexing_only_while_first_index_is_pending() {
        assert!(quick_open_indexing_pending(false, 0, Some(7)));
        assert!(!quick_open_indexing_pending(true, 0, Some(7)));
        assert!(!quick_open_indexing_pending(false, 2, Some(7)));
        assert!(!quick_open_indexing_pending(false, 0, None));
        assert_eq!(
            quick_open_empty_state_message(true),
            "Indexing workspace..."
        );
        assert_eq!(quick_open_empty_state_message(false), "No matching files");
    }

    #[test]
    fn quick_open_visible_result_count_requires_result_and_label() {
        let query = QuickOpenQuery {
            pattern: "src".to_owned(),
            line: None,
            column: 1,
        };
        let cache = quick_open_results_cache(
            query,
            vec![
                quick_open_result("src/main.rs", Some((4, 2))),
                quick_open_result("src/lib.rs", Some((8, 1))),
            ],
            vec!["src/main.rs:4:2".to_owned()],
        );

        assert_eq!(quick_open_visible_result_count(&cache), 1);
    }

    #[test]
    fn quick_open_open_target_ignores_rows_without_labels() {
        let query = QuickOpenQuery {
            pattern: "src".to_owned(),
            line: None,
            column: 1,
        };
        let cache = quick_open_results_cache(
            query,
            vec![
                quick_open_result("src/main.rs", Some((4, 2))),
                quick_open_result("src/lib.rs", Some((8, 1))),
            ],
            vec!["src/main.rs:4:2".to_owned()],
        );
        let visible_count = quick_open_visible_result_count(&cache);

        assert!(quick_open_open_target_at(&cache, 1, visible_count, "src").is_none());
    }

    #[test]
    fn quick_open_open_target_preserves_path_query_and_target_selection() {
        let query = QuickOpenQuery {
            pattern: "src/main.rs".to_owned(),
            line: Some(9),
            column: 3,
        };
        let result = quick_open_result("src/main.rs", Some((4, 2)));
        let expected_path = result.path.clone();
        let cache =
            quick_open_results_cache(query, vec![result], vec!["src/main.rs:9:3".to_owned()]);

        let visible_count = quick_open_visible_result_count(&cache);
        let target = quick_open_open_target_at(&cache, 0, visible_count, "src/main.rs")
            .expect("row should open");

        assert_eq!(target.path, expected_path);
        assert_eq!(target.line_column, Some((9, 3)));
        assert_eq!(target.query_pattern, "src/main.rs");
    }

    #[test]
    fn quick_open_open_target_rejects_stale_query_cache() {
        let query = QuickOpenQuery {
            pattern: "src/main.rs".to_owned(),
            line: None,
            column: 1,
        };
        let cache = quick_open_results_cache(
            query,
            vec![quick_open_result("src/main.rs", Some((4, 2)))],
            vec!["src/main.rs:4:2".to_owned()],
        );

        let visible_count = quick_open_visible_result_count(&cache);

        assert!(quick_open_open_target_at(&cache, 0, visible_count, "src/lib.rs").is_none());
    }

    #[test]
    fn quick_open_visible_rows_and_open_target_reject_stale_result_label() {
        let query = QuickOpenQuery {
            pattern: "src".to_owned(),
            line: None,
            column: 1,
        };
        let cache = quick_open_results_cache(
            query,
            vec![quick_open_result("src/main.rs", Some((4, 2)))],
            vec!["src/lib.rs:8:1".to_owned()],
        );

        let visible_count = quick_open_visible_result_count(&cache);

        assert_eq!(visible_count, 0);
        assert_eq!(
            quick_open_prepare_visible_rows(&cache, 0..1, visible_count).count(),
            0
        );
        assert!(quick_open_open_target_at(&cache, 0, visible_count, "src").is_none());
    }

    #[test]
    fn quick_open_prepare_visible_rows_clamps_to_prepared_labels() {
        let query = QuickOpenQuery {
            pattern: "src".to_owned(),
            line: None,
            column: 1,
        };
        let cache = quick_open_results_cache(
            query,
            vec![
                quick_open_result("src/main.rs", Some((4, 2))),
                quick_open_result("src/lib.rs", Some((8, 1))),
            ],
            vec!["src/main.rs:4:2".to_owned()],
        );

        let visible_count = quick_open_visible_result_count(&cache);
        let rows = quick_open_prepare_visible_rows(&cache, 0..3, visible_count).collect::<Vec<_>>();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].index, 0);
        assert_eq!(rows[0].label, "src/main.rs:4:2");
    }

    #[test]
    fn quick_open_reuses_cached_ranking_when_only_explicit_target_changes() {
        let previous_query = QuickOpenQuery {
            pattern: "src/main.rs".to_owned(),
            line: None,
            column: 1,
        };
        let mut cache = quick_open_results_cache(
            previous_query,
            vec![quick_open_result("src/main.rs", Some((4, 2)))],
            vec!["src/main.rs:4:2".to_owned()],
        );
        let cached_result_rel = cache.results[0].rel.as_ptr();
        let parsed_query = QuickOpenQuery {
            pattern: "src/main.rs".to_owned(),
            line: Some(9),
            column: 3,
        };
        let recent_files = VecDeque::new();
        let open_files: Vec<PathBuf> = Vec::new();
        let query_memory = VecDeque::new();

        assert!(quick_open_cache_ranking_inputs_match_ignoring_query_target(
            &cache,
            &parsed_query,
            7,
            &cache.index_file_identity,
            &recent_files,
            open_files.iter().map(PathBuf::as_path),
            &query_memory,
            &[],
        ));

        let navigation_back = VecDeque::new();
        let navigation_forward = VecDeque::new();
        quick_open_refresh_cached_query_metadata(
            &mut cache,
            "src/main.rs:9:3",
            parsed_query,
            &navigation_back,
            &navigation_forward,
            None,
            &[],
        );

        assert_eq!(cache.query_input, "src/main.rs:9:3");
        assert_eq!(cache.results[0].rel.as_ptr(), cached_result_rel);
        assert_eq!(cache.result_labels, vec!["src/main.rs:9:3"]);
    }

    #[test]
    fn quick_open_keeps_cached_labels_when_query_target_metadata_matches() {
        let query = QuickOpenQuery {
            pattern: "src/main.rs".to_owned(),
            line: Some(9),
            column: 3,
        };
        let cached_label = "src/main.rs:9:3".to_owned();
        let cached_label_text = cached_label.as_ptr();
        let mut cache = quick_open_results_cache(
            query.clone(),
            vec![quick_open_result("src/main.rs", Some((4, 2)))],
            vec![cached_label],
        );
        let navigation_back = VecDeque::new();
        let navigation_forward = VecDeque::new();

        quick_open_refresh_cached_query_metadata(
            &mut cache,
            "src/main.rs:09:03",
            query,
            &navigation_back,
            &navigation_forward,
            None,
            &[],
        );

        assert_eq!(cache.query_input, "src/main.rs:09:03");
        assert_eq!(cache.result_labels[0].as_ptr(), cached_label_text);
        assert_eq!(cache.result_labels, vec!["src/main.rs:9:3"]);
    }

    #[test]
    fn quick_open_refresh_rebuilds_stale_cached_labels_when_query_metadata_matches() {
        let query = QuickOpenQuery {
            pattern: "src/main.rs".to_owned(),
            line: None,
            column: 1,
        };
        let mut cache = quick_open_results_cache(
            query.clone(),
            vec![quick_open_result("src/main.rs", Some((4, 2)))],
            vec![format!(
                "src/main.rs:999:999\n{}",
                "x".repeat(QUICK_OPEN_RESULT_LABEL_MAX_CHARS)
            )],
        );
        let navigation_back = VecDeque::new();
        let navigation_forward = VecDeque::new();

        quick_open_refresh_cached_query_metadata(
            &mut cache,
            "src/main.rs",
            query,
            &navigation_back,
            &navigation_forward,
            None,
            &[],
        );

        assert_eq!(cache.result_labels, vec!["src/main.rs:4:2"]);
        assert!(cache.result_labels[0].chars().count() <= QUICK_OPEN_RESULT_LABEL_MAX_CHARS);
    }

    #[test]
    fn quick_open_exact_cache_hit_repairs_stale_display_metadata() {
        let query = QuickOpenQuery {
            pattern: "src".to_owned(),
            line: None,
            column: 1,
        };
        let mut cache = quick_open_results_cache(
            query,
            vec![
                quick_open_result("src/main.rs", Some((4, 2))),
                quick_open_result("src/lib.rs", Some((8, 1))),
            ],
            vec!["src/lib.rs:8:1".to_owned()],
        );

        assert_eq!(quick_open_visible_result_count(&cache), 0);

        let visible_count = quick_open_refresh_stale_display_metadata(&mut cache);

        assert_eq!(
            cache.result_labels,
            vec!["src/main.rs:4:2", "src/lib.rs:8:1"]
        );
        assert_eq!(visible_count, 2);
        assert_eq!(quick_open_visible_result_count(&cache), visible_count);
    }

    #[test]
    fn quick_open_reuses_display_metadata_when_results_match() {
        let query = QuickOpenQuery {
            pattern: "main".to_owned(),
            line: None,
            column: 1,
        };
        let cached_result = quick_open_result("src/main.rs", Some((4, 2)));
        let cached_result_rel = cached_result.rel.as_ptr();
        let cached_label = "src/main.rs:4:2".to_owned();
        let cached_label_text = cached_label.as_ptr();
        let cache =
            quick_open_results_cache(query.clone(), vec![cached_result], vec![cached_label]);
        let rebuilt_results = vec![quick_open_result("src/main.rs", Some((4, 2)))];

        let (results, labels) =
            quick_open_results_with_reused_display_metadata(rebuilt_results, &query, Some(cache));

        assert_eq!(results[0].rel.as_ptr(), cached_result_rel);
        assert_eq!(labels[0].as_ptr(), cached_label_text);
        assert_eq!(labels, vec!["src/main.rs:4:2"]);
    }

    #[test]
    fn quick_open_reuses_display_metadata_when_results_reorder() {
        let query = QuickOpenQuery {
            pattern: "src".to_owned(),
            line: None,
            column: 1,
        };
        let first_result = quick_open_result("src/lib.rs", Some((8, 1)));
        let second_result = quick_open_result("src/main.rs", Some((4, 2)));
        let first_label = "src/lib.rs:8:1".to_owned();
        let second_label = "src/main.rs:4:2".to_owned();
        let first_label_text = first_label.as_ptr();
        let second_label_text = second_label.as_ptr();
        let cache = quick_open_results_cache(
            query.clone(),
            vec![first_result, second_result],
            vec![first_label, second_label],
        );
        let rebuilt_results = vec![
            quick_open_result("src/main.rs", Some((4, 2))),
            quick_open_result("src/lib.rs", Some((8, 1))),
        ];

        let (_results, labels) =
            quick_open_results_with_reused_display_metadata(rebuilt_results, &query, Some(cache));

        assert_eq!(labels[0].as_ptr(), second_label_text);
        assert_eq!(labels[1].as_ptr(), first_label_text);
        assert_eq!(labels, vec!["src/main.rs:4:2", "src/lib.rs:8:1"]);
    }

    #[test]
    fn quick_open_reuses_display_metadata_for_lexically_equivalent_result_paths() {
        let query = QuickOpenQuery {
            pattern: "main".to_owned(),
            line: None,
            column: 1,
        };
        let mut cached_result = quick_open_result("src/main.rs", Some((4, 2)));
        cached_result.path = PathBuf::from("workspace/src/../src/main.rs");
        let cached_label = "src/main.rs:4:2".to_owned();
        let cached_label_text = cached_label.as_ptr();
        let cache =
            quick_open_results_cache(query.clone(), vec![cached_result], vec![cached_label]);
        let rebuilt_results = vec![quick_open_result("src/main.rs", Some((4, 2)))];

        let (_results, labels) =
            quick_open_results_with_reused_display_metadata(rebuilt_results, &query, Some(cache));

        assert_eq!(labels[0].as_ptr(), cached_label_text);
        assert_eq!(labels, vec!["src/main.rs:4:2"]);
    }

    #[test]
    fn quick_open_rebuilds_display_metadata_when_result_path_key_changes() {
        let query = QuickOpenQuery {
            pattern: "main".to_owned(),
            line: None,
            column: 1,
        };
        let mut cached_result = quick_open_result("src/main.rs", Some((4, 2)));
        cached_result.path = PathBuf::from("other-workspace/src/main.rs");
        let cached_label = "src/main.rs:4:2".to_owned();
        let cached_label_text = cached_label.as_ptr();
        let cache =
            quick_open_results_cache(query.clone(), vec![cached_result], vec![cached_label]);
        let rebuilt_results = vec![quick_open_result("src/main.rs", Some((4, 2)))];

        let (_results, labels) =
            quick_open_results_with_reused_display_metadata(rebuilt_results, &query, Some(cache));

        assert_ne!(labels[0].as_ptr(), cached_label_text);
        assert_eq!(labels, vec!["src/main.rs:4:2"]);
    }

    #[test]
    fn quick_open_rebuilds_display_metadata_when_explicit_target_changes() {
        let previous_query = QuickOpenQuery {
            pattern: "main".to_owned(),
            line: Some(4),
            column: 2,
        };
        let query = QuickOpenQuery {
            pattern: "main".to_owned(),
            line: Some(9),
            column: 3,
        };
        let cache = quick_open_results_cache(
            previous_query,
            vec![quick_open_result("src/main.rs", Some((4, 2)))],
            vec!["src/main.rs:4:2".to_owned()],
        );
        let rebuilt_results = vec![quick_open_result("src/main.rs", Some((4, 2)))];

        let (_results, labels) =
            quick_open_results_with_reused_display_metadata(rebuilt_results, &query, Some(cache));

        assert_eq!(labels, vec!["src/main.rs:9:3"]);
    }

    #[test]
    fn quick_open_rebuilds_stale_display_metadata_when_results_match() {
        let query = QuickOpenQuery {
            pattern: "main".to_owned(),
            line: None,
            column: 1,
        };
        let cache = quick_open_results_cache(
            query.clone(),
            vec![quick_open_result("src/main.rs", Some((4, 2)))],
            vec![format!(
                "src/main.rs:999:999\n{}",
                "x".repeat(QUICK_OPEN_RESULT_LABEL_MAX_CHARS)
            )],
        );
        let rebuilt_results = vec![quick_open_result("src/main.rs", Some((4, 2)))];

        let (_results, labels) =
            quick_open_results_with_reused_display_metadata(rebuilt_results, &query, Some(cache));

        assert_eq!(labels, vec!["src/main.rs:4:2"]);
        assert!(labels[0].chars().count() <= QUICK_OPEN_RESULT_LABEL_MAX_CHARS);
    }
}
