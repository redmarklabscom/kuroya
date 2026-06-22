use super::super::{
    capture::{VimKeyCaptureState, vim_key_capture_manual_controls_enabled},
    editing::{
        command_vim_key_override, default_custom_disabled_vim_binding, vim_key_override_to_keys,
    },
};
use eframe::egui;
use kuroya_core::EditorVimSettings;

mod disabled_rows;
mod override_rows;

pub(super) fn render_custom_bindings(
    ui: &mut egui::Ui,
    vim: &mut EditorVimSettings,
    capture_state: &mut VimKeyCaptureState,
) {
    ui.vertical(|ui| {
        render_create_custom_combo(ui, vim, capture_state);
        disabled_rows::render_custom_disabled_bindings(ui, vim, capture_state);
        override_rows::render_custom_key_overrides(ui, vim, capture_state);
    });
}

fn render_create_custom_combo(
    ui: &mut egui::Ui,
    vim: &mut EditorVimSettings,
    capture_state: &VimKeyCaptureState,
) {
    ui.add_enabled_ui(
        vim_key_capture_manual_controls_enabled(capture_state),
        |ui| {
            egui::ComboBox::from_id_salt("settings_vim_create_custom_combo")
                .selected_text("Create custom")
                .show_ui(ui, |ui| {
                    let can_disable = default_custom_disabled_vim_binding(vim).is_some();
                    if ui
                        .add_enabled_ui(can_disable, |ui| {
                            ui.selectable_label(false, "Disable a Vim key")
                        })
                        .inner
                        .clicked()
                        && let Some(binding) = default_custom_disabled_vim_binding(vim)
                    {
                        vim.disabled_bindings.push(binding);
                        ui.close();
                    }
                    let can_remap = vim_key_override_to_keys(vim).is_some();
                    if ui
                        .add_enabled_ui(can_remap, |ui| {
                            ui.selectable_label(false, "Remap to Vim keys")
                        })
                        .inner
                        .clicked()
                        && let Some(binding) = vim_key_override_to_keys(vim)
                    {
                        vim.key_overrides.push(binding);
                        ui.close();
                    }
                    let can_command = command_vim_key_override(vim).is_some();
                    if ui
                        .add_enabled_ui(can_command, |ui| {
                            ui.selectable_label(false, "Run an app command")
                        })
                        .inner
                        .clicked()
                        && let Some(binding) = command_vim_key_override(vim)
                    {
                        vim.key_overrides.push(binding);
                        ui.close();
                    }
                });
        },
    );
}
