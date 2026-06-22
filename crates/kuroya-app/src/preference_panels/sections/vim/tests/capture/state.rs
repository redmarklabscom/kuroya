use super::super::{
    super::{capture as vim_capture, editing},
    helpers::key_event,
};
use eframe::egui::{Context, Key, Modifiers};
use kuroya_core::EditorVimSettings;

#[test]
fn vim_key_capture_disables_manual_controls_and_other_capture_buttons() {
    let active_target = vim_capture::VimKeyCaptureTarget::BuiltIn("h");
    let other_target = vim_capture::VimKeyCaptureTarget::BuiltIn("j");
    let mut state = vim_capture::VimKeyCaptureState::default();

    assert!(vim_capture::vim_key_capture_manual_controls_enabled(&state));
    assert!(vim_capture::vim_key_capture_button_enabled(
        &state,
        active_target
    ));
    assert!(vim_capture::vim_key_capture_button_enabled(
        &state,
        other_target
    ));

    state.start(active_target, "h".to_owned());

    assert!(!vim_capture::vim_key_capture_manual_controls_enabled(
        &state
    ));
    assert!(vim_capture::vim_key_capture_button_enabled(
        &state,
        active_target
    ));
    assert!(!vim_capture::vim_key_capture_button_enabled(
        &state,
        other_target
    ));
}

#[test]
fn vim_key_capture_frame_lock_keeps_controls_disabled_after_capture_finishes() {
    let active_target = vim_capture::VimKeyCaptureTarget::BuiltIn("h");
    let other_target = vim_capture::VimKeyCaptureTarget::BuiltIn("j");
    let mut state = vim_capture::VimKeyCaptureState::default();

    state.lock_controls_for_frame();

    assert!(!vim_capture::vim_key_capture_manual_controls_enabled(
        &state
    ));
    assert!(!vim_capture::vim_key_capture_button_enabled(
        &state,
        active_target
    ));
    assert!(!vim_capture::vim_key_capture_button_enabled(
        &state,
        other_target
    ));

    state.clear_frame_controls_lock();

    assert!(vim_capture::vim_key_capture_manual_controls_enabled(&state));
    assert!(vim_capture::vim_key_capture_button_enabled(
        &state,
        active_target
    ));
}

#[test]
fn handle_vim_key_capture_reports_when_it_consumes_input() {
    let ctx = Context::default();
    let mut vim = EditorVimSettings::default();
    let mut state = vim_capture::VimKeyCaptureState::default();

    assert!(!vim_capture::handle_vim_key_capture(
        &ctx, &mut vim, &mut state
    ));

    state.start(
        vim_capture::VimKeyCaptureTarget::BuiltIn("h"),
        "h".to_owned(),
    );
    ctx.input_mut(|input| input.events.push(key_event(Key::H, Modifiers::SHIFT)));

    assert!(vim_capture::handle_vim_key_capture(
        &ctx, &mut vim, &mut state
    ));
    assert_eq!(editing::vim_builtin_effective_binding(&vim, "h"), "H");
    assert!(state.target.is_none());
}
