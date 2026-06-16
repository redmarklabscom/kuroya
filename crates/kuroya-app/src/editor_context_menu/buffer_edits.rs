use crate::editor_input::EditorContextAction;
use eframe::egui;

pub(super) fn render_buffer_edit_context_menu(
    ui: &mut egui::Ui,
    pending_action: &mut Option<EditorContextAction>,
) {
    if ui.button("Duplicate Lines").clicked() {
        *pending_action = Some(EditorContextAction::DuplicateLines);
        ui.close();
    }
    if ui.button("Move Line Up").clicked() {
        *pending_action = Some(EditorContextAction::MoveLineUp);
        ui.close();
    }
    if ui.button("Move Line Down").clicked() {
        *pending_action = Some(EditorContextAction::MoveLineDown);
        ui.close();
    }
    if ui.button("Toggle Line Comment").clicked() {
        *pending_action = Some(EditorContextAction::ToggleLineComment);
        ui.close();
    }
    if ui.button("Delete Lines").clicked() {
        *pending_action = Some(EditorContextAction::DeleteLines);
        ui.close();
    }
    if ui.button("Join Lines").clicked() {
        *pending_action = Some(EditorContextAction::JoinLines);
        ui.close();
    }
    if ui.button("Add Cursors to Line Ends").clicked() {
        *pending_action = Some(EditorContextAction::AddCursorsToLineEnds);
        ui.close();
    }
    ui.separator();
    if ui.button("Indent Lines").clicked() {
        *pending_action = Some(EditorContextAction::IndentLines);
        ui.close();
    }
    if ui.button("Outdent Lines").clicked() {
        *pending_action = Some(EditorContextAction::OutdentLines);
        ui.close();
    }
}
