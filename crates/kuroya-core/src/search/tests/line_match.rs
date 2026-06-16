use super::*;
use std::cell::Cell;

#[test]
fn line_match_scanner_can_stop_without_collecting_remaining_matches() {
    let mut matches = Vec::new();
    let completed =
        for_each_line_match("needle needle needle", "needle", true, false, |byte_col| {
            matches.push(byte_col);
            false
        });

    assert!(!completed);
    assert_eq!(matches, vec![0]);
}

#[test]
fn line_match_scanner_matches_ascii_case_insensitively_without_copying_line() {
    let mut matches = Vec::new();
    let completed = for_each_line_match("Alpha alpha ALPHA", "alpha", false, false, |byte_col| {
        matches.push(byte_col);
        true
    });

    assert!(completed);
    assert_eq!(matches, vec![0, 6, 12]);
}

#[test]
fn line_match_scanner_preserves_ascii_only_case_folding() {
    let mut matches = Vec::new();
    let completed = for_each_line_match("über Über", "über", false, false, |byte_col| {
        matches.push(byte_col);
        true
    });

    assert!(completed);
    assert_eq!(matches, vec![0]);
}

#[test]
fn line_match_scanner_whole_word_ascii_boundaries_match_unicode_path() {
    let mut matches = Vec::new();
    let completed = for_each_line_match(
        "alpha alphabet alpha_1 betaalpha alpha-alpha",
        "alpha",
        false,
        true,
        |byte_col| {
            matches.push(byte_col);
            true
        },
    );

    assert!(completed);
    assert_eq!(matches, vec![0, 33, 39]);
}

#[test]
fn line_match_scanner_preserves_order_across_cancel_checkpoints() {
    let prefix = "a".repeat(SEARCH_CANCEL_BYTE_INTERVAL - 3);
    let line = format!("{prefix}needle needle");
    let needle = LineSearchNeedle::new("needle", true);
    let mut cancellation_checks = 0usize;
    let mut matches = Vec::new();

    let scan = for_each_line_match_with_cancel(
        &line,
        &needle,
        false,
        || {
            cancellation_checks = cancellation_checks.saturating_add(1);
            false
        },
        |byte_col| {
            matches.push(byte_col);
            true
        },
    );

    assert_eq!(scan, LineMatchScan::Completed);
    assert_eq!(
        matches,
        vec![
            SEARCH_CANCEL_BYTE_INTERVAL - 3,
            SEARCH_CANCEL_BYTE_INTERVAL + 4
        ]
    );
    assert!(cancellation_checks > 0);
}

#[test]
fn line_match_scanner_checks_cancellation_by_bytes_for_large_needles() {
    let needle_text = "n".repeat(SEARCH_CANCEL_BYTE_INTERVAL * 2);
    let line = "h".repeat(SEARCH_CANCEL_BYTE_INTERVAL * 3);
    let needle = LineSearchNeedle::new(&needle_text, true);
    let cancellation_checks = Cell::new(0usize);

    let scan = for_each_line_match_with_cancel(
        &line,
        &needle,
        false,
        || {
            cancellation_checks.set(cancellation_checks.get().saturating_add(1));
            true
        },
        |_| true,
    );

    assert_eq!(scan, LineMatchScan::Cancelled);
    assert_eq!(cancellation_checks.get(), 1);
}

#[test]
fn line_match_scanner_preserves_progress_after_large_needle_matches() {
    let needle_text = "n".repeat(SEARCH_CANCEL_BYTE_INTERVAL * 2);
    let line = format!("{needle_text}{needle_text}");
    let needle = LineSearchNeedle::new(&needle_text, true);
    let mut matches = Vec::new();

    let scan = for_each_line_match_with_cancel(
        &line,
        &needle,
        false,
        || false,
        |byte_col| {
            matches.push(byte_col);
            true
        },
    );

    assert_eq!(scan, LineMatchScan::Completed);
    assert_eq!(matches, vec![0, needle_text.len()]);
}

#[test]
fn line_search_needle_find_next_clamps_case_sensitive_offsets() {
    let needle = LineSearchNeedle::new("needle", true);

    assert_eq!(needle.find_next("\u{00e9}needle", 1), Some(2));
    assert_eq!(needle.find_next("needle", usize::MAX), None);
}
