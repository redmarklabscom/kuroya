use super::*;

#[test]
fn multicursor_insert_is_one_undo_step() {
    let mut buffer = TextBuffer::from_text(1, None, "abcd".to_owned());
    buffer.set_cursors([1, 3]);
    buffer.insert_at_cursors("|");
    assert_eq!(buffer.text(), "a|bc|d");
    assert_eq!(
        buffer
            .cursor_positions()
            .into_iter()
            .map(|pos| pos.char_idx)
            .collect::<Vec<_>>(),
        vec![2, 5]
    );
    assert!(buffer.undo());
    assert_eq!(buffer.text(), "abcd");
    assert!(buffer.redo());
    assert_eq!(buffer.text(), "a|bc|d");
}

#[test]
fn multicursor_insert_texts_at_cursors_spreads_text_and_undoes_once() {
    let mut buffer = TextBuffer::from_text(1, None, "abcd".to_owned());
    buffer.set_cursors([1, 3]);

    assert!(buffer.insert_texts_at_cursors(vec!["X".to_owned(), "YY".to_owned()]));
    assert_eq!(buffer.text(), "aXbcYYd");
    assert_eq!(
        buffer
            .cursor_positions()
            .into_iter()
            .map(|pos| pos.char_idx)
            .collect::<Vec<_>>(),
        vec![2, 6]
    );
    assert!(buffer.undo());
    assert_eq!(buffer.text(), "abcd");
    assert!(buffer.redo());
    assert_eq!(buffer.text(), "aXbcYYd");
}

#[test]
fn multicursor_insert_texts_at_cursors_rejects_mismatched_or_empty_input() {
    let mut buffer = TextBuffer::from_text(1, None, "abcd".to_owned());
    buffer.set_cursors([1, 3]);

    assert!(!buffer.insert_texts_at_cursors(vec!["X".to_owned()]));
    assert_eq!(buffer.text(), "abcd");
    assert!(!buffer.insert_texts_at_cursors(vec![String::new(), String::new()]));
    assert_eq!(buffer.text(), "abcd");
}

#[test]
fn multicursor_identifier_typing_coalesces_into_one_undo_step() {
    let mut buffer = TextBuffer::from_text(1, None, "abcd".to_owned());
    buffer.set_cursors([1, 3]);

    buffer.insert_at_cursors("x");
    buffer.insert_at_cursors("y");
    buffer.insert_at_cursors("z");

    assert_eq!(buffer.text(), "axyzbcxyzd");
    assert_eq!(
        buffer
            .cursor_positions()
            .into_iter()
            .map(|pos| pos.char_idx)
            .collect::<Vec<_>>(),
        vec![4, 9]
    );
    assert!(buffer.undo());
    assert_eq!(buffer.text(), "abcd");
    assert!(!buffer.undo());
    assert!(buffer.redo());
    assert_eq!(buffer.text(), "axyzbcxyzd");
}

#[test]
fn multicursor_typing_coalescing_stops_after_cursor_set_changes() {
    let mut buffer = TextBuffer::from_text(1, None, "abcd".to_owned());
    buffer.set_cursors([1, 3]);
    buffer.insert_at_cursors("x");
    buffer.set_cursors([0, buffer.len_chars()]);
    buffer.insert_at_cursors("y");

    assert_eq!(buffer.text(), "yaxbcxdy");
    assert!(buffer.undo());
    assert_eq!(buffer.text(), "axbcxd");
    assert!(buffer.undo());
    assert_eq!(buffer.text(), "abcd");
}

#[test]
fn multicursor_typing_history_snapshot_replays_coalesced_runs() {
    let mut buffer = TextBuffer::from_text(1, None, "abcd".to_owned());
    buffer.set_cursors([1, 3]);
    buffer.insert_at_cursors("x");
    buffer.insert_at_cursors("y");
    let snapshot = buffer.history_snapshot(16, 4096).unwrap();

    let mut restored = TextBuffer::from_text(2, None, "axybcxyd".to_owned());
    assert!(restored.restore_history_snapshot(snapshot));
    assert!(restored.undo());
    assert_eq!(restored.text(), "abcd");
    assert!(restored.redo());
    assert_eq!(restored.text(), "axybcxyd");
}

#[test]
fn consecutive_identifier_typing_coalesces_into_one_undo_step() {
    let mut buffer = TextBuffer::new_untitled(1);

    buffer.insert_at_cursor("a");
    buffer.insert_at_cursor("b");
    buffer.insert_at_cursor("c");

    assert_eq!(buffer.text(), "abc");
    assert!(buffer.undo());
    assert_eq!(buffer.text(), "");
    assert!(!buffer.undo());
    assert!(buffer.redo());
    assert_eq!(buffer.text(), "abc");
}

#[test]
fn batched_identifier_typing_coalesces_with_adjacent_typing() {
    let mut buffer = TextBuffer::new_untitled(1);

    buffer.insert_at_cursor("ab");
    buffer.insert_at_cursor("c");
    buffer.insert_at_cursor("de");

    assert_eq!(buffer.text(), "abcde");
    assert!(buffer.undo());
    assert_eq!(buffer.text(), "");
    assert!(!buffer.undo());
    assert!(buffer.redo());
    assert_eq!(buffer.text(), "abcde");
}

#[test]
fn batched_multicursor_identifier_typing_coalesces_with_adjacent_typing() {
    let mut buffer = TextBuffer::from_text(1, None, "abcd".to_owned());
    buffer.set_cursors([1, 3]);

    buffer.insert_at_cursors("xy");
    buffer.insert_at_cursors("z");

    assert_eq!(buffer.text(), "axyzbcxyzd");
    assert_eq!(
        buffer
            .cursor_positions()
            .into_iter()
            .map(|pos| pos.char_idx)
            .collect::<Vec<_>>(),
        vec![4, 9]
    );
    assert!(buffer.undo());
    assert_eq!(buffer.text(), "abcd");
    assert!(!buffer.undo());
    assert!(buffer.redo());
    assert_eq!(buffer.text(), "axyzbcxyzd");
}

#[test]
fn typing_coalescing_stops_at_word_boundaries() {
    let mut buffer = TextBuffer::new_untitled(1);

    buffer.insert_at_cursor("a");
    buffer.insert_at_cursor(" ");
    buffer.insert_at_cursor("b");

    assert_eq!(buffer.text(), "a b");
    assert!(buffer.undo());
    assert_eq!(buffer.text(), "a ");
    assert!(buffer.undo());
    assert_eq!(buffer.text(), "a");
}

#[test]
fn typing_coalescing_respects_cursor_moves() {
    let mut buffer = TextBuffer::new_untitled(1);

    buffer.insert_at_cursor("a");
    buffer.set_single_cursor(0);
    buffer.insert_at_cursor("b");

    assert_eq!(buffer.text(), "ba");
    assert!(buffer.undo());
    assert_eq!(buffer.text(), "a");
    assert!(buffer.undo());
    assert_eq!(buffer.text(), "");
}

#[test]
fn delete_coalescing_backspace_merges_plain_char_deletes_into_one_undo_step() {
    let mut buffer = TextBuffer::from_text(1, None, "abcd".to_owned());
    buffer.set_single_cursor(buffer.len_chars());

    assert!(buffer.delete_backward());
    assert!(buffer.delete_backward());
    assert!(buffer.delete_backward());

    assert_eq!(buffer.text(), "a");
    assert!(buffer.undo());
    assert_eq!(buffer.text(), "abcd");
    assert!(!buffer.undo());
    assert!(buffer.redo());
    assert_eq!(buffer.text(), "a");
}

#[test]
fn delete_coalescing_forward_merges_plain_char_deletes_into_one_undo_step() {
    let mut buffer = TextBuffer::from_text(1, None, "abcd".to_owned());
    buffer.set_single_cursor(1);

    assert!(buffer.delete_forward());
    assert!(buffer.delete_forward());

    assert_eq!(buffer.text(), "ad");
    assert!(buffer.undo());
    assert_eq!(buffer.text(), "abcd");
    assert!(!buffer.undo());
    assert!(buffer.redo());
    assert_eq!(buffer.text(), "ad");
}

#[test]
fn delete_coalescing_stops_after_cursor_move() {
    let mut buffer = TextBuffer::from_text(1, None, "abcd".to_owned());
    buffer.set_single_cursor(buffer.len_chars());

    assert!(buffer.delete_backward());
    buffer.set_single_cursor(1);
    assert!(buffer.delete_backward());

    assert_eq!(buffer.text(), "bc");
    assert!(buffer.undo());
    assert_eq!(buffer.text(), "abc");
    assert!(buffer.undo());
    assert_eq!(buffer.text(), "abcd");
}

#[test]
fn delete_coalescing_skips_selection_and_pair_deletes() {
    let mut selection = TextBuffer::from_text(1, None, "abcd".to_owned());
    selection.set_selection(1, 3);

    assert!(selection.delete_backward());
    assert!(selection.delete_backward());

    assert_eq!(selection.text(), "d");
    assert!(selection.undo());
    assert_eq!(selection.text(), "ad");
    assert!(selection.undo());
    assert_eq!(selection.text(), "abcd");

    let mut pair = TextBuffer::from_text(1, None, "()x".to_owned());
    pair.set_single_cursor(1);

    assert!(pair.delete_backward());
    assert!(pair.delete_forward());

    assert_eq!(pair.text(), "");
    assert!(pair.undo());
    assert_eq!(pair.text(), "x");
    assert!(pair.undo());
    assert_eq!(pair.text(), "()x");
}

#[test]
fn delete_coalescing_skips_newline_deletes() {
    let mut buffer = TextBuffer::from_text(1, None, "a\nb".to_owned());
    buffer.set_single_cursor(2);

    assert!(buffer.delete_backward());
    assert!(buffer.delete_backward());

    assert_eq!(buffer.text(), "b");
    assert!(buffer.undo());
    assert_eq!(buffer.text(), "ab");
    assert!(buffer.undo());
    assert_eq!(buffer.text(), "a\nb");
}

#[test]
fn delete_coalescing_does_not_merge_generic_apply_edits() {
    let mut buffer = TextBuffer::from_text(1, None, "abcd".to_owned());
    buffer.set_single_cursor(buffer.len_chars());

    assert!(buffer.delete_backward());
    assert!(buffer.apply_edits(vec![TextEdit {
        range: 2..3,
        inserted: String::new(),
    }]));

    assert_eq!(buffer.text(), "ab");
    assert!(buffer.undo());
    assert_eq!(buffer.text(), "abc");
    assert!(buffer.undo());
    assert_eq!(buffer.text(), "abcd");
}

#[test]
fn delete_coalescing_history_snapshot_replays_merged_delete_runs() {
    let mut backspace = TextBuffer::from_text(1, None, "abcd".to_owned());
    backspace.set_single_cursor(backspace.len_chars());
    assert!(backspace.delete_backward());
    assert!(backspace.delete_backward());
    let backspace_snapshot = backspace.history_snapshot(16, 4096).unwrap();

    let mut restored_backspace = TextBuffer::from_text(2, None, "ab".to_owned());
    assert!(restored_backspace.restore_history_snapshot(backspace_snapshot));
    assert!(restored_backspace.undo());
    assert_eq!(restored_backspace.text(), "abcd");
    assert!(restored_backspace.redo());
    assert_eq!(restored_backspace.text(), "ab");

    let mut forward = TextBuffer::from_text(3, None, "abcd".to_owned());
    forward.set_single_cursor(1);
    assert!(forward.delete_forward());
    assert!(forward.delete_forward());
    let forward_snapshot = forward.history_snapshot(16, 4096).unwrap();

    let mut restored_forward = TextBuffer::from_text(4, None, "ad".to_owned());
    assert!(restored_forward.restore_history_snapshot(forward_snapshot));
    assert!(restored_forward.undo());
    assert_eq!(restored_forward.text(), "abcd");
    assert!(restored_forward.redo());
    assert_eq!(restored_forward.text(), "ad");
}

#[test]
fn undo_history_is_bounded() {
    let mut buffer = TextBuffer::new_untitled(1);
    for _ in 0..(MAX_UNDO_ENTRIES + 20) {
        let cursor = buffer.len_chars();
        buffer.apply_edit(TextEdit {
            range: cursor..cursor,
            inserted: "x".to_owned(),
        });
    }

    let mut undo_count = 0;
    while buffer.undo() {
        undo_count += 1;
    }

    assert_eq!(undo_count, MAX_UNDO_ENTRIES);
    assert_eq!(buffer.len_chars(), 20);
}

#[test]
fn apply_edits_rejects_stale_ranges_without_mutating() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha".to_owned());
    let version = buffer.version();

    assert!(!buffer.apply_edits(vec![TextEdit {
        range: 99..99,
        inserted: "!".to_owned(),
    }]));
    assert_eq!(buffer.text(), "alpha");
    assert_eq!(buffer.version(), version);
    assert!(!buffer.undo());

    assert_eq!(
        buffer.replace_range_with_options(99..100, "omega", true),
        None
    );
    assert_eq!(buffer.text(), "alpha");
}

#[test]
fn apply_edits_rejects_overlapping_conflicting_ranges_without_partial_apply() {
    let mut buffer = TextBuffer::from_text(1, None, "abcdef".to_owned());
    let version = buffer.version();

    assert!(!buffer.apply_edits(vec![
        TextEdit {
            range: 1..4,
            inserted: "X".to_owned(),
        },
        TextEdit {
            range: 3..5,
            inserted: "Y".to_owned(),
        },
    ]));

    assert_eq!(buffer.text(), "abcdef");
    assert_eq!(buffer.version(), version);
    assert!(!buffer.undo());
}

#[test]
fn history_snapshot_restores_undo_redo_for_matching_text() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha".to_owned());
    buffer.set_single_cursor(buffer.len_chars());
    buffer.insert_at_cursor(" beta");
    assert!(buffer.undo());
    assert_eq!(buffer.text(), "alpha");
    let snapshot = buffer.history_snapshot(16, 4096).unwrap();

    let mut restored = TextBuffer::from_text(2, None, "alpha".to_owned());
    assert!(restored.restore_history_snapshot(snapshot));

    assert!(restored.redo());
    assert_eq!(restored.text(), "alpha beta");
    assert!(restored.undo());
    assert_eq!(restored.text(), "alpha");
}

#[test]
fn history_snapshot_rejects_mismatched_text() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha".to_owned());
    buffer.insert_at_cursor(" beta");
    let snapshot = buffer.history_snapshot(16, 4096).unwrap();

    let mut restored = TextBuffer::from_text(2, None, "changed".to_owned());

    assert!(!restored.restore_history_snapshot(snapshot));
    assert!(!restored.undo());
}

#[test]
fn history_snapshot_rejects_invalid_undo_ranges() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha".to_owned());
    buffer.set_single_cursor(buffer.len_chars());
    buffer.insert_at_cursor(" beta");
    let mut snapshot = buffer.history_snapshot(16, 4096).unwrap();
    snapshot.undo[0].inverses[0].start = 99;
    snapshot.undo[0].inverses[0].end = 100;

    let mut restored = TextBuffer::from_text(2, None, "alpha beta".to_owned());

    assert!(!restored.restore_history_snapshot(snapshot));
    assert!(!restored.undo());
}

#[test]
fn history_snapshot_rejects_invalid_redo_ranges() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha".to_owned());
    buffer.set_single_cursor(buffer.len_chars());
    buffer.insert_at_cursor(" beta");
    assert!(buffer.undo());
    let mut snapshot = buffer.history_snapshot(16, 4096).unwrap();
    snapshot.redo[0].edits[0].start = 99;
    snapshot.redo[0].edits[0].end = 99;

    let mut restored = TextBuffer::from_text(2, None, "alpha".to_owned());

    assert!(!restored.restore_history_snapshot(snapshot));
    assert!(!restored.redo());
}

#[test]
fn history_snapshot_rejects_empty_selection_lists() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha".to_owned());
    buffer.set_single_cursor(buffer.len_chars());
    buffer.insert_at_cursor(" beta");
    let mut snapshot = buffer.history_snapshot(16, 4096).unwrap();
    snapshot.undo[0].selections_before.clear();

    let mut restored = TextBuffer::from_text(2, None, "alpha beta".to_owned());

    assert!(!restored.restore_history_snapshot(snapshot));
    assert!(!restored.undo());
}

#[test]
fn history_snapshot_rejects_unnormalized_selection_lists() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha".to_owned());
    buffer.set_single_cursor(buffer.len_chars());
    buffer.insert_at_cursor(" beta");
    let mut snapshot = buffer.history_snapshot(16, 4096).unwrap();
    snapshot.undo[0].selections_after = vec![Selection::caret(7), Selection::caret(3)];

    let mut restored = TextBuffer::from_text(2, None, "alpha beta".to_owned());

    assert!(!restored.restore_history_snapshot(snapshot));
    assert!(!restored.undo());
}

#[test]
fn undo_rejects_stale_live_history_without_clamping_ranges() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha".to_owned());
    buffer.set_single_cursor(buffer.len_chars());
    buffer.insert_at_cursor(" beta");
    buffer.undo[0].inverses[0].range = 99..100;

    assert!(!buffer.undo());
    assert_eq!(buffer.text(), "alpha beta");
    assert_eq!(buffer.undo.len(), 1);
    assert!(buffer.redo.is_empty());
}

#[test]
fn redo_rejects_stale_live_history_without_clamping_ranges() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha".to_owned());
    buffer.set_single_cursor(buffer.len_chars());
    buffer.insert_at_cursor(" beta");
    assert!(buffer.undo());
    buffer.redo[0].edits[0].range = 99..99;

    assert!(!buffer.redo());
    assert_eq!(buffer.text(), "alpha");
    assert_eq!(buffer.redo.len(), 1);
    assert!(buffer.undo.is_empty());
}

#[test]
fn history_snapshot_rejects_stale_undo_redo_edit_pair() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha".to_owned());
    buffer.set_single_cursor(buffer.len_chars());
    buffer.insert_at_cursor(" beta");
    let mut snapshot = buffer.history_snapshot(16, 4096).unwrap();
    snapshot.undo[0].edits[0].inserted = " zeta".to_owned();

    let mut restored = TextBuffer::from_text(2, None, "alpha beta".to_owned());

    assert!(!restored.restore_history_snapshot(snapshot));
    assert!(!restored.undo());
}

#[test]
fn history_snapshot_rejects_stale_redo_undo_inverse_pair() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha".to_owned());
    buffer.set_single_cursor(buffer.len_chars());
    buffer.insert_at_cursor(" beta");
    assert!(buffer.undo());
    let mut snapshot = buffer.history_snapshot(16, 4096).unwrap();
    snapshot.redo[0].inverses[0].inserted = "stale".to_owned();

    let mut restored = TextBuffer::from_text(2, None, "alpha".to_owned());

    assert!(!restored.restore_history_snapshot(snapshot));
    assert!(!restored.redo());
}

#[test]
fn history_snapshot_keeps_recent_replayable_entries_within_byte_budget() {
    let mut buffer = TextBuffer::new_untitled(1);
    for text in ["a", "b", "c"] {
        let cursor = buffer.len_chars();
        buffer.apply_edit(TextEdit {
            range: cursor..cursor,
            inserted: text.to_owned(),
        });
    }

    let snapshot = buffer.history_snapshot(16, 140).unwrap();

    assert_eq!(snapshot.undo.len(), 1);
    let mut restored = TextBuffer::from_text(2, None, "abc".to_owned());
    assert!(restored.restore_history_snapshot(snapshot));
    assert!(restored.undo());
    assert_eq!(restored.text(), "ab");
    assert!(!restored.undo());
}

#[test]
fn read_only_buffers_reject_text_mutations_and_history_replay() {
    let mut buffer = TextBuffer::from_text(1, None, "alpha".to_owned());
    buffer.set_single_cursor(buffer.len_chars());
    buffer.insert_at_cursor(" beta");
    assert_eq!(buffer.text(), "alpha beta");

    buffer.set_read_only(true);
    assert!(!buffer.undo());
    assert_eq!(buffer.text(), "alpha beta");
    assert!(!buffer.insert_text_with_auto_pairs("!"));
    assert_eq!(buffer.text(), "alpha beta");
    assert!(!buffer.apply_edits(vec![TextEdit {
        range: 0..5,
        inserted: "omega".to_owned(),
    }]));
    assert_eq!(buffer.text(), "alpha beta");
}

#[test]
fn apply_edits_with_inserted_selection_tracks_primary_insert_after_prior_edits() {
    let mut buffer = TextBuffer::from_text(1, None, "fn main() {\n    pri\n}\n".to_owned());
    let import = TextEdit {
        range: 0..0,
        inserted: "use std::fmt;\n".to_owned(),
    };
    let primary = TextEdit {
        range: 16..19,
        inserted: "println!(value);".to_owned(),
    };

    assert!(buffer.apply_edits_with_inserted_selection(
        vec![import, primary.clone()],
        &primary,
        9..14
    ));

    assert_eq!(
        buffer.text(),
        "use std::fmt;\nfn main() {\n    println!(value);\n}\n"
    );
    assert_eq!(
        buffer.selections(),
        &[Selection {
            anchor: 39,
            cursor: 44
        }]
    );
    assert!(buffer.undo());
    assert_eq!(buffer.text(), "fn main() {\n    pri\n}\n");
    assert!(buffer.redo());
    assert_eq!(
        buffer.selections(),
        &[Selection {
            anchor: 39,
            cursor: 44
        }]
    );
}
