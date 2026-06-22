use super::super::KeybindingPanelItem;
use super::{
    KEYBINDING_ROW_CAPTURE_HINT, KEYBINDING_ROW_CHORD_LIMIT, KEYBINDING_ROW_LABEL_LIMIT,
    PreparedKeybindingRowsCache, keybinding_empty_state_label, keybinding_empty_state_query_cow,
    keybinding_row_display_chord, keybinding_row_display_chord_cow, keybinding_row_display_label,
    keybinding_row_display_label_cow, keybinding_row_widget_info, prepare_keybinding_row,
    row_command_matches_item, write_keybinding_row_tooltip,
};
use eframe::egui::WidgetType;
use kuroya_core::Command;
use std::borrow::Cow;
use std::sync::Arc;

fn row_tooltip(label: &str, chord: &str) -> String {
    prepared_row(label, chord).tooltip()
}

fn row_accessibility_label(label: &str, chord: &str, row_index: usize, row_count: usize) -> String {
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

    assert_eq!(
        tooltip,
        format!("prefix:Quick Open\nShortcut: Unassigned\n{KEYBINDING_ROW_CAPTURE_HINT}")
    );
    assert_eq!(
        row.tooltip(),
        format!("Quick Open\nShortcut: Unassigned\n{KEYBINDING_ROW_CAPTURE_HINT}")
    );
}

#[test]
fn keybinding_row_tooltip_names_shortcut_or_unassigned_state() {
    assert_eq!(
        row_tooltip("Quick Open", "Ctrl+P"),
        format!("Quick Open\nShortcut: Ctrl+P\n{KEYBINDING_ROW_CAPTURE_HINT}")
    );
    assert_eq!(
        row_tooltip("Quick Open", ""),
        format!("Quick Open\nShortcut: Unassigned\n{KEYBINDING_ROW_CAPTURE_HINT}")
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
            <= KEYBINDING_ROW_LABEL_LIMIT
                + "\nShortcut: ".len()
                + KEYBINDING_ROW_CHORD_LIMIT
                + "\n".len()
                + KEYBINDING_ROW_CAPTURE_HINT.len()
    );
}

#[test]
fn keybinding_row_tooltip_falls_back_for_blank_display_text() {
    assert_eq!(
        row_tooltip("\n\t\u{202e}", "\n\t\u{202e}"),
        format!("Unnamed command\nShortcut: Invalid shortcut\n{KEYBINDING_ROW_CAPTURE_HINT}")
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
    assert_eq!(
        row.tooltip(),
        format!("Quick Open\nShortcut: Ctrl+P\n{KEYBINDING_ROW_CAPTURE_HINT}")
    );
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
    assert_eq!(
        first[0].tooltip(),
        format!("Quick Open\nShortcut: Ctrl+P\n{KEYBINDING_ROW_CAPTURE_HINT}")
    );
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
