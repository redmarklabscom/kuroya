use crate::editor_input::EditorContextAction;
use eframe::egui;

pub(super) fn render_lsp_context_menu(
    ui: &mut egui::Ui,
    pending_action: &mut Option<EditorContextAction>,
) {
    if ui.button("Show Hover").clicked() {
        *pending_action = Some(EditorContextAction::ShowHover);
        ui.close();
    }
    if ui.button("Document Highlights").clicked() {
        *pending_action = Some(EditorContextAction::DocumentHighlights);
        ui.close();
    }
    if ui.button("Go to Definition").clicked() {
        *pending_action = Some(EditorContextAction::GoToDefinition);
        ui.close();
    }
    if ui.button("Find References").clicked() {
        *pending_action = Some(EditorContextAction::FindReferences);
        ui.close();
    }
    if ui.button("Call Hierarchy").clicked() {
        *pending_action = Some(EditorContextAction::ShowCallHierarchy);
        ui.close();
    }
    if ui.button("Type Hierarchy").clicked() {
        *pending_action = Some(EditorContextAction::ShowTypeHierarchy);
        ui.close();
    }
    if ui.button("Rename Symbol").clicked() {
        *pending_action = Some(EditorContextAction::RenameSymbol);
        ui.close();
    }
    if ui.button("File Symbols").clicked() {
        *pending_action = Some(EditorContextAction::ShowSymbols);
        ui.close();
    }
    if ui.button("Workspace Symbols").clicked() {
        *pending_action = Some(EditorContextAction::WorkspaceSymbols);
        ui.close();
    }
    if ui.button("Completions").clicked() {
        *pending_action = Some(EditorContextAction::ShowCompletions);
        ui.close();
    }
    if ui.button("Signature Help").clicked() {
        *pending_action = Some(EditorContextAction::SignatureHelp);
        ui.close();
    }
    if ui.button("Load Folds").clicked() {
        *pending_action = Some(EditorContextAction::LoadFolds);
        ui.close();
    }
    if ui.button("Toggle Fold").clicked() {
        *pending_action = Some(EditorContextAction::ToggleFold);
        ui.close();
    }
    if ui.button("Expand All Folds").clicked() {
        *pending_action = Some(EditorContextAction::ExpandAllFolds);
        ui.close();
    }
    if ui.button("Format Document").clicked() {
        *pending_action = Some(EditorContextAction::FormatDocument);
        ui.close();
    }
    if ui.button("Code Actions").clicked() {
        *pending_action = Some(EditorContextAction::CodeActions);
        ui.close();
    }
}
