use crate::preference_panels::sections::{
    SETTINGS_TARGET_FILES_SAVE_ACTIONS, SETTINGS_TARGET_FILES_SAVE_CLEANUP, SettingsHighlightState,
    settings_target_heading,
};
use eframe::egui;
use kuroya_core::EditorSettings;

pub(super) fn render_files_settings_with_highlight(
    ui: &mut egui::Ui,
    draft: &mut EditorSettings,
    highlight: &mut SettingsHighlightState<'_>,
) {
    settings_target_heading(
        ui,
        highlight,
        SETTINGS_TARGET_FILES_SAVE_ACTIONS,
        "Save Actions",
    );
    egui::Grid::new("settings_files_save_actions_grid")
        .num_columns(2)
        .spacing([18.0, 10.0])
        .show(ui, |ui| {
            ui.label("Format on save");
            ui.checkbox(&mut draft.format_on_save, "Format before writing files");
            ui.end_row();
        });

    ui.add_space(12.0);
    settings_target_heading(
        ui,
        highlight,
        SETTINGS_TARGET_FILES_SAVE_CLEANUP,
        "Save Cleanup",
    );
    egui::Grid::new("settings_files_save_cleanup_grid")
        .num_columns(2)
        .spacing([18.0, 10.0])
        .show(ui, |ui| {
            ui.label("Trim trailing whitespace");
            ui.checkbox(
                &mut draft.trim_trailing_whitespace,
                "Remove trailing spaces and tabs on save",
            );
            ui.end_row();

            ui.label("Insert final newline");
            ui.checkbox(&mut draft.insert_final_newline, "End files with a newline");
            ui.end_row();

            ui.label("Trim final newlines");
            ui.checkbox(
                &mut draft.trim_final_newlines,
                "Keep one final newline at most",
            );
            ui.end_row();
        });
}
