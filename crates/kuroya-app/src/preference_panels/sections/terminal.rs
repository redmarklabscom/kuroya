use eframe::egui;
use kuroya_core::EditorSettings;

use super::SettingsHighlightState;

mod buffer_text;
mod color_feedback;
mod cursor;
mod interaction;
mod profile;

pub(super) fn render_terminal_settings(
    ui: &mut egui::Ui,
    draft: &mut EditorSettings,
    highlight: &mut SettingsHighlightState<'_>,
) {
    ui.set_width(ui.available_width());
    profile::render_profile_settings_with_highlight(ui, draft, highlight);
    terminal_section_separator(ui);
    buffer_text::render_buffer_text_settings_with_highlight(ui, draft, highlight);
    terminal_section_separator(ui);
    cursor::render_cursor_settings_with_highlight(ui, draft, highlight);
    terminal_section_separator(ui);
    color_feedback::render_color_feedback_settings_with_highlight(ui, draft, highlight);
    terminal_section_separator(ui);
    interaction::render_interaction_settings_with_highlight(ui, draft, highlight);
}

fn terminal_section_separator(ui: &mut egui::Ui) {
    ui.add_space(10.0);
    ui.separator();
}
