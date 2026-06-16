use crate::{
    KuroyaApp,
    commands::command_label,
    keybinding_chords::keybinding_requires_primary_modifier,
    keybinding_parse::{normalize_key_chord, parse_key_chord},
    keybindings::{assign_normalized_keybinding_chord, remove_keybinding_assignment},
    path_display::{display_error_label_cow, sanitized_display_label_cow},
    workspace_state::settings_path,
};
use eframe::egui::KeyboardShortcut;
use kuroya_core::{Command, keymap::KeyBinding};
use std::{borrow::Cow, fmt::Write};

const KEYBINDING_STATUS_LABEL_MAX_CHARS: usize = 120;
const KEYBINDING_STATUS_CHORD_MAX_CHARS: usize = 48;

pub(crate) fn keybinding_chord_rejection_reason(chord: &str) -> Option<&'static str> {
    keybinding_shortcut_rejection_reason(parse_key_chord(chord)?)
}

fn keybinding_shortcut_rejection_reason(shortcut: KeyboardShortcut) -> Option<&'static str> {
    if keybinding_requires_primary_modifier(shortcut.logical_key)
        && !(shortcut.modifiers.ctrl
            || shortcut.modifiers.alt
            || shortcut.modifiers.mac_cmd
            || shortcut.modifiers.command)
    {
        return Some("Use Ctrl, Alt, or Cmd with text shortcuts");
    }
    None
}

pub(crate) fn malformed_keybinding_chord_rejection_reason(chord: &str) -> Option<&'static str> {
    let Some(shortcut) = parse_key_chord(chord) else {
        return Some("That shortcut is not supported");
    };
    if normalize_key_chord(chord).is_none() {
        return Some("That shortcut is not supported");
    }
    keybinding_shortcut_rejection_reason(shortcut)
}

impl KuroyaApp {
    pub(crate) fn save_keybinding_chord(&mut self, command: Command, chord: String) {
        let label = command_label(&command);
        if let Some(reason) = malformed_keybinding_chord_rejection_reason(&chord) {
            self.status = keybinding_rejection_status(&label, reason);
            return;
        }
        let Some(chord) = normalize_key_chord(&chord) else {
            self.status = keybinding_rejection_status(&label, "That shortcut is not supported");
            return;
        };
        let mut settings = self.settings.clone();
        let stale_count = prune_stale_keybinding_assignments(&mut settings.keymap.bindings);

        let status_chord = chord.clone();
        let conflict =
            assign_normalized_keybinding_chord(&mut settings.keymap.bindings, command, chord);
        let conflict_label = conflict.as_ref().map(command_label);

        match settings.save(&settings_path(&self.workspace.root)) {
            Ok(()) => {
                self.settings = settings;
                self.status = keybinding_bound_status(
                    &label,
                    &status_chord,
                    conflict_label.as_deref(),
                    stale_count,
                );
            }
            Err(error) => {
                self.status = keybinding_change_save_failed_status(error);
            }
        }
    }

    pub(crate) fn remove_keybinding_for_command(&mut self, command: Command) {
        let label = command_label(&command);
        let mut settings = self.settings.clone();
        if !remove_keybinding_assignment(&mut settings.keymap.bindings, &command) {
            self.status = keybinding_no_shortcut_status(&label);
            return;
        }

        match settings.save(&settings_path(&self.workspace.root)) {
            Ok(()) => {
                self.settings = settings;
                self.status = keybinding_removed_status(&label);
            }
            Err(error) => {
                self.status = keybinding_remove_save_failed_status(error);
            }
        }
    }
}

fn prune_stale_keybinding_assignments(bindings: &mut Vec<KeyBinding>) -> usize {
    let mut next = Vec::with_capacity(bindings.len());
    let mut stale_count = 0usize;

    for binding in bindings.drain(..) {
        let Some(chord) = normalized_supported_keybinding_chord(&binding.chord) else {
            stale_count += 1;
            continue;
        };
        if next.iter().any(|kept: &KeyBinding| {
            kept.command == binding.command || kept.chord.as_str() == chord.as_str()
        }) {
            stale_count += 1;
            continue;
        }

        next.push(KeyBinding {
            chord,
            command: binding.command,
        });
    }

    *bindings = next;
    stale_count
}

fn normalized_supported_keybinding_chord(chord: &str) -> Option<String> {
    let chord = normalize_key_chord(chord)?;
    keybinding_chord_rejection_reason(&chord)
        .is_none()
        .then_some(chord)
}

fn push_stale_keybinding_cleanup_suffix(status: &mut String, stale_count: usize) {
    match stale_count {
        0 => {}
        1 => status.push_str("; cleaned 1 stale shortcut"),
        _ => {
            let _ = write!(status, "; cleaned {stale_count} stale shortcuts");
        }
    }
}

fn keybinding_rejection_status(command_label: &str, reason: &str) -> String {
    let label = keybinding_status_label_cow(command_label);
    let mut status = String::with_capacity("Could not bind : ".len() + label.len() + reason.len());
    status.push_str("Could not bind ");
    status.push_str(&label);
    status.push_str(": ");
    status.push_str(reason);
    status
}

fn keybinding_bound_status(
    command_label: &str,
    chord: &str,
    conflict_label: Option<&str>,
    stale_count: usize,
) -> String {
    let label = keybinding_status_label_cow(command_label);
    let chord = keybinding_status_chord_cow(chord);
    let mut status = if let Some(conflict_label) = conflict_label {
        let conflict_label = keybinding_status_label_cow(conflict_label);
        let mut status = String::with_capacity(
            "Bound  to , replacing ".len() + label.len() + chord.len() + conflict_label.len(),
        );
        status.push_str("Bound ");
        status.push_str(&label);
        status.push_str(" to ");
        status.push_str(&chord);
        status.push_str(", replacing ");
        status.push_str(&conflict_label);
        status
    } else {
        let mut status = String::with_capacity("Bound  to ".len() + label.len() + chord.len());
        status.push_str("Bound ");
        status.push_str(&label);
        status.push_str(" to ");
        status.push_str(&chord);
        status
    };
    push_stale_keybinding_cleanup_suffix(&mut status, stale_count);
    status
}

fn keybinding_no_shortcut_status(command_label: &str) -> String {
    let label = keybinding_status_label_cow(command_label);
    let mut status = String::with_capacity(label.len() + " has no shortcut".len());
    status.push_str(&label);
    status.push_str(" has no shortcut");
    status
}

fn keybinding_removed_status(command_label: &str) -> String {
    let label = keybinding_status_label_cow(command_label);
    let mut status = String::with_capacity("Removed shortcut for ".len() + label.len());
    status.push_str("Removed shortcut for ");
    status.push_str(&label);
    status
}

fn keybinding_change_save_failed_status(error: impl std::fmt::Display) -> String {
    let error = error.to_string();
    let error = display_error_label_cow(&error);
    let mut status =
        String::with_capacity("Could not save keybinding change: ".len() + error.len());
    status.push_str("Could not save keybinding change: ");
    status.push_str(&error);
    status
}

fn keybinding_remove_save_failed_status(error: impl std::fmt::Display) -> String {
    let error = error.to_string();
    let error = display_error_label_cow(&error);
    let mut status =
        String::with_capacity("Could not save keybinding removal: ".len() + error.len());
    status.push_str("Could not save keybinding removal: ");
    status.push_str(&error);
    status
}

#[cfg(test)]
fn keybinding_status_label(label: &str) -> String {
    keybinding_status_label_cow(label).into_owned()
}

fn keybinding_status_label_cow(label: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(label, KEYBINDING_STATUS_LABEL_MAX_CHARS, "command")
}

#[cfg(test)]
fn keybinding_status_chord(chord: &str) -> String {
    keybinding_status_chord_cow(chord).into_owned()
}

fn keybinding_status_chord_cow(chord: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(chord, KEYBINDING_STATUS_CHORD_MAX_CHARS, "shortcut")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path_display::DISPLAY_ERROR_LABEL_MAX_CHARS;

    #[test]
    fn keybinding_bound_status_sanitizes_and_bounds_display_labels() {
        let status = keybinding_bound_status(
            &format!(
                "Run Plugin Command plugin\n{}:save\u{202e}",
                "x".repeat(240)
            ),
            &format!("Ctrl+Shift+{}\n\u{202e}", "K".repeat(120)),
            Some(&format!(
                "Run Plugin Command conflict\u{202e}:{}",
                "y".repeat(240)
            )),
            2,
        );

        assert_status_text_is_safe(&status);
        assert!(status.contains("replacing"));
        assert!(status.contains("; cleaned 2 stale shortcuts"));
        assert!(status.contains("..."));
        assert!(
            status.chars().count()
                <= "Bound  to , replacing ; cleaned 2 stale shortcuts"
                    .chars()
                    .count()
                    + KEYBINDING_STATUS_LABEL_MAX_CHARS * 2
                    + KEYBINDING_STATUS_CHORD_MAX_CHARS
        );
    }

    #[test]
    fn keybinding_remove_statuses_sanitize_display_labels() {
        let label = format!(
            "Run Plugin Command plugin\n{}:remove\u{202e}",
            "x".repeat(240)
        );

        let no_shortcut = keybinding_no_shortcut_status(&label);
        let removed = keybinding_removed_status(&label);

        assert_status_text_is_safe(&no_shortcut);
        assert_status_text_is_safe(&removed);
        assert!(no_shortcut.contains("..."));
        assert!(removed.contains("..."));
        assert!(
            no_shortcut.chars().count()
                <= " has no shortcut".chars().count() + KEYBINDING_STATUS_LABEL_MAX_CHARS
        );
        assert!(
            removed.chars().count()
                <= "Removed shortcut for ".chars().count() + KEYBINDING_STATUS_LABEL_MAX_CHARS
        );
    }

    #[test]
    fn keybinding_save_failed_statuses_sanitize_error_text() {
        let error = format!("line one\nline two \u{202e}{}", "x".repeat(400));

        let changed = keybinding_change_save_failed_status(&error);
        let removed = keybinding_remove_save_failed_status(&error);

        assert_status_text_is_safe(&changed);
        assert_status_text_is_safe(&removed);
        assert!(changed.contains("..."));
        assert!(removed.contains("..."));
        assert!(
            changed.chars().count()
                <= "Could not save keybinding change: ".chars().count()
                    + DISPLAY_ERROR_LABEL_MAX_CHARS
        );
        assert!(
            removed.chars().count()
                <= "Could not save keybinding removal: ".chars().count()
                    + DISPLAY_ERROR_LABEL_MAX_CHARS
        );
    }

    #[test]
    fn keybinding_rejection_status_sanitizes_command_label() {
        let status = keybinding_rejection_status(
            &format!(
                "Run Plugin Command plugin\n{}:reject\u{202e}",
                "x".repeat(240)
            ),
            "That shortcut is not supported",
        );

        assert_status_text_is_safe(&status);
        assert!(status.contains("..."));
        assert!(
            status.chars().count()
                <= "Could not bind : That shortcut is not supported"
                    .chars()
                    .count()
                    + KEYBINDING_STATUS_LABEL_MAX_CHARS
        );
    }

    #[test]
    fn keybinding_status_cow_helpers_borrow_clean_ascii_and_unicode() {
        assert!(matches!(
            keybinding_status_label_cow("Toggle Terminal"),
            std::borrow::Cow::Borrowed("Toggle Terminal")
        ));

        let unicode_label = "Run \u{03bb} Command";
        match keybinding_status_label_cow(unicode_label) {
            std::borrow::Cow::Borrowed(label) => assert_eq!(label, unicode_label),
            std::borrow::Cow::Owned(label) => {
                panic!("expected borrowed command label, got {label:?}")
            }
        }

        assert!(matches!(
            keybinding_status_chord_cow("Ctrl+Shift+P"),
            std::borrow::Cow::Borrowed("Ctrl+Shift+P")
        ));

        let unicode_chord = "Ctrl+\u{03bb}";
        match keybinding_status_chord_cow(unicode_chord) {
            std::borrow::Cow::Borrowed(label) => assert_eq!(label, unicode_chord),
            std::borrow::Cow::Owned(label) => {
                panic!("expected borrowed shortcut label, got {label:?}")
            }
        }
    }

    #[test]
    fn keybinding_status_cow_helpers_own_dirty_truncated_and_fallback_output() {
        assert_owned_cow_eq(
            keybinding_status_label_cow("alpha\nbeta\u{202e}"),
            "alpha beta",
        );
        assert_owned_cow_eq(keybinding_status_chord_cow("Ctrl+K\n\u{2066}"), "Ctrl+K");

        let long_label = format!(
            "command-{}-tail",
            "x".repeat(KEYBINDING_STATUS_LABEL_MAX_CHARS * 2)
        );
        let truncated_label = keybinding_status_label_cow(&long_label);
        assert!(truncated_label.as_ref().starts_with("command-"));
        assert!(truncated_label.as_ref().contains("..."));
        assert!(truncated_label.as_ref().ends_with("-tail"));
        assert_eq!(
            truncated_label.as_ref().chars().count(),
            KEYBINDING_STATUS_LABEL_MAX_CHARS
        );
        assert!(matches!(truncated_label, std::borrow::Cow::Owned(_)));

        let long_chord = format!(
            "Ctrl+{}+End",
            "K".repeat(KEYBINDING_STATUS_CHORD_MAX_CHARS * 2)
        );
        let truncated_chord = keybinding_status_chord_cow(&long_chord);
        assert!(truncated_chord.as_ref().starts_with("Ctrl+"));
        assert!(truncated_chord.as_ref().contains("..."));
        assert!(truncated_chord.as_ref().ends_with("+End"));
        assert_eq!(
            truncated_chord.as_ref().chars().count(),
            KEYBINDING_STATUS_CHORD_MAX_CHARS
        );
        assert!(matches!(truncated_chord, std::borrow::Cow::Owned(_)));

        assert_owned_cow_eq(keybinding_status_label_cow("\n\u{202e}\u{0007}"), "command");
        assert_owned_cow_eq(
            keybinding_status_chord_cow("\n\u{202e}\u{0007}"),
            "shortcut",
        );
    }

    #[test]
    fn keybinding_status_wrappers_and_builders_match_cow_output() {
        let long_label = format!(
            "command-{}-tail",
            "x".repeat(KEYBINDING_STATUS_LABEL_MAX_CHARS * 2)
        );
        for value in [
            "Toggle Terminal",
            "Run \u{03bb} Command",
            "alpha\nbeta\u{202e}",
            "\n\u{202e}\u{0007}",
            long_label.as_str(),
        ] {
            assert_eq!(
                keybinding_status_label(value),
                keybinding_status_label_cow(value).into_owned()
            );
        }

        let long_chord = format!(
            "Ctrl+{}+End",
            "K".repeat(KEYBINDING_STATUS_CHORD_MAX_CHARS * 2)
        );
        for value in [
            "Ctrl+Shift+P",
            "Ctrl+\u{03bb}",
            "Ctrl+K\n\u{2066}",
            "\n\u{202e}\u{0007}",
            long_chord.as_str(),
        ] {
            assert_eq!(
                keybinding_status_chord(value),
                keybinding_status_chord_cow(value).into_owned()
            );
        }

        let command_label = "Run\nCommand\u{202e}";
        let chord = "Ctrl+K\n\u{2066}";
        let conflict_label = "Open\nFile\u{202e}";
        let label = keybinding_status_label_cow(command_label);
        let status_chord = keybinding_status_chord_cow(chord);
        let conflict = keybinding_status_label_cow(conflict_label);

        assert_eq!(
            keybinding_rejection_status(command_label, "That shortcut is not supported"),
            format!("Could not bind {label}: That shortcut is not supported")
        );
        assert_eq!(
            keybinding_bound_status(command_label, chord, Some(conflict_label), 2),
            format!(
                "Bound {label} to {status_chord}, replacing {conflict}; cleaned 2 stale shortcuts"
            )
        );
        assert_eq!(
            keybinding_bound_status(command_label, chord, None, 0),
            format!("Bound {label} to {status_chord}")
        );
        assert_eq!(
            keybinding_no_shortcut_status(command_label),
            format!("{label} has no shortcut")
        );
        assert_eq!(
            keybinding_removed_status(command_label),
            format!("Removed shortcut for {label}")
        );
    }

    #[test]
    fn malformed_keybinding_chord_rejects_parsed_but_unsupported_keys() {
        assert_eq!(
            malformed_keybinding_chord_rejection_reason("Ctrl+Escape"),
            Some("That shortcut is not supported")
        );
        assert_eq!(
            malformed_keybinding_chord_rejection_reason("Ctrl+Unknown"),
            Some("That shortcut is not supported")
        );
        assert_eq!(
            malformed_keybinding_chord_rejection_reason("A"),
            Some("Use Ctrl, Alt, or Cmd with text shortcuts")
        );
        assert_eq!(
            malformed_keybinding_chord_rejection_reason(" control + shift + z "),
            None
        );
    }

    #[test]
    fn prune_stale_keybinding_assignments_keeps_first_valid_command_and_chord() {
        let mut bindings = vec![
            KeyBinding {
                chord: "ctrl+p".to_owned(),
                command: Command::ToggleQuickOpen,
            },
            KeyBinding {
                chord: "Ctrl+P".to_owned(),
                command: Command::ToggleCommandPalette,
            },
            KeyBinding {
                chord: "Ctrl+Shift+P".to_owned(),
                command: Command::ToggleQuickOpen,
            },
            KeyBinding {
                chord: "P".to_owned(),
                command: Command::ToggleTerminal,
            },
            KeyBinding {
                chord: "Ctrl+`".to_owned(),
                command: Command::ToggleTerminal,
            },
        ];

        assert_eq!(prune_stale_keybinding_assignments(&mut bindings), 3);
        assert_eq!(
            bindings,
            vec![
                KeyBinding {
                    chord: "Ctrl+P".to_owned(),
                    command: Command::ToggleQuickOpen,
                },
                KeyBinding {
                    chord: "Ctrl+`".to_owned(),
                    command: Command::ToggleTerminal,
                },
            ]
        );
    }

    fn assert_status_text_is_safe(status: &str) {
        assert!(!status.contains('\n'));
        assert!(!status.contains('\r'));
        assert!(!status.contains('\u{202e}'));
        assert!(!status.contains('\u{2066}'));
    }

    fn assert_owned_cow_eq(value: std::borrow::Cow<'_, str>, expected: &str) {
        match value {
            std::borrow::Cow::Owned(label) => assert_eq!(label, expected),
            std::borrow::Cow::Borrowed(label) => {
                panic!("expected owned label, got borrowed {label:?}")
            }
        }
    }
}
