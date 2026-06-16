mod rows;

use crate::{
    KuroyaApp,
    path_display::display_path_label_cow,
    ui_icons::{IconKind, icon_button, icon_label, icon_text_button},
};
use eframe::egui::{self, Align, RichText};
use kuroya_core::Command;
use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

pub(super) struct SymbolPanelJump {
    path: PathBuf,
    line: usize,
    column: usize,
}

impl KuroyaApp {
    pub(crate) fn render_symbols_panel(&mut self, ui: &mut egui::Ui) {
        let active_path = self
            .active_buffer()
            .and_then(|buffer| buffer.path().cloned());
        let symbols_match_active = active_path
            .as_ref()
            .zip(self.document_symbols_path.as_ref())
            .is_some_and(|(active, symbols)| active == symbols);
        let mut refresh = false;
        let mut close = false;
        let mut jump = None;

        ui.horizontal(|ui| {
            icon_label(
                ui,
                IconKind::Code,
                ui.visuals().widgets.inactive.fg_stroke.color,
                "File outline",
            );
            ui.label(RichText::new("Outline").strong());
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if icon_button(ui, IconKind::Close, "Close outline").clicked() {
                    close = true;
                }
                if icon_button(ui, IconKind::Refresh, "Refresh symbols").clicked() {
                    refresh = true;
                }
            });
        });

        if let Some(path) = &active_path {
            let path = symbols_panel_path_label(path);
            ui.label(RichText::new(path.as_ref()).small());
        } else {
            ui.label(RichText::new("No active file").small());
        }
        ui.separator();

        if active_path.is_none() {
            ui.add_space(24.0);
            ui.centered_and_justified(|ui| {
                ui.label("Open a file to show symbols");
            });
        } else if !symbols_match_active {
            ui.add_space(24.0);
            ui.centered_and_justified(|ui| {
                if icon_text_button(ui, IconKind::Refresh, "Load symbols", None, 148.0).clicked() {
                    refresh = true;
                }
            });
        } else {
            jump = self.render_document_symbol_rows(ui);
        }

        if close {
            self.symbols_panel = false;
        } else if refresh {
            self.request_lsp_document_symbols();
        }

        if let Some(SymbolPanelJump { path, line, column }) = jump {
            self.command_bus
                .push(Command::OpenFileAt { path, line, column });
        }
    }
}

fn symbols_panel_path_label(path: &Path) -> Cow<'_, str> {
    display_path_label_cow(path)
}

#[cfg(test)]
mod tests {
    use super::symbols_panel_path_label;
    use crate::path_display::DISPLAY_PATH_LABEL_MAX_CHARS;
    use std::path::PathBuf;

    #[test]
    fn symbols_panel_path_label_sanitizes_and_bounds_display_path() {
        let path = PathBuf::from("workspace").join(format!(
            "main\n{}\u{202e}.rs",
            "x".repeat(DISPLAY_PATH_LABEL_MAX_CHARS * 2)
        ));

        let label = symbols_panel_path_label(&path);

        assert!(!label.contains('\n'));
        assert!(!label.contains('\u{202e}'));
        assert!(label.contains("..."));
        assert!(label.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);
    }
}
