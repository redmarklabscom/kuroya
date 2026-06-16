use crate::{
    KuroyaApp, diagnostics_panel::diagnostic_display_path,
    lsp_lifecycle::background_language_block_reason, ui_events::UiEvent,
};
use kuroya_core::diagnostics::static_diagnostics_scan_allowed;
use kuroya_core::{BufferId, TextBuffer, analyze_text};
use std::{collections::HashMap, path::PathBuf};

impl KuroyaApp {
    pub(crate) fn spawn_diagnostics_for(&mut self, id: BufferId) {
        let Some((path, version, background_blocked)) = self.buffer(id).map(|buffer| {
            let path = self.diagnostic_path_for(buffer);
            let background_blocked = background_language_block_reason(
                id,
                buffer,
                &self.lossy_decoded_buffers,
                &self.binary_preview_buffers,
            )
            .is_some();
            (path, buffer.version(), background_blocked)
        }) else {
            self.clear_static_diagnostics_request(id);
            return;
        };

        if background_blocked {
            self.invalidate_static_diagnostics_request(id);
            self.diagnostics.replace_static(path, Vec::new());
            return;
        }

        let Some(text_len_bytes) = self.buffer(id).map(TextBuffer::len_bytes) else {
            self.clear_static_diagnostics_request(id);
            return;
        };
        if !static_diagnostics_should_scan_text(text_len_bytes) {
            self.invalidate_static_diagnostics_request(id);
            self.diagnostics.replace_static(path, Vec::new());
            return;
        }

        let Some(request_id) = self.begin_static_diagnostics_request(id) else {
            return;
        };
        let Some(text) = self.buffer(id).map(|buffer| buffer.text_snapshot()) else {
            self.clear_static_diagnostics_request(id);
            return;
        };
        let tx = self.tx.clone();
        self.record_async_task_started("Static Diagnostics", diagnostic_display_path(&path));
        self.runtime.spawn_blocking(move || {
            let text = text.text();
            let diagnostics = analyze_text(path.clone(), &text);
            let _ = crate::ui_event_channel::send_critical_ui_event(
                &tx,
                UiEvent::DiagnosticsComputed {
                    request_id,
                    id,
                    path,
                    version,
                    diagnostics,
                },
            );
        });
    }

    pub(crate) fn diagnostic_path_for(&self, buffer: &TextBuffer) -> PathBuf {
        buffer
            .path()
            .cloned()
            .unwrap_or_else(|| PathBuf::from(format!("<untitled-{}>", buffer.id())))
    }

    fn begin_static_diagnostics_request(&mut self, id: BufferId) -> Option<u64> {
        begin_static_diagnostics_request_state(
            &mut self.static_diagnostics_next_request_id,
            &mut self.static_diagnostics_active_request_ids,
            &mut self.static_diagnostics_in_flight_request_ids,
            &mut self.static_diagnostics_reload_queued,
            id,
        )
    }

    pub(crate) fn finish_static_diagnostics_request(
        &mut self,
        id: BufferId,
        request_id: u64,
    ) -> bool {
        finish_static_diagnostics_request_state(
            &mut self.static_diagnostics_active_request_ids,
            &mut self.static_diagnostics_in_flight_request_ids,
            &mut self.static_diagnostics_reload_queued,
            id,
            request_id,
        )
    }

    pub(crate) fn invalidate_static_diagnostics_request(&mut self, id: BufferId) {
        invalidate_static_diagnostics_request_state(
            &mut self.static_diagnostics_next_request_id,
            &mut self.static_diagnostics_active_request_ids,
            &mut self.static_diagnostics_in_flight_request_ids,
            &mut self.static_diagnostics_reload_queued,
            id,
        );
    }

    pub(crate) fn clear_static_diagnostics_request(&mut self, id: BufferId) {
        clear_static_diagnostics_request_state(
            &mut self.static_diagnostics_active_request_ids,
            &mut self.static_diagnostics_in_flight_request_ids,
            &mut self.static_diagnostics_reload_queued,
            id,
        );
    }
}

fn static_diagnostics_should_scan_text(byte_len: usize) -> bool {
    static_diagnostics_scan_allowed(byte_len)
}

fn reserve_static_diagnostics_request_id_state(
    next_request_id: &mut u64,
    active_request_ids: &mut HashMap<BufferId, u64>,
    in_flight_request_ids: &HashMap<BufferId, u64>,
    id: BufferId,
) -> u64 {
    let previous_active_request_id = active_request_ids.get(&id).copied();
    let previous_in_flight_request_id = in_flight_request_ids.get(&id).copied();
    loop {
        *next_request_id = next_static_diagnostics_request_id(*next_request_id);
        let request_id = *next_request_id;
        if Some(request_id) != previous_active_request_id
            && Some(request_id) != previous_in_flight_request_id
        {
            active_request_ids.insert(id, request_id);
            return request_id;
        }
    }
}

fn next_static_diagnostics_request_id(current: u64) -> u64 {
    match current.wrapping_add(1) {
        0 => 1,
        request_id => request_id,
    }
}

fn begin_static_diagnostics_request_state(
    next_request_id: &mut u64,
    active_request_ids: &mut HashMap<BufferId, u64>,
    in_flight_request_ids: &mut HashMap<BufferId, u64>,
    reload_queued: &mut std::collections::HashSet<BufferId>,
    id: BufferId,
) -> Option<u64> {
    let request_id = reserve_static_diagnostics_request_id_state(
        next_request_id,
        active_request_ids,
        in_flight_request_ids,
        id,
    );
    if let std::collections::hash_map::Entry::Vacant(entry) = in_flight_request_ids.entry(id) {
        entry.insert(request_id);
        Some(request_id)
    } else {
        reload_queued.insert(id);
        None
    }
}

fn finish_static_diagnostics_request_state(
    active_request_ids: &mut HashMap<BufferId, u64>,
    in_flight_request_ids: &mut HashMap<BufferId, u64>,
    reload_queued: &mut std::collections::HashSet<BufferId>,
    id: BufferId,
    request_id: u64,
) -> bool {
    if in_flight_request_ids.get(&id) != Some(&request_id) {
        return false;
    }
    in_flight_request_ids.remove(&id);
    let should_spawn_queued_reload = reload_queued.remove(&id);
    if !should_spawn_queued_reload {
        active_request_ids.remove(&id);
    }
    should_spawn_queued_reload
}

fn invalidate_static_diagnostics_request_state(
    next_request_id: &mut u64,
    active_request_ids: &mut HashMap<BufferId, u64>,
    in_flight_request_ids: &mut HashMap<BufferId, u64>,
    reload_queued: &mut std::collections::HashSet<BufferId>,
    id: BufferId,
) {
    let _ = reserve_static_diagnostics_request_id_state(
        next_request_id,
        active_request_ids,
        in_flight_request_ids,
        id,
    );
    in_flight_request_ids.remove(&id);
    reload_queued.remove(&id);
}

fn clear_static_diagnostics_request_state(
    active_request_ids: &mut HashMap<BufferId, u64>,
    in_flight_request_ids: &mut HashMap<BufferId, u64>,
    reload_queued: &mut std::collections::HashSet<BufferId>,
    id: BufferId,
) {
    active_request_ids.remove(&id);
    in_flight_request_ids.remove(&id);
    reload_queued.remove(&id);
}

#[cfg(test)]
mod tests {
    use super::{
        begin_static_diagnostics_request_state, clear_static_diagnostics_request_state,
        finish_static_diagnostics_request_state, invalidate_static_diagnostics_request_state,
        static_diagnostics_should_scan_text,
    };
    use std::collections::{HashMap, HashSet};

    #[test]
    fn static_diagnostics_scan_guard_allows_limit_and_rejects_oversize() {
        assert!(static_diagnostics_should_scan_text(
            kuroya_core::diagnostics::STATIC_DIAGNOSTIC_SCAN_MAX_BYTES
        ));
        assert!(!static_diagnostics_should_scan_text(
            kuroya_core::diagnostics::STATIC_DIAGNOSTIC_SCAN_MAX_BYTES + 1
        ));
    }

    #[test]
    fn static_diagnostics_request_starts_when_idle() {
        let mut next_request_id = 0;
        let mut active = HashMap::new();
        let mut in_flight = HashMap::new();
        let mut queued = HashSet::new();

        assert_eq!(
            begin_static_diagnostics_request_state(
                &mut next_request_id,
                &mut active,
                &mut in_flight,
                &mut queued,
                7,
            ),
            Some(1)
        );

        assert_eq!(next_request_id, 1);
        assert_eq!(active.get(&7), Some(&1));
        assert_eq!(in_flight.get(&7), Some(&1));
        assert!(!queued.contains(&7));
    }

    #[test]
    fn static_diagnostics_request_queues_latest_while_in_flight() {
        let mut next_request_id = 0;
        let mut active = HashMap::new();
        let mut in_flight = HashMap::new();
        let mut queued = HashSet::new();

        assert_eq!(
            begin_static_diagnostics_request_state(
                &mut next_request_id,
                &mut active,
                &mut in_flight,
                &mut queued,
                7,
            ),
            Some(1)
        );
        assert_eq!(
            begin_static_diagnostics_request_state(
                &mut next_request_id,
                &mut active,
                &mut in_flight,
                &mut queued,
                7,
            ),
            None
        );
        assert_eq!(
            begin_static_diagnostics_request_state(
                &mut next_request_id,
                &mut active,
                &mut in_flight,
                &mut queued,
                7,
            ),
            None
        );

        assert_eq!(next_request_id, 3);
        assert_eq!(active.get(&7), Some(&3));
        assert_eq!(in_flight.get(&7), Some(&1));
        assert!(queued.contains(&7));
    }

    #[test]
    fn static_diagnostics_finish_drains_queued_reload_once() {
        let mut next_request_id = 0;
        let mut active = HashMap::new();
        let mut in_flight = HashMap::new();
        let mut queued = HashSet::new();

        assert_eq!(
            begin_static_diagnostics_request_state(
                &mut next_request_id,
                &mut active,
                &mut in_flight,
                &mut queued,
                7,
            ),
            Some(1)
        );
        assert_eq!(
            begin_static_diagnostics_request_state(
                &mut next_request_id,
                &mut active,
                &mut in_flight,
                &mut queued,
                7,
            ),
            None
        );

        assert!(finish_static_diagnostics_request_state(
            &mut active,
            &mut in_flight,
            &mut queued,
            7,
            1,
        ));
        assert!(in_flight.is_empty());
        assert!(!queued.contains(&7));
        assert_eq!(active.get(&7), Some(&2));
    }

    #[test]
    fn static_diagnostics_finish_ignores_unrelated_request_id() {
        let mut active = HashMap::from([(7, 4)]);
        let mut in_flight = HashMap::from([(7, 4)]);
        let mut queued = HashSet::from([7]);

        assert!(!finish_static_diagnostics_request_state(
            &mut active,
            &mut in_flight,
            &mut queued,
            7,
            3,
        ));

        assert_eq!(active.get(&7), Some(&4));
        assert_eq!(in_flight.get(&7), Some(&4));
        assert!(queued.contains(&7));
    }

    #[test]
    fn static_diagnostics_invalidation_keeps_request_ids_monotonic() {
        let mut next_request_id = 4;
        let mut active = HashMap::from([(7, 4)]);
        let mut in_flight = HashMap::from([(7, 4)]);
        let mut queued = HashSet::from([7]);

        invalidate_static_diagnostics_request_state(
            &mut next_request_id,
            &mut active,
            &mut in_flight,
            &mut queued,
            7,
        );

        assert_eq!(next_request_id, 5);
        assert_eq!(active.get(&7), Some(&5));
        assert!(in_flight.is_empty());
        assert!(!queued.contains(&7));
        assert_eq!(
            begin_static_diagnostics_request_state(
                &mut next_request_id,
                &mut active,
                &mut in_flight,
                &mut queued,
                7,
            ),
            Some(6)
        );
    }

    #[test]
    fn static_diagnostics_request_ids_wrap_without_zero() {
        let mut next_request_id = u64::MAX - 1;
        let mut active = HashMap::new();
        let mut in_flight = HashMap::new();
        let mut queued = HashSet::new();

        assert_eq!(
            begin_static_diagnostics_request_state(
                &mut next_request_id,
                &mut active,
                &mut in_flight,
                &mut queued,
                7,
            ),
            Some(u64::MAX)
        );
        assert!(!finish_static_diagnostics_request_state(
            &mut active,
            &mut in_flight,
            &mut queued,
            7,
            u64::MAX,
        ));

        assert_eq!(
            begin_static_diagnostics_request_state(
                &mut next_request_id,
                &mut active,
                &mut in_flight,
                &mut queued,
                8,
            ),
            Some(1)
        );
        assert_eq!(next_request_id, 1);
        assert!(!active.values().any(|request_id| *request_id == 0));
        assert!(!in_flight.values().any(|request_id| *request_id == 0));
    }

    #[test]
    fn static_diagnostics_queued_reload_wrap_skips_current_in_flight_request_id() {
        let mut next_request_id = u64::MAX;
        let mut active = HashMap::from([(7, 1)]);
        let mut in_flight = HashMap::from([(7, 1)]);
        let mut queued = HashSet::new();

        assert_eq!(
            begin_static_diagnostics_request_state(
                &mut next_request_id,
                &mut active,
                &mut in_flight,
                &mut queued,
                7,
            ),
            None
        );

        assert_eq!(next_request_id, 2);
        assert_eq!(active.get(&7), Some(&2));
        assert_eq!(in_flight.get(&7), Some(&1));
        assert!(queued.contains(&7));
    }

    #[test]
    fn static_diagnostics_clear_removes_buffer_state() {
        let mut active = HashMap::from([(7, 4), (8, 2)]);
        let mut in_flight = HashMap::from([(7, 4), (8, 2)]);
        let mut queued = HashSet::from([7, 8]);

        clear_static_diagnostics_request_state(&mut active, &mut in_flight, &mut queued, 7);

        assert!(!active.contains_key(&7));
        assert_eq!(active.get(&8), Some(&2));
        assert!(!in_flight.contains_key(&7));
        assert_eq!(in_flight.get(&8), Some(&2));
        assert!(!queued.contains(&7));
        assert!(queued.contains(&8));
    }
}
