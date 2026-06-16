use crate::{
    KuroyaApp,
    path_display::{compact_path, sanitized_display_label_cow},
    ui_icons::{IconKind, icon_label},
};
use eframe::egui::{self, Color32, Label, RichText, Sense};
use kuroya_core::{BufferId, Command, LspDocumentSymbol};
use std::{
    borrow::Cow,
    fmt::Write as _,
    path::{Component, Path, PathBuf},
};

const BREADCRUMB_LABEL_MAX_CHARS: usize = 48;
const SYMBOL_BREADCRUMB_LABEL_MAX_CHARS: usize = 80;
const BREADCRUMB_HOVER_MAX_CHARS: usize = 160;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BreadcrumbItem {
    pub(crate) label: String,
    pub(crate) hover_text: String,
    pub(crate) path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SymbolBreadcrumbItem {
    pub(crate) label: String,
    pub(crate) hover_text: String,
    pub(crate) path: PathBuf,
    pub(crate) line: usize,
    pub(crate) column: usize,
}

impl KuroyaApp {
    pub(crate) fn render_breadcrumbs(
        &mut self,
        ui: &mut egui::Ui,
        active_id: BufferId,
        path: &Path,
    ) {
        let active_path = lexical_path(path);
        let items = breadcrumb_items_for_lexical_path(&self.workspace.root, &active_path);
        let cursor_line = self
            .buffer(active_id)
            .map(|buffer| buffer.cursor_position().line.saturating_add(1));
        let symbol_items = match (self.document_symbols_path.as_deref(), cursor_line) {
            (Some(symbols_path), Some(cursor_line))
                if path_matches_lexical_path(symbols_path, &active_path) =>
            {
                symbol_breadcrumb_items_for_lexical_path(
                    &self.document_symbols,
                    &active_path,
                    cursor_line,
                )
            }
            _ => Vec::new(),
        };

        let last_index = items.len().saturating_sub(1);
        for (index, item) in items.into_iter().enumerate() {
            if index > 0 {
                icon_label(
                    ui,
                    IconKind::ChevronRight,
                    Color32::from_rgb(96, 105, 118),
                    "Path separator",
                );
            }
            let text_color = if index == last_index {
                Color32::from_rgb(214, 219, 227)
            } else {
                Color32::from_rgb(136, 146, 160)
            };
            let BreadcrumbItem {
                label,
                hover_text,
                path: item_path,
            } = item;
            let response = ui
                .add(
                    Label::new(RichText::new(label).small().color(text_color))
                        .sense(Sense::click()),
                )
                .on_hover_text(hover_text);
            if response.clicked() {
                self.command_bus
                    .push(breadcrumb_click_command(&item_path, path));
            }
        }

        for item in symbol_items {
            let SymbolBreadcrumbItem {
                label,
                hover_text,
                path: target_path,
                line,
                column,
            } = item;
            icon_label(
                ui,
                IconKind::ChevronRight,
                Color32::from_rgb(96, 105, 118),
                "Symbol separator",
            );
            let response = ui
                .add(
                    Label::new(
                        RichText::new(label)
                            .small()
                            .color(Color32::from_rgb(214, 219, 227)),
                    )
                    .sense(Sense::click()),
                )
                .on_hover_text(hover_text);
            if response.clicked() {
                self.command_bus.push(Command::OpenFileAt {
                    path: target_path,
                    line,
                    column,
                });
            }
        }
    }
}

#[cfg(test)]
pub(crate) fn breadcrumb_items(root: &Path, path: &Path) -> Vec<BreadcrumbItem> {
    let path = lexical_path(path);
    breadcrumb_items_for_lexical_path(root, &path)
}

fn breadcrumb_items_for_lexical_path(root: &Path, path: &Path) -> Vec<BreadcrumbItem> {
    let root = lexical_path(root);
    let Ok(relative) = path.strip_prefix(&root) else {
        return vec![breadcrumb_item(&compact_path(path), path.to_path_buf())];
    };

    let mut items = Vec::new();
    items.push(breadcrumb_item(&compact_path(&root), root.clone()));

    let mut current = root;
    for component in relative.components() {
        let label = component.as_os_str().to_string_lossy();
        current.push(component.as_os_str());
        items.push(breadcrumb_item(&label, current.clone()));
    }
    items
}

fn breadcrumb_item(label_text: &str, path: PathBuf) -> BreadcrumbItem {
    BreadcrumbItem {
        label: breadcrumb_path_label(label_text),
        hover_text: breadcrumb_path_hover_text(&path),
        path,
    }
}

pub(crate) fn breadcrumb_click_command(item_path: &Path, active_path: &Path) -> Command {
    if paths_match_lexically(item_path, active_path) {
        Command::OpenFile(item_path.to_path_buf())
    } else {
        Command::RevealFileInExplorer(item_path.to_path_buf())
    }
}

#[cfg(test)]
pub(crate) fn symbol_breadcrumb_items(
    symbols: &[LspDocumentSymbol],
    active_path: &Path,
    cursor_line: usize,
) -> Vec<SymbolBreadcrumbItem> {
    let active_path = lexical_path(active_path);
    symbol_breadcrumb_items_for_lexical_path(symbols, &active_path, cursor_line)
}

fn symbol_breadcrumb_items_for_lexical_path(
    symbols: &[LspDocumentSymbol],
    active_path: &Path,
    cursor_line: usize,
) -> Vec<SymbolBreadcrumbItem> {
    let mut candidates = symbols
        .iter()
        .enumerate()
        .filter(|(_, symbol)| {
            symbol_location_is_valid(symbol)
                && symbol_contains_line(symbol, cursor_line)
                && path_matches_lexical_path(&symbol.path, active_path)
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|(left_index, left), (right_index, right)| {
        left.depth
            .cmp(&right.depth)
            .then_with(|| left.line.cmp(&right.line))
            .then_with(|| left.column.cmp(&right.column))
            .then_with(|| right.end_line.cmp(&left.end_line))
            .then_with(|| right.end_column.cmp(&left.end_column))
            .then_with(|| left_index.cmp(right_index))
    });

    let mut stack = Vec::<(usize, SymbolBreadcrumbItem)>::with_capacity(candidates.len());
    for (_, symbol) in candidates {
        while stack
            .last()
            .is_some_and(|(depth, _)| *depth >= symbol.depth)
        {
            stack.pop();
        }
        stack.push((symbol.depth, symbol_breadcrumb_item(symbol)));
    }

    stack.into_iter().map(|(_, item)| item).collect()
}

fn symbol_breadcrumb_item(symbol: &LspDocumentSymbol) -> SymbolBreadcrumbItem {
    SymbolBreadcrumbItem {
        label: symbol_breadcrumb_label(&symbol.name),
        hover_text: symbol_breadcrumb_target_hover_text(&symbol.path, symbol.line, symbol.column),
        path: symbol.path.clone(),
        line: symbol.line,
        column: symbol.column,
    }
}

fn symbol_contains_line(symbol: &LspDocumentSymbol, cursor_line: usize) -> bool {
    let start = symbol.line.min(symbol.end_line);
    let end = symbol.line.max(symbol.end_line);
    (start..=end).contains(&cursor_line)
}

fn symbol_location_is_valid(symbol: &LspDocumentSymbol) -> bool {
    symbol.line > 0 && symbol.column > 0 && symbol.end_line > 0 && symbol.end_column > 0
}

fn breadcrumb_path_label(text: &str) -> String {
    normalized_breadcrumb_label(text, BREADCRUMB_LABEL_MAX_CHARS)
        .unwrap_or_else(|| "<unnamed>".to_owned())
}

fn symbol_breadcrumb_label(text: &str) -> String {
    normalized_breadcrumb_label(text, SYMBOL_BREADCRUMB_LABEL_MAX_CHARS)
        .unwrap_or_else(|| "<unnamed>".to_owned())
}

fn normalized_breadcrumb_label(text: &str, max_chars: usize) -> Option<String> {
    if max_chars == 0 {
        return None;
    }

    let mut normalized = String::with_capacity(text.len().min(max_chars));
    let mut chars = 0usize;
    let mut pending_space = false;
    let mut truncated = false;

    for ch in text.trim().chars() {
        if is_bidi_format_control(ch) {
            continue;
        }

        if ch.is_control() || ch.is_whitespace() {
            pending_space = !normalized.is_empty();
            continue;
        }

        let needed = usize::from(pending_space) + 1;
        if chars + needed > max_chars {
            truncated = true;
            break;
        }

        if pending_space {
            normalized.push(' ');
            chars += 1;
            pending_space = false;
        }

        normalized.push(ch);
        chars += 1;
    }

    if normalized.is_empty() {
        return None;
    }

    if truncated {
        Some(truncated_breadcrumb_label(&normalized, max_chars))
    } else {
        Some(normalized)
    }
}

fn truncated_breadcrumb_label(text: &str, max_chars: usize) -> String {
    const TRUNCATION: &str = "...";
    const TRUNCATION_CHARS: usize = 3;

    if max_chars <= TRUNCATION_CHARS {
        return text.chars().take(max_chars).collect();
    }

    let keep = max_chars - TRUNCATION_CHARS;
    let mut label = text.chars().take(keep).collect::<String>();
    label.push_str(TRUNCATION);
    label
}

fn is_bidi_format_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{061c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
    )
}

fn paths_match_lexically(left: &Path, right: &Path) -> bool {
    left == right || path_matches_lexical_path(left, &lexical_path(right))
}

fn path_matches_lexical_path(candidate: &Path, lexical_candidate: &Path) -> bool {
    candidate == lexical_candidate || lexical_path(candidate) == lexical_candidate
}

fn lexical_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match normalized.components().next_back() {
                Some(Component::Normal(_)) => {
                    normalized.pop();
                }
                Some(Component::Prefix(_)) | Some(Component::RootDir) => {}
                Some(Component::CurDir) | Some(Component::ParentDir) | None => {
                    normalized.push(component.as_os_str());
                }
            },
            Component::Normal(part) => normalized.push(part),
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
        }
    }

    if normalized.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        normalized
    }
}

#[cfg(test)]
pub(crate) fn symbol_breadcrumb_click_command(item: &SymbolBreadcrumbItem) -> Command {
    Command::OpenFileAt {
        path: item.path.clone(),
        line: item.line,
        column: item.column,
    }
}

fn breadcrumb_path_hover_text(path: &Path) -> String {
    breadcrumb_owned_display_label(path.display().to_string(), BREADCRUMB_HOVER_MAX_CHARS, ".")
}

#[cfg(test)]
fn symbol_breadcrumb_hover_text(item: &SymbolBreadcrumbItem) -> String {
    item.hover_text.clone()
}

fn symbol_breadcrumb_target_hover_text(path: &Path, line: usize, column: usize) -> String {
    let mut value = path.display().to_string();
    write!(&mut value, ":{line}:{column}").expect("writing to a String cannot fail");
    breadcrumb_owned_display_label(value, BREADCRUMB_HOVER_MAX_CHARS, ".")
}

fn breadcrumb_owned_display_label(value: String, max_chars: usize, fallback: &str) -> String {
    match sanitized_display_label_cow(&value, max_chars, fallback) {
        Cow::Borrowed(label) if label.as_ptr() == value.as_ptr() && label.len() == value.len() => {
            value
        }
        Cow::Borrowed(label) => label.to_owned(),
        Cow::Owned(label) => label,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BREADCRUMB_HOVER_MAX_CHARS, BREADCRUMB_LABEL_MAX_CHARS, SYMBOL_BREADCRUMB_LABEL_MAX_CHARS,
        SymbolBreadcrumbItem, breadcrumb_click_command, breadcrumb_items,
        breadcrumb_path_hover_text, lexical_path, path_matches_lexical_path,
        symbol_breadcrumb_click_command, symbol_breadcrumb_hover_text, symbol_breadcrumb_items,
        symbol_breadcrumb_items_for_lexical_path,
    };
    use kuroya_core::{Command, LspDocumentSymbol};
    use std::path::{Path, PathBuf};

    #[test]
    fn breadcrumb_items_include_workspace_root_and_relative_components() {
        let root = PathBuf::from("workspace");
        let path = root.join("src").join("main.rs");
        let items = breadcrumb_items(&root, &path);

        assert_eq!(
            items
                .iter()
                .map(|item| item.label.as_str())
                .collect::<Vec<_>>(),
            ["workspace", "src", "main.rs"]
        );
        assert_eq!(items[0].path, root);
        assert_eq!(items[1].path, PathBuf::from("workspace").join("src"));
        assert_eq!(items[2].path, path);
    }

    #[test]
    fn breadcrumb_items_keep_external_files_self_contained() {
        let root = PathBuf::from("workspace");
        let path = PathBuf::from("external").join("notes.txt");
        let items = breadcrumb_items(&root, &path);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "notes.txt");
        assert_eq!(items[0].path, path);
    }

    #[test]
    fn breadcrumb_items_collapse_lexical_components_inside_workspace() {
        let root = PathBuf::from("workspace");
        let path = root.join("src").join("..").join("main.rs");
        let items = breadcrumb_items(&root, &path);

        assert_eq!(
            items
                .iter()
                .map(|item| item.label.as_str())
                .collect::<Vec<_>>(),
            ["workspace", "main.rs"]
        );
        assert_eq!(items[1].path, root.join("main.rs"));
    }

    #[test]
    fn breadcrumb_items_do_not_keep_escaped_workspace_segments() {
        let root = PathBuf::from("workspace");
        let path = root.join("..").join("outside").join("notes.txt");
        let items = breadcrumb_items(&root, &path);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "notes.txt");
        assert_eq!(items[0].path, PathBuf::from("outside").join("notes.txt"));
    }

    #[test]
    fn breadcrumb_items_normalize_and_truncate_display_labels_only() {
        let root = PathBuf::from("workspace");
        let spaced = "  src\nmodule\tname  ";
        let long = "very_long_component_name_that_should_be_truncated_before_it_overflows.rs";
        let path = root.join(spaced).join(long);

        let items = breadcrumb_items(&root, &path);

        assert_eq!(items[1].label, "src module name");
        assert!(items[2].label.ends_with("..."));
        assert!(items[2].label.chars().count() <= BREADCRUMB_LABEL_MAX_CHARS);
        assert!(!items[2].label.contains('\n'));
        assert!(!items[2].label.contains('\t'));
        assert_eq!(items[1].path, root.join(spaced));
        assert_eq!(items[2].path, path);
    }

    #[test]
    fn breadcrumb_items_strip_bidi_controls_from_display_labels_only() {
        let root = PathBuf::from("workspace");
        let bidi_component = "src\u{202e}module\u{200f}";
        let path = root.join(bidi_component).join("main.rs");

        let items = breadcrumb_items(&root, &path);

        assert_eq!(items[1].label, "srcmodule");
        assert!(!items[1].label.chars().any(super::is_bidi_format_control));
        assert_eq!(items[1].path, root.join(bidi_component));
    }

    #[test]
    fn breadcrumb_items_keep_blank_components_visible() {
        let root = PathBuf::from("workspace");
        let blank = "   ";
        let path = root.join(blank);

        let items = breadcrumb_items(&root, &path);

        assert_eq!(items[1].label, "<unnamed>");
        assert_eq!(items[1].path, path);
    }

    #[test]
    fn breadcrumb_clicks_open_leaf_and_reveal_ancestors() {
        let root = PathBuf::from("workspace");
        let path = root.join("src").join("main.rs");

        assert_eq!(
            breadcrumb_click_command(&path, &path),
            Command::OpenFile(path.clone())
        );
        assert_eq!(
            breadcrumb_click_command(&root, &path),
            Command::RevealFileInExplorer(root)
        );
    }

    #[test]
    fn breadcrumb_clicks_treat_lexically_equivalent_leaf_as_active() {
        let path = PathBuf::from("workspace").join("src").join("main.rs");
        let equivalent = PathBuf::from("workspace")
            .join("src")
            .join("..")
            .join("src")
            .join("main.rs");

        assert_eq!(
            breadcrumb_click_command(&path, &equivalent),
            Command::OpenFile(path)
        );
    }

    #[test]
    fn symbol_breadcrumbs_follow_nested_cursor_scope() {
        let path = PathBuf::from("workspace/src/main.rs");
        let symbols = vec![
            symbol(&path, "App", 1, 1, 80, 1, 0),
            symbol(&path, "render", 20, 5, 40, 6, 1),
            symbol(&path, "helper", 45, 5, 60, 6, 1),
        ];

        let items = symbol_breadcrumb_items(&symbols, &path, 22);

        assert_eq!(
            items
                .iter()
                .map(|item| item.label.as_str())
                .collect::<Vec<_>>(),
            ["App", "render"]
        );
    }

    #[test]
    fn symbol_breadcrumbs_match_lexically_equivalent_paths() {
        let active_path = PathBuf::from("workspace/src/main.rs");
        let symbol_path = PathBuf::from("workspace")
            .join("src")
            .join("..")
            .join("src")
            .join("main.rs");
        let symbols = vec![symbol(&symbol_path, "App", 1, 1, 20, 1, 0)];

        let items = symbol_breadcrumb_items(&symbols, &active_path, 4);

        assert_eq!(items, vec![symbol_item("App", &symbol_path, 1, 1)]);
        assert_eq!(
            items[0].hover_text,
            super::symbol_breadcrumb_target_hover_text(&symbol_path, 1, 1)
        );
    }

    #[test]
    fn precomputed_lexical_path_matches_equivalent_candidates() {
        let active_path = PathBuf::from("workspace")
            .join("src")
            .join("..")
            .join("src")
            .join("main.rs");
        let active_path = lexical_path(&active_path);

        assert_eq!(
            active_path,
            PathBuf::from("workspace").join("src").join("main.rs")
        );
        assert!(path_matches_lexical_path(
            &PathBuf::from("workspace/src/main.rs"),
            &active_path
        ));
        assert!(path_matches_lexical_path(&active_path, &active_path));
        assert!(!path_matches_lexical_path(
            &PathBuf::from("workspace/src/lib.rs"),
            &active_path
        ));
    }

    #[test]
    fn symbol_breadcrumbs_accept_precomputed_lexical_active_path() {
        let active_path = PathBuf::from("workspace")
            .join("src")
            .join("..")
            .join("src")
            .join("main.rs");
        let active_path = lexical_path(&active_path);
        let symbol_path = PathBuf::from("workspace/src/main.rs");
        let other_path = PathBuf::from("workspace/src/lib.rs");
        let symbols = vec![
            symbol(&other_path, "Other", 1, 1, 80, 1, 0),
            symbol(&symbol_path, "App", 1, 1, 80, 1, 0),
            symbol(&symbol_path, "render", 20, 5, 40, 6, 1),
            symbol(&symbol_path, "later", 45, 5, 60, 6, 1),
        ];

        let items = symbol_breadcrumb_items_for_lexical_path(&symbols, &active_path, 22);

        assert_eq!(
            items
                .iter()
                .map(|item| item.label.as_str())
                .collect::<Vec<_>>(),
            ["App", "render"]
        );
        assert!(items.iter().all(|item| item.path == symbol_path));
    }

    #[test]
    fn symbol_breadcrumbs_choose_matching_flat_sibling() {
        let path = PathBuf::from("workspace/src/main.rs");
        let symbols = vec![
            symbol(&path, "first", 1, 1, 10, 1, 0),
            symbol(&path, "second", 20, 1, 30, 1, 0),
        ];

        let items = symbol_breadcrumb_items(&symbols, &path, 25);

        assert_eq!(items, vec![symbol_item("second", &path, 20, 1)]);
    }

    #[test]
    fn symbol_breadcrumbs_recover_parent_path_from_out_of_order_symbols() {
        let path = PathBuf::from("workspace/src/main.rs");
        let symbols = vec![
            symbol(&path, "render", 20, 5, 40, 6, 1),
            symbol(&path, "helper", 45, 5, 60, 6, 1),
            symbol(&path, "App", 1, 1, 80, 1, 0),
        ];

        let items = symbol_breadcrumb_items(&symbols, &path, 22);

        assert_eq!(
            items
                .iter()
                .map(|item| item.label.as_str())
                .collect::<Vec<_>>(),
            ["App", "render"]
        );
    }

    #[test]
    fn symbol_breadcrumbs_normalize_and_truncate_display_labels_only() {
        let path = PathBuf::from("workspace/src/main.rs");
        let long_name =
            "render_with_a_symbol_name_that_is_far_too_long_for_the_editor_toolbar_without_bounds";
        let symbols = vec![
            symbol(&path, "  App\nRoot  ", 1, 1, 80, 1, 0),
            symbol(&path, long_name, 20, 5, 40, 6, 1),
        ];

        let items = symbol_breadcrumb_items(&symbols, &path, 22);

        assert_eq!(items[0].label, "App Root");
        assert!(items[1].label.ends_with("..."));
        assert!(items[1].label.chars().count() <= SYMBOL_BREADCRUMB_LABEL_MAX_CHARS);
        assert_eq!(items[1].path, path);
        assert_eq!(items[1].line, 20);
        assert_eq!(items[1].column, 5);
    }

    #[test]
    fn symbol_breadcrumbs_strip_bidi_controls_from_display_labels_only() {
        let path = PathBuf::from("workspace/src/main.rs");
        let symbols = vec![symbol(&path, "\u{202e}Ren\u{200f}der", 1, 1, 20, 1, 0)];

        let items = symbol_breadcrumb_items(&symbols, &path, 4);

        assert_eq!(items[0].label, "Render");
        assert!(!items[0].label.chars().any(super::is_bidi_format_control));
        assert_eq!(items[0].path, path);
    }

    #[test]
    fn symbol_breadcrumbs_keep_blank_names_visible() {
        let path = PathBuf::from("workspace/src/main.rs");
        let symbols = vec![symbol(&path, "\n\t\u{0}", 1, 1, 20, 1, 0)];

        let items = symbol_breadcrumb_items(&symbols, &path, 4);

        assert_eq!(items[0].label, "<unnamed>");
    }

    #[test]
    fn symbol_breadcrumbs_filter_invalid_zero_locations() {
        let path = PathBuf::from("workspace/src/main.rs");
        let symbols = vec![
            symbol(&path, "zero line", 0, 1, 10, 1, 0),
            symbol(&path, "zero column", 1, 0, 10, 1, 0),
            symbol(&path, "zero end", 1, 1, 0, 1, 0),
            symbol(&path, "valid", 1, 1, 10, 1, 0),
        ];

        let items = symbol_breadcrumb_items(&symbols, &path, 4);

        assert_eq!(items, vec![symbol_item("valid", &path, 1, 1)]);
    }

    #[test]
    fn breadcrumb_hover_text_sanitizes_and_bounds_path_display() {
        let path = PathBuf::from("workspace")
            .join(format!("bad\n{}\u{202e}tail.rs", "very-long-".repeat(32)));
        let item = symbol_item("render", &path, 20, 5);

        let file_hover = breadcrumb_path_hover_text(&path);
        let symbol_hover = symbol_breadcrumb_hover_text(&item);

        assert_breadcrumb_display_text_is_safe(&file_hover);
        assert_breadcrumb_display_text_is_safe(&symbol_hover);
        assert!(file_hover.contains("..."));
        assert!(symbol_hover.contains("..."));
        assert!(symbol_hover.ends_with(":20:5"));
        assert!(file_hover.chars().count() <= BREADCRUMB_HOVER_MAX_CHARS);
        assert!(symbol_hover.chars().count() <= BREADCRUMB_HOVER_MAX_CHARS);
    }

    #[test]
    fn symbol_breadcrumb_hover_text_preserves_exact_target_format() {
        let path = PathBuf::from("workspace/src/main.rs");
        let item = symbol_item("render", &path, 20, 5);

        assert_eq!(
            symbol_breadcrumb_hover_text(&item),
            "workspace/src/main.rs:20:5"
        );
    }

    #[test]
    fn symbol_breadcrumb_clicks_open_symbol_location() {
        let path = PathBuf::from("workspace/src/main.rs");
        let item = symbol_item("render", &path, 20, 5);

        assert_eq!(
            symbol_breadcrumb_click_command(&item),
            Command::OpenFileAt {
                path,
                line: 20,
                column: 5
            }
        );
    }

    #[test]
    fn breadcrumb_items_precompute_hover_text_from_target_path() {
        let root = PathBuf::from("workspace");
        let raw_component = "src\nmodule\u{202e}";
        let path = root.join(raw_component).join("main.rs");

        let items = breadcrumb_items(&root, &path);

        assert_eq!(items[1].path, root.join(raw_component));
        assert_eq!(
            items[1].hover_text,
            breadcrumb_path_hover_text(&items[1].path)
        );
        assert_breadcrumb_display_text_is_safe(&items[1].hover_text);
    }

    fn symbol(
        path: &Path,
        name: &str,
        line: usize,
        column: usize,
        end_line: usize,
        end_column: usize,
        depth: usize,
    ) -> LspDocumentSymbol {
        LspDocumentSymbol {
            name: name.to_owned(),
            detail: None,
            kind: 12,
            path: path.to_path_buf(),
            line,
            column,
            end_line,
            end_column,
            depth,
        }
    }

    fn symbol_item(label: &str, path: &Path, line: usize, column: usize) -> SymbolBreadcrumbItem {
        SymbolBreadcrumbItem {
            label: label.to_owned(),
            hover_text: super::symbol_breadcrumb_target_hover_text(path, line, column),
            path: path.to_path_buf(),
            line,
            column,
        }
    }

    fn assert_breadcrumb_display_text_is_safe(value: &str) {
        assert!(
            !value.chars().any(is_unsafe_breadcrumb_display_char),
            "display text contains unsafe characters: {value:?}"
        );
    }

    fn is_unsafe_breadcrumb_display_char(ch: char) -> bool {
        ch.is_control()
            || matches!(ch, '\u{2028}' | '\u{2029}')
            || super::is_bidi_format_control(ch)
    }
}
