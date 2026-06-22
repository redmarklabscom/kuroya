use crate::path_display::sanitized_display_label_cow;
use eframe::egui::{Id, Ui, WidgetInfo, WidgetType};
use kuroya_core::Command;
use std::{borrow::Cow, fmt::Write, sync::Arc};

use super::super::KeybindingPanelItem;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PreparedKeybindingRow {
    pub(super) command: Command,
    pub(super) label: String,
    pub(super) raw_label: Option<String>,
    pub(super) shortcut: String,
    pub(super) raw_chord: Option<String>,
    shortcut_assigned: bool,
    pub(super) accessibility_label: String,
}

impl PreparedKeybindingRow {
    pub(super) fn label(&self) -> &str {
        self.label.as_str()
    }

    pub(super) fn shortcut(&self) -> &str {
        self.shortcut.as_str()
    }

    pub(super) fn accessibility_label(&self) -> &str {
        self.accessibility_label.as_str()
    }

    pub(super) fn tooltip_ui(&self, ui: &mut Ui) {
        ui.set_max_width(ui.spacing().tooltip_width);
        ui.label(keybinding_row_tooltip(self.label(), self.shortcut()));
    }

    #[cfg(test)]
    pub(super) fn tooltip(&self) -> String {
        keybinding_row_tooltip(self.label(), self.shortcut())
    }

    pub(super) fn matches_item(&self, item: &KeybindingPanelItem) -> bool {
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

pub(super) fn keybinding_row_widget_info(
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
pub(super) struct PreparedKeybindingRowsCache {
    valid: bool,
    rows: Arc<Vec<PreparedKeybindingRow>>,
}

impl PreparedKeybindingRowsCache {
    pub(super) fn rows_for(
        &mut self,
        items: &[KeybindingPanelItem],
    ) -> Arc<Vec<PreparedKeybindingRow>> {
        if !self.matches(items) {
            self.valid = true;
            self.rows = Arc::new(prepare_keybinding_rows(items));
        }
        Arc::clone(&self.rows)
    }

    pub(super) fn matches(&self, items: &[KeybindingPanelItem]) -> bool {
        self.valid
            && self.rows.len() == items.len()
            && self
                .rows
                .iter()
                .zip(items)
                .all(|(row, item)| row.matches_item(item))
    }
}

pub(super) fn cached_prepared_keybinding_rows(
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

pub(super) fn row_command_matches_item(
    items: &[KeybindingPanelItem],
    row_index: usize,
    command: &Command,
) -> bool {
    items
        .get(row_index)
        .is_some_and(|item| &item.command == command)
}

pub(super) fn prepare_keybinding_row(
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
    let mut tooltip = String::with_capacity(
        label.len()
            + "\nShortcut: ".len()
            + shortcut.len()
            + "\n".len()
            + KEYBINDING_ROW_CAPTURE_HINT.len(),
    );
    write_keybinding_row_tooltip(&mut tooltip, label, shortcut);
    tooltip
}

pub(super) fn write_keybinding_row_tooltip(text: &mut String, label: &str, shortcut: &str) {
    text.push_str(label);
    text.push_str("\nShortcut: ");
    text.push_str(shortcut);
    text.push('\n');
    text.push_str(KEYBINDING_ROW_CAPTURE_HINT);
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
pub(super) fn keybinding_row_display_label(label: &str) -> String {
    keybinding_row_display_label_cow(label).into_owned()
}

pub(super) fn keybinding_row_display_label_cow(label: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(
        label,
        KEYBINDING_ROW_LABEL_LIMIT,
        KEYBINDING_ROW_LABEL_FALLBACK,
    )
}

#[cfg(test)]
pub(super) fn keybinding_row_display_chord(chord: &str) -> String {
    keybinding_row_display_chord_cow(chord).into_owned()
}

pub(super) fn keybinding_row_display_chord_cow(chord: &str) -> Cow<'_, str> {
    if chord.is_empty() {
        return Cow::Borrowed(KEYBINDING_ROW_UNASSIGNED_LABEL);
    }
    sanitized_display_label_cow(
        chord,
        KEYBINDING_ROW_CHORD_LIMIT,
        KEYBINDING_ROW_CHORD_FALLBACK,
    )
}

pub(super) fn keybinding_empty_state_label(query: &str) -> String {
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

pub(super) fn keybinding_empty_state_query_cow(query: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(query, KEYBINDING_EMPTY_QUERY_LIMIT, "")
}

pub(super) const KEYBINDING_ROW_LABEL_LIMIT: usize = super::super::KEYBINDING_TEXT_MAX_CHARS;
pub(super) const KEYBINDING_ROW_CHORD_LIMIT: usize = super::super::KEYBINDING_TEXT_MAX_CHARS;
pub(super) const KEYBINDING_ROW_LABEL_FALLBACK: &str = "Unnamed command";
pub(super) const KEYBINDING_ROW_CHORD_FALLBACK: &str = "Invalid shortcut";
pub(super) const KEYBINDING_ROW_UNASSIGNED_LABEL: &str = "Unassigned";
pub(super) const KEYBINDING_ROW_CAPTURE_HINT: &str =
    "Double-click or press Enter to capture a shortcut.";
pub(super) const KEYBINDING_EMPTY_QUERY_LIMIT: usize = 48;
const KEYBINDING_ROWS_CACHE_ID: &str = "kuroya.keybindings_panel.rows_cache";
