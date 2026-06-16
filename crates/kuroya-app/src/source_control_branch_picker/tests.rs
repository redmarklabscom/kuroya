use super::{
    SourceControlBranchPreparedRow, source_control_branch_clamped_index,
    source_control_branch_copy_status, source_control_branch_copy_text,
    source_control_branch_create_tooltip, source_control_branch_display_fragment,
    source_control_branch_display_fragment_cow, source_control_branch_display_name,
    source_control_branch_label, source_control_branch_label_with_suffix,
    source_control_branch_prepared_row_at, source_control_branch_prepared_visible_rows,
    source_control_branch_ref_index_by_name, source_control_branch_rename_blocked_reason,
    source_control_branch_rename_target, source_control_branch_rename_tooltip,
    source_control_branch_selected_identity, source_control_branch_selection_after_reload,
    source_control_branch_status_detail, source_control_branch_tooltip_fragment_cow,
    source_control_branch_visible_row_bounds, source_control_filtered_branch_refs,
    source_control_sorted_branch_refs, source_control_sorted_branches,
};
use kuroya_core::{GitBranch, GitBranchSortOrder, GitCheckoutType};
use std::borrow::Cow;

fn branch(name: &str, is_current: bool, time: i64) -> GitBranch {
    GitBranch {
        name: name.to_owned(),
        is_current,
        kind: GitCheckoutType::Local,
        committer_time_seconds: time,
    }
}

fn branch_with_kind(name: &str, is_current: bool, kind: GitCheckoutType) -> GitBranch {
    GitBranch {
        name: name.to_owned(),
        is_current,
        kind,
        committer_time_seconds: 0,
    }
}

#[test]
fn branch_picker_uses_borrowed_sorted_rows_for_large_branch_lists() {
    let branches = vec![
        branch("feature/old", false, 10),
        branch("main", true, 20),
        branch("feature/new", false, 30),
    ];

    let sorted = source_control_sorted_branch_refs(&branches, GitBranchSortOrder::CommitterDate);

    assert_eq!(
        sorted
            .iter()
            .map(|branch| branch.name.as_str())
            .collect::<Vec<_>>(),
        vec!["main", "feature/new", "feature/old"]
    );
    assert!(std::ptr::eq(sorted[0], &branches[1]));
}

#[test]
fn branch_picker_filters_borrowed_rows_without_changing_match_behavior() {
    let branches = vec![
        branch("main", true, 30),
        branch("feature/search", false, 20),
        branch("feature/find", false, 10),
    ];
    let sorted = source_control_sorted_branch_refs(&branches, GitBranchSortOrder::Alphabetically);

    let filtered = source_control_filtered_branch_refs(&sorted, "SEARCH");

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].name, "feature/search");
    assert!(std::ptr::eq(filtered[0], &branches[1]));
    assert_eq!(
        source_control_sorted_branches(&branches, GitBranchSortOrder::Alphabetically)
            .into_iter()
            .map(|branch| branch.name)
            .collect::<Vec<_>>(),
        vec!["main", "feature/find", "feature/search"]
    );
}

#[test]
fn branch_picker_prepares_only_visible_rows_with_source_indices() {
    let branches = vec![
        branch("alpha", false, 10),
        branch("bravo", false, 20),
        branch("charlie", false, 30),
        branch("delta", false, 40),
    ];
    let sorted = source_control_sorted_branch_refs(&branches, GitBranchSortOrder::Alphabetically);

    let rows = source_control_branch_prepared_visible_rows(&sorted, 1..3, None).collect::<Vec<_>>();

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].row_index(), 1);
    assert_eq!(rows[0].raw_name(), "bravo");
    assert_eq!(rows[1].row_index(), 2);
    assert_eq!(rows[1].raw_name(), "charlie");
}

#[test]
fn branch_picker_prepared_row_helpers_clamp_stale_ranges_without_labels_for_missing_rows() {
    let branches = vec![branch("alpha", false, 10), branch("bravo", false, 20)];
    let sorted = source_control_sorted_branch_refs(&branches, GitBranchSortOrder::Alphabetically);

    let rows = source_control_branch_prepared_visible_rows(&sorted, 1..usize::MAX, None)
        .collect::<Vec<_>>();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].row_index(), 1);
    assert_eq!(rows[0].raw_name(), "bravo");
    assert!(source_control_branch_prepared_row_at(&sorted, 2, None).is_none());
}

#[test]
fn branch_picker_prepared_visible_rows_reject_reversed_and_extreme_ranges() {
    let branches = vec![branch("alpha", false, 10), branch("bravo", false, 20)];
    let sorted = source_control_sorted_branch_refs(&branches, GitBranchSortOrder::Alphabetically);
    let start = 1;
    let end = 0;

    let rows =
        source_control_branch_prepared_visible_rows(&sorted, start..end, None).collect::<Vec<_>>();

    assert!(rows.is_empty());
    assert_eq!(
        source_control_branch_visible_row_bounds(sorted.len(), 0..usize::MAX),
        (0, sorted.len())
    );
}

#[test]
fn branch_picker_visible_rows_iterate_without_intermediate_row_vec() {
    let branches = vec![
        branch("alpha", false, 10),
        branch("bravo", false, 20),
        branch("charlie", false, 30),
        branch("delta", false, 40),
    ];
    let sorted = source_control_sorted_branch_refs(&branches, GitBranchSortOrder::Alphabetically);

    let rows = source_control_branch_prepared_visible_rows(&sorted, 1..3, None);

    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows.map(|row| row.raw_name().to_owned())
            .collect::<Vec<_>>(),
        vec!["bravo".to_owned(), "charlie".to_owned()]
    );
}

#[test]
fn branch_picker_selection_after_reload_preserves_filtered_branch_identity() {
    let branches = vec![
        branch("main", true, 40),
        branch("feature/new", false, 30),
        branch("feature/old", false, 20),
    ];
    let selected = source_control_branch_selected_identity(
        &branches,
        "feature",
        None,
        GitBranchSortOrder::CommitterDate,
        0,
    );

    let reloaded = vec![
        branch("feature/extra", false, 50),
        branch("main", true, 40),
        branch("feature/new", false, 10),
        branch("feature/old", false, 30),
    ];

    assert_eq!(
        source_control_branch_selection_after_reload(
            &reloaded,
            "feature",
            None,
            GitBranchSortOrder::CommitterDate,
            0,
            selected.as_ref().map(|(name, kind)| (name.as_str(), *kind)),
        ),
        2
    );
}

#[test]
fn branch_picker_selected_identity_clamps_stale_selection() {
    let branches = vec![
        branch("main", true, 40),
        branch("feature/new", false, 30),
        branch("feature/old", false, 20),
    ];

    assert_eq!(
        source_control_branch_selected_identity(
            &branches,
            "feature",
            None,
            GitBranchSortOrder::CommitterDate,
            99,
        ),
        Some(("feature/old".to_owned(), GitCheckoutType::Local))
    );
}

#[test]
fn branch_picker_clamped_index_handles_empty_and_out_of_range_selection() {
    assert_eq!(source_control_branch_clamped_index(0, 99), None);
    assert_eq!(source_control_branch_clamped_index(3, 0), Some(0));
    assert_eq!(source_control_branch_clamped_index(3, 99), Some(2));
}

#[test]
fn branch_picker_selection_after_reload_uses_clamped_previous_identity() {
    let branches = vec![
        branch("main", true, 40),
        branch("feature/new", false, 30),
        branch("feature/old", false, 20),
    ];
    let selected = source_control_branch_selected_identity(
        &branches,
        "feature",
        None,
        GitBranchSortOrder::CommitterDate,
        99,
    );

    let reloaded = vec![
        branch("main", true, 40),
        branch("feature/old", false, 50),
        branch("feature/new", false, 10),
    ];

    assert_eq!(
        source_control_branch_selection_after_reload(
            &reloaded,
            "feature",
            None,
            GitBranchSortOrder::CommitterDate,
            99,
            selected.as_ref().map(|(name, kind)| (name.as_str(), *kind)),
        ),
        0
    );
}

#[test]
fn branch_picker_selection_after_reload_clamps_when_branch_disappears() {
    let branches = vec![branch("feature/old", false, 20)];

    assert_eq!(
        source_control_branch_selection_after_reload(
            &branches,
            "feature",
            None,
            GitBranchSortOrder::CommitterDate,
            4,
            Some(("feature/missing", GitCheckoutType::Local)),
        ),
        0
    );
}

#[test]
fn branch_picker_rename_selection_matches_local_identity_for_duplicate_names() {
    let mut tag = branch("release", false, 40);
    tag.kind = GitCheckoutType::Tags;
    let branches = vec![tag, branch("release", false, 10), branch("main", true, 30)];

    assert_eq!(
        source_control_branch_selection_after_reload(
            &branches,
            "release",
            Some("release"),
            GitBranchSortOrder::CommitterDate,
            0,
            None,
        ),
        2
    );

    assert_eq!(
        SourceControlBranchPreparedRow::new(0, &branches[0], Some("release")).label(),
        "release  tag"
    );
    assert_eq!(
        SourceControlBranchPreparedRow::new(1, &branches[1], Some("release")).label(),
        "release  renaming"
    );

    let sorted = source_control_sorted_branch_refs(&branches, GitBranchSortOrder::CommitterDate);
    assert_eq!(
        source_control_branch_ref_index_by_name(&sorted, "release"),
        Some(2)
    );
}

#[test]
fn branch_display_name_sanitizes_controls_bidi_and_bounds_length() {
    let raw = format!("feature/\n{}\u{202e}\u{2066}tail", "branch-".repeat(80));

    let display = source_control_branch_display_name(&raw);

    assert!(display.starts_with("feature/ branch-"));
    assert!(display.contains("..."));
    assert_branch_display_text_is_safe(&display);
    assert!(display.chars().count() <= super::SOURCE_CONTROL_BRANCH_DISPLAY_MAX_CHARS);
}

#[test]
fn branch_status_detail_sanitizes_controls_bidi_and_bounds_length() {
    let raw = format!("first line\r\nsecond line\u{202e}{}", "error-".repeat(80));

    let detail = source_control_branch_status_detail(&raw);

    assert!(detail.starts_with("first line second line"));
    assert!(detail.contains("..."));
    assert_branch_display_text_is_safe(&detail);
    assert!(detail.chars().count() <= super::SOURCE_CONTROL_BRANCH_STATUS_DETAIL_MAX_CHARS);
}

#[test]
fn branch_display_fragments_use_expected_fallbacks_when_sanitized_blank() {
    assert_eq!(
        source_control_branch_display_name("\n\r\u{202e}\u{2066}"),
        "unnamed branch"
    );
    assert_eq!(
        source_control_branch_status_detail("\u{2028}\u{2029}\u{200f}"),
        "unknown error"
    );
}

#[test]
fn branch_display_fragment_cow_borrows_clean_ascii_and_unicode() {
    let ascii = "feature/cow-clean";
    let unicode = "feature/東京-über";

    for value in [ascii, unicode] {
        match source_control_branch_display_fragment_cow(
            value,
            super::SOURCE_CONTROL_BRANCH_DISPLAY_MAX_CHARS,
            "unnamed branch",
        ) {
            Cow::Borrowed(label) => assert_eq!(label, value),
            Cow::Owned(label) => panic!("expected borrowed display fragment, got {label:?}"),
        }
    }
}

#[test]
fn branch_display_fragment_cow_owns_dirty_truncated_and_fallback_values() {
    let dirty = source_control_branch_display_fragment_cow(
        "feature/\nbranch\u{202e}\u{2066}",
        super::SOURCE_CONTROL_BRANCH_DISPLAY_MAX_CHARS,
        "unnamed branch",
    );
    assert!(matches!(dirty, Cow::Owned(_)));
    assert_eq!(dirty.as_ref(), "feature/ branch");

    let raw_long = format!("feature/{}", "long-branch-".repeat(40));
    let truncated = source_control_branch_display_fragment_cow(&raw_long, 40, "unnamed branch");
    assert!(matches!(truncated, Cow::Owned(_)));
    assert!(truncated.contains("..."));
    assert!(truncated.chars().count() <= 40);

    let fallback = source_control_branch_display_fragment_cow(
        "\n\r\u{202e}\u{2066}",
        super::SOURCE_CONTROL_BRANCH_DISPLAY_MAX_CHARS,
        "unnamed branch",
    );
    assert!(matches!(fallback, Cow::Owned(_)));
    assert_eq!(fallback.as_ref(), "unnamed branch");
}

#[test]
fn branch_display_fragment_string_wrapper_matches_cow_output() {
    let raw_long = format!("feature/{}", "wrapper-".repeat(40));
    let cases = [
        "feature/wrapper-clean",
        "feature/東京-wrapper",
        " feature/trimmed ",
        "feature/\nwrapped\u{202e}",
        "\n\r\u{202e}\u{2066}",
        raw_long.as_str(),
    ];

    for value in cases {
        assert_eq!(
            source_control_branch_display_fragment(
                value,
                super::SOURCE_CONTROL_BRANCH_DISPLAY_MAX_CHARS,
                "unnamed branch",
            ),
            source_control_branch_display_fragment_cow(
                value,
                super::SOURCE_CONTROL_BRANCH_DISPLAY_MAX_CHARS,
                "unnamed branch",
            )
            .into_owned()
        );
    }
}

#[test]
fn source_control_branch_label_with_suffix_sanitizes_fragment_and_preserves_suffixes() {
    assert_eq!(
        source_control_branch_label_with_suffix("feature/東京", "  current"),
        "feature/東京  current"
    );

    let raw = format!("feature/\n{}\u{202e}", "suffix-".repeat(80));
    let label = source_control_branch_label_with_suffix(&raw, "  current  remote");

    assert!(label.starts_with("feature/ suffix-"));
    assert!(label.ends_with("  current  remote"));
    assert!(label.contains("..."));
    assert_branch_display_text_is_safe(&label);
    assert!(label.chars().count() <= super::SOURCE_CONTROL_BRANCH_LABEL_MAX_CHARS);
}

#[test]
fn branch_row_display_keeps_current_remote_local_labels_identical() {
    let cases = [
        (
            branch_with_kind("main", false, GitCheckoutType::Local),
            "main",
        ),
        (
            branch_with_kind("main", true, GitCheckoutType::Local),
            "main  current",
        ),
        (
            branch_with_kind("origin/main", false, GitCheckoutType::Remote),
            "origin/main  remote",
        ),
        (
            branch_with_kind("origin/main", true, GitCheckoutType::Remote),
            "origin/main  current  remote",
        ),
        (
            branch_with_kind("v1.0.0", false, GitCheckoutType::Tags),
            "v1.0.0  tag",
        ),
    ];

    for (branch, expected) in cases {
        assert_eq!(
            SourceControlBranchPreparedRow::new(0, &branch, None).label(),
            expected
        );
        assert_eq!(source_control_branch_label(&branch), expected);
    }
}

#[test]
fn branch_row_display_reuses_cached_label_between_reads() {
    let branch = branch_with_kind("origin/main", true, GitCheckoutType::Remote);
    let row = SourceControlBranchPreparedRow::new(0, &branch, None);

    let first = row.label();
    let first_ptr = first.as_ptr();
    let first_len = first.len();
    let second = row.label();

    assert_eq!(first, "origin/main  current  remote");
    assert_eq!(second, first);
    assert_eq!(second.as_ptr(), first_ptr);
    assert_eq!(second.len(), first_len);
    assert_eq!(
        SourceControlBranchPreparedRow::new(0, &branch, Some("origin/main")).label(),
        "origin/main  current  remote"
    );

    let local = branch_with_kind("origin/main", false, GitCheckoutType::Local);
    assert_eq!(
        SourceControlBranchPreparedRow::new(0, &local, Some("origin/main")).label(),
        "origin/main  renaming"
    );
}

#[test]
fn branch_row_display_copy_is_raw_while_label_and_status_are_safe() {
    let raw = format!("feature/\n{}\u{202e}\u{2066}", "row-".repeat(90));
    let branch = branch_with_kind(&raw, true, GitCheckoutType::Local);
    let row = SourceControlBranchPreparedRow::new(0, &branch, Some(&raw));

    let copy = row.copy_name();
    assert_eq!(copy.raw_name(), raw.as_str());
    assert_eq!(source_control_branch_copy_text(&branch), raw);
    assert_eq!(row.checkout_target(), (raw.clone(), GitCheckoutType::Local));

    let label = row.label();
    assert!(label.starts_with("feature/ row-"));
    assert!(label.ends_with("  current  renaming"));
    assert!(label.contains("..."));
    assert_branch_display_text_is_safe(label);
    assert!(label.chars().count() <= super::SOURCE_CONTROL_BRANCH_LABEL_MAX_CHARS);

    let status = copy.status();
    assert!(status.starts_with("Copied branch name feature/ row-"));
    assert!(status.contains("..."));
    assert_branch_display_text_is_safe(&status);
    assert!(
        status.chars().count()
            <= "Copied branch name ".chars().count()
                + super::SOURCE_CONTROL_BRANCH_DISPLAY_MAX_CHARS
    );
}

#[test]
fn branch_copy_text_preserves_raw_name_while_status_is_display_safe() {
    let raw = format!("feature/\n{}\u{202e}\u{2066}", "raw-".repeat(80));
    let branch = GitBranch {
        name: raw.clone(),
        is_current: false,
        kind: GitCheckoutType::Local,
        committer_time_seconds: 0,
    };

    assert_eq!(source_control_branch_copy_text(&branch), raw);

    let status = source_control_branch_copy_status(&branch);
    assert!(status.starts_with("Copied branch name feature/ raw-"));
    assert!(status.contains("..."));
    assert_branch_display_text_is_safe(&status);

    let label = source_control_branch_label(&branch);
    assert!(label.starts_with("feature/ raw-"));
    assert!(label.contains("..."));
    assert_branch_display_text_is_safe(&label);
}

#[test]
fn branch_rename_target_rejects_stale_or_non_local_sources() {
    let branches = vec![branch("feature/old", false, 20), branch("main", true, 10)];

    assert_eq!(
        source_control_branch_rename_target("feature/new", "feature/old", &branches, ""),
        Some("feature/new".to_owned())
    );
    assert_eq!(
        source_control_branch_rename_target("feature/new", "feature/missing", &branches, ""),
        None
    );
    assert_eq!(
        source_control_branch_rename_blocked_reason(
            "feature/new",
            "feature/missing",
            &branches,
            "",
        ),
        Some("Branch feature/missing is no longer available".to_owned())
    );

    let remote = vec![branch_with_kind(
        "origin/main",
        false,
        GitCheckoutType::Remote,
    )];

    assert_eq!(
        source_control_branch_rename_target("feature/new", "origin/main", &remote, ""),
        None
    );
    assert_eq!(
        source_control_branch_rename_blocked_reason("feature/new", "origin/main", &remote, "",),
        Some("Can only rename local branches, not remote".to_owned())
    );
}

#[test]
fn branch_tooltips_borrow_clean_fragments_and_keep_exact_text() {
    match source_control_branch_tooltip_fragment_cow(
        "feature/search",
        "Create Branch ".chars().count(),
    ) {
        Cow::Borrowed(fragment) => assert_eq!(fragment, "feature/search"),
        Cow::Owned(fragment) => {
            panic!("expected borrowed tooltip fragment, got {fragment:?}")
        }
    }

    assert_eq!(
        source_control_branch_create_tooltip("feature/search"),
        "Create Branch feature/search"
    );
    assert_eq!(
        source_control_branch_rename_tooltip("feature/search", Some("feature/results"), None),
        "Rename Branch feature/search to feature/results"
    );
    assert_eq!(
        source_control_branch_rename_tooltip("feature/search", None, None),
        "Type a new branch name for feature/search"
    );
}

#[test]
fn branch_tooltips_bound_combined_branch_names_and_details() {
    let old_branch = format!("feature/old-{}", "branch-".repeat(80));
    let new_branch = format!("feature/new-{}", "target-".repeat(80));
    let blocked_reason = format!("invalid branch\n{}\u{202e}", "reason-".repeat(80));
    let tooltips = [
        source_control_branch_create_tooltip(&new_branch),
        source_control_branch_rename_tooltip(&old_branch, Some(&new_branch), None),
        source_control_branch_rename_tooltip(&old_branch, None, None),
        source_control_branch_rename_tooltip(&old_branch, None, Some(&blocked_reason)),
    ];

    for tooltip in tooltips {
        assert!(tooltip.contains("..."));
        assert_branch_display_text_is_safe(&tooltip);
        assert!(tooltip.chars().count() <= super::SOURCE_CONTROL_BRANCH_TOOLTIP_MAX_CHARS);
    }
}

fn assert_branch_display_text_is_safe(value: &str) {
    assert!(
        !value.chars().any(is_unsafe_branch_display_char),
        "display text contains unsafe characters: {value:?}"
    );
}

fn is_unsafe_branch_display_char(ch: char) -> bool {
    ch.is_control()
        || matches!(
            ch,
            '\u{061c}'
                | '\u{200e}'
                | '\u{200f}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202a}'..='\u{202e}'
                | '\u{2066}'..='\u{2069}'
        )
}
