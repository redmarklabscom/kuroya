use super::super::super::{
    VimBindingOwner,
    capture::{VimKeyCaptureState, VimKeyCaptureTarget, vim_key_capture_manual_controls_enabled},
    editing::{
        custom_disabled_binding_indices, normalized_vim_binding_edit,
        vim_disabled_binding_edit_error,
    },
};
use super::super::widgets::{
    VIM_BINDING_EDIT_WIDTH, render_vim_capture_key_button, vim_capture_hint,
    vim_icon_button_enabled, vim_settings_error,
};
use crate::{
    preference_panels::sections::bounded_singleline_text_edit_with_hint, ui_icons::IconKind,
};
use eframe::egui;
use kuroya_core::EditorVimSettings;

pub(super) fn render_custom_disabled_bindings(
    ui: &mut egui::Ui,
    vim: &mut EditorVimSettings,
    capture_state: &mut VimKeyCaptureState,
) {
    let visible_indices = custom_disabled_binding_indices(vim);
    if visible_indices.is_empty() {
        return;
    }

    let mut remove_binding = None;
    for (row_index, index) in visible_indices.into_iter().enumerate() {
        let capture_target = VimKeyCaptureTarget::CustomDisabled(index);
        let manual_controls_enabled = vim_key_capture_manual_controls_enabled(capture_state);
        let mut edited_binding = vim.disabled_bindings[index].clone();
        let mut error = vim_disabled_binding_edit_error(
            &edited_binding,
            VimBindingOwner::CustomDisabled(index),
            vim,
        );
        ui.push_id(("vim_custom_disabled_binding", index), |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("Disabled {}", row_index + 1));
                if ui
                    .add_enabled_ui(manual_controls_enabled, |ui| {
                        bounded_singleline_text_edit_with_hint(
                            ui,
                            &mut edited_binding,
                            VIM_BINDING_EDIT_WIDTH,
                            Some("q"),
                        )
                    })
                    .inner
                    .changed()
                {
                    let candidate = normalized_vim_binding_edit(&edited_binding);
                    match vim_disabled_binding_edit_error(
                        &candidate,
                        VimBindingOwner::CustomDisabled(index),
                        vim,
                    ) {
                        Some(message) => error = Some(message),
                        None => {
                            vim.disabled_bindings[index] = candidate;
                            capture_state.clear_error(capture_target);
                        }
                    }
                }
                let current_capture_value = vim.disabled_bindings[index].clone();
                render_vim_capture_key_button(
                    ui,
                    vim,
                    capture_state,
                    capture_target,
                    current_capture_value.as_str(),
                );
                if vim_icon_button_enabled(
                    ui,
                    manual_controls_enabled,
                    IconKind::Trash,
                    "Remove disabled Vim binding",
                )
                .clicked()
                {
                    remove_binding = Some(index);
                }
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
        });
    }
    if let Some(index) = remove_binding {
        vim.disabled_bindings.remove(index);
        if matches!(
            capture_state.target,
            Some(VimKeyCaptureTarget::CustomDisabled(_))
        ) {
            capture_state.clear_all();
        }
    }
}
