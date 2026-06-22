use super::super::super::super::{
    capture::{VimKeyCaptureState, vim_key_capture_manual_controls_enabled},
    editing::{default_vim_override_command, switch_vim_override_to_keys},
};
use eframe::egui;
use kuroya_core::EditorVimKeyOverride;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VimOverrideTarget {
    Keys,
    Command,
}

pub(super) fn render_custom_override_target(
    ui: &mut egui::Ui,
    vim: &mut kuroya_core::EditorVimSettings,
    index: usize,
    capture_state: &VimKeyCaptureState,
) {
    ui.horizontal(|ui| {
        ui.label("Target");
        render_override_target_combo(
            ui,
            index,
            &mut vim.key_overrides[index],
            vim_key_capture_manual_controls_enabled(capture_state),
        );
    });
}

fn render_override_target_combo(
    ui: &mut egui::Ui,
    index: usize,
    binding: &mut EditorVimKeyOverride,
    enabled: bool,
) {
    let mut target = if binding.command.is_some() {
        VimOverrideTarget::Command
    } else {
        VimOverrideTarget::Keys
    };
    let previous_target = target;
    ui.add_enabled_ui(enabled, |ui| {
        egui::ComboBox::from_id_salt(("vim_override_target", index))
            .selected_text(vim_override_target_label(target))
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut target,
                    VimOverrideTarget::Keys,
                    vim_override_target_label(VimOverrideTarget::Keys),
                );
                ui.selectable_value(
                    &mut target,
                    VimOverrideTarget::Command,
                    vim_override_target_label(VimOverrideTarget::Command),
                );
            });
    });

    if enabled && target != previous_target {
        match target {
            VimOverrideTarget::Keys => switch_vim_override_to_keys(binding),
            VimOverrideTarget::Command => {
                binding.after.clear();
                if binding.command.is_none() {
                    binding.command = Some(default_vim_override_command());
                }
            }
        }
    }
}

fn vim_override_target_label(target: VimOverrideTarget) -> &'static str {
    match target {
        VimOverrideTarget::Keys => "Vim keys",
        VimOverrideTarget::Command => "Command",
    }
}
