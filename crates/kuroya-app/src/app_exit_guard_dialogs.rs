use crate::{KuroyaApp, transient_state::PendingExit};
use confirm::render_exit_confirm_guard;
use eframe::egui::Context;
use saving::render_exit_saving_guard;

mod confirm;
mod saving;

impl KuroyaApp {
    pub(crate) fn render_exit_guard(&mut self, ctx: &Context) {
        if matches!(self.pending_exit.as_ref(), Some(PendingExit::Confirm)) {
            render_exit_confirm_guard(self, ctx);
        } else if matches!(self.pending_exit.as_ref(), Some(PendingExit::Saving { .. })) {
            render_exit_saving_guard(self, ctx);
        }
    }
}
