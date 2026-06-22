use super::super::{
    VimBindingOwner,
    builtins::{VimBuiltInBinding, vim_builtin_bindings},
    capture::{VimKeyCaptureState, VimKeyCaptureTarget, vim_key_capture_manual_controls_enabled},
    editing::{
        disable_builtin_vim_binding, normalized_vim_binding_edit, reset_builtin_vim_binding,
        set_builtin_vim_binding, vim_binding_edit_error, vim_binding_existing_error,
        vim_builtin_effective_binding,
    },
};
use super::widgets::{
    VIM_ACTION_LABEL_WIDTH, VIM_BINDING_EDIT_WIDTH, VIM_DEFAULT_LABEL_WIDTH,
    render_vim_capture_key_button, vim_capture_hint, vim_icon_button_enabled, vim_settings_error,
};
use crate::{
    preference_panels::sections::bounded_singleline_text_edit_with_hint, ui_icons::IconKind,
};
use eframe::egui;
use kuroya_core::EditorVimSettings;

pub(super) fn render_builtin_bindings(
    ui: &mut egui::Ui,
    vim: &mut EditorVimSettings,
    capture_state: &mut VimKeyCaptureState,
) {
    ui.vertical(|ui| {
        for binding in vim_builtin_bindings() {
            render_builtin_binding_row(ui, vim, binding, capture_state);
        }
    });
}

fn render_builtin_binding_row(
    ui: &mut egui::Ui,
    vim: &mut EditorVimSettings,
    binding: VimBuiltInBinding,
    capture_state: &mut VimKeyCaptureState,
) {
    let capture_target = VimKeyCaptureTarget::BuiltIn(binding.default);
    let current_binding = vim_builtin_effective_binding(vim, binding.default);
    let mut edited_binding = current_binding.clone();
    let manual_controls_enabled = vim_key_capture_manual_controls_enabled(capture_state);
    let mut error = vim_binding_existing_error(
        &edited_binding,
        VimBindingOwner::BuiltIn(binding.default),
        vim,
    );

    ui.horizontal(|ui| {
        ui.add_sized(
            [VIM_ACTION_LABEL_WIDTH, ui.spacing().interact_size.y],
            egui::Label::new(binding.label),
        );
        ui.add_sized(
            [VIM_DEFAULT_LABEL_WIDTH, ui.spacing().interact_size.y],
            egui::Label::new(egui::RichText::new(binding.default).monospace()),
        );
        let response = ui
            .add_enabled_ui(manual_controls_enabled, |ui| {
                bounded_singleline_text_edit_with_hint(
                    ui,
                    &mut edited_binding,
                    VIM_BINDING_EDIT_WIDTH,
                    Some("disabled"),
                )
            })
            .inner;
        if response.changed() {
            let candidate = normalized_vim_binding_edit(&edited_binding);
            match vim_binding_edit_error(&candidate, VimBindingOwner::BuiltIn(binding.default), vim)
            {
                Some(message) => error = Some(message),
                None => {
                    set_builtin_vim_binding(vim, binding.default, candidate);
                    capture_state.clear_error(capture_target);
                }
            }
        }
        render_vim_capture_key_button(
            ui,
            vim,
            capture_state,
            capture_target,
            current_binding.as_str(),
        );
        if vim_icon_button_enabled(
            ui,
            manual_controls_enabled,
            IconKind::Refresh,
            "Reset Vim binding",
        )
        .clicked()
        {
            reset_builtin_vim_binding(vim, binding.default);
            error = None;
            capture_state.clear_error(capture_target);
        }
        if vim_icon_button_enabled(
            ui,
            manual_controls_enabled,
            IconKind::Trash,
            "Disable Vim binding",
        )
        .clicked()
        {
            disable_builtin_vim_binding(vim, binding.default);
            error = None;
            capture_state.clear_error(capture_target);
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
}
