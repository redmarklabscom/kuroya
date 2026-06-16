use crate::{
    KuroyaApp,
    large_file_mode::buffer_uses_large_file_mode,
    navigation_targets::{
        navigation_path_label, navigation_status_text, navigation_target_label,
        next_changed_line_kind, next_diff_hunk_header_line_for_buffer,
    },
};
use kuroya_core::{LanguageId, TextBuffer};
use std::fmt::Write as _;

impl KuroyaApp {
    pub(crate) fn goto_git_change(&mut self, direction: isize) {
        let Some(id) = self.active else {
            self.status = "No active file".to_owned();
            return;
        };

        if self
            .buffer(id)
            .is_some_and(|buffer| buffer.language() == LanguageId::Diff)
        {
            self.goto_diff_hunk(id, direction);
            return;
        }

        let Some((path, current_line, large_file_mode)) = self.buffer(id).and_then(|buffer| {
            let path = buffer.path()?.clone();
            Some((
                path,
                buffer.cursor_position().line + 1,
                !git_change_navigation_enabled(buffer),
            ))
        }) else {
            self.status = "No file-backed buffer for git changes".to_owned();
            return;
        };
        if large_file_mode {
            self.status = "Git change navigation is disabled in large file mode".to_owned();
            return;
        }

        let path_label = navigation_path_label(&path);
        let diff_lines = self.diff_lines_for(id);
        let Some(line) = next_changed_line_kind(&diff_lines, current_line, direction) else {
            if self.diff_lines_pending_for(id) {
                let mut status =
                    String::with_capacity("Loading git changes in ".len() + path_label.len());
                status.push_str("Loading git changes in ");
                status.push_str(&path_label);
                self.status = navigation_status_text(status);
                return;
            }
            let mut status = String::with_capacity("No git changes in ".len() + path_label.len());
            status.push_str("No git changes in ");
            status.push_str(&path_label);
            self.status = navigation_status_text(status);
            return;
        };

        self.apply_file_jump_with_history(id, line, 1);
        let label = if direction < 0 {
            "Previous git change"
        } else {
            "Next git change"
        };
        let mut status = String::with_capacity(label.len() + path_label.len() + 16);
        let _ = write!(status, "{label} at {path_label}:{line}");
        self.status = navigation_status_text(status);
    }

    pub(crate) fn goto_active_diff_hunk(&mut self, direction: isize) {
        let Some(id) = self.active else {
            self.status = "No active diff buffer".to_owned();
            return;
        };
        if !self
            .buffer(id)
            .is_some_and(|buffer| buffer.language() == LanguageId::Diff)
        {
            self.status = "No active diff buffer".to_owned();
            return;
        }

        self.goto_diff_hunk(id, direction);
    }

    fn goto_diff_hunk(&mut self, id: kuroya_core::BufferId, direction: isize) {
        let Some(line) = self.buffer(id).map(|buffer| {
            next_diff_hunk_header_line_for_buffer(
                buffer,
                buffer.cursor_position().line + 1,
                direction,
            )
        }) else {
            self.status = "No active diff buffer".to_owned();
            return;
        };

        let label = navigation_target_label(&self.buffer_label(id));
        let Some(line) = line else {
            let mut status = String::with_capacity("No diff hunks in ".len() + label.len());
            status.push_str("No diff hunks in ");
            status.push_str(&label);
            self.status = navigation_status_text(status);
            return;
        };

        self.apply_file_jump_with_history(id, line, 1);
        let direction_label = if direction < 0 {
            "Previous diff hunk"
        } else {
            "Next diff hunk"
        };
        let mut status = String::with_capacity(direction_label.len() + label.len() + 16);
        let _ = write!(status, "{direction_label} at {label}:{line}");
        self.status = navigation_status_text(status);
    }

    pub(crate) fn goto_matching_bracket(&mut self) {
        let Some(id) = self.active else {
            self.status = "No active file".to_owned();
            return;
        };

        let Some((line, column)) = self.buffer(id).and_then(|buffer| {
            let (_, target) = buffer.matching_bracket()?;
            let position = buffer.char_position(target);
            Some((position.line + 1, position.column + 1))
        }) else {
            self.status = "No matching bracket at cursor".to_owned();
            return;
        };

        self.apply_file_jump_with_history(id, line, column);
        let mut status = String::with_capacity(48);
        let _ = write!(status, "Matching bracket at line {line}, column {column}");
        self.status = status;
    }
}

fn git_change_navigation_enabled(buffer: &TextBuffer) -> bool {
    !buffer_uses_large_file_mode(buffer)
}

#[cfg(test)]
mod tests {
    use super::git_change_navigation_enabled;
    use crate::large_file_mode::LARGE_FILE_MODE_MAX_BYTES;
    use kuroya_core::TextBuffer;

    #[test]
    fn git_change_navigation_skips_large_file_mode_buffers() {
        let small = TextBuffer::from_text(1, None, "tracked\n".to_owned());
        let large = TextBuffer::from_text(2, None, "x".repeat(LARGE_FILE_MODE_MAX_BYTES + 1));

        assert!(git_change_navigation_enabled(&small));
        assert!(!git_change_navigation_enabled(&large));
    }
}
