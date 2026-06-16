use crate::{
    KuroyaApp,
    keybinding_input::capture_keybinding_input,
    keybinding_parse::normalize_key_chord,
    keybindings::{keybinding_items, keybinding_search_text},
    keybindings_panel_actions::PendingKeybindingsPanelActions,
    path_display::sanitized_display_label_cow,
};
use eframe::egui::{self, Context, Id};
use kuroya_core::{Command, keymap::KeyBinding, text_match::ascii_case_insensitive_contains};
use std::{borrow::Cow, sync::Arc};

mod buttons;
mod controls;
mod rows;

impl KuroyaApp {
    pub(crate) fn render_keybindings_panel(&mut self, ctx: &Context) {
        if sanitize_keybindings_query(&mut self.keybindings_query) {
            self.keybindings_selected = 0;
        }
        let query = self.keybindings_query.trim();
        let items = cached_keybinding_items(ctx, &self.settings.keymap.bindings, query);
        crate::ui_state::clamp_selection(&mut self.keybindings_selected, items.len());
        let capturing = self.keybinding_capture_command.is_some();
        let mut actions = PendingKeybindingsPanelActions {
            captured: capturing.then(|| capture_keybinding_input(ctx)).flatten(),
            ..Default::default()
        };

        egui::Window::new("Keyboard Shortcuts")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 72.0])
            .fixed_size([620.0, 440.0])
            .show(ctx, |ui| {
                let selection_changed =
                    controls::render_keybinding_controls(self, ui, &items, capturing, &mut actions);
                buttons::render_keybinding_buttons(self, ui, &items, capturing, &mut actions);
                ui.separator();

                rows::render_keybinding_rows(
                    ui,
                    &items,
                    &self.keybindings_query,
                    &mut self.keybindings_selected,
                    capturing,
                    self.settings.ui_font_size,
                    selection_changed,
                    &mut actions,
                );
            });

        guard_keybindings_panel_actions(&mut actions, &items);
        self.apply_keybindings_panel_actions(actions);
    }
}

const KEYBINDINGS_QUERY_MAX_CHARS: usize = 160;
const KEYBINDING_TEXT_MAX_CHARS: usize = 96;
const KEYBINDING_INLINE_QUERY_TERMS: usize = 8;
const KEYBINDINGS_PANEL_CACHE_ID: &str = "kuroya.keybindings_panel.items_cache";

#[derive(Clone, Debug, PartialEq, Eq)]
struct KeybindingPanelItem {
    chord: String,
    command: Command,
    label: String,
    search_text: String,
}

#[derive(Clone, Default)]
struct KeybindingsPanelItemsCache {
    bindings_valid: bool,
    filtered_valid: bool,
    query: String,
    bindings: Vec<KeyBinding>,
    sanitized_items: Arc<Vec<KeybindingPanelItem>>,
    items: Arc<Vec<KeybindingPanelItem>>,
}

impl KeybindingsPanelItemsCache {
    fn items_for(&mut self, bindings: &[KeyBinding], query: &str) -> Arc<Vec<KeybindingPanelItem>> {
        if !self.bindings_match(bindings) {
            self.bindings_valid = true;
            self.filtered_valid = false;
            self.bindings.clear();
            self.bindings.extend_from_slice(bindings);
            self.sanitized_items = Arc::new(sanitized_keybinding_items(bindings));
        }

        if !self.filtered_match(query) {
            let next_items = if query.is_empty() {
                Arc::clone(&self.sanitized_items)
            } else {
                let source_items = if self.can_refine_previous_filter(query) {
                    Arc::clone(&self.items)
                } else {
                    Arc::clone(&self.sanitized_items)
                };
                match filter_keybinding_items_for_cache(source_items.as_slice(), query) {
                    FilteredKeybindingItems::All => source_items,
                    FilteredKeybindingItems::Filtered(filtered_items) => Arc::new(filtered_items),
                }
            };
            self.filtered_valid = true;
            self.query.clear();
            self.query.push_str(query);
            self.items = next_items;
        }
        Arc::clone(&self.items)
    }

    #[cfg(test)]
    fn matches(&self, bindings: &[KeyBinding], query: &str) -> bool {
        self.bindings_match(bindings) && self.filtered_match(query)
    }

    fn bindings_match(&self, bindings: &[KeyBinding]) -> bool {
        self.bindings_valid && self.bindings == bindings
    }

    fn filtered_match(&self, query: &str) -> bool {
        self.filtered_valid && self.query == query
    }

    fn can_refine_previous_filter(&self, query: &str) -> bool {
        self.filtered_valid && !self.query.is_empty() && query.starts_with(self.query.as_str())
    }

    #[cfg(test)]
    fn filter_source_for(&self, query: &str) -> &[KeybindingPanelItem] {
        if self.can_refine_previous_filter(query) {
            &self.items
        } else {
            &self.sanitized_items
        }
    }
}

fn cached_keybinding_items(
    ctx: &Context,
    bindings: &[KeyBinding],
    query: &str,
) -> Arc<Vec<KeybindingPanelItem>> {
    ctx.data_mut(|data| {
        data.get_temp_mut_or_default::<KeybindingsPanelItemsCache>(Id::new(
            KEYBINDINGS_PANEL_CACHE_ID,
        ))
        .items_for(bindings, query)
    })
}

fn sanitized_keybinding_items(bindings: &[KeyBinding]) -> Vec<KeybindingPanelItem> {
    keybinding_items(bindings)
        .into_iter()
        .map(sanitized_keybinding_item)
        .collect()
}

#[cfg(test)]
fn filter_keybinding_items(items: &[KeybindingPanelItem], query: &str) -> Vec<KeybindingPanelItem> {
    match filter_keybinding_items_for_cache(items, query) {
        FilteredKeybindingItems::All => items.to_vec(),
        FilteredKeybindingItems::Filtered(filtered) => filtered,
    }
}

enum FilteredKeybindingItems {
    All,
    Filtered(Vec<KeybindingPanelItem>),
}

fn filter_keybinding_items_for_cache(
    items: &[KeybindingPanelItem],
    query: &str,
) -> FilteredKeybindingItems {
    let terms = keybinding_query_terms(query);
    if terms.is_empty() {
        return FilteredKeybindingItems::All;
    }

    let mut filtered: Option<Vec<KeybindingPanelItem>> = None;
    for (index, item) in items.iter().enumerate() {
        if keybinding_search_text_matches_terms(&item.search_text, terms.as_slice()) {
            if let Some(filtered) = filtered.as_mut() {
                filtered.push(item.clone());
            }
        } else if filtered.is_none() {
            let mut kept = Vec::with_capacity(items.len());
            kept.extend(items[..index].iter().cloned());
            filtered = Some(kept);
        }
    }

    match filtered {
        Some(filtered) => FilteredKeybindingItems::Filtered(filtered),
        None => FilteredKeybindingItems::All,
    }
}

#[derive(Debug, PartialEq, Eq)]
enum KeybindingQueryTerms<'a> {
    Inline {
        terms: [&'a str; KEYBINDING_INLINE_QUERY_TERMS],
        len: usize,
    },
    Heap(Vec<&'a str>),
}

impl<'a> KeybindingQueryTerms<'a> {
    fn as_slice(&self) -> &[&'a str] {
        match self {
            Self::Inline { terms, len } => &terms[..*len],
            Self::Heap(terms) => terms,
        }
    }

    fn is_empty(&self) -> bool {
        self.as_slice().is_empty()
    }
}

fn keybinding_query_terms(query: &str) -> KeybindingQueryTerms<'_> {
    let mut terms = [""; KEYBINDING_INLINE_QUERY_TERMS];
    let mut len = 0usize;
    let mut split = query.split_whitespace();
    while let Some(term) = split.next() {
        if len < KEYBINDING_INLINE_QUERY_TERMS {
            terms[len] = term;
            len += 1;
        } else {
            let (remaining, _) = split.size_hint();
            let mut heap = Vec::with_capacity(KEYBINDING_INLINE_QUERY_TERMS + 1 + remaining);
            heap.extend_from_slice(&terms);
            heap.push(term);
            heap.extend(split);
            return KeybindingQueryTerms::Heap(heap);
        }
    }

    KeybindingQueryTerms::Inline { terms, len }
}

fn keybinding_search_text_matches_terms(search_text: &str, terms: &[&str]) -> bool {
    terms
        .iter()
        .all(|term| ascii_case_insensitive_contains(search_text, term))
}

fn guard_keybindings_panel_actions(
    actions: &mut PendingKeybindingsPanelActions,
    items: &[KeybindingPanelItem],
) {
    if actions
        .start_capture
        .as_ref()
        .is_some_and(|command| !items_contain_command(items, command))
    {
        actions.start_capture = None;
    }

    if actions
        .remove_binding
        .as_ref()
        .is_some_and(|command| !items_contain_bound_command(items, command))
    {
        actions.remove_binding = None;
    }
}

fn items_contain_command(items: &[KeybindingPanelItem], command: &Command) -> bool {
    items.iter().any(|item| &item.command == command)
}

fn items_contain_bound_command(items: &[KeybindingPanelItem], command: &Command) -> bool {
    items
        .iter()
        .any(|item| &item.command == command && !item.chord.is_empty())
}

fn sanitize_keybindings_query(query: &mut String) -> bool {
    if query.is_empty() {
        return false;
    }

    let Cow::Owned(sanitized) = sanitize_keybindings_query_cow(query.as_str()) else {
        return false;
    };
    if sanitized == *query {
        return false;
    }
    *query = sanitized;
    true
}

fn sanitize_keybindings_query_cow(query: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(query, KEYBINDINGS_QUERY_MAX_CHARS, "")
}

fn sanitized_keybinding_item(
    (mut chord, command, mut label): (String, Command, String),
) -> KeybindingPanelItem {
    sanitize_keybinding_chord_in_place(&mut chord);
    sanitize_keybinding_label_in_place(&mut label);
    let search_text = keybinding_search_text(chord.as_str(), label.as_str());
    KeybindingPanelItem {
        chord,
        command,
        label,
        search_text,
    }
}

fn sanitize_keybinding_chord_in_place(chord: &mut String) {
    let Cow::Owned(sanitized) = sanitize_keybinding_chord_cow(chord.as_str()) else {
        return;
    };
    if sanitized != chord.as_str() {
        *chord = sanitized;
    }
}

fn sanitize_keybinding_label_in_place(label: &mut String) {
    let Cow::Owned(sanitized) = sanitize_keybinding_label_cow(label.as_str()) else {
        return;
    };
    if sanitized != label.as_str() {
        *label = sanitized;
    }
}

#[cfg(test)]
fn sanitize_keybinding_chord(chord: &str) -> String {
    sanitize_keybinding_chord_cow(chord).into_owned()
}

fn sanitize_keybinding_chord_cow(chord: &str) -> Cow<'_, str> {
    if chord.is_empty() {
        return Cow::Borrowed("");
    }
    if keybinding_chord_is_display_normalized(chord) {
        return Cow::Borrowed(chord);
    }
    if let Some(normalized) = normalize_key_chord(chord) {
        return if normalized == chord {
            Cow::Borrowed(chord)
        } else {
            Cow::Owned(normalized)
        };
    }
    sanitized_display_label_cow(chord, KEYBINDING_TEXT_MAX_CHARS, "Invalid shortcut")
}

fn keybinding_chord_is_display_normalized(chord: &str) -> bool {
    let mut next_modifier_index = 0usize;
    let mut saw_key = false;

    for part in chord.split('+') {
        if part.is_empty() || part.chars().any(char::is_whitespace) {
            return false;
        }

        if let Some(modifier_index) = keybinding_display_modifier_index(part) {
            if saw_key || modifier_index < next_modifier_index {
                return false;
            }
            next_modifier_index = modifier_index + 1;
        } else {
            if saw_key || !keybinding_display_key_name_is_canonical(part) {
                return false;
            }
            saw_key = true;
        }
    }

    saw_key
}

fn keybinding_display_modifier_index(part: &str) -> Option<usize> {
    match part {
        "Ctrl" => Some(0),
        "Alt" => Some(1),
        "Shift" => Some(2),
        "Cmd" => Some(3),
        _ => None,
    }
}

fn keybinding_display_key_name_is_canonical(part: &str) -> bool {
    matches!(
        part,
        "A" | "B"
            | "C"
            | "D"
            | "E"
            | "F"
            | "G"
            | "H"
            | "I"
            | "J"
            | "K"
            | "L"
            | "M"
            | "N"
            | "O"
            | "P"
            | "Q"
            | "R"
            | "S"
            | "T"
            | "U"
            | "V"
            | "W"
            | "X"
            | "Y"
            | "Z"
            | "0"
            | "1"
            | "2"
            | "3"
            | "4"
            | "5"
            | "6"
            | "7"
            | "8"
            | "9"
            | "Up"
            | "Down"
            | "Left"
            | "Right"
            | "Enter"
            | "Tab"
            | "Space"
            | "Backspace"
            | "Delete"
            | "Home"
            | "End"
            | "PageUp"
            | "PageDown"
            | "F1"
            | "F2"
            | "F3"
            | "F4"
            | "F5"
            | "F6"
            | "F7"
            | "F8"
            | "F9"
            | "F10"
            | "F11"
            | "F12"
            | "`"
            | "\\"
            | "["
            | "]"
            | ","
            | "."
            | "/"
            | "-"
            | "="
            | ";"
            | "'"
    )
}

#[cfg(test)]
fn sanitize_keybinding_label(label: &str) -> String {
    sanitize_keybinding_label_cow(label).into_owned()
}

fn sanitize_keybinding_label_cow(label: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(label, KEYBINDING_TEXT_MAX_CHARS, "Unnamed command")
}

#[cfg(test)]
mod tests {
    use super::{
        KEYBINDING_INLINE_QUERY_TERMS, KEYBINDING_TEXT_MAX_CHARS, KEYBINDINGS_QUERY_MAX_CHARS,
        KeybindingQueryTerms, KeybindingsPanelItemsCache, filter_keybinding_items,
        guard_keybindings_panel_actions, keybinding_query_terms,
        keybinding_search_text_matches_terms, sanitize_keybinding_chord,
        sanitize_keybinding_chord_cow, sanitize_keybinding_chord_in_place,
        sanitize_keybinding_label, sanitize_keybinding_label_cow,
        sanitize_keybinding_label_in_place, sanitize_keybindings_query,
        sanitize_keybindings_query_cow, sanitized_keybinding_item,
    };
    use crate::keybindings_panel_actions::PendingKeybindingsPanelActions;
    use kuroya_core::{Command, keymap::KeyBinding};
    use std::path::PathBuf;
    use std::{borrow::Cow, sync::Arc};

    #[test]
    fn keybindings_query_is_single_line_trimmed_and_bounded() {
        let mut query = format!(
            " \n{}\u{202e}{} ",
            "find\tcommand",
            "x".repeat(KEYBINDINGS_QUERY_MAX_CHARS + 24)
        );

        assert!(sanitize_keybindings_query(&mut query));

        assert!(!query.chars().any(char::is_control));
        assert!(!query.contains('\u{202e}'));
        assert!(!query.starts_with(' '));
        assert!(!query.ends_with(' '));
        assert_eq!(query.chars().count(), KEYBINDINGS_QUERY_MAX_CHARS);
    }

    #[test]
    fn keybindings_query_cow_borrows_clean_labels_and_in_place_noops() {
        assert!(matches!(
            sanitize_keybindings_query_cow("quick open"),
            Cow::Borrowed("quick open")
        ));

        let unicode = "\u{30ad}\u{30fc}\u{30dc}\u{30fc}\u{30c9}\u{691c}\u{7d22}";
        match sanitize_keybindings_query_cow(unicode) {
            Cow::Borrowed(label) => assert_eq!(label, unicode),
            Cow::Owned(label) => panic!("expected borrowed query label, got {label:?}"),
        }

        let mut query = unicode.to_owned();
        let ptr = query.as_ptr();
        assert!(!sanitize_keybindings_query(&mut query));
        assert_eq!(query, unicode);
        assert_eq!(query.as_ptr(), ptr);

        let mut empty = String::new();
        assert!(!sanitize_keybindings_query(&mut empty));
        assert!(empty.is_empty());
    }

    #[test]
    fn keybindings_query_cow_owns_dirty_truncated_and_fallback_labels() {
        let dirty = sanitize_keybindings_query_cow(" quick\nopen\u{202e} ");
        assert!(matches!(&dirty, Cow::Owned(_)));
        assert_eq!(dirty.as_ref(), "quick open");

        let long = "x".repeat(KEYBINDINGS_QUERY_MAX_CHARS + 24);
        let truncated = sanitize_keybindings_query_cow(&long);
        assert!(matches!(&truncated, Cow::Owned(_)));
        assert_eq!(truncated.chars().count(), KEYBINDINGS_QUERY_MAX_CHARS);
        assert!(truncated.contains("..."));

        let fallback = sanitize_keybindings_query_cow("\n\t\u{202e}");
        assert!(matches!(&fallback, Cow::Owned(_)));
        assert_eq!(fallback.as_ref(), "");
    }

    #[test]
    fn keybinding_display_text_is_single_line_and_bounded() {
        let label = sanitize_keybinding_label(&format!(
            "Open\n\u{202e}{}",
            "Workspace".repeat(KEYBINDING_TEXT_MAX_CHARS)
        ));
        let chord = sanitize_keybinding_chord("Ctrl+\tShift+\nP");

        assert!(!label.chars().any(char::is_control));
        assert!(!label.contains('\u{202e}'));
        assert_eq!(label.chars().count(), KEYBINDING_TEXT_MAX_CHARS);
        assert_eq!(chord, "Ctrl+Shift+P");
    }

    #[test]
    fn keybinding_chord_cow_borrows_clean_labels_and_in_place_noops() {
        assert!(matches!(
            sanitize_keybinding_chord_cow("Ctrl+P"),
            Cow::Borrowed("Ctrl+P")
        ));
        assert!(matches!(
            sanitize_keybinding_chord_cow("Ctrl+Alt+Shift+Cmd+F12"),
            Cow::Borrowed("Ctrl+Alt+Shift+Cmd+F12")
        ));
        assert!(matches!(
            sanitize_keybinding_chord_cow("PageDown"),
            Cow::Borrowed("PageDown")
        ));

        let unicode = "\u{2318} custom shortcut";
        match sanitize_keybinding_chord_cow(unicode) {
            Cow::Borrowed(label) => assert_eq!(label, unicode),
            Cow::Owned(label) => panic!("expected borrowed chord label, got {label:?}"),
        }

        let mut chord = unicode.to_owned();
        let ptr = chord.as_ptr();
        sanitize_keybinding_chord_in_place(&mut chord);
        assert_eq!(chord, unicode);
        assert_eq!(chord.as_ptr(), ptr);
    }

    #[test]
    fn keybinding_chord_cow_owns_normalized_dirty_truncated_and_fallback_labels() {
        let normalized = sanitize_keybinding_chord_cow(" control + p ");
        assert!(matches!(&normalized, Cow::Owned(_)));
        assert_eq!(normalized.as_ref(), "Ctrl+P");

        let reordered = sanitize_keybinding_chord_cow("Shift+Ctrl+P");
        assert!(matches!(&reordered, Cow::Owned(_)));
        assert_eq!(reordered.as_ref(), "Ctrl+Shift+P");

        let named_alias = sanitize_keybinding_chord_cow("ctrl+enter");
        assert!(matches!(&named_alias, Cow::Owned(_)));
        assert_eq!(named_alias.as_ref(), "Ctrl+Enter");

        let dirty = sanitize_keybinding_chord_cow("Ctrl+\tShift+\nP");
        assert!(matches!(&dirty, Cow::Owned(_)));
        assert_eq!(dirty.as_ref(), "Ctrl+Shift+P");

        let long = "x".repeat(KEYBINDING_TEXT_MAX_CHARS + 24);
        let truncated = sanitize_keybinding_chord_cow(&long);
        assert!(matches!(&truncated, Cow::Owned(_)));
        assert_eq!(truncated.chars().count(), KEYBINDING_TEXT_MAX_CHARS);
        assert!(truncated.contains("..."));

        let fallback = sanitize_keybinding_chord_cow("\n\t");
        assert!(matches!(&fallback, Cow::Owned(_)));
        assert_eq!(fallback.as_ref(), "Invalid shortcut");
    }

    #[test]
    fn keybinding_label_cow_borrows_clean_labels_and_in_place_noops() {
        assert!(matches!(
            sanitize_keybinding_label_cow("Open Workspace"),
            Cow::Borrowed("Open Workspace")
        ));

        let unicode =
            "\u{30ef}\u{30fc}\u{30af}\u{30b9}\u{30da}\u{30fc}\u{30b9}\u{3092}\u{958b}\u{304f}";
        match sanitize_keybinding_label_cow(unicode) {
            Cow::Borrowed(label) => assert_eq!(label, unicode),
            Cow::Owned(label) => panic!("expected borrowed command label, got {label:?}"),
        }

        let mut label = unicode.to_owned();
        let ptr = label.as_ptr();
        sanitize_keybinding_label_in_place(&mut label);
        assert_eq!(label, unicode);
        assert_eq!(label.as_ptr(), ptr);
    }

    #[test]
    fn keybinding_label_cow_owns_dirty_truncated_and_fallback_labels() {
        let dirty = sanitize_keybinding_label_cow(" Open\nWorkspace\u{202e} ");
        assert!(matches!(&dirty, Cow::Owned(_)));
        assert_eq!(dirty.as_ref(), "Open Workspace");

        let long = "x".repeat(KEYBINDING_TEXT_MAX_CHARS + 24);
        let truncated = sanitize_keybinding_label_cow(&long);
        assert!(matches!(&truncated, Cow::Owned(_)));
        assert_eq!(truncated.chars().count(), KEYBINDING_TEXT_MAX_CHARS);
        assert!(truncated.contains("..."));

        let fallback = sanitize_keybinding_label_cow("\n\t");
        assert!(matches!(&fallback, Cow::Owned(_)));
        assert_eq!(fallback.as_ref(), "Unnamed command");
    }

    #[test]
    fn keybinding_display_text_preserves_empty_and_names_invalid_values() {
        assert_eq!(sanitize_keybinding_chord(""), "");
        assert_eq!(sanitize_keybinding_chord("\n\t"), "Invalid shortcut");
        assert_eq!(sanitize_keybinding_label("\n\t"), "Unnamed command");
    }

    #[test]
    fn sanitized_keybinding_items_preserve_raw_command_for_actions() {
        let command = Command::OpenFile(PathBuf::from("raw\npath.rs"));
        let item = sanitized_keybinding_item((
            " control + p ".to_owned(),
            command.clone(),
            format!(
                "Open\n\u{202e}{}",
                "Workspace".repeat(KEYBINDING_TEXT_MAX_CHARS)
            ),
        ));

        assert_eq!(item.command, command);
        assert_eq!(item.chord, "Ctrl+P");
        assert!(!item.label.contains('\n'));
        assert!(!item.label.contains('\u{202e}'));
        assert!(item.search_text.contains("Ctrl+P"));
        assert!(item.search_text.contains("Ctrl P"));
        assert!(item.search_text.contains("Open Workspace"));
        let terms = keybinding_query_terms("ctrl workspace");
        assert!(keybinding_search_text_matches_terms(
            &item.search_text,
            terms.as_slice()
        ));
    }

    #[test]
    fn keybindings_panel_actions_drop_stale_or_unbound_commands() {
        let items = vec![
            keybinding_item("", Command::ToggleQuickOpen),
            keybinding_item("Ctrl+`", Command::ToggleTerminal),
        ];
        let mut actions = PendingKeybindingsPanelActions {
            start_capture: Some(Command::ToggleCommandPalette),
            remove_binding: Some(Command::ToggleQuickOpen),
            ..PendingKeybindingsPanelActions::default()
        };

        guard_keybindings_panel_actions(&mut actions, &items);

        assert_eq!(actions.start_capture, None);
        assert_eq!(actions.remove_binding, None);

        let mut actions = PendingKeybindingsPanelActions {
            start_capture: Some(Command::ToggleQuickOpen),
            remove_binding: Some(Command::ToggleTerminal),
            ..PendingKeybindingsPanelActions::default()
        };

        guard_keybindings_panel_actions(&mut actions, &items);

        assert_eq!(actions.start_capture, Some(Command::ToggleQuickOpen));
        assert_eq!(actions.remove_binding, Some(Command::ToggleTerminal));
    }

    #[test]
    fn keybindings_panel_items_cache_reuses_and_invalidates_filtered_items() {
        let bindings = vec![KeyBinding {
            chord: "Ctrl+P".to_owned(),
            command: Command::ToggleQuickOpen,
        }];
        let mut cache = KeybindingsPanelItemsCache::default();

        let first = cache.items_for(&bindings, "quick");
        let second = cache.items_for(&bindings, "quick");
        assert!(Arc::ptr_eq(&first, &second));
        assert!(cache.matches(&bindings, "quick"));
        assert!(
            first
                .iter()
                .any(|item| item.chord == "Ctrl+P" && item.command == Command::ToggleQuickOpen)
        );

        let sanitized_items = Arc::clone(&cache.sanitized_items);
        let changed_query = cache.items_for(&bindings, "terminal");
        assert!(!Arc::ptr_eq(&first, &changed_query));
        assert!(Arc::ptr_eq(&sanitized_items, &cache.sanitized_items));
        assert!(cache.matches(&bindings, "terminal"));

        let changed_bindings = vec![KeyBinding {
            chord: "Ctrl+Shift+P".to_owned(),
            command: Command::ToggleQuickOpen,
        }];
        let changed_keymap = cache.items_for(&changed_bindings, "quick");
        assert!(!Arc::ptr_eq(&changed_query, &changed_keymap));
        assert!(!Arc::ptr_eq(&sanitized_items, &cache.sanitized_items));
        assert!(
            changed_keymap.iter().any(
                |item| item.chord == "Ctrl+Shift+P" && item.command == Command::ToggleQuickOpen
            )
        );
    }

    #[test]
    fn keybindings_panel_items_cache_reuses_sanitized_items_for_empty_query() {
        let bindings = vec![KeyBinding {
            chord: " control + p ".to_owned(),
            command: Command::ToggleQuickOpen,
        }];
        let mut cache = KeybindingsPanelItemsCache::default();

        let items = cache.items_for(&bindings, "");

        assert!(Arc::ptr_eq(&items, &cache.sanitized_items));
        assert!(
            items
                .iter()
                .any(|item| item.chord == "Ctrl+P" && item.command == Command::ToggleQuickOpen)
        );
    }

    #[test]
    fn keybindings_panel_items_cache_reuses_filtered_source_when_refinement_keeps_all_rows() {
        let mut item = keybinding_item("Ctrl+P", Command::ToggleQuickOpen);
        item.search_text = "Command extra".to_owned();
        let source = Arc::new(vec![item]);
        let mut cache = KeybindingsPanelItemsCache {
            bindings_valid: true,
            filtered_valid: true,
            query: "command".to_owned(),
            bindings: Vec::new(),
            sanitized_items: Arc::clone(&source),
            items: Arc::clone(&source),
        };

        let refined = cache.items_for(&[], "command extra");

        assert!(Arc::ptr_eq(&source, &refined));
        assert!(cache.matches(&[], "command extra"));
    }

    #[test]
    fn keybindings_panel_items_cache_refines_extended_queries_from_previous_results() {
        let mut cache = KeybindingsPanelItemsCache::default();
        let bindings = Vec::new();

        let terminal_items = cache.items_for(&bindings, "terminal");
        assert!(cache.can_refine_previous_filter("terminal search"));
        assert_eq!(
            cache.filter_source_for("terminal search").len(),
            terminal_items.len()
        );
        assert!(terminal_items.len() < cache.sanitized_items.len());

        let refined_items = cache.items_for(&bindings, "terminal search");
        let fresh_items = filter_keybinding_items(&cache.sanitized_items, "terminal search");
        assert_eq!(&*refined_items, &fresh_items);

        assert_eq!(
            cache.filter_source_for("quick").len(),
            cache.sanitized_items.len()
        );
    }

    #[test]
    fn keybindings_panel_filter_uses_cached_shortcut_and_alias_search_text() {
        let bindings = vec![KeyBinding {
            chord: "Ctrl+Alt+K".to_owned(),
            command: Command::ToggleKeybindingsPanel,
        }];
        let mut cache = KeybindingsPanelItemsCache::default();

        let shortcut_words = cache.items_for(&bindings, "ctrl k");
        assert!(shortcut_words.iter().any(|item| {
            item.command == Command::ToggleKeybindingsPanel && item.chord == "Ctrl+Alt+K"
        }));

        let alias_words = cache.items_for(&bindings, "key binds");
        assert!(
            alias_words
                .iter()
                .any(|item| item.command == Command::ToggleKeybindingsPanel)
        );
        assert!(alias_words.iter().all(|item| !item.search_text.is_empty()));
    }

    #[test]
    fn keybindings_panel_query_terms_use_inline_storage_for_typical_queries() {
        let terms = keybinding_query_terms(" ctrl  quick open ");

        assert_eq!(terms.as_slice(), &["ctrl", "quick", "open"]);
        assert!(matches!(terms, KeybindingQueryTerms::Inline { len: 3, .. }));

        let long_query = (0..=KEYBINDING_INLINE_QUERY_TERMS)
            .map(|index| format!("term{index}"))
            .collect::<Vec<_>>()
            .join(" ");
        let terms = keybinding_query_terms(&long_query);

        assert_eq!(terms.as_slice().len(), KEYBINDING_INLINE_QUERY_TERMS + 1);
        assert!(matches!(terms, KeybindingQueryTerms::Heap(_)));
    }

    #[test]
    fn keybindings_panel_filter_reuses_split_query_terms_for_matching() {
        let terms = ["ctrl", "open"];

        assert!(keybinding_search_text_matches_terms(
            "Ctrl+P Ctrl P Quick Open",
            &terms
        ));
        assert!(!keybinding_search_text_matches_terms(
            "Ctrl+P Ctrl P Toggle Terminal",
            &terms
        ));
    }

    fn keybinding_item(chord: &str, command: Command) -> super::KeybindingPanelItem {
        super::KeybindingPanelItem {
            chord: chord.to_owned(),
            command,
            label: "Command".to_owned(),
            search_text: "Command".to_owned(),
        }
    }
}
