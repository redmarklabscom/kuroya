use eframe::egui::{Context, Id};

const VIM_KEY_CAPTURE_STATE_ID: &str = "kuroya.settings.vim.key_capture";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::preference_panels::sections::vim) enum VimKeyCaptureTarget {
    BuiltIn(&'static str),
    CustomDisabled(usize),
    CustomOverrideBefore(usize),
    CustomOverrideAfter(usize),
}

#[derive(Clone, Debug, Default)]
pub(in crate::preference_panels::sections::vim) struct VimKeyCaptureState {
    pub(in crate::preference_panels::sections::vim) target: Option<VimKeyCaptureTarget>,
    pub(in crate::preference_panels::sections::vim) error: Option<VimKeyCaptureError>,
    pub(in crate::preference_panels::sections::vim) original: Option<VimKeyCaptureOriginal>,
    pub(in crate::preference_panels::sections::vim) escape_cancel: Option<VimEscapeCancel>,
    pub(in crate::preference_panels::sections::vim) frame_controls_locked: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::preference_panels::sections::vim) struct VimKeyCaptureError {
    pub(in crate::preference_panels::sections::vim) target: VimKeyCaptureTarget,
    pub(in crate::preference_panels::sections::vim) message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::preference_panels::sections::vim) struct VimKeyCaptureOriginal {
    pub(in crate::preference_panels::sections::vim) target: VimKeyCaptureTarget,
    pub(in crate::preference_panels::sections::vim) value: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::preference_panels::sections::vim) struct VimEscapeCancel {
    pub(in crate::preference_panels::sections::vim) target: VimKeyCaptureTarget,
    pub(in crate::preference_panels::sections::vim) started_at: f64,
}

impl VimKeyCaptureState {
    pub(in crate::preference_panels::sections::vim) fn is_capturing(
        &self,
        target: VimKeyCaptureTarget,
    ) -> bool {
        self.target == Some(target)
    }

    pub(in crate::preference_panels::sections::vim) fn start(
        &mut self,
        target: VimKeyCaptureTarget,
        original_value: String,
    ) {
        self.target = Some(target);
        self.original = Some(VimKeyCaptureOriginal {
            target,
            value: original_value,
        });
        self.escape_cancel = None;
        self.clear_error(target);
    }

    pub(in crate::preference_panels::sections::vim) fn start_escape_cancel(
        &mut self,
        target: VimKeyCaptureTarget,
        started_at: f64,
    ) {
        self.escape_cancel = Some(VimEscapeCancel { target, started_at });
    }

    pub(in crate::preference_panels::sections::vim) fn clear_all(&mut self) {
        self.target = None;
        self.error = None;
        self.original = None;
        self.escape_cancel = None;
    }

    pub(in crate::preference_panels::sections::vim) fn lock_controls_for_frame(&mut self) {
        self.frame_controls_locked = true;
    }

    pub(in crate::preference_panels::sections::vim) fn clear_frame_controls_lock(&mut self) {
        self.frame_controls_locked = false;
    }

    pub(in crate::preference_panels::sections::vim) fn set_error(
        &mut self,
        target: VimKeyCaptureTarget,
        message: String,
    ) {
        self.error = Some(VimKeyCaptureError { target, message });
    }

    pub(in crate::preference_panels::sections::vim) fn clear_error(
        &mut self,
        target: VimKeyCaptureTarget,
    ) {
        if self
            .error
            .as_ref()
            .is_some_and(|error| error.target == target)
        {
            self.error = None;
        }
    }

    pub(in crate::preference_panels::sections::vim) fn error_for(
        &self,
        target: VimKeyCaptureTarget,
    ) -> Option<&str> {
        self.error
            .as_ref()
            .filter(|error| error.target == target)
            .map(|error| error.message.as_str())
    }
}

pub(in crate::preference_panels::sections::vim) fn vim_key_capture_active(ctx: &Context) -> bool {
    vim_key_capture_state(ctx).target.is_some()
}

pub(in crate::preference_panels::sections::vim) fn vim_key_capture_clear(ctx: &Context) {
    ctx.data_mut(|data| data.remove::<VimKeyCaptureState>(Id::new(VIM_KEY_CAPTURE_STATE_ID)));
}

pub(in crate::preference_panels::sections::vim) fn vim_key_capture_manual_controls_enabled(
    state: &VimKeyCaptureState,
) -> bool {
    state.target.is_none() && !state.frame_controls_locked
}

pub(in crate::preference_panels::sections::vim) fn vim_key_capture_button_enabled(
    state: &VimKeyCaptureState,
    target: VimKeyCaptureTarget,
) -> bool {
    !state.frame_controls_locked && (state.target.is_none() || state.is_capturing(target))
}

pub(in crate::preference_panels::sections::vim) fn vim_key_capture_state(
    ctx: &Context,
) -> VimKeyCaptureState {
    ctx.data(|data| {
        data.get_temp::<VimKeyCaptureState>(Id::new(VIM_KEY_CAPTURE_STATE_ID))
            .unwrap_or_default()
    })
}

pub(in crate::preference_panels::sections::vim) fn store_vim_key_capture_state(
    ctx: &Context,
    state: VimKeyCaptureState,
) {
    ctx.data_mut(|data| data.insert_temp(Id::new(VIM_KEY_CAPTURE_STATE_ID), state));
}
