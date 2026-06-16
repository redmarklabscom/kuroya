use crate::preference_panels::sections::{
    SETTINGS_TARGET_TERMINAL_BUFFER, SettingsHighlightState, guarded_f32_drag_value,
    settings_target_heading,
};
use eframe::egui;
use kuroya_core::{
    DEFAULT_TERMINAL_FAST_SCROLL_SENSITIVITY, DEFAULT_TERMINAL_FONT_SIZE,
    DEFAULT_TERMINAL_LETTER_SPACING, DEFAULT_TERMINAL_LINE_HEIGHT,
    DEFAULT_TERMINAL_MOUSE_WHEEL_SCROLL_SENSITIVITY, EditorSettings, MAX_TERMINAL_FONT_SIZE,
    MAX_TERMINAL_LETTER_SPACING, MAX_TERMINAL_LINE_HEIGHT, MAX_TERMINAL_MIN_COLUMNS,
    MAX_TERMINAL_MIN_ROWS, MAX_TERMINAL_SCROLL_SENSITIVITY, MAX_TERMINAL_SCROLLBACK_ROWS,
    MIN_TERMINAL_FONT_SIZE, MIN_TERMINAL_LETTER_SPACING, MIN_TERMINAL_LINE_HEIGHT,
    MIN_TERMINAL_MIN_COLUMNS, MIN_TERMINAL_MIN_ROWS, MIN_TERMINAL_SCROLL_SENSITIVITY,
    MIN_TERMINAL_SCROLLBACK_ROWS,
};
use std::ops::RangeInclusive;

#[cfg(test)]
pub(super) fn render_buffer_text_settings(ui: &mut egui::Ui, draft: &mut EditorSettings) {
    let mut highlight = SettingsHighlightState::disabled();
    render_buffer_text_settings_with_highlight(ui, draft, &mut highlight);
}

pub(super) fn render_buffer_text_settings_with_highlight(
    ui: &mut egui::Ui,
    draft: &mut EditorSettings,
    highlight: &mut SettingsHighlightState<'_>,
) {
    settings_target_heading(
        ui,
        highlight,
        SETTINGS_TARGET_TERMINAL_BUFFER,
        "Buffer and Text",
    );
    egui::Grid::new("settings_terminal_buffer_grid")
        .num_columns(2)
        .spacing([18.0, 10.0])
        .show(ui, |ui| {
            drag_usize_row(
                ui,
                "Scrollback rows",
                &mut draft.terminal_scrollback_rows,
                100.0,
                MIN_TERMINAL_SCROLLBACK_ROWS..=MAX_TERMINAL_SCROLLBACK_ROWS,
                None,
            );

            drag_f32_row(
                ui,
                "Wheel sensitivity",
                &mut draft.terminal_mouse_wheel_scroll_sensitivity,
                0.1,
                MIN_TERMINAL_SCROLL_SENSITIVITY..=MAX_TERMINAL_SCROLL_SENSITIVITY,
                DEFAULT_TERMINAL_MOUSE_WHEEL_SCROLL_SENSITIVITY,
                Some("Multiplier for terminal mouse wheel scrolling."),
            );

            drag_f32_row(
                ui,
                "Alt fast scroll",
                &mut draft.terminal_fast_scroll_sensitivity,
                0.25,
                MIN_TERMINAL_SCROLL_SENSITIVITY..=MAX_TERMINAL_SCROLL_SENSITIVITY,
                DEFAULT_TERMINAL_FAST_SCROLL_SENSITIVITY,
                Some("Extra multiplier for mouse wheel scrolling while Alt is pressed."),
            );

            checkbox_row(
                ui,
                "Wheel zoom",
                &mut draft.terminal_mouse_wheel_zoom,
                "Ctrl/Cmd + wheel changes terminal font size",
                Some("When enabled, Ctrl on Windows/Linux or Cmd on macOS zooms terminal text instead of scrolling."),
            );

            drag_u16_row(
                ui,
                "Minimum rows",
                &mut draft.terminal_min_rows,
                1.0,
                MIN_TERMINAL_MIN_ROWS..=MAX_TERMINAL_MIN_ROWS,
                Some("Smallest PTY row count when the terminal panel or a split is narrow."),
            );

            drag_u16_row(
                ui,
                "Minimum columns",
                &mut draft.terminal_min_columns,
                1.0,
                MIN_TERMINAL_MIN_COLUMNS..=MAX_TERMINAL_MIN_COLUMNS,
                Some("Smallest PTY column count when the terminal panel or a split is narrow."),
            );

            drag_f32_row(
                ui,
                "Font size",
                &mut draft.terminal_font_size,
                0.25,
                MIN_TERMINAL_FONT_SIZE..=MAX_TERMINAL_FONT_SIZE,
                DEFAULT_TERMINAL_FONT_SIZE,
                None,
            );

            drag_f32_row(
                ui,
                "Line height",
                &mut draft.terminal_line_height,
                0.05,
                MIN_TERMINAL_LINE_HEIGHT..=MAX_TERMINAL_LINE_HEIGHT,
                DEFAULT_TERMINAL_LINE_HEIGHT,
                None,
            );

            drag_f32_row(
                ui,
                "Letter spacing",
                &mut draft.terminal_letter_spacing,
                0.25,
                MIN_TERMINAL_LETTER_SPACING..=MAX_TERMINAL_LETTER_SPACING,
                DEFAULT_TERMINAL_LETTER_SPACING,
                None,
            );
        });
}

fn drag_usize_row(
    ui: &mut egui::Ui,
    label: &'static str,
    value: &mut usize,
    speed: f64,
    range: RangeInclusive<usize>,
    tooltip: Option<&'static str>,
) {
    ui.label(label);
    finish_setting_row(
        ui,
        egui::DragValue::new(value).speed(speed).range(range),
        tooltip,
    );
}

fn drag_u16_row(
    ui: &mut egui::Ui,
    label: &'static str,
    value: &mut u16,
    speed: f64,
    range: RangeInclusive<u16>,
    tooltip: Option<&'static str>,
) {
    ui.label(label);
    finish_setting_row(
        ui,
        egui::DragValue::new(value).speed(speed).range(range),
        tooltip,
    );
}

fn drag_f32_row(
    ui: &mut egui::Ui,
    label: &'static str,
    value: &mut f32,
    speed: f64,
    range: RangeInclusive<f32>,
    fallback: f32,
    tooltip: Option<&'static str>,
) {
    ui.label(label);
    let response = guarded_f32_drag_value(ui, value, speed, range, fallback);
    if let Some(tooltip) = tooltip {
        response.on_hover_text(tooltip);
    }
    ui.end_row();
}

fn checkbox_row(
    ui: &mut egui::Ui,
    label: &'static str,
    value: &mut bool,
    text: &'static str,
    tooltip: Option<&'static str>,
) {
    ui.label(label);
    finish_setting_row(ui, egui::Checkbox::new(value, text), tooltip);
}

fn finish_setting_row(ui: &mut egui::Ui, widget: impl egui::Widget, tooltip: Option<&'static str>) {
    let response = ui.add(widget);
    if let Some(tooltip) = tooltip {
        response.on_hover_text(tooltip);
    }
    ui.end_row();
}

#[cfg(test)]
mod tests {
    use super::render_buffer_text_settings;
    use eframe::egui;
    use kuroya_core::EditorSettings;

    #[test]
    fn terminal_buffer_render_preserves_non_finite_raw_drafts() {
        let ctx = egui::Context::default();
        let mut draft = EditorSettings {
            terminal_mouse_wheel_scroll_sensitivity: f32::NAN,
            terminal_fast_scroll_sensitivity: f32::INFINITY,
            terminal_font_size: f32::NEG_INFINITY,
            terminal_line_height: f32::NAN,
            terminal_letter_spacing: f32::INFINITY,
            ..EditorSettings::default()
        };

        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render_buffer_text_settings(ui, &mut draft);
            });
        });

        assert!(draft.terminal_mouse_wheel_scroll_sensitivity.is_nan());
        assert!(draft.terminal_fast_scroll_sensitivity.is_infinite());
        assert!(draft.terminal_font_size.is_infinite());
        assert!(draft.terminal_line_height.is_nan());
        assert!(draft.terminal_letter_spacing.is_infinite());
    }
}
