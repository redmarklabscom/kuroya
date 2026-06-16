use super::*;

#[test]
fn terminal_direct_input_sends_bytes_to_active_session_pty() {
    let size = test_terminal_size();
    let (mut pane, rx_command) = pane_with_command_session(size);

    pane.send_input("Get-Location\r");

    match rx_command.try_recv().unwrap() {
        TerminalCommand::Input(input) => assert_eq!(input, "Get-Location\r"),
        TerminalCommand::Resize(_) => panic!("expected terminal input"),
        TerminalCommand::Close => panic!("expected terminal input"),
    }
}

#[test]
fn terminal_direct_input_does_not_stop_session_when_command_queue_is_full() {
    let size = test_terminal_size();
    let (tx_command, rx_command) = bounded(1);
    tx_command
        .send(TerminalCommand::Input("queued".to_owned()))
        .unwrap();
    let mut pane = pane_with_session(session_with_command(1, size, tx_command), size);

    pane.send_input("dropped\r");

    assert!(pane.sessions[0].started);
    assert!(pane.sessions[0].tx_command.is_some());
    match rx_command.try_recv().unwrap() {
        TerminalCommand::Input(input) => assert_eq!(input, "queued"),
        TerminalCommand::Resize(_) | TerminalCommand::Close => panic!("expected queued input"),
    }
    assert!(rx_command.try_recv().is_err());
}

#[test]
fn terminal_direct_input_stops_session_when_command_channel_disconnects() {
    let size = test_terminal_size();
    let (tx_command, rx_command) = bounded(1);
    drop(rx_command);
    let mut pane = pane_with_session(session_with_command(1, size, tx_command), size);

    pane.send_input("Get-Location\r");

    assert!(!pane.sessions[0].started);
    assert!(pane.sessions[0].tx_command.is_none());
}

#[test]
fn terminal_paste_input_wraps_bracketed_paste_when_shell_requests_it() {
    assert_eq!(
        terminal_paste_input("hello".to_owned(), true, false),
        "\x1b[200~hello\x1b[201~"
    );
    assert_eq!(
        terminal_paste_input("hello".to_owned(), true, true),
        "hello"
    );
    assert_eq!(
        terminal_paste_input("hello".to_owned(), false, false),
        "hello"
    );
}

#[test]
fn terminal_alt_click_cursor_input_moves_by_terminal_cell_offset() {
    let cursor = TerminalCellPosition { row: 2, col: 4 };
    let target = TerminalCellPosition { row: 1, col: 8 };

    assert_eq!(
        terminal_alt_click_cursor_input(cursor, target, 10).unwrap(),
        "\x1b[D".repeat(6)
    );
    assert_eq!(
        terminal_alt_click_cursor_input(target, cursor, 10).unwrap(),
        "\x1b[C".repeat(6)
    );
    assert_eq!(terminal_alt_click_cursor_input(cursor, cursor, 10), None);
    assert_eq!(terminal_alt_click_cursor_input(cursor, target, 0), None);
}

#[test]
fn terminal_send_paste_input_respects_runtime_bracketed_paste_mode() {
    let size = test_terminal_size();
    let (mut pane, rx_command) = pane_with_command_session(size);
    pane.sessions[0].parser.process(b"\x1b[?2004h");

    pane.send_paste_input(0, "hello\nworld");

    match rx_command.try_recv().unwrap() {
        TerminalCommand::Input(input) => {
            assert_eq!(input, "\x1b[200~hello\nworld\x1b[201~")
        }
        TerminalCommand::Resize(_) | TerminalCommand::Close => panic!("expected terminal paste"),
    }
}

#[test]
fn terminal_send_paste_input_can_ignore_runtime_bracketed_paste_mode() {
    let size = test_terminal_size();
    let (mut pane, rx_command) = pane_with_command_session(size);
    pane.sessions[0].parser.process(b"\x1b[?2004h");
    pane.set_ignore_bracketed_paste_mode(true);

    pane.send_paste_input(0, "hello\nworld");

    match rx_command.try_recv().unwrap() {
        TerminalCommand::Input(input) => assert_eq!(input, "hello\nworld"),
        TerminalCommand::Resize(_) | TerminalCommand::Close => panic!("expected terminal paste"),
    }
}

#[test]
fn terminal_multiline_paste_warning_setting_is_configurable() {
    let mut pane = TerminalPane::new(
        PathBuf::from("workspace"),
        TERMINAL_SCROLLBACK_ROWS,
        DEFAULT_TERMINAL_FONT_SIZE,
        DEFAULT_TERMINAL_LINE_HEIGHT,
    );

    pane.set_multi_line_paste_warning(TerminalMultiLinePasteWarning::Never);

    assert_eq!(
        pane.multi_line_paste_warning,
        TerminalMultiLinePasteWarning::Never
    );
}

#[test]
fn terminal_multiline_paste_warning_queues_paste_until_confirmed() {
    let size = test_terminal_size();
    let (mut pane, rx_command) = pane_with_command_session(size);

    pane.paste_text(0, "one\ntwo".to_owned());

    assert!(pane.pending_multiline_paste.is_some());
    assert_eq!(pane.pending_multiline_paste_line_count(), Some(2));
    assert!(rx_command.try_recv().is_err());

    pane.confirm_pending_multiline_paste();

    match rx_command.try_recv().unwrap() {
        TerminalCommand::Input(input) => assert_eq!(input, "one\ntwo"),
        TerminalCommand::Resize(_) | TerminalCommand::Close => panic!("expected terminal paste"),
    }
    assert!(pane.pending_multiline_paste.is_none());
}

#[test]
fn terminal_multiline_paste_warning_auto_allows_bracketed_paste_without_prompt() {
    let size = test_terminal_size();
    let (mut pane, rx_command) = pane_with_command_session(size);
    pane.sessions[0].parser.process(b"\x1b[?2004h");

    pane.paste_text(0, "one\ntwo".to_owned());

    assert!(pane.pending_multiline_paste.is_none());
    match rx_command.try_recv().unwrap() {
        TerminalCommand::Input(input) => assert_eq!(input, "\x1b[200~one\ntwo\x1b[201~"),
        TerminalCommand::Resize(_) | TerminalCommand::Close => panic!("expected terminal paste"),
    }
}

#[test]
fn terminal_multiline_paste_warning_never_pastes_multiline_without_prompt() {
    let size = test_terminal_size();
    let (mut pane, rx_command) = pane_with_command_session(size);
    pane.set_multi_line_paste_warning(TerminalMultiLinePasteWarning::Never);

    pane.paste_text(0, "one\ntwo".to_owned());

    assert!(pane.pending_multiline_paste.is_none());
    match rx_command.try_recv().unwrap() {
        TerminalCommand::Input(input) => assert_eq!(input, "one\ntwo"),
        TerminalCommand::Resize(_) | TerminalCommand::Close => panic!("expected terminal paste"),
    }
}

#[test]
fn terminal_send_alt_click_cursor_input_sends_left_right_cursor_sequence() {
    let size = test_terminal_size();
    let (mut pane, rx_command) = pane_with_command_session(size);
    pane.sessions[0].parser.process(b"\x1b[3;5H");

    pane.send_alt_click_cursor_input(0, TerminalCellPosition { row: 2, col: 8 });

    match rx_command.try_recv().unwrap() {
        TerminalCommand::Input(input) => assert_eq!(input, "\x1b[C".repeat(4)),
        TerminalCommand::Resize(_) | TerminalCommand::Close => panic!("expected cursor input"),
    }
}

#[test]
fn terminal_close_last_session_hides_pane_without_creating_replacement() {
    let size = test_terminal_size();
    let (mut pane, rx_command) = pane_with_command_session(size);

    pane.close_active_session();

    assert!(!pane.visible);
    assert!(pane.sessions.is_empty());
    assert_eq!(pane.active_session, 0);
    match rx_command.try_recv().unwrap() {
        TerminalCommand::Close => {}
        TerminalCommand::Input(_) | TerminalCommand::Resize(_) => panic!("expected close command"),
    }
}

#[test]
fn terminal_close_last_session_can_keep_empty_pane_open() {
    let size = test_terminal_size();
    let (mut pane, rx_command) = pane_with_command_session(size);
    pane.set_hide_on_last_closed(false);

    pane.close_active_session();

    assert!(pane.visible);
    assert!(pane.sessions.is_empty());
    assert_eq!(pane.active_session, 0);
    assert!(!pane.split_view);
    match rx_command.try_recv().unwrap() {
        TerminalCommand::Close => {}
        TerminalCommand::Input(_) | TerminalCommand::Resize(_) => panic!("expected close command"),
    }
}

#[test]
fn terminal_request_close_prompts_when_confirm_on_kill_applies_to_panel() {
    let size = test_terminal_size();
    let (mut pane, rx_command) = pane_with_command_session(size);
    pane.set_confirm_on_kill(TerminalConfirmOnKill::Panel);

    pane.request_close_active_session();

    assert_eq!(pane.pending_kill_session_id, Some(1));
    assert_eq!(pane.sessions.len(), 1);
    assert!(rx_command.try_recv().is_err());

    pane.confirm_pending_kill();

    assert!(pane.sessions.is_empty());
    assert_eq!(pane.pending_kill_session_id, None);
    match rx_command.try_recv().unwrap() {
        TerminalCommand::Close => {}
        TerminalCommand::Input(_) | TerminalCommand::Resize(_) => panic!("expected close command"),
    }
}

#[test]
fn terminal_request_close_uses_default_editor_scope_without_panel_prompt() {
    let size = test_terminal_size();
    let (mut pane, rx_command) = pane_with_command_session(size);

    pane.request_close_active_session();

    assert!(pane.sessions.is_empty());
    assert_eq!(pane.pending_kill_session_id, None);
    match rx_command.try_recv().unwrap() {
        TerminalCommand::Close => {}
        TerminalCommand::Input(_) | TerminalCommand::Resize(_) => panic!("expected close command"),
    }
}

#[test]
fn terminal_close_active_session_keeps_panel_open_when_other_sessions_remain() {
    let size = test_terminal_size();
    let (first_tx, _first_rx) = unbounded();
    let (second_tx, second_rx) = unbounded();
    let first = session_with_command(1, size, first_tx);
    let second = session_with_command(2, size, second_tx);
    let mut pane = pane_with_sessions(vec![first, second], size);
    pane.active_session = 1;
    pane.split_view = true;

    pane.close_active_session();

    assert!(pane.visible);
    assert_eq!(pane.sessions.len(), 1);
    assert_eq!(pane.sessions[0].id, 1);
    assert_eq!(pane.active_session, 0);
    assert!(!pane.split_view);
    assert!(pane.focus_input_on_show);
    match second_rx.try_recv().unwrap() {
        TerminalCommand::Close => {}
        TerminalCommand::Input(_) | TerminalCommand::Resize(_) => panic!("expected close command"),
    }
}

#[test]
fn terminal_close_session_by_id_targets_specific_session() {
    let size = test_terminal_size();
    let (first_tx, first_rx) = unbounded();
    let (second_tx, second_rx) = unbounded();
    let first = session_with_command(1, size, first_tx);
    let second = session_with_command(2, size, second_tx);
    let mut pane = pane_with_sessions(vec![first, second], size);
    pane.active_session = 0;
    pane.split_view = true;

    assert!(pane.close_session_by_id(2));

    assert!(pane.visible);
    assert_eq!(pane.sessions.len(), 1);
    assert_eq!(pane.sessions[0].id, 1);
    assert_eq!(pane.active_session, 0);
    assert!(!pane.split_view);
    assert!(first_rx.try_recv().is_err());
    match second_rx.try_recv().unwrap() {
        TerminalCommand::Close => {}
        TerminalCommand::Input(_) | TerminalCommand::Resize(_) => panic!("expected close command"),
    }
    assert!(!pane.close_session_by_id(2));
}

#[test]
fn terminal_close_background_session_before_active_preserves_active_session() {
    let size = test_terminal_size();
    let (first_tx, first_rx) = unbounded();
    let (second_tx, second_rx) = unbounded();
    let (third_tx, third_rx) = unbounded();
    let first = session_with_command(1, size, first_tx);
    let second = session_with_command(2, size, second_tx);
    let third = session_with_command(3, size, third_tx);
    let mut pane = pane_with_sessions(vec![first, second, third], size);
    pane.active_session = 2;

    assert!(pane.close_session_by_id(1));

    assert_eq!(
        pane.sessions
            .iter()
            .map(|session| session.id)
            .collect::<Vec<_>>(),
        vec![2, 3]
    );
    assert_eq!(pane.active_session, 1);
    assert_eq!(pane.sessions[pane.active_session].id, 3);
    match first_rx.try_recv().unwrap() {
        TerminalCommand::Close => {}
        TerminalCommand::Input(_) | TerminalCommand::Resize(_) => panic!("expected close command"),
    }
    assert!(second_rx.try_recv().is_err());
    assert!(third_rx.try_recv().is_err());
}

#[test]
fn terminal_close_background_session_after_active_preserves_active_session() {
    let size = test_terminal_size();
    let (first_tx, first_rx) = unbounded();
    let (second_tx, second_rx) = unbounded();
    let (third_tx, third_rx) = unbounded();
    let first = session_with_command(1, size, first_tx);
    let second = session_with_command(2, size, second_tx);
    let third = session_with_command(3, size, third_tx);
    let mut pane = pane_with_sessions(vec![first, second, third], size);
    pane.active_session = 0;

    assert!(pane.close_session_by_id(3));

    assert_eq!(
        pane.sessions
            .iter()
            .map(|session| session.id)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(pane.active_session, 0);
    assert_eq!(pane.sessions[pane.active_session].id, 1);
    assert!(first_rx.try_recv().is_err());
    assert!(second_rx.try_recv().is_err());
    match third_rx.try_recv().unwrap() {
        TerminalCommand::Close => {}
        TerminalCommand::Input(_) | TerminalCommand::Resize(_) => panic!("expected close command"),
    }
}

#[test]
fn terminal_close_session_by_id_uses_out_of_band_signal_when_command_queue_is_full() {
    let size = test_terminal_size();
    let (first_tx, first_rx) = unbounded();
    let first = session_with_command(1, size, first_tx);
    let (second_tx, second_rx) = bounded(1);
    second_tx
        .send(TerminalCommand::Input("queued".to_owned()))
        .unwrap();
    let (second_close_tx, second_close_rx) = terminal_close_channel();
    let second = TerminalSession {
        tx_command: Some(second_tx),
        tx_close: Some(second_close_tx),
        ..session_without_command(2, size)
    };
    let close_requested = second.close_requested.clone();
    let mut pane = pane_with_sessions(vec![first, second], size);

    assert!(pane.close_session_by_id(2));

    assert_eq!(pane.sessions.len(), 1);
    assert_eq!(pane.sessions[0].id, 1);
    assert!(first_rx.try_recv().is_err());
    assert!(second_close_rx.try_recv().is_ok());
    assert!(close_requested.load(std::sync::atomic::Ordering::SeqCst));
    match second_rx.try_recv().unwrap() {
        TerminalCommand::Input(input) => assert_eq!(input, "queued"),
        TerminalCommand::Resize(_) | TerminalCommand::Close => panic!("expected queued input"),
    }
    assert!(second_rx.try_recv().is_err());
}

#[test]
fn terminal_close_all_sessions_for_shutdown_uses_out_of_band_signal_when_command_queues_are_full() {
    let size = test_terminal_size();
    let (first_tx, first_rx) = bounded(1);
    first_tx
        .send(TerminalCommand::Input("queued-1".to_owned()))
        .unwrap();
    let (first_close_tx, first_close_rx) = terminal_close_channel();
    let first = TerminalSession {
        tx_command: Some(first_tx),
        tx_close: Some(first_close_tx),
        ..session_without_command(1, size)
    };
    let first_close_requested = first.close_requested.clone();

    let (second_tx, second_rx) = bounded(1);
    second_tx
        .send(TerminalCommand::Input("queued-2".to_owned()))
        .unwrap();
    let (second_close_tx, second_close_rx) = terminal_close_channel();
    let second = TerminalSession {
        tx_command: Some(second_tx),
        tx_close: Some(second_close_tx),
        ..session_without_command(2, size)
    };
    let second_close_requested = second.close_requested.clone();

    let mut pane = pane_with_sessions(vec![first, second], size);
    pane.split_view = true;
    pane.pending_kill_session_id = Some(2);

    pane.close_all_sessions_for_shutdown();

    assert!(pane.sessions.is_empty());
    assert!(pane.split_weights.is_empty());
    assert!(!pane.visible);
    assert!(!pane.split_view);
    assert_eq!(pane.pending_kill_session_id, None);
    assert!(first_close_rx.try_recv().is_ok());
    assert!(second_close_rx.try_recv().is_ok());
    assert!(first_close_requested.load(std::sync::atomic::Ordering::SeqCst));
    assert!(second_close_requested.load(std::sync::atomic::Ordering::SeqCst));
    match first_rx.try_recv().unwrap() {
        TerminalCommand::Input(input) => assert_eq!(input, "queued-1"),
        TerminalCommand::Resize(_) | TerminalCommand::Close => panic!("expected queued input"),
    }
    match second_rx.try_recv().unwrap() {
        TerminalCommand::Input(input) => assert_eq!(input, "queued-2"),
        TerminalCommand::Resize(_) | TerminalCommand::Close => panic!("expected queued input"),
    }
    assert!(first_rx.try_recv().is_err());
    assert!(second_rx.try_recv().is_err());
}

#[test]
fn terminal_close_session_clears_state_owned_by_closed_session() {
    let size = test_terminal_size();
    let first = session_without_command(1, size);
    let second = session_without_command(2, size);
    let mut pane = pane_with_sessions(vec![first, second], size);
    pane.pending_paste_session_id = Some(2);
    pane.pending_kill_session_id = Some(2);
    pane.pending_multiline_paste = Some(TerminalPendingPaste {
        session_id: 2,
        text: "one\ntwo".to_owned(),
    });
    pane.selected_session_id = Some(2);
    pane.selected_text = Some(TerminalTextSelection {
        session_id: 2,
        text: "selected".to_owned(),
        range: TerminalSelectionRange {
            start: TerminalCellPosition { row: 0, col: 0 },
            end: TerminalCellPosition { row: 0, col: 8 },
        },
    });
    pane.selection_drag = Some(TerminalSelectionDrag {
        session_id: 2,
        anchor: TerminalCellPosition { row: 0, col: 0 },
    });

    assert!(pane.close_session_by_id(2));

    assert_eq!(pane.sessions.len(), 1);
    assert_eq!(pane.sessions[0].id, 1);
    assert_eq!(pane.pending_paste_session_id, None);
    assert_eq!(pane.pending_kill_session_id, None);
    assert!(pane.pending_multiline_paste.is_none());
    assert_eq!(pane.selected_session_id, None);
    assert!(pane.selected_text.is_none());
    assert!(pane.selection_drag.is_none());
}

#[test]
fn terminal_prune_stale_session_state_clears_only_invalid_session_state() {
    let size = test_terminal_size();
    let live = session_without_command(1, size);
    let mut stopped = session_without_command(2, size);
    stopped.started = false;
    let mut pane = pane_with_sessions(vec![live, stopped], size);
    pane.pending_paste_session_id = Some(1);
    pane.pending_kill_session_id = Some(2);
    pane.pending_multiline_paste = Some(TerminalPendingPaste {
        session_id: 99,
        text: "one\ntwo".to_owned(),
    });
    pane.selected_session_id = Some(1);
    pane.selected_text = Some(TerminalTextSelection {
        session_id: 99,
        text: "selected".to_owned(),
        range: TerminalSelectionRange {
            start: TerminalCellPosition { row: 0, col: 0 },
            end: TerminalCellPosition { row: 0, col: 8 },
        },
    });
    pane.selection_drag = Some(TerminalSelectionDrag {
        session_id: 1,
        anchor: TerminalCellPosition { row: 0, col: 0 },
    });

    pane.prune_stale_session_state();

    assert_eq!(pane.pending_paste_session_id, Some(1));
    assert_eq!(pane.pending_kill_session_id, None);
    assert!(pane.pending_multiline_paste.is_none());
    assert_eq!(pane.selected_session_id, Some(1));
    assert!(pane.selected_text.is_none());
    assert_eq!(
        pane.selection_drag,
        Some(TerminalSelectionDrag {
            session_id: 1,
            anchor: TerminalCellPosition { row: 0, col: 0 },
        })
    );
}

#[test]
fn terminal_prune_stale_session_state_clears_stale_search_cache() {
    let size = test_terminal_size();
    let mut pane = pane_with_session(session_with_output(1, size, b"alpha\n"), size);
    pane.search_query = "alpha".to_owned();
    pane.search_cache = TerminalSearchCache {
        scope: TerminalSearchCacheScope::Single {
            session_id: 99,
            generation: 0,
        },
        query: "alpha".to_owned(),
        matches: vec![TerminalSearchMatch {
            session_id: 99,
            line: 0,
            start: 0,
            end: 5,
            preview: std::sync::Arc::new("stale".to_owned()),
        }],
        progress: Default::default(),
    };
    pane.search_match = 8;

    pane.prune_stale_session_state();

    assert_eq!(pane.search_cache.scope, TerminalSearchCacheScope::Empty);
    assert!(pane.search_cache.query.is_empty());
    assert!(pane.search_cache.matches.is_empty());
    assert_eq!(pane.search_match, 0);
}

#[test]
fn terminal_prune_stale_session_state_keeps_live_search_cache_after_append() {
    let size = test_terminal_size();
    let mut pane = pane_with_session(session_with_output(1, size, b"alpha one\n"), size);
    pane.search_query = "alpha".to_owned();
    assert_eq!(pane.active_terminal_search_matches().len(), 1);
    let retained_preview = pane.search_cache.matches[0].preview.clone();

    pane.sessions[0].append_search_output(b"alpha two\n");
    pane.prune_stale_session_state();

    assert_eq!(pane.search_cache.query, "alpha");
    assert_eq!(pane.search_cache.matches.len(), 1);
    assert!(std::sync::Arc::ptr_eq(
        &retained_preview,
        &pane.search_cache.matches[0].preview
    ));

    let matches = pane.active_terminal_search_matches();
    assert_eq!(matches.len(), 2);
    assert!(std::sync::Arc::ptr_eq(
        &retained_preview,
        &matches[0].preview
    ));
}

#[test]
fn terminal_prunes_kill_prompt_when_session_stops() {
    let size = test_terminal_size();
    let mut pane = pane_with_session(session_without_command(1, size), size);
    pane.pending_kill_session_id = Some(1);
    pane.sessions[0]
        .tx_output
        .send(TerminalEvent::Finished {
            message: None,
            process_exit_code: None,
            reason: TerminalFinishReason::ProcessExit,
        })
        .unwrap();

    assert_eq!(pane.drain_output(), 1);

    assert_eq!(pane.pending_kill_session_id, None);
    assert!(!pane.sessions[0].started);
}
