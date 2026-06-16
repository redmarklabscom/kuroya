use super::commands::LspClientCommand;
use crate::ui_event_channel::Sender as UiSender;
use crate::ui_events::UiEvent;
use kuroya_core::{LspRequestId, LspServerConfig};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::{
    runtime::Runtime,
    sync::{
        mpsc::{self, Sender as CommandSender},
        watch::{self, Sender as ShutdownSender},
    },
};

pub(crate) const LSP_COMMAND_QUEUE_CAPACITY: usize = 1024;
static NEXT_LSP_CLIENT_GENERATION: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
pub struct LspClientHandle {
    pub(super) tx: CommandSender<LspClientCommand>,
    shutdown_tx: ShutdownSender<bool>,
    pub(super) generation: u64,
}

impl LspClientHandle {
    pub fn spawn_on(
        runtime: &Runtime,
        config: LspServerConfig,
        root: PathBuf,
        ui_tx: UiSender<UiEvent>,
    ) -> Self {
        let (tx, rx) = mpsc::channel(LSP_COMMAND_QUEUE_CAPACITY);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let generation = NEXT_LSP_CLIENT_GENERATION.fetch_add(1, Ordering::Relaxed);
        let handle = Self {
            tx,
            shutdown_tx,
            generation,
        };

        runtime.spawn(async move {
            super::runtime::run_lsp_client(generation, config, root, rx, shutdown_rx, ui_tx).await;
        });

        handle
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    pub fn shutdown(&self) -> bool {
        let signaled = self.shutdown_tx.send(true).is_ok();
        let queued = self.queue_command(LspClientCommand::Shutdown);
        signaled || queued
    }

    pub fn apply_workspace_edit_response(
        &self,
        request_id: LspRequestId,
        applied: bool,
        failure_reason: Option<String>,
    ) -> bool {
        self.queue_command(LspClientCommand::ApplyWorkspaceEditResponse {
            request_id,
            applied,
            failure_reason,
        })
    }

    pub(super) fn queue_command(&self, command: LspClientCommand) -> bool {
        self.tx.try_send(command).is_ok()
    }

    #[cfg(test)]
    pub(crate) fn disconnected_for_test() -> Self {
        Self::disconnected_with_generation_for_test(1)
    }

    #[cfg(test)]
    pub(crate) fn disconnected_with_generation_for_test(generation: u64) -> Self {
        let (tx, _rx) = mpsc::channel(LSP_COMMAND_QUEUE_CAPACITY);
        Self::from_sender_for_test(tx, generation)
    }

    #[cfg(test)]
    pub(crate) fn accepting_for_test() -> Self {
        let (tx, rx) = mpsc::channel(LSP_COMMAND_QUEUE_CAPACITY);
        let _rx = Box::leak(Box::new(rx));
        Self::from_sender_for_test(tx, 1)
    }

    #[cfg(test)]
    pub(crate) fn full_queue_for_test() -> Self {
        let (tx, rx) = mpsc::channel(1);
        tx.try_send(LspClientCommand::DidSave {
            path: PathBuf::from("queued.rs"),
        })
        .expect("test queue should accept initial command");
        let _rx = Box::leak(Box::new(rx));
        Self::from_sender_for_test(tx, 1)
    }

    #[cfg(test)]
    pub(in crate::lsp_client) fn from_sender_for_test(
        tx: mpsc::Sender<LspClientCommand>,
        generation: u64,
    ) -> Self {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let _shutdown_rx = Box::leak(Box::new(shutdown_rx));
        Self {
            tx,
            shutdown_tx,
            generation,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LspClientHandle;
    use crate::lsp_client::commands::LspClientCommand;
    use kuroya_core::LspRequestId;
    use std::path::PathBuf;
    use tokio::sync::{mpsc, watch};

    #[test]
    fn lsp_command_queue_is_bounded_and_nonblocking() {
        let (tx, _rx) = mpsc::channel(1);
        let handle = LspClientHandle::from_sender_for_test(tx, 1);

        assert!(handle.queue_command(LspClientCommand::DidSave {
            path: PathBuf::from("src/main.rs"),
        }));
        assert!(!handle.queue_command(LspClientCommand::DidSave {
            path: PathBuf::from("src/lib.rs"),
        }));
    }

    #[test]
    fn apply_workspace_edit_response_queues_direct_response_command() {
        let (tx, mut rx) = mpsc::channel(1);
        let handle = LspClientHandle::from_sender_for_test(tx, 1);

        assert!(handle.apply_workspace_edit_response(
            LspRequestId::Number(17),
            false,
            Some("buffer changed".to_owned())
        ));

        match rx.try_recv() {
            Ok(LspClientCommand::ApplyWorkspaceEditResponse {
                request_id,
                applied,
                failure_reason,
            }) => {
                assert_eq!(request_id, LspRequestId::Number(17));
                assert!(!applied);
                assert_eq!(failure_reason.as_deref(), Some("buffer changed"));
            }
            other => panic!("expected apply-edit response command, got {other:?}"),
        }
    }

    #[test]
    fn shutdown_signals_even_when_command_queue_is_full() {
        let (tx, _rx) = mpsc::channel(1);
        tx.try_send(LspClientCommand::DidSave {
            path: PathBuf::from("queued.rs"),
        })
        .expect("test queue should accept initial command");
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        let handle = LspClientHandle {
            tx,
            shutdown_tx,
            generation: 1,
        };

        assert!(!handle.queue_command(LspClientCommand::DidSave {
            path: PathBuf::from("blocked.rs"),
        }));
        assert!(handle.shutdown());

        assert!(
            shutdown_rx.has_changed().expect("shutdown sender is live"),
            "shutdown signal should not depend on command queue capacity"
        );
        assert!(*shutdown_rx.borrow_and_update());
    }
}
