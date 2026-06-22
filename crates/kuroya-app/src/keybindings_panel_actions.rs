use crate::{
    KuroyaApp,
    app_state::KeybindingEscapeCancel,
    commands::{command_label, keybinding_chord_for_command},
    keybinding_input::CapturedKeybinding,
    keybindings_runtime::{
        keybinding_cancel_save_failed_status, malformed_keybinding_chord_rejection_reason,
        prune_stale_keybinding_assignments,
    },
    workspace_state::settings_path,
};
use kuroya_core::Command;
use std::time::{Duration, Instant};

const KEYBINDING_ESCAPE_CANCEL_WINDOW: Duration = Duration::from_secs(1);
const KEYBINDING_ESCAPE_CANCEL_HINT: &str = "; press Esc again to cancel";

#[derive(Default)]
pub(crate) struct PendingKeybindingsPanelActions {
    pub(crate) captured: Option<CapturedKeybinding>,
    pub(crate) start_capture: Option<Command>,
    pub(crate) remove_binding: Option<Command>,
    pub(crate) close: bool,
    pub(crate) open_settings: bool,
}

impl KuroyaApp {
    pub(crate) fn apply_keybindings_panel_actions(
        &mut self,
        actions: PendingKeybindingsPanelActions,
    ) {
        if let Some(captured) = actions.captured {
            match captured {
                CapturedKeybinding::Cancel => {
                    self.cancel_keybinding_capture();
                }
                CapturedKeybinding::Rejected(reason) => {
                    self.status = reason;
                }
                CapturedKeybinding::Escape => self.handle_keybinding_escape_capture(Instant::now()),
                CapturedKeybinding::Chord(chord) => {
                    self.save_captured_keybinding_chord(chord);
                }
            }
        } else if let Some(command) = actions.start_capture {
            if self.has_keybinding_capture_in_progress() && !self.cancel_keybinding_capture() {
                return;
            }
            self.keybinding_capture_command = Some(command);
            self.status = "Capturing shortcut; press keys, or press Esc twice to cancel".to_owned();
        } else if let Some(command) = actions.remove_binding {
            if self.has_keybinding_capture_in_progress() && !self.cancel_keybinding_capture() {
                return;
            }
            self.remove_keybinding_for_command(command);
        } else if actions.close {
            if self.has_keybinding_capture_in_progress() && !self.cancel_keybinding_capture() {
                return;
            }
            self.keybindings_open = false;
            self.status = "Closed keyboard shortcuts".to_owned();
        } else if actions.open_settings {
            if self.has_keybinding_capture_in_progress() && !self.cancel_keybinding_capture() {
                return;
            }
            self.keybindings_open = false;
            self.open_settings_file();
        }
    }

    pub(crate) fn finish_expired_keybinding_escape_capture(&mut self, now: Instant) -> bool {
        let Some(pending) = self.keybinding_escape_cancel.as_ref() else {
            return false;
        };
        if now < pending.deadline {
            return false;
        }
        if pending.saved_escape_binding
            && self.keybinding_capture_command == Some(pending.command.clone())
        {
            self.keybinding_capture_command = None;
        }
        self.keybinding_escape_cancel = None;
        if let Some(status) = self.status.strip_suffix(KEYBINDING_ESCAPE_CANCEL_HINT) {
            self.status = status.to_owned();
        }
        true
    }

    pub(crate) fn keybinding_escape_cancel_remaining(&self, now: Instant) -> Option<Duration> {
        self.keybinding_escape_cancel
            .as_ref()
            .map(|pending| pending.deadline.saturating_duration_since(now))
    }

    pub(crate) fn has_keybinding_capture_in_progress(&self) -> bool {
        self.keybinding_capture_command.is_some() || self.keybinding_escape_cancel.is_some()
    }

    pub(crate) fn cancel_keybinding_capture(&mut self) -> bool {
        if self.keybinding_escape_cancel.is_some() {
            self.restore_keybinding_escape_cancel();
            self.keybinding_escape_cancel.is_none()
        } else {
            self.keybinding_capture_command = None;
            self.status = "Canceled shortcut capture".to_owned();
            true
        }
    }

    fn save_captured_keybinding_chord(&mut self, chord: String) {
        if let Some(reason) = malformed_keybinding_chord_rejection_reason(&chord) {
            self.status = reason.to_owned();
            return;
        }

        let Some(command) = self.keybinding_capture_command.clone() else {
            return;
        };
        if self.keybinding_escape_cancel.is_some() {
            self.restore_keybinding_escape_cancel();
            if self.keybinding_escape_cancel.is_some() {
                return;
            }
        }
        self.keybinding_capture_command = None;
        if !self.save_keybinding_chord(command.clone(), chord) {
            self.keybinding_capture_command = Some(command);
        }
    }

    fn handle_keybinding_escape_capture(&mut self, now: Instant) {
        let Some(command) = self.keybinding_capture_command.clone() else {
            return;
        };
        if self
            .keybinding_escape_cancel
            .as_ref()
            .is_some_and(|pending| pending.command == command && now <= pending.deadline)
        {
            self.restore_keybinding_escape_cancel();
            return;
        }

        let mut bindings_before_escape = self.settings.keymap.bindings.clone();
        prune_stale_keybinding_assignments(&mut bindings_before_escape);
        let saved_escape_binding = self.save_keybinding_chord(command.clone(), "Escape".to_owned())
            && keybinding_chord_for_command(&self.settings.keymap.bindings, &command).as_deref()
                == Some("Escape");
        if saved_escape_binding {
            self.keybinding_capture_command = Some(command.clone());
        } else {
            self.keybinding_capture_command = Some(command);
        }
        self.keybinding_escape_cancel = Some(KeybindingEscapeCancel {
            command: self
                .keybinding_capture_command
                .clone()
                .expect("capture command should remain active after Escape"),
            bindings_before_escape,
            deadline: now + KEYBINDING_ESCAPE_CANCEL_WINDOW,
            saved_escape_binding,
        });
        self.status.push_str(KEYBINDING_ESCAPE_CANCEL_HINT);
    }

    fn restore_keybinding_escape_cancel(&mut self) {
        let Some(pending) = self.keybinding_escape_cancel.take() else {
            return;
        };
        if !pending.saved_escape_binding {
            self.keybinding_capture_command = None;
            self.status = "Canceled shortcut capture".to_owned();
            return;
        }
        let mut settings = self.settings.clone();
        settings.keymap.bindings = pending.bindings_before_escape.clone();
        prune_stale_keybinding_assignments(&mut settings.keymap.bindings);
        let label = command_label(&pending.command);
        match settings.save(&settings_path(&self.workspace.root)) {
            Ok(()) => {
                self.settings = settings;
                self.keybinding_capture_command = None;
                self.status = "Canceled shortcut capture".to_owned();
            }
            Err(error) => {
                self.keybinding_capture_command = Some(pending.command.clone());
                self.keybinding_escape_cancel = Some(pending);
                self.status = keybinding_cancel_save_failed_status(&label, error);
            }
        }
    }
}
