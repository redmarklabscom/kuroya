use crate::popup_buttons::{PopupButtonKind, popup_compact_button, popup_compact_button_enabled};
use crate::preference_panels::sections::{
    SETTINGS_DISPLAY_TEXT_MAX_CHARS, SETTINGS_TARGET_FONTS, SettingsHighlightState,
    bounded_settings_display_text, settings_target_block,
};
use eframe::egui;

pub(super) fn render_font_settings_with_highlight(
    ui: &mut egui::Ui,
    editor_font_path: &str,
    ui_font_path: &str,
    choose_editor_font: &mut bool,
    clear_editor_font: &mut bool,
    choose_ui_font: &mut bool,
    clear_ui_font: &mut bool,
    highlight: &mut SettingsHighlightState<'_>,
) {
    settings_target_block(ui, highlight, SETTINGS_TARGET_FONTS, |ui| {
        render_font_settings_content(
            ui,
            editor_font_path,
            ui_font_path,
            choose_editor_font,
            clear_editor_font,
            choose_ui_font,
            clear_ui_font,
        );
    });
}

fn render_font_settings_content(
    ui: &mut egui::Ui,
    editor_font_path: &str,
    ui_font_path: &str,
    choose_editor_font: &mut bool,
    clear_editor_font: &mut bool,
    choose_ui_font: &mut bool,
    clear_ui_font: &mut bool,
) {
    egui::Grid::new("settings_fonts_grid")
        .num_columns(2)
        .spacing([18.0, 10.0])
        .show(ui, |ui| {
            ui.label("Editor font file");
            render_font_file_picker(ui, editor_font_path, choose_editor_font, clear_editor_font);
            ui.end_row();

            ui.label("UI font file");
            render_font_file_picker(ui, ui_font_path, choose_ui_font, clear_ui_font);
            ui.end_row();
        });
}

fn render_font_file_picker(
    ui: &mut egui::Ui,
    current: &str,
    choose_file: &mut bool,
    clear_file: &mut bool,
) {
    let selected = current.trim();
    let has_selection = !selected.is_empty();
    let selected_display = bounded_settings_display_text(
        selected,
        SETTINGS_DISPLAY_TEXT_MAX_CHARS,
        "Custom font file",
    );
    let label = if has_selection {
        selected_display.as_str()
    } else {
        "Use bundled font"
    };
    let text = egui::RichText::new(label)
        .monospace()
        .color(if has_selection {
            ui.visuals().text_color()
        } else {
            ui.visuals().weak_text_color()
        });

    ui.horizontal(|ui| {
        let label_width = (ui.available_width() - 168.0).clamp(96.0, 260.0);
        let response = ui.add_sized(
            [label_width, ui.spacing().interact_size.y],
            egui::Label::new(text).truncate(),
        );
        if has_selection {
            response.on_hover_text(selected_display);
        }

        if popup_compact_button(ui, "Choose", PopupButtonKind::Primary).clicked() {
            *choose_file = true;
        }

        if popup_compact_button_enabled(ui, has_selection, "Clear", PopupButtonKind::Secondary)
            .clicked()
        {
            *clear_file = true;
        }
    });
}
