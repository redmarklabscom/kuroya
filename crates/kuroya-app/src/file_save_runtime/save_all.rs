use crate::{
    KuroyaApp,
    save_lifecycle::{SaveAllBlocker, plan_save_all_dirty_buffers},
    ui_text::count_label,
};

impl KuroyaApp {
    pub(crate) fn save_all_dirty_buffers(&mut self) {
        let merged_changed_on_disk;
        let changed_on_disk = if self.has_pending_reload_external_change_sources() {
            merged_changed_on_disk = self.observed_external_change_buffer_ids();
            &merged_changed_on_disk
        } else {
            &self.external_change_buffers
        };
        let plan = plan_save_all_dirty_buffers(
            &self.buffers,
            changed_on_disk,
            &self.lossy_decoded_buffers,
            &self.binary_preview_buffers,
        );
        let save_count = plan.savable.len();

        for id in plan.savable {
            self.spawn_save(id);
        }

        if let Some(blocker) = plan.first_blocker {
            match blocker {
                SaveAllBlocker::Untitled(id) => {
                    self.begin_save_as(id);
                }
                SaveAllBlocker::ExternalChange(id) => {
                    self.save_conflict_buffer.get_or_insert(id);
                    self.set_active_buffer(id);
                    self.status = format!("{} changed on disk", self.file_io_buffer_label(id));
                }
                SaveAllBlocker::ProtectedPreview(id, reason) => {
                    self.set_active_buffer(id);
                    self.block_protected_preview_save(id, reason);
                }
            }
            return;
        }

        self.status = save_all_dirty_buffers_status(save_count);
    }
}

fn save_all_dirty_buffers_status(save_count: usize) -> String {
    if save_count == 0 {
        "No unsaved files to save".to_owned()
    } else {
        format!("Saving {}", count_label(save_count, "file", "files"))
    }
}

#[cfg(test)]
mod tests {
    use super::save_all_dirty_buffers_status;

    #[test]
    fn save_all_dirty_buffers_status_formats_count() {
        assert_eq!(save_all_dirty_buffers_status(0), "No unsaved files to save");
        assert_eq!(save_all_dirty_buffers_status(1), "Saving 1 file");
        assert_eq!(save_all_dirty_buffers_status(2), "Saving 2 files");
    }
}
