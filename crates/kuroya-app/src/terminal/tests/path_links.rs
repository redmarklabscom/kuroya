use super::*;

#[test]
fn terminal_path_links_parse_relative_error_locations() {
    let workspace = PathBuf::from("workspace");
    let link = terminal_path_link_at_text_position("--> src/main.rs:12:5", 6, &workspace).unwrap();

    assert_eq!(link.path, workspace.join("src/main.rs"));
    assert_eq!(link.line, 12);
    assert_eq!(link.column, 5);
}

#[test]
fn terminal_path_links_map_wide_cells_to_text_columns() {
    let size = PtySize {
        rows: 4,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    };
    let prefix = "\u{754c}".repeat(24);
    let text = format!("{prefix} src/main.rs:12:5");
    let pane = pane_with_session(session_with_output(1, size, text.as_bytes()), size);

    let link = pane
        .terminal_path_link_at_cell(
            0,
            TerminalCellPosition {
                row: 0,
                col: (prefix.chars().count() * 2 + 1) as u16,
            },
        )
        .unwrap();

    assert_eq!(link.path, PathBuf::from("workspace").join("src/main.rs"));
    assert_eq!(link.line, 12);
    assert_eq!(link.column, 5);
}

#[test]
fn terminal_path_links_resolve_relative_paths_from_session_cwd() {
    let size = PtySize {
        rows: 4,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    };
    let mut session = session_with_output(1, size, b"src/main.rs:12:5");
    session.initial_cwd = Some(PathBuf::from("workspace/tools"));
    let pane = pane_with_session(session, size);

    let link = pane
        .terminal_path_link_at_cell(0, TerminalCellPosition { row: 0, col: 2 })
        .unwrap();

    assert_eq!(
        link.path,
        PathBuf::from("workspace").join("tools").join("src/main.rs")
    );
    assert_eq!(link.line, 12);
    assert_eq!(link.column, 5);
}

#[test]
fn terminal_path_links_resolve_restored_relative_paths_from_restored_cwd() {
    let root = temp_terminal_root("restored-link-cwd");
    let tools = root.join("tools");
    fs::create_dir_all(&tools).unwrap();
    let size = PtySize {
        rows: 4,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    };
    let mut pane = pane_with_sessions(Vec::new(), size);
    pane.cwd = root.clone();
    pane.restore_terminal_sessions(
        &[PersistedTerminalSession {
            cwd: Some(tools.clone()),
            scrollback: "src/main.rs:12:5".to_owned(),
            scrollback_offset: 0,
            custom_title: None,
            process_label: Some("Restored".to_owned()),
            process_status: None,
            window_title: None,
        }],
        0,
        false,
        &[1.0],
        true,
    );

    let link = pane
        .terminal_path_link_at_cell(0, TerminalCellPosition { row: 0, col: 2 })
        .unwrap();

    assert_eq!(link.path, tools.join("src/main.rs"));
    assert_eq!(link.line, 12);
    assert_eq!(link.column, 5);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn terminal_path_links_parse_windows_absolute_locations() {
    let workspace = PathBuf::from("workspace");
    let link =
        terminal_path_link_at_text_position("C:\\repo\\src\\main.rs:42:9", 10, &workspace).unwrap();

    assert_eq!(link.path, PathBuf::from("C:\\repo\\src\\main.rs"));
    assert_eq!(link.line, 42);
    assert_eq!(link.column, 9);
}

#[test]
fn terminal_path_links_parse_trailing_colon_error_locations() {
    let workspace = PathBuf::from("workspace");
    let link =
        terminal_path_link_at_text_position("src/main.rs:12:5: error", 5, &workspace).unwrap();

    assert_eq!(link.path, workspace.join("src/main.rs"));
    assert_eq!(link.line, 12);
    assert_eq!(link.column, 5);
}

#[test]
fn terminal_path_links_parse_adjacent_diagnostic_suffixes() {
    let workspace = PathBuf::from("workspace");

    let link = terminal_path_link_at_text_position("src/main.rs:12:5:error[E0425]", 5, &workspace)
        .unwrap();
    assert_eq!(link.path, workspace.join("src/main.rs"));
    assert_eq!(link.line, 12);
    assert_eq!(link.column, 5);

    let windows = "C:\\repo\\src\\main.rs:42:9:error:expected";
    let windows_link =
        terminal_path_link_at_text_position(windows, windows.find("main").unwrap(), &workspace)
            .unwrap();
    assert_eq!(windows_link.path, PathBuf::from("C:\\repo\\src\\main.rs"));
    assert_eq!(windows_link.line, 42);
    assert_eq!(windows_link.column, 9);

    let parenthesized = "tests/smoke.rs(7,3):failed";
    let parenthesized_link = terminal_path_link_at_text_position(
        parenthesized,
        parenthesized.find("smoke").unwrap(),
        &workspace,
    )
    .unwrap();
    assert_eq!(parenthesized_link.path, workspace.join("tests/smoke.rs"));
    assert_eq!(parenthesized_link.line, 7);
    assert_eq!(parenthesized_link.column, 3);
}

#[test]
fn terminal_path_links_parse_colon_location_ranges() {
    let workspace = PathBuf::from("workspace");

    let link =
        terminal_path_link_at_text_position("src/main.rs:12:5-12:9 error", 5, &workspace).unwrap();
    assert_eq!(link.path, workspace.join("src/main.rs"));
    assert_eq!(link.line, 12);
    assert_eq!(link.column, 5);

    let same_line =
        terminal_path_link_at_text_position("src/main.rs:12:5-9 warning", 5, &workspace).unwrap();
    assert_eq!(same_line.path, workspace.join("src/main.rs"));
    assert_eq!(same_line.line, 12);
    assert_eq!(same_line.column, 5);

    let line_range =
        terminal_path_link_at_text_position("src/main.rs:12-14 failed", 5, &workspace).unwrap();
    assert_eq!(line_range.path, workspace.join("src/main.rs"));
    assert_eq!(line_range.line, 12);
    assert_eq!(line_range.column, 1);
}

#[test]
fn terminal_path_links_parse_windows_location_ranges() {
    let workspace = PathBuf::from("workspace");
    let link =
        terminal_path_link_at_text_position("C:\\repo\\src\\main.rs:42:9-43:1", 10, &workspace)
            .unwrap();

    assert_eq!(link.path, PathBuf::from("C:\\repo\\src\\main.rs"));
    assert_eq!(link.line, 42);
    assert_eq!(link.column, 9);
}

#[test]
fn terminal_path_links_parse_parenthesized_error_locations() {
    let workspace = PathBuf::from("workspace");
    let text = "tests/smoke.rs(42,9): failed";
    let link = terminal_path_link_at_text_position(text, 8, &workspace).unwrap();

    assert_eq!(link.path, workspace.join("tests/smoke.rs"));
    assert_eq!(link.line, 42);
    assert_eq!(link.column, 9);

    let location_link =
        terminal_path_link_at_text_position(text, text.find("42").unwrap(), &workspace).unwrap();
    assert_eq!(location_link.path, workspace.join("tests/smoke.rs"));
    assert_eq!(location_link.line, 42);
    assert_eq!(location_link.column, 9);

    let line_only =
        terminal_path_link_at_text_position("tests/smoke.rs(7): failed", 8, &workspace).unwrap();
    assert_eq!(line_only.path, workspace.join("tests/smoke.rs"));
    assert_eq!(line_only.line, 7);
    assert_eq!(line_only.column, 1);

    let range = terminal_path_link_at_text_position("tests/smoke.rs(7,3-8): failed", 8, &workspace)
        .unwrap();
    assert_eq!(range.path, workspace.join("tests/smoke.rs"));
    assert_eq!(range.line, 7);
    assert_eq!(range.column, 3);

    let line_range =
        terminal_path_link_at_text_position("tests/smoke.rs(7-9): failed", 8, &workspace).unwrap();
    assert_eq!(line_range.path, workspace.join("tests/smoke.rs"));
    assert_eq!(line_range.line, 7);
    assert_eq!(line_range.column, 1);
}

#[test]
fn terminal_path_links_ignore_java_stack_method_prefixes() {
    let workspace = PathBuf::from("workspace");
    let text = "at com.example.Main.run(Main.java:42)";

    assert!(
        terminal_path_link_at_text_position(text, text.find("example").unwrap(), &workspace)
            .is_none()
    );

    let link =
        terminal_path_link_at_text_position(text, text.find("Main.java").unwrap(), &workspace)
            .unwrap();
    assert_eq!(link.path, workspace.join("Main.java"));
    assert_eq!(link.line, 42);
    assert_eq!(link.column, 1);
}

#[test]
fn terminal_path_links_parse_quoted_paths_with_spaces() {
    let workspace = PathBuf::from("workspace");
    let text = "error in \"src/my file.rs:12:5\"";

    let link =
        terminal_path_link_at_text_position(text, text.find("my file").unwrap() + 2, &workspace)
            .unwrap();

    assert_eq!(link.path, workspace.join("src/my file.rs"));
    assert_eq!(link.line, 12);
    assert_eq!(link.column, 5);
}

#[test]
fn terminal_path_links_parse_bracketed_paths_with_spaces() {
    let workspace = PathBuf::from("workspace");
    let text = "failed at <tests/smoke spec.rs:9>";

    let link =
        terminal_path_link_at_text_position(text, text.find("smoke spec").unwrap() + 5, &workspace)
            .unwrap();

    assert_eq!(link.path, workspace.join("tests/smoke spec.rs"));
    assert_eq!(link.line, 9);
    assert_eq!(link.column, 1);
}

#[test]
fn terminal_path_links_parse_pytest_node_ids_as_file_paths() {
    let workspace = PathBuf::from("workspace");
    let text = "FAILED tests/test_api.py::test_handles_user - AssertionError";

    let path_link =
        terminal_path_link_at_text_position(text, text.find("test_api").unwrap(), &workspace)
            .unwrap();
    assert_eq!(path_link.path, workspace.join("tests/test_api.py"));
    assert_eq!(path_link.line, 1);
    assert_eq!(path_link.column, 1);

    let node_link =
        terminal_path_link_at_text_position(text, text.find("handles").unwrap(), &workspace)
            .unwrap();
    assert_eq!(node_link.path, workspace.join("tests/test_api.py"));
    assert_eq!(node_link.line, 1);
    assert_eq!(node_link.column, 1);

    assert!(
        terminal_path_link_at_text_position("crate::module::test failed", 2, &workspace).is_none()
    );
}

#[test]
fn terminal_path_links_parse_traceback_line_suffix() {
    let workspace = PathBuf::from("workspace");
    let text = "  File \"src/my module.py\", line 27, in <module>";

    let link =
        terminal_path_link_at_text_position(text, text.find("my module").unwrap() + 2, &workspace)
            .unwrap();

    assert_eq!(link.path, workspace.join("src/my module.py"));
    assert_eq!(link.line, 27);
    assert_eq!(link.column, 1);
}

#[test]
fn terminal_path_links_parse_tilde_home_locations() {
    let workspace = PathBuf::from("workspace");
    let home = PathBuf::from("home/user");
    let link = terminal_path_link_at_text_position_with_home(
        "~/project/src/main.rs:12:5",
        3,
        &workspace,
        Some(&home),
    )
    .unwrap();

    assert_eq!(link.path, home.join("project").join("src").join("main.rs"));
    assert_eq!(link.line, 12);
    assert_eq!(link.column, 5);
}

#[test]
fn terminal_path_links_ignore_tilde_without_home_directory() {
    let workspace = PathBuf::from("workspace");

    assert!(
        terminal_path_link_at_text_position_with_home(
            "~/project/src/main.rs:12:5",
            3,
            &workspace,
            None,
        )
        .is_none()
    );
}

#[test]
fn terminal_path_links_parse_file_uris() {
    let workspace = PathBuf::from("workspace");
    let text = "see file://localhost/tmp/my%20file.rs:7:3";
    let link =
        terminal_path_link_at_text_position(text, text.find("my%20file").unwrap() + 2, &workspace)
            .unwrap();

    assert!(link.path.ends_with(Path::new("tmp/my file.rs")));
    assert_eq!(link.line, 7);
    assert_eq!(link.column, 3);
}

#[test]
fn terminal_path_links_parse_file_uri_fragments_and_queries() {
    let workspace = PathBuf::from("workspace");
    let fragment = "see file://localhost/tmp/my%20file.rs#L7C3";
    let link = terminal_path_link_at_text_position(
        fragment,
        fragment.find("my%20file").unwrap() + 2,
        &workspace,
    )
    .unwrap();

    assert!(link.path.ends_with(Path::new("tmp/my file.rs")));
    assert_eq!(link.line, 7);
    assert_eq!(link.column, 3);

    let colon_fragment = "see file:///tmp/my%20file.rs#L13:5";
    let colon_fragment_link = terminal_path_link_at_text_position(
        colon_fragment,
        colon_fragment.find("my%20file").unwrap() + 2,
        &workspace,
    )
    .unwrap();
    assert!(
        colon_fragment_link
            .path
            .ends_with(Path::new("tmp/my file.rs"))
    );
    assert_eq!(colon_fragment_link.line, 13);
    assert_eq!(colon_fragment_link.column, 5);

    let range = "see file:///tmp/my%20file.rs#L9-L12";
    let range_link = terminal_path_link_at_text_position(
        range,
        range.find("my%20file").unwrap() + 2,
        &workspace,
    )
    .unwrap();
    assert!(range_link.path.ends_with(Path::new("tmp/my file.rs")));
    assert_eq!(range_link.line, 9);
    assert_eq!(range_link.column, 1);

    let query = "see file:///tmp/my%20file.rs?line=11&column=2";
    let query_link = terminal_path_link_at_text_position(
        query,
        query.find("my%20file").unwrap() + 2,
        &workspace,
    )
    .unwrap();
    assert!(query_link.path.ends_with(Path::new("tmp/my file.rs")));
    assert_eq!(query_link.line, 11);
    assert_eq!(query_link.column, 2);
}

#[test]
fn terminal_path_links_ignore_malformed_file_uris() {
    let workspace = PathBuf::from("workspace");
    let text = "see file:///tmp/bad%zz.rs:7:3";

    assert!(
        terminal_path_link_at_text_position(text, text.find("bad").unwrap(), &workspace).is_none()
    );
}

#[test]
fn terminal_path_links_ignore_unsafe_display_controls_in_paths() {
    let workspace = PathBuf::from("workspace");

    for text in [
        "src/bad\u{202e}name.rs:3:2",
        "src/bad\u{1b}name.rs:3:2",
        "file://src/bad%E2%80%AEname.rs:3:2",
    ] {
        assert!(
            terminal_path_link_at_text_position(text, text.find("bad").unwrap(), &workspace)
                .is_none(),
            "text={text:?}"
        );
    }
}

#[test]
fn terminal_path_links_parse_unquoted_line_column_suffix() {
    let workspace = PathBuf::from("workspace");
    let text = "src/main.rs, line 12, column 5: failed";

    let link = terminal_path_link_at_text_position(text, 4, &workspace).unwrap();

    assert_eq!(link.path, workspace.join("src/main.rs"));
    assert_eq!(link.line, 12);
    assert_eq!(link.column, 5);
}

#[test]
fn terminal_path_links_ignore_urls_and_plain_numbers() {
    let workspace = PathBuf::from("workspace");

    assert!(
        terminal_path_link_at_text_position("https://example.test/file.rs:4", 10, &workspace)
            .is_none()
    );
    assert!(terminal_path_link_at_text_position("error:12", 3, &workspace).is_none());
}

#[test]
fn terminal_path_links_ignore_columns_past_rendered_text() {
    let workspace = PathBuf::from("workspace");

    assert!(terminal_path_link_at_text_position("src/main.rs:12:5", 99, &workspace).is_none());
}
