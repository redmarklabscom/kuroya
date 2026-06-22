use super::super::super::super::{
    VimBindingOwner,
    capture::{VimKeyCaptureState, VimKeyCaptureTarget, vim_key_capture_manual_controls_enabled},
    editing::{
        normalized_vim_binding_edit, vim_binding_existing_error, vim_override_before_edit_error,
    },
};
use super::super::super::widgets::{
    VIM_BINDING_EDIT_WIDTH, render_vim_capture_key_button, vim_capture_hint, vim_settings_error,
};
use crate::preference_panels::sections::bounded_singleline_text_edit_with_hint;
use eframe::egui;
use kuroya_core::EditorVimSettings;

pub(super) fn render_custom_override_before(
    ui: &mut egui::Ui,
    vim: &mut EditorVimSettings,
    index: usize,
    capture_state: &mut VimKeyCaptureState,
) {
    let capture_target = VimKeyCaptureTarget::CustomOverrideBefore(index);
    let mut edited_before = vim.key_overrides[index].before.clone();
    let manual_controls_enabled = vim_key_capture_manual_controls_enabled(capture_state);
    let mut error =
        vim_binding_existing_error(&edited_before, VimBindingOwner::CustomOverride(index), vim);

    ui.horizontal(|ui| {
        ui.label("Before");
        if ui
            .add_enabled_ui(manual_controls_enabled, |ui| {
                bounded_singleline_text_edit_with_hint(
                    ui,
                    &mut edited_before,
                    VIM_BINDING_EDIT_WIDTH,
                    Some("<Space>f"),
                )
            })
            .inner
            .changed()
        {
            let candidate = normalized_vim_binding_edit(&edited_before);
            match vim_override_before_edit_error(
                &candidate,
                VimBindingOwner::CustomOverride(index),
                vim,
            ) {
                Some(message) => error = Some(message),
                None => {
                    vim.key_overrides[index].before = candidate;
                    capture_state.clear_error(capture_target);
                }
            }
        }
        let current_capture_value = vim.key_overrides[index].before.clone();
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
