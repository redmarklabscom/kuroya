pub(super) fn begin_workspace_task_load_request_state(
    next_request_id: &mut u64,
    active_request_id: &mut u64,
    in_flight_request_id: &mut Option<u64>,
    reload_queued: &mut bool,
) -> Option<u64> {
    let request_id = reserve_workspace_task_load_request_id_state(
        next_request_id,
        active_request_id,
        *in_flight_request_id,
    );
    if in_flight_request_id.is_some() {
        *reload_queued = true;
        None
    } else {
        *in_flight_request_id = Some(request_id);
        Some(request_id)
    }
}

pub(super) fn finish_workspace_task_load_request_state(
    in_flight_request_id: &mut Option<u64>,
    reload_queued: &mut bool,
    request_id: u64,
) -> bool {
    if *in_flight_request_id != Some(request_id) {
        return false;
    }
    *in_flight_request_id = None;
    let should_spawn_queued_reload = *reload_queued;
    *reload_queued = false;
    should_spawn_queued_reload
}

pub(super) fn invalidate_workspace_task_load_request_state(
    next_request_id: &mut u64,
    active_request_id: &mut u64,
    in_flight_request_id: &mut Option<u64>,
    reload_queued: &mut bool,
) {
    let _ = reserve_workspace_task_load_request_id_state(
        next_request_id,
        active_request_id,
        *in_flight_request_id,
    );
    *in_flight_request_id = None;
    *reload_queued = false;
}

fn reserve_workspace_task_load_request_id_state(
    next_request_id: &mut u64,
    active_request_id: &mut u64,
    reserved_request_id: Option<u64>,
) -> u64 {
    let mut candidate = next_workspace_task_load_request_id(*next_request_id);
    if Some(candidate) == reserved_request_id {
        candidate = next_workspace_task_load_request_id(candidate);
    }
    *next_request_id = candidate;
    *active_request_id = *next_request_id;
    *active_request_id
}

fn next_workspace_task_load_request_id(current: u64) -> u64 {
    current.checked_add(1).unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::{
        begin_workspace_task_load_request_state, finish_workspace_task_load_request_state,
        invalidate_workspace_task_load_request_state,
    };

    #[test]
    fn workspace_task_load_request_starts_when_idle() {
        let mut next_request_id = 0;
        let mut active_request_id = 0;
        let mut in_flight = None;
        let mut queued = false;

        assert_eq!(
            begin_workspace_task_load_request_state(
                &mut next_request_id,
                &mut active_request_id,
                &mut in_flight,
                &mut queued,
            ),
            Some(1)
        );

        assert_eq!(next_request_id, 1);
        assert_eq!(active_request_id, 1);
        assert_eq!(in_flight, Some(1));
        assert!(!queued);
    }

    #[test]
    fn workspace_task_load_request_queues_once_while_in_flight() {
        let mut next_request_id = 0;
        let mut active_request_id = 0;
        let mut in_flight = None;
        let mut queued = false;

        assert_eq!(
            begin_workspace_task_load_request_state(
                &mut next_request_id,
                &mut active_request_id,
                &mut in_flight,
                &mut queued,
            ),
            Some(1)
        );
        assert_eq!(
            begin_workspace_task_load_request_state(
                &mut next_request_id,
                &mut active_request_id,
                &mut in_flight,
                &mut queued,
            ),
            None
        );
        assert_eq!(
            begin_workspace_task_load_request_state(
                &mut next_request_id,
                &mut active_request_id,
                &mut in_flight,
                &mut queued,
            ),
            None
        );

        assert_eq!(next_request_id, 3);
        assert_eq!(active_request_id, 3);
        assert_eq!(in_flight, Some(1));
        assert!(queued);
    }

    #[test]
    fn workspace_task_load_finish_drains_queued_reload_once() {
        let mut next_request_id = 0;
        let mut active_request_id = 0;
        let mut in_flight = None;
        let mut queued = false;

        assert_eq!(
            begin_workspace_task_load_request_state(
                &mut next_request_id,
                &mut active_request_id,
                &mut in_flight,
                &mut queued,
            ),
            Some(1)
        );
        assert_eq!(
            begin_workspace_task_load_request_state(
                &mut next_request_id,
                &mut active_request_id,
                &mut in_flight,
                &mut queued,
            ),
            None
        );

        assert!(finish_workspace_task_load_request_state(
            &mut in_flight,
            &mut queued,
            1
        ));
        assert_eq!(in_flight, None);
        assert!(!queued);
        assert_eq!(
            begin_workspace_task_load_request_state(
                &mut next_request_id,
                &mut active_request_id,
                &mut in_flight,
                &mut queued,
            ),
            Some(3)
        );
    }

    #[test]
    fn workspace_task_load_finish_ignores_unrelated_request_id() {
        let mut in_flight = Some(4);
        let mut queued = true;

        assert!(!finish_workspace_task_load_request_state(
            &mut in_flight,
            &mut queued,
            3
        ));

        assert_eq!(in_flight, Some(4));
        assert!(queued);
    }

    #[test]
    fn workspace_task_load_invalidation_keeps_request_ids_monotonic() {
        let mut next_request_id = 4;
        let mut active_request_id = 4;
        let mut in_flight = Some(4);
        let mut queued = true;

        invalidate_workspace_task_load_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        );

        assert_eq!(next_request_id, 5);
        assert_eq!(active_request_id, 5);
        assert_eq!(in_flight, None);
        assert!(!queued);
        assert_eq!(
            begin_workspace_task_load_request_state(
                &mut next_request_id,
                &mut active_request_id,
                &mut in_flight,
                &mut queued,
            ),
            Some(6)
        );
    }

    #[test]
    fn workspace_task_load_request_ids_wrap_to_nonzero_after_max() {
        let mut next_request_id = u64::MAX;
        let mut active_request_id = u64::MAX;
        let mut in_flight = None;
        let mut queued = false;

        assert_eq!(
            begin_workspace_task_load_request_state(
                &mut next_request_id,
                &mut active_request_id,
                &mut in_flight,
                &mut queued,
            ),
            Some(1)
        );

        assert_eq!(next_request_id, 1);
        assert_eq!(active_request_id, 1);
        assert_eq!(in_flight, Some(1));
        assert!(!queued);
    }

    #[test]
    fn queued_workspace_task_load_request_ids_skip_current_in_flight_after_wrap() {
        let mut next_request_id = u64::MAX;
        let mut active_request_id = 1;
        let mut in_flight = Some(1);
        let mut queued = false;

        assert_eq!(
            begin_workspace_task_load_request_state(
                &mut next_request_id,
                &mut active_request_id,
                &mut in_flight,
                &mut queued,
            ),
            None
        );

        assert_eq!(next_request_id, 2);
        assert_eq!(active_request_id, 2);
        assert_eq!(in_flight, Some(1));
        assert!(queued);
    }

    #[test]
    fn workspace_task_load_invalidation_skips_current_in_flight_after_wrap() {
        let mut next_request_id = u64::MAX;
        let mut active_request_id = 1;
        let mut in_flight = Some(1);
        let mut queued = true;

        invalidate_workspace_task_load_request_state(
            &mut next_request_id,
            &mut active_request_id,
            &mut in_flight,
            &mut queued,
        );

        assert_eq!(next_request_id, 2);
        assert_eq!(active_request_id, 2);
        assert_eq!(in_flight, None);
        assert!(!queued);
    }
}
