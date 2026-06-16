mod command_queue;
mod messages;
mod read_result;
mod startup;
mod status;

use super::{
    command_dispatch::{LspClientCommandOutcome, LspClientStopReason, handle_lsp_client_command},
    commands::LspClientCommand,
    pending::PendingLspRequest,
    response::emit_pending_lsp_request_failures_for_server,
    wire::{LspMessageReadBuffer, read_message},
};
use crate::lsp_ui_events::LspServerResultTarget;
use crate::ui_event_channel::Sender;
use crate::ui_events::UiEvent;
use kuroya_core::LspServerConfig;
use std::{collections::HashMap, path::PathBuf, time::Duration};
use tokio::{
    process::{Child, ChildStdin},
    sync::{mpsc, watch},
    time::{self, MissedTickBehavior},
};

use command_queue::LspClientCommandQueue;
use read_result::handle_lsp_read_result;
use startup::{StartedLspClient, start_lsp_process};
use status::send_lsp_stopped_status;

const LSP_CHILD_EXIT_POLL_INTERVAL: Duration = Duration::from_millis(250);

pub(super) async fn run_lsp_client(
    generation: u64,
    config: LspServerConfig,
    root: PathBuf,
    mut rx: mpsc::Receiver<LspClientCommand>,
    mut shutdown_rx: watch::Receiver<bool>,
    ui_tx: Sender<UiEvent>,
) {
    if shutdown_signal_requested(&mut shutdown_rx) {
        return;
    }

    let Some(StartedLspClient {
        mut child,
        mut writer,
        mut reader,
    }) = start_lsp_process(&config, &root, generation, &mut shutdown_rx, &ui_tx).await
    else {
        return;
    };

    let mut next_request_id = 3;
    let mut pending_requests: HashMap<u64, PendingLspRequest> = HashMap::new();
    let mut command_queue = LspClientCommandQueue::default();
    let mut read_buffer = LspMessageReadBuffer::default();
    let mut child_exit_poll = time::interval(LSP_CHILD_EXIT_POLL_INTERVAL);
    let mut shutdown_signal_closed = false;
    child_exit_poll.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let stop_reason = loop {
        tokio::select! {
            biased;
            changed = shutdown_rx.changed(), if !shutdown_signal_closed => {
                match changed {
                    Ok(()) if shutdown_signal_requested(&mut shutdown_rx) => {
                        break handle_lsp_shutdown_signal(
                            &mut writer,
                            &mut child,
                            &mut next_request_id,
                            &mut pending_requests,
                            &ui_tx,
                        )
                        .await;
                    }
                    Ok(()) => {}
                    Err(_) => {
                        shutdown_signal_closed = true;
                    }
                }
            }
            command = command_queue.recv(&mut rx) => {
                match handle_lsp_client_command(
                    command,
                    &mut writer,
                    &mut child,
                    &mut next_request_id,
                    &mut pending_requests,
                    &ui_tx,
                )
                .await
                {
                    LspClientCommandOutcome::Continue => {}
                    LspClientCommandOutcome::Stop(reason) => {
                        break reason;
                    }
                }
            }
            _ = child_exit_poll.tick() => {
                if let Some(reason) = poll_lsp_child_exit(&mut child) {
                    break reason;
                }
            }
            message = read_message(&mut reader, &mut read_buffer) => {
                if !handle_lsp_read_result(
                    message,
                    &config.language,
                    &root,
                    generation,
                    &mut pending_requests,
                    &ui_tx,
                    &mut writer,
                )
                .await
                {
                    break LspClientStopReason::Unexpected;
                }
            }
        }
    };

    messages::forget_created_work_done_progress_tokens_for_server(
        &config.language,
        &root,
        generation,
    );
    if should_emit_lsp_stopped_status(stop_reason) {
        emit_pending_lsp_request_failures_for_server(
            LspServerResultTarget {
                language: config.language.clone(),
                root: root.clone(),
                generation,
            },
            &mut pending_requests,
            &ui_tx,
        );
        send_lsp_stopped_status(&config.language, &root, generation, &ui_tx);
    }
}

async fn handle_lsp_shutdown_signal(
    writer: &mut ChildStdin,
    child: &mut Child,
    next_request_id: &mut u64,
    pending_requests: &mut HashMap<u64, PendingLspRequest>,
    ui_tx: &Sender<UiEvent>,
) -> LspClientStopReason {
    match handle_lsp_client_command(
        Some(LspClientCommand::Shutdown),
        writer,
        child,
        next_request_id,
        pending_requests,
        ui_tx,
    )
    .await
    {
        LspClientCommandOutcome::Continue => LspClientStopReason::Intentional,
        LspClientCommandOutcome::Stop(reason) => reason,
    }
}

fn should_emit_lsp_stopped_status(stop_reason: LspClientStopReason) -> bool {
    matches!(stop_reason, LspClientStopReason::Unexpected)
}

pub(super) fn shutdown_signal_requested(shutdown_rx: &mut watch::Receiver<bool>) -> bool {
    *shutdown_rx.borrow_and_update()
}

fn poll_lsp_child_exit(child: &mut Child) -> Option<LspClientStopReason> {
    lsp_child_exit_poll_stop_reason(child.try_wait())
}

fn lsp_child_exit_poll_stop_reason(
    result: std::io::Result<Option<std::process::ExitStatus>>,
) -> Option<LspClientStopReason> {
    match result {
        Ok(Some(_)) | Err(_) => Some(LspClientStopReason::Unexpected),
        Ok(None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopped_status_is_only_emitted_for_unexpected_stops() {
        assert!(should_emit_lsp_stopped_status(
            LspClientStopReason::Unexpected
        ));
        assert!(!should_emit_lsp_stopped_status(
            LspClientStopReason::Intentional
        ));
    }

    #[test]
    fn shutdown_signal_requested_reads_and_marks_latest_signal() {
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

        assert!(!shutdown_signal_requested(&mut shutdown_rx));

        shutdown_tx
            .send(true)
            .expect("shutdown receiver should be live");

        assert!(shutdown_signal_requested(&mut shutdown_rx));
        assert!(
            !shutdown_rx
                .has_changed()
                .expect("shutdown sender should still be live")
        );
    }

    #[test]
    fn child_exit_poll_continues_while_child_is_running() {
        assert_eq!(lsp_child_exit_poll_stop_reason(Ok(None)), None);
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn child_exit_poll_stops_when_child_has_exited() {
        assert_eq!(
            lsp_child_exit_poll_stop_reason(Ok(Some(success_exit_status()))),
            Some(LspClientStopReason::Unexpected)
        );
    }

    #[test]
    fn child_exit_poll_errors_stop_as_unexpected() {
        assert_eq!(
            lsp_child_exit_poll_stop_reason(Err(std::io::Error::other("poll failed"))),
            Some(LspClientStopReason::Unexpected)
        );
    }

    #[cfg(windows)]
    fn success_exit_status() -> std::process::ExitStatus {
        use std::os::windows::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(0)
    }

    #[cfg(unix)]
    fn success_exit_status() -> std::process::ExitStatus {
        use std::os::unix::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(0)
    }
}
