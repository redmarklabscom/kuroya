use kuroya_core::{BufferId, TextBuffer};
use std::collections::HashSet;

#[cfg(test)]
pub(crate) use crate::save_guard_reasons::protected_preview_save_block_reason;
#[cfg(test)]
pub(crate) use crate::save_guard_reasons::save_needs_external_change_confirmation;
pub(crate) use crate::save_guard_reasons::{
    buffer_display_name, dirty_buffer_save_block_reason, workspace_switch_save_block_reason,
};
use crate::save_guard_reasons::{
    buffer_needs_external_change_confirmation, protected_preview_save_block_reason_for_buffer,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SaveAllBlocker {
    Untitled(BufferId),
    ExternalChange(BufferId),
    ProtectedPreview(BufferId, &'static str),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct SaveAllPlan {
    pub(crate) savable: Vec<BufferId>,
    pub(crate) first_blocker: Option<SaveAllBlocker>,
}

pub(crate) fn dirty_buffer_ids(buffers: &[TextBuffer]) -> Vec<BufferId> {
    let mut ids = Vec::new();
    for buffer in buffers {
        if buffer.is_dirty() {
            ids.push(buffer.id());
        }
    }
    ids
}

pub(crate) fn autosave_buffer_ids(
    buffers: &[TextBuffer],
    changed_on_disk: &HashSet<BufferId>,
    blocked_buffers: &HashSet<BufferId>,
    lossy_buffers: &HashSet<BufferId>,
    binary_buffers: &HashSet<BufferId>,
) -> Vec<BufferId> {
    let mut ids = Vec::new();
    for buffer in buffers {
        let id = buffer.id();
        if buffer.is_dirty()
            && buffer.path().is_some()
            && !changed_on_disk.contains(&id)
            && !blocked_buffers.contains(&id)
            && protected_preview_save_block_reason_for_buffer(buffer, lossy_buffers, binary_buffers)
                .is_none()
        {
            ids.push(id);
        }
    }
    ids
}

pub(crate) fn plan_save_all_dirty_buffers(
    buffers: &[TextBuffer],
    changed_on_disk: &HashSet<BufferId>,
    lossy_buffers: &HashSet<BufferId>,
    binary_buffers: &HashSet<BufferId>,
) -> SaveAllPlan {
    let mut plan = SaveAllPlan::default();

    for buffer in buffers.iter().filter(|buffer| buffer.is_dirty()) {
        let id = buffer.id();
        if let Some(reason) =
            protected_preview_save_block_reason_for_buffer(buffer, lossy_buffers, binary_buffers)
        {
            plan.first_blocker
                .get_or_insert(SaveAllBlocker::ProtectedPreview(id, reason));
            continue;
        }
        if buffer.path().is_none() {
            plan.first_blocker
                .get_or_insert(SaveAllBlocker::Untitled(id));
            continue;
        }
        if buffer_needs_external_change_confirmation(buffer, changed_on_disk) {
            plan.first_blocker
                .get_or_insert(SaveAllBlocker::ExternalChange(id));
            continue;
        }

        plan.savable.push(id);
    }

    plan
}
