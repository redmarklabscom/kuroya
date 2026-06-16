use crate::{
    KuroyaApp, keybinding_input::CapturedKeybinding,
    keybindings_runtime::malformed_keybinding_chord_rejection_reason,
};
use kuroya_core::Command;

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
                    self.keybinding_capture_command = None;
                    self.status = "Canceled shortcut capture".to_owned();
                }
                CapturedKeybinding::Rejected(reason) => {
                    self.status = reason;
                }
                CapturedKeybinding::Chord(chord) => {
                    if let Some(reason) = malformed_keybinding_chord_rejection_reason(&chord) {
                        self.status = reason.to_owned();
                    } else if let Some(command) = self.keybinding_capture_command.take() {
                        self.save_keybinding_chord(command, chord);
                    }
                }
            }
        } else if let Some(command) = actions.start_capture {
            self.keybinding_capture_command = Some(command);
            self.status = "Press the new shortcut".to_owned();
        } else if let Some(command) = actions.remove_binding {
            self.remove_keybinding_for_command(command);
        } else if actions.close {
            self.keybindings_open = false;
            self.keybinding_capture_command = None;
            self.status = "Closed keyboard shortcuts".to_owned();
        } else if actions.open_settings {
            self.keybindings_open = false;
            self.keybinding_capture_command = None;
            self.open_settings_file();
        }
    }
}
