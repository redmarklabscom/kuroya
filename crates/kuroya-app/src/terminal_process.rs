use crate::path_display::{display_error_label_cow, sanitized_display_label_cow};
#[cfg(test)]
use crossbeam_channel::TrySendError;
use crossbeam_channel::{Receiver, Sender};
use egui::Context;
use portable_pty::{ExitStatus, PtySize, native_pty_system};
use std::{
    borrow::Cow,
    collections::BTreeMap,
    fmt::Display,
    fmt::Write as _,
    io::{Read, Write},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

pub(crate) use shell::TerminalShellProfile;
pub(crate) use shell::default_shell_label;
pub(crate) use shell::detected_shell_profiles;
pub(crate) use shell::terminal_shell_label;
use shell::{configured_process, configured_shell};

mod shell;

const TERMINAL_EVENT_OUTPUT_CHUNK_BYTES: usize = 4096;
const TERMINAL_PROCESS_INPUT_MAX_BYTES: usize = 1024 * 1024;
const TERMINAL_FAILURE_LABEL_MAX_CHARS: usize = 48;

pub(crate) enum TerminalCommand {
    Input(String),
    Resize(PtySize),
    Close,
}

pub(crate) enum TerminalLaunch {
    Shell {
        shell_path: Option<String>,
        shell_args: Vec<String>,
    },
    Process {
        program: String,
        args: Vec<String>,
        env: BTreeMap<String, String>,
    },
}

pub(crate) enum TerminalEvent {
    Output(Vec<u8>),
    Finished {
        message: Option<String>,
        process_exit_code: Option<i32>,
        reason: TerminalFinishReason,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TerminalFinishReason {
    ProcessExit,
    TerminalError,
}

#[cfg(test)]
pub(crate) fn send_terminal_event(tx: &Sender<TerminalEvent>, event: TerminalEvent) -> bool {
    send_terminal_event_with_repaint(tx, event, None)
}

#[cfg(test)]
pub(crate) fn send_terminal_event_with_repaint(
    tx: &Sender<TerminalEvent>,
    event: TerminalEvent,
    repaint_context: Option<&Context>,
) -> bool {
    if let TerminalEvent::Output(output) = event {
        return send_terminal_output_chunks_with_repaint(tx, output, repaint_context);
    }
    send_terminal_event_single_with_repaint(tx, event, repaint_context)
}

#[cfg(test)]
fn send_terminal_event_single_with_repaint(
    tx: &Sender<TerminalEvent>,
    event: TerminalEvent,
    repaint_context: Option<&Context>,
) -> bool {
    match tx.try_send(event) {
        Ok(()) => {
            request_terminal_repaint(repaint_context);
            true
        }
        Err(TrySendError::Full(_)) => {
            request_terminal_repaint(repaint_context);
            false
        }
        Err(TrySendError::Disconnected(_)) => false,
    }
}

#[cfg(test)]
fn send_terminal_output_chunks_with_repaint(
    tx: &Sender<TerminalEvent>,
    output: Vec<u8>,
    repaint_context: Option<&Context>,
) -> bool {
    if output.len() <= TERMINAL_EVENT_OUTPUT_CHUNK_BYTES {
        return send_terminal_event_single_with_repaint(
            tx,
            TerminalEvent::Output(output),
            repaint_context,
        );
    }

    request_terminal_repaint(repaint_context);
    for chunk in output.chunks(TERMINAL_EVENT_OUTPUT_CHUNK_BYTES) {
        if !send_terminal_event_single(tx, TerminalEvent::Output(chunk.to_vec())) {
            return false;
        }
    }
    request_terminal_repaint(repaint_context);
    true
}

#[cfg(test)]
fn send_terminal_event_single(tx: &Sender<TerminalEvent>, event: TerminalEvent) -> bool {
    match tx.try_send(event) {
        Ok(()) => true,
        Err(TrySendError::Full(_)) => false,
        Err(TrySendError::Disconnected(_)) => false,
    }
}

pub(crate) fn send_terminal_event_blocking_with_repaint(
    tx: &Sender<TerminalEvent>,
    event: TerminalEvent,
    repaint_context: Option<&Context>,
) -> bool {
    if let TerminalEvent::Output(output) = event {
        return send_terminal_output_chunks_blocking_with_repaint(tx, output, repaint_context);
    }
    send_terminal_event_single_blocking_with_repaint(tx, event, repaint_context)
}

fn send_terminal_event_single_blocking_with_repaint(
    tx: &Sender<TerminalEvent>,
    event: TerminalEvent,
    repaint_context: Option<&Context>,
) -> bool {
    request_terminal_repaint(repaint_context);
    match tx.send(event) {
        Ok(()) => {
            request_terminal_repaint(repaint_context);
            true
        }
        Err(_) => false,
    }
}

fn send_terminal_output_chunks_blocking_with_repaint(
    tx: &Sender<TerminalEvent>,
    output: Vec<u8>,
    repaint_context: Option<&Context>,
) -> bool {
    if output.len() <= TERMINAL_EVENT_OUTPUT_CHUNK_BYTES {
        return send_terminal_event_single_blocking_with_repaint(
            tx,
            TerminalEvent::Output(output),
            repaint_context,
        );
    }

    request_terminal_repaint(repaint_context);
    for chunk in output.chunks(TERMINAL_EVENT_OUTPUT_CHUNK_BYTES) {
        if !send_terminal_event_single_blocking(tx, TerminalEvent::Output(chunk.to_vec())) {
            return false;
        }
    }
    request_terminal_repaint(repaint_context);
    true
}

fn send_terminal_event_single_blocking(tx: &Sender<TerminalEvent>, event: TerminalEvent) -> bool {
    tx.send(event).is_ok()
}

fn request_terminal_repaint(repaint_context: Option<&Context>) {
    if let Some(ctx) = repaint_context {
        ctx.request_repaint();
    }
}

pub(crate) fn run_pty(
    cwd: PathBuf,
    initial_size: PtySize,
    launch: TerminalLaunch,
    show_exit_alert: bool,
    rx_command: Receiver<TerminalCommand>,
    rx_close: Receiver<()>,
    close_requested: Arc<AtomicBool>,
    tx_output: Sender<TerminalEvent>,
    repaint_context: Option<Context>,
) -> anyhow::Result<()> {
    let pty_system = native_pty_system();
    let pair = pty_system.openpty(initial_size)?;

    let is_process_launch = matches!(launch, TerminalLaunch::Process { .. });
    let mut cmd = match launch {
        TerminalLaunch::Shell {
            shell_path,
            shell_args,
        } => configured_shell(shell_path.as_deref(), &shell_args),
        TerminalLaunch::Process { program, args, env } => configured_process(&program, &args, &env),
    };
    cmd.cwd(cwd);
    let mut child = pair.slave.spawn_command(cmd)?;
    let mut child_killer = child.clone_killer();
    let mut close_killer = child.clone_killer();
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader()?;
    let mut writer = pair.master.take_writer()?;
    let master = pair.master;

    let terminal_finished = Arc::new(AtomicBool::new(false));
    let reader_tx = tx_output.clone();
    let reader_close_requested = Arc::clone(&close_requested);
    let reader_terminal_finished = Arc::clone(&terminal_finished);
    let reader_repaint_context = repaint_context.clone();
    let reader_handle = thread::spawn(move || {
        let mut buf = [0_u8; TERMINAL_EVENT_OUTPUT_CHUNK_BYTES];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if !send_terminal_output_if_unfinished(
                        &reader_tx,
                        buf[..n].to_vec(),
                        &reader_terminal_finished,
                        reader_repaint_context.as_ref(),
                    ) {
                        break;
                    }
                }
                Err(error) => {
                    if !reader_close_requested.load(Ordering::SeqCst) {
                        let _ = send_terminal_finished_once(
                            &reader_tx,
                            TerminalEvent::Finished {
                                message: Some(terminal_failure_message(
                                    "terminal read error",
                                    error,
                                )),
                                process_exit_code: None,
                                reason: TerminalFinishReason::TerminalError,
                            },
                            &reader_terminal_finished,
                            reader_repaint_context.as_ref(),
                        );
                    }
                    break;
                }
            }
        }
    });

    let close_signal_requested = Arc::clone(&close_requested);
    thread::spawn(move || {
        if rx_close.recv().is_ok() {
            close_signal_requested.store(true, Ordering::SeqCst);
            let _ = close_killer.kill();
        }
    });

    let writer_close_requested = Arc::clone(&close_requested);
    let writer_terminal_finished = Arc::clone(&terminal_finished);
    thread::spawn(move || {
        while let Ok(command) = rx_command.recv() {
            if terminal_command_writer_should_stop(
                &writer_close_requested,
                &writer_terminal_finished,
            ) {
                break;
            }
            match command {
                TerminalCommand::Input(input) => {
                    let input_result = write_terminal_input_if_running(
                        &mut writer,
                        &input,
                        &writer_close_requested,
                        &writer_terminal_finished,
                    );
                    match input_result {
                        Ok(true) => {}
                        Ok(false) | Err(_) => break,
                    }
                }
                TerminalCommand::Resize(size) => {
                    if terminal_command_writer_should_stop(
                        &writer_close_requested,
                        &writer_terminal_finished,
                    ) {
                        break;
                    }
                    let _ = master.resize(size);
                }
                TerminalCommand::Close => {
                    writer_close_requested.store(true, Ordering::SeqCst);
                    let _ = child_killer.kill();
                    break;
                }
            }
        }
        if !writer_close_requested.load(Ordering::SeqCst) {
            writer_close_requested.store(true, Ordering::SeqCst);
            let _ = child_killer.kill();
        }
    });

    let status = child.wait();
    let _ = reader_handle.join();
    let status = status?;
    let close_requested = close_requested.load(Ordering::SeqCst);
    let _ = send_terminal_finished_once(
        &tx_output,
        TerminalEvent::Finished {
            message: terminal_exit_alert_message(&status, show_exit_alert, close_requested),
            process_exit_code: terminal_process_exit_code(
                &status,
                is_process_launch,
                close_requested,
            ),
            reason: TerminalFinishReason::ProcessExit,
        },
        &terminal_finished,
        repaint_context.as_ref(),
    );
    Ok(())
}

fn send_terminal_output_if_unfinished(
    tx: &Sender<TerminalEvent>,
    output: Vec<u8>,
    terminal_finished: &AtomicBool,
    repaint_context: Option<&Context>,
) -> bool {
    if terminal_finished.load(Ordering::SeqCst) {
        return false;
    }
    send_terminal_event_blocking_with_repaint(tx, TerminalEvent::Output(output), repaint_context)
}

fn send_terminal_finished_once(
    tx: &Sender<TerminalEvent>,
    event: TerminalEvent,
    terminal_finished: &AtomicBool,
    repaint_context: Option<&Context>,
) -> bool {
    if terminal_finished.swap(true, Ordering::SeqCst) {
        return false;
    }
    send_terminal_event_blocking_with_repaint(tx, event, repaint_context)
}

fn terminal_command_writer_should_stop(
    close_requested: &AtomicBool,
    terminal_finished: &AtomicBool,
) -> bool {
    close_requested.load(Ordering::SeqCst) || terminal_finished.load(Ordering::SeqCst)
}

fn write_terminal_input_if_running(
    writer: &mut impl Write,
    input: &str,
    close_requested: &AtomicBool,
    terminal_finished: &AtomicBool,
) -> std::io::Result<bool> {
    if terminal_command_writer_should_stop(close_requested, terminal_finished) {
        return Ok(false);
    }
    write_terminal_input(writer, input)?;
    Ok(true)
}

fn write_terminal_input(writer: &mut impl Write, input: &str) -> std::io::Result<()> {
    let input = bounded_terminal_process_input(input, TERMINAL_PROCESS_INPUT_MAX_BYTES);
    if input.is_empty() {
        return Ok(());
    }
    writer.write_all(input.as_bytes())?;
    writer.flush()
}

fn bounded_terminal_process_input(input: &str, max_bytes: usize) -> &str {
    if input.len() <= max_bytes {
        return input;
    }

    let mut truncate_at = max_bytes;
    while truncate_at > 0 && !input.is_char_boundary(truncate_at) {
        truncate_at -= 1;
    }
    &input[..truncate_at]
}

fn terminal_process_exit_code(
    status: &ExitStatus,
    is_process_launch: bool,
    close_requested: bool,
) -> Option<i32> {
    if !is_process_launch || close_requested {
        return None;
    }
    Some(i32::try_from(status.exit_code()).unwrap_or(i32::MAX))
}

pub(crate) fn terminal_exit_alert_message(
    status: &ExitStatus,
    show_exit_alert: bool,
    close_requested: bool,
) -> Option<String> {
    (show_exit_alert && !close_requested && !status.success())
        .then(|| terminal_failure_message("process exited", status))
}

fn terminal_failure_label_cow(label: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(label, TERMINAL_FAILURE_LABEL_MAX_CHARS, "terminal error")
}

pub(crate) fn terminal_failure_message(label: &str, detail: impl Display) -> String {
    let label = terminal_failure_label_cow(label);
    let detail = detail.to_string();
    let detail = display_error_label_cow(&detail);
    let mut message = String::with_capacity(label.len() + detail.len() + 6);
    let _ = write!(&mut message, "\r\n{label}: {detail}\r\n");
    message
}

#[cfg(test)]
fn terminal_event_output_chunk_bytes_for_test() -> usize {
    TERMINAL_EVENT_OUTPUT_CHUNK_BYTES
}

#[cfg(test)]
mod tests {
    use super::{
        TerminalEvent, TerminalFinishReason, send_terminal_event,
        send_terminal_event_blocking_with_repaint, send_terminal_event_with_repaint,
        terminal_event_output_chunk_bytes_for_test, terminal_exit_alert_message,
        terminal_failure_label_cow, terminal_failure_message, terminal_process_exit_code,
        write_terminal_input, write_terminal_input_if_running,
    };
    use crate::path_display::DISPLAY_ERROR_LABEL_MAX_CHARS;
    use crossbeam_channel::{TryRecvError, bounded};
    use egui::Context;
    use portable_pty::ExitStatus;
    use std::{
        borrow::Cow,
        io::{self, Write},
        sync::atomic::AtomicBool,
        thread,
    };

    #[test]
    fn terminal_exit_alert_message_only_shows_non_zero_when_enabled() {
        assert_eq!(
            terminal_exit_alert_message(&ExitStatus::with_exit_code(0), true, false),
            None
        );
        assert_eq!(
            terminal_exit_alert_message(&ExitStatus::with_exit_code(1), false, false),
            None
        );
        assert_eq!(
            terminal_exit_alert_message(&ExitStatus::with_exit_code(1), true, false),
            Some("\r\nprocess exited: Exited with code 1\r\n".to_owned())
        );
    }

    #[test]
    fn terminal_exit_alert_message_suppresses_intentional_close() {
        assert_eq!(
            terminal_exit_alert_message(&ExitStatus::with_exit_code(1), true, true),
            None
        );
    }

    #[test]
    fn terminal_input_write_reports_flush_failures() {
        let mut writer = FlushFailWriter::default();

        let error =
            write_terminal_input(&mut writer, "queued input").expect_err("flush should fail");

        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
        assert_eq!(writer.bytes, b"queued input");
        assert_eq!(writer.flushes, 1);
    }

    #[test]
    fn terminal_input_write_preserves_raw_bytes() {
        let mut writer = FlushFailWriter::default();
        let input = "\0\x1b[31mraw\r\n\u{7}";

        let error = write_terminal_input(&mut writer, input).expect_err("flush should fail");

        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
        assert_eq!(writer.bytes, input.as_bytes());
        assert_eq!(writer.flushes, 1);
    }

    #[test]
    fn terminal_input_write_caps_oversized_input_at_utf8_boundary() {
        let mut writer = FlushFailWriter::default();
        let input = format!(
            "{}\u{e9}",
            "a".repeat(super::TERMINAL_PROCESS_INPUT_MAX_BYTES)
        );

        let error = write_terminal_input(&mut writer, &input).expect_err("flush should fail");

        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
        assert_eq!(writer.bytes.len(), super::TERMINAL_PROCESS_INPUT_MAX_BYTES);
        assert!(std::str::from_utf8(&writer.bytes).is_ok());
        assert!(writer.bytes.ends_with(b"a"));
        assert_eq!(writer.flushes, 1);
    }

    #[test]
    fn terminal_input_write_skips_finished_processes() {
        let mut writer = FlushFailWriter::default();
        let close_requested = AtomicBool::new(false);
        let terminal_finished = AtomicBool::new(true);

        let wrote = write_terminal_input_if_running(
            &mut writer,
            "late input",
            &close_requested,
            &terminal_finished,
        )
        .expect("finished process should skip input without writer errors");

        assert!(!wrote);
        assert!(writer.bytes.is_empty());
        assert_eq!(writer.flushes, 0);
    }

    #[test]
    fn terminal_process_exit_code_tracks_process_launches_only() {
        assert_eq!(
            terminal_process_exit_code(&ExitStatus::with_exit_code(0), true, false),
            Some(0)
        );
        assert_eq!(
            terminal_process_exit_code(&ExitStatus::with_exit_code(17), true, false),
            Some(17)
        );
        assert_eq!(
            terminal_process_exit_code(&ExitStatus::with_exit_code(17), true, true),
            None
        );
        assert_eq!(
            terminal_process_exit_code(&ExitStatus::with_exit_code(17), false, false),
            None
        );
    }

    #[test]
    fn terminal_failure_label_cow_borrows_clean_ascii_and_unicode() {
        assert!(matches!(
            terminal_failure_label_cow("terminal read error"),
            Cow::Borrowed("terminal read error")
        ));

        let unicode = "terminal \u{03bb} error";
        match terminal_failure_label_cow(unicode) {
            Cow::Borrowed(label) => assert_eq!(label, unicode),
            Cow::Owned(label) => panic!("expected borrowed label, got {label:?}"),
        }
    }

    #[test]
    fn terminal_failure_label_cow_owns_dirty_truncated_and_fallback_labels() {
        let dirty = terminal_failure_label_cow("terminal\nread");
        assert_eq!(dirty.as_ref(), "terminal read");
        assert!(matches!(dirty, Cow::Owned(_)));

        let long = format!("{}tail", "a".repeat(80));
        let truncated = terminal_failure_label_cow(&long);
        assert!(truncated.contains("..."));
        assert!(truncated.chars().count() <= super::TERMINAL_FAILURE_LABEL_MAX_CHARS);
        assert!(matches!(truncated, Cow::Owned(_)));

        let fallback = terminal_failure_label_cow("   ");
        assert_eq!(fallback.as_ref(), "terminal error");
        assert!(matches!(fallback, Cow::Owned(_)));
    }

    #[test]
    fn terminal_failure_message_preserves_formatting_bounds_and_detail_sanitization() {
        assert_eq!(
            terminal_failure_message("process exited", ExitStatus::with_exit_code(17)),
            "\r\nprocess exited: Exited with code 17\r\n"
        );

        let message = terminal_failure_message(
            &format!("read error {}", "x".repeat(96)),
            "first line\r\nsecond line \u{202e}detail",
        );
        let body = message
            .strip_prefix("\r\n")
            .expect("terminal message should start with CRLF")
            .strip_suffix("\r\n")
            .expect("terminal message should end with CRLF");
        let (label, detail) = body
            .split_once(": ")
            .expect("terminal message should separate label and detail");

        assert_eq!(message.matches("\r\n").count(), 2);
        assert!(label.contains("..."));
        assert!(label.chars().count() <= super::TERMINAL_FAILURE_LABEL_MAX_CHARS);
        assert!(!detail.contains('\n'));
        assert!(!detail.contains('\r'));
        assert!(!detail.contains('\u{202e}'));
        assert!(detail.contains("first line second line detail"));
        assert!(detail.chars().count() <= DISPLAY_ERROR_LABEL_MAX_CHARS);
    }

    #[test]
    fn terminal_failure_message_sanitizes_and_bounds_error_details() {
        let message = terminal_failure_message(
            "terminal error",
            format!("first line\nsecond line \u{202e}{}", "x".repeat(512)),
        );
        let detail = terminal_message_detail(&message, "terminal error");

        assert_eq!(message.matches("\r\n").count(), 2);
        assert!(!detail.contains('\n'));
        assert!(!detail.contains('\r'));
        assert!(!detail.contains('\u{202e}'));
        assert!(detail.contains("first line second line"));
        assert!(detail.contains("..."));
        assert!(detail.chars().count() <= DISPLAY_ERROR_LABEL_MAX_CHARS);
    }

    #[test]
    fn terminal_exit_alert_message_sanitizes_and_bounds_signal_details() {
        let status = ExitStatus::with_signal(&format!(
            "signal line\nnext line \u{202e}{}",
            "x".repeat(512)
        ));
        let message = terminal_exit_alert_message(&status, true, false)
            .expect("nonzero signal exit should show alert");
        let detail = terminal_message_detail(&message, "process exited");

        assert!(!detail.contains('\n'));
        assert!(!detail.contains('\r'));
        assert!(!detail.contains('\u{202e}'));
        assert!(detail.contains("Terminated by signal line next line"));
        assert!(detail.contains("..."));
        assert!(detail.chars().count() <= DISPLAY_ERROR_LABEL_MAX_CHARS);
    }

    fn terminal_message_detail<'a>(message: &'a str, label: &str) -> &'a str {
        let prefix = format!("\r\n{label}: ");
        message
            .strip_prefix(&prefix)
            .expect("terminal message should start with label")
            .strip_suffix("\r\n")
            .expect("terminal message should end with CRLF")
    }

    #[derive(Default)]
    struct FlushFailWriter {
        bytes: Vec<u8>,
        flushes: usize,
    }

    impl Write for FlushFailWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.bytes.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            self.flushes = self.flushes.saturating_add(1);
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "flush failed"))
        }
    }

    #[test]
    fn send_terminal_event_returns_false_when_output_queue_is_full() {
        let (tx, _rx) = bounded(1);

        assert!(send_terminal_event(
            &tx,
            TerminalEvent::Output(b"first".to_vec())
        ));
        assert!(!send_terminal_event(
            &tx,
            TerminalEvent::Output(b"overflow".to_vec())
        ));
    }

    #[test]
    fn send_terminal_event_enqueues_when_capacity_is_available() {
        let (tx, rx) = bounded(1);

        assert!(send_terminal_event(
            &tx,
            TerminalEvent::Finished {
                message: Some("done".to_owned()),
                process_exit_code: Some(0),
                reason: TerminalFinishReason::ProcessExit,
            }
        ));

        match rx.try_recv() {
            Ok(TerminalEvent::Finished {
                message: Some(message),
                process_exit_code,
                reason,
            }) => {
                assert_eq!(message, "done");
                assert_eq!(process_exit_code, Some(0));
                assert_eq!(reason, TerminalFinishReason::ProcessExit);
            }
            Ok(TerminalEvent::Finished { message: None, .. }) => {
                panic!("expected finished message")
            }
            Ok(TerminalEvent::Output(_)) => panic!("expected finished event"),
            Err(TryRecvError::Empty) => panic!("expected queued terminal event"),
            Err(TryRecvError::Disconnected) => panic!("terminal event queue disconnected"),
        }
    }

    #[test]
    fn send_terminal_event_splits_oversized_output_chunks() {
        let chunk_size = terminal_event_output_chunk_bytes_for_test();
        let (tx, rx) = bounded(2);

        assert!(send_terminal_event(
            &tx,
            TerminalEvent::Output(vec![b'x'; chunk_size + 3])
        ));

        match rx.try_recv() {
            Ok(TerminalEvent::Output(output)) => assert_eq!(output.len(), chunk_size),
            Ok(TerminalEvent::Finished { .. }) => panic!("expected output event"),
            Err(TryRecvError::Empty) => panic!("expected queued terminal output"),
            Err(TryRecvError::Disconnected) => panic!("terminal event queue disconnected"),
        }
        match rx.try_recv() {
            Ok(TerminalEvent::Output(output)) => assert_eq!(output.len(), 3),
            Ok(TerminalEvent::Finished { .. }) => panic!("expected output event"),
            Err(TryRecvError::Empty) => panic!("expected queued terminal output"),
            Err(TryRecvError::Disconnected) => panic!("terminal event queue disconnected"),
        }
    }

    #[test]
    fn send_terminal_event_blocking_waits_for_capacity_and_enqueues() {
        let (tx, rx) = bounded(1);
        assert!(send_terminal_event(
            &tx,
            TerminalEvent::Output(b"first".to_vec())
        ));

        let blocking_tx = tx.clone();
        let handle = thread::spawn(move || {
            send_terminal_event_blocking_with_repaint(
                &blocking_tx,
                TerminalEvent::Finished {
                    message: Some("done".to_owned()),
                    process_exit_code: Some(17),
                    reason: TerminalFinishReason::ProcessExit,
                },
                None,
            )
        });

        match rx.recv() {
            Ok(TerminalEvent::Output(output)) => assert_eq!(output, b"first"),
            Ok(TerminalEvent::Finished { .. }) => panic!("expected first output event"),
            Err(_) => panic!("terminal event queue disconnected"),
        }
        assert!(
            handle
                .join()
                .expect("blocking enqueue thread should finish")
        );
        match rx.try_recv() {
            Ok(TerminalEvent::Finished {
                message: Some(message),
                process_exit_code,
                reason,
            }) => {
                assert_eq!(message, "done");
                assert_eq!(process_exit_code, Some(17));
                assert_eq!(reason, TerminalFinishReason::ProcessExit);
            }
            Ok(TerminalEvent::Finished { message: None, .. }) => {
                panic!("expected finished message")
            }
            Ok(TerminalEvent::Output(_)) => panic!("expected finished event"),
            Err(TryRecvError::Empty) => panic!("expected queued terminal event"),
            Err(TryRecvError::Disconnected) => panic!("terminal event queue disconnected"),
        }
    }

    #[test]
    fn send_terminal_event_with_repaint_requests_repaint_when_enqueued() {
        let (tx, _rx) = bounded(1);
        let ctx = Context::default();

        assert!(send_terminal_event_with_repaint(
            &tx,
            TerminalEvent::Output(b"output".to_vec()),
            Some(&ctx)
        ));

        assert!(ctx.has_requested_repaint());
    }

    #[test]
    fn send_terminal_event_with_repaint_requests_repaint_when_queue_is_full() {
        let (tx, _rx) = bounded(1);
        assert!(send_terminal_event(
            &tx,
            TerminalEvent::Output(b"first".to_vec())
        ));
        let ctx = Context::default();

        assert!(!send_terminal_event_with_repaint(
            &tx,
            TerminalEvent::Output(b"overflow".to_vec()),
            Some(&ctx)
        ));

        assert!(ctx.has_requested_repaint());
    }

    #[test]
    fn send_terminal_event_returns_false_when_output_queue_is_disconnected() {
        let (tx, rx) = bounded(1);
        drop(rx);

        assert!(!send_terminal_event(
            &tx,
            TerminalEvent::Output(b"output".to_vec())
        ));
        assert!(!send_terminal_event_blocking_with_repaint(
            &tx,
            TerminalEvent::Finished {
                message: None,
                process_exit_code: None,
                reason: TerminalFinishReason::ProcessExit,
            },
            None,
        ));
    }

    #[test]
    fn terminal_finished_guard_suppresses_duplicate_finished_events() {
        let (tx, rx) = bounded(2);
        let terminal_finished = AtomicBool::new(false);

        assert!(super::send_terminal_finished_once(
            &tx,
            TerminalEvent::Finished {
                message: Some("first".to_owned()),
                process_exit_code: Some(0),
                reason: TerminalFinishReason::ProcessExit,
            },
            &terminal_finished,
            None,
        ));
        assert!(!super::send_terminal_finished_once(
            &tx,
            TerminalEvent::Finished {
                message: Some("second".to_owned()),
                process_exit_code: Some(1),
                reason: TerminalFinishReason::TerminalError,
            },
            &terminal_finished,
            None,
        ));

        match rx.try_recv() {
            Ok(TerminalEvent::Finished {
                message,
                process_exit_code,
                reason,
            }) => {
                assert_eq!(message.as_deref(), Some("first"));
                assert_eq!(process_exit_code, Some(0));
                assert_eq!(reason, TerminalFinishReason::ProcessExit);
            }
            Ok(TerminalEvent::Output(_)) => panic!("expected finished event"),
            Err(TryRecvError::Empty) => panic!("expected queued terminal event"),
            Err(TryRecvError::Disconnected) => panic!("terminal event queue disconnected"),
        }
        assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
    }

    #[test]
    fn terminal_finished_guard_suppresses_output_after_finished() {
        let (tx, rx) = bounded(3);
        let terminal_finished = AtomicBool::new(false);

        assert!(super::send_terminal_output_if_unfinished(
            &tx,
            b"before".to_vec(),
            &terminal_finished,
            None,
        ));
        assert!(super::send_terminal_finished_once(
            &tx,
            TerminalEvent::Finished {
                message: None,
                process_exit_code: Some(0),
                reason: TerminalFinishReason::ProcessExit,
            },
            &terminal_finished,
            None,
        ));
        assert!(!super::send_terminal_output_if_unfinished(
            &tx,
            b"after".to_vec(),
            &terminal_finished,
            None,
        ));

        match rx.try_recv() {
            Ok(TerminalEvent::Output(output)) => assert_eq!(output, b"before"),
            Ok(TerminalEvent::Finished { .. }) => panic!("expected output event"),
            Err(TryRecvError::Empty) => panic!("expected queued terminal output"),
            Err(TryRecvError::Disconnected) => panic!("terminal event queue disconnected"),
        }
        match rx.try_recv() {
            Ok(TerminalEvent::Finished {
                process_exit_code, ..
            }) => {
                assert_eq!(process_exit_code, Some(0));
            }
            Ok(TerminalEvent::Output(_)) => panic!("expected finished event"),
            Err(TryRecvError::Empty) => panic!("expected queued terminal event"),
            Err(TryRecvError::Disconnected) => panic!("terminal event queue disconnected"),
        }
        assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
    }
}
