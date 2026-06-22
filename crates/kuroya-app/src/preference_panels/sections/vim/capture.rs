#[cfg(test)]
pub(super) use self::input::capture_vim_key_event;
pub(super) use self::{
    apply::apply_captured_vim_key,
    input::CapturedVimKey,
    state::{
        VimKeyCaptureState, VimKeyCaptureTarget, store_vim_key_capture_state,
        vim_key_capture_active, vim_key_capture_button_enabled, vim_key_capture_clear,
        vim_key_capture_manual_controls_enabled, vim_key_capture_state,
    },
};
use self::{
    apply::{restore_vim_key_capture_original, vim_key_capture_target_exists},
    input::capture_vim_key_input,
};
use eframe::egui::Context;
use kuroya_core::EditorVimSettings;
use std::time::Duration;

mod apply;
mod input;
mod state;

const VIM_ESCAPE_CANCEL_WINDOW_SECS: f64 = 1.0;

pub(super) fn handle_vim_key_capture(
    ctx: &Context,
    vim: &mut EditorVimSettings,
    state: &mut VimKeyCaptureState,
) -> bool {
    let Some(target) = state.target else {
        return false;
    };
    if !vim_key_capture_target_exists(target, vim) {
        state.clear_all();
        return false;
    }
    let now = ctx.input(|input| input.time);
    if finish_expired_vim_escape_capture(state, now) {
        return false;
    }

    let Some(captured) = capture_vim_key_input(ctx) else {
        return false;
    };
    handle_captured_vim_key(ctx, vim, state, target, captured, now);
    true
}

pub(super) fn handle_captured_vim_key(
    ctx: &Context,
    vim: &mut EditorVimSettings,
    state: &mut VimKeyCaptureState,
    target: VimKeyCaptureTarget,
    captured: CapturedVimKey,
    now: f64,
) {
    match captured {
        CapturedVimKey::Escape => handle_captured_vim_escape(ctx, vim, state, target, now),
        CapturedVimKey::Rejected(message) => state.set_error(target, message),
        CapturedVimKey::Key(key) => match apply_captured_vim_key(vim, target, key) {
            Ok(()) => state.clear_all(),
            Err(message) => state.set_error(target, message),
        },
    }
}

fn handle_captured_vim_escape(
    ctx: &Context,
    vim: &mut EditorVimSettings,
    state: &mut VimKeyCaptureState,
    target: VimKeyCaptureTarget,
    now: f64,
) {
    if state
        .escape_cancel
        .is_some_and(|pending| pending.target == target)
    {
        cancel_vim_key_capture(vim, state);
        return;
    }

    match apply_captured_vim_key(vim, target, "<Esc>".to_owned()) {
        Ok(()) => {
            state.start_escape_cancel(target, now);
            state.clear_error(target);
            request_vim_escape_cancel_repaint(ctx);
        }
        Err(message) => {
            state.start_escape_cancel(target, now);
            state.set_error(target, message);
            request_vim_escape_cancel_repaint(ctx);
        }
    }
}

pub(super) fn finish_expired_vim_escape_capture(state: &mut VimKeyCaptureState, now: f64) -> bool {
    let Some(pending) = state.escape_cancel else {
        return false;
    };
    if now - pending.started_at < VIM_ESCAPE_CANCEL_WINDOW_SECS {
        return false;
    }
    if state.error_for(pending.target).is_some() {
        state.escape_cancel = None;
        return false;
    }
    state.clear_all();
    true
}

pub(super) fn cancel_vim_key_capture(vim: &mut EditorVimSettings, state: &mut VimKeyCaptureState) {
    if let Some(original) = state.original.clone() {
        restore_vim_key_capture_original(vim, &original);
    }
    state.clear_all();
}

fn request_vim_escape_cancel_repaint(ctx: &Context) {
    ctx.request_repaint_after(Duration::from_secs_f64(VIM_ESCAPE_CANCEL_WINDOW_SECS));
}
