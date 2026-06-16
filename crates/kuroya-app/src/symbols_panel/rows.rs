use super::SymbolPanelJump;
use crate::{
    KuroyaApp,
    lsp_labels::symbol_kind_label,
    lsp_text_positions::lsp_one_based_utf16_column_to_char_column,
    path_display::sanitized_display_label_cow,
    ui_state::{
        clamp_selection, handle_list_navigation_keys, plain_key_pressed,
        selected_row_scroll_offset, selection_page_step,
    },
};
use eframe::egui::{Key, ScrollArea, Ui};
use kuroya_core::{LspDocumentSymbol, TextBuffer};
use std::{borrow::Cow, fmt::Write as _};

const DOCUMENT_SYMBOL_ROW_HEIGHT_MIN: f32 = 22.0;
const DOCUMENT_SYMBOL_NAME_MAX_CHARS: usize = 160;
const DOCUMENT_SYMBOL_DETAIL_MAX_CHARS: usize = 240;

impl KuroyaApp {
    pub(super) fn render_document_symbol_rows(&mut self, ui: &mut Ui) -> Option<SymbolPanelJump> {
        if self.document_symbols.is_empty() {
            ui.add_space(24.0);
            ui.centered_and_justified(|ui| {
                ui.label("No symbols");
            });
            return None;
        }

        let mut selected_index = self.document_symbols_selected;
        clamp_selection(&mut selected_index, self.document_symbols.len());
        let mut jump = None;
        let row_height = ui
            .spacing()
            .interact_size
            .y
            .max(DOCUMENT_SYMBOL_ROW_HEIGHT_MIN);
        let viewport_height = ui.available_height();
        let focus_id = ui.make_persistent_id("document-symbol-rows");
        let rows_focused = ui.memory(|memory| memory.has_focus(focus_id));
        let mut scroll_to_selection = false;

        if rows_focused {
            scroll_to_selection = ui.input(|input| {
                handle_list_navigation_keys(
                    input,
                    &mut selected_index,
                    self.document_symbols.len(),
                    selection_page_step(row_height, viewport_height),
                )
            });
            if ui.input(|input| plain_key_pressed(input, Key::Enter))
                && let Some(symbol) = self.document_symbols.get(selected_index)
            {
                jump = Some(self.document_symbol_jump(symbol));
            }
        }

        let mut scroll_area = ScrollArea::vertical();
        if scroll_to_selection {
            scroll_area = scroll_area.vertical_scroll_offset(selected_row_scroll_offset(
                selected_index,
                self.document_symbols.len(),
                row_height,
                viewport_height,
            ));
        }
        scroll_area.show_rows(ui, row_height, self.document_symbols.len(), |ui, rows| {
            let mut tooltip = String::new();
            for idx in rows {
                let Some(symbol) = self.document_symbols.get(idx) else {
                    continue;
                };
                let display = DocumentSymbolRowDisplay::new(symbol);
                let response = ui
                    .selectable_label(idx == selected_index, display.label())
                    .on_hover_ui(|ui| {
                        tooltip.clear();
                        display.write_tooltip(&mut tooltip);
                        ui.label(tooltip.as_str());
                    });
                if response.clicked() {
                    ui.memory_mut(|memory| memory.request_focus(focus_id));
                    selected_index = idx;
                    jump = Some(self.document_symbol_jump(symbol));
                }
            }
        });
        self.document_symbols_selected = selected_index;
        jump
    }

    fn document_symbol_jump(&self, symbol: &LspDocumentSymbol) -> SymbolPanelJump {
        let column = document_symbol_jump_column(self.buffer_by_lexical_path(&symbol.path), symbol);
        SymbolPanelJump {
            path: symbol.path.clone(),
            line: symbol.line,
            column,
        }
    }
}

#[cfg(test)]
pub(crate) fn document_symbol_row_label(symbol: &LspDocumentSymbol) -> String {
    DocumentSymbolRowDisplay::new(symbol).label().to_owned()
}

struct DocumentSymbolRowDisplay<'a> {
    label: String,
    kind: &'static str,
    name: Cow<'a, str>,
    detail: Option<Cow<'a, str>>,
    line: usize,
    column: usize,
}

impl<'a> DocumentSymbolRowDisplay<'a> {
    fn new(symbol: &'a LspDocumentSymbol) -> Self {
        let indent_width = symbol.depth.min(8) * 2;
        let kind = symbol_kind_label(symbol.kind);
        let name = document_symbol_name(&symbol.name);
        let detail = symbol.detail.as_deref().and_then(document_symbol_detail);
        let label = format_document_symbol_row_label(
            indent_width,
            kind,
            name.as_ref(),
            detail.as_deref(),
            symbol.line,
            symbol.column,
        );

        Self {
            label,
            kind,
            name,
            detail,
            line: symbol.line,
            column: symbol.column,
        }
    }

    fn label(&self) -> &str {
        &self.label
    }

    #[cfg(test)]
    fn tooltip(&self) -> String {
        format_document_symbol_row_tooltip(
            self.kind,
            self.name.as_ref(),
            self.detail.as_deref(),
            self.line,
            self.column,
        )
    }

    fn write_tooltip(&self, tooltip: &mut String) {
        write_document_symbol_row_tooltip(
            tooltip,
            self.kind,
            self.name.as_ref(),
            self.detail.as_deref(),
            self.line,
            self.column,
        )
    }
}

#[cfg(test)]
fn document_symbol_row_tooltip(symbol: &LspDocumentSymbol) -> String {
    DocumentSymbolRowDisplay::new(symbol).tooltip()
}

fn format_document_symbol_row_label(
    indent_width: usize,
    kind: &str,
    name: &str,
    detail: Option<&str>,
    line: usize,
    column: usize,
) -> String {
    let detail_len = detail.map_or(0, |detail| 2 + detail.len());
    let mut label = String::with_capacity(indent_width + kind.len() + name.len() + detail_len + 24);

    for _ in 0..indent_width {
        label.push(' ');
    }
    label.push_str(kind);
    label.push(' ');
    label.push_str(name);
    let _ = write!(&mut label, "  {line}:{column}");
    if let Some(detail) = detail {
        label.push_str("  ");
        label.push_str(detail);
    }

    label
}

#[cfg(test)]
fn format_document_symbol_row_tooltip(
    kind: &str,
    name: &str,
    detail: Option<&str>,
    line: usize,
    column: usize,
) -> String {
    let detail_len = detail.map_or(0, |detail| 3 + detail.len());
    let mut tooltip = String::with_capacity(kind.len() + name.len() + detail_len + 32);

    write_document_symbol_row_tooltip(&mut tooltip, kind, name, detail, line, column);

    tooltip
}

fn write_document_symbol_row_tooltip(
    tooltip: &mut String,
    kind: &str,
    name: &str,
    detail: Option<&str>,
    line: usize,
    column: usize,
) {
    let _ = write!(tooltip, "{kind} {name} at line {line}, column {column}");
    if let Some(detail) = detail {
        tooltip.push_str(" - ");
        tooltip.push_str(detail);
    }
}

fn document_symbol_jump_column(buffer: Option<&TextBuffer>, symbol: &LspDocumentSymbol) -> usize {
    buffer
        .and_then(|buffer| {
            lsp_one_based_utf16_column_to_char_column(buffer, symbol.line, symbol.column)
        })
        .map(|column| column + 1)
        .unwrap_or(symbol.column)
}

fn document_symbol_name(name: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(name, DOCUMENT_SYMBOL_NAME_MAX_CHARS, "<unnamed>")
}

fn document_symbol_detail_label(detail: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(detail, DOCUMENT_SYMBOL_DETAIL_MAX_CHARS, "")
}

fn document_symbol_detail(detail: &str) -> Option<Cow<'_, str>> {
    let detail = document_symbol_detail_label(detail);
    (!detail.is_empty()).then_some(detail)
}

#[cfg(test)]
mod tests {
    use super::{
        DOCUMENT_SYMBOL_DETAIL_MAX_CHARS, DOCUMENT_SYMBOL_NAME_MAX_CHARS, DocumentSymbolRowDisplay,
        document_symbol_detail, document_symbol_detail_label, document_symbol_jump_column,
        document_symbol_name, document_symbol_row_label, document_symbol_row_tooltip,
    };
    use kuroya_core::{LspDocumentSymbol, TextBuffer};
    use std::borrow::Cow;
    use std::path::PathBuf;

    #[test]
    fn document_symbol_row_label_includes_depth_kind_location_and_detail() {
        let symbol = LspDocumentSymbol {
            name: "run".to_owned(),
            detail: Some("fn()".to_owned()),
            kind: 12,
            path: PathBuf::from("src/main.rs"),
            line: 3,
            column: 5,
            end_line: 3,
            end_column: 8,
            depth: 2,
        };

        assert_eq!(document_symbol_row_label(&symbol), "    fn run  3:5  fn()");
    }

    #[test]
    fn document_symbol_row_label_sanitizes_lsp_fields() {
        let symbol = LspDocumentSymbol {
            name: "  run\n\t\u{202e}\u{0}now  ".to_owned(),
            detail: Some("  fn(\r\n\u{2066}\u{7}value: usize)\t ".to_owned()),
            kind: 12,
            path: PathBuf::from("src/main.rs"),
            line: 3,
            column: 5,
            end_line: 3,
            end_column: 8,
            depth: 32,
        };

        assert_eq!(
            document_symbol_row_label(&symbol),
            "                fn run now  3:5  fn( value: usize)"
        );
        assert_eq!(
            document_symbol_row_tooltip(&symbol),
            "fn run now at line 3, column 5 - fn( value: usize)"
        );
    }

    #[test]
    fn document_symbol_row_display_formats_sanitized_fields_consistently() {
        let symbol = LspDocumentSymbol {
            name: "  parse\n\u{202e}tokens  ".to_owned(),
            detail: Some("  Result<Ast,\u{2066} Error>\r\n ".to_owned()),
            kind: 12,
            path: PathBuf::from("src/parser.rs"),
            line: 11,
            column: 7,
            end_line: 15,
            end_column: 2,
            depth: 1,
        };
        let display = DocumentSymbolRowDisplay::new(&symbol);

        assert_eq!(
            display.label(),
            "  fn parse tokens  11:7  Result<Ast, Error>"
        );
        assert_eq!(
            display.tooltip(),
            "fn parse tokens at line 11, column 7 - Result<Ast, Error>"
        );

        let mut tooltip = String::from("stale tooltip");
        tooltip.clear();
        display.write_tooltip(&mut tooltip);
        assert_eq!(
            tooltip,
            "fn parse tokens at line 11, column 7 - Result<Ast, Error>"
        );
    }

    #[test]
    fn document_symbol_row_display_preserves_raw_symbol_fields() {
        let raw_name = "  parse\n\u{202e}tokens  ".to_owned();
        let raw_detail = "  Result<Ast,\u{2066} Error>\r\n ".to_owned();
        let raw_path = PathBuf::from("src/parser.rs");
        let symbol = LspDocumentSymbol {
            name: raw_name.clone(),
            detail: Some(raw_detail.clone()),
            kind: 12,
            path: raw_path.clone(),
            line: 11,
            column: 7,
            end_line: 15,
            end_column: 2,
            depth: 1,
        };

        let display = DocumentSymbolRowDisplay::new(&symbol);

        assert_eq!(
            display.label(),
            "  fn parse tokens  11:7  Result<Ast, Error>"
        );
        assert_eq!(symbol.name, raw_name);
        assert_eq!(symbol.detail.as_deref(), Some(raw_detail.as_str()));
        assert_eq!(symbol.path, raw_path);
        assert_eq!((symbol.line, symbol.column), (11, 7));
        assert_eq!((symbol.end_line, symbol.end_column), (15, 2));
        assert_eq!(symbol.depth, 1);
    }

    #[test]
    fn document_symbol_jump_column_converts_utf16_column_with_matching_buffer() {
        let path = PathBuf::from("src/main.rs");
        let buffer = TextBuffer::from_text(1, Some(path.clone()), "\u{1f600}alpha\n".to_owned());
        let symbol = LspDocumentSymbol {
            name: "alpha".to_owned(),
            detail: None,
            kind: 12,
            path,
            line: 1,
            column: 3,
            end_line: 1,
            end_column: 8,
            depth: 0,
        };

        assert_eq!(document_symbol_jump_column(Some(&buffer), &symbol), 2);
        assert_eq!(document_symbol_jump_column(None, &symbol), 3);
    }

    #[test]
    fn document_symbol_row_label_bounds_display_fields() {
        let symbol = LspDocumentSymbol {
            name: "n".repeat(DOCUMENT_SYMBOL_NAME_MAX_CHARS * 2),
            detail: Some("d".repeat(DOCUMENT_SYMBOL_DETAIL_MAX_CHARS * 2)),
            kind: 12,
            path: PathBuf::from("src/main.rs"),
            line: 3,
            column: 5,
            end_line: 3,
            end_column: 8,
            depth: 0,
        };
        let label = document_symbol_row_label(&symbol);
        let body = label
            .strip_prefix("fn ")
            .expect("symbol label should include kind and name");
        let (name, detail) = body
            .split_once("  3:5  ")
            .expect("symbol label should include bounded detail");

        assert_eq!(name.chars().count(), DOCUMENT_SYMBOL_NAME_MAX_CHARS);
        assert_eq!(detail.chars().count(), DOCUMENT_SYMBOL_DETAIL_MAX_CHARS);
        assert!(name.contains("..."));
        assert!(detail.contains("..."));
    }

    #[test]
    fn document_symbol_row_labels_borrow_clean_ascii_and_unicode() {
        for name in ["run", "parse_\u{03bb}"] {
            match document_symbol_name(name) {
                Cow::Borrowed(label) => assert_eq!(label, name),
                Cow::Owned(label) => panic!("expected borrowed name label, got {label:?}"),
            }
        }

        for detail in ["fn()", "Result<\u{03bb}>"] {
            match document_symbol_detail_label(detail) {
                Cow::Borrowed(label) => assert_eq!(label, detail),
                Cow::Owned(label) => panic!("expected borrowed detail label, got {label:?}"),
            }
        }
    }

    #[test]
    fn document_symbol_row_labels_own_dirty_truncated_and_fallback_values() {
        let dirty_name = document_symbol_name("run\n\u{202e}now");
        assert_eq!(dirty_name.as_ref(), "run now");
        assert!(matches!(dirty_name, Cow::Owned(_)));

        let long_name = "symbol-".repeat(DOCUMENT_SYMBOL_NAME_MAX_CHARS);
        let truncated_name = document_symbol_name(&long_name);
        assert!(truncated_name.as_ref().contains("..."));
        assert!(truncated_name.as_ref().chars().count() <= DOCUMENT_SYMBOL_NAME_MAX_CHARS);
        assert!(matches!(truncated_name, Cow::Owned(_)));

        let fallback_name = document_symbol_name("\n\t\u{202e}");
        assert_eq!(fallback_name.as_ref(), "<unnamed>");
        assert!(matches!(fallback_name, Cow::Owned(_)));

        let dirty_detail = document_symbol_detail_label("fn\n\u{2066}value");
        assert_eq!(dirty_detail.as_ref(), "fn value");
        assert!(matches!(dirty_detail, Cow::Owned(_)));

        let long_detail = "detail-".repeat(DOCUMENT_SYMBOL_DETAIL_MAX_CHARS);
        let truncated_detail = document_symbol_detail_label(&long_detail);
        assert!(truncated_detail.as_ref().contains("..."));
        assert!(truncated_detail.as_ref().chars().count() <= DOCUMENT_SYMBOL_DETAIL_MAX_CHARS);
        assert!(matches!(truncated_detail, Cow::Owned(_)));

        let fallback_detail = document_symbol_detail_label("\n\t\u{2066}");
        assert_eq!(fallback_detail.as_ref(), "");
        assert!(matches!(fallback_detail, Cow::Owned(_)));
        assert!(document_symbol_detail("\n\t\u{2066}").is_none());
    }

    #[test]
    fn document_symbol_row_label_names_empty_fields() {
        let symbol = LspDocumentSymbol {
            name: "\n\t\u{202e}\u{0}".to_owned(),
            detail: Some("\r\n\u{2066}".to_owned()),
            kind: 12,
            path: PathBuf::from("src/main.rs"),
            line: 3,
            column: 5,
            end_line: 3,
            end_column: 8,
            depth: 0,
        };

        assert_eq!(document_symbol_row_label(&symbol), "fn <unnamed>  3:5");
        assert_eq!(
            document_symbol_row_tooltip(&symbol),
            "fn <unnamed> at line 3, column 5"
        );
    }
}
