use crate::{
    KuroyaApp,
    popup_buttons::{PopupButtonKind, popup_button},
    save_lifecycle::has_active_save_work,
    transient_state::PendingExit,
    ui_text::count_label,
};
use eframe::egui::{self, Align, Context, Key, RichText};
use kuroya_core::{BufferId, TextBuffer};

pub(super) fn render_exit_saving_guard(app: &mut KuroyaApp, ctx: &Context) {
    app.advance_pending_exit_after_save();
    let remaining = match app.pending_exit.as_ref() {
        Some(PendingExit::Saving { ids }) => pending_exit_remaining_saves(app, ids),
        _ => {
            if app.exit_confirmed {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            return;
        }
    };
    let mut cancel = false;

    egui::Window::new("Exit Kuroya")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size([520.0, 132.0])
        .show(ctx, |ui| {
            ui.label(RichText::new("Saving before exit").strong());
            ui.label(exit_saving_body(remaining));

            if ui.input(|input| input.key_pressed(Key::Escape)) {
                cancel = true;
            }

            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if popup_button(ui, "Cancel", PopupButtonKind::Secondary).clicked() {
                    cancel = true;
                }
            });
        });

    if cancel {
        app.pending_exit = None;
        app.status = "Exit canceled; in-flight saves will still finish".to_owned();
    }
}

fn pending_exit_remaining_saves(app: &KuroyaApp, ids: &[BufferId]) -> usize {
    ids.iter()
        .filter(|id| {
            has_active_save_work(
                **id,
                &app.in_flight_saves,
                &app.queued_save_paths,
                &app.pending_format_on_save,
            ) || app.buffer(**id).is_some_and(TextBuffer::is_dirty)
        })
        .count()
}

fn exit_saving_body(remaining: usize) -> String {
    format!("Saving {}.", count_label(remaining, "file", "files"))
}

#[cfg(test)]
mod tests {
    use super::exit_saving_body;

    #[test]
    fn exit_saving_body_uses_file_count_labels() {
        assert_eq!(exit_saving_body(1), "Saving 1 file.");
        assert_eq!(exit_saving_body(2), "Saving 2 files.");
    }
}
