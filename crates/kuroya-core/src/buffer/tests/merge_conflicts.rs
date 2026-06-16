use super::*;

#[test]
fn buffer_detects_and_resolves_merge_conflict_at_cursor() {
    let mut buffer = TextBuffer::from_text(
        1,
        None,
        "one\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> feature\ntwo\n".to_owned(),
    );
    buffer.set_single_cursor(buffer.line_column_to_char(2, 0));

    let conflicts = buffer.merge_conflicts();
    assert_eq!(conflicts.len(), 1);
    assert!(buffer.resolve_merge_conflict_at_cursor(MergeConflictResolution::Incoming));

    assert_eq!(buffer.text(), "one\ntheirs\ntwo\n");
}

#[test]
fn buffer_resolves_merge_conflict_at_target_line_not_cursor() {
    let original = concat!(
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
    let mut buffer = TextBuffer::from_text(1, None, original.to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(2, 0));

    assert!(buffer.resolve_merge_conflict_at_line(8, MergeConflictResolution::Incoming));

    assert_eq!(
        buffer.text(),
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
}

#[test]
fn buffer_merge_conflicts_scan_rope_lines_without_requiring_string_lines() {
    let padding = " ".repeat(4096);
    let buffer = TextBuffer::from_text(
        1,
        None,
        format!("{padding}<<<<<<< HEAD\nours\n=======\ntheirs\n{padding}>>>>>>> feature\n"),
    );

    assert_eq!(
        buffer.merge_conflicts(),
        vec![MergeConflict {
            start_line: 0,
            separator_line: 2,
            end_line: 4,
        }]
    );
}

#[test]
fn buffer_merge_conflict_resolution_is_scoped_and_undoable() {
    let original =
        "before\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> feature\nafter\n".to_owned();
    let mut buffer = TextBuffer::from_text(1, None, original.clone());
    buffer.set_single_cursor(buffer.line_column_to_char(3, 0));

    assert!(buffer.resolve_merge_conflict_at_cursor(MergeConflictResolution::Current));
    assert_eq!(buffer.text(), "before\nours\nafter\n");
    assert!(buffer.undo());
    assert_eq!(buffer.text(), original);
}
