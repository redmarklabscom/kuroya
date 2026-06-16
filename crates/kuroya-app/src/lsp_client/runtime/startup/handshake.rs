use crate::ui_event_channel::Sender;
use crate::{
    lsp_client::{
        pending::PendingLspRequest,
        runtime::messages::handle_lsp_server_message,
        wire::{LspMessageReadBuffer, read_message, write_message},
    },
    lsp_runtime::{
        lsp_language_display_label, lsp_server_ready_status, lsp_status_display_message,
    },
    lsp_ui_events::LspUiEvent,
    path_display::display_error_label_cow,
    ui_events::UiEvent,
};
use kuroya_core::{LspServerConfig, LspWireMessage};
use serde_json::Value;
use std::{collections::HashMap, path::Path, time::Duration};
use tokio::{
    io::BufReader,
    process::{ChildStdin, ChildStdout},
    sync::watch,
    time::{Instant, timeout_at},
};

use super::super::shutdown_signal_requested;

const LSP_INITIALIZE_REQUEST_ID: u64 = 1;
pub(super) const LSP_INITIALIZE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq)]
enum InitializeResponseState {
    Waiting,
    Ready,
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LspStartupHandshakeResult {
    Ready,
    Failed,
    ShutdownRequested,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum StartupHandshakeError {
    Failed(String),
    ShutdownRequested,
}

pub(super) async fn complete_lsp_startup_handshake(
    writer: &mut ChildStdin,
    reader: &mut BufReader<ChildStdout>,
    root: &Path,
    config: &LspServerConfig,
    generation: u64,
    shutdown_rx: &mut watch::Receiver<bool>,
    ui_tx: &Sender<UiEvent>,
) -> LspStartupHandshakeResult {
    if shutdown_signal_requested(shutdown_rx) {
        return LspStartupHandshakeResult::ShutdownRequested;
    }

    if let Err(error) = write_message(
        writer,
        &LspWireMessage::initialize(LSP_INITIALIZE_REQUEST_ID, root).to_json(),
    )
    .await
    {
        send_lsp_startup_status(
            &config.language,
            root,
            generation,
            lsp_initialize_failed_status_message(&config.language, &error.to_string()),
            ui_tx,
        );
        return LspStartupHandshakeResult::Failed;
    }

    match wait_for_initialize_response(
        writer,
        reader,
        &config.language,
        root,
        generation,
        shutdown_rx,
        ui_tx,
    )
    .await
    {
        Ok(()) => {}
        Err(StartupHandshakeError::ShutdownRequested) => {
            return LspStartupHandshakeResult::ShutdownRequested;
        }
        Err(StartupHandshakeError::Failed(error)) => {
            send_lsp_startup_status(
                &config.language,
                root,
                generation,
                lsp_initialize_failed_status_message(&config.language, &error),
                ui_tx,
            );
            return LspStartupHandshakeResult::Failed;
        }
    }

    if shutdown_signal_requested(shutdown_rx) {
        return LspStartupHandshakeResult::ShutdownRequested;
    }

    if let Err(error) = write_message(writer, &LspWireMessage::initialized().to_json()).await {
        send_lsp_startup_status(
            &config.language,
            root,
            generation,
            lsp_initialized_notification_failed_status_message(
                &config.language,
                &error.to_string(),
            ),
            ui_tx,
        );
        return LspStartupHandshakeResult::Failed;
    }

    if shutdown_signal_requested(shutdown_rx) {
        return LspStartupHandshakeResult::ShutdownRequested;
    }

    send_lsp_startup_status(
        &config.language,
        root,
        generation,
        lsp_startup_ready_status_message(&config.language),
        ui_tx,
    );
    send_lsp_server_ready(&config.language, root, generation, ui_tx);
    LspStartupHandshakeResult::Ready
}

async fn wait_for_initialize_response(
    writer: &mut ChildStdin,
    reader: &mut BufReader<ChildStdout>,
    language: &str,
    root: &Path,
    generation: u64,
    shutdown_rx: &mut watch::Receiver<bool>,
    ui_tx: &Sender<UiEvent>,
) -> Result<(), StartupHandshakeError> {
    if shutdown_signal_requested(shutdown_rx) {
        return Err(StartupHandshakeError::ShutdownRequested);
    }

    let mut startup_pending_requests = HashMap::<u64, PendingLspRequest>::new();
    let mut read_buffer = LspMessageReadBuffer::default();
    let deadline = Instant::now() + LSP_INITIALIZE_TIMEOUT;
    loop {
        let message = tokio::select! {
            biased;
            changed = shutdown_rx.changed() => {
                match changed {
                    Ok(()) if shutdown_signal_requested(shutdown_rx) => {
                        return Err(StartupHandshakeError::ShutdownRequested);
                    }
                    Ok(()) => {
                        continue;
                    }
                    Err(_) => {
                        return Err(StartupHandshakeError::ShutdownRequested);
                    }
                }
            }
            message = timeout_at(deadline, read_message(reader, &mut read_buffer)) => {
                message
                    .map_err(|_| {
                        StartupHandshakeError::Failed(
                            "timed out waiting for initialize response".to_owned(),
                        )
                    })?
                    .map_err(|error| StartupHandshakeError::Failed(error.to_string()))?
            }
        };

        let Some(value) = message else {
            return Err(StartupHandshakeError::Failed(
                "server closed stdout before initialize response".to_owned(),
            ));
        };

        match initialize_response_state(&value, LSP_INITIALIZE_REQUEST_ID) {
            InitializeResponseState::Ready => return Ok(()),
            InitializeResponseState::Failed(error) => {
                return Err(StartupHandshakeError::Failed(error));
            }
            InitializeResponseState::Waiting => {
                handle_lsp_server_message(
                    value,
                    language,
                    root,
                    generation,
                    &mut startup_pending_requests,
                    ui_tx,
                    writer,
                )
                .await;
            }
        }
    }
}

fn initialize_response_state(value: &Value, request_id: u64) -> InitializeResponseState {
    if value.get("id").and_then(Value::as_u64) != Some(request_id) {
        return InitializeResponseState::Waiting;
    }

    if let Some(error) = value.get("error") {
        return InitializeResponseState::Failed(initialize_error_summary(error));
    }

    if value.get("result").is_some() {
        InitializeResponseState::Ready
    } else {
        InitializeResponseState::Failed("initialize response missing result".to_owned())
    }
}

fn initialize_error_summary(error: &Value) -> String {
    error
        .get("message")
        .and_then(Value::as_str)
        .filter(|message| !message.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| error.to_string())
}

fn lsp_initialize_failed_status_message(language: &str, error: &str) -> String {
    lsp_status_display_message(&format!(
        "{} LSP initialize failed: {}",
        lsp_language_display_label(language),
        display_error_label_cow(error)
    ))
}

fn lsp_initialized_notification_failed_status_message(language: &str, error: &str) -> String {
    lsp_status_display_message(&format!(
        "{} LSP initialized notification failed: {}",
        lsp_language_display_label(language),
        display_error_label_cow(error)
    ))
}

fn lsp_startup_ready_status_message(language: &str) -> String {
    lsp_status_display_message(&lsp_server_ready_status(language))
}

fn send_lsp_startup_status(
    language: &str,
    root: &Path,
    generation: u64,
    message: String,
    ui_tx: &Sender<UiEvent>,
) {
    let _ = crate::ui_event_channel::send_ui_event(
        ui_tx,
        UiEvent::Lsp(LspUiEvent::Status {
            language: language.to_owned(),
            root: root.to_path_buf(),
            generation,
            message: lsp_status_display_message(&message),
        }),
    );
}

fn send_lsp_server_ready(language: &str, root: &Path, generation: u64, ui_tx: &Sender<UiEvent>) {
    let _ = crate::ui_event_channel::send_critical_ui_event(
        ui_tx,
        UiEvent::Lsp(LspUiEvent::ServerReady {
            language: language.to_owned(),
            root: root.to_path_buf(),
            generation,
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::{
        InitializeResponseState, StartupHandshakeError, initialize_response_state,
        lsp_initialize_failed_status_message, lsp_initialized_notification_failed_status_message,
        lsp_startup_ready_status_message, send_lsp_server_ready, send_lsp_startup_status,
        wait_for_initialize_response,
    };
    use crate::{
        lsp_runtime::LSP_STATUS_MESSAGE_MAX_CHARS, lsp_ui_events::LspUiEvent,
        ui_event_channel::ui_event_channel, ui_events::UiEvent,
    };
    use serde_json::json;
    use std::{path::PathBuf, process::Stdio};
    use tokio::{
        io::BufReader,
        process::{Child, ChildStdin, ChildStdout, Command},
        sync::watch,
    };

    #[test]
    fn initialize_response_state_waits_for_matching_id() {
        assert_eq!(
            initialize_response_state(&json!({"jsonrpc": "2.0", "method": "window/logMessage"}), 1),
            InitializeResponseState::Waiting
        );
        assert_eq!(
            initialize_response_state(&json!({"jsonrpc": "2.0", "id": 2, "result": {}}), 1),
            InitializeResponseState::Waiting
        );
    }

    #[test]
    fn initialize_response_state_accepts_result_for_matching_id() {
        assert_eq!(
            initialize_response_state(&json!({"jsonrpc": "2.0", "id": 1, "result": {}}), 1),
            InitializeResponseState::Ready
        );
    }

    #[test]
    fn initialize_response_state_reports_error_for_matching_id() {
        assert_eq!(
            initialize_response_state(
                &json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "error": {"code": -32603, "message": "workspace rejected"}
                }),
                1
            ),
            InitializeResponseState::Failed("workspace rejected".to_owned())
        );
    }

    #[test]
    fn initialize_response_error_status_sanitizes_newline_bidi_and_long_details() {
        let language = format!("rust\n{}\u{202e}", "language-fragment-".repeat(16));
        let error = format!(
            "first line\nsecond line \u{2066}{}",
            "error-fragment-".repeat(24)
        );

        let initialize_failed = lsp_initialize_failed_status_message(&language, &error);
        let initialized_failed =
            lsp_initialized_notification_failed_status_message(&language, &error);
        let ready = lsp_startup_ready_status_message(&language);

        for message in [initialize_failed, initialized_failed, ready] {
            assert_display_safe(&message);
            assert!(message.contains("..."));
            assert!(message.chars().count() <= LSP_STATUS_MESSAGE_MAX_CHARS);
        }
    }

    #[test]
    fn initialize_response_state_rejects_malformed_matching_response() {
        assert_eq!(
            initialize_response_state(&json!({"jsonrpc": "2.0", "id": 1}), 1),
            InitializeResponseState::Failed("initialize response missing result".to_owned())
        );
    }

    #[test]
    fn lsp_server_ready_event_names_language_and_root() {
        let (tx, rx) = ui_event_channel();
        let root = PathBuf::from("workspace");

        send_lsp_server_ready("rust", &root, 11, &tx);

        let event = rx.try_recv().expect("ready event should be queued");
        assert!(matches!(
            event,
            UiEvent::Lsp(LspUiEvent::ServerReady { language, root: event_root, generation })
                if language == "rust" && event_root == root && generation == 11
        ));
    }

    #[test]
    fn lsp_startup_status_event_sanitizes_message_but_preserves_raw_language() {
        let (tx, rx) = ui_event_channel();
        let root = PathBuf::from("workspace");
        let language = format!("rust\n{}\u{202e}", "language-fragment-".repeat(16));
        let error = format!(
            "first line\nsecond line \u{2066}{}",
            "error-fragment-".repeat(24)
        );

        send_lsp_startup_status(
            &language,
            &root,
            12,
            lsp_initialize_failed_status_message(&language, &error),
            &tx,
        );

        let event = rx.try_recv().expect("startup status should be queued");
        let UiEvent::Lsp(LspUiEvent::Status {
            language: event_language,
            root: event_root,
            generation,
            message,
        }) = event
        else {
            panic!("expected LSP status event");
        };
        assert_eq!(event_language, language);
        assert_eq!(event_root, root);
        assert_eq!(generation, 12);
        assert_display_safe(&message);
        assert!(message.contains("..."));
        assert!(message.chars().count() <= LSP_STATUS_MESSAGE_MAX_CHARS);
    }

    #[test]
    fn lsp_startup_ready_status_sanitizes_message_but_ready_event_preserves_raw_language() {
        let (tx, rx) = ui_event_channel();
        let root = PathBuf::from("workspace");
        let language = format!("rust\n{}\u{202e}", "language-fragment-".repeat(16));

        send_lsp_startup_status(
            &language,
            &root,
            13,
            lsp_startup_ready_status_message(&language),
            &tx,
        );
        send_lsp_server_ready(&language, &root, 13, &tx);

        let status_event = rx.try_recv().expect("ready status should be queued");
        let UiEvent::Lsp(LspUiEvent::Status {
            language: status_language,
            message,
            ..
        }) = status_event
        else {
            panic!("expected LSP status event");
        };
        assert_eq!(status_language, language);
        assert_display_safe(&message);
        assert!(message.contains("..."));
        assert!(message.chars().count() <= LSP_STATUS_MESSAGE_MAX_CHARS);

        let ready_event = rx.try_recv().expect("server ready should be queued");
        assert!(matches!(
            ready_event,
            UiEvent::Lsp(LspUiEvent::ServerReady { language: event_language, root: event_root, generation })
                if event_language == language && event_root == root && generation == 13
        ));
    }

    fn assert_display_safe(value: &str) {
        assert!(!value.chars().any(char::is_control), "{value:?}");
        assert!(!value.chars().any(is_bidi_format_control), "{value:?}");
    }

    fn is_bidi_format_control(ch: char) -> bool {
        matches!(
            ch,
            '\u{061c}'
                | '\u{200e}'
                | '\u{200f}'
                | '\u{202a}'..='\u{202e}'
                | '\u{2066}'..='\u{2069}'
        )
    }

    #[tokio::test]
    async fn wait_for_initialize_response_aborts_when_shutdown_is_requested() {
        let (mut child, mut writer, mut reader) = silent_child_with_stdio().await;
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        shutdown_tx
            .send(true)
            .expect("shutdown receiver should be live");
        let (ui_tx, _ui_rx) = ui_event_channel();
        let root = PathBuf::from("workspace");

        let result = wait_for_initialize_response(
            &mut writer,
            &mut reader,
            "rust",
            &root,
            7,
            &mut shutdown_rx,
            &ui_tx,
        )
        .await;

        assert_eq!(result, Err(StartupHandshakeError::ShutdownRequested));
        let _ = child.kill().await;
    }

    #[tokio::test]
    async fn wait_for_initialize_response_aborts_when_shutdown_sender_is_dropped() {
        let (mut child, mut writer, mut reader) = silent_child_with_stdio().await;
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        drop(shutdown_tx);
        let (ui_tx, _ui_rx) = ui_event_channel();
        let root = PathBuf::from("workspace");

        let result = wait_for_initialize_response(
            &mut writer,
            &mut reader,
            "rust",
            &root,
            8,
            &mut shutdown_rx,
            &ui_tx,
        )
        .await;

        assert_eq!(result, Err(StartupHandshakeError::ShutdownRequested));
        let _ = child.kill().await;
    }

    async fn silent_child_with_stdio() -> (Child, ChildStdin, BufReader<ChildStdout>) {
        #[cfg(windows)]
        let mut command = {
            let mut command = Command::new("cmd");
            command.args(["/C", "more > NUL"]);
            command
        };

        #[cfg(not(windows))]
        let mut command = {
            let mut command = Command::new("sh");
            command.args(["-c", "cat >/dev/null"]);
            command
        };

        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn silent child");
        let writer = child.stdin.take().expect("child stdin should be piped");
        let stdout = child.stdout.take().expect("child stdout should be piped");
        (child, writer, BufReader::new(stdout))
    }
}
