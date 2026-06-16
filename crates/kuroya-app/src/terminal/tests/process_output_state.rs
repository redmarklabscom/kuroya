use super::*;

#[test]
fn terminal_exit_confirmation_count_follows_setting_and_running_sessions() {
    let size = test_terminal_size();
    let (mut pane, _rx_command) = pane_with_command_session(size);

    assert_eq!(
        pane.exit_confirmation_session_count(TerminalConfirmOnExit::Never),
        0
    );
    assert_eq!(
        pane.exit_confirmation_session_count(TerminalConfirmOnExit::Always),
        1
    );
    assert_eq!(
        pane.exit_confirmation_session_count(TerminalConfirmOnExit::HasChildProcesses),
        1
    );

    pane.sessions[0].mark_stopped();

    assert_eq!(
        pane.exit_confirmation_session_count(TerminalConfirmOnExit::Always),
        0
    );
}

#[test]
fn terminal_finished_event_marks_active_session_restartable() {
    let size = test_terminal_size();
    let (mut pane, _) = pane_with_command_session(size);

    pane.sessions[0]
        .tx_output
        .send(TerminalEvent::Finished {
            message: Some("\r\nprocess exited\r\n".to_owned()),
            process_exit_code: None,
            reason: TerminalFinishReason::ProcessExit,
        })
        .unwrap();
    pane.drain_output();

    assert!(!pane.sessions[0].started);
    assert!(pane.sessions[0].auto_start_shell);
    assert!(pane.sessions[0].tx_command.is_none());
    assert!(
        pane.sessions[0]
            .parser
            .screen()
            .contents()
            .contains("process exited")
    );
}

#[test]
fn terminal_finished_event_survives_after_budgeted_output() {
    let size = test_terminal_size();
    let (mut pane, _) = pane_with_command_session(size);

    for _ in 0..TERMINAL_DRAIN_EVENT_BUDGET {
        pane.sessions[0]
            .tx_output
            .send(TerminalEvent::Output(b"x".to_vec()))
            .unwrap();
    }
    pane.sessions[0]
        .tx_output
        .send(TerminalEvent::Finished {
            message: None,
            process_exit_code: None,
            reason: TerminalFinishReason::ProcessExit,
        })
        .unwrap();

    pane.drain_output();
    assert!(pane.sessions[0].started);

    pane.drain_output();
    assert!(!pane.sessions[0].started);
    assert!(pane.sessions[0].tx_command.is_none());
}

#[test]
fn terminal_restart_drops_stale_finished_events_from_previous_output_channel() {
    let size = test_terminal_size();
    let mut pane = pane_with_session(session_without_command(1, size), size);
    let stale_tx = pane.sessions[0].tx_output.clone();
    pane.sessions[0].mark_stopped();

    stale_tx
        .send(TerminalEvent::Finished {
            message: Some("\r\nold process exited\r\n".to_owned()),
            process_exit_code: Some(17),
            reason: TerminalFinishReason::ProcessExit,
        })
        .unwrap();
    pane.sessions[0].replace_output_channel_for_launch_for_test();
    pane.sessions[0].started = true;

    assert!(
        stale_tx
            .send(TerminalEvent::Finished {
                message: Some("\r\nlate old process exited\r\n".to_owned()),
                process_exit_code: Some(19),
                reason: TerminalFinishReason::ProcessExit,
            })
            .is_err()
    );
    assert_eq!(pane.drain_output(), 0);
    assert!(pane.sessions[0].started);
    assert_eq!(pane.sessions[0].last_process_exit_code, None);
    assert!(
        !pane.sessions[0]
            .parser
            .screen()
            .contents()
            .contains("old process exited")
    );

    pane.sessions[0]
        .tx_output
        .send(TerminalEvent::Output(b"new process output".to_vec()))
        .unwrap();

    assert_eq!(pane.drain_output(), 1);
    assert!(pane.sessions[0].started);
    assert!(
        pane.sessions[0]
            .parser
            .screen()
            .contents()
            .contains("new process output")
    );
}

#[test]
fn terminal_finished_process_session_does_not_auto_start_shell() {
    let size = test_terminal_size();
    let mut session = session_without_command(1, size);
    session.process_label = Some("Cargo Test".to_owned());
    session.auto_start_shell = false;
    session.mark_stopped();
    let mut pane = pane_with_session(session, size);

    pane.set_visible(false);
    pane.set_visible(true);

    assert!(!pane.sessions[0].started);
    assert!(pane.sessions[0].tx_command.is_none());
    assert!(pane.visible);
}

#[test]
fn terminal_finished_event_can_stop_without_rendering_exit_message() {
    let size = test_terminal_size();
    let (mut pane, _) = pane_with_command_session(size);

    pane.sessions[0]
        .tx_output
        .send(TerminalEvent::Finished {
            message: None,
            process_exit_code: None,
            reason: TerminalFinishReason::ProcessExit,
        })
        .unwrap();
    pane.drain_output();

    assert!(!pane.sessions[0].started);
    assert!(pane.sessions[0].tx_command.is_none());
    assert!(
        !pane.sessions[0]
            .parser
            .screen()
            .contents()
            .contains("process exited")
    );
}

#[test]
fn terminal_finished_process_exit_status_is_reported_after_stop() {
    let size = test_terminal_size();
    let mut success = session_without_command(1, size);
    success.auto_start_shell = false;
    success.process_label = Some("Cargo Test".to_owned());
    let mut success_pane = pane_with_session(success, size);

    success_pane.sessions[0]
        .tx_output
        .send(TerminalEvent::Finished {
            message: None,
            process_exit_code: Some(0),
            reason: TerminalFinishReason::ProcessExit,
        })
        .unwrap();
    success_pane.drain_output();

    assert!(!success_pane.sessions[0].started);
    assert_eq!(
        success_pane.sessions[0].command_status(),
        TerminalCommandStatus::Succeeded
    );

    let mut failure = session_without_command(1, size);
    failure.auto_start_shell = false;
    failure.process_label = Some("Cargo Test".to_owned());
    let mut failure_pane = pane_with_session(failure, size);
    failure_pane.sessions[0]
        .tx_output
        .send(TerminalEvent::Finished {
            message: None,
            process_exit_code: Some(17),
            reason: TerminalFinishReason::ProcessExit,
        })
        .unwrap();
    failure_pane.drain_output();

    assert!(!failure_pane.sessions[0].started);
    assert_eq!(
        failure_pane.sessions[0].command_status(),
        TerminalCommandStatus::Failed(17)
    );
}

#[test]
fn terminal_finished_terminal_error_status_is_reported_after_stop() {
    let size = test_terminal_size();
    let mut session = session_without_command(1, size);
    session.auto_start_shell = false;
    session.process_label = Some("Cargo Test".to_owned());
    let mut pane = pane_with_session(session, size);

    pane.sessions[0]
        .tx_output
        .send(TerminalEvent::Finished {
            message: None,
            process_exit_code: None,
            reason: TerminalFinishReason::TerminalError,
        })
        .unwrap();
    pane.drain_output();

    assert!(!pane.sessions[0].started);
    assert_eq!(
        pane.sessions[0].command_status(),
        TerminalCommandStatus::TerminalError
    );
    assert_eq!(
        pane.process_session_state_by_id(1),
        Some(TerminalProcessSessionState::TerminalError)
    );
}

#[test]
fn terminal_process_exit_status_clears_on_start() {
    let size = test_terminal_size();
    let mut session = session_without_command(1, size);
    session.last_process_exit_code = Some(17);
    session.last_process_terminal_error = true;
    session.mark_stopped();

    session.start_process(
        &PathBuf::from("workspace"),
        size,
        "cmd".to_owned(),
        Vec::new(),
        Default::default(),
        false,
        "Task".to_owned(),
        None,
    );

    assert!(session.started);
    assert_eq!(session.last_process_exit_code, None);
    assert!(!session.last_process_terminal_error);
    assert_eq!(session.command_status(), TerminalCommandStatus::Unknown);
    session.close();
}

#[test]
fn terminal_drain_output_caps_total_events_across_sessions() {
    let size = test_terminal_size();
    let mut pane = pane_with_sessions(
        vec![
            session_without_command(1, size),
            session_without_command(2, size),
        ],
        size,
    );

    for index in 0..pane.sessions.len() {
        for _ in 0..(TERMINAL_DRAIN_EVENT_BUDGET + 1) {
            pane.sessions[index]
                .tx_output
                .send(TerminalEvent::Output(b"x".to_vec()))
                .unwrap();
        }
    }

    assert_eq!(pane.drain_output(), TERMINAL_DRAIN_EVENT_BUDGET);
    assert!(!pane.sessions[0].rx_output.is_empty());
    assert!(!pane.sessions[1].rx_output.is_empty());
}

#[test]
fn terminal_drain_output_caps_total_bytes_across_sessions() {
    let size = test_terminal_size();
    let mut pane = pane_with_sessions(
        vec![
            session_without_command(1, size),
            session_without_command(2, size),
        ],
        size,
    );
    let chunk = vec![b'x'; TERMINAL_DRAIN_BYTE_BUDGET / 2];

    for index in 0..pane.sessions.len() {
        for _ in 0..3 {
            pane.sessions[index]
                .tx_output
                .send(TerminalEvent::Output(chunk.clone()))
                .unwrap();
        }
    }

    assert_eq!(pane.drain_output(), 2);
    assert_eq!(pane.sessions[0].rx_output.len(), 2);
    assert_eq!(pane.sessions[1].rx_output.len(), 2);
    assert!(pane.has_pending_output());
}

#[test]
fn terminal_drain_output_reuses_unused_fair_budget_for_busy_sessions() {
    let size = test_terminal_size();
    let mut pane = pane_with_sessions(
        vec![
            session_without_command(1, size),
            session_without_command(2, size),
            session_without_command(3, size),
        ],
        size,
    );

    pane.sessions[1]
        .tx_output
        .send(TerminalEvent::Output(b"brief".to_vec()))
        .unwrap();
    for _ in 0..(TERMINAL_DRAIN_EVENT_BUDGET + 2) {
        pane.sessions[2]
            .tx_output
            .send(TerminalEvent::Output(b"busy".to_vec()))
            .unwrap();
    }

    assert_eq!(pane.drain_output(), TERMINAL_DRAIN_EVENT_BUDGET);
    assert!(pane.sessions[0].rx_output.is_empty());
    assert!(pane.sessions[1].rx_output.is_empty());
    assert_eq!(pane.sessions[2].rx_output.len(), 3);
    assert!(pane.has_pending_output());
}

#[test]
fn terminal_output_updates_vt_screen_instead_of_raw_log_text() {
    let size = test_terminal_size();
    let mut pane = pane_with_session(session_without_command(1, size), size);

    pane.sessions[0]
        .tx_output
        .send(TerminalEvent::Output(
            b"red \x1b[31mtext\x1b[0m\r\n\x1b[6n".to_vec(),
        ))
        .unwrap();
    pane.drain_output();

    let screen = pane.sessions[0].parser.screen();
    assert!(screen.contents().contains("red text"));
    assert_eq!(screen.cell(0, 4).unwrap().fgcolor(), vt100::Color::Idx(1));
    assert!(!screen.contents().contains("[6n"));
}

#[test]
fn terminal_replies_to_cursor_position_queries() {
    let size = test_terminal_size();
    let (mut pane, rx_command) = pane_with_command_session(size);

    pane.sessions[0]
        .tx_output
        .send(TerminalEvent::Output(b"\x1b[6n".to_vec()))
        .unwrap();
    pane.drain_output();

    match rx_command.try_recv().unwrap() {
        TerminalCommand::Input(input) => assert_eq!(input, "\x1b[1;1R"),
        TerminalCommand::Resize(_) => panic!("expected cursor position response"),
        TerminalCommand::Close => panic!("expected cursor position response"),
    }
}

#[test]
fn terminal_task_session_status_ignores_non_process_sessions() {
    let size = test_terminal_size();
    let shell = session_without_command(1, size);
    let mut task = session_without_command(2, size);
    task.process_label = Some("Build".to_owned());
    let pane = pane_with_sessions(vec![shell, task], size);

    assert_eq!(pane.process_session_state_by_id(1), None);
    assert_eq!(
        pane.process_session_state_by_id(2),
        Some(TerminalProcessSessionState::Running)
    );
}

#[test]
fn terminal_process_session_state_ignores_non_process_sessions() {
    let size = test_terminal_size();
    let mut shell = session_without_command(1, size);
    shell.started = false;
    shell.last_process_exit_code = Some(0);
    let mut task = session_without_command(2, size);
    task.started = false;
    task.process_label = Some("Build".to_owned());
    task.last_process_exit_code = Some(17);
    let pane = pane_with_sessions(vec![shell, task], size);

    assert_eq!(pane.process_session_state_by_id(1), None);
    assert_eq!(
        pane.process_session_state_by_id(2),
        Some(TerminalProcessSessionState::Exited(17))
    );
}
