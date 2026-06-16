use crate::KuroyaApp;
use eframe::egui::{Key, Modifiers};
use kuroya_core::BufferId;

pub(super) fn handle_modified_editor_key_event(
    app: &mut KuroyaApp,
    buffer_id: BufferId,
    key: Key,
    modifiers: Modifiers,
    changed: &mut bool,
) -> bool {
    if (modifiers.ctrl || modifiers.alt)
        && matches!(
            key,
            Key::ArrowLeft | Key::ArrowRight | Key::Backspace | Key::Delete
        )
    {
        if let Some(buffer) = app.buffer_mut(buffer_id) {
            let handled = match key {
                Key::ArrowLeft if modifiers.shift => {
                    buffer.extend_word_left();
                    true
                }
                Key::ArrowLeft => {
                    buffer.move_word_left();
                    true
                }
                Key::ArrowRight if modifiers.shift => {
                    buffer.extend_word_right();
                    true
                }
                Key::ArrowRight => {
                    buffer.move_word_right();
                    true
                }
                Key::Backspace => {
                    *changed |= buffer.delete_word_backward();
                    true
                }
                Key::Delete => {
                    *changed |= buffer.delete_word_forward();
                    true
                }
                _ => false,
            };
            if handled {
                return true;
            }
        }
    }

    if modifiers.command || modifiers.ctrl {
        match key {
            Key::A => {
                if let Some(buffer) = app.buffer_mut(buffer_id) {
                    buffer.select_all();
                }
            }
            Key::ArrowLeft if modifiers.shift => {
                if let Some(buffer) = app.buffer_mut(buffer_id) {
                    buffer.extend_line_start();
                }
            }
            Key::ArrowLeft => {
                if let Some(buffer) = app.buffer_mut(buffer_id) {
                    buffer.move_line_start();
                }
            }
            Key::ArrowRight if modifiers.shift => {
                if let Some(buffer) = app.buffer_mut(buffer_id) {
                    buffer.extend_line_end();
                }
            }
            Key::ArrowRight => {
                if let Some(buffer) = app.buffer_mut(buffer_id) {
                    buffer.move_line_end();
                }
            }
            Key::Z if modifiers.shift => {
                if let Some(buffer) = app.buffer_mut(buffer_id) {
                    *changed |= buffer.redo();
                }
            }
            Key::Z => {
                if let Some(buffer) = app.buffer_mut(buffer_id) {
                    *changed |= buffer.undo();
                }
            }
            Key::Y => {
                if let Some(buffer) = app.buffer_mut(buffer_id) {
                    *changed |= buffer.redo();
                }
            }
            _ => {}
        }
        return true;
    }

    false
}
