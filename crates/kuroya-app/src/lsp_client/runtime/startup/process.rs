mod stdio;

use super::StartedLspClient;
use crate::lsp_runtime::{lsp_language_display_label, lsp_status_display_message};
use crate::path_display::sanitized_display_label_cow;
use crate::ui_event_channel::Sender;
use crate::{lsp_ui_events::LspUiEvent, ui_events::UiEvent};
use kuroya_core::LspServerConfig;
use std::path::Path;
use stdio::take_lsp_stdio;
use tokio::{io::BufReader, process::Command};

const LSP_COMMAND_STATUS_MAX_CHARS: usize = 24;
const LSP_UNAVAILABLE_DETAIL_MAX_CHARS: usize = 32;

pub(super) fn prepare_lsp_process_io(
    config: &LspServerConfig,
    root: &Path,
    generation: u64,
    ui_tx: &Sender<UiEvent>,
) -> Option<StartedLspClient> {
    let mut command = lsp_process_command(config, root);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            let _ = crate::ui_event_channel::send_critical_ui_event(
                ui_tx,
                UiEvent::Lsp(LspUiEvent::Status {
                    language: config.language.clone(),
                    root: root.to_path_buf(),
                    generation,
                    message: lsp_unavailable_status_message(
                        &config.language,
                        &config.command,
                        &error.to_string(),
                    ),
                }),
            );
            return None;
        }
    };

    let (writer, stdout) = take_lsp_stdio(&mut child, config, root, generation, ui_tx)?;

    Some(StartedLspClient {
        child,
        writer,
        reader: BufReader::new(stdout),
    })
}

fn lsp_process_command(config: &LspServerConfig, root: &Path) -> Command {
    let mut command = Command::new(&config.command);
    command
        .args(&config.args)
        .current_dir(root)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    command
}

fn lsp_unavailable_status_message(language: &str, command: &str, detail: &str) -> String {
    lsp_status_display_message(&format!(
        "{} LSP unavailable: failed to start {}: {}",
        lsp_language_display_label(language),
        lsp_command_display_label(command),
        lsp_unavailable_detail_label(detail)
    ))
}

fn lsp_command_display_label(command: &str) -> std::borrow::Cow<'_, str> {
    sanitized_display_label_cow(
        command.trim(),
        LSP_COMMAND_STATUS_MAX_CHARS,
        "configured command",
    )
}

fn lsp_unavailable_detail_label(detail: &str) -> std::borrow::Cow<'_, str> {
    sanitized_display_label_cow(detail, LSP_UNAVAILABLE_DETAIL_MAX_CHARS, "unknown error")
}

#[cfg(test)]
mod tests {
    use super::{lsp_process_command, lsp_unavailable_status_message, prepare_lsp_process_io};
    use crate::{
        lsp_runtime::LSP_STATUS_MESSAGE_MAX_CHARS,
        lsp_ui_events::LspUiEvent,
        ui_event_channel::{UI_EVENT_CHANNEL_BOUND, ui_event_channel},
        ui_events::UiEvent,
    };
    use kuroya_core::LspServerConfig;
    use std::{path::PathBuf, thread, time::Duration};

    #[test]
    fn lsp_process_is_killed_when_runtime_task_drops_child() {
        let config = LspServerConfig {
            language: "test".to_owned(),
            command: "test-language-server".to_owned(),
            args: vec!["--stdio".to_owned()],
            extensions: Vec::new(),
            root_markers: vec![],
        };

        let command = lsp_process_command(&config, &PathBuf::from("workspace"));

        assert!(command.get_kill_on_drop());
    }

    #[test]
    fn lsp_process_unavailable_status_sanitizes_language_and_error() {
        let language = format!("rust\n{}\u{202e}", "language-fragment-".repeat(16));
        let error = format!(
            "first line\nsecond line \u{2066}{}",
            "error-fragment-".repeat(24)
        );

        let command = "rust-analyzer\n--bad\u{2066}";
        let message = lsp_unavailable_status_message(&language, command, &error);

        assert_display_safe(&message);
        assert!(message.contains("failed to start"));
        assert!(message.contains("..."));
        assert!(message.chars().count() <= LSP_STATUS_MESSAGE_MAX_CHARS);
    }

    #[test]
    fn lsp_process_spawn_failure_preserves_raw_language_in_status_event() {
        let (tx, rx) = ui_event_channel();
        let language = format!("rust\n{}\u{202e}", "language-fragment-".repeat(16));
        let config = LspServerConfig {
            language: language.clone(),
            command: String::new(),
            args: Vec::new(),
            extensions: Vec::new(),
            root_markers: Vec::new(),
        };

        let started = prepare_lsp_process_io(&config, &PathBuf::from("."), 7, &tx);

        assert!(started.is_none());
        let event = rx
            .try_recv()
            .expect("spawn failure status should be queued");
        let UiEvent::Lsp(LspUiEvent::Status {
            language: event_language,
            root,
            generation,
            message,
        }) = event
        else {
            panic!("expected LSP status event");
        };
        assert_eq!(event_language, language);
        assert_eq!(root, PathBuf::from("."));
        assert_eq!(generation, 7);
        assert_display_safe(&message);
        assert!(message.chars().count() <= LSP_STATUS_MESSAGE_MAX_CHARS);
    }

    #[test]
    fn lsp_process_spawn_failure_status_survives_ui_backpressure() {
        let (tx, rx) = ui_event_channel();
        for index in 0..UI_EVENT_CHANNEL_BOUND {
            tx.try_send(UiEvent::Lsp(LspUiEvent::WorkDoneProgressCreated {
                token: format!("progress-token-{index}"),
            }))
            .expect("event should fit within channel bound");
        }
        let config = LspServerConfig {
            language: "rust".to_owned(),
            command: String::new(),
            args: Vec::new(),
            extensions: Vec::new(),
            root_markers: Vec::new(),
        };
        let status_tx = tx.clone();

        let sender = thread::spawn(move || {
            prepare_lsp_process_io(&config, &PathBuf::from("workspace"), 7, &status_tx).is_none()
        });

        let _ = rx
            .recv_timeout(Duration::from_secs(1))
            .expect("freeing capacity should unblock unavailable status sender");
        assert!(sender.join().unwrap());

        let mut delivered = false;
        for _ in 0..UI_EVENT_CHANNEL_BOUND {
            let event = rx
                .recv_timeout(Duration::from_secs(1))
                .expect("unavailable status should be queued after capacity is freed");
            if matches!(
                &event,
                UiEvent::Lsp(LspUiEvent::Status { language, root, generation, message })
                    if language == "rust"
                        && root == &PathBuf::from("workspace")
                        && *generation == 7
                        && message.starts_with("rust LSP unavailable: failed to start configured command:")
            ) {
                delivered = true;
                break;
            }
        }
        assert!(delivered);
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
}
