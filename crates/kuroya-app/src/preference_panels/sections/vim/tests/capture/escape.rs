use super::super::super::{capture as vim_capture, editing, ui};
use eframe::egui::Context;
use kuroya_core::{Command, EditorVimKeyOverride, EditorVimSettings};

#[test]
fn captured_vim_escape_captures_first_press_and_second_press_cancels() {
    let ctx = Context::default();
    let target = vim_capture::VimKeyCaptureTarget::BuiltIn("<Esc>");
    let mut vim = EditorVimSettings::default();
    let mut state = vim_capture::VimKeyCaptureState::default();
    state.start(target, "<Esc>".to_owned());

    vim_capture::handle_captured_vim_key(
        &ctx,
        &mut vim,
        &mut state,
        target,
        vim_capture::CapturedVimKey::Escape,
        10.0,
    );

    assert_eq!(
        editing::vim_builtin_effective_binding(&vim, "<Esc>"),
        "<Esc>"
    );
    assert!(state.is_capturing(target));
    assert!(state.escape_cancel.is_some());

    vim_capture::handle_captured_vim_key(
        &ctx,
        &mut vim,
        &mut state,
        target,
        vim_capture::CapturedVimKey::Escape,
        10.2,
    );

    assert_eq!(
        editing::vim_builtin_effective_binding(&vim, "<Esc>"),
        "<Esc>"
    );
    assert!(state.target.is_none());
    assert!(state.escape_cancel.is_none());
}

#[test]
fn captured_vim_escape_keeps_binding_after_cancel_window() {
    let ctx = Context::default();
    let target = vim_capture::VimKeyCaptureTarget::BuiltIn("<Esc>");
    let mut vim = EditorVimSettings::default();
    let mut state = vim_capture::VimKeyCaptureState::default();
    state.start(target, "<Esc>".to_owned());

    vim_capture::handle_captured_vim_key(
        &ctx,
        &mut vim,
        &mut state,
        target,
        vim_capture::CapturedVimKey::Escape,
        10.0,
    );

    assert!(vim_capture::finish_expired_vim_escape_capture(
        &mut state, 11.1
    ));
    assert_eq!(
        editing::vim_builtin_effective_binding(&vim, "<Esc>"),
        "<Esc>"
    );
    assert!(state.target.is_none());
}

#[test]
fn captured_vim_escape_rejects_keys_tied_to_escape_builtin() {
    let ctx = Context::default();
    let target = vim_capture::VimKeyCaptureTarget::BuiltIn("h");
    let mut vim = EditorVimSettings::default();
    let mut state = vim_capture::VimKeyCaptureState::default();
    state.start(target, "h".to_owned());

    vim_capture::handle_captured_vim_key(
        &ctx,
        &mut vim,
        &mut state,
        target,
        vim_capture::CapturedVimKey::Escape,
        10.0,
    );

    assert_eq!(editing::vim_builtin_effective_binding(&vim, "h"), "h");
    assert_eq!(state.error_for(target), Some("Already used by Escape"));
    assert!(state.is_capturing(target));
    assert!(state.escape_cancel.is_some());
}

#[test]
fn rejected_vim_escape_capture_keeps_capture_active_after_cancel_window() {
    let ctx = Context::default();
    let target = vim_capture::VimKeyCaptureTarget::BuiltIn("h");
    let mut vim = EditorVimSettings::default();
    let mut state = vim_capture::VimKeyCaptureState::default();
    state.start(target, "h".to_owned());

    vim_capture::handle_captured_vim_key(
        &ctx,
        &mut vim,
        &mut state,
        target,
        vim_capture::CapturedVimKey::Escape,
        10.0,
    );

    assert!(!vim_capture::finish_expired_vim_escape_capture(
        &mut state, 11.1
    ));
    assert!(state.is_capturing(target));
    assert_eq!(state.error_for(target), Some("Already used by Escape"));
    assert!(state.escape_cancel.is_none());
}

#[test]
fn captured_vim_escape_hint_distinguishes_captured_and_rejected_escape() {
    let ctx = Context::default();
    let target = vim_capture::VimKeyCaptureTarget::BuiltIn("h");
    let mut vim = EditorVimSettings {
        key_overrides: vec![EditorVimKeyOverride {
            before: "<Esc>".to_owned(),
            after: String::new(),
            command: Some(Command::RequestHover),
        }],
        ..EditorVimSettings::default()
    };
    let mut state = vim_capture::VimKeyCaptureState::default();
    state.start(target, "h".to_owned());

    assert_eq!(
        ui::vim_capture_hint_text(&state, target),
        "Press one Vim key. Esc once sets <Esc>; Esc twice cancels."
    );

    vim_capture::handle_captured_vim_key(
        &ctx,
        &mut vim,
        &mut state,
        target,
        vim_capture::CapturedVimKey::Escape,
        10.0,
    );

    assert!(state.error_for(target).is_some());
    assert_eq!(
        ui::vim_capture_hint_text(&state, target),
        "Esc was rejected. Press Esc again to cancel."
    );
}
