use egui::Modifiers;
use kuroya_core::clamp_terminal_font_size;
use vt100::{MouseProtocolEncoding, MouseProtocolMode};

use super::super::{
    TerminalCellPosition, TerminalSelectionRange, TerminalSession, TerminalTextSelection,
};

pub(super) const TERMINAL_INPUT_MAX_BYTES: usize = 1024 * 1024;
pub(super) const TERMINAL_BRACKETED_PASTE_PREFIX: &str = "\x1b[200~";
pub(super) const TERMINAL_BRACKETED_PASTE_SUFFIX: &str = "\x1b[201~";
pub(super) const TERMINAL_BRACKETED_PASTE_WRAPPER_BYTES: usize = 12;
pub(super) const TERMINAL_CURSOR_INPUT_REPEAT_LIMIT: usize = TERMINAL_INPUT_MAX_BYTES / 3;
const TERMINAL_WHEEL_INPUT_REPEAT_LIMIT: usize = 64;

pub(super) fn bounded_terminal_input(mut input: String, max_bytes: usize) -> String {
    if input.len() <= max_bytes {
        return input;
    }

    let mut truncate_at = max_bytes;
    while truncate_at > 0 && !input.is_char_boundary(truncate_at) {
        truncate_at -= 1;
    }
    input.truncate(truncate_at);
    input
}

pub(in crate::terminal) fn terminal_paste_input(
    text: String,
    bracketed_paste: bool,
    ignore_bracketed_paste_mode: bool,
) -> String {
    if bracketed_paste && !ignore_bracketed_paste_mode {
        let text = bounded_terminal_input(
            text,
            TERMINAL_INPUT_MAX_BYTES.saturating_sub(TERMINAL_BRACKETED_PASTE_WRAPPER_BYTES),
        );
        let mut input = String::with_capacity(
            text.len()
                + TERMINAL_BRACKETED_PASTE_PREFIX.len()
                + TERMINAL_BRACKETED_PASTE_SUFFIX.len(),
        );
        input.push_str(TERMINAL_BRACKETED_PASTE_PREFIX);
        input.push_str(&text);
        input.push_str(TERMINAL_BRACKETED_PASTE_SUFFIX);
        input
    } else {
        bounded_terminal_input(text, TERMINAL_INPUT_MAX_BYTES)
    }
}

pub(super) fn terminal_paste_has_multiple_lines(text: &str) -> bool {
    text.contains('\n') || text.contains('\r')
}

pub(super) fn terminal_paste_line_count(text: &str) -> usize {
    let mut lines = 1usize;
    let mut previous_was_cr = false;
    for ch in text.chars() {
        if ch == '\n' {
            if !previous_was_cr {
                lines = lines.saturating_add(1);
            }
            previous_was_cr = false;
        } else if ch == '\r' {
            lines = lines.saturating_add(1);
            previous_was_cr = true;
        } else {
            previous_was_cr = false;
        }
    }
    lines
}

pub(in crate::terminal) fn terminal_alt_click_cursor_input(
    cursor: TerminalCellPosition,
    target: TerminalCellPosition,
    cols: u16,
) -> Option<String> {
    if cols == 0 {
        return None;
    }

    let cols = i64::from(cols);
    let cursor_offset = i64::from(cursor.row) * cols + i64::from(cursor.col);
    let target_offset = i64::from(target.row) * cols + i64::from(target.col);
    let delta = target_offset - cursor_offset;
    if delta == 0 {
        return None;
    }

    let repeat = delta
        .unsigned_abs()
        .min(TERMINAL_CURSOR_INPUT_REPEAT_LIMIT as u64) as usize;
    if repeat == 0 {
        return None;
    }
    let arrow = if delta.is_negative() {
        "\x1b[D"
    } else {
        "\x1b[C"
    };
    Some(arrow.repeat(repeat))
}

pub(super) fn terminal_mouse_wheel_input(
    screen: &vt100::Screen,
    position: TerminalCellPosition,
    delta_rows: i32,
    modifiers: Modifiers,
) -> Option<String> {
    if screen.mouse_protocol_mode() == MouseProtocolMode::None {
        return None;
    }
    let (rows, cols) = screen.size();
    if position.row >= rows || position.col >= cols {
        return None;
    }
    let repeat = terminal_wheel_input_repeat(delta_rows)?;
    let button = terminal_mouse_wheel_button(delta_rows, modifiers)?;
    let col = u32::from(position.col).saturating_add(1);
    let row = u32::from(position.row).saturating_add(1);
    let event = match screen.mouse_protocol_encoding() {
        MouseProtocolEncoding::Default => terminal_default_mouse_input(button, col, row),
        MouseProtocolEncoding::Utf8 => terminal_utf8_mouse_input(button, col, row),
        MouseProtocolEncoding::Sgr => Some(terminal_sgr_mouse_input(button, col, row)),
    }?;
    Some(event.repeat(repeat))
}

pub(super) fn terminal_alternate_scroll_input(
    screen: &vt100::Screen,
    delta_rows: i32,
) -> Option<String> {
    if !screen.alternate_screen() || screen.mouse_protocol_mode() != MouseProtocolMode::None {
        return None;
    }
    let repeat = terminal_wheel_input_repeat(delta_rows)?;
    let input = if delta_rows.is_positive() {
        "\x1b[A"
    } else {
        "\x1b[B"
    };
    Some(input.repeat(repeat))
}

fn terminal_wheel_input_repeat(delta_rows: i32) -> Option<usize> {
    let repeat = (delta_rows.unsigned_abs() as usize).min(TERMINAL_WHEEL_INPUT_REPEAT_LIMIT);
    (repeat > 0).then_some(repeat)
}

fn terminal_mouse_wheel_button(delta_rows: i32, modifiers: Modifiers) -> Option<u16> {
    let button = if delta_rows.is_positive() {
        64
    } else if delta_rows.is_negative() {
        65
    } else {
        return None;
    };
    Some(button + terminal_mouse_modifier_bits(modifiers))
}

fn terminal_mouse_modifier_bits(modifiers: Modifiers) -> u16 {
    let mut bits = 0;
    if modifiers.shift {
        bits |= 4;
    }
    if modifiers.alt {
        bits |= 8;
    }
    if modifiers.ctrl {
        bits |= 16;
    }
    bits
}

fn terminal_default_mouse_input(button: u16, col: u32, row: u32) -> Option<String> {
    let cb = u32::from(button).checked_add(32)?;
    let cx = col.checked_add(32)?;
    let cy = row.checked_add(32)?;
    if cb > 0x7f || cx > 0x7f || cy > 0x7f {
        return None;
    }
    let mut input = String::from("\x1b[M");
    input.push(char::from_u32(cb)?);
    input.push(char::from_u32(cx)?);
    input.push(char::from_u32(cy)?);
    Some(input)
}

fn terminal_utf8_mouse_input(button: u16, col: u32, row: u32) -> Option<String> {
    let mut input = String::from("\x1b[M");
    push_terminal_utf8_mouse_code(&mut input, u32::from(button).checked_add(32)?)?;
    push_terminal_utf8_mouse_code(&mut input, col.checked_add(32)?)?;
    push_terminal_utf8_mouse_code(&mut input, row.checked_add(32)?)?;
    Some(input)
}

fn push_terminal_utf8_mouse_code(input: &mut String, code: u32) -> Option<()> {
    input.push(char::from_u32(code)?);
    Some(())
}

pub(super) fn terminal_sgr_mouse_input(button: u16, col: u32, row: u32) -> String {
    let mut input = String::with_capacity(
        "\x1b[<".len()
            + decimal_digit_count(u32::from(button))
            + 1
            + decimal_digit_count(col)
            + 1
            + decimal_digit_count(row)
            + 1,
    );
    input.push_str("\x1b[<");
    push_decimal_u32(&mut input, u32::from(button));
    input.push(';');
    push_decimal_u32(&mut input, col);
    input.push(';');
    push_decimal_u32(&mut input, row);
    input.push('M');
    input
}

fn decimal_digit_count(mut value: u32) -> usize {
    let mut digits = 1;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

fn push_decimal_u32(input: &mut String, mut value: u32) {
    let mut digits = [0_u8; 10];
    let mut len = 0;
    loop {
        digits[len] = b'0' + (value % 10) as u8;
        len += 1;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    for digit in digits[..len].iter().rev() {
        input.push(char::from(*digit));
    }
}

pub(in crate::terminal) fn terminal_zoomed_font_size(
    font_size: f32,
    wheel_delta_y: f32,
) -> Option<f32> {
    let step = if wheel_delta_y > 0.0 {
        1.0
    } else if wheel_delta_y < 0.0 {
        -1.0
    } else {
        return None;
    };
    let next = clamp_terminal_font_size(font_size + step);
    (next.to_bits() != font_size.to_bits()).then_some(next)
}

pub(super) fn terminal_selection_from_points(
    session: &TerminalSession,
    anchor: TerminalCellPosition,
    cursor: TerminalCellPosition,
) -> Option<TerminalTextSelection> {
    let screen = session.parser.screen();
    let (_, cols) = screen.size();
    let range = normalized_terminal_selection_range(anchor, cursor, cols);
    let text = terminal_text_for_range(screen, range);
    if text.is_empty() {
        return None;
    }
    Some(TerminalTextSelection {
        session_id: session.id,
        text,
        range,
    })
}

fn normalized_terminal_selection_range(
    anchor: TerminalCellPosition,
    cursor: TerminalCellPosition,
    cols: u16,
) -> TerminalSelectionRange {
    let (start, end_inclusive) = if (cursor.row, cursor.col) < (anchor.row, anchor.col) {
        (cursor, anchor)
    } else {
        (anchor, cursor)
    };
    TerminalSelectionRange {
        start,
        end: TerminalCellPosition {
            row: end_inclusive.row,
            col: end_inclusive.col.saturating_add(1).min(cols),
        },
    }
}

fn terminal_text_for_range(screen: &vt100::Screen, range: TerminalSelectionRange) -> String {
    let (rows, cols) = screen.size();
    if rows == 0 || cols == 0 {
        return String::new();
    }

    let start_row = range.start.row.min(rows.saturating_sub(1));
    let end_row = range.end.row.min(rows.saturating_sub(1));
    let selected_rows = usize::from(end_row.saturating_sub(start_row).saturating_add(1));
    let mut text = String::with_capacity(selected_rows.saturating_mul(usize::from(cols)));
    let mut line = String::with_capacity(usize::from(cols));
    let mut emitted_lines = 0usize;
    let mut pending_empty_lines = 0usize;
    for row in start_row..=end_row {
        line.clear();
        let start_col = if row == range.start.row {
            range.start.col.min(cols)
        } else {
            0
        };
        let end_col = if row == range.end.row {
            range.end.col.min(cols)
        } else {
            cols
        };
        if end_col <= start_col {
            pending_empty_lines += 1;
            continue;
        }
        line.extend((start_col..end_col).filter_map(|col| terminal_cell_char(screen, row, col)));
        let trimmed_len = line.trim_end().len();
        line.truncate(trimmed_len);
        if line.is_empty() {
            pending_empty_lines += 1;
            continue;
        }
        flush_pending_terminal_empty_lines(&mut text, &mut emitted_lines, &mut pending_empty_lines);
        push_terminal_text_line(&mut text, &mut emitted_lines, &line);
    }
    text
}

fn terminal_cell_char(screen: &vt100::Screen, row: u16, col: u16) -> Option<char> {
    let cell = screen.cell(row, col)?;
    (!cell.is_wide_continuation())
        .then(|| cell.contents().chars().next())
        .flatten()
}

pub(super) fn trimmed_terminal_text(contents: &str) -> String {
    let mut text = String::new();
    let mut emitted_lines = 0usize;
    let mut pending_empty_lines = 0usize;
    for line in contents.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            pending_empty_lines += 1;
            continue;
        }
        flush_pending_terminal_empty_lines(&mut text, &mut emitted_lines, &mut pending_empty_lines);
        push_terminal_text_line(&mut text, &mut emitted_lines, line);
    }
    text
}

fn flush_pending_terminal_empty_lines(
    text: &mut String,
    emitted_lines: &mut usize,
    pending_empty_lines: &mut usize,
) {
    while *pending_empty_lines > 0 {
        if *emitted_lines > 0 {
            text.push('\n');
        }
        *emitted_lines += 1;
        *pending_empty_lines -= 1;
    }
}

fn push_terminal_text_line(text: &mut String, emitted_lines: &mut usize, line: &str) {
    if *emitted_lines > 0 {
        text.push('\n');
    }
    text.push_str(line);
    *emitted_lines += 1;
}
