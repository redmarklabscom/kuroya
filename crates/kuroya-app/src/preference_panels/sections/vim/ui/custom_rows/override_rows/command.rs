use super::super::super::super::capture::{
    VimKeyCaptureState, vim_key_capture_manual_controls_enabled,
};
use crate::{command_catalog::command_catalog_slice, commands::command_label};
use eframe::egui;
use kuroya_core::Command;

pub(super) fn render_command_combo_row(
    ui: &mut egui::Ui,
    index: usize,
    command: &mut Command,
    capture_state: &VimKeyCaptureState,
) {
    ui.horizontal(|ui| {
        ui.label("Command");
        render_command_combo(
            ui,
            index,
            command,
            vim_key_capture_manual_controls_enabled(capture_state),
        );
    });
}

fn render_command_combo(ui: &mut egui::Ui, index: usize, command: &mut Command, enabled: bool) {
    ui.add_enabled_ui(enabled, |ui| {
        egui::ComboBox::from_id_salt(("vim_override_command", index))
            .selected_text(command_label(command))
            .show_ui(ui, |ui| {
                for catalog_command in command_catalog_slice() {
                    ui.selectable_value(
                        command,
                        catalog_command.clone(),
                        command_label(catalog_command),
                    );
                }
            });
    });
}
