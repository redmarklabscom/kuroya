use crate::keybindings_panel_actions::PendingKeybindingsPanelActions;
use crate::path_display::sanitized_display_label_cow;
use crate::ui_state::selected_row_scroll_offset;
use eframe::egui::{
    self, Color32, FontFamily, FontId, Id, ScrollArea, Sense, Ui, WidgetInfo, WidgetType, pos2,
    vec2,
};
use kuroya_core::Command;
use std::{borrow::Cow, fmt::Write, sync::Arc};

use super::KeybindingPanelItem;

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
            if selected {
                painter.rect_filled(rect, 4.0, Color32::from_rgb(31, 35, 42));
            } else if hovered {
                painter.rect_filled(rect, 4.0, Color32::from_rgb(25, 29, 36));
            }
            let enabled = ui.is_enabled();
            response.widget_info(|| keybinding_row_widget_info(row, enabled, selected));
            painter.text(
                pos2(rect.left() + 10.0, rect.top() + 8.0),
                egui::Align2::LEFT_TOP,
                row.label.as_str(),
                label_font.clone(),
                Color32::from_rgb(222, 226, 233),
            );
            let chord_color = if row.shortcut_assigned {
                Color32::from_rgb(126, 136, 150)
            } else {
                Color32::from_rgb(88, 96, 110)
            };
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct PreparedKeybindingRow {
    command: Command,
    label: String,
    raw_label: Option<String>,
    shortcut: String,
    raw_chord: Option<String>,
    shortcut_assigned: bool,
    accessibility_label: String,
}

impl PreparedKeybindingRow {
    fn label(&self) -> &str {
        self.label.as_str()
    }

    fn shortcut(&self) -> &str {
        self.shortcut.as_str()
    }

    fn accessibility_label(&self) -> &str {
        self.accessibility_label.as_str()
    }

    fn tooltip_ui(&self, ui: &mut Ui) {
        ui.set_max_width(ui.spacing().tooltip_width);
        ui.label(keybinding_row_tooltip(self.label(), self.shortcut()));
    }

    #[cfg(test)]
    fn tooltip(&self) -> String {
        keybinding_row_tooltip(self.label(), self.shortcut())
    }

    fn matches_item(&self, item: &KeybindingPanelItem) -> bool {
        self.command == item.command
            && self.source_label() == item.label
            && self.source_chord_matches(item.chord.as_str())
    }

    fn source_label(&self) -> &str {
        self.raw_label.as_deref().unwrap_or(self.label.as_str())
    }

    fn source_chord_matches(&self, chord: &str) -> bool {
        if !self.shortcut_assigned {
            return chord.is_empty();
        }
        self.raw_chord.as_deref().unwrap_or(self.shortcut.as_str()) == chord
    }
}

fn keybinding_row_widget_info(
    row: &PreparedKeybindingRow,
    enabled: bool,
    selected: bool,
) -> WidgetInfo {
    WidgetInfo::selected(
        WidgetType::SelectableLabel,
        enabled,
        selected,
        row.accessibility_label(),
    )
}

#[derive(Clone, Default)]
struct PreparedKeybindingRowsCache {
    valid: bool,
    rows: Arc<Vec<PreparedKeybindingRow>>,
}

impl PreparedKeybindingRowsCache {
    fn rows_for(&mut self, items: &[KeybindingPanelItem]) -> Arc<Vec<PreparedKeybindingRow>> {
        if !self.matches(items) {
            self.valid = true;
            self.rows = Arc::new(prepare_keybinding_rows(items));
        }
        Arc::clone(&self.rows)
    }

    fn matches(&self, items: &[KeybindingPanelItem]) -> bool {
        self.valid
            && self.rows.len() == items.len()
            && self
                .rows
                .iter()
                .zip(items)
                .all(|(row, item)| row.matches_item(item))
    }
}

fn cached_prepared_keybinding_rows(
    ui: &Ui,
    items: &[KeybindingPanelItem],
) -> Arc<Vec<PreparedKeybindingRow>> {
    ui.ctx().data_mut(|data| {
        data.get_temp_mut_or_default::<PreparedKeybindingRowsCache>(Id::new(
            KEYBINDING_ROWS_CACHE_ID,
        ))
        .rows_for(items)
    })
}

fn prepare_keybinding_rows(items: &[KeybindingPanelItem]) -> Vec<PreparedKeybindingRow> {
    let row_count = items.len();
    items
        .iter()
        .enumerate()
        .map(|(idx, item)| prepare_keybinding_row(item, idx, row_count))
        .collect()
}

fn row_command_matches_item(
    items: &[KeybindingPanelItem],
    row_index: usize,
    command: &Command,
) -> bool {
    items
        .get(row_index)
        .is_some_and(|item| &item.command == command)
}

fn prepare_keybinding_row(
    item: &KeybindingPanelItem,
    row_index: usize,
    row_count: usize,
) -> PreparedKeybindingRow {
    let label = PreparedKeybindingRowText::label(item.label.as_str());
    let shortcut = PreparedKeybindingRowText::chord(item.chord.as_str());
    let shortcut_assigned = !item.chord.is_empty();
    let accessibility_label = keybinding_row_accessibility_label(
        label.display.as_str(),
        shortcut.display.as_str(),
        shortcut_assigned,
        row_index,
        row_count,
    );

    PreparedKeybindingRow {
        command: item.command.clone(),
        label: label.display,
        raw_label: label.raw,
        shortcut: shortcut.display,
        raw_chord: shortcut.raw,
        shortcut_assigned,
        accessibility_label,
    }
}

struct PreparedKeybindingRowText {
    display: String,
    raw: Option<String>,
}

impl PreparedKeybindingRowText {
    fn label(label: &str) -> Self {
        Self::from_cow(label, keybinding_row_display_label_cow(label))
    }

    fn chord(chord: &str) -> Self {
        if chord.is_empty() {
            return Self {
                display: KEYBINDING_ROW_UNASSIGNED_LABEL.to_owned(),
                raw: None,
            };
        }
        Self::from_cow(chord, keybinding_row_display_chord_cow(chord))
    }

    fn from_cow(raw: &str, display: Cow<'_, str>) -> Self {
        let raw = (display.as_ref() != raw).then(|| raw.to_owned());
        Self {
            display: display.into_owned(),
            raw,
        }
    }
}

fn keybinding_row_tooltip(label: &str, shortcut: &str) -> String {
    let mut tooltip = String::with_capacity(label.len() + "\nShortcut: ".len() + shortcut.len());
    write_keybinding_row_tooltip(&mut tooltip, label, shortcut);
    tooltip
}

fn write_keybinding_row_tooltip(text: &mut String, label: &str, shortcut: &str) {
    text.push_str(label);
    text.push_str("\nShortcut: ");
    text.push_str(shortcut);
}

fn keybinding_row_accessibility_label(
    label: &str,
    shortcut: &str,
    shortcut_assigned: bool,
    row_index: usize,
    row_count: usize,
) -> String {
    let mut status = String::with_capacity(label.len() + shortcut.len() + 48);
    let _ = write!(
        status,
        "Command {}, position {} of {}",
        label,
        row_index + 1,
        row_count
    );
    if shortcut_assigned {
        status.push_str(", shortcut ");
        status.push_str(shortcut);
    } else {
        status.push_str(", unassigned");
    }
    status
}

#[cfg(test)]
fn keybinding_row_display_label(label: &str) -> String {
    keybinding_row_display_label_cow(label).into_owned()
}

fn keybinding_row_display_label_cow(label: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(
        label,
        KEYBINDING_ROW_LABEL_LIMIT,
        KEYBINDING_ROW_LABEL_FALLBACK,
    )
}

#[cfg(test)]
fn keybinding_row_display_chord(chord: &str) -> String {
    keybinding_row_display_chord_cow(chord).into_owned()
}

fn keybinding_row_display_chord_cow(chord: &str) -> Cow<'_, str> {
    if chord.is_empty() {
        return Cow::Borrowed(KEYBINDING_ROW_UNASSIGNED_LABEL);
    }
    sanitized_display_label_cow(
        chord,
        KEYBINDING_ROW_CHORD_LIMIT,
        KEYBINDING_ROW_CHORD_FALLBACK,
    )
}

fn keybinding_empty_state_label(query: &str) -> String {
    let query = keybinding_empty_state_query_cow(query);
    if query.is_empty() {
        "No keybinding commands available".to_owned()
    } else {
        let query = query.as_ref();
        let mut label = String::with_capacity("No shortcuts match \"\"".len() + query.len());
        label.push_str("No shortcuts match \"");
        label.push_str(query);
        label.push('"');
        label
    }
}

fn keybinding_empty_state_query_cow(query: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(query, KEYBINDING_EMPTY_QUERY_LIMIT, "")
}

const KEYBINDING_ROW_LABEL_LIMIT: usize = super::KEYBINDING_TEXT_MAX_CHARS;
const KEYBINDING_ROW_CHORD_LIMIT: usize = super::KEYBINDING_TEXT_MAX_CHARS;
const KEYBINDING_ROW_LABEL_FALLBACK: &str = "Unnamed command";
const KEYBINDING_ROW_CHORD_FALLBACK: &str = "Invalid shortcut";
const KEYBINDING_ROW_UNASSIGNED_LABEL: &str = "Unassigned";
const KEYBINDING_EMPTY_QUERY_LIMIT: usize = 48;
const KEYBINDING_ROWS_CACHE_ID: &str = "kuroya.keybindings_panel.rows_cache";

#[cfg(test)]
mod tests {
    use super::super::KeybindingPanelItem;
    use super::{
        KEYBINDING_ROW_CHORD_LIMIT, KEYBINDING_ROW_LABEL_LIMIT, PreparedKeybindingRowsCache,
        keybinding_empty_state_label, keybinding_empty_state_query_cow,
        keybinding_row_display_chord, keybinding_row_display_chord_cow,
        keybinding_row_display_label, keybinding_row_display_label_cow, keybinding_row_widget_info,
        prepare_keybinding_row, row_command_matches_item, write_keybinding_row_tooltip,
    };
    use eframe::egui::WidgetType;
    use kuroya_core::Command;
    use std::borrow::Cow;
    use std::sync::Arc;

    fn row_tooltip(label: &str, chord: &str) -> String {
        prepared_row(label, chord).tooltip()
    }

    fn row_accessibility_label(
        label: &str,
        chord: &str,
        row_index: usize,
        row_count: usize,
    ) -> String {
        prepare_keybinding_row(
            &keybinding_item(label, chord, Command::ToggleQuickOpen),
            row_index,
            row_count,
        )
        .accessibility_label()
        .to_owned()
    }

    fn prepared_row(label: &str, chord: &str) -> super::PreparedKeybindingRow {
        prepare_keybinding_row(
            &keybinding_item(label, chord, Command::ToggleQuickOpen),
            0,
            1,
        )
    }

    fn keybinding_item(label: &str, chord: &str, command: Command) -> KeybindingPanelItem {
        KeybindingPanelItem {
            chord: chord.to_owned(),
            command,
            label: label.to_owned(),
            search_text: format!("{label} {chord}"),
        }
    }

    #[test]
    fn keybinding_empty_state_names_failed_filter() {
        assert_eq!(
            keybinding_empty_state_label(" ctrl alt made-up "),
            "No shortcuts match \"ctrl alt made-up\""
        );
        assert_eq!(
            keybinding_empty_state_label("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"),
            "No shortcuts match \"abcdefghijklmnopqrstuv...DEFGHIJKLMNOPQRSTUVWXYZ\""
        );
    }

    #[test]
    fn keybinding_empty_state_sanitizes_failed_filter_text() {
        let label = keybinding_empty_state_label(" ctrl\nbad\u{202e}\tquery ");

        assert_eq!(label, "No shortcuts match \"ctrl bad query\"");
        assert!(!label.chars().any(char::is_control));
        assert!(!label.contains('\u{202e}'));
    }

    #[test]
    fn keybinding_empty_state_handles_empty_catalog() {
        assert_eq!(
            keybinding_empty_state_label(""),
            "No keybinding commands available"
        );
    }

    #[test]
    fn keybinding_empty_state_query_cow_borrows_clean_ascii_and_unicode() {
        assert!(matches!(
            keybinding_empty_state_query_cow("ctrl alt made-up"),
            Cow::Borrowed("ctrl alt made-up")
        ));

        let unicode = "aller \u{00e0} d\u{00e9}finition";
        match keybinding_empty_state_query_cow(unicode) {
            Cow::Borrowed(query) => assert_eq!(query, unicode),
            Cow::Owned(query) => panic!("expected borrowed query, got {query:?}"),
        }
    }

    #[test]
    fn keybinding_empty_state_query_cow_owns_dirty_truncated_and_fallback_values() {
        let dirty = keybinding_empty_state_query_cow(" ctrl\nbad\u{202e}\tquery ");
        assert_eq!(dirty.as_ref(), "ctrl bad query");
        assert!(matches!(dirty, Cow::Owned(_)));

        let overlong = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
        let truncated = keybinding_empty_state_query_cow(overlong);
        assert_eq!(
            truncated.as_ref(),
            "abcdefghijklmnopqrstuv...DEFGHIJKLMNOPQRSTUVWXYZ"
        );
        assert!(matches!(truncated, Cow::Owned(_)));

        let fallback = keybinding_empty_state_query_cow("\n\t\u{202e}");
        assert_eq!(fallback.as_ref(), "");
        assert!(matches!(fallback, Cow::Owned(_)));
    }

    #[test]
    fn keybinding_row_display_label_cow_borrows_clean_ascii_and_unicode() {
        assert!(matches!(
            keybinding_row_display_label_cow("Quick Open"),
            Cow::Borrowed("Quick Open")
        ));

        let unicode = "Aller \u{00e0} D\u{00e9}finition";
        match keybinding_row_display_label_cow(unicode) {
            Cow::Borrowed(label) => assert_eq!(label, unicode),
            Cow::Owned(label) => panic!("expected borrowed label, got {label:?}"),
        }
    }

    #[test]
    fn keybinding_row_display_label_cow_owns_dirty_truncated_and_fallback_values() {
        let dirty = keybinding_row_display_label_cow("Open\nWorkspace\u{202e}");
        assert_eq!(dirty.as_ref(), "Open Workspace");
        assert!(matches!(dirty, Cow::Owned(_)));

        let overlong = "OpenWorkspace".repeat(KEYBINDING_ROW_LABEL_LIMIT);
        let truncated = keybinding_row_display_label_cow(&overlong);
        assert!(truncated.contains("..."));
        assert!(truncated.chars().count() <= KEYBINDING_ROW_LABEL_LIMIT);
        assert!(matches!(truncated, Cow::Owned(_)));

        let fallback = keybinding_row_display_label_cow("\n\t\u{202e}");
        assert_eq!(fallback.as_ref(), "Unnamed command");
        assert!(matches!(fallback, Cow::Owned(_)));
    }

    #[test]
    fn keybinding_row_display_chord_cow_borrows_clean_ascii_unicode_and_unassigned() {
        assert!(matches!(
            keybinding_row_display_chord_cow("Ctrl+P"),
            Cow::Borrowed("Ctrl+P")
        ));
        assert!(matches!(
            keybinding_row_display_chord_cow(""),
            Cow::Borrowed("Unassigned")
        ));

        let unicode = "\u{2318}+\u{03a9}";
        match keybinding_row_display_chord_cow(unicode) {
            Cow::Borrowed(chord) => assert_eq!(chord, unicode),
            Cow::Owned(chord) => panic!("expected borrowed chord, got {chord:?}"),
        }
    }

    #[test]
    fn keybinding_row_display_chord_cow_owns_dirty_truncated_and_fallback_values() {
        let dirty = keybinding_row_display_chord_cow("Ctrl+\nP\u{202e}");
        assert_eq!(dirty.as_ref(), "Ctrl+ P");
        assert!(matches!(dirty, Cow::Owned(_)));

        let overlong = "Ctrl+Shift+".repeat(KEYBINDING_ROW_CHORD_LIMIT);
        let truncated = keybinding_row_display_chord_cow(&overlong);
        assert!(truncated.contains("..."));
        assert!(truncated.chars().count() <= KEYBINDING_ROW_CHORD_LIMIT);
        assert!(matches!(truncated, Cow::Owned(_)));

        let fallback = keybinding_row_display_chord_cow("\n\t\u{202e}");
        assert_eq!(fallback.as_ref(), "Invalid shortcut");
        assert!(matches!(fallback, Cow::Owned(_)));
    }

    #[test]
    fn keybinding_row_display_string_wrappers_match_cow_helpers() {
        for label in [
            "Quick Open",
            "Aller \u{00e0} D\u{00e9}finition",
            "Open\nWorkspace\u{202e}",
            "\n\t\u{202e}",
        ] {
            assert_eq!(
                keybinding_row_display_label(label),
                keybinding_row_display_label_cow(label).as_ref()
            );
        }

        for chord in ["Ctrl+P", "\u{2318}+\u{03a9}", "Ctrl+\nP\u{202e}", ""] {
            assert_eq!(
                keybinding_row_display_chord(chord),
                keybinding_row_display_chord_cow(chord).as_ref()
            );
        }
    }

    #[test]
    fn keybinding_row_tooltip_writer_appends_exact_display_text() {
        let row = prepared_row("Quick Open", "");
        let mut tooltip = String::from("prefix:");

        write_keybinding_row_tooltip(&mut tooltip, row.label(), row.shortcut());

        assert_eq!(tooltip, "prefix:Quick Open\nShortcut: Unassigned");
        assert_eq!(row.tooltip(), "Quick Open\nShortcut: Unassigned");
    }

    #[test]
    fn keybinding_row_tooltip_names_shortcut_or_unassigned_state() {
        assert_eq!(
            row_tooltip("Quick Open", "Ctrl+P"),
            "Quick Open\nShortcut: Ctrl+P"
        );
        assert_eq!(
            row_tooltip("Quick Open", ""),
            "Quick Open\nShortcut: Unassigned"
        );
    }

    #[test]
    fn keybinding_row_tooltip_sanitizes_and_bounds_display_text() {
        let tooltip = row_tooltip(
            &format!(
                "Open\n\u{202e}{}",
                "Workspace".repeat(KEYBINDING_ROW_LABEL_LIMIT)
            ),
            &format!(
                "Ctrl+\t{}\nP\u{202e}",
                "Shift+".repeat(KEYBINDING_ROW_CHORD_LIMIT)
            ),
        );

        assert!(!tooltip.chars().any(|ch| ch != '\n' && ch.is_control()));
        assert!(!tooltip.contains('\u{202e}'));
        assert!(tooltip.contains("..."));
        assert!(
            tooltip.chars().count()
                <= KEYBINDING_ROW_LABEL_LIMIT + "\nShortcut: ".len() + KEYBINDING_ROW_CHORD_LIMIT
        );
    }

    #[test]
    fn keybinding_row_tooltip_falls_back_for_blank_display_text() {
        assert_eq!(
            row_tooltip("\n\t\u{202e}", "\n\t\u{202e}"),
            "Unnamed command\nShortcut: Invalid shortcut"
        );
    }

    #[test]
    fn prepared_keybinding_row_helpers_borrow_cached_display_metadata() {
        let row = prepared_row("Quick Open", "Ctrl+P");

        assert_eq!(row.label(), "Quick Open");
        assert_eq!(row.shortcut(), "Ctrl+P");
        assert_eq!(
            row.accessibility_label(),
            "Command Quick Open, position 1 of 1, shortcut Ctrl+P"
        );
        assert_eq!(row.tooltip(), "Quick Open\nShortcut: Ctrl+P");
        assert_eq!(row.label().as_ptr(), row.label.as_ptr());
        assert_eq!(row.shortcut().as_ptr(), row.shortcut.as_ptr());
        assert_eq!(
            row.accessibility_label().as_ptr(),
            row.accessibility_label.as_ptr()
        );
    }

    #[test]
    fn keybinding_row_widget_info_preserves_accessibility_metadata() {
        let row = prepare_keybinding_row(
            &keybinding_item("Quick Open", "Ctrl+P", Command::ToggleQuickOpen),
            1,
            4,
        );

        let info = keybinding_row_widget_info(&row, false, true);

        assert_eq!(info.typ, WidgetType::SelectableLabel);
        assert!(!info.enabled);
        assert_eq!(info.selected, Some(true));
        assert_eq!(
            info.label.as_deref(),
            Some("Command Quick Open, position 2 of 4, shortcut Ctrl+P")
        );
    }

    #[test]
    fn keybinding_row_accessibility_label_names_shortcut_or_unassigned_state() {
        assert_eq!(
            row_accessibility_label("Quick Open", "Ctrl+P", 2, 8),
            "Command Quick Open, position 3 of 8, shortcut Ctrl+P"
        );
        assert_eq!(
            row_accessibility_label("Quick Open", "", 0, 8),
            "Command Quick Open, position 1 of 8, unassigned"
        );
    }

    #[test]
    fn keybinding_row_accessibility_label_sanitizes_and_bounds_display_text() {
        let label = row_accessibility_label(
            &format!(
                "Open\n\u{202e}{}",
                "Workspace".repeat(KEYBINDING_ROW_LABEL_LIMIT)
            ),
            &format!(
                "Ctrl+\t{}\nP\u{202e}",
                "Shift+".repeat(KEYBINDING_ROW_CHORD_LIMIT)
            ),
            4,
            16,
        );

        assert!(!label.chars().any(char::is_control));
        assert!(!label.contains('\u{202e}'));
        assert!(label.contains("..."));
        assert!(
            label.chars().count()
                <= "Command ".len()
                    + KEYBINDING_ROW_LABEL_LIMIT
                    + ", position 5 of 16, shortcut ".len()
                    + KEYBINDING_ROW_CHORD_LIMIT
        );
    }

    #[test]
    fn keybinding_row_accessibility_label_falls_back_for_blank_display_text() {
        assert_eq!(
            row_accessibility_label("\n\t\u{202e}", "\n\t\u{202e}", 0, 1),
            "Command Unnamed command, position 1 of 1, shortcut Invalid shortcut"
        );
    }

    #[test]
    fn prepared_keybinding_rows_preserve_raw_command_for_dispatch() {
        let noisy_label = format!("Run\n\u{202e}{}", "Task".repeat(KEYBINDING_ROW_LABEL_LIMIT));
        let item = keybinding_item(&noisy_label, "Ctrl+Alt+T", Command::ToggleTerminal);

        let row = prepare_keybinding_row(&item, 0, 1);

        assert_eq!(row.command, Command::ToggleTerminal);
        assert_ne!(row.label, noisy_label);
        assert_eq!(item.command, Command::ToggleTerminal);
    }

    #[test]
    fn prepared_keybinding_rows_cache_reuses_display_text_until_items_change() {
        let items = vec![keybinding_item(
            "Quick Open",
            "Ctrl+P",
            Command::ToggleQuickOpen,
        )];
        let mut cache = PreparedKeybindingRowsCache::default();

        let first = cache.rows_for(&items);
        let second = cache.rows_for(&items);

        assert!(Arc::ptr_eq(&first, &second));
        assert!(cache.matches(&items));
        assert_eq!(first[0].label(), "Quick Open");
        assert_eq!(first[0].tooltip(), "Quick Open\nShortcut: Ctrl+P");
        assert_eq!(
            first[0].accessibility_label(),
            "Command Quick Open, position 1 of 1, shortcut Ctrl+P"
        );

        let mut changed_search_text = items.clone();
        changed_search_text[0].search_text.push_str(" palette");
        let changed = cache.rows_for(&changed_search_text);

        assert!(Arc::ptr_eq(&first, &changed));
        assert!(cache.matches(&changed_search_text));

        let mut changed_label = items.clone();
        changed_label[0].label.push_str(" Palette");
        let changed = cache.rows_for(&changed_label);

        assert!(!Arc::ptr_eq(&first, &changed));
        assert!(cache.matches(&changed_label));
        assert_eq!(changed[0].label, "Quick Open Palette");
    }

    #[test]
    fn prepared_keybinding_rows_cache_matches_clean_rows_without_raw_source_clones() {
        let item = keybinding_item("Quick Open", "Ctrl+P", Command::ToggleQuickOpen);
        let row = prepare_keybinding_row(&item, 0, 1);

        assert_eq!(row.label, "Quick Open");
        assert_eq!(row.shortcut, "Ctrl+P");
        assert!(row.raw_label.is_none());
        assert!(row.raw_chord.is_none());
        assert!(row.matches_item(&item));
    }

    #[test]
    fn prepared_keybinding_rows_cache_keeps_dirty_raw_text_for_exact_matching() {
        let items = vec![keybinding_item(
            "Open Workspace",
            "Ctrl+ P",
            Command::ToggleQuickOpen,
        )];
        let mut cache = PreparedKeybindingRowsCache::default();
        let clean = cache.rows_for(&items);

        let dirty_items = vec![keybinding_item(
            "Open\nWorkspace",
            "Ctrl+\nP",
            Command::ToggleQuickOpen,
        )];
        let dirty = cache.rows_for(&dirty_items);

        assert!(!Arc::ptr_eq(&clean, &dirty));
        assert_eq!(clean[0].label, dirty[0].label);
        assert_eq!(clean[0].shortcut, dirty[0].shortcut);
        assert_eq!(dirty[0].raw_label.as_deref(), Some("Open\nWorkspace"));
        assert_eq!(dirty[0].raw_chord.as_deref(), Some("Ctrl+\nP"));
        assert!(cache.matches(&dirty_items));
        assert!(!cache.matches(&items));
    }

    #[test]
    fn row_command_guard_rejects_stale_prepared_row_actions() {
        let items = vec![keybinding_item(
            "Quick Open",
            "Ctrl+P",
            Command::ToggleQuickOpen,
        )];

        assert!(row_command_matches_item(
            &items,
            0,
            &Command::ToggleQuickOpen
        ));
        assert!(!row_command_matches_item(
            &items,
            0,
            &Command::ToggleTerminal
        ));
        assert!(!row_command_matches_item(
            &items,
            1,
            &Command::ToggleQuickOpen
        ));
    }
}
