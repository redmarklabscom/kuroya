use kuroya_core::TextBuffer;

use super::super::{VIM_DEFAULT_CTRL_SCROLL_LINES, VIM_DEFAULT_PAGE_SCROLL_LINES, VIM_MAX_COUNT};

pub(in crate::editor_vim_key_events) fn vim_ctrl_scroll_lines(count: Option<usize>) -> usize {
    count
        .unwrap_or(VIM_DEFAULT_CTRL_SCROLL_LINES)
        .clamp(1, VIM_MAX_COUNT)
}

pub(in crate::editor_vim_key_events) fn vim_line_scroll_lines(count: Option<usize>) -> usize {
    count.unwrap_or(1).clamp(1, VIM_MAX_COUNT)
}

pub(in crate::editor_vim_key_events) fn vim_page_scroll_lines(count: Option<usize>) -> usize {
    count
        .unwrap_or(1)
        .max(1)
        .saturating_mul(VIM_DEFAULT_PAGE_SCROLL_LINES)
        .min(VIM_MAX_COUNT)
}

pub(in crate::editor_vim_key_events) fn vim_move_down_lines(buffer: &mut TextBuffer, count: usize) {
    for _ in 0..count {
        buffer.move_down();
    }
}

pub(in crate::editor_vim_key_events) fn vim_move_up_lines(buffer: &mut TextBuffer, count: usize) {
    for _ in 0..count {
        buffer.move_up();
    }
}
