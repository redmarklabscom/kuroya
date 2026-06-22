use crate::keybindings_panel_actions::PendingKeybindingsPanelActions;
use crate::ui_state::selected_row_scroll_offset;
use eframe::egui::{self, FontFamily, FontId, ScrollArea, Sense, Ui, pos2, vec2};

use super::KeybindingPanelItem;

mod prepared;

use prepared::{
    cached_prepared_keybinding_rows, keybinding_empty_state_label, keybinding_row_widget_info,
    row_command_matches_item,
};

#[cfg(test)]
use prepared::{
    KEYBINDING_ROW_CAPTURE_HINT, KEYBINDING_ROW_CHORD_LIMIT, KEYBINDING_ROW_LABEL_LIMIT,
    PreparedKeybindingRow, PreparedKeybindingRowsCache, keybinding_empty_state_query_cow,
    keybinding_row_display_chord, keybinding_row_display_chord_cow, keybinding_row_display_label,
    keybinding_row_display_label_cow, prepare_keybinding_row, write_keybinding_row_tooltip,
};

pub(super) const KEYBINDING_ROW_HEIGHT: f32 = 34.0;

pub(super) fn render_keybinding_rows(
    ui: &mut Ui,
    items: &[KeybindingPanelItem],
    query: &str,
    selected_index: &mut usize,
    capturing: bool,
    ui_font_size: f32,
    scroll_to_selection: bool,
    actions: &mut PendingKeybindingsPanelActions,
) {
    if items.is_empty() {
        ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(24.0);
            ui.centered_and_justified(|ui| {
                ui.label(keybinding_empty_state_label(query));
            });
        });
        return;
    }

    let viewport_height = ui.available_height();
    let mut scroll_area = ScrollArea::vertical();
    if scroll_to_selection {
        scroll_area = scroll_area.vertical_scroll_offset(selected_row_scroll_offset(
            *selected_index,
            items.len(),
            KEYBINDING_ROW_HEIGHT,
            viewport_height,
        ));
    }
    let label_font = FontId::new(ui_font_size, FontFamily::Proportional);
    let chord_font = FontId::new(ui_font_size, FontFamily::Monospace);
    let prepared_rows = cached_prepared_keybinding_rows(ui, items);
    scroll_area.show_rows(ui, KEYBINDING_ROW_HEIGHT, items.len(), |ui, rows| {
        for idx in rows {
            let Some(row) = prepared_rows.get(idx) else {
                continue;
            };
            let selected = idx == *selected_index;
            let (rect, response) = ui.allocate_exact_size(
                vec2(ui.available_width(), KEYBINDING_ROW_HEIGHT),
                Sense::click(),
            );
            if response.clicked() {
                *selected_index = idx;
            }
            if !capturing
                && response.double_clicked()
                && row_command_matches_item(items, idx, &row.command)
            {
                actions.start_capture = Some(row.command.clone());
            }

            let hovered = response.hovered();
            let painter = ui.painter();
            let visuals = ui.visuals();
            if selected {
                painter.rect_filled(rect, 4.0, visuals.selection.bg_fill);
            } else if hovered {
                painter.rect_filled(rect, 4.0, visuals.widgets.hovered.bg_fill);
            }
            let enabled = ui.is_enabled();
            response.widget_info(|| keybinding_row_widget_info(row, enabled, selected));
            painter.text(
                pos2(rect.left() + 10.0, rect.top() + 8.0),
                egui::Align2::LEFT_TOP,
                row.label.as_str(),
                label_font.clone(),
                visuals.text_color(),
            );
            let chord_color = visuals.weak_text_color();
            painter.text(
                pos2(rect.right() - 150.0, rect.top() + 8.0),
                egui::Align2::LEFT_TOP,
                row.shortcut.as_str(),
                chord_font.clone(),
                chord_color,
            );
            if hovered {
                response.on_hover_ui(|ui| row.tooltip_ui(ui));
            }
        }
    });
}

#[cfg(test)]
mod tests;
