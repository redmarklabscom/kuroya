use super::super::{super::capture as vim_capture, helpers::key_event};
use eframe::egui::{Key, Modifiers};

#[test]
fn vim_key_capture_captures_plain_shifted_and_named_keys() {
    assert_eq!(
        vim_capture::capture_vim_key_event(&[key_event(Key::H, Modifiers::NONE)]),
        Some(vim_capture::CapturedVimKey::Key("h".to_owned()))
    );
    assert_eq!(
        vim_capture::capture_vim_key_event(&[key_event(Key::H, Modifiers::SHIFT)]),
        Some(vim_capture::CapturedVimKey::Key("H".to_owned()))
    );
    assert_eq!(
        vim_capture::capture_vim_key_event(&[key_event(Key::Space, Modifiers::NONE)]),
        Some(vim_capture::CapturedVimKey::Key("<Space>".to_owned()))
    );
    assert_eq!(
        vim_capture::capture_vim_key_event(&[key_event(Key::Enter, Modifiers::NONE)]),
        Some(vim_capture::CapturedVimKey::Key("<Enter>".to_owned()))
    );
    assert_eq!(
        vim_capture::capture_vim_key_event(&[key_event(Key::Home, Modifiers::NONE)]),
        Some(vim_capture::CapturedVimKey::Key("<Home>".to_owned()))
    );
    assert_eq!(
        vim_capture::capture_vim_key_event(&[key_event(Key::End, Modifiers::NONE)]),
        Some(vim_capture::CapturedVimKey::Key("<End>".to_owned()))
    );
    assert_eq!(
        vim_capture::capture_vim_key_event(&[key_event(Key::R, Modifiers::CTRL)]),
        Some(vim_capture::CapturedVimKey::Key("<C-r>".to_owned()))
    );
    let platform_ctrl = Modifiers {
        ctrl: true,
        command: true,
        ..Modifiers::NONE
    };
    assert_eq!(
        vim_capture::capture_vim_key_event(&[key_event(Key::R, platform_ctrl)]),
        Some(vim_capture::CapturedVimKey::Key("<C-r>".to_owned()))
    );
    assert_eq!(
        vim_capture::capture_vim_key_event(&[key_event(Key::OpenBracket, Modifiers::CTRL)]),
        Some(vim_capture::CapturedVimKey::Escape)
    );
    assert_eq!(
        vim_capture::capture_vim_key_event(&[key_event(Key::OpenBracket, platform_ctrl)]),
        Some(vim_capture::CapturedVimKey::Escape)
    );
    assert_eq!(
        vim_capture::capture_vim_key_event(&[key_event(Key::Comma, Modifiers::SHIFT)]),
        Some(vim_capture::CapturedVimKey::Key("<".to_owned()))
    );
}

#[test]
fn vim_key_capture_cancels_and_rejects_unsupported_shortcuts() {
    assert_eq!(
        vim_capture::capture_vim_key_event(&[key_event(Key::Escape, Modifiers::NONE)]),
        Some(vim_capture::CapturedVimKey::Escape)
    );
    assert_eq!(
        vim_capture::capture_vim_key_event(&[key_event(Key::A, Modifiers::CTRL)]),
        Some(vim_capture::CapturedVimKey::Rejected(
            "That Ctrl Vim binding is not supported here yet".to_owned()
        ))
    );
    let platform_ctrl = Modifiers {
        ctrl: true,
        command: true,
        ..Modifiers::NONE
    };
    assert_eq!(
        vim_capture::capture_vim_key_event(&[key_event(Key::A, platform_ctrl)]),
        Some(vim_capture::CapturedVimKey::Rejected(
            "That Ctrl Vim binding is not supported here yet".to_owned()
        ))
    );
    assert_eq!(
        vim_capture::capture_vim_key_event(&[key_event(
            Key::R,
            Modifiers {
                mac_cmd: true,
                command: true,
                ..Modifiers::NONE
            }
        )]),
        Some(vim_capture::CapturedVimKey::Rejected(
            "Alt and Cmd Vim bindings are not supported here yet".to_owned()
        ))
    );
    assert_eq!(
        vim_capture::capture_vim_key_event(&[key_event(Key::F1, Modifiers::NONE)]),
        Some(vim_capture::CapturedVimKey::Rejected(
            "That key is not supported for Vim bindings".to_owned()
        ))
    );
}
