use crate::{KuroyaApp, transient_state::PendingWorkspaceSwitch};
use confirm::render_workspace_switch_confirm_guard;
use eframe::egui::Context;
use saving::render_workspace_switch_saving_guard;

mod confirm;
mod saving;

impl KuroyaApp {
    pub(crate) fn render_workspace_switch_guard(&mut self, ctx: &Context) {
        if self.exit_confirmed || self.pending_exit.is_some() {
            self.clear_pending_workspace_switch_for_exit();
            return;
        }
        if self.cancel_invalid_pending_workspace_switch() {
            return;
        }

        let confirm_target = match self.pending_workspace_switch.as_ref() {
            Some(PendingWorkspaceSwitch::Confirm { target }) => Some(target.clone()),
            Some(PendingWorkspaceSwitch::Saving { .. }) => None,
            None => return,
        };

        if let Some(target) = confirm_target {
            render_workspace_switch_confirm_guard(self, ctx, target);
        } else {
            render_workspace_switch_saving_guard(self, ctx);
        }
    }
}
