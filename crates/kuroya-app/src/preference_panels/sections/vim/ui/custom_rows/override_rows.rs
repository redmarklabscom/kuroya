use super::super::super::{
    capture::{VimKeyCaptureState, VimKeyCaptureTarget},
    editing::custom_override_indices,
};
use super::super::widgets::vim_icon_button_enabled;
use crate::ui_icons::IconKind;
use eframe::egui;
use kuroya_core::EditorVimSettings;

mod after;
mod before;
mod command;
mod target;

pub(super) fn render_custom_key_overrides(
    ui: &mut egui::Ui,
    vim: &mut EditorVimSettings,
    capture_state: &mut VimKeyCaptureState,
) {
    let visible_indices = custom_override_indices(vim);
    let mut remove_override = None;
    for (row_index, index) in visible_indices.into_iter().enumerate() {
        if row_index > 0 {
            ui.add_space(4.0);
        }
        ui.push_id(("vim_key_override", index), |ui| {
            let mut remove_current = false;
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(custom_override_row_label(row_index)).strong());
                    if vim_icon_button_enabled(
                        ui,
                        super::super::super::capture::vim_key_capture_manual_controls_enabled(
                            capture_state,
                        ),
                        IconKind::Trash,
                        "Remove Vim override",
                    )
                    .clicked()
                    {
                        remove_current = true;
                    }
                });

                before::render_custom_override_before(ui, vim, index, capture_state);
                target::render_custom_override_target(ui, vim, index, capture_state);
                render_custom_override_target_value(ui, vim, index, capture_state);
            });
            if remove_current {
                remove_override = Some(index);
            }
        });
    }
    if let Some(index) = remove_override {
        vim.key_overrides.remove(index);
        if matches!(
            capture_state.target,
            Some(
                VimKeyCaptureTarget::CustomOverrideBefore(target_index)
                    | VimKeyCaptureTarget::CustomOverrideAfter(target_index)
            ) if target_index >= index
        ) {
            capture_state.clear_all();
        }
    }
}

fn custom_override_row_label(row_index: usize) -> String {
    format!("Custom {}", row_index + 1)
}

fn render_custom_override_target_value(
    ui: &mut egui::Ui,
    vim: &mut EditorVimSettings,
    index: usize,
    capture_state: &mut VimKeyCaptureState,
) {
    match vim.key_overrides[index].command.as_mut() {
        Some(app_command) => {
            command::render_command_combo_row(ui, index, app_command, capture_state)
        }
        None => after::render_custom_override_after(ui, vim, index, capture_state),
    }
}

#[cfg(test)]
mod tests {
    use super::custom_override_row_label;

    #[test]
    fn custom_override_row_labels_follow_visible_order() {
        assert_eq!(custom_override_row_label(0), "Custom 1");
        assert_eq!(custom_override_row_label(1), "Custom 2");
    }
}
