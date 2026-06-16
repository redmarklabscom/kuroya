use super::*;

#[test]
fn source_control_branch_filter_matches_names_case_insensitively() {
    let branches = vec![
        GitBranch {
            name: "main".to_owned(),
            is_current: true,
            kind: GitCheckoutType::Local,
            committer_time_seconds: 30,
        },
        GitBranch {
            name: "feature/search".to_owned(),
            is_current: false,
            kind: GitCheckoutType::Local,
            committer_time_seconds: 20,
        },
        GitBranch {
            name: "feature/r\u{00e9}sum\u{00e9}".to_owned(),
            is_current: false,
            kind: GitCheckoutType::Local,
            committer_time_seconds: 10,
        },
    ];

    let all = source_control_filtered_branches(&branches, "");
    assert_eq!(all.len(), 3);

    let matches = source_control_filtered_branches(&branches, "SEARCH");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].name, "feature/search");

    let unicode = source_control_filtered_branches(&branches, "r\u{00e9}sum");
    assert_eq!(unicode.len(), 1);
    assert_eq!(unicode[0].name, "feature/r\u{00e9}sum\u{00e9}");
    assert!(source_control_filtered_branches(&branches, "R\u{00c9}SUM").is_empty());
}

#[test]
fn source_control_branch_sort_order_matches_git_setting() {
    let branches = vec![
        GitBranch {
            name: "feature/old".to_owned(),
            is_current: false,
            kind: GitCheckoutType::Local,
            committer_time_seconds: 10,
        },
        GitBranch {
            name: "main".to_owned(),
            is_current: true,
            kind: GitCheckoutType::Local,
            committer_time_seconds: 20,
        },
        GitBranch {
            name: "feature/new".to_owned(),
            is_current: false,
            kind: GitCheckoutType::Local,
            committer_time_seconds: 30,
        },
        GitBranch {
            name: "alpha".to_owned(),
            is_current: false,
            kind: GitCheckoutType::Local,
            committer_time_seconds: 5,
        },
    ];

    assert_eq!(
        source_control_sorted_branches(&branches, GitBranchSortOrder::CommitterDate)
            .into_iter()
            .map(|branch| branch.name)
            .collect::<Vec<_>>(),
        vec!["main", "feature/new", "feature/old", "alpha"]
    );
    assert_eq!(
        source_control_sorted_branches(&branches, GitBranchSortOrder::Alphabetically)
            .into_iter()
            .map(|branch| branch.name)
            .collect::<Vec<_>>(),
        vec!["main", "alpha", "feature/new", "feature/old"]
    );
}

#[test]
fn source_control_branch_copy_actions_match_branch_name() {
    let branch = GitBranch {
        name: "feature/search".to_owned(),
        is_current: false,
        kind: GitCheckoutType::Local,
        committer_time_seconds: 0,
    };

    assert_eq!(source_control_branch_copy_text(&branch), "feature/search");
    assert_eq!(
        source_control_branch_copy_status(&branch),
        "Copied branch name feature/search"
    );
}

#[test]
fn source_control_branch_copy_text_stays_raw_while_display_strings_are_safe() {
    let branch_name = format!("feature/\u{202e}\n{}{}", "x".repeat(400), "\u{2066}");
    let branch = GitBranch {
        name: branch_name.clone(),
        is_current: false,
        kind: GitCheckoutType::Local,
        committer_time_seconds: 0,
    };

    assert_eq!(source_control_branch_copy_text(&branch), branch_name);

    let copy_status = source_control_branch_copy_status(&branch);
    assert!(copy_status.starts_with("Copied branch name feature/ "));
    assert!(copy_status.contains("..."));
    assert_branch_display_text_is_safe(&copy_status);
    assert!(copy_status.chars().count() <= 200);

    let label = source_control_branch_label(&branch);
    assert!(label.starts_with("feature/ "));
    assert!(label.contains("..."));
    assert_branch_display_text_is_safe(&label);
    assert!(label.chars().count() <= 180);
}

#[test]
fn source_control_branch_tooltips_and_blocked_reasons_sanitize_display_names() {
    let branch_name = format!("feature/\u{2066}{}", "n".repeat(400));
    let target_name = format!("target/\u{202e}\n{}", "t".repeat(400));
    let branches = vec![GitBranch {
        name: branch_name.clone(),
        is_current: false,
        kind: GitCheckoutType::Local,
        committer_time_seconds: 0,
    }];

    let create_tooltip = source_control_branch_create_tooltip(&branch_name);
    assert!(create_tooltip.starts_with("Create Branch feature/"));
    assert!(create_tooltip.contains("..."));
    assert_branch_display_text_is_safe(&create_tooltip);
    assert!(create_tooltip.chars().count() <= 180);

    let create_blocked =
        source_control_branch_create_blocked_reason(&branch_name, &branches, "", "", "-")
            .expect("existing branch should block create");
    assert!(create_blocked.starts_with("Branch feature/"));
    assert!(create_blocked.ends_with(" already exists"));
    assert_branch_display_text_is_safe(&create_blocked);
    assert!(create_blocked.chars().count() <= 190);

    let rename_tooltip =
        source_control_branch_rename_tooltip(&branch_name, Some(&target_name), None);
    assert!(rename_tooltip.starts_with("Rename Branch feature/"));
    assert!(rename_tooltip.contains(" to target/ "));
    assert!(rename_tooltip.contains("..."));
    assert_branch_display_text_is_safe(&rename_tooltip);
    assert!(rename_tooltip.chars().count() <= 340);

    let blocked_detail = format!("first line\u{202e}\nsecond line{}", "e".repeat(400));
    let blocked_tooltip =
        source_control_branch_rename_tooltip(&branch_name, None, Some(&blocked_detail));
    assert!(blocked_tooltip.starts_with("first line second line"));
    assert!(blocked_tooltip.contains("..."));
    assert_branch_display_text_is_safe(&blocked_tooltip);
    assert!(blocked_tooltip.chars().count() <= 240);
}

fn assert_branch_display_text_is_safe(value: &str) {
    assert!(
        !value.chars().any(is_unsafe_branch_display_char),
        "display text contains unsafe characters: {value:?}"
    );
    assert!(
        value.chars().count() <= 700,
        "display text should be bounded: {} chars",
        value.chars().count()
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

#[test]
fn source_control_branch_keyboard_actions_match_selected_branch_controls() {
    assert_eq!(
        source_control_branch_keyboard_action_labels(),
        vec![
            "Alt+C Copy Branch Name",
            "Alt+R Rename Branch",
            "Alt+D Delete Branch"
        ]
    );
}

#[test]
fn source_control_branch_empty_label_distinguishes_loading_and_filter_states() {
    assert_eq!(
        source_control_branch_empty_label("", true),
        "Loading git branches"
    );
    assert_eq!(
        source_control_branch_empty_label("  ", false),
        "No branches found"
    );
    assert_eq!(
        source_control_branch_empty_label("feature", false),
        "No matching branches"
    );
}

#[test]
fn source_control_branch_create_name_requires_new_branch_query() {
    let branches = vec![
        GitBranch {
            name: "main".to_owned(),
            is_current: true,
            kind: GitCheckoutType::Local,
            committer_time_seconds: 20,
        },
        GitBranch {
            name: "feature/search".to_owned(),
            is_current: false,
            kind: GitCheckoutType::Local,
            committer_time_seconds: 10,
        },
    ];

    assert_eq!(
        source_control_branch_create_name("feature/new", &branches, "", "", "-"),
        Some("feature/new".to_owned())
    );
    assert_eq!(
        source_control_branch_create_name("  ", &branches, "", "", "-"),
        None
    );
    assert_eq!(
        source_control_branch_create_name("feature/search", &branches, "", "", "-"),
        None
    );
    assert_eq!(
        source_control_branch_create_tooltip("feature/new"),
        "Create Branch feature/new"
    );
}

#[test]
fn source_control_branch_create_name_applies_prefix_and_whitespace_setting() {
    let branches = vec![GitBranch {
        name: "feature/search-ui".to_owned(),
        is_current: false,
        kind: GitCheckoutType::Local,
        committer_time_seconds: 10,
    }];

    assert_eq!(
        source_control_new_branch_name("search ui", "feature/", "-"),
        Some("feature/search-ui".to_owned())
    );
    assert_eq!(
        source_control_new_branch_name("feature/search ui", "feature/", "_"),
        Some("feature/search_ui".to_owned())
    );
    assert_eq!(
        source_control_branch_create_name("search ui", &branches, "feature/", "", "-"),
        None
    );
    assert_eq!(
        source_control_branch_create_name("search ui", &branches, "feature/", "", "_"),
        Some("feature/search_ui".to_owned())
    );
}

#[test]
fn source_control_branch_create_name_respects_validation_regex() {
    let branches = Vec::new();

    assert_eq!(
        source_control_branch_create_name("feature/search", &branches, "", "^feature/", "-"),
        Some("feature/search".to_owned())
    );
    assert_eq!(
        source_control_branch_create_name("bugfix/search", &branches, "", "^feature/", "-"),
        None
    );
    assert_eq!(
        source_control_branch_create_blocked_reason(
            "bugfix/search",
            &branches,
            "",
            "^feature/",
            "-"
        ),
        Some("Branch name does not match git.branchValidationRegex".to_owned())
    );
    assert!(
        source_control_branch_create_blocked_reason("feature/search", &branches, "", "[", "-")
            .is_some_and(|reason| reason.starts_with("Invalid git.branchValidationRegex:"))
    );
}

#[test]
fn source_control_branch_delete_action_skips_current_branch() {
    let current = GitBranch {
        name: "main".to_owned(),
        is_current: true,
        kind: GitCheckoutType::Local,
        committer_time_seconds: 20,
    };
    let inactive = GitBranch {
        name: "feature/search".to_owned(),
        is_current: false,
        kind: GitCheckoutType::Local,
        committer_time_seconds: 10,
    };
    let remote = GitBranch {
        name: "origin/main".to_owned(),
        is_current: false,
        kind: GitCheckoutType::Remote,
        committer_time_seconds: 30,
    };

    assert!(!source_control_branch_can_delete(&current));
    assert_eq!(
        source_control_branch_delete_tooltip(&current),
        "Cannot delete the current branch"
    );
    assert!(source_control_branch_can_delete(&inactive));
    assert_eq!(
        source_control_branch_delete_tooltip(&inactive),
        "Delete Branch (Alt+D)"
    );
    assert!(!source_control_branch_can_delete(&remote));
    assert_eq!(
        source_control_branch_delete_tooltip(&remote),
        "Can only delete local branches"
    );
    assert!(!source_control_branch_can_rename(&remote));
    assert_eq!(
        source_control_branch_rename_action_tooltip(&remote),
        "Can only rename local branches, not remote"
    );
}

#[test]
fn source_control_branch_rename_target_requires_new_unique_name() {
    let branches = vec![
        GitBranch {
            name: "main".to_owned(),
            is_current: true,
            kind: GitCheckoutType::Local,
            committer_time_seconds: 20,
        },
        GitBranch {
            name: "feature/search".to_owned(),
            is_current: false,
            kind: GitCheckoutType::Local,
            committer_time_seconds: 10,
        },
    ];

    assert_eq!(
        source_control_branch_rename_target("feature/find", "feature/search", &branches, ""),
        Some("feature/find".to_owned())
    );
    assert_eq!(
        source_control_branch_rename_target("  ", "feature/search", &branches, ""),
        None
    );
    assert_eq!(
        source_control_branch_rename_target("feature/search", "feature/search", &branches, ""),
        None
    );
    assert_eq!(
        source_control_branch_rename_target("main", "feature/search", &branches, ""),
        None
    );
    assert_eq!(
        source_control_branch_rename_tooltip("feature/search", Some("feature/find"), None),
        "Rename Branch feature/search to feature/find"
    );
    assert_eq!(
        source_control_branch_rename_tooltip("feature/search", None, None),
        "Type a new branch name for feature/search"
    );
    assert_eq!(
        source_control_branch_rename_target(
            "bugfix/search",
            "feature/search",
            &branches,
            "^feature/"
        ),
        None
    );
    assert_eq!(
        source_control_branch_rename_blocked_reason(
            "bugfix/search",
            "feature/search",
            &branches,
            "^feature/"
        ),
        Some("Branch name does not match git.branchValidationRegex".to_owned())
    );
    assert_eq!(
        source_control_branch_rename_tooltip(
            "feature/search",
            None,
            Some("Branch name does not match git.branchValidationRegex")
        ),
        "Branch name does not match git.branchValidationRegex"
    );
}

#[test]
fn source_control_branch_statuses_report_lifecycle() {
    assert_eq!(git_branch_list_pending_status(), "Loading git branches");
    assert_eq!(
        git_branch_list_success_status(0),
        "No local git branches found"
    );
    assert_eq!(git_branch_list_success_status(1), "Loaded 1 git branch");
    assert_eq!(git_branch_list_success_status(2), "Loaded 2 git branches");
    assert_eq!(
        git_branch_list_failure_status("not a repo"),
        "Could not load git branches: not a repo"
    );
    assert_eq!(
        git_branch_switch_pending_status("feature/search"),
        "Switching to feature/search"
    );
    assert_eq!(
        git_branch_switch_success_status("feature/search"),
        "Switched to feature/search"
    );
    assert_eq!(
        git_branch_switch_failure_status("feature/search", "dirty worktree"),
        "Could not switch to feature/search: dirty worktree"
    );
    assert_eq!(
        git_branch_create_pending_status("feature/new"),
        "Creating branch feature/new"
    );
    assert_eq!(
        git_branch_create_success_status("feature/new"),
        "Created and switched to feature/new"
    );
    assert_eq!(
        git_branch_create_failure_status("feature/new", "invalid name"),
        "Could not create branch feature/new: invalid name"
    );
    assert_eq!(
        git_branch_delete_pending_status("feature/old"),
        "Deleting branch feature/old"
    );
    assert_eq!(
        git_branch_delete_success_status("feature/old"),
        "Deleted branch feature/old"
    );
    assert_eq!(
        git_branch_delete_failure_status("feature/old", "not found"),
        "Could not delete branch feature/old: not found"
    );
    assert_eq!(
        git_branch_rename_pending_status("feature/old", "feature/new"),
        "Renaming branch feature/old to feature/new"
    );
    assert_eq!(
        git_branch_rename_success_status("feature/old", "feature/new"),
        "Renamed branch feature/old to feature/new"
    );
    assert_eq!(
        git_branch_rename_failure_status("feature/old", "feature/new", "exists"),
        "Could not rename branch feature/old to feature/new: exists"
    );
}

#[test]
fn source_control_branch_statuses_sanitize_names_and_failure_details() {
    let branch_name = format!("feature/\u{202e}\n{}", "b".repeat(400));
    let target_name = format!("target/\u{2066}\n{}", "t".repeat(400));
    let error = format!("first line\u{202e}\nsecond line{}", "e".repeat(400));

    let statuses = [
        git_branch_list_failure_status(&error),
        git_branch_switch_pending_status(&branch_name),
        git_branch_switch_success_status(&branch_name),
        git_branch_switch_failure_status(&branch_name, &error),
        git_branch_create_pending_status(&branch_name),
        git_branch_create_success_status(&branch_name),
        git_branch_create_failure_status(&branch_name, &error),
        git_branch_delete_pending_status(&branch_name),
        git_branch_delete_success_status(&branch_name),
        git_branch_delete_failure_status(&branch_name, &error),
        git_branch_rename_pending_status(&branch_name, &target_name),
        git_branch_rename_success_status(&branch_name, &target_name),
        git_branch_rename_failure_status(&branch_name, &target_name, &error),
    ];

    for status in statuses {
        assert_branch_display_text_is_safe(&status);
        assert!(status.chars().count() <= 640);
    }

    let failure = git_branch_rename_failure_status(&branch_name, &target_name, &error);
    assert!(failure.contains("first line second line"));
    assert!(failure.contains("..."));
}
