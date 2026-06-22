use eframe::egui::{Key, Modifiers};
use kuroya_core::TextBuffer;

use super::super::{EditorVimCharFind, EditorVimPendingKey, VimKeyResult};
use super::{
    vim_set_visual_character_selection, vim_visual_character_char_find_repeat_key,
    vim_visual_character_char_find_target, vim_visual_character_motion_target,
};

pub(in crate::editor_vim_key_events) fn handle_vim_visual_character_navigation_key_event(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    pending: &mut Option<EditorVimPendingKey>,
    last_char_find: &mut Option<EditorVimCharFind>,
    anchor: usize,
    cursor: usize,
    count: Option<usize>,
    suppress_text: Option<char>,
) -> Option<VimKeyResult> {
    if let Some(reverse) = vim_visual_character_char_find_repeat_key(key, modifiers) {
        if let Some(last) = *last_char_find {
            let motion = if reverse {
                last.motion.reversed()
            } else {
                last.motion
            };
            if let Some(target) = vim_visual_character_char_find_target(
                buffer,
                cursor,
                count.unwrap_or(1),
                motion,
                last.target,
            ) {
                vim_set_visual_character_selection(buffer, anchor, target);
                *pending = Some(EditorVimPendingKey::VisualCharacter {
                    anchor,
                    cursor: target,
                });
                return Some(VimKeyResult::handled(suppress_text));
            }
        }

        vim_set_visual_character_selection(buffer, anchor, cursor);
        *pending = Some(EditorVimPendingKey::VisualCharacter { anchor, cursor });
        return Some(VimKeyResult::handled(suppress_text));
    }

    if let Some(target) =
        vim_visual_character_motion_target(buffer, cursor, count.unwrap_or(1), key, modifiers)
    {
        vim_set_visual_character_selection(buffer, anchor, target);
        *pending = Some(EditorVimPendingKey::VisualCharacter {
            anchor,
            cursor: target,
        });
        return Some(VimKeyResult::handled(suppress_text));
    }

    None
}
