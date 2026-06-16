use crate::KuroyaApp;
use kuroya_core::{
    BufferId, MergeConflict, MergeConflictResolution, TextBuffer, merge_conflict_at_line,
};

impl KuroyaApp {
    pub(crate) fn resolve_active_merge_conflict(&mut self, resolution: MergeConflictResolution) {
        let Some(buffer_id) = self.active else {
            self.status = "No active file for merge conflict resolution".to_owned();
            return;
        };
        self.resolve_merge_conflict_for_buffer(buffer_id, resolution);
    }

    pub(crate) fn resolve_merge_conflict_for_buffer(
        &mut self,
        buffer_id: BufferId,
        resolution: MergeConflictResolution,
    ) {
        self.resolve_merge_conflict_for_buffer_target(buffer_id, resolution, None);
    }

    pub(crate) fn resolve_merge_conflict_for_buffer_at_line(
        &mut self,
        buffer_id: BufferId,
        line: usize,
        resolution: MergeConflictResolution,
    ) {
        self.resolve_merge_conflict_for_buffer_target(buffer_id, resolution, Some(line));
    }

    fn resolve_merge_conflict_for_buffer_target(
        &mut self,
        buffer_id: BufferId,
        resolution: MergeConflictResolution,
        line: Option<usize>,
    ) {
        let Some(buffer) = self.buffer(buffer_id) else {
            self.status = "No buffer for merge conflict resolution".to_owned();
            return;
        };
        if buffer.is_read_only() {
            self.status = "Cannot resolve merge conflict in read-only buffer".to_owned();
            return;
        }

        let Some(selection) = merge_conflict_selection_identity(buffer, line) else {
            self.status = merge_conflict_resolution_missing_status(line);
            return;
        };
        let changed = self.buffer_mut(buffer_id).is_some_and(|buffer| {
            merge_conflict_selection_identity_matches(buffer, &selection)
                && buffer.resolve_merge_conflict_at_line(selection.target_line, resolution)
        });
        if changed {
            self.mark_buffer_changed(buffer_id);
            self.status = merge_conflict_resolution_success_status(resolution);
        } else {
            self.status = "Merge conflict selection changed".to_owned();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MergeConflictSelectionIdentity {
    buffer_id: BufferId,
    buffer_version: u64,
    target_line: usize,
    conflict: MergeConflict,
}

fn merge_conflict_selection_identity(
    buffer: &TextBuffer,
    line: Option<usize>,
) -> Option<MergeConflictSelectionIdentity> {
    let target_line = line.unwrap_or_else(|| buffer.cursor_position().line);
    if target_line >= buffer.len_lines() {
        return None;
    }

    let conflicts = buffer.merge_conflicts();
    let conflict = merge_conflict_at_line(&conflicts, target_line)?.clone();
    Some(MergeConflictSelectionIdentity {
        buffer_id: buffer.id(),
        buffer_version: buffer.version(),
        target_line,
        conflict,
    })
}

fn merge_conflict_selection_identity_matches(
    buffer: &TextBuffer,
    selection: &MergeConflictSelectionIdentity,
) -> bool {
    if buffer.id() != selection.buffer_id || buffer.version() != selection.buffer_version {
        return false;
    }
    let conflicts = buffer.merge_conflicts();
    merge_conflict_at_line(&conflicts, selection.target_line)
        .is_some_and(|conflict| conflict == &selection.conflict)
}

fn merge_conflict_resolution_missing_status(line: Option<usize>) -> String {
    if line.is_some() {
        "No merge conflict at selected line".to_owned()
    } else {
        "No merge conflict at cursor".to_owned()
    }
}

pub(crate) fn merge_conflict_resolution_success_status(
    resolution: MergeConflictResolution,
) -> String {
    format!(
        "Accepted {} merge conflict",
        merge_conflict_resolution_label(resolution)
    )
}

fn merge_conflict_resolution_label(resolution: MergeConflictResolution) -> &'static str {
    match resolution {
        MergeConflictResolution::Current => "current",
        MergeConflictResolution::Incoming => "incoming",
        MergeConflictResolution::Both => "both",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        merge_conflict_resolution_missing_status, merge_conflict_selection_identity,
        merge_conflict_selection_identity_matches,
    };
    use kuroya_core::TextBuffer;

    #[test]
    fn conflict_selection_identity_tracks_buffer_version_line_and_range() {
        let mut buffer = TextBuffer::from_text(
            7,
            None,
            "one\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> feature\ntwo\n".to_owned(),
        );
        let selection = merge_conflict_selection_identity(&buffer, Some(2)).unwrap();

        assert!(merge_conflict_selection_identity_matches(
            &buffer, &selection
        ));

        buffer.replace_from_disk(
            "one\ninserted\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> feature\ntwo\n".to_owned(),
        );

        assert!(!merge_conflict_selection_identity_matches(
            &buffer, &selection
        ));
    }

    #[test]
    fn conflict_selection_identity_rejects_out_of_bounds_line_targets() {
        let buffer = TextBuffer::from_text(
            7,
            None,
            "one\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> feature\ntwo\n".to_owned(),
        );

        assert!(merge_conflict_selection_identity(&buffer, Some(buffer.len_lines())).is_none());
        assert!(merge_conflict_selection_identity(&buffer, Some(usize::MAX)).is_none());
    }

    #[test]
    fn conflict_resolution_missing_status_preserves_cursor_and_line_messages() {
        assert_eq!(
            merge_conflict_resolution_missing_status(Some(0)),
            "No merge conflict at selected line"
        );
        assert_eq!(
            merge_conflict_resolution_missing_status(None),
            "No merge conflict at cursor"
        );
    }
}
