use crate::diagnostics_panel::{
    diagnostic_display_path, diagnostic_panel_normalized_selection, diagnostic_panel_open_target,
    diagnostic_panel_row_label, diagnostic_panel_summary_label,
};
use kuroya_core::{Diagnostic, DiagnosticSet, DiagnosticSeverity};
use std::{ops::Range, path::PathBuf};

fn diagnostic(message: &str, line: usize, column: usize) -> Diagnostic {
    Diagnostic {
        path: PathBuf::from("workspace/src/main.rs"),
        line,
        column,
        char_range: Range { start: 0, end: 1 },
        severity: DiagnosticSeverity::Error,
        source: "rust-analyzer".to_owned(),
        message: message.to_owned(),
        unused: false,
        deprecated: false,
    }
}

fn diagnostic_with_severity(severity: DiagnosticSeverity, message: &str) -> Diagnostic {
    Diagnostic {
        severity,
        message: message.to_owned(),
        ..diagnostic(message, 1, 1)
    }
}

fn diagnostic_with_path(path: PathBuf, message: &str, line: usize, column: usize) -> Diagnostic {
    Diagnostic {
        path,
        line,
        column,
        char_range: Range { start: 0, end: 1 },
        severity: DiagnosticSeverity::Error,
        source: "rust-analyzer".to_owned(),
        message: message.to_owned(),
        unused: false,
        deprecated: false,
    }
}

fn assert_safe_display_label(label: &str) {
    assert!(
        !label
            .chars()
            .any(|ch| ch.is_control() || matches!(ch, '\u{2028}' | '\u{2029}'))
    );
    assert!(!label.chars().any(|ch| matches!(
        ch,
        '\u{061c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
    )));
}

#[test]
fn diagnostics_panel_row_label_summarizes_location_severity_and_message() {
    let diagnostic = diagnostic("first line\nsecond line", 3, 5);

    assert_eq!(
        diagnostic_panel_row_label(&diagnostic),
        "main.rs:3:5  error  first line second line"
    );
}

#[test]
fn diagnostics_panel_row_label_sanitizes_and_bounds_user_visible_text() {
    let path = PathBuf::from(format!(
        "workspace/src/{}tail\u{202e}.rs",
        "very-long\n".repeat(24)
    ));
    let diagnostic = diagnostic_with_path(
        path,
        &format!("{}\n\u{2066}second line", "mismatch ".repeat(40)),
        3,
        5,
    );

    let label = diagnostic_panel_row_label(&diagnostic);

    assert_safe_display_label(&label);
    assert!(label.contains(":3:5  error  "));
    assert!(label.ends_with("..."));
    assert!(label.chars().count() <= 260);
}

#[test]
fn diagnostics_panel_row_label_keeps_sanitized_diagnostic_message_summary() {
    let diagnostic = diagnostic_with_path(
        PathBuf::from("workspace/src/main.rs"),
        "\n  unresolved\tname\u{7}\u{202e}\ntry importing it\u{2066}  ",
        3,
        5,
    );

    assert_eq!(
        diagnostic_panel_row_label(&diagnostic),
        "main.rs:3:5  error  unresolved name try importing it"
    );
}

#[test]
fn diagnostic_display_path_sanitizes_and_bounds_hostile_path_labels() {
    let path = PathBuf::from(format!(
        "workspace/src/{}tail\u{202e}.rs",
        "segment\n\t".repeat(24)
    ));
    let label = diagnostic_display_path(&path);

    assert_safe_display_label(&label);
    assert!(label.contains("..."));
    assert!(label.chars().count() <= 80);
}

#[test]
fn diagnostic_display_path_falls_back_for_blank_control_path_labels() {
    let path = PathBuf::from("\n\t\u{202e}\u{2066}");

    assert_eq!(diagnostic_display_path(&path), ".");
}

#[test]
fn diagnostics_panel_open_target_uses_selected_diagnostic_location() {
    let diagnostic = diagnostic("mismatch", 8, 2);

    assert_eq!(
        diagnostic_panel_open_target(&diagnostic),
        (PathBuf::from("workspace/src/main.rs"), 8, 2)
    );
}

#[test]
fn diagnostics_panel_open_target_preserves_raw_diagnostic_path() {
    let raw_path = PathBuf::from("workspace/src/bad\n\u{202e}.rs");
    let diagnostic = diagnostic_with_path(raw_path.clone(), "mismatch", 0, 0);

    assert_eq!(
        diagnostic_panel_open_target(&diagnostic),
        (raw_path.clone(), 1, 1)
    );
    assert_eq!(diagnostic.path, raw_path);
}

#[test]
fn diagnostics_panel_open_target_clamps_invalid_zero_location() {
    let diagnostic = diagnostic("bad payload", 0, 0);

    assert_eq!(
        diagnostic_panel_open_target(&diagnostic),
        (PathBuf::from("workspace/src/main.rs"), 1, 1)
    );
    assert_eq!(
        diagnostic_panel_row_label(&diagnostic),
        "main.rs:1:1  error  bad payload"
    );
}

#[test]
fn diagnostics_panel_summary_names_all_present_severities() {
    let path = PathBuf::from("workspace/src/main.rs");
    let mut diagnostics = DiagnosticSet::default();
    diagnostics.replace(
        path,
        vec![
            diagnostic_with_severity(DiagnosticSeverity::Error, "error"),
            diagnostic_with_severity(DiagnosticSeverity::Warning, "warning one"),
            diagnostic_with_severity(DiagnosticSeverity::Warning, "warning two"),
            diagnostic_with_severity(DiagnosticSeverity::Info, "info"),
            diagnostic_with_severity(DiagnosticSeverity::Hint, "hint"),
        ],
    );

    assert_eq!(
        diagnostic_panel_summary_label(&diagnostics),
        "1 error, 2 warnings, 1 info, 1 hint"
    );
}

#[test]
fn diagnostics_panel_summary_handles_empty_and_hint_only_states() {
    let mut diagnostics = DiagnosticSet::default();
    assert_eq!(
        diagnostic_panel_summary_label(&diagnostics),
        "No diagnostics"
    );

    diagnostics.replace(
        PathBuf::from("workspace/src/main.rs"),
        vec![diagnostic_with_severity(DiagnosticSeverity::Hint, "hint")],
    );

    assert_eq!(diagnostic_panel_summary_label(&diagnostics), "1 hint");
}

#[test]
fn diagnostics_panel_selection_clamps_and_resets_empty_state() {
    assert_eq!(diagnostic_panel_normalized_selection(8, 3), 2);
    assert_eq!(diagnostic_panel_normalized_selection(2, 3), 2);
    assert_eq!(diagnostic_panel_normalized_selection(8, 0), 0);
}
