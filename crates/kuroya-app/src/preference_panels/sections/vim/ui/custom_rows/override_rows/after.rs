use super::super::super::super::{
    capture::{VimKeyCaptureState, VimKeyCaptureTarget, vim_key_capture_manual_controls_enabled},
    editing::{apply_custom_override_after_edit, vim_after_sequence_error},
};
use super::super::super::widgets::{
    VIM_SEQUENCE_EDIT_WIDTH, render_vim_capture_key_button, settings_capture_button_width,
    vim_capture_hint, vim_settings_error,
};
use crate::preference_panels::sections::{
    bounded_settings_text_edit_width, bounded_singleline_text_edit_with_hint,
};
use eframe::egui;
use kuroya_core::EditorVimSettings;

pub(super) fn render_custom_override_after(
    ui: &mut egui::Ui,
    vim: &mut EditorVimSettings,
    index: usize,
    capture_state: &mut VimKeyCaptureState,
) {
    let capture_target = VimKeyCaptureTarget::CustomOverrideAfter(index);
    let manual_controls_enabled = vim_key_capture_manual_controls_enabled(capture_state);
    let mut edited_after = vim.key_overrides[index].after.clone();
    let mut error = vim_after_sequence_error(&edited_after);
    ui.horizontal(|ui| {
        ui.label("After");
        let edit_width = bounded_settings_text_edit_width(
            ui.available_width() - settings_capture_button_width(ui),
            VIM_SEQUENCE_EDIT_WIDTH,
        );
        let response = ui
            .add_enabled_ui(manual_controls_enabled, |ui| {
                bounded_singleline_text_edit_with_hint(ui, &mut edited_after, edit_width, Some("0"))
            })
            .inner;
        if response.changed() {
            error = apply_custom_override_after_edit(vim, index, &edited_after);
            if error.is_none() {
                capture_state.clear_error(capture_target);
            }
        }
        let current_capture_value = vim.key_overrides[index].after.clone();
        render_vim_capture_key_button(
            ui,
            vim,
            capture_state,
            capture_target,
            current_capture_value.as_str(),
        );
    });
    if capture_state.is_capturing(capture_target) {
        vim_capture_hint(ui, capture_state, capture_target);
    }
    if let Some(message) = capture_state
        .error_for(capture_target)
        .map(str::to_owned)
        .or(error)
    {
        vim_settings_error(ui, &message);
    }
}
