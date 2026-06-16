use crate::{
    KuroyaApp, lsp_lifecycle::background_language_block_reason,
    lsp_runtime::lsp_command_queue_failed_status, path_display::display_path_label_cow,
    runtime_ticks::FORMAT_ON_TYPE_DEBOUNCE,
};
use eframe::egui::Context;
use kuroya_core::BufferId;
#[cfg(test)]
use std::path::Path;
use std::time::Instant;

impl KuroyaApp {
    pub(crate) fn request_lsp_formatting(&mut self) {
        let Some(id) = self.active else {
            self.status = "No active document to format".to_owned();
            return;
        };
        let _ = self.request_lsp_formatting_for_buffer(id, Some("Requesting format for"), true);
    }

    pub(crate) fn schedule_lsp_format_on_type_for_buffer(&mut self, ctx: &Context, id: BufferId) {
        self.pending_format_on_type_requests
            .insert(id, Instant::now());
        ctx.request_repaint_after(FORMAT_ON_TYPE_DEBOUNCE);
    }

    pub(crate) fn request_lsp_formatting_for_buffer(
        &mut self,
        id: BufferId,
        status_prefix: Option<&str>,
        report_failures: bool,
    ) -> Option<u64> {
        self.pending_format_on_type_requests.remove(&id);
        let Some(buffer) = self.buffer(id) else {
            if report_failures {
                self.status = "No document to format".to_owned();
            }
            return None;
        };
        if let Some(reason) = background_language_block_reason(
            id,
            buffer,
            &self.lossy_decoded_buffers,
            &self.binary_preview_buffers,
        ) {
            if report_failures {
                self.status = reason.formatting_status().to_owned();
            }
            return None;
        }
        let Some(path) = buffer.path().cloned() else {
            if report_failures {
                self.status = "Save the document before formatting".to_owned();
            }
            return None;
        };
        let version = buffer.version();
        let Some(client) = self.ensure_lsp_for_buffer(id) else {
            if report_failures {
                self.status = "No LSP server configured for this buffer".to_owned();
            }
            return None;
        };

        let indent = self.indent_options_for_buffer(id);
        let request_id = next_lsp_formatting_request_id(self.formatting_next_request_id);
        let path_label = display_path_label_cow(&path);
        if !client.formatting(
            request_id,
            id,
            path.clone(),
            version,
            indent.tab_size,
            indent.insert_spaces,
        ) {
            if report_failures {
                self.status = lsp_command_queue_failed_status("textDocument/formatting");
            }
            return None;
        }
        self.formatting_next_request_id = request_id;
        self.completion_open = false;
        self.lsp_hover = None;
        if let Some(status_prefix) = status_prefix {
            self.status =
                lsp_formatting_request_status_for_label(status_prefix, path_label.as_ref());
        }
        self.record_lsp_client_trace("textDocument/formatting", path_label);
        Some(request_id)
    }
}

fn next_lsp_formatting_request_id(current: u64) -> u64 {
    current.checked_add(1).unwrap_or(1)
}

#[cfg(test)]
fn lsp_formatting_request_status(status_prefix: &str, path: &Path) -> String {
    lsp_formatting_request_status_for_label(status_prefix, display_path_label_cow(path).as_ref())
}

fn lsp_formatting_request_status_for_label(status_prefix: &str, path_label: &str) -> String {
    format!("{status_prefix} {path_label}")
}

#[cfg(test)]
mod tests {
    use super::{lsp_formatting_request_status, next_lsp_formatting_request_id};
    use crate::path_display::{DISPLAY_PATH_LABEL_MAX_CHARS, display_path_label_cow};
    use std::path::Path;

    #[test]
    fn formatting_request_status_sanitizes_and_bounds_path_label() {
        let path = Path::new("workspace/src")
            .join(format!("bad\n{}\u{202e}fmt.rs", "very-long-".repeat(32)));

        let status = lsp_formatting_request_status("Requesting format for", &path);
        let label = display_path_label_cow(&path);

        assert_eq!(status, format!("Requesting format for {label}"));
        assert!(!status.contains('\n'));
        assert!(!status.contains('\u{202e}'));
        assert!(label.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);
        assert!(label.contains("..."));
    }

    #[test]
    fn formatting_request_ids_skip_zero_after_wrap() {
        assert_eq!(next_lsp_formatting_request_id(0), 1);
        assert_eq!(next_lsp_formatting_request_id(41), 42);
        assert_eq!(next_lsp_formatting_request_id(u64::MAX), 1);
    }
}
