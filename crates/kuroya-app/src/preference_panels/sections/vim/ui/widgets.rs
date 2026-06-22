use super::super::capture::{
    VimKeyCaptureState, VimKeyCaptureTarget, cancel_vim_key_capture, vim_key_capture_button_enabled,
};
use crate::ui_icons::{IconKind, icon_button};
use eframe::egui;
use kuroya_core::EditorVimSettings;

pub(super) const VIM_ACTION_LABEL_WIDTH: f32 = 170.0;
pub(super) const VIM_DEFAULT_LABEL_WIDTH: f32 = 52.0;
pub(super) const VIM_BINDING_EDIT_WIDTH: f32 = 96.0;
pub(super) const VIM_SEQUENCE_EDIT_WIDTH: f32 = 150.0;

pub(super) fn vim_icon_button_enabled(
    ui: &mut egui::Ui,
    enabled: bool,
    icon: IconKind,
    tooltip: &str,
) -> egui::Response {
    ui.add_enabled_ui(enabled, |ui| icon_button(ui, icon, tooltip))
        .inner
}

pub(super) fn render_vim_capture_key_button(
    ui: &mut egui::Ui,
    vim: &mut EditorVimSettings,
    state: &mut VimKeyCaptureState,
    target: VimKeyCaptureTarget,
    current_value: &str,
) {
    let capturing = state.is_capturing(target);
    let icon = if capturing {
        IconKind::Close
    } else {
        IconKind::Keyboard
    };
    let tooltip = if capturing {
        "Cancel Vim key capture"
    } else {
        "Capture a Vim key for this binding"
    };
    if vim_icon_button_enabled(
        ui,
        vim_key_capture_button_enabled(state, target),
        icon,
        tooltip,
    )
    .clicked()
    {
        if capturing {
            cancel_vim_key_capture(vim, state);
        } else {
            ui.ctx().memory_mut(|memory| memory.stop_text_input());
            state.start(target, current_value.to_owned());
        }
    }
}

pub(super) fn vim_capture_hint(
    ui: &mut egui::Ui,
    state: &VimKeyCaptureState,
    target: VimKeyCaptureTarget,
) {
    ui.label(
        egui::RichText::new(vim_capture_hint_text(state, target))
            .small()
            .color(ui.visuals().weak_text_color()),
    );
}

pub(super) fn vim_capture_hint_text(
    state: &VimKeyCaptureState,
    target: VimKeyCaptureTarget,
) -> &'static str {
    if state
        .escape_cancel
        .is_some_and(|pending| pending.target == target)
    {
        if state.error_for(target).is_some() {
            "Esc was rejected. Press Esc again to cancel."
        } else {
            "Esc captured. Press Esc again to cancel."
        }
    } else {
        "Press one Vim key. Esc once sets <Esc>; Esc twice cancels."
    }
}

pub(super) fn settings_capture_button_width(ui: &egui::Ui) -> f32 {
    ui.spacing().interact_size.y.max(34.0) + ui.spacing().item_spacing.x
}

pub(super) fn vim_settings_error(ui: &mut egui::Ui, message: &str) {
    ui.colored_label(ui.visuals().error_fg_color, message);
}
