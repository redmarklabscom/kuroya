use crate::preference_panels::sections::{
    SETTINGS_TARGET_VIM_KEYBINDINGS, SettingsHighlightState, settings_target_block,
};
use eframe::egui;
use kuroya_core::EditorSettings;

pub(super) fn render_vim_settings(
    ui: &mut egui::Ui,
    draft: &mut EditorSettings,
    highlight: &mut SettingsHighlightState<'_>,
) {
    settings_target_block(ui, highlight, SETTINGS_TARGET_VIM_KEYBINDINGS, |ui| {
        ui.label(egui::RichText::new("Vim Keybindings").strong());
        egui::Grid::new("settings_vim_keybindings_grid")
            .num_columns(2)
            .spacing([18.0, 10.0])
            .show(ui, |ui| {
                ui.label("Vim keybindings");
                ui.checkbox(&mut draft.vim_keybindings, "Use modal editor keys");
                ui.end_row();
            });
    });
}
