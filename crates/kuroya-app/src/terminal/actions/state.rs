use super::super::{TerminalProcessSessionState, TerminalSession};

#[derive(Default)]
pub(super) struct TerminalScopedStatePresence {
    pub(super) pending_kill_session_started: bool,
    pub(super) pending_paste_session_exists: bool,
    pub(super) pending_multiline_paste_session_exists: bool,
    pub(super) selected_session_exists: bool,
    pub(super) selected_text_session_exists: bool,
    pub(super) selection_drag_session_exists: bool,
}

impl TerminalScopedStatePresence {
    fn all_valid(&self) -> bool {
        self.pending_kill_session_started
            && self.pending_paste_session_exists
            && self.pending_multiline_paste_session_exists
            && self.selected_session_exists
            && self.selected_text_session_exists
            && self.selection_drag_session_exists
    }
}

pub(super) fn terminal_scoped_state_presence(
    sessions: &[TerminalSession],
    pending_kill_session_id: Option<usize>,
    pending_paste_session_id: Option<usize>,
    pending_multiline_paste_session_id: Option<usize>,
    selected_session_id: Option<usize>,
    selected_text_session_id: Option<usize>,
    selection_drag_session_id: Option<usize>,
) -> TerminalScopedStatePresence {
    let mut presence = TerminalScopedStatePresence {
        pending_kill_session_started: pending_kill_session_id.is_none(),
        pending_paste_session_exists: pending_paste_session_id.is_none(),
        pending_multiline_paste_session_exists: pending_multiline_paste_session_id.is_none(),
        selected_session_exists: selected_session_id.is_none(),
        selected_text_session_exists: selected_text_session_id.is_none(),
        selection_drag_session_exists: selection_drag_session_id.is_none(),
    };

    for session in sessions {
        let session_id = session.id;
        if pending_kill_session_id == Some(session_id) && session.started {
            presence.pending_kill_session_started = true;
        }
        if pending_paste_session_id == Some(session_id) {
            presence.pending_paste_session_exists = true;
        }
        if pending_multiline_paste_session_id == Some(session_id) {
            presence.pending_multiline_paste_session_exists = true;
        }
        if selected_session_id == Some(session_id) {
            presence.selected_session_exists = true;
        }
        if selected_text_session_id == Some(session_id) {
            presence.selected_text_session_exists = true;
        }
        if selection_drag_session_id == Some(session_id) {
            presence.selection_drag_session_exists = true;
        }
        if presence.all_valid() {
            break;
        }
    }

    presence
}

pub(super) fn terminal_process_session_state(
    session: &TerminalSession,
) -> TerminalProcessSessionState {
    if session.started {
        TerminalProcessSessionState::Running
    } else if session.last_process_terminal_error {
        TerminalProcessSessionState::TerminalError
    } else if let Some(exit_code) = session.last_process_exit_code {
        TerminalProcessSessionState::Exited(exit_code)
    } else {
        TerminalProcessSessionState::Stopped
    }
}

pub(super) fn enforce_min_split_widths(widths: &mut [f32], available_width: f32) {
    if widths.is_empty() {
        return;
    }

    let available_width = bounded_split_dimension(available_width);
    for width in widths.iter_mut() {
        *width = bounded_split_dimension(*width);
    }
    let min_width = split_min_width().min(available_width / widths.len() as f32);
    let mut fixed = vec![false; widths.len()];
    loop {
        let mut changed = false;
        for (index, width) in widths.iter_mut().enumerate() {
            if !fixed[index] && *width < min_width {
                *width = min_width;
                fixed[index] = true;
                changed = true;
            }
        }
        if !changed {
            break;
        }

        let fixed_total = widths
            .iter()
            .zip(&fixed)
            .filter_map(|(width, fixed)| fixed.then_some(*width))
            .sum::<f32>();
        let remaining_width = (available_width - fixed_total).max(0.0);
        let flexible_total = widths
            .iter()
            .zip(&fixed)
            .filter_map(|(width, fixed)| (!fixed).then_some(*width))
            .sum::<f32>();
        let width_count = widths.len() as f32;
        for (width, fixed) in widths.iter_mut().zip(&fixed) {
            if !fixed {
                *width = if flexible_total > 0.0 {
                    (*width / flexible_total) * remaining_width
                } else {
                    remaining_width / width_count
                };
            }
        }
    }
}

pub(super) fn bounded_split_dimension(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

pub(super) fn split_min_width() -> f32 {
    160.0
}

pub(super) fn non_empty_text(text: String) -> Option<String> {
    (!text.is_empty()).then_some(text)
}
