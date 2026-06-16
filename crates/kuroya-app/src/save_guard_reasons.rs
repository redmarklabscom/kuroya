use crate::path_display::display_path_label;
use kuroya_core::{BufferId, TextBuffer};
use std::{collections::HashSet, path::Path};

#[cfg(test)]
pub(crate) fn save_needs_external_change_confirmation(
    id: BufferId,
    changed_on_disk: &HashSet<BufferId>,
    buffers: &[TextBuffer],
) -> bool {
    changed_on_disk.contains(&id)
        && buffers.iter().any(|buffer| {
            buffer.id() == id && buffer_needs_external_change_confirmation(buffer, changed_on_disk)
        })
}

pub(crate) fn buffer_needs_external_change_confirmation(
    buffer: &TextBuffer,
    changed_on_disk: &HashSet<BufferId>,
) -> bool {
    buffer_needs_external_change_confirmation_for_state(
        buffer.id(),
        buffer.is_dirty(),
        buffer.path().is_some(),
        changed_on_disk,
    )
}

pub(crate) fn workspace_switch_save_block_reason(
    dirty_ids: &[BufferId],
    buffers: &[TextBuffer],
    changed_on_disk: &HashSet<BufferId>,
    lossy_buffers: &HashSet<BufferId>,
    binary_buffers: &HashSet<BufferId>,
) -> Option<String> {
    dirty_buffer_save_block_reason(
        dirty_ids,
        buffers,
        changed_on_disk,
        lossy_buffers,
        binary_buffers,
        "switching",
    )
}

pub(crate) fn dirty_buffer_save_block_reason(
    dirty_ids: &[BufferId],
    buffers: &[TextBuffer],
    changed_on_disk: &HashSet<BufferId>,
    lossy_buffers: &HashSet<BufferId>,
    binary_buffers: &HashSet<BufferId>,
    action: &str,
) -> Option<String> {
    for id in dirty_ids {
        let Some(buffer) = buffers.iter().find(|buffer| buffer.id() == *id) else {
            continue;
        };
        if !buffer.is_dirty() {
            continue;
        }
        let buffer_id = buffer.id();
        let buffer_path = buffer.path().map(|path| path.as_path());
        let has_path = buffer_path.is_some();

        if let Some(reason) = protected_preview_save_block_reason_for_buffer_state(
            buffer,
            has_path,
            lossy_buffers,
            binary_buffers,
        ) {
            let display_name = buffer_display_name_for_path(buffer_path);
            return Some(format!("Cannot save {display_name}; {reason}"));
        }
        if !has_path {
            let display_name = buffer_display_name_for_path(buffer_path);
            return Some(format!(
                "{display_name} must be saved with Save As before {action}",
            ));
        }
        if buffer_needs_external_change_confirmation_for_state(
            buffer_id,
            buffer.is_dirty(),
            has_path,
            changed_on_disk,
        ) {
            let display_name = buffer_display_name_for_path(buffer_path);
            return Some(format!(
                "{display_name} changed on disk; resolve the save conflict before {action}",
            ));
        }
    }

    None
}

#[cfg(test)]
pub(crate) fn protected_preview_save_block_reason(
    id: BufferId,
    lossy_buffers: &HashSet<BufferId>,
    binary_buffers: &HashSet<BufferId>,
    buffers: &[TextBuffer],
) -> Option<&'static str> {
    let buffer = buffers.iter().find(|buffer| buffer.id() == id)?;
    protected_preview_save_block_reason_for_buffer(buffer, lossy_buffers, binary_buffers)
}

pub(crate) fn protected_preview_save_block_reason_for_buffer(
    buffer: &TextBuffer,
    lossy_buffers: &HashSet<BufferId>,
    binary_buffers: &HashSet<BufferId>,
) -> Option<&'static str> {
    protected_preview_save_block_reason_for_buffer_state(
        buffer,
        buffer.path().is_some(),
        lossy_buffers,
        binary_buffers,
    )
}

fn protected_preview_save_block_reason_for_buffer_state(
    buffer: &TextBuffer,
    has_path: bool,
    lossy_buffers: &HashSet<BufferId>,
    binary_buffers: &HashSet<BufferId>,
) -> Option<&'static str> {
    let id = buffer.id();
    if has_path && binary_buffers.contains(&id) {
        Some("binary previews are read-only")
    } else if has_path && lossy_buffers.contains(&id) {
        Some("file was decoded with replacement characters")
    } else if buffer.is_read_only() {
        Some("buffer is read-only")
    } else {
        None
    }
}

pub(crate) fn buffer_display_name(buffer: &TextBuffer) -> String {
    buffer_display_name_for_path(buffer.path().map(|path| path.as_path()))
}

fn buffer_needs_external_change_confirmation_for_state(
    id: BufferId,
    is_dirty: bool,
    has_path: bool,
    changed_on_disk: &HashSet<BufferId>,
) -> bool {
    changed_on_disk.contains(&id) && is_dirty && has_path
}

fn buffer_display_name_for_path(path: Option<&Path>) -> String {
    path.map(display_path_label)
        .unwrap_or_else(|| "Untitled".to_owned())
}

#[cfg(test)]
mod tests {
    use super::{buffer_display_name, dirty_buffer_save_block_reason};
    use kuroya_core::TextBuffer;
    use std::{collections::HashSet, path::PathBuf};

    #[test]
    fn buffer_display_name_sanitizes_path_labels() {
        let buffer = TextBuffer::from_text(
            7,
            Some(PathBuf::from(format!(
                "workspace/src/bad\n{}\u{202e}.rs",
                "very-long-name-".repeat(16)
            ))),
            "text".to_owned(),
        );

        let label = buffer_display_name(&buffer);

        assert!(!label.contains('\n'));
        assert!(!label.contains('\u{202e}'));
        assert!(label.contains("..."));
    }

    #[test]
    fn dirty_buffer_save_block_reason_keeps_raw_path_presence_for_fallback_labels() {
        let mut buffer = TextBuffer::from_text(
            8,
            Some(PathBuf::from("workspace/src/\n\u{202e}")),
            "text".to_owned(),
        );
        buffer.mark_dirty();

        let reason = dirty_buffer_save_block_reason(
            &[8],
            &[buffer],
            &HashSet::from([8]),
            &HashSet::new(),
            &HashSet::new(),
            "exiting",
        );

        assert_eq!(
            reason,
            Some(". changed on disk; resolve the save conflict before exiting".to_owned())
        );
    }

    #[test]
    fn dirty_buffer_save_block_reason_ignores_stale_clean_dirty_ids() {
        let clean_untitled = TextBuffer::new_untitled(7);
        let clean_changed = TextBuffer::from_text(
            8,
            Some(PathBuf::from("workspace/src/clean.rs")),
            "clean".to_owned(),
        );

        assert_eq!(
            dirty_buffer_save_block_reason(
                &[7, 8],
                &[clean_untitled, clean_changed],
                &HashSet::from([8]),
                &HashSet::new(),
                &HashSet::new(),
                "exiting",
            ),
            None
        );
    }
}
