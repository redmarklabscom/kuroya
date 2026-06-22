use crate::editor_vim_key_events::{vim_key_sequence_is_single_supported, vim_key_token_for_event};
use eframe::egui::{Context, Event, Key, Modifiers};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::preference_panels::sections::vim) enum CapturedVimKey {
    Key(String),
    Escape,
    Rejected(String),
}

pub(in crate::preference_panels::sections::vim) fn capture_vim_key_input(
    ctx: &Context,
) -> Option<CapturedVimKey> {
    ctx.input(|input| capture_vim_key_event(&input.events))
}

pub(in crate::preference_panels::sections::vim) fn capture_vim_key_event(
    events: &[Event],
) -> Option<CapturedVimKey> {
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

        if modifiers.alt || modifiers.mac_cmd || command_modifier_without_ctrl(*modifiers) {
            return Some(CapturedVimKey::Rejected(
                "Alt and Cmd Vim bindings are not supported here yet".to_owned(),
            ));
        }
        let modifiers = normalize_platform_ctrl_modifiers(*modifiers);
        if vim_capture_escape_key(*key, modifiers) {
            return Some(CapturedVimKey::Escape);
        }
        if modifiers.ctrl {
            return Some(
                vim_key_token_for_event(*key, modifiers)
                    .filter(|token| vim_key_sequence_is_single_supported(token))
                    .map(CapturedVimKey::Key)
                    .unwrap_or_else(|| {
                        CapturedVimKey::Rejected(
                            "That Ctrl Vim binding is not supported here yet".to_owned(),
                        )
                    }),
            );
        }
        return Some(
            vim_key_token_for_event(*key, modifiers)
                .map(canonical_vim_capture_token)
                .filter(|token| vim_key_sequence_is_single_supported(token))
                .map(CapturedVimKey::Key)
                .unwrap_or_else(|| {
                    CapturedVimKey::Rejected(
                        "That key is not supported for Vim bindings".to_owned(),
                    )
                }),
        );
    }

    None
}

fn vim_capture_escape_key(key: Key, modifiers: Modifiers) -> bool {
    key == Key::Escape
        || (key == Key::OpenBracket
            && modifiers.ctrl
            && !modifiers.shift
            && !modifiers.alt
            && !command_modifier_without_ctrl(modifiers))
}

fn command_modifier_without_ctrl(modifiers: Modifiers) -> bool {
    modifiers.command && !modifiers.ctrl
}

fn normalize_platform_ctrl_modifiers(mut modifiers: Modifiers) -> Modifiers {
    if modifiers.ctrl && modifiers.command && !modifiers.mac_cmd {
        modifiers.command = false;
    }
    modifiers
}

fn canonical_vim_capture_token(token: String) -> String {
    if token == " " {
        "<Space>".to_owned()
    } else {
        token
    }
}
