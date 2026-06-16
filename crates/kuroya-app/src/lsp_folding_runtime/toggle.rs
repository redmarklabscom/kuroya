use crate::{
    KuroyaApp, folding::remove_fold_containing_line,
    lsp_lifecycle::background_language_block_reason, path_display::display_path_label_cow,
};
use kuroya_core::BufferId;

impl KuroyaApp {
    pub(crate) fn toggle_fold_at_cursor(&mut self) {
        let Some(id) = self.active else {
            self.status = "No active buffer to fold".to_owned();
            return;
        };
        let Some(line) = self
            .buffer(id)
            .map(|buffer| buffer.cursor_position().line + 1)
        else {
            self.status = "No active buffer to fold".to_owned();
            return;
        };
        self.toggle_fold_at_line(id, line);
    }

    pub(crate) fn toggle_fold_at_line(&mut self, id: BufferId, line: usize) {
        let Some(buffer) = self.buffer(id) else {
            self.status = "No buffer to fold".to_owned();
            return;
        };
        if let Some(reason) = background_language_block_reason(
            id,
            buffer,
            &self.lossy_decoded_buffers,
            &self.binary_preview_buffers,
        ) {
            self.status = reason.folding_status().to_owned();
            return;
        }
        let Some(path) = buffer.path().cloned() else {
            self.status = "Save the buffer before loading LSP folds".to_owned();
            return;
        };

        if self
            .folded_ranges
            .get_mut(&path)
            .is_some_and(|folded| remove_fold_containing_line(folded, line))
        {
            self.status = format!(
                "Expanded fold at {}:{}",
                display_path_label_cow(&path),
                line
            );
            return;
        }

        if self.folding_ranges.contains_key(&path) {
            self.apply_fold_at_line(&path, line);
            return;
        }

        self.pending_fold_line = Some((path.clone(), line));
        self.request_lsp_folding_ranges_for(id, path);
    }

    pub(crate) fn expand_all_folds(&mut self) {
        let Some(buffer) = self.active.and_then(|id| self.buffer(id)) else {
            self.status = "No active buffer to expand".to_owned();
            return;
        };
        let Some(path) = buffer.path().cloned() else {
            self.status = "No folds for untitled buffer".to_owned();
            return;
        };

        let removed = self
            .folded_ranges
            .remove(&path)
            .map(|folds| folds.len())
            .unwrap_or_default();
        let path_label = display_path_label_cow(&path);
        self.status = if removed == 0 {
            format!("No folded ranges in {path_label}")
        } else {
            format!("Expanded {removed} folds in {path_label}")
        };
    }
}
