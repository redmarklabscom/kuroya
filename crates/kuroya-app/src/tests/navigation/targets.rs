use crate::{
    history::NavigationLocation,
    navigation_targets::{
        NAVIGATION_STATUS_MAX_CHARS, NAVIGATION_TARGET_LABEL_MAX_CHARS, diff_hunk_header_lines,
        diff_hunk_header_lines_for_buffer, diff_hunk_index_at_buffer_line, diff_hunk_index_at_line,
        navigation_location_label, navigation_path_label, navigation_status_text,
        navigation_target_label, next_changed_line, next_changed_line_kind, next_diagnostic_index,
        normalize_changed_line_kinds_for_buffer, normalize_changed_lines_for_buffer,
    },
};
use kuroya_core::{Diagnostic, DiagnosticSeverity, GitLineChangeKind, TextBuffer};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

#[test]
fn navigation_location_labels_are_single_line_and_bounded() {
    let long_name = format!(
        "{}\n\u{202e}tail.rs",
        "a".repeat(NAVIGATION_TARGET_LABEL_MAX_CHARS * 2)
    );
    let location = NavigationLocation::new(PathBuf::from("workspace").join(long_name), 42, 7);
    let label = navigation_location_label(&location);

    assert!(label.ends_with(":42:7"));
    assert!(label.chars().count() <= NAVIGATION_TARGET_LABEL_MAX_CHARS);
    assert!(label.contains("..."));
    assert!(!label.contains('\n'));
    assert!(!label.contains('\u{202e}'));
}

#[test]
fn navigation_target_labels_sanitize_controls_bidi_and_blank_values() {
    let label = navigation_target_label("alpha\tbeta\u{2028}gamma\u{202e}");

    assert_eq!(label, "alpha beta gamma");
    assert_eq!(navigation_target_label("\n\u{202e}\t"), ".");
    assert_eq!(navigation_path_label(std::path::Path::new("")), ".");
}

#[test]
fn navigation_status_text_is_single_line_and_bounded() {
    assert_eq!(
        navigation_status_text("Next diff hunk at src/main.rs:12"),
        "Next diff hunk at src/main.rs:12"
    );

    let status = navigation_status_text(format!(
        "Next diff hunk at {}\n\u{202e}:12",
        "x".repeat(NAVIGATION_STATUS_MAX_CHARS * 2)
    ));

    assert!(status.chars().count() <= NAVIGATION_STATUS_MAX_CHARS);
    assert!(status.ends_with("..."));
    assert!(!status.contains('\n'));
    assert!(!status.contains('\u{202e}'));
}

#[test]
fn diagnostic_navigation_wraps_around_anchor() {
    let diagnostics = vec![
        Diagnostic {
            path: PathBuf::from("a.rs"),
            line: 2,
            column: 1,
            char_range: 0..0,
            severity: DiagnosticSeverity::Warning,
            source: "test".to_owned(),
            message: "a".to_owned(),
            unused: false,
            deprecated: false,
        },
        Diagnostic {
            path: PathBuf::from("a.rs"),
            line: 5,
            column: 3,
            char_range: 0..0,
            severity: DiagnosticSeverity::Error,
            source: "test".to_owned(),
            message: "b".to_owned(),
            unused: false,
            deprecated: false,
        },
        Diagnostic {
            path: PathBuf::from("b.rs"),
            line: 1,
            column: 1,
            char_range: 0..0,
            severity: DiagnosticSeverity::Info,
            source: "test".to_owned(),
            message: "c".to_owned(),
            unused: false,
            deprecated: false,
        },
    ];

    assert_eq!(
        next_diagnostic_index(&diagnostics, &PathBuf::from("a.rs"), 2, 1, 1),
        1
    );
    assert_eq!(
        next_diagnostic_index(&diagnostics, &PathBuf::from("a.rs"), 5, 3, 1),
        2
    );
    assert_eq!(
        next_diagnostic_index(&diagnostics, &PathBuf::from("b.rs"), 1, 1, 1),
        0
    );
    assert_eq!(
        next_diagnostic_index(&diagnostics, &PathBuf::from("a.rs"), 5, 3, -1),
        0
    );
    assert_eq!(
        next_diagnostic_index(&diagnostics, &PathBuf::from("a.rs"), 2, 1, -1),
        2
    );

    let borrowed = diagnostics.iter().collect::<Vec<_>>();
    assert_eq!(
        next_diagnostic_index(&borrowed, &PathBuf::from("a.rs"), 2, 1, 1),
        1
    );
}

#[test]
fn git_change_navigation_wraps_around_anchor() {
    let changed_lines = BTreeSet::from([2, 5, 9]);

    assert_eq!(next_changed_line(&changed_lines, 1, 1), Some(2));
    assert_eq!(next_changed_line(&changed_lines, 2, 1), Some(5));
    assert_eq!(next_changed_line(&changed_lines, 9, 1), Some(2));
    assert_eq!(next_changed_line(&changed_lines, 5, -1), Some(2));
    assert_eq!(next_changed_line(&changed_lines, 2, -1), Some(9));
    assert_eq!(next_changed_line(&changed_lines, 1, -1), Some(9));
    assert_eq!(next_changed_line(&BTreeSet::new(), 1, 1), None);
}

#[test]
fn git_change_kind_navigation_wraps_around_anchor() {
    let changed_lines = BTreeMap::from([
        (2, GitLineChangeKind::Modified),
        (5, GitLineChangeKind::Added),
        (9, GitLineChangeKind::Deleted),
    ]);

    assert_eq!(next_changed_line_kind(&changed_lines, 1, 1), Some(2));
    assert_eq!(next_changed_line_kind(&changed_lines, 2, 1), Some(5));
    assert_eq!(next_changed_line_kind(&changed_lines, 9, 1), Some(2));
    assert_eq!(next_changed_line_kind(&changed_lines, 5, -1), Some(2));
    assert_eq!(next_changed_line_kind(&changed_lines, 2, -1), Some(9));
    assert_eq!(next_changed_line_kind(&changed_lines, 1, -1), Some(9));
    assert_eq!(next_changed_line_kind(&BTreeMap::new(), 1, 1), None);
}

#[test]
fn git_change_lines_clamp_to_visible_buffer_lines() {
    assert_eq!(
        normalize_changed_lines_for_buffer(BTreeSet::from([0, 2, 9, 12]), 10),
        BTreeSet::from([2, 9, 10])
    );
    assert_eq!(
        normalize_changed_lines_for_buffer(BTreeSet::from([3]), 0),
        BTreeSet::from([1])
    );
}

#[test]
fn git_change_line_kinds_clamp_to_visible_buffer_lines() {
    assert_eq!(
        normalize_changed_line_kinds_for_buffer(
            BTreeMap::from([
                (0, GitLineChangeKind::Added),
                (2, GitLineChangeKind::Modified),
                (12, GitLineChangeKind::Deleted),
            ]),
            10,
        ),
        BTreeMap::from([
            (2, GitLineChangeKind::Modified),
            (10, GitLineChangeKind::Deleted),
        ])
    );
}

#[test]
fn diff_hunk_header_lines_identify_unified_diff_sections() {
    let diff = "\
diff --git a/src/main.rs b/src/main.rs
index 0000000..1111111 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,4 @@
 context
-old
+new
@@@ -10,4 -12,4 +14,5 @@@
+combined
";

    assert_eq!(diff_hunk_header_lines(diff), BTreeSet::from([5, 9]));
    assert_eq!(
        next_changed_line(&diff_hunk_header_lines(diff), 5, 1),
        Some(9)
    );
    assert_eq!(
        next_changed_line(&diff_hunk_header_lines(diff), 5, -1),
        Some(9)
    );
}

#[test]
fn diff_hunk_header_lines_scan_buffer_prefixes() {
    let buffer = TextBuffer::from_text(
        1,
        None,
        "diff --git a/main.rs b/main.rs\n@@ -1 +1 @@\n-foo\n+bar\n@@@ -4,1 -4,1 +4,2 @@@\n+long\n"
            .to_owned(),
    );

    assert_eq!(
        diff_hunk_header_lines_for_buffer(&buffer),
        BTreeSet::from([2, 5])
    );
}

#[test]
fn diff_hunk_index_tracks_current_unified_diff_hunk() {
    let diff = "\
diff --git a/src/main.rs b/src/main.rs
index 0000000..1111111 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,4 @@
 context
-old
+new
@@ -10,2 +11,3 @@
+another
";

    assert_eq!(diff_hunk_index_at_line(diff, 1), None);
    assert_eq!(diff_hunk_index_at_line(diff, 5), Some(0));
    assert_eq!(diff_hunk_index_at_line(diff, 8), Some(0));
    assert_eq!(diff_hunk_index_at_line(diff, 9), Some(1));
    assert_eq!(diff_hunk_index_at_line(diff, 10), Some(1));

    let buffer = TextBuffer::from_text(1, None, diff.to_owned());
    assert_eq!(diff_hunk_index_at_buffer_line(&buffer, 1), None);
    assert_eq!(diff_hunk_index_at_buffer_line(&buffer, 5), Some(0));
    assert_eq!(diff_hunk_index_at_buffer_line(&buffer, 8), Some(0));
    assert_eq!(diff_hunk_index_at_buffer_line(&buffer, 9), Some(1));
    assert_eq!(diff_hunk_index_at_buffer_line(&buffer, 10), Some(1));
}
