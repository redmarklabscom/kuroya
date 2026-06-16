use crate::KuroyaApp;
use kuroya_core::BufferId;

impl KuroyaApp {
    pub(crate) fn toggle_line_comment_for_buffer(&mut self, id: BufferId) {
        let comments_insert_space = self.settings.comments_insert_space;
        let comments_ignore_empty_lines = self.settings.comments_ignore_empty_lines;
        let Some(buffer) = self.buffer_mut(id) else {
            self.status = "No line comment syntax for this file".to_owned();
            return;
        };
        let Some(prefix) = buffer.language().line_comment_prefix() else {
            self.status = "No line comment syntax for this file".to_owned();
            return;
        };

        let changed = buffer.toggle_line_comments_with_options(
            prefix,
            comments_insert_space,
            comments_ignore_empty_lines,
        );
        if changed {
            self.mark_buffer_changed(id);
            self.status = "Toggled line comments".to_owned();
        } else {
            self.status = "No lines to comment".to_owned();
        }
    }
}
