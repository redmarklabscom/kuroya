use crate::preference_panels::sections::SettingsHighlightState;
use eframe::egui;
use kuroya_core::EditorSettings;

mod builtins;
mod capture;
mod editing;
mod ui;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VimBindingOwner {
    BuiltIn(&'static str),
    CustomDisabled(usize),
    CustomOverride(usize),
}

pub(super) fn render_vim_settings(
    ui: &mut egui::Ui,
    draft: &mut EditorSettings,
    highlight: &mut SettingsHighlightState<'_>,
) {
    self::ui::render_vim_settings(ui, draft, highlight);
}

pub(super) fn vim_key_capture_active(ctx: &egui::Context) -> bool {
    capture::vim_key_capture_active(ctx)
}

pub(super) fn vim_key_capture_clear(ctx: &egui::Context) {
    capture::vim_key_capture_clear(ctx);
}

#[cfg(test)]
mod tests;
