use crate::keybinding_chords::{keybinding_chord_from_key, keybinding_requires_primary_modifier};
use eframe::egui::{Context, Event, Key};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CapturedKeybinding {
    Chord(String),
    Escape,
    Cancel,
    Rejected(String),
}

pub(crate) fn capture_keybinding_input(ctx: &Context) -> Option<CapturedKeybinding> {
    ctx.input(|input| capture_keybinding_event(&input.events))
}

pub(crate) fn capture_keybinding_event(events: &[Event]) -> Option<CapturedKeybinding> {
    for event in events {
        let Event::Key {
            key,
            pressed: true,
            repeat: false,
            modifiers,
            ..
        } = event
        else {
            continue;
        };

        if *key == Key::Escape
            && !modifiers.ctrl
            && !modifiers.alt
            && !modifiers.shift
            && !modifiers.command
            && !modifiers.mac_cmd
        {
            return Some(CapturedKeybinding::Escape);
        }
        if keybinding_requires_primary_modifier(*key)
            && !(modifiers.ctrl || modifiers.alt || modifiers.mac_cmd)
        {
            return Some(CapturedKeybinding::Rejected(
                "Use Ctrl, Alt, or Cmd with text shortcuts".to_owned(),
            ));
        }

        return Some(
            keybinding_chord_from_key(*key, *modifiers)
                .map(CapturedKeybinding::Chord)
                .unwrap_or_else(|| {
                    CapturedKeybinding::Rejected(
                        "That key is not supported for shortcuts".to_owned(),
                    )
                }),
        );
    }

    None
}
