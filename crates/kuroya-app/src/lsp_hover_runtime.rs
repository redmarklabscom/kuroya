use crate::{
    KuroyaApp,
    transient_state::{LspHoverPopup, LspHoverRequestTarget, PendingLspHover},
    workspace_state::PaneId,
};
use eframe::egui::Context;
use kuroya_core::{BufferId, clamp_hover_delay_ms};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PendingHoverUpdate {
    Idle,
    Waiting {
        remaining: Duration,
        stale_request_buffer_id: Option<BufferId>,
    },
    Request {
        char_idx: usize,
        stale_request_buffer_id: Option<BufferId>,
    },
}

impl KuroyaApp {
    pub(crate) fn update_editor_lsp_hover_target(
        &mut self,
        ctx: &Context,
        pane_id: PaneId,
        buffer_id: BufferId,
        hover_char_idx: Option<usize>,
    ) {
        if !self.settings.hover_enabled {
            clear_pending_hover_for_pane_buffer(&mut self.pending_lsp_hover, pane_id, buffer_id);
            self.lsp_hover_request = None;
            self.lsp_hover = None;
            return;
        }
        let Some(version) = self.buffer(buffer_id).map(|buffer| buffer.version()) else {
            clear_pending_hover_for_pane_buffer(&mut self.pending_lsp_hover, pane_id, buffer_id);
            clear_hover_request_for_buffer(&mut self.lsp_hover_request, buffer_id);
            clear_hover_popup_for_buffer(&mut self.lsp_hover, buffer_id);
            return;
        };
        if hover_char_idx.is_none() {
            clear_pending_hover_for_pane_buffer(&mut self.pending_lsp_hover, pane_id, buffer_id);
            clear_hover_request_for_buffer(&mut self.lsp_hover_request, buffer_id);
            return;
        }
        let delay =
            Duration::from_millis(clamp_hover_delay_ms(self.settings.hover_delay_ms) as u64);
        match update_pending_lsp_hover_target(
            &mut self.pending_lsp_hover,
            pane_id,
            buffer_id,
            version,
            hover_char_idx,
            Instant::now(),
            delay,
        ) {
            PendingHoverUpdate::Idle => {}
            PendingHoverUpdate::Waiting {
                remaining,
                stale_request_buffer_id,
            } => {
                if let Some(stale_buffer_id) = stale_request_buffer_id {
                    clear_hover_request_for_buffer(&mut self.lsp_hover_request, stale_buffer_id);
                }
                ctx.request_repaint_after(remaining);
            }
            PendingHoverUpdate::Request {
                char_idx,
                stale_request_buffer_id,
            } => {
                if let Some(stale_buffer_id) = stale_request_buffer_id {
                    clear_hover_request_for_buffer(&mut self.lsp_hover_request, stale_buffer_id);
                }
                let _requested = self.request_lsp_hover_for_buffer_char(buffer_id, char_idx);
            }
        }
    }

    pub(crate) fn clear_pending_lsp_hover_for_buffer(&mut self, id: BufferId) {
        if self
            .pending_lsp_hover
            .as_ref()
            .is_some_and(|pending| pending.buffer_id == id)
        {
            self.pending_lsp_hover = None;
        }
        if self
            .lsp_hover_request
            .as_ref()
            .is_some_and(|target| target.id == id)
        {
            self.lsp_hover_request = None;
        }
    }
}

pub(crate) fn update_pending_lsp_hover_target(
    pending: &mut Option<PendingLspHover>,
    pane_id: PaneId,
    buffer_id: BufferId,
    version: u64,
    hover_char_idx: Option<usize>,
    now: Instant,
    delay: Duration,
) -> PendingHoverUpdate {
    let Some(char_idx) = hover_char_idx else {
        clear_pending_hover_for_pane_buffer(pending, pane_id, buffer_id);
        return PendingHoverUpdate::Idle;
    };

    if let Some(current) = pending.as_mut()
        && current.pane_id == pane_id
        && current.buffer_id == buffer_id
        && current.char_idx == char_idx
        && current.version == version
    {
        if current.requested {
            return PendingHoverUpdate::Idle;
        }
        let elapsed = now.saturating_duration_since(current.started_at);
        if elapsed >= delay {
            current.requested = true;
            return PendingHoverUpdate::Request {
                char_idx,
                stale_request_buffer_id: None,
            };
        }
        return PendingHoverUpdate::Waiting {
            remaining: delay - elapsed,
            stale_request_buffer_id: None,
        };
    }

    let stale_request_buffer_id = pending.as_ref().and_then(|current| {
        (current.pane_id != pane_id
            || current.buffer_id != buffer_id
            || current.char_idx != char_idx
            || current.version != version)
            .then_some(current.buffer_id)
    });
    *pending = Some(PendingLspHover {
        pane_id,
        buffer_id,
        char_idx,
        version,
        started_at: now,
        requested: delay.is_zero(),
    });
    if delay.is_zero() {
        return PendingHoverUpdate::Request {
            char_idx,
            stale_request_buffer_id,
        };
    }
    PendingHoverUpdate::Waiting {
        remaining: delay,
        stale_request_buffer_id,
    }
}

fn clear_pending_hover_for_pane_buffer(
    pending: &mut Option<PendingLspHover>,
    pane_id: PaneId,
    buffer_id: BufferId,
) {
    if pending
        .as_ref()
        .is_some_and(|hover| hover.pane_id == pane_id && hover.buffer_id == buffer_id)
    {
        *pending = None;
    }
}

fn clear_hover_request_for_buffer(
    request: &mut Option<LspHoverRequestTarget>,
    buffer_id: BufferId,
) -> bool {
    if request
        .as_ref()
        .is_some_and(|target| target.id == buffer_id)
    {
        *request = None;
        true
    } else {
        false
    }
}

fn clear_hover_popup_for_buffer(hover: &mut Option<LspHoverPopup>, buffer_id: BufferId) -> bool {
    if hover.as_ref().is_some_and(|popup| popup.id == buffer_id) {
        *hover = None;
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PendingHoverUpdate, clear_hover_popup_for_buffer, clear_hover_request_for_buffer,
        update_pending_lsp_hover_target,
    };
    use crate::transient_state::{LspHoverPopup, LspHoverRequestTarget};
    use std::{
        path::PathBuf,
        time::{Duration, Instant},
    };

    #[test]
    fn pending_lsp_hover_waits_until_delay_then_requests_once() {
        let now = Instant::now();
        let delay = Duration::from_millis(100);
        let mut pending = None;

        assert_eq!(
            update_pending_lsp_hover_target(&mut pending, 1, 7, 3, Some(42), now, delay),
            PendingHoverUpdate::Waiting {
                remaining: delay,
                stale_request_buffer_id: None
            }
        );
        assert_eq!(
            update_pending_lsp_hover_target(
                &mut pending,
                1,
                7,
                3,
                Some(42),
                now + Duration::from_millis(40),
                delay,
            ),
            PendingHoverUpdate::Waiting {
                remaining: Duration::from_millis(60),
                stale_request_buffer_id: None
            }
        );
        assert_eq!(
            update_pending_lsp_hover_target(&mut pending, 1, 7, 3, Some(42), now + delay, delay,),
            PendingHoverUpdate::Request {
                char_idx: 42,
                stale_request_buffer_id: None
            }
        );
        assert_eq!(
            update_pending_lsp_hover_target(
                &mut pending,
                1,
                7,
                3,
                Some(42),
                now + delay + Duration::from_millis(20),
                delay,
            ),
            PendingHoverUpdate::Idle
        );
    }

    #[test]
    fn pending_lsp_hover_requests_immediately_for_zero_delay() {
        let now = Instant::now();
        let mut pending = None;

        assert_eq!(
            update_pending_lsp_hover_target(&mut pending, 1, 7, 3, Some(42), now, Duration::ZERO),
            PendingHoverUpdate::Request {
                char_idx: 42,
                stale_request_buffer_id: None
            }
        );
        assert_eq!(
            update_pending_lsp_hover_target(
                &mut pending,
                1,
                7,
                3,
                Some(42),
                now + Duration::from_millis(20),
                Duration::ZERO,
            ),
            PendingHoverUpdate::Idle
        );
    }

    #[test]
    fn pending_lsp_hover_resets_when_target_or_version_changes() {
        let now = Instant::now();
        let delay = Duration::from_millis(100);
        let mut pending = None;

        assert_eq!(
            update_pending_lsp_hover_target(&mut pending, 1, 7, 3, Some(42), now, delay),
            PendingHoverUpdate::Waiting {
                remaining: delay,
                stale_request_buffer_id: None
            }
        );
        assert_eq!(
            update_pending_lsp_hover_target(
                &mut pending,
                1,
                7,
                4,
                Some(42),
                now + Duration::from_millis(120),
                delay,
            ),
            PendingHoverUpdate::Waiting {
                remaining: delay,
                stale_request_buffer_id: Some(7)
            }
        );
        assert_eq!(
            update_pending_lsp_hover_target(
                &mut pending,
                1,
                7,
                4,
                Some(43),
                now + Duration::from_millis(240),
                delay,
            ),
            PendingHoverUpdate::Waiting {
                remaining: delay,
                stale_request_buffer_id: Some(7)
            }
        );
    }

    #[test]
    fn pending_lsp_hover_clears_when_pointer_leaves_target_pane() {
        let now = Instant::now();
        let delay = Duration::from_millis(100);
        let mut pending = None;

        let _ = update_pending_lsp_hover_target(&mut pending, 1, 7, 3, Some(42), now, delay);
        assert!(pending.is_some());
        assert_eq!(
            update_pending_lsp_hover_target(
                &mut pending,
                1,
                7,
                3,
                None,
                now + Duration::from_millis(20),
                delay,
            ),
            PendingHoverUpdate::Idle
        );
        assert!(pending.is_none());
    }

    #[test]
    fn pending_lsp_hover_marks_version_changes_as_stale_target_changes() {
        let now = Instant::now();
        let delay = Duration::from_millis(100);
        let mut pending = None;

        assert_eq!(
            update_pending_lsp_hover_target(&mut pending, 1, 7, 3, Some(42), now, delay),
            PendingHoverUpdate::Waiting {
                remaining: delay,
                stale_request_buffer_id: None
            }
        );

        assert_eq!(
            update_pending_lsp_hover_target(
                &mut pending,
                1,
                7,
                4,
                Some(42),
                now + Duration::from_millis(10),
                Duration::ZERO,
            ),
            PendingHoverUpdate::Request {
                char_idx: 42,
                stale_request_buffer_id: Some(7)
            }
        );
    }

    #[test]
    fn pending_lsp_hover_reports_previous_buffer_when_target_moves_buffers() {
        let now = Instant::now();
        let delay = Duration::from_millis(100);
        let mut pending = None;

        assert_eq!(
            update_pending_lsp_hover_target(&mut pending, 1, 7, 3, Some(42), now, delay),
            PendingHoverUpdate::Waiting {
                remaining: delay,
                stale_request_buffer_id: None
            }
        );

        assert_eq!(
            update_pending_lsp_hover_target(
                &mut pending,
                2,
                8,
                1,
                Some(9),
                now + Duration::from_millis(10),
                delay,
            ),
            PendingHoverUpdate::Waiting {
                remaining: delay,
                stale_request_buffer_id: Some(7)
            }
        );
    }

    #[test]
    fn hover_request_and_popup_clear_by_buffer_id() {
        let mut request = Some(LspHoverRequestTarget::from_request(
            7,
            PathBuf::from("src/main.rs"),
            3,
            0,
            2,
        ));
        let mut hover = Some(LspHoverPopup {
            id: 7,
            path: PathBuf::from("src/main.rs"),
            line: 1,
            column: 3,
            contents: "hover".to_owned(),
            opened_at: Instant::now(),
        });

        assert!(!clear_hover_request_for_buffer(&mut request, 8));
        assert!(request.is_some());
        assert!(!clear_hover_popup_for_buffer(&mut hover, 8));
        assert!(hover.is_some());

        assert!(clear_hover_request_for_buffer(&mut request, 7));
        assert!(request.is_none());
        assert!(clear_hover_popup_for_buffer(&mut hover, 7));
        assert!(hover.is_none());
    }
}
