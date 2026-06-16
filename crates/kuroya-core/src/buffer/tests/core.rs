use super::*;

#[test]
fn duplicate_position_transform_saturates_extreme_offsets() {
    let duplicate = LineDuplicateEdit {
        edit: TextEdit {
            range: 20..20,
            inserted: "copy".to_owned(),
        },
        source_range: 10..20,
        duplicate_start_offset: usize::MAX,
    };

    assert_eq!(
        transform_duplicate_position(15, &[duplicate], &[]),
        usize::MAX
    );
}

#[test]
fn line_move_position_transform_handles_extreme_local_starts() {
    let buffer = TextBuffer::from_text(1, None, "a\n".to_owned());
    let line_move = LineMoveEdit {
        edit: TextEdit {
            range: 0..0,
            inserted: String::new(),
        },
        block: 0..1,
        moved_block_local_start: usize::MAX,
        replacement_lines: vec!["a".to_owned()],
        trailing_newline: true,
    };

    assert_eq!(
        transform_line_move_position(&buffer, buffer.len_chars(), &[line_move], &[]),
        buffer.len_chars()
    );
}

#[test]
fn rope_edits_are_reversible() {
    let mut buffer = TextBuffer::from_text(1, None, "hello world".to_owned());
    buffer.apply_edit(TextEdit {
        range: 6..11,
        inserted: "rust".to_owned(),
    });
    assert_eq!(buffer.text(), "hello rust");
    assert!(buffer.undo());
    assert_eq!(buffer.text(), "hello world");
    assert!(buffer.redo());
    assert_eq!(buffer.text(), "hello rust");
}

#[test]
fn ui_diff_keeps_unicode_boundaries() {
    let mut buffer = TextBuffer::from_text(1, None, "a\u{1f642}c".to_owned());
    buffer.replace_text_from_ui("a\u{1f642}bc");
    assert_eq!(buffer.text(), "a\u{1f642}bc");
}

#[test]
fn ui_diff_keeps_large_unchanged_prefix_and_suffix_scoped() {
    let original = format!("{}middle{}", "a".repeat(4096), "z".repeat(4096));
    let replacement = format!("{}changed{}", "a".repeat(4096), "z".repeat(4096));
    let mut buffer = TextBuffer::from_text(1, None, original.clone());

    buffer.replace_text_from_ui(&replacement);

    assert_eq!(buffer.text(), replacement);
    assert!(buffer.undo());
    assert_eq!(buffer.text(), original);
}

#[test]
fn rope_diff_to_edit_handles_equal_insert_and_delete_cases() {
    let old = Rope::from_str("alpha beta gamma");
    assert_eq!(
        rope_diff_to_edit(&old, "alpha beta gamma"),
        TextEdit {
            range: 16..16,
            inserted: String::new(),
        }
    );
    assert_eq!(
        rope_diff_to_edit(&old, "alpha slow beta gamma"),
        TextEdit {
            range: 6..6,
            inserted: "slow ".to_owned(),
        }
    );
    assert_eq!(
        rope_diff_to_edit(&old, "alpha gamma"),
        TextEdit {
            range: 6..11,
            inserted: String::new(),
        }
    );
}

#[test]
fn byte_char_index_helpers_keep_unicode_boundaries() {
    let buffer = TextBuffer::from_text(1, None, "a\u{1f642}c".to_owned());

    assert_eq!(buffer.char_to_byte(2), 5);
    assert_eq!(buffer.byte_to_char(5), 2);
}

#[test]
fn text_equals_compares_without_requiring_full_text_clone() {
    let buffer = TextBuffer::from_text(1, None, "alpha\nb\u{00e9}ta\n".to_owned());

    assert!(buffer.text_equals("alpha\nb\u{00e9}ta\n"));
    assert!(!buffer.text_equals("alpha\nbeta\n"));
    assert!(!buffer.text_equals("alpha\nb\u{00e9}ta"));
}

#[test]
fn text_equals_buffer_compares_rope_content_across_edit_chunking() {
    let expected = TextBuffer::from_text(1, None, "a\u{1f642}bc".to_owned());
    let mut edited = TextBuffer::from_text(2, None, "abc".to_owned());
    assert!(edited.replace_range(1..1, "\u{1f642}"));

    assert!(edited.text_equals_buffer(&expected));

    let different = TextBuffer::from_text(3, None, "a\u{1f642}bd".to_owned());
    assert!(!edited.text_equals_buffer(&different));
}

#[test]
fn rope_slice_text_borrows_contiguous_lines() {
    let rope = Rope::from_str("alpha\nbeta");
    let line = rope.line(0);

    match rope_slice_text(&line) {
        std::borrow::Cow::Borrowed(text) => assert_eq!(text, "alpha\n"),
        std::borrow::Cow::Owned(_) => panic!("expected contiguous line to be borrowed"),
    }
}

#[test]
fn replace_from_disk_buffer_reuses_prebuilt_rope_and_resets_edit_state() {
    let mut buffer = TextBuffer::from_text(1, None, "old".to_owned());
    assert!(buffer.replace_range(0..3, "dirty"));
    let replacement = TextBuffer::from_text(2, None, "new".to_owned());

    buffer.replace_from_disk_buffer(replacement);

    assert_eq!(buffer.text(), "new");
    assert!(!buffer.is_dirty());
    assert!(!buffer.undo());
}

#[test]
fn text_snapshot_preserves_buffer_text_after_edits() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha".to_owned());
    let snapshot = buffer.text_snapshot();

    buffer.insert_at_cursor(" beta");

    assert_eq!(snapshot.len_bytes(), "alpha".len());
    assert_eq!(snapshot.chunks().collect::<String>(), "alpha");
    assert_eq!(buffer.text(), " betaalpha");
}

#[test]
fn line_content_char_count_capped_ignores_line_endings_and_caps() {
    let buffer = TextBuffer::from_text(1, None, "abcd\nxy\r\n\u{1f642}z".to_owned());

    assert_eq!(buffer.line_content_char_count_capped(0, 10), 4);
    assert_eq!(buffer.line_content_char_count_capped(0, 2), 2);
    assert_eq!(buffer.line_content_char_count_capped(1, 10), 2);
    assert_eq!(buffer.line_content_char_count_capped(2, 10), 2);
    assert_eq!(buffer.line_content_char_count_capped(2, 1), 1);
    assert_eq!(buffer.line_content_char_count_capped(99, 10), 0);
    assert_eq!(buffer.line_content_char_count_capped(0, 0), 0);
}

#[test]
fn line_content_end_char_trims_line_endings_without_full_line_scan() {
    let long = "x".repeat(128 * 1024);
    let text = format!("{long}\nshort\r\nfinal");
    let buffer = TextBuffer::from_text(1, None, text);

    let first_end = long.chars().count();
    let second_start = first_end + 1;
    let second_end = second_start + "short".chars().count();
    let final_start = second_end + "\r\n".chars().count();

    assert_eq!(buffer.line_content_end_char(0), first_end);
    assert_eq!(buffer.line_content_end_char(1), second_end);
    assert_eq!(
        buffer.line_content_end_char(2),
        final_start + "final".chars().count()
    );
    assert_eq!(buffer.line_content_end_char(99), buffer.len_chars());
    assert_eq!(buffer.line_column_to_char(0, usize::MAX), first_end);
}

#[test]
fn line_content_prefix_caps_visible_text_without_newline() {
    let buffer = TextBuffer::from_text(1, None, "alpha\nb\u{00e9}ta\n".to_owned());

    assert_eq!(buffer.line_content_prefix(0, 3).as_deref(), Some("alp"));
    assert_eq!(buffer.line_content_prefix(0, 20).as_deref(), Some("alpha"));
    assert_eq!(
        buffer.line_content_prefix(1, 2).as_deref(),
        Some("b\u{00e9}")
    );
    assert_eq!(buffer.line_content_prefix(2, 20).as_deref(), Some(""));
    assert_eq!(buffer.line_content_prefix(99, 20), None);
}

#[test]
fn text_range_returns_only_the_requested_rope_slice() {
    let buffer = TextBuffer::from_text(1, None, "alpha\nb\u{00e9}ta\n".to_owned());
    let start = "alpha\n".chars().count();
    let end = start + "b\u{00e9}ta".chars().count();

    assert_eq!(
        buffer.text_range(start..end).as_deref(),
        Some("b\u{00e9}ta")
    );
    assert_eq!(buffer.text_range(end..start), None);
    assert_eq!(buffer.text_range(0..buffer.len_chars() + 1), None);
}

#[test]
fn char_at_reads_single_rope_chars_without_allocating_ranges() {
    let buffer = TextBuffer::from_text(1, None, "a\u{00e9}\n".to_owned());

    assert_eq!(buffer.char_at(0), Some('a'));
    assert_eq!(buffer.char_at(1), Some('\u{00e9}'));
    assert_eq!(buffer.char_at(2), Some('\n'));
    assert_eq!(buffer.char_at(3), None);
}

#[test]
fn line_starts_with_checks_rope_lines_without_cloning_them() {
    let buffer = TextBuffer::from_text(1, None, "@@ -1 +1 @@\n+\u{00e9}clair\n".to_owned());

    assert!(buffer.line_starts_with(0, "@@"));
    assert!(buffer.line_starts_with(1, "+\u{00e9}"));
    assert!(!buffer.line_starts_with(1, "++"));
    assert!(!buffer.line_starts_with(99, "@@"));
}

#[test]
fn line_leading_indent_visual_width_stops_before_content_without_cloning_line() {
    let buffer = TextBuffer::from_text(
        1,
        None,
        "\t  child with a very long tail\nno indent\n    ".to_owned(),
    );

    assert_eq!(
        buffer.line_leading_indent_visual_width_capped(0, 99, 4),
        Some(6)
    );
    assert_eq!(
        buffer.line_leading_indent_visual_width_capped(0, 1, 4),
        Some(4)
    );
    assert_eq!(
        buffer.line_leading_indent_visual_width_capped(1, 99, 4),
        Some(0)
    );
    assert_eq!(
        buffer.line_leading_indent_visual_width_capped(2, 99, 4),
        Some(4)
    );
    assert_eq!(
        buffer.line_leading_indent_visual_width_capped(99, 99, 4),
        None
    );
}

#[test]
fn line_snapshot_reports_number_range_and_text() {
    let buffer = TextBuffer::from_text(1, None, "a\n\u{3b2}c".to_owned());

    let first = buffer.line_snapshot(0).unwrap();
    assert_eq!(first.number, 1);
    assert_eq!(first.char_range, 0..2);
    assert_eq!(first.text, "a\n");

    let second = buffer.line_snapshot(1).unwrap();
    assert_eq!(second.number, 2);
    assert_eq!(second.char_range, 2..4);
    assert_eq!(second.text, "\u{3b2}c");

    assert!(buffer.line_snapshot(2).is_none());
}

#[test]
fn line_snapshot_prefix_caps_text_and_range_without_line_endings() {
    let buffer = TextBuffer::from_text(1, None, "alpha\n\u{3b2}c\r\n".to_owned());

    let first = buffer.line_snapshot_prefix(0, 3).unwrap();
    assert_eq!(first.number, 1);
    assert_eq!(first.char_range, 0..3);
    assert_eq!(first.text, "alp");

    let second = buffer.line_snapshot_prefix(1, 10).unwrap();
    assert_eq!(second.number, 2);
    assert_eq!(second.char_range, 6..8);
    assert_eq!(second.text, "\u{3b2}c");

    assert!(buffer.line_snapshot_prefix(99, 3).is_none());
}

#[test]
fn visible_lines_returns_exact_window_snapshots() {
    let buffer = TextBuffer::from_text(1, None, "zero\none\n\u{3b2}two\nthree".to_owned());

    let visible = buffer.visible_lines(1, 2);

    assert_eq!(visible.len(), 2);
    assert_eq!(visible.capacity(), 2);
    assert_eq!(visible[0].number, 2);
    assert_eq!(visible[0].char_range, 5..9);
    assert_eq!(visible[0].text, "one\n");
    assert_eq!(visible[1].number, 3);
    assert_eq!(visible[1].char_range, 9..14);
    assert_eq!(visible[1].text, "\u{3b2}two\n");
}
