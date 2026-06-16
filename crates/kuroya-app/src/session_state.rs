use crate::{
    persistence::{
        BufferHistoryState, BufferSelectionState, BufferViewState, PaneBufferViewState,
        RecoveredBufferHistoryState, RecoveredBufferViewState, normalize_recent_projects,
    },
    recovery::{recovery_path_key, recovery_path_winners},
    workspace_state::{PaneId, paths_match_exact_or_lexically},
};
use kuroya_core::{BufferId, Selection, TextBuffer, clamp_editor_line_height};
use std::{collections::HashMap, path::PathBuf};

const SESSION_HISTORY_MAX_ENTRIES_PER_STACK: usize = 64;
const SESSION_HISTORY_MAX_BYTES_PER_BUFFER: usize = 256 * 1024;
const SESSION_HISTORY_MAX_TOTAL_BYTES: usize = 4 * 1024 * 1024;
const SESSION_VIEW_MAX_SELECTIONS_PER_BUFFER: usize = 256;

#[derive(Debug, Clone)]
struct CapturedBufferViewState {
    cursor_line: usize,
    cursor_column: usize,
    scroll_line: usize,
    horizontal_scroll_offset: f32,
    selections: Vec<BufferSelectionState>,
}

#[derive(Debug, Clone)]
pub(crate) struct EditorPane {
    pub(crate) id: PaneId,
    pub(crate) active: Option<BufferId>,
    pub(crate) weight: f32,
}

pub(crate) fn editor_row_height(font_size: f32, configured_line_height: f32) -> f32 {
    let configured = clamp_editor_line_height(configured_line_height);
    if configured > 0.0 {
        configured
    } else {
        (font_size + 7.0).max(18.0)
    }
}

fn scroll_line_from_offset(offset: f32, row_height: f32) -> usize {
    if !offset.is_finite() || row_height <= 0.0 {
        return 1;
    }
    (offset.max(0.0) / row_height).floor() as usize + 1
}

pub(crate) fn session_view_states(
    buffers: &[TextBuffer],
    panes: &[EditorPane],
    scroll_offsets: &HashMap<(PaneId, BufferId), f32>,
    horizontal_scroll_offsets: &HashMap<(PaneId, BufferId), f32>,
    active_pane: PaneId,
    row_height: f32,
) -> Vec<BufferViewState> {
    buffers
        .iter()
        .filter_map(|buffer| {
            let path = buffer.path()?.clone();
            let captured = capture_buffer_view_state(
                buffer,
                panes,
                scroll_offsets,
                horizontal_scroll_offsets,
                active_pane,
                row_height,
            );

            Some(BufferViewState {
                path,
                cursor_line: captured.cursor_line,
                cursor_column: captured.cursor_column,
                scroll_line: captured.scroll_line,
                horizontal_scroll_offset: captured.horizontal_scroll_offset,
                selections: captured.selections,
            })
        })
        .collect()
}

pub(crate) fn session_recovery_view_states(
    buffers: &[TextBuffer],
    panes: &[EditorPane],
    scroll_offsets: &HashMap<(PaneId, BufferId), f32>,
    horizontal_scroll_offsets: &HashMap<(PaneId, BufferId), f32>,
    active_pane: PaneId,
    row_height: f32,
    recovery_buffer_max_bytes: usize,
    recovery_session_max_bytes: usize,
) -> Vec<RecoveredBufferViewState> {
    recoverable_session_buffers(
        buffers,
        recovery_buffer_max_bytes,
        recovery_session_max_bytes,
    )
    .into_iter()
    .filter(|(_, buffer)| buffer.path().is_none())
    .map(|(recovery_index, buffer)| {
        let captured = capture_buffer_view_state(
            buffer,
            panes,
            scroll_offsets,
            horizontal_scroll_offsets,
            active_pane,
            row_height,
        );
        RecoveredBufferViewState {
            recovery_index,
            cursor_line: captured.cursor_line,
            cursor_column: captured.cursor_column,
            scroll_line: captured.scroll_line,
            horizontal_scroll_offset: captured.horizontal_scroll_offset,
            selections: captured.selections,
        }
    })
    .collect()
}

pub(crate) fn session_pane_view_states(
    buffers: &[TextBuffer],
    panes: &[EditorPane],
    scroll_offsets: &HashMap<(PaneId, BufferId), f32>,
    horizontal_scroll_offsets: &HashMap<(PaneId, BufferId), f32>,
    row_height: f32,
) -> Vec<PaneBufferViewState> {
    panes
        .iter()
        .enumerate()
        .filter_map(|(pane_index, pane)| {
            let buffer_id = pane.active?;
            let buffer = buffers.iter().find(|buffer| buffer.id() == buffer_id)?;
            let path = buffer.path()?.clone();
            let scroll_line = scroll_offsets
                .get(&(pane.id, buffer_id))
                .copied()
                .filter(|offset| offset.is_finite())
                .map(|offset| scroll_line_from_offset(offset, row_height))
                .unwrap_or_else(|| buffer.cursor_position().line + 1);
            let horizontal_scroll_offset = horizontal_scroll_offsets
                .get(&(pane.id, buffer_id))
                .copied()
                .map(sanitized_scroll_offset)
                .unwrap_or_default();

            Some(PaneBufferViewState {
                pane_index,
                path,
                scroll_line,
                horizontal_scroll_offset,
            })
        })
        .collect()
}

fn capture_buffer_view_state(
    buffer: &TextBuffer,
    panes: &[EditorPane],
    scroll_offsets: &HashMap<(PaneId, BufferId), f32>,
    horizontal_scroll_offsets: &HashMap<(PaneId, BufferId), f32>,
    active_pane: PaneId,
    row_height: f32,
) -> CapturedBufferViewState {
    let cursor = buffer.cursor_position();
    let scroll_line =
        session_scroll_offset_for_buffer(buffer.id(), active_pane, panes, scroll_offsets)
            .map(|offset| scroll_line_from_offset(offset, row_height))
            .unwrap_or(cursor.line + 1);
    let horizontal_scroll_offset = session_scroll_offset_for_buffer(
        buffer.id(),
        active_pane,
        panes,
        horizontal_scroll_offsets,
    )
    .map(sanitized_scroll_offset)
    .unwrap_or_default();

    CapturedBufferViewState {
        cursor_line: cursor.line + 1,
        cursor_column: cursor.column + 1,
        scroll_line,
        horizontal_scroll_offset,
        selections: buffer
            .selections()
            .iter()
            .take(SESSION_VIEW_MAX_SELECTIONS_PER_BUFFER)
            .copied()
            .map(|selection| buffer_selection_state(buffer, selection))
            .collect(),
    }
}

fn sanitized_scroll_offset(offset: f32) -> f32 {
    if offset.is_finite() {
        offset.max(0.0)
    } else {
        0.0
    }
}

fn session_scroll_offset_for_buffer(
    buffer_id: BufferId,
    active_pane: PaneId,
    panes: &[EditorPane],
    scroll_offsets: &HashMap<(PaneId, BufferId), f32>,
) -> Option<f32> {
    if let Some(offset) = scroll_offsets
        .get(&(active_pane, buffer_id))
        .copied()
        .filter(|offset| offset.is_finite())
    {
        return Some(offset);
    }

    panes
        .iter()
        .filter(|pane| pane.active == Some(buffer_id))
        .filter_map(|pane| {
            scroll_offsets
                .get(&(pane.id, buffer_id))
                .copied()
                .filter(|offset| offset.is_finite())
        })
        .next()
        .or_else(|| {
            scroll_offsets
                .iter()
                .filter_map(|((pane_id, id), offset)| {
                    (*id == buffer_id && offset.is_finite()).then_some((*pane_id, *offset))
                })
                .min_by_key(|(pane_id, _)| *pane_id)
                .map(|(_, offset)| offset)
        })
}

pub(crate) fn apply_buffer_view_state(buffer: &mut TextBuffer, state: &BufferViewState) -> usize {
    if !buffer
        .path()
        .is_some_and(|path| paths_match_exact_or_lexically(path, &state.path))
    {
        return current_buffer_scroll_line(buffer);
    }

    apply_view_state_parts(
        buffer,
        state.cursor_line,
        state.cursor_column,
        state.scroll_line,
        &state.selections,
    )
}

fn current_buffer_scroll_line(buffer: &TextBuffer) -> usize {
    buffer
        .cursor_position()
        .line
        .min(buffer.len_lines().saturating_sub(1))
}

pub(crate) fn apply_recovered_buffer_view_state(
    buffer: &mut TextBuffer,
    state: &RecoveredBufferViewState,
) -> usize {
    apply_view_state_parts(
        buffer,
        state.cursor_line,
        state.cursor_column,
        state.scroll_line,
        &state.selections,
    )
}

fn apply_view_state_parts(
    buffer: &mut TextBuffer,
    cursor_line: usize,
    cursor_column: usize,
    scroll_line: usize,
    selections: &[BufferSelectionState],
) -> usize {
    let max_line = buffer.len_lines().saturating_sub(1);
    let cursor_line = cursor_line.saturating_sub(1).min(max_line);
    let cursor_column = cursor_column.saturating_sub(1);
    let cursor = buffer.line_column_to_char(cursor_line, cursor_column);
    if selections.is_empty() {
        buffer.set_single_cursor(cursor);
    } else {
        let selections = selections
            .iter()
            .take(SESSION_VIEW_MAX_SELECTIONS_PER_BUFFER)
            .map(|selection| selection_from_buffer_view_state(buffer, selection))
            .collect::<Vec<_>>();
        buffer.set_selections(selections);
    }
    scroll_line.saturating_sub(1).min(max_line)
}

pub(crate) fn horizontal_scroll_offset_from_view_state(state: &BufferViewState) -> f32 {
    sanitized_scroll_offset(state.horizontal_scroll_offset)
}

pub(crate) fn pane_scroll_line_from_view_state(
    buffer: &TextBuffer,
    state: &PaneBufferViewState,
) -> usize {
    state
        .scroll_line
        .saturating_sub(1)
        .min(buffer.len_lines().saturating_sub(1))
}

pub(crate) fn horizontal_scroll_offset_from_pane_view_state(state: &PaneBufferViewState) -> f32 {
    sanitized_scroll_offset(state.horizontal_scroll_offset)
}

pub(crate) fn horizontal_scroll_offset_from_recovered_view_state(
    state: &RecoveredBufferViewState,
) -> f32 {
    sanitized_scroll_offset(state.horizontal_scroll_offset)
}

fn buffer_selection_state(buffer: &TextBuffer, selection: Selection) -> BufferSelectionState {
    let anchor = buffer.char_position(selection.anchor);
    let cursor = buffer.char_position(selection.cursor);
    BufferSelectionState {
        anchor_line: anchor.line + 1,
        anchor_column: anchor.column + 1,
        cursor_line: cursor.line + 1,
        cursor_column: cursor.column + 1,
    }
}

fn selection_from_buffer_view_state(
    buffer: &TextBuffer,
    state: &BufferSelectionState,
) -> Selection {
    Selection {
        anchor: buffer.line_column_to_char(
            state.anchor_line.saturating_sub(1),
            state.anchor_column.saturating_sub(1),
        ),
        cursor: buffer.line_column_to_char(
            state.cursor_line.saturating_sub(1),
            state.cursor_column.saturating_sub(1),
        ),
    }
}

pub(crate) fn session_history_states(
    buffers: &[TextBuffer],
    active: Option<BufferId>,
) -> Vec<BufferHistoryState> {
    session_history_states_with_limits(
        buffers,
        active,
        SESSION_HISTORY_MAX_ENTRIES_PER_STACK,
        SESSION_HISTORY_MAX_BYTES_PER_BUFFER,
        SESSION_HISTORY_MAX_TOTAL_BYTES,
    )
}

fn session_history_states_with_limits(
    buffers: &[TextBuffer],
    active: Option<BufferId>,
    max_entries_per_stack: usize,
    max_bytes_per_buffer: usize,
    max_total_bytes: usize,
) -> Vec<BufferHistoryState> {
    let mut states = Vec::new();
    let mut total_history_bytes = 0usize;

    if let Some(active_buffer) =
        active.and_then(|active| buffers.iter().find(|buffer| buffer.id() == active))
    {
        push_session_history_state(
            &mut states,
            &mut total_history_bytes,
            active_buffer,
            max_entries_per_stack,
            max_bytes_per_buffer,
            max_total_bytes,
        );
    }

    for buffer in buffers {
        if Some(buffer.id()) == active {
            continue;
        }
        push_session_history_state(
            &mut states,
            &mut total_history_bytes,
            buffer,
            max_entries_per_stack,
            max_bytes_per_buffer,
            max_total_bytes,
        );
    }

    states
}

pub(crate) fn session_recovery_history_states(
    buffers: &[TextBuffer],
    active: Option<BufferId>,
    recovery_buffer_max_bytes: usize,
    recovery_session_max_bytes: usize,
) -> Vec<RecoveredBufferHistoryState> {
    session_recovery_history_states_with_limits(
        buffers,
        active,
        recovery_buffer_max_bytes,
        recovery_session_max_bytes,
        SESSION_HISTORY_MAX_ENTRIES_PER_STACK,
        SESSION_HISTORY_MAX_BYTES_PER_BUFFER,
        SESSION_HISTORY_MAX_TOTAL_BYTES,
    )
}

fn session_recovery_history_states_with_limits(
    buffers: &[TextBuffer],
    active: Option<BufferId>,
    recovery_buffer_max_bytes: usize,
    recovery_session_max_bytes: usize,
    max_entries_per_stack: usize,
    max_bytes_per_buffer: usize,
    max_total_bytes: usize,
) -> Vec<RecoveredBufferHistoryState> {
    let recoverable = recoverable_session_buffers(
        buffers,
        recovery_buffer_max_bytes,
        recovery_session_max_bytes,
    );
    let mut states = Vec::new();
    let mut total_history_bytes = 0usize;

    if let Some((recovery_index, active_buffer)) = active.and_then(|active| {
        recoverable
            .iter()
            .find(|(_, buffer)| buffer.id() == active)
            .copied()
    }) {
        push_recovery_session_history_state(
            &mut states,
            &mut total_history_bytes,
            recovery_index,
            active_buffer,
            max_entries_per_stack,
            max_bytes_per_buffer,
            max_total_bytes,
        );
    }

    for (recovery_index, buffer) in recoverable {
        if Some(buffer.id()) == active {
            continue;
        }
        push_recovery_session_history_state(
            &mut states,
            &mut total_history_bytes,
            recovery_index,
            buffer,
            max_entries_per_stack,
            max_bytes_per_buffer,
            max_total_bytes,
        );
    }

    states
}

fn push_session_history_state(
    states: &mut Vec<BufferHistoryState>,
    total_history_bytes: &mut usize,
    buffer: &TextBuffer,
    max_entries_per_stack: usize,
    max_bytes_per_buffer: usize,
    max_total_bytes: usize,
) {
    let Some(path) = buffer.path().cloned() else {
        return;
    };
    let Some(history) = buffer.history_snapshot(max_entries_per_stack, max_bytes_per_buffer) else {
        return;
    };
    if history.is_empty() {
        return;
    }

    let estimated_bytes = history.estimated_bytes();
    let next_total = total_history_bytes.saturating_add(estimated_bytes);
    if next_total > max_total_bytes {
        return;
    }

    *total_history_bytes = next_total;
    states.push(BufferHistoryState { path, history });
}

fn push_recovery_session_history_state(
    states: &mut Vec<RecoveredBufferHistoryState>,
    total_history_bytes: &mut usize,
    recovery_index: usize,
    buffer: &TextBuffer,
    max_entries_per_stack: usize,
    max_bytes_per_buffer: usize,
    max_total_bytes: usize,
) {
    if buffer.path().is_some() {
        return;
    }
    let Some(history) = buffer.history_snapshot(max_entries_per_stack, max_bytes_per_buffer) else {
        return;
    };
    if history.is_empty() {
        return;
    }

    let estimated_bytes = history.estimated_bytes();
    let next_total = total_history_bytes.saturating_add(estimated_bytes);
    if next_total > max_total_bytes {
        return;
    }

    *total_history_bytes = next_total;
    states.push(RecoveredBufferHistoryState {
        recovery_index,
        history,
    });
}

pub(crate) fn apply_buffer_history_state(
    buffer: &mut TextBuffer,
    state: BufferHistoryState,
) -> bool {
    if buffer.path() != Some(&state.path) {
        return false;
    }

    buffer.restore_history_snapshot(state.history)
}

pub(crate) fn apply_recovered_buffer_history_state(
    buffer: &mut TextBuffer,
    state: RecoveredBufferHistoryState,
) -> bool {
    buffer.restore_history_snapshot(state.history)
}

fn recoverable_session_buffers(
    buffers: &[TextBuffer],
    per_buffer_limit: usize,
    session_limit: usize,
) -> Vec<(usize, &TextBuffer)> {
    let mut recovered = Vec::new();
    let mut used_bytes = 0usize;
    let path_winners = recovery_path_winners(buffers, per_buffer_limit, session_limit);

    for (buffer_index, buffer) in buffers
        .iter()
        .enumerate()
        .filter(|(_, buffer)| buffer.is_dirty())
    {
        let bytes = buffer.len_bytes();
        if bytes > per_buffer_limit {
            continue;
        }
        if used_bytes.saturating_add(bytes) > session_limit {
            continue;
        }
        if let Some(path) = buffer.path()
            && path_winners
                .get(&recovery_path_key(path))
                .is_some_and(|winner_index| *winner_index != buffer_index)
        {
            continue;
        }

        used_bytes = used_bytes.saturating_add(bytes);
        recovered.push((recovered.len(), buffer));
    }

    recovered
}

pub(crate) fn recent_projects_with_recorded(current: &[PathBuf], root: PathBuf) -> Vec<PathBuf> {
    let mut projects = Vec::with_capacity(current.len() + 1);
    projects.push(root);
    projects.extend(current.iter().cloned());
    normalize_recent_projects(projects)
}

pub(crate) fn merged_recent_projects(current: &[PathBuf], restored: &[PathBuf]) -> Vec<PathBuf> {
    let mut projects = Vec::with_capacity(current.len() + restored.len());
    projects.extend(current.iter().cloned());
    projects.extend(restored.iter().cloned());
    normalize_recent_projects(projects)
}

#[cfg(test)]
mod tests {
    use super::{editor_row_height, session_history_states_with_limits};
    use crate::persistence::BufferViewState;
    use kuroya_core::{BufferId, TextBuffer};
    use std::path::PathBuf;

    #[test]
    fn editor_row_height_uses_auto_or_configured_value() {
        assert_eq!(editor_row_height(13.0, 0.0), 20.0);
        assert_eq!(editor_row_height(13.0, 24.0), 24.0);
        assert_eq!(editor_row_height(13.0, f32::NAN), 20.0);
    }

    #[test]
    fn session_history_states_prioritize_active_buffer_within_total_budget() {
        let first_path = PathBuf::from("workspace/src/first.rs");
        let active_path = PathBuf::from("workspace/src/active.rs");
        let first = buffer_with_history(1, first_path, 512);
        let active = buffer_with_history(2, active_path.clone(), 512);

        let states =
            session_history_states_with_limits(&[first, active], Some(2), 16, 4 * 1024, 900);

        assert_eq!(states.len(), 1);
        assert_eq!(states[0].path, active_path);
    }

    #[test]
    fn session_history_states_bound_total_history_budget() {
        let first_path = PathBuf::from("workspace/src/first.rs");
        let second_path = PathBuf::from("workspace/src/second.rs");
        let first = buffer_with_history(1, first_path.clone(), 512);
        let second = buffer_with_history(2, second_path, 512);

        let states = session_history_states_with_limits(&[first, second], None, 16, 4 * 1024, 900);

        assert_eq!(states.len(), 1);
        assert_eq!(states[0].path, first_path);
    }

    #[test]
    fn apply_buffer_view_state_rejects_stale_path_state() {
        let path = PathBuf::from("workspace/src/current.rs");
        let mut buffer = TextBuffer::from_text(7, Some(path), "one\ntwo\nthree\nfour".to_owned());
        buffer.set_single_cursor(buffer.line_column_to_char(1, 1));
        let state = BufferViewState {
            path: PathBuf::from("workspace/src/stale.rs"),
            cursor_line: 4,
            cursor_column: 1,
            scroll_line: 4,
            horizontal_scroll_offset: 0.0,
            selections: Vec::new(),
        };

        let scroll_line = super::apply_buffer_view_state(&mut buffer, &state);

        assert_eq!(buffer.cursor_position().line, 1);
        assert_eq!(buffer.cursor_position().column, 1);
        assert_eq!(scroll_line, 1);
    }

    fn buffer_with_history(id: BufferId, path: PathBuf, inserted_len: usize) -> TextBuffer {
        let mut buffer = TextBuffer::from_text(id, Some(path), "base".to_owned());
        buffer.set_single_cursor(buffer.len_chars());
        buffer.insert_at_cursor(&"x".repeat(inserted_len));
        buffer
    }
}
