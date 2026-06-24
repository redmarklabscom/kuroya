use crate::{
    KuroyaApp, path_display::display_error_label_cow, save_lifecycle::finish_session_save,
    workspace_state::workspace_event_matches,
};
use std::path::PathBuf;

pub(super) fn handle_session_saved_event(app: &mut KuroyaApp, root: PathBuf) {
    finish_session_save_and_start_next(app, root);
}

pub(super) fn handle_session_save_failed_event(app: &mut KuroyaApp, root: PathBuf, error: String) {
    if workspace_event_matches(&app.workspace.root, &root) {
        app.status = session_save_failure_status(&error);
    }
    finish_session_save_and_start_next(app, root);
}

fn finish_session_save_and_start_next(app: &mut KuroyaApp, root: PathBuf) {
    let was_current = app.session_save_in_flight.as_deref() == Some(root.as_path());
    if let Some((next_root, next_session)) = finish_session_save(
        &root,
        &mut app.session_save_in_flight,
        &mut app.queued_session_saves,
    ) {
        app.spawn_session_save(next_root, next_session);
    } else if was_current {
        app.session_save_in_flight_snapshot = None;
        app.session_save_in_flight_task = None;
    }
}

fn session_save_failure_status(error: &str) -> String {
    let error = display_error_label_cow(error);
    format!("Could not save session: {}", error.as_ref())
}

#[cfg(test)]
mod tests {
    use super::session_save_failure_status;
    use crate::path_display::DISPLAY_ERROR_LABEL_MAX_CHARS;

    #[test]
    fn session_save_failure_status_sanitizes_and_bounds_error_detail() {
        let status = session_save_failure_status(&format!(
            "first line\nsecond line \u{202e}{}",
            "x".repeat(400)
        ));

        assert!(status.starts_with("Could not save session: first line "));
        assert!(!status.contains('\n'));
        assert!(!status.contains('\u{202e}'));
        assert!(status.contains("..."));
        assert!(
            status.chars().count()
                <= "Could not save session: ".chars().count() + DISPLAY_ERROR_LABEL_MAX_CHARS
        );
    }

    #[test]
    fn session_save_failure_status_falls_back_for_blank_error_detail() {
        assert_eq!(
            session_save_failure_status("\n\u{202e}\u{0007}"),
            "Could not save session: unknown error"
        );
    }
}
