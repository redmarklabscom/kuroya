use super::{
    SETTINGS_TARGET_GENERAL, SettingsHighlightState, guarded_f32_drag_value, settings_target_block,
};
use eframe::egui;
use kuroya_core::{
    DEFAULT_WINDOW_ZOOM_LEVEL, EditorAutoSaveMode, EditorSettings, MAX_AUTOSAVE_DELAY_MS,
    MAX_WINDOW_ZOOM_LEVEL, MIN_AUTOSAVE_DELAY_MS, MIN_WINDOW_ZOOM_LEVEL,
};

pub(super) fn render_general_settings_with_highlight(
    ui: &mut egui::Ui,
    draft: &mut EditorSettings,
    highlight: &mut SettingsHighlightState<'_>,
) {
    settings_target_block(ui, highlight, SETTINGS_TARGET_GENERAL, |ui| {
        render_general_settings_content(ui, draft);
    });
}

fn render_general_settings_content(ui: &mut egui::Ui, draft: &mut EditorSettings) {
    egui::Grid::new("settings_general_autosave_grid")
        .num_columns(2)
        .spacing([18.0, 10.0])
        .show(ui, |ui| {
            ui.label("Autosave");
            autosave_mode_combo(ui, "settings_autosave_mode", draft);
            ui.end_row();

            let autosave_after_delay =
                draft.effective_autosave_mode() == EditorAutoSaveMode::AfterDelay;
            ui.add_enabled_ui(autosave_after_delay, |ui| {
                ui.label("Autosave delay");
            });
            ui.add_enabled_ui(autosave_after_delay, |ui| {
                ui.add(
                    egui::DragValue::new(&mut draft.autosave_delay_ms)
                        .speed(250.0)
                        .suffix(" ms")
                        .range(MIN_AUTOSAVE_DELAY_MS..=MAX_AUTOSAVE_DELAY_MS),
                );
            });
            ui.end_row();
        });

    egui::Grid::new("settings_general_window_grid")
        .num_columns(2)
        .spacing([18.0, 10.0])
        .show(ui, |ui| {
            ui.label("Window zoom");
            guarded_f32_drag_value(
                ui,
                &mut draft.window_zoom_level,
                0.1,
                MIN_WINDOW_ZOOM_LEVEL..=MAX_WINDOW_ZOOM_LEVEL,
                DEFAULT_WINDOW_ZOOM_LEVEL,
            );
            ui.end_row();
        });

    ui.checkbox(&mut draft.minimap, "Minimap");
    ui.checkbox(&mut draft.smooth_scrolling, "Smooth scroll");
    ui.checkbox(
        &mut draft.scroll_beyond_last_line,
        "Scroll beyond last line",
    );
    ui.checkbox(&mut draft.status_bar_visible, "Status bar");
    ui.checkbox(
        &mut draft.devtools_verbose_logging,
        "Verbose devtools logging",
    );
    ui.add_enabled_ui(cfg!(debug_assertions), |ui| {
        ui.checkbox(&mut draft.devtools_profiling_enabled, "Devtools profiling");
    });
}

fn autosave_mode_combo(ui: &mut egui::Ui, id: &'static str, draft: &mut EditorSettings) {
    let mut mode = draft.effective_autosave_mode();
    egui::ComboBox::from_id_salt(id)
        .selected_text(autosave_mode_label(mode))
        .show_ui(ui, |ui| {
            for candidate in [
                EditorAutoSaveMode::Off,
                EditorAutoSaveMode::AfterDelay,
                EditorAutoSaveMode::OnFocusChange,
                EditorAutoSaveMode::OnWindowChange,
            ] {
                ui.selectable_value(&mut mode, candidate, autosave_mode_label(candidate));
            }
        });

    draft.autosave = mode != EditorAutoSaveMode::Off;
    draft.autosave_mode = mode;
}

fn autosave_mode_label(mode: EditorAutoSaveMode) -> &'static str {
    match mode {
        EditorAutoSaveMode::Off => "Off",
        EditorAutoSaveMode::AfterDelay => "After delay",
        EditorAutoSaveMode::OnFocusChange => "On focus change",
        EditorAutoSaveMode::OnWindowChange => "On window change",
    }
}

#[cfg(test)]
mod tests {
    use super::autosave_mode_label;
    use kuroya_core::EditorAutoSaveMode;

    #[test]
    fn autosave_mode_label_names_modes() {
        assert_eq!(autosave_mode_label(EditorAutoSaveMode::Off), "Off");
        assert_eq!(
            autosave_mode_label(EditorAutoSaveMode::AfterDelay),
            "After delay"
        );
    }
}
