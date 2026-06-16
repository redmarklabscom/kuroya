use super::*;

#[test]
fn source_control_conflict_resolution_statuses_report_choice() {
    assert_eq!(
        merge_conflict_resolution_success_status(MergeConflictResolution::Current),
        "Accepted current merge conflict"
    );
    assert_eq!(
        merge_conflict_resolution_success_status(MergeConflictResolution::Incoming),
        "Accepted incoming merge conflict"
    );
    assert_eq!(
        merge_conflict_resolution_success_status(MergeConflictResolution::Both),
        "Accepted both merge conflict"
    );
}

#[test]
fn merge_conflict_resolution_reports_missing_buffer() {
    let mut app = app_for_source_control_test(PathBuf::from("workspace"));

    app.resolve_merge_conflict_for_buffer(99, MergeConflictResolution::Current);

    assert_eq!(app.status, "No buffer for merge conflict resolution");
}

#[test]
fn merge_conflict_resolution_rejects_read_only_buffers_without_mutating() {
    let root = PathBuf::from("workspace");
    let conflict_text = "one\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> feature\ntwo\n";
    let mut app = app_for_source_control_test(root.clone());
    let mut buffer = TextBuffer::from_text(
        1,
        Some(root.join("src/conflict.rs")),
        conflict_text.to_owned(),
    );
    let conflict_line = buffer.line_column_to_char(2, 0);
    buffer.set_single_cursor(conflict_line);
    buffer.set_read_only(true);
    app.buffers.push(buffer);
    app.set_active_buffer(1);

    app.resolve_merge_conflict_for_buffer(1, MergeConflictResolution::Incoming);

    assert_eq!(
        app.status,
        "Cannot resolve merge conflict in read-only buffer"
    );
    assert_eq!(app.buffer(1).unwrap().text(), conflict_text);
}

#[test]
fn merge_conflict_resolution_at_line_ignores_cursor_conflict() {
    let root = PathBuf::from("workspace");
    let conflict_text = concat!(
        "one\n",
        "<<<<<<< HEAD\n",
        "ours one\n",
        "=======\n",
        "theirs one\n",
        ">>>>>>> feature\n",
        "middle\n",
        "<<<<<<< HEAD\n",
        "ours two\n",
        "=======\n",
        "theirs two\n",
        ">>>>>>> feature\n",
        "end\n",
    );
    let mut app = app_for_source_control_test(root.clone());
    let mut buffer = TextBuffer::from_text(
        1,
        Some(root.join("src/conflict.rs")),
        conflict_text.to_owned(),
    );
    buffer.set_single_cursor(buffer.line_column_to_char(2, 0));
    app.buffers.push(buffer);
    app.set_active_buffer(1);

    assert!(
        app.run_editor_buffer_context_action(
            1,
            EditorContextAction::AcceptIncomingConflictAtLine(8)
        )
    );

    assert_eq!(
        app.buffer(1).unwrap().text(),
        concat!(
            "one\n",
            "<<<<<<< HEAD\n",
            "ours one\n",
            "=======\n",
            "theirs one\n",
            ">>>>>>> feature\n",
            "middle\n",
            "theirs two\n",
            "end\n",
        )
    );
    assert_eq!(
        app.status,
        merge_conflict_resolution_success_status(MergeConflictResolution::Incoming)
    );
}

#[test]
fn merge_conflict_resolution_at_line_reports_selected_line_miss() {
    let root = PathBuf::from("workspace");
    let conflict_text = "one\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> feature\ntwo\n";
    let mut app = app_for_source_control_test(root.clone());
    let mut buffer = TextBuffer::from_text(
        1,
        Some(root.join("src/conflict.rs")),
        conflict_text.to_owned(),
    );
    buffer.set_single_cursor(buffer.line_column_to_char(2, 0));
    app.buffers.push(buffer);
    app.set_active_buffer(1);

    app.resolve_merge_conflict_for_buffer_at_line(1, 0, MergeConflictResolution::Incoming);

    assert_eq!(app.status, "No merge conflict at selected line");
    assert_eq!(app.buffer(1).unwrap().text(), conflict_text);
}
