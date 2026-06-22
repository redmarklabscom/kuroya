use eframe::egui::Modifiers;

pub(in crate::editor_vim_key_events) fn no_text_modifiers(modifiers: Modifiers) -> bool {
    !modifiers.ctrl && !modifiers.command && !modifiers.alt && !modifiers.shift
}
