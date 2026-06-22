use super::super::{commands::LspClientCommand, handle::LspClientHandle};
use kuroya_core::{BufferId, TextSnapshot};
use std::path::PathBuf;

impl LspClientHandle {
    pub fn did_open(
        &self,
        id: BufferId,
        path: PathBuf,
        language: String,
        version: u64,
        text: TextSnapshot,
    ) -> bool {
        self.queue_command(LspClientCommand::DidOpen {
            id,
            path,
            language,
            version,
            text,
        })
    }

    pub fn did_change(
        &self,
        id: BufferId,
        path: PathBuf,
        version: u64,
        text: TextSnapshot,
    ) -> bool {
        self.queue_command(LspClientCommand::DidChange {
            id,
            path,
            version,
            text,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuroya_core::TextBuffer;
    use tokio::sync::mpsc;

    #[test]
    fn did_open_sends_text_snapshot_without_requiring_string_text() {
        let (tx, mut rx) = mpsc::channel(1);
        let handle = LspClientHandle::from_sender_for_test(tx, 1);
        let mut buffer = TextBuffer::from_text(7, None, "alpha".to_owned());
        let snapshot = buffer.text_snapshot();

        buffer.insert_at_cursor(" beta");
        assert!(handle.did_open(
            buffer.id(),
            PathBuf::from("src/main.rs"),
            "rust".to_owned(),
            buffer.version(),
            snapshot,
        ));

        let command = rx.try_recv().expect("did_open command should be sent");
        match command {
            LspClientCommand::DidOpen { text, .. } => assert_eq!(text.text(), "alpha"),
            other => panic!("expected didOpen command, got {other:?}"),
        }
    }

    #[test]
    fn did_change_sends_text_snapshot_without_requiring_string_text() {
        let (tx, mut rx) = mpsc::channel(1);
        let handle = LspClientHandle::from_sender_for_test(tx, 1);
        let mut buffer = TextBuffer::from_text(8, None, "alpha".to_owned());
        let snapshot = buffer.text_snapshot();

        buffer.insert_at_cursor(" beta");
        assert!(handle.did_change(
            buffer.id(),
            PathBuf::from("src/main.rs"),
            buffer.version(),
            snapshot,
        ));

        let command = rx.try_recv().expect("did_change command should be sent");
        match command {
            LspClientCommand::DidChange { text, .. } => assert_eq!(text.text(), "alpha"),
            other => panic!("expected didChange command, got {other:?}"),
        }
    }
}
