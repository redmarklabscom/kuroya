use super::*;

#[test]
fn select_next_occurrence_first_selects_word_under_cursor() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta".to_owned());
    buffer.set_single_cursor(2);

    assert!(buffer.select_next_occurrence());
    assert_eq!(
        buffer.selections(),
        &[Selection {
            anchor: 0,
            cursor: 5
        }]
    );
    assert_eq!(buffer.selected_text().as_deref(), Some("alpha"));
}

#[test]
fn word_at_cursor_returns_identifier_under_cursor() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha + beta".to_owned());
    buffer.set_single_cursor(1);
    assert_eq!(buffer.word_at_cursor().as_deref(), Some("alpha"));
    buffer.set_single_cursor(5);
    assert_eq!(buffer.word_at_cursor().as_deref(), Some("alpha"));
    buffer.set_single_cursor(buffer.line_column_to_char(0, 6));
    assert_eq!(buffer.word_at_cursor(), None);
}

#[test]
fn completion_prefix_range_tracks_identifier_before_cursor() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha.beta".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 5));
    assert_eq!(
        buffer.completion_prefix_range(),
        Some(buffer.line_column_to_char(0, 0)..buffer.line_column_to_char(0, 5))
    );
    buffer.set_single_cursor(buffer.line_column_to_char(0, 6));
    assert_eq!(
        buffer.completion_prefix_range(),
        Some(buffer.line_column_to_char(0, 6)..buffer.line_column_to_char(0, 6))
    );
    buffer.set_single_cursor(buffer.line_column_to_char(0, 10));
    assert_eq!(
        buffer.completion_prefix_range(),
        Some(buffer.line_column_to_char(0, 6)..buffer.line_column_to_char(0, 10))
    );
}

#[test]
fn word_separators_control_word_range_and_completion_prefix() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha.beta".to_owned());
    buffer.set_single_cursor(buffer.len_chars());
    assert_eq!(buffer.word_at_cursor().as_deref(), Some("beta"));
    assert_eq!(buffer.completion_prefix_range(), Some(6..10));

    buffer.set_word_separators("+-");
    assert_eq!(buffer.word_at_cursor().as_deref(), Some("alpha.beta"));
    assert_eq!(buffer.completion_prefix_range(), Some(0..10));
}

#[test]
fn word_separators_control_word_navigation() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha.beta".to_owned());
    buffer.set_single_cursor(buffer.len_chars());

    buffer.move_word_left();
    assert_eq!(buffer.cursor(), 6);

    buffer.set_word_separators("");
    buffer.set_single_cursor(buffer.len_chars());
    buffer.move_word_left();
    assert_eq!(buffer.cursor(), 0);
}

#[test]
fn select_next_occurrence_adds_next_matching_selection() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta alpha alpha".to_owned());
    buffer.set_single_cursor(1);

    assert!(buffer.select_next_occurrence());
    assert!(buffer.select_next_occurrence());
    assert_eq!(
        buffer.selections(),
        &[
            Selection {
                anchor: 0,
                cursor: 5
            },
            Selection {
                anchor: 11,
                cursor: 16
            }
        ]
    );
    assert_eq!(buffer.selected_text().as_deref(), Some("alpha\nalpha"));
}

#[test]
fn select_next_occurrence_wraps_and_avoids_duplicates() {
    let mut buffer = TextBuffer::from_text(1, None, "one two one".to_owned());
    buffer.set_selection(8, 11);

    assert!(buffer.select_next_occurrence());
    assert_eq!(
        buffer.selections(),
        &[
            Selection {
                anchor: 0,
                cursor: 3
            },
            Selection {
                anchor: 8,
                cursor: 11
            }
        ]
    );
    assert!(!buffer.select_next_occurrence());
    assert_eq!(buffer.selections().len(), 2);
}

#[test]
fn select_next_occurrence_handles_single_char_selection_without_query_vector() {
    let mut buffer = TextBuffer::from_text(1, None, "a \u{03b2} a \u{03b2}".to_owned());
    let beta_start = buffer.line_column_to_char(0, 2);
    let beta_end = buffer.line_column_to_char(0, 3);
    buffer.set_selection(beta_start, beta_end);

    assert!(buffer.select_next_occurrence());

    let next_beta_start = buffer.line_column_to_char(0, 6);
    let next_beta_end = buffer.line_column_to_char(0, 7);
    assert_eq!(
        buffer.selections(),
        &[
            Selection {
                anchor: beta_start,
                cursor: beta_end,
            },
            Selection {
                anchor: next_beta_start,
                cursor: next_beta_end,
            },
        ]
    );
}

#[test]
fn select_next_occurrence_preserves_multiline_selection_matches() {
    let mut buffer = TextBuffer::from_text(1, None, "one\ntwo\none\ntwo\n".to_owned());
    buffer.set_selection(0, 7);

    assert!(buffer.select_next_occurrence());
    assert_eq!(
        buffer.selections(),
        &[
            Selection {
                anchor: 0,
                cursor: 7
            },
            Selection {
                anchor: 8,
                cursor: 15
            }
        ]
    );
}

#[test]
fn select_next_occurrence_ignores_cursor_on_whitespace() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha  beta".to_owned());
    buffer.set_single_cursor(6);

    assert!(!buffer.select_next_occurrence());
    assert_eq!(buffer.selections(), &[Selection::caret(6)]);
}

#[test]
fn select_all_occurrences_selects_word_under_cursor() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta alpha alpha".to_owned());
    buffer.set_single_cursor(1);

    assert_eq!(buffer.select_all_occurrences(100), 3);
    assert_eq!(
        buffer.selections(),
        &[
            Selection {
                anchor: 0,
                cursor: 5
            },
            Selection {
                anchor: 11,
                cursor: 16
            },
            Selection {
                anchor: 17,
                cursor: 22
            }
        ]
    );
    assert_eq!(
        buffer.selected_text().as_deref(),
        Some("alpha\nalpha\nalpha")
    );
}

#[test]
fn select_all_occurrences_respects_existing_selection_and_limit() {
    let mut buffer = TextBuffer::from_text(1, None, "one two one two one".to_owned());
    buffer.set_selection(4, 7);

    assert_eq!(buffer.select_all_occurrences(1), 1);
    assert_eq!(
        buffer.selections(),
        &[Selection {
            anchor: 4,
            cursor: 7
        }]
    );

    assert_eq!(buffer.select_all_occurrences(100), 2);
    assert_eq!(
        buffer.selections(),
        &[
            Selection {
                anchor: 4,
                cursor: 7
            },
            Selection {
                anchor: 12,
                cursor: 15
            }
        ]
    );
}

#[test]
fn select_all_occurrences_preserves_multiline_selection_matches() {
    let mut buffer = TextBuffer::from_text(1, None, "one\ntwo\none\ntwo\n".to_owned());
    buffer.set_selection(0, 7);

    assert_eq!(buffer.select_all_occurrences(100), 2);
    assert_eq!(
        buffer.selections(),
        &[
            Selection {
                anchor: 0,
                cursor: 7
            },
            Selection {
                anchor: 8,
                cursor: 15
            }
        ]
    );
}

#[test]
fn add_cursor_below_preserves_column() {
    let mut buffer = TextBuffer::from_text(1, None, "aa\nbb\ncc".to_owned());
    buffer.set_single_cursor(1);
    buffer.add_cursor_below();
    assert_eq!(
        buffer
            .cursor_positions()
            .into_iter()
            .map(|pos| (pos.line, pos.column))
            .collect::<Vec<_>>(),
        vec![(0, 1), (1, 1)]
    );
}

#[test]
fn add_cursor_with_limit_caps_pointer_added_cursors() {
    let mut buffer = TextBuffer::from_text(1, None, "abcd".to_owned());
    buffer.set_cursors([0, 2]);

    assert!(!buffer.add_cursor_with_limit(3, 2));
    assert_eq!(
        buffer
            .cursor_positions()
            .into_iter()
            .map(|pos| pos.char_idx)
            .collect::<Vec<_>>(),
        vec![0, 2]
    );
    assert!(buffer.add_cursor_with_limit(3, 3));
    assert_eq!(
        buffer
            .cursor_positions()
            .into_iter()
            .map(|pos| pos.char_idx)
            .collect::<Vec<_>>(),
        vec![0, 2, 3]
    );
}

#[test]
fn add_cursor_below_with_limit_caps_new_cursors() {
    let mut buffer = TextBuffer::from_text(1, None, "aa\nbb\ncc".to_owned());
    buffer.set_cursors([
        buffer.line_column_to_char(0, 1),
        buffer.line_column_to_char(1, 1),
    ]);

    assert!(buffer.add_cursor_below_with_limit(3));
    assert_eq!(
        buffer
            .cursor_positions()
            .into_iter()
            .map(|pos| (pos.line, pos.column))
            .collect::<Vec<_>>(),
        vec![(0, 1), (1, 1), (2, 1)]
    );
    assert!(!buffer.add_cursor_below_with_limit(3));
}

#[test]
fn add_cursor_below_ignores_duplicate_candidates_when_limited() {
    let mut buffer = TextBuffer::from_text(1, None, "wide\nx".to_owned());
    buffer.set_cursors([
        buffer.line_column_to_char(0, 2),
        buffer.line_column_to_char(0, 4),
    ]);

    assert!(buffer.add_cursor_below_with_limit(3));
    assert_eq!(
        buffer
            .cursor_positions()
            .into_iter()
            .map(|pos| (pos.line, pos.column))
            .collect::<Vec<_>>(),
        vec![(0, 2), (0, 4), (1, 1)]
    );
    assert!(!buffer.add_cursor_below_with_limit(3));
}

#[test]
fn add_cursors_to_line_ends_uses_selected_lines() {
    let mut buffer = TextBuffer::from_text(1, None, "one\ntwo\nthree".to_owned());
    buffer.set_selection(1, 9);

    assert!(buffer.add_cursors_to_line_ends());
    assert_eq!(
        buffer
            .cursor_positions()
            .into_iter()
            .map(|pos| (pos.line, pos.column))
            .collect::<Vec<_>>(),
        vec![(0, 3), (1, 3), (2, 5)]
    );

    buffer.insert_at_cursors("!");
    assert_eq!(buffer.text(), "one!\ntwo!\nthree!");
}

#[test]
fn add_cursors_to_line_ends_with_limit_caps_selected_lines() {
    let mut buffer = TextBuffer::from_text(1, None, "one\ntwo\nthree".to_owned());
    buffer.set_selection(1, 9);

    assert!(buffer.add_cursors_to_line_ends_with_limit(2));
    assert_eq!(
        buffer
            .cursor_positions()
            .into_iter()
            .map(|pos| (pos.line, pos.column))
            .collect::<Vec<_>>(),
        vec![(0, 3), (1, 3)]
    );
}

#[test]
fn add_cursors_to_line_ends_excludes_next_line_start() {
    let mut buffer = TextBuffer::from_text(1, None, "one\ntwo".to_owned());
    buffer.set_selection(0, 4);

    assert!(buffer.add_cursors_to_line_ends());
    assert_eq!(
        buffer
            .cursor_positions()
            .into_iter()
            .map(|pos| (pos.line, pos.column))
            .collect::<Vec<_>>(),
        vec![(0, 3)]
    );
}

#[test]
fn rectangular_block_selection_splits_multiline_range_by_columns() {
    let mut buffer = TextBuffer::from_text(1, None, "abcd\nef\nwxyz".to_owned());
    let anchor = buffer.line_column_to_char(0, 1);
    let cursor = buffer.line_column_to_char(2, 3);
    buffer.set_selection(anchor, cursor);

    assert!(buffer.select_rectangular_block());
    assert_eq!(
        selection_positions(&buffer),
        vec![((0, 1), (0, 3)), ((1, 1), (1, 2)), ((2, 1), (2, 3))]
    );
}

#[test]
fn rectangular_block_selection_with_limit_caps_rows() {
    let mut buffer = TextBuffer::from_text(1, None, "abcd\nef\nwxyz".to_owned());
    let anchor = buffer.line_column_to_char(0, 1);
    let cursor = buffer.line_column_to_char(2, 3);
    buffer.set_selection(anchor, cursor);

    assert!(buffer.select_rectangular_block_with_limit(2));
    assert_eq!(
        selection_positions(&buffer),
        vec![((0, 1), (0, 3)), ((1, 1), (1, 2))]
    );
}

#[test]
fn rectangular_block_selection_supports_zero_width_columns() {
    let mut buffer = TextBuffer::from_text(1, None, "abcd\nef\nwxyz".to_owned());
    let anchor = buffer.line_column_to_char(0, 2);
    let cursor = buffer.line_column_to_char(2, 2);
    buffer.set_selection(anchor, cursor);

    assert!(buffer.select_rectangular_block());
    assert_eq!(
        selection_positions(&buffer),
        vec![((0, 2), (0, 2)), ((1, 2), (1, 2)), ((2, 2), (2, 2))]
    );
}

#[test]
fn rectangular_block_selection_preserves_leftward_direction() {
    let mut buffer = TextBuffer::from_text(1, None, "abcd\nef\nwxyz".to_owned());
    let anchor = buffer.line_column_to_char(0, 3);
    let cursor = buffer.line_column_to_char(2, 1);
    buffer.set_selection(anchor, cursor);

    assert!(buffer.select_rectangular_block());
    assert_eq!(
        selection_positions(&buffer),
        vec![((0, 3), (0, 1)), ((1, 2), (1, 1)), ((2, 3), (2, 1))]
    );
}

#[test]
fn rectangular_block_selection_ignores_single_line_selection() {
    let mut buffer = TextBuffer::from_text(1, None, "abcd\nef".to_owned());
    buffer.set_selection(1, 3);

    assert!(!buffer.select_rectangular_block());
    assert_eq!(
        buffer.selections(),
        &[Selection {
            anchor: 1,
            cursor: 3
        }]
    );
}

#[test]
fn expand_selection_grows_from_word_to_syntax_ranges() {
    let mut buffer = TextBuffer::from_text(
        1,
        Some(PathBuf::from("src/main.rs")),
        "fn main() {\n    call(alpha);\n}\n".to_owned(),
    );
    let cursor = buffer.text().find("alpha").unwrap() + 2;
    buffer.set_single_cursor(cursor);

    assert!(buffer.expand_selection());
    assert_eq!(buffer.selected_text().as_deref(), Some("alpha"));

    assert!(buffer.expand_selection());
    assert_eq!(buffer.selected_text().as_deref(), Some("(alpha)"));

    assert!(buffer.expand_selection());
    assert_eq!(
        buffer.selected_text().as_deref(),
        Some("    call(alpha);\n")
    );

    assert!(buffer.expand_selection());
    assert_eq!(
        buffer.selected_text().as_deref(),
        Some("\n    call(alpha);\n")
    );
}

#[test]
fn expand_selection_preserves_reversed_selection_direction() {
    let mut buffer = TextBuffer::from_text(1, None, "call(alpha)\n".to_owned());
    let start = buffer.text().find("alpha").unwrap();
    let end = start + "alpha".len();
    buffer.set_selection(end, start);

    assert!(buffer.expand_selection());

    let selection = buffer.selections()[0];
    assert!(selection.anchor > selection.cursor);
    assert_eq!(buffer.selected_text().as_deref(), Some("(alpha)"));
}

#[test]
fn select_lines_selects_current_line_and_expands_on_repeat() {
    let mut buffer = TextBuffer::from_text(1, None, "one\ntwo\nthree".to_owned());
    buffer.set_single_cursor(5);

    assert!(buffer.select_lines());
    assert_eq!(buffer.selected_text().as_deref(), Some("two\n"));
    assert_eq!(
        buffer.selections(),
        &[Selection {
            anchor: 4,
            cursor: 8
        }]
    );

    assert!(buffer.select_lines());
    assert_eq!(buffer.selected_text().as_deref(), Some("two\nthree"));
    assert_eq!(
        buffer.selections(),
        &[Selection {
            anchor: 4,
            cursor: 13
        }]
    );

    assert!(!buffer.select_lines());
}

#[test]
fn select_lines_expands_partial_selection_to_full_line_block() {
    let mut buffer = TextBuffer::from_text(1, None, "one\ntwo\nthree".to_owned());
    buffer.set_selection(1, 5);

    assert!(buffer.select_lines());
    assert_eq!(buffer.selected_text().as_deref(), Some("one\ntwo\n"));
    assert_eq!(
        buffer.selections(),
        &[Selection {
            anchor: 0,
            cursor: 8
        }]
    );
}

#[test]
fn select_lines_excludes_next_line_start_for_partial_selection() {
    let mut buffer = TextBuffer::from_text(1, None, "one\ntwo".to_owned());
    buffer.set_selection(1, 4);

    assert!(buffer.select_lines());
    assert_eq!(buffer.selected_text().as_deref(), Some("one\n"));
    assert_eq!(
        buffer.selections(),
        &[Selection {
            anchor: 0,
            cursor: 4
        }]
    );
}

#[test]
fn final_newline_line_detection_tracks_trailing_newline() {
    let with_final_newline = TextBuffer::from_text(1, None, "one\n".to_owned());
    assert!(with_final_newline.ends_with_newline());
    assert!(!with_final_newline.is_final_newline_line(0));
    assert!(with_final_newline.is_final_newline_line(1));
    assert!(!with_final_newline.is_final_newline_line(usize::MAX));

    let without_final_newline = TextBuffer::from_text(1, None, "one".to_owned());
    assert!(!without_final_newline.ends_with_newline());
    assert!(!without_final_newline.is_final_newline_line(0));
}

#[test]
fn smart_line_start_jumps_to_indent_then_column_zero() {
    let mut buffer = TextBuffer::from_text(1, None, "    let value = 1;".to_owned());
    buffer.set_single_cursor(buffer.len_chars());

    buffer.move_line_start();
    assert_eq!(buffer.cursor_position().column, 4);

    buffer.move_line_start();
    assert_eq!(buffer.cursor_position().column, 0);

    buffer.move_line_start();
    assert_eq!(buffer.cursor_position().column, 4);
}

#[test]
fn explicit_line_start_motions_separate_column_zero_and_indent() {
    let mut buffer = TextBuffer::from_text(1, None, "    let value = 1;\n    \n".to_owned());
    buffer.set_single_cursor(buffer.line_column_to_char(0, 9));

    buffer.move_line_first_non_whitespace();
    assert_eq!(buffer.cursor_position().column, 4);

    buffer.set_single_cursor(buffer.line_column_to_char(0, 9));
    buffer.move_line_column_start();
    assert_eq!(buffer.cursor_position().column, 0);

    buffer.set_single_cursor(buffer.line_column_to_char(1, 2));
    buffer.move_line_first_non_whitespace();
    assert_eq!(buffer.cursor_position().column, 4);
    buffer.move_line_column_start();
    assert_eq!(buffer.cursor_position().column, 0);
}

#[test]
fn smart_line_start_keeps_blank_lines_at_column_zero() {
    let mut buffer = TextBuffer::from_text(1, None, "    \nnext".to_owned());
    buffer.set_single_cursor(2);

    buffer.move_line_start();
    assert_eq!(buffer.cursor_position().column, 0);
}

#[test]
fn smart_line_start_extends_selection_to_indent_then_column_zero() {
    let mut buffer = TextBuffer::from_text(1, None, "    let value = 1;".to_owned());
    let end = buffer.len_chars();
    buffer.set_single_cursor(end);

    buffer.extend_line_start();
    assert_eq!(
        buffer.selections(),
        &[Selection {
            anchor: end,
            cursor: 4
        }]
    );

    buffer.extend_line_start();
    assert_eq!(
        buffer.selections(),
        &[Selection {
            anchor: end,
            cursor: 0
        }]
    );
}

#[test]
fn shift_selection_can_copy_and_delete_ranges() {
    let mut buffer = TextBuffer::from_text(1, None, "abcd".to_owned());
    buffer.set_single_cursor(1);
    buffer.extend_right();
    buffer.extend_right();

    assert!(buffer.has_selection());
    assert_eq!(buffer.selected_text().as_deref(), Some("bc"));
    assert!(buffer.delete_selection_ranges());
    assert_eq!(buffer.text(), "ad");
    assert_eq!(buffer.cursor(), 1);
}

#[test]
fn selected_text_or_lines_returns_selection_when_present() {
    let mut buffer = TextBuffer::from_text(1, None, "abcd".to_owned());
    buffer.set_selection(1, 3);

    assert_eq!(buffer.selected_text_or_lines().as_deref(), Some("bc"));
    assert!(buffer.delete_selection_or_lines());
    assert_eq!(buffer.text(), "ad");
}

#[test]
fn selected_text_or_lines_uses_current_line_without_selection() {
    let mut buffer = TextBuffer::from_text(1, None, "one\ntwo\nthree".to_owned());
    buffer.set_single_cursor(5);

    assert_eq!(buffer.selected_text_or_lines().as_deref(), Some("two\n"));
}

#[test]
fn selected_text_or_lines_uses_unique_multicursor_line_blocks() {
    let mut buffer = TextBuffer::from_text(1, None, "one\ntwo\nthree\nfour".to_owned());
    buffer.set_cursors([1, 6, 11]);

    assert_eq!(
        buffer.selected_text_or_lines().as_deref(),
        Some("one\ntwo\nthree\n")
    );
}

#[test]
fn delete_selection_or_lines_cuts_current_line_without_selection() {
    let mut buffer = TextBuffer::from_text(1, None, "one\ntwo\nthree".to_owned());
    buffer.set_single_cursor(5);

    assert_eq!(buffer.selected_text_or_lines().as_deref(), Some("two\n"));
    assert!(buffer.delete_selection_or_lines());
    assert_eq!(buffer.text(), "one\nthree");
    assert_eq!(buffer.cursor_position().line, 1);
    assert!(buffer.undo());
    assert_eq!(buffer.text(), "one\ntwo\nthree");
}

#[test]
fn delete_selection_or_lines_cuts_final_line_without_trailing_newline() {
    let mut buffer = TextBuffer::from_text(1, None, "one\ntwo".to_owned());
    buffer.set_single_cursor(buffer.len_chars());

    assert_eq!(buffer.selected_text_or_lines().as_deref(), Some("two"));
    assert!(buffer.delete_selection_or_lines());
    assert_eq!(buffer.text(), "one");
    assert_eq!(buffer.cursor(), 3);
}

#[test]
fn word_navigation_moves_by_text_groups() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta.gamma".to_owned());
    buffer.set_single_cursor(buffer.len_chars());

    buffer.move_word_left();
    assert_eq!(buffer.cursor(), 11);
    buffer.move_word_left();
    assert_eq!(buffer.cursor(), 10);
    buffer.move_word_left();
    assert_eq!(buffer.cursor(), 6);
    buffer.move_word_left();
    assert_eq!(buffer.cursor(), 0);

    buffer.move_word_right();
    assert_eq!(buffer.cursor(), 5);
    buffer.move_word_right();
    assert_eq!(buffer.cursor(), 10);
    buffer.move_word_right();
    assert_eq!(buffer.cursor(), 11);
    buffer.move_word_right();
    assert_eq!(buffer.cursor(), buffer.len_chars());
}

#[test]
fn big_word_navigation_moves_by_whitespace_groups() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta.gamma  delta".to_owned());
    buffer.set_single_cursor(buffer.len_chars());

    buffer.move_big_word_left();
    assert_eq!(buffer.cursor(), 18);
    buffer.move_big_word_left();
    assert_eq!(buffer.cursor(), 6);
    buffer.move_big_word_left();
    assert_eq!(buffer.cursor(), 0);

    buffer.move_big_word_right();
    assert_eq!(buffer.cursor(), 6);
    buffer.move_big_word_right();
    assert_eq!(buffer.cursor(), 18);
    buffer.move_big_word_right();
    assert_eq!(buffer.cursor(), buffer.len_chars());
}

#[test]
fn word_end_navigation_moves_to_group_ends() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta.gamma".to_owned());
    buffer.set_single_cursor(0);

    buffer.move_word_end();
    assert_eq!(buffer.cursor(), 4);
    buffer.move_word_end();
    assert_eq!(buffer.cursor(), 9);
    buffer.move_word_end();
    assert_eq!(buffer.cursor(), 10);
    buffer.move_word_end();
    assert_eq!(buffer.cursor(), 15);
    buffer.move_word_end();
    assert_eq!(buffer.cursor(), buffer.len_chars());
}

#[test]
fn big_word_end_navigation_moves_to_whitespace_group_ends() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta.gamma  delta".to_owned());
    buffer.set_single_cursor(0);

    buffer.move_big_word_end();
    assert_eq!(buffer.cursor(), 4);
    buffer.move_big_word_end();
    assert_eq!(buffer.cursor(), 15);
    buffer.move_big_word_end();
    assert_eq!(buffer.cursor(), 22);
    buffer.move_big_word_end();
    assert_eq!(buffer.cursor(), buffer.len_chars());
}

#[test]
fn previous_word_end_navigation_moves_to_group_ends() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta.gamma".to_owned());
    buffer.set_single_cursor(buffer.len_chars());

    buffer.move_previous_word_end();
    assert_eq!(buffer.cursor(), 15);
    buffer.move_previous_word_end();
    assert_eq!(buffer.cursor(), 10);
    buffer.move_previous_word_end();
    assert_eq!(buffer.cursor(), 9);
    buffer.move_previous_word_end();
    assert_eq!(buffer.cursor(), 4);
    buffer.move_previous_word_end();
    assert_eq!(buffer.cursor(), 0);
}

#[test]
fn word_selection_and_deletion_use_transactions() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha beta gamma".to_owned());
    buffer.set_single_cursor(0);
    buffer.extend_word_right();
    assert_eq!(buffer.selected_text().as_deref(), Some("alpha"));

    assert!(buffer.delete_word_forward());
    assert_eq!(buffer.text(), " beta gamma");
    assert_eq!(buffer.cursor(), 0);

    buffer.set_single_cursor(buffer.len_chars());
    assert!(buffer.delete_word_backward());
    assert_eq!(buffer.text(), " beta ");
    assert!(buffer.undo());
    assert_eq!(buffer.text(), " beta gamma");
}

#[test]
fn select_all_replaces_entire_buffer() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha".to_owned());
    buffer.select_all();
    assert_eq!(buffer.selected_text().as_deref(), Some("alpha"));
    buffer.insert_at_cursor("beta");
    assert_eq!(buffer.text(), "beta");
}

#[test]
fn set_path_updates_language() {
    let mut buffer = TextBuffer::new_untitled(1);
    assert_eq!(buffer.language(), LanguageId::PlainText);
    buffer.set_path(PathBuf::from("main.rs"));
    assert_eq!(buffer.path(), Some(&PathBuf::from("main.rs")));
    assert_eq!(buffer.language(), LanguageId::Rust);
}

#[test]
fn replace_from_disk_refreshes_text_and_clears_dirty_state() {
    let mut buffer = TextBuffer::from_text(1, None, "old text".to_owned());
    buffer.insert_at_cursor("dirty ");
    assert!(buffer.is_dirty());
    let previous_version = buffer.version();

    buffer.replace_from_disk("new".to_owned());

    assert_eq!(buffer.text(), "new");
    assert!(!buffer.is_dirty());
    assert!(buffer.version() > previous_version);
    assert_eq!(buffer.selections(), &[Selection::caret(3)]);
    assert!(!buffer.undo());
}
