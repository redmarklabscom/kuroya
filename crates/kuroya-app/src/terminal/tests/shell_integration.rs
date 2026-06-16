use super::*;

#[test]
fn terminal_shell_integration_markers_track_prompt_and_command_lifecycle() {
    let size = test_terminal_size();
    let mut session = session_without_command(1, size);

    session.parser.process(b"\x1b]133;A\x07");

    let shell = &session.parser.callbacks().shell_integration;
    assert_eq!(
        shell.last_marker,
        Some(TerminalShellIntegrationMarker::PromptStart)
    );
    assert!(shell.prompt_active);
    assert!(!shell.command_running);

    session.parser.process(b"\x1b]133;B\x07\x1b]133;C\x07");

    let shell = &session.parser.callbacks().shell_integration;
    assert_eq!(
        shell.last_marker,
        Some(TerminalShellIntegrationMarker::CommandStart)
    );
    assert!(!shell.prompt_active);
    assert!(shell.command_running);
    assert_eq!(shell.last_command_exit_code, None);

    session.parser.process(b"\x1b]133;D;17\x07");

    let shell = &session.parser.callbacks().shell_integration;
    assert_eq!(
        shell.last_marker,
        Some(TerminalShellIntegrationMarker::CommandFinish)
    );
    assert!(!shell.command_running);
    assert_eq!(shell.last_command_exit_code, Some(17));
}

#[test]
fn terminal_shell_integration_accepts_vscode_marker_namespace() {
    let size = test_terminal_size();
    let mut session = session_without_command(1, size);

    session.parser.process(b"\x1b]633;A\x1b\\");
    session.parser.process(b"\x1b]633;B\x1b\\");
    session.parser.process(b"\x1b]633;C\x1b\\");
    session.parser.process(b"\x1b]633;D;0\x1b\\");

    let shell = &session.parser.callbacks().shell_integration;
    assert_eq!(
        shell.last_marker,
        Some(TerminalShellIntegrationMarker::CommandFinish)
    );
    assert!(!shell.prompt_active);
    assert!(!shell.command_running);
    assert_eq!(shell.last_command_exit_code, Some(0));
}

#[test]
fn terminal_shell_integration_ignores_unknown_markers() {
    let size = test_terminal_size();
    let mut session = session_without_command(1, size);

    session.parser.process(b"\x1b]777;C\x07\x1b]133;Z\x07");

    let shell = &session.parser.callbacks().shell_integration;
    assert_eq!(shell.last_marker, None);
    assert!(!shell.prompt_active);
    assert!(!shell.command_running);
    assert_eq!(shell.last_command_exit_code, None);
    assert_eq!(session.command_status(), TerminalCommandStatus::Unknown);
}

#[test]
fn terminal_command_status_is_derived_from_shell_integration_markers() {
    let size = test_terminal_size();
    let mut session = session_without_command(1, size);

    assert_eq!(session.command_status(), TerminalCommandStatus::Unknown);

    session.parser.process(b"\x1b]133;A\x07");
    assert_eq!(session.command_status(), TerminalCommandStatus::Prompt);

    session.parser.process(b"\x1b]133;B\x07\x1b]133;C\x07");
    assert_eq!(session.command_status(), TerminalCommandStatus::Running);

    session.parser.process(b"\x1b]133;D\x07");
    assert_eq!(session.command_status(), TerminalCommandStatus::Finished);

    session.parser.process(b"\x1b]133;D;0\x07");
    assert_eq!(session.command_status(), TerminalCommandStatus::Succeeded);

    session.parser.process(b"\x1b]133;D;17\x07");
    assert_eq!(session.command_status(), TerminalCommandStatus::Failed(17));

    session.parser.process(b"\x1b]133;C\x07");
    assert_eq!(session.command_status(), TerminalCommandStatus::Running);
    assert_eq!(
        session
            .parser
            .callbacks()
            .shell_integration
            .last_command_exit_code,
        None
    );
}

#[test]
fn terminal_shell_integration_markers_update_through_output_drain() {
    let size = test_terminal_size();
    let mut pane = pane_with_session(session_without_command(1, size), size);

    pane.sessions[0]
        .tx_output
        .send(TerminalEvent::Output(b"\x1b]133;C\x07".to_vec()))
        .unwrap();

    assert_eq!(pane.drain_output(), 1);
    assert_eq!(
        pane.sessions[0].command_status(),
        TerminalCommandStatus::Running
    );
}

#[test]
fn terminal_command_status_stopped_overrides_stale_shell_marker_state() {
    let size = test_terminal_size();
    let mut pane = pane_with_session(session_without_command(1, size), size);
    pane.sessions[0].parser.process(b"\x1b]133;C\x07");

    assert_eq!(
        pane.sessions[0].command_status(),
        TerminalCommandStatus::Running
    );

    pane.sessions[0]
        .tx_output
        .send(TerminalEvent::Finished {
            message: None,
            process_exit_code: None,
            reason: TerminalFinishReason::ProcessExit,
        })
        .unwrap();

    assert_eq!(pane.drain_output(), 1);
    assert!(!pane.sessions[0].started);
    assert!(
        pane.sessions[0]
            .parser
            .callbacks()
            .shell_integration
            .command_running
    );
    assert_eq!(
        pane.sessions[0].command_status(),
        TerminalCommandStatus::Stopped
    );
}

#[test]
fn terminal_shell_integration_state_resets_for_session_restart() {
    let size = test_terminal_size();
    let mut session = session_without_command(1, size);
    session.parser.process(b"\x1b]133;D;17\x07");

    assert_eq!(session.command_status(), TerminalCommandStatus::Failed(17));

    session.mark_stopped();
    session.started = true;
    session.reset_shell_integration_state();

    let shell = &session.parser.callbacks().shell_integration;
    assert_eq!(shell.last_marker, None);
    assert!(!shell.prompt_active);
    assert!(!shell.command_running);
    assert_eq!(shell.last_command_exit_code, None);
    assert_eq!(session.command_status(), TerminalCommandStatus::Unknown);
}

#[test]
fn terminal_session_label_ignores_shell_command_status() {
    let size = test_terminal_size();
    let mut pane = pane_with_session(session_without_command(1, size), size);
    let label = pane.terminal_session_label(&pane.sessions[0]);

    pane.sessions[0].parser.process(b"\x1b]133;C\x07");

    assert_eq!(pane.terminal_session_label(&pane.sessions[0]), label);
}
