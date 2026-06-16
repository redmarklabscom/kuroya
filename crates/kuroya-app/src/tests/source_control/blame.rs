use super::*;

#[test]
fn source_control_blame_key_normalizes_workspace_relative_paths() {
    let root = PathBuf::from("workspace");
    let expected = root.join("src").join("main.rs");

    assert_eq!(
        source_control_blame_key_for_path(
            &root,
            &PathBuf::from(".")
                .join("workspace")
                .join("src")
                .join(".")
                .join("main.rs")
        ),
        Some(expected.clone())
    );
    assert_eq!(
        source_control_blame_key_for_path(
            &root,
            &PathBuf::from("src")
                .join("generated")
                .join("..")
                .join("main.rs")
        ),
        Some(expected)
    );
}

#[test]
fn source_control_blame_key_rejects_relative_path_escaping_workspace() {
    let root = PathBuf::from("workspace");

    assert_eq!(
        source_control_blame_key_for_path(
            &root,
            &PathBuf::from("..")
                .join("workspace")
                .join("src")
                .join("main.rs")
        ),
        None
    );
    assert_eq!(
        source_control_blame_key_for_path(
            &root,
            &PathBuf::from("src")
                .join("..")
                .join("..")
                .join("workspace")
                .join("src")
                .join("main.rs")
        ),
        None
    );
}

#[cfg(windows)]
#[test]
fn source_control_blame_key_rejects_absolute_path_outside_workspace() {
    let root = PathBuf::from(r"C:\repo");

    assert_eq!(
        source_control_blame_key_for_path(&root, &PathBuf::from(r"C:\outside\main.rs")),
        None
    );
}

#[cfg(windows)]
#[test]
fn source_control_blame_key_matches_windows_case_aliases() {
    let root = PathBuf::from(r"C:\Repo");
    let expected = PathBuf::from(r"c:\repo\src\main.rs");

    assert_eq!(
        source_control_blame_key_for_path(
            &root,
            &PathBuf::from(r"c:\repo").join("SRC").join("MAIN.rs")
        ),
        Some(expected.clone())
    );
    assert_eq!(
        source_control_blame_key_for_path(&root, &PathBuf::from("SRC").join("MAIN.rs")),
        Some(expected)
    );
}

#[test]
fn duplicate_git_blame_request_reuses_in_flight_normalized_path() {
    let root = PathBuf::from("workspace");
    let key_path = root.join("src").join("main.rs");
    let variant_path = root.join("src").join("..").join("src").join("main.rs");
    let mut app = app_for_source_control_test(root);
    app.source_control_blame_next_request_id = 1;
    app.source_control_blame_active_request_id = 1;
    app.source_control_blame_active_request_ids
        .insert(key_path.clone(), 1);
    app.source_control_blame_in_flight_request_ids
        .insert(key_path.clone(), 1);

    app.request_file_blame(variant_path.clone(), false);

    assert_eq!(app.source_control_blame_next_request_id, 1);
    assert_eq!(app.source_control_blame_active_request_id, 1);
    assert_eq!(
        app.source_control_blame_pending_path,
        Some(key_path.clone())
    );
    assert_eq!(
        app.source_control_blame_in_flight_request_ids
            .get(&key_path),
        Some(&1)
    );
    assert!(
        !app.source_control_blame_reload_queued_paths
            .contains(&key_path)
    );
    assert!(
        !app.source_control_blame_in_flight_request_ids
            .contains_key(&variant_path)
    );
}

#[test]
fn git_blame_loaded_result_matches_and_caches_normalized_path_alias() {
    let root = PathBuf::from("workspace");
    let key_path = root.join("src").join("main.rs");
    let event_path = root.join("src").join("..").join("src").join("main.rs");
    let lines = vec![GitBlameLine {
        line_number: 1,
        short_oid: "12345678".to_owned(),
        author: "Kuroya Test".to_owned(),
        author_time_seconds: 1_700_000_000,
        summary: "current".to_owned(),
    }];
    let mut app = app_for_source_control_test(root.clone());
    app.source_control_blame_active_request_id = 1;
    app.source_control_blame_active_request_ids
        .insert(key_path.clone(), 1);
    app.source_control_blame_pending_path = Some(key_path.clone());

    app.apply_git_blame_loaded(
        1,
        root.clone(),
        root,
        event_path.clone(),
        lines,
        "one\n".to_owned(),
    );

    assert!(app.source_control_blame_active_request_ids.is_empty());
    assert_eq!(app.source_control_blame_pending_path, None);
    assert_eq!(
        app.source_control_blame_cache
            .get(&key_path)
            .and_then(|lines| lines.first())
            .map(|line| line.summary.as_str()),
        Some("current")
    );
    assert_eq!(
        app.source_control_blame_cache
            .get(&event_path)
            .and_then(|lines| lines.first())
            .map(|line| line.summary.as_str()),
        Some("current")
    );
}

#[test]
fn clear_source_control_blame_removes_normalized_path_alias_cache_entries() {
    let root = PathBuf::from("workspace");
    let key_path = root.join("src").join("main.rs");
    let event_path = root.join("src").join("..").join("src").join("main.rs");
    let lines = vec![GitBlameLine {
        line_number: 1,
        short_oid: "12345678".to_owned(),
        author: "Kuroya Test".to_owned(),
        author_time_seconds: 1_700_000_000,
        summary: "current".to_owned(),
    }];
    let mut app = app_for_source_control_test(root.clone());
    app.source_control_blame_active_request_id = 1;
    app.source_control_blame_active_request_ids
        .insert(key_path.clone(), 1);

    app.apply_git_blame_loaded(
        1,
        root.clone(),
        root,
        event_path.clone(),
        lines,
        "one\n".to_owned(),
    );

    assert!(app.source_control_blame_cache.contains_key(&key_path));
    assert!(app.source_control_blame_cache.contains_key(&event_path));

    app.clear_source_control_blame_for_path(&key_path);

    assert!(!app.source_control_blame_cache.contains_key(&key_path));
    assert!(!app.source_control_blame_cache.contains_key(&event_path));
}

#[test]
fn source_control_blame_view_formats_blame_metadata_with_source_lines() {
    let lines = vec![
        GitBlameLine {
            line_number: 1,
            short_oid: "12345678".to_owned(),
            author: "Kuroya Test".to_owned(),
            author_time_seconds: 1_700_000_000,
            summary: "initial".to_owned(),
        },
        GitBlameLine {
            line_number: 2,
            short_oid: "abcdef12".to_owned(),
            author: "Very Long Contributor Name".to_owned(),
            author_time_seconds: 1_700_000_000,
            summary: "change second line".to_owned(),
        },
    ];

    let view = format_git_blame_view(&lines, "one\ntwo\n");

    assert!(view.contains("12345678 Kuroya Test"));
    assert!(view.contains("initial | one"));
    assert!(view.contains("abcdef12 Very Long Contrib..."));
    assert!(view.contains("change second line | two"));
}

#[test]
fn source_control_blame_view_formats_committed_and_missing_lines() {
    let lines = vec![GitBlameLine {
        line_number: 1,
        short_oid: "12345678".to_owned(),
        author: "Kuroya Test".to_owned(),
        author_time_seconds: 1_700_000_000,
        summary: "initial".to_owned(),
    }];

    let view = format_git_blame_view(&lines, "one\ntwo\n");

    assert!(view.contains("     1 12345678 Kuroya Test          initial | one\n"));
    assert!(view.contains("     2 -------- Unknown              (uncommitted) | two\n"));
}

#[test]
fn source_control_blame_view_sanitizes_metadata_without_touching_source_text() {
    let lines = vec![GitBlameLine {
        line_number: 1,
        short_oid: "abc\n123".to_owned(),
        author: "Ada\nLovelace\tTeam".to_owned(),
        author_time_seconds: 1_700_000_000,
        summary: "fix\r\nmetadata\u{0007} row".to_owned(),
    }];

    let view = format_git_blame_view(&lines, "let tab = \"\t\";\n");

    assert!(view.contains("abc 123 Ada Lovelace Team"));
    assert!(view.contains("fix metadata row | let tab = \"\t\";"));
    assert!(!view.contains("Ada\nLovelace"));
    assert!(!view.contains('\u{0007}'));
}

#[test]
fn source_control_blame_status_bar_label_uses_active_line_author_and_summary() {
    let lines = vec![
        GitBlameLine {
            line_number: 1,
            short_oid: "12345678".to_owned(),
            author: "Kuroya Test".to_owned(),
            author_time_seconds: 1_699_996_400,
            summary: "initial".to_owned(),
        },
        GitBlameLine {
            line_number: 2,
            short_oid: "abcdef12".to_owned(),
            author: "Very Long Contributor Name".to_owned(),
            author_time_seconds: 1_699_913_600,
            summary: "change second line with a summary that should be shortened".to_owned(),
        },
    ];

    assert_eq!(
        git_blame_status_bar_label_at(&lines, 1, "${authorName} (${authorDateAgo})", 1_700_000_000),
        Some("Kuroya Test (1 hour ago)".to_owned())
    );
    assert_eq!(
        git_blame_status_bar_label_at(&lines, 2, "${subject} - ${authorName}", 1_700_000_000),
        Some("change second line with a summary that should... - Very Long Contr...".to_owned())
    );
    assert_eq!(
        git_blame_editor_decoration_label_at(
            &lines,
            2,
            "${hash}: ${subject}, ${authorName} (${authorDateAgo})",
            1_700_000_000
        ),
        Some(
            "abcdef12: change second line with a summary that should..., Very Long Contr... (1 day ago)"
                .to_owned()
        )
    );
    assert_eq!(
        git_blame_status_bar_label_at(&lines, 3, "${authorName}", 1_700_000_000),
        None
    );
    assert_eq!(
        git_blame_editor_decoration_label_at(&lines, 3, "${subject}", 1_700_000_000),
        None
    );
}

#[test]
fn source_control_blame_template_sanitizes_metadata_fields() {
    let lines = vec![GitBlameLine {
        line_number: 1,
        short_oid: "abc\n123".to_owned(),
        author: "\tAda\nLovelace".to_owned(),
        author_time_seconds: 1_699_996_400,
        summary: "fix\rbug".to_owned(),
    }];

    assert_eq!(
        git_blame_status_bar_label_at(
            &lines,
            1,
            "${hash}: ${subject} by ${authorName}",
            1_700_000_000
        ),
        Some("abc 123: fix bug by Ada Lovelace".to_owned())
    );
}

#[test]
fn source_control_blame_editor_decoration_hover_follows_disable_setting() {
    let lines = vec![GitBlameLine {
        line_number: 1,
        short_oid: "12345678".to_owned(),
        author: "Kuroya Test".to_owned(),
        author_time_seconds: 1_699_996_400,
        summary: "initial".to_owned(),
    }];

    assert_eq!(
        git_blame_editor_decoration_hover_text_at(
            &lines,
            1,
            "${hash}: ${subject}, ${authorName}",
            false,
            1_700_000_000,
        )
        .as_deref(),
        Some("12345678: initial, Kuroya Test")
    );
    assert_eq!(
        git_blame_editor_decoration_hover_text_at(
            &lines,
            1,
            "${hash}: ${subject}, ${authorName}",
            true,
            1_700_000_000,
        ),
        None
    );
}
