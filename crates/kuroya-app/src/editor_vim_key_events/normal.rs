use eframe::egui::{Key, Modifiers};
use kuroya_core::TextBuffer;

use super::{
    EditorVimCharFind, EditorVimLastChange, EditorVimMode, EditorVimPendingKey, EditorVimRegister,
    VIM_MAX_COUNT, VimKeyResult, handle_vim_command_input_key_event,
    handle_vim_pending_or_direct_normal_key_event, handle_vim_search_input_key_event,
    vim_cancel_pending_visual_character, vim_clear_command_input, vim_clear_search_input,
    vim_command_input_accept_key, vim_command_input_cancel_key, vim_command_input_control_edit,
    vim_count_digit, vim_ctrl_scroll_lines, vim_escape_key, vim_line_scroll_lines,
    vim_move_down_lines, vim_move_up_lines, vim_page_scroll_lines, vim_printable_key_char,
    vim_push_count_digit, vim_search_input_accept_key, vim_search_input_cancel_key,
    vim_search_input_control_edit,
};

pub(super) fn handle_vim_normal_key_event(
    buffer: &mut TextBuffer,
    key: Key,
    modifiers: Modifiers,
    mode: &mut EditorVimMode,
    pending: &mut Option<EditorVimPendingKey>,
    last_char_find: &mut Option<EditorVimCharFind>,
    unnamed_register: &mut Option<EditorVimRegister>,
    last_change: &mut Option<EditorVimLastChange>,
    indent_unit: &str,
) -> VimKeyResult {
    if modifiers.command || modifiers.alt {
        if matches!(*pending, Some(EditorVimPendingKey::SearchInput { .. })) {
            vim_clear_search_input();
        }
        if matches!(*pending, Some(EditorVimPendingKey::CommandInput)) {
            vim_clear_command_input();
        }
        vim_cancel_pending_visual_character(buffer, *pending);
        *pending = None;
        return VimKeyResult::ignored();
    }

    let suppress_text = vim_printable_key_char(key, modifiers);
    let count = if let Some(EditorVimPendingKey::Count(count)) = *pending {
        if let Some(digit) = vim_count_digit(key, modifiers, true) {
            *pending = Some(EditorVimPendingKey::Count(vim_push_count_digit(
                count, digit,
            )));
            return VimKeyResult::handled(suppress_text);
        }
        *pending = None;
        Some(count)
    } else {
        None
    };
    let count_value = count.unwrap_or(1).clamp(1, VIM_MAX_COUNT);
    if vim_escape_key(key, modifiers) {
        if matches!(*pending, Some(EditorVimPendingKey::SearchInput { .. })) {
            vim_clear_search_input();
        }
        if matches!(*pending, Some(EditorVimPendingKey::CommandInput)) {
            vim_clear_command_input();
        }
        vim_cancel_pending_visual_character(buffer, *pending);
        *pending = None;
        return VimKeyResult::handled(None);
    }
    if let Some(EditorVimPendingKey::SearchInput { count, forward }) = *pending
        && vim_search_input_accept_key(key, modifiers)
    {
        *pending = None;
        return handle_vim_search_input_key_event(
            buffer,
            key,
            modifiers,
            pending,
            count,
            forward,
            suppress_text,
        );
    }
    if let Some(EditorVimPendingKey::SearchInput { count, forward }) = *pending
        && vim_search_input_control_edit(key, modifiers).is_some()
    {
        *pending = None;
        return handle_vim_search_input_key_event(
            buffer,
            key,
            modifiers,
            pending,
            count,
            forward,
            suppress_text,
        );
    }
    if matches!(*pending, Some(EditorVimPendingKey::CommandInput))
        && vim_command_input_accept_key(key, modifiers)
    {
        *pending = None;
        return handle_vim_command_input_key_event(buffer, key, modifiers, pending, suppress_text);
    }
    if matches!(*pending, Some(EditorVimPendingKey::CommandInput))
        && vim_command_input_control_edit(key, modifiers).is_some()
    {
        *pending = None;
        return handle_vim_command_input_key_event(buffer, key, modifiers, pending, suppress_text);
    }
    if matches!(*pending, Some(EditorVimPendingKey::CommandInput))
        && vim_command_input_cancel_key(key, modifiers)
    {
        vim_clear_command_input();
        *pending = None;
        return VimKeyResult::handled(None);
    }
    if matches!(*pending, Some(EditorVimPendingKey::SearchInput { .. }))
        && vim_search_input_cancel_key(key, modifiers)
    {
        vim_clear_search_input();
        *pending = None;
        return VimKeyResult::handled(None);
    }
    if modifiers.ctrl {
        if matches!(*pending, Some(EditorVimPendingKey::SearchInput { .. })) {
            vim_clear_search_input();
        }
        if matches!(*pending, Some(EditorVimPendingKey::CommandInput)) {
            vim_clear_command_input();
        }
        vim_cancel_pending_visual_character(buffer, *pending);
        *pending = None;
        if modifiers.shift {
            return VimKeyResult::ignored();
        }
        return match key {
            Key::R => {
                if buffer.redo() {
                    VimKeyResult::changed(None)
                } else {
                    VimKeyResult::handled(None)
                }
            }
            Key::D => {
                vim_move_down_lines(buffer, vim_ctrl_scroll_lines(count));
                VimKeyResult::handled(None)
            }
            Key::E => {
                vim_move_down_lines(buffer, vim_line_scroll_lines(count));
                VimKeyResult::handled(None)
            }
            Key::F => {
                vim_move_down_lines(buffer, vim_page_scroll_lines(count));
                VimKeyResult::handled(None)
            }
            Key::N => {
                vim_move_down_lines(buffer, vim_line_scroll_lines(count));
                VimKeyResult::handled(None)
            }
            Key::B => {
                vim_move_up_lines(buffer, vim_page_scroll_lines(count));
                VimKeyResult::handled(None)
            }
            Key::P => {
                vim_move_up_lines(buffer, vim_line_scroll_lines(count));
                VimKeyResult::handled(None)
            }
            Key::U => {
                vim_move_up_lines(buffer, vim_ctrl_scroll_lines(count));
                VimKeyResult::handled(None)
            }
            Key::Y => {
                vim_move_up_lines(buffer, vim_line_scroll_lines(count));
                VimKeyResult::handled(None)
            }
            _ => VimKeyResult::ignored(),
        };
    }
    handle_vim_pending_or_direct_normal_key_event(
        buffer,
        key,
        modifiers,
        mode,
        pending,
        last_char_find,
        unnamed_register,
        last_change,
        indent_unit,
        count,
        count_value,
        suppress_text,
    )
}
