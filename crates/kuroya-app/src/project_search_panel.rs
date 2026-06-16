use crate::{
    KuroyaApp,
    project_search_panel::{
        controls::{render_project_search_controls, sanitize_project_search_inputs},
        results::{
            ProjectSearchOpenTarget, project_search_open_target, project_search_result_row_height,
        },
    },
    ui_state::{
        clamp_selection, handle_list_navigation_keys, plain_key_pressed, selection_page_step,
    },
};
use eframe::egui::{self, Key};

mod controls;
mod results;

impl KuroyaApp {
    pub(crate) fn render_project_search(&mut self, ui: &mut egui::Ui) {
        let controls = render_project_search_controls(self, ui);

        let mut results_match_query = self.project_search_results_match_current_query();
        let mut open_selected = false;
        let mut run_search = controls.search_requested;

        let result_count = self.project_search_result.matches.len();
        clamp_selection(&mut self.project_search_selected, result_count);
        let selection_changed = if controls.input_has_focus || result_count == 0 {
            false
        } else {
            let row_height = project_search_result_row_height(ui);
            let viewport_height = ui.available_height();
            ui.input(|input| {
                handle_list_navigation_keys(
                    input,
                    &mut self.project_search_selected,
                    result_count,
                    selection_page_step(row_height, viewport_height),
                )
            })
        };
        if ui.input(|input| plain_key_pressed(input, Key::Enter)) {
            if !controls.input_has_focus && results_match_query && result_count > 0 {
                open_selected = true;
            } else {
                run_search = true;
            }
        }
        ui.separator();

        if open_selected {
            if let Some(target) = project_search_open_target(
                &self.project_search_result,
                self.project_search_selected,
                results_match_query,
            ) {
                open_project_search_target(self, target);
            }
        }
        if run_search {
            sanitize_project_search_inputs(self);
            self.spawn_project_search();
            if mark_project_search_results_pending(
                &mut self.project_search_result_query,
                &self.project_search_query,
            ) {
                results_match_query = false;
            }
        }

        let current_query = self.project_search_query.trim();
        if let Some(target) = results::render_project_search_results(
            ui,
            &self.workspace.root,
            &self.project_search_result,
            current_query,
            results_match_query,
            &mut self.project_search_selected,
            selection_changed,
        ) {
            open_project_search_target(self, target);
        }
    }
}

fn open_project_search_target(app: &mut KuroyaApp, target: ProjectSearchOpenTarget) {
    app.open_file_at_known_openable(target.path, target.line, target.column);
}

fn mark_project_search_results_pending(result_query: &mut String, current_query: &str) -> bool {
    if current_query.trim().is_empty() {
        return false;
    }

    result_query.clear();
    true
}

#[cfg(test)]
mod tests {
    use super::mark_project_search_results_pending;

    #[test]
    fn non_empty_submitted_project_search_marks_existing_results_pending() {
        let mut result_query = "needle".to_owned();

        let pending = mark_project_search_results_pending(&mut result_query, " needle ");

        assert!(pending);
        assert!(result_query.is_empty());
    }

    #[test]
    fn empty_submitted_project_search_leaves_result_token_to_spawn_clear_path() {
        let mut result_query = "needle".to_owned();

        let pending = mark_project_search_results_pending(&mut result_query, "   ");

        assert!(!pending);
        assert_eq!(result_query, "needle");
    }
}
