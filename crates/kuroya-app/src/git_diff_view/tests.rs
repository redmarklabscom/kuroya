use super::{
    SOURCE_CONTROL_DIFF_STATUS_MAX_CHARS, accessible_diff_label, accessible_diff_label_cow,
    diff_buffer_display_label, diff_buffer_line_is_hunk_header, diff_buffer_line_starts_with,
    diff_hunk_header_line, hunk_header_line_in_diff_buffer,
    hunk_modified_start_line_in_diff_buffer, hunk_modified_start_line_in_unified_diff,
    hunk_original_start_line_in_diff_buffer, hunk_original_start_line_in_unified_diff,
    hunk_patch_from_diff_buffer, hunk_patch_from_unified_diff, hunk_start_lines_in_unified_diff,
    source_control_diff_display_label, source_control_diff_display_label_cow,
    source_control_diff_hunk_identity_stale_status, source_control_diff_text_source_for_status,
    source_control_patch_text_source_for_status,
};
use crate::{
    path_display::DISPLAY_PATH_LABEL_MAX_CHARS, source_control_diff_runtime::SourceControlDiffText,
    source_control_patch_runtime::SourceControlPatchText,
};
use kuroya_core::{GitFileStatus, TextBuffer};
use std::{borrow::Cow, path::PathBuf};

fn sample_diff() -> String {
    "\
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
diff --git a/src/lib.rs b/src/lib.rs
@@ -20,1 +30,2 @@
-lib
+library
"
    .to_owned()
}

#[test]
fn hunk_patch_from_diff_buffer_matches_string_patch_without_full_buffer_clone() {
    let diff = sample_diff();
    let buffer = TextBuffer::from_text(1, None, diff.clone());

    assert_eq!(
        hunk_patch_from_diff_buffer(&buffer, 1),
        hunk_patch_from_unified_diff(&diff, 1)
    );
    assert_eq!(
        hunk_patch_from_diff_buffer(&buffer, 2),
        hunk_patch_from_unified_diff(&diff, 2)
    );
}

#[test]
fn hunk_metadata_reads_from_diff_buffer_lines() {
    let buffer = TextBuffer::from_text(1, None, sample_diff());

    assert_eq!(hunk_header_line_in_diff_buffer(&buffer, 0), Some(5));
    assert_eq!(hunk_header_line_in_diff_buffer(&buffer, 1), Some(9));
    assert_eq!(
        hunk_original_start_line_in_diff_buffer(&buffer, 1),
        Some(10)
    );
    assert_eq!(
        hunk_modified_start_line_in_diff_buffer(&buffer, 1),
        Some(11)
    );
    assert_eq!(
        hunk_start_lines_in_unified_diff(&sample_diff(), 1),
        Some((10, 11))
    );
    assert_eq!(hunk_start_lines_in_unified_diff(&sample_diff(), 99), None);
}

#[test]
fn diff_buffer_prefix_scans_skip_long_non_hunk_lines() {
    let diff = format!(
        "{}\ndiff --git a/src/main.rs b/src/main.rs\n@@ -1,1 +1,1 @@\n-old\n+new\n",
        "body".repeat(2000)
    );
    let buffer = TextBuffer::from_text(1, None, diff);

    assert!(!diff_buffer_line_is_hunk_header(&buffer, 0));
    assert!(diff_buffer_line_starts_with(&buffer, 1, "diff --git "));
    assert_eq!(hunk_header_line_in_diff_buffer(&buffer, 0), Some(3));
}

#[test]
fn diff_hunk_header_line_rejects_malformed_hunk_like_lines() {
    assert!(diff_hunk_header_line("@@ -1,2 +3 @@"));
    assert!(diff_hunk_header_line("@@@ -1,2 -4,5 +7,8 @@@"));
    assert_eq!(
        hunk_original_start_line_in_unified_diff("@@@ -1,2 -4,5 +7,8 @@@\n", 0),
        Some(4)
    );
    assert_eq!(
        hunk_modified_start_line_in_unified_diff("@@@ -1,2 -4,5 +7,8 @@@\n", 0),
        Some(7)
    );
    assert!(!diff_hunk_header_line("@@ not a hunk @@"));
    assert!(!diff_hunk_header_line("@@ -1,2 +3,4"));
    assert!(!diff_hunk_header_line("@@ -1,2 +3,4 @@@"));
    assert!(!diff_hunk_header_line("@@ -1, +3,4 @@"));
}

#[test]
fn diff_hunk_header_line_rejects_invalid_ranges() {
    assert!(diff_hunk_header_line("@@ -0,0 +1,1 @@"));
    assert_eq!(
        hunk_original_start_line_in_unified_diff("@@ -0,0 +1,1 @@\n", 0),
        Some(1)
    );

    assert!(!diff_hunk_header_line("@@ -0,1 +1,1 @@"));
    assert!(!diff_hunk_header_line("@@ -1,1 +0,1 @@"));
    assert!(!diff_hunk_header_line("@@ -0 +1,1 @@"));
    assert!(!diff_hunk_header_line("@@ -0,0 +0,0 @@"));
    assert!(!diff_hunk_header_line(
        "@@ -1,999999999999999999999999 +1,1 @@"
    ));

    let diff = "\
diff --git a/src/main.rs b/src/main.rs
@@ -0,1 +1,1 @@
ignored
@@ -2,1 +2,1 @@
-old
+new
";
    let buffer = TextBuffer::from_text(1, None, diff.to_owned());

    assert_eq!(hunk_header_line_in_diff_buffer(&buffer, 0), Some(4));
    assert_eq!(hunk_start_lines_in_unified_diff(diff, 0), Some((2, 2)));
}

#[test]
fn hunk_helpers_ignore_malformed_hunk_like_lines() {
    let diff = "\
diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,1 +1,1 @@
-old
+new
@@ not a hunk @@
still malformed patch content
@@ -9,1 +10,1 @@
-again
+again-new
";
    let buffer = TextBuffer::from_text(1, None, diff.to_owned());

    assert_eq!(hunk_header_line_in_diff_buffer(&buffer, 0), Some(4));
    assert_eq!(hunk_header_line_in_diff_buffer(&buffer, 1), Some(9));
    assert_eq!(hunk_original_start_line_in_unified_diff(diff, 1), Some(9));
    assert_eq!(hunk_modified_start_line_in_unified_diff(diff, 1), Some(10));
    assert_eq!(
        hunk_patch_from_unified_diff(diff, 1).as_deref(),
        Some(concat!(
            "diff --git a/src/main.rs b/src/main.rs\n",
            "--- a/src/main.rs\n",
            "+++ b/src/main.rs\n",
            "@@ -9,1 +10,1 @@\n",
            "-again\n",
            "+again-new\n",
        ))
    );
}

#[test]
fn source_control_diff_display_label_cow_borrows_clean_ascii_and_unicode_labels() {
    let ascii = "src/main.rs";
    assert!(matches!(
        source_control_diff_display_label_cow(ascii),
        Cow::Borrowed(label) if label == ascii
    ));

    let unicode = "clean-\u{03bb}.rs";
    assert!(matches!(
        source_control_diff_display_label_cow(unicode),
        Cow::Borrowed(label) if label == unicode
    ));
}

#[test]
fn source_control_diff_display_label_cow_owns_dirty_truncated_and_fallback_labels() {
    let dirty = source_control_diff_display_label_cow("alpha\nbeta");
    assert_eq!(dirty.as_ref(), "alpha beta");
    assert!(matches!(dirty, Cow::Owned(_)));

    let bidi = source_control_diff_display_label_cow("alpha\u{202e}beta");
    assert_eq!(bidi.as_ref(), "alphabeta");
    assert!(matches!(bidi, Cow::Owned(_)));

    let long = format!(
        "start-{}-finish",
        "x".repeat(DISPLAY_PATH_LABEL_MAX_CHARS * 2)
    );
    let truncated = source_control_diff_display_label_cow(&long);
    assert!(truncated.contains("..."));
    assert!(truncated.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);
    assert!(matches!(truncated, Cow::Owned(_)));

    let fallback = source_control_diff_display_label_cow("\n\u{202e}");
    assert_eq!(fallback.as_ref(), "diff");
    assert!(matches!(fallback, Cow::Owned(_)));
}

#[test]
fn source_control_diff_display_label_string_wrapper_matches_cow_helper() {
    let cases = [
        "src/main.rs".to_owned(),
        "clean-\u{03bb}.rs".to_owned(),
        "alpha\nbeta".to_owned(),
        "\n\u{202e}".to_owned(),
        format!(
            "start-{}-finish",
            "x".repeat(DISPLAY_PATH_LABEL_MAX_CHARS * 2)
        ),
    ];

    for value in cases {
        assert_eq!(
            source_control_diff_display_label(&value),
            source_control_diff_display_label_cow(&value).as_ref()
        );
    }
}

#[test]
fn diff_buffer_display_label_reuses_owned_clean_labels() {
    let label = "clean-owned-label.rs".to_owned();
    let ptr = label.as_ptr();
    let len = label.len();
    let display = diff_buffer_display_label(label, false);

    assert_eq!(display, "clean-owned-label.rs");
    assert_eq!(display.as_ptr(), ptr);
    assert_eq!(display.len(), len);

    let accessible = "clean-owned-label.rs (Accessible Diff)".to_owned();
    let ptr = accessible.as_ptr();
    let len = accessible.len();
    let display = diff_buffer_display_label(accessible, true);

    assert_eq!(display, "clean-owned-label.rs (Accessible Diff)");
    assert_eq!(display.as_ptr(), ptr);
    assert_eq!(display.len(), len);
}

#[test]
fn accessible_diff_label_preserves_existing_suffix() {
    let suffixed = "clean.rs (Accessible Diff)";
    assert_eq!(accessible_diff_label(suffixed), suffixed);
    assert_eq!(
        accessible_diff_label(" clean.rs (Accessible Diff) "),
        suffixed
    );
    assert_eq!(accessible_diff_label("clean.rs"), suffixed);
    assert_eq!(
        accessible_diff_label("clean.rs (Accessible Diff)")
            .matches(" (Accessible Diff)")
            .count(),
        1
    );
    assert!(matches!(
        accessible_diff_label_cow(suffixed),
        Cow::Borrowed(label) if label == suffixed
    ));
}

#[test]
fn source_control_diff_display_label_remains_safe_and_bounded() {
    let raw_label = format!("bad\n{}\u{202e}tail.rs", "very-long-component-".repeat(24));
    let label = source_control_diff_display_label(&raw_label);

    assert!(!label.contains('\n'));
    assert!(!label.contains('\u{202e}'));
    assert!(label.contains("..."));
    assert!(label.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);
    assert_eq!(source_control_diff_display_label("\n\u{202e}"), "diff");
}

#[test]
fn source_control_diff_status_and_accessible_labels_are_bounded() {
    let path = PathBuf::from(format!(
        "workspace/src/bad\n{}\u{202e}tail.rs",
        "very-long-component-".repeat(24)
    ));
    let status = source_control_diff_hunk_identity_stale_status(
        &format!("action-{}", "long-".repeat(64)),
        &path,
        usize::MAX,
    );
    let label = accessible_diff_label(&format!("diff-{}tail", "long-".repeat(64)));

    assert!(!status.contains('\n'));
    assert!(!status.contains('\u{202e}'));
    assert!(status.contains("..."));
    assert!(status.chars().count() <= SOURCE_CONTROL_DIFF_STATUS_MAX_CHARS);
    assert!(label.ends_with(" (Accessible Diff)"));
    assert!(label.contains("..."));
    assert!(label.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);
}

#[test]
fn source_control_diff_text_deleted_status_overrides_open_buffer_snapshot() {
    let path = PathBuf::from("deleted.txt");
    let buffer = TextBuffer::from_text(7, Some(path.clone()), "stale buffer\n".to_owned());

    assert!(matches!(
        source_control_diff_text_source_for_status(
            path,
            Some(GitFileStatus::Deleted),
            Some(&buffer),
            1024
        ),
        SourceControlDiffText::Deleted
    ));
}

#[test]
fn source_control_patch_text_deleted_status_overrides_open_buffer_snapshot() {
    let path = PathBuf::from("deleted.txt");
    let buffer = TextBuffer::from_text(7, Some(path.clone()), "stale buffer\n".to_owned());

    assert!(matches!(
        source_control_patch_text_source_for_status(
            &path,
            Some(GitFileStatus::Deleted),
            Some(&buffer),
            1024
        ),
        SourceControlPatchText::Deleted
    ));
}

#[test]
fn source_control_diff_text_uses_open_buffer_when_file_is_not_deleted() {
    let path = PathBuf::from("modified.txt");
    let buffer = TextBuffer::from_text(7, Some(path.clone()), "open buffer\n".to_owned());

    assert!(matches!(
        source_control_diff_text_source_for_status(
            path,
            Some(GitFileStatus::Modified),
            Some(&buffer),
            1024
        ),
        SourceControlDiffText::Snapshot(_)
    ));
}

#[test]
fn source_control_patch_text_uses_open_buffer_when_file_is_not_deleted() {
    let path = PathBuf::from("modified.txt");
    let buffer = TextBuffer::from_text(7, Some(path.clone()), "open buffer\n".to_owned());

    assert!(matches!(
        source_control_patch_text_source_for_status(
            &path,
            Some(GitFileStatus::Modified),
            Some(&buffer),
            1024
        ),
        SourceControlPatchText::Snapshot(_)
    ));
}
