use crate::terminal_support::terminal_cell_size;
use egui::{Key, Modifiers, MouseWheelUnit, Vec2};

pub(super) fn wheel_scroll_rows(
    unit: MouseWheelUnit,
    delta: Vec2,
    viewport_height: f32,
    font_size: f32,
    line_height: f32,
    letter_spacing: f32,
    sensitivity: f32,
) -> i32 {
    let (_, cell_height) = terminal_cell_size(font_size, line_height, letter_spacing);
    if !delta.y.is_finite() || !cell_height.is_finite() || cell_height <= 0.0 {
        return 0;
    }
    if !sensitivity.is_finite() || sensitivity <= 0.0 {
        return 0;
    }

    let rows = match unit {
        MouseWheelUnit::Point => delta.y / cell_height,
        MouseWheelUnit::Line => delta.y,
        MouseWheelUnit::Page => {
            if !viewport_height.is_finite() || viewport_height <= 0.0 {
                return 0;
            }
            delta.y * (viewport_height / cell_height)
        }
    } * sensitivity;
    if !rows.is_finite() {
        return 0;
    }

    let rounded = rows.round() as i32;
    if rounded == 0 && rows != 0.0 {
        rows.signum() as i32
    } else {
        rounded
    }
}

const TERMINAL_PAGE_SCROLL_ROWS: i32 = 12;
const TERMINAL_SCROLL_TOP_DELTA: i32 = i32::MAX / 4;
const TERMINAL_SCROLL_BOTTOM_DELTA: i32 = i32::MIN / 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TerminalKeyClassification {
    scroll_delta: Option<i32>,
    input: Option<&'static str>,
    shift_input: Option<&'static str>,
    ctrl_input: Option<&'static str>,
}

impl TerminalKeyClassification {
    const NONE: Self = Self {
        scroll_delta: None,
        input: None,
        shift_input: None,
        ctrl_input: None,
    };

    const fn input(input: &'static str) -> Self {
        Self {
            scroll_delta: None,
            input: Some(input),
            shift_input: None,
            ctrl_input: None,
        }
    }

    const fn input_with_shift(input: &'static str, shift_input: &'static str) -> Self {
        Self {
            scroll_delta: None,
            input: Some(input),
            shift_input: Some(shift_input),
            ctrl_input: None,
        }
    }

    const fn ctrl_input(ctrl_input: &'static str) -> Self {
        Self {
            scroll_delta: None,
            input: None,
            shift_input: None,
            ctrl_input: Some(ctrl_input),
        }
    }

    const fn scroll_input(scroll_delta: i32, input: &'static str) -> Self {
        Self {
            scroll_delta: Some(scroll_delta),
            input: Some(input),
            shift_input: None,
            ctrl_input: None,
        }
    }
}

#[inline]
pub(super) fn terminal_scroll_key_delta(key: Key, modifiers: Modifiers) -> Option<i32> {
    if !modifiers.shift {
        return None;
    }

    terminal_key_classification(key).scroll_delta
}

#[inline]
pub(crate) fn terminal_key_input(key: Key, modifiers: Modifiers) -> Option<&'static str> {
    let classification = terminal_key_classification(key);
    if modifiers.ctrl {
        return classification.ctrl_input;
    }

    if modifiers.shift {
        if let Some(input) = classification.shift_input {
            return Some(input);
        }
    }

    classification.input
}

#[inline]
pub(super) fn terminal_copy_shortcut(key: Key, modifiers: Modifiers) -> bool {
    key == Key::C && modifiers.ctrl && modifiers.shift
}

#[inline]
fn terminal_key_classification(key: Key) -> TerminalKeyClassification {
    match key {
        Key::Enter => TerminalKeyClassification::input("\r"),
        Key::Backspace => TerminalKeyClassification::input("\x08"),
        Key::Insert => TerminalKeyClassification::input("\x1b[2~"),
        Key::Delete => TerminalKeyClassification::input("\x1b[3~"),
        Key::Tab => TerminalKeyClassification::input_with_shift("\t", "\x1b[Z"),
        Key::ArrowUp => TerminalKeyClassification::input("\x1b[A"),
        Key::ArrowDown => TerminalKeyClassification::input("\x1b[B"),
        Key::ArrowRight => TerminalKeyClassification::input("\x1b[C"),
        Key::ArrowLeft => TerminalKeyClassification::input("\x1b[D"),
        Key::Home => TerminalKeyClassification::scroll_input(TERMINAL_SCROLL_TOP_DELTA, "\x1b[H"),
        Key::End => TerminalKeyClassification::scroll_input(TERMINAL_SCROLL_BOTTOM_DELTA, "\x1b[F"),
        Key::PageUp => {
            TerminalKeyClassification::scroll_input(TERMINAL_PAGE_SCROLL_ROWS, "\x1b[5~")
        }
        Key::PageDown => {
            TerminalKeyClassification::scroll_input(-TERMINAL_PAGE_SCROLL_ROWS, "\x1b[6~")
        }
        Key::Escape => TerminalKeyClassification::input("\x1b"),
        Key::Space => TerminalKeyClassification::ctrl_input("\0"),
        Key::OpenBracket => TerminalKeyClassification::ctrl_input("\x1b"),
        Key::Backslash | Key::Pipe => TerminalKeyClassification::ctrl_input("\x1c"),
        Key::CloseBracket => TerminalKeyClassification::ctrl_input("\x1d"),
        Key::Minus => TerminalKeyClassification::ctrl_input("\x1f"),
        Key::A => TerminalKeyClassification::ctrl_input("\x01"),
        Key::B => TerminalKeyClassification::ctrl_input("\x02"),
        Key::C => TerminalKeyClassification::ctrl_input("\x03"),
        Key::D => TerminalKeyClassification::ctrl_input("\x04"),
        Key::E => TerminalKeyClassification::ctrl_input("\x05"),
        Key::F => TerminalKeyClassification::ctrl_input("\x06"),
        Key::H => TerminalKeyClassification::ctrl_input("\x08"),
        Key::J => TerminalKeyClassification::ctrl_input("\n"),
        Key::K => TerminalKeyClassification::ctrl_input("\x0b"),
        Key::L => TerminalKeyClassification::ctrl_input("\x0c"),
        Key::M => TerminalKeyClassification::ctrl_input("\r"),
        Key::N => TerminalKeyClassification::ctrl_input("\x0e"),
        Key::P => TerminalKeyClassification::ctrl_input("\x10"),
        Key::U => TerminalKeyClassification::ctrl_input("\x15"),
        Key::W => TerminalKeyClassification::ctrl_input("\x17"),
        Key::Y => TerminalKeyClassification::ctrl_input("\x19"),
        Key::Z => TerminalKeyClassification::ctrl_input("\x1a"),
        Key::F1 => TerminalKeyClassification::input("\x1bOP"),
        Key::F2 => TerminalKeyClassification::input("\x1bOQ"),
        Key::F3 => TerminalKeyClassification::input("\x1bOR"),
        Key::F4 => TerminalKeyClassification::input("\x1bOS"),
        Key::F5 => TerminalKeyClassification::input("\x1b[15~"),
        Key::F6 => TerminalKeyClassification::input("\x1b[17~"),
        Key::F7 => TerminalKeyClassification::input("\x1b[18~"),
        Key::F8 => TerminalKeyClassification::input("\x1b[19~"),
        Key::F9 => TerminalKeyClassification::input("\x1b[20~"),
        Key::F10 => TerminalKeyClassification::input("\x1b[21~"),
        Key::F11 => TerminalKeyClassification::input("\x1b[23~"),
        Key::F12 => TerminalKeyClassification::input("\x1b[24~"),
        _ => TerminalKeyClassification::NONE,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        terminal_copy_shortcut, terminal_key_input, terminal_scroll_key_delta, wheel_scroll_rows,
    };
    use egui::{Key, Modifiers, MouseWheelUnit, vec2};

    #[test]
    fn wheel_scroll_rows_applies_scroll_sensitivity() {
        assert_eq!(
            wheel_scroll_rows(
                MouseWheelUnit::Line,
                vec2(0.0, 2.0),
                120.0,
                12.0,
                1.0,
                0.0,
                3.0
            ),
            6
        );
    }

    #[test]
    fn wheel_scroll_rows_preserves_tiny_scroll_direction_after_sensitivity() {
        assert_eq!(
            wheel_scroll_rows(
                MouseWheelUnit::Line,
                vec2(0.0, 0.1),
                120.0,
                12.0,
                1.0,
                0.0,
                0.5
            ),
            1
        );
    }

    #[test]
    fn wheel_scroll_rows_ignores_invalid_metrics_and_sensitivity() {
        assert_eq!(
            wheel_scroll_rows(
                MouseWheelUnit::Point,
                vec2(0.0, 24.0),
                120.0,
                12.0,
                0.0,
                0.0,
                1.0
            ),
            0
        );
        assert_eq!(
            wheel_scroll_rows(
                MouseWheelUnit::Line,
                vec2(0.0, 2.0),
                120.0,
                12.0,
                1.0,
                0.0,
                f32::INFINITY
            ),
            0
        );
        assert_eq!(
            wheel_scroll_rows(
                MouseWheelUnit::Line,
                vec2(0.0, 2.0),
                120.0,
                12.0,
                1.0,
                0.0,
                -1.0
            ),
            0
        );
    }

    #[test]
    fn wheel_scroll_rows_ignores_non_finite_delta_and_page_height() {
        assert_eq!(
            wheel_scroll_rows(
                MouseWheelUnit::Line,
                vec2(0.0, f32::NAN),
                120.0,
                12.0,
                1.0,
                0.0,
                1.0
            ),
            0
        );
        assert_eq!(
            wheel_scroll_rows(
                MouseWheelUnit::Page,
                vec2(0.0, 1.0),
                f32::NAN,
                12.0,
                1.0,
                0.0,
                1.0
            ),
            0
        );
    }

    #[test]
    fn terminal_ctrl_key_input_covers_common_control_characters() {
        let ctrl = Modifiers::CTRL;

        assert_eq!(terminal_key_input(Key::Space, ctrl), Some("\0"));
        assert_eq!(terminal_key_input(Key::OpenBracket, ctrl), Some("\x1b"));
        assert_eq!(terminal_key_input(Key::Backslash, ctrl), Some("\x1c"));
        assert_eq!(terminal_key_input(Key::Pipe, ctrl), Some("\x1c"));
        assert_eq!(terminal_key_input(Key::CloseBracket, ctrl), Some("\x1d"));
        assert_eq!(terminal_key_input(Key::Minus, ctrl), Some("\x1f"));
    }

    #[test]
    fn terminal_ctrl_shift_c_is_copy_shortcut() {
        let ctrl_shift = Modifiers::CTRL.plus(Modifiers::SHIFT);

        assert!(terminal_copy_shortcut(Key::C, ctrl_shift));
        assert!(!terminal_copy_shortcut(Key::C, Modifiers::CTRL));
        assert!(!terminal_copy_shortcut(Key::V, ctrl_shift));
    }

    #[test]
    fn terminal_shift_tab_sends_backtab_sequence() {
        assert_eq!(terminal_key_input(Key::Tab, Modifiers::NONE), Some("\t"));
        assert_eq!(
            terminal_key_input(Key::Tab, Modifiers::SHIFT),
            Some("\x1b[Z")
        );
    }

    #[test]
    fn terminal_insert_and_function_keys_send_standard_sequences() {
        let expected = [
            (Key::Insert, "\x1b[2~"),
            (Key::F1, "\x1bOP"),
            (Key::F2, "\x1bOQ"),
            (Key::F3, "\x1bOR"),
            (Key::F4, "\x1bOS"),
            (Key::F5, "\x1b[15~"),
            (Key::F6, "\x1b[17~"),
            (Key::F7, "\x1b[18~"),
            (Key::F8, "\x1b[19~"),
            (Key::F9, "\x1b[20~"),
            (Key::F10, "\x1b[21~"),
            (Key::F11, "\x1b[23~"),
            (Key::F12, "\x1b[24~"),
        ];

        for (key, input) in expected {
            assert_eq!(terminal_key_input(key, Modifiers::NONE), Some(input));
        }
    }

    #[test]
    fn terminal_scroll_keys_preserve_direct_pty_mappings() {
        let shift = Modifiers::SHIFT;

        assert_eq!(terminal_scroll_key_delta(Key::PageUp, shift), Some(12));
        assert_eq!(terminal_scroll_key_delta(Key::PageDown, shift), Some(-12));
        assert_eq!(
            terminal_scroll_key_delta(Key::Home, shift),
            Some(i32::MAX / 4)
        );
        assert_eq!(
            terminal_scroll_key_delta(Key::End, shift),
            Some(i32::MIN / 4)
        );

        assert_eq!(terminal_key_input(Key::PageUp, shift), Some("\x1b[5~"));
        assert_eq!(terminal_key_input(Key::PageDown, shift), Some("\x1b[6~"));
        assert_eq!(terminal_key_input(Key::Home, shift), Some("\x1b[H"));
        assert_eq!(terminal_key_input(Key::End, shift), Some("\x1b[F"));
    }

    #[test]
    fn terminal_non_scroll_shift_keys_still_send_input() {
        let shift = Modifiers::SHIFT;

        assert_eq!(terminal_scroll_key_delta(Key::ArrowUp, shift), None);
        assert_eq!(terminal_key_input(Key::ArrowUp, shift), Some("\x1b[A"));
    }

    #[test]
    fn terminal_ctrl_input_does_not_fallback_to_navigation_sequences() {
        let ctrl = Modifiers::CTRL;
        let ctrl_shift = Modifiers::CTRL.plus(Modifiers::SHIFT);

        assert_eq!(terminal_key_input(Key::Enter, ctrl), None);
        assert_eq!(terminal_key_input(Key::PageUp, ctrl), None);
        assert_eq!(terminal_key_input(Key::PageUp, ctrl_shift), None);
        assert_eq!(terminal_scroll_key_delta(Key::PageUp, ctrl_shift), Some(12));
    }
}
