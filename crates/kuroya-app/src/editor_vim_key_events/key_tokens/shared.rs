use eframe::egui::Key;

pub(super) fn vim_ctrl_key_char(key: Key) -> Option<char> {
    match key {
        Key::B => Some('b'),
        Key::D => Some('d'),
        Key::E => Some('e'),
        Key::F => Some('f'),
        Key::N => Some('n'),
        Key::P => Some('p'),
        Key::R => Some('r'),
        Key::U => Some('u'),
        Key::Y => Some('y'),
        _ => None,
    }
}

pub(super) fn vim_letter_key(ch: char) -> Key {
    match ch {
        'a' => Key::A,
        'b' => Key::B,
        'c' => Key::C,
        'd' => Key::D,
        'e' => Key::E,
        'f' => Key::F,
        'g' => Key::G,
        'h' => Key::H,
        'i' => Key::I,
        'j' => Key::J,
        'k' => Key::K,
        'l' => Key::L,
        'm' => Key::M,
        'n' => Key::N,
        'o' => Key::O,
        'p' => Key::P,
        'q' => Key::Q,
        'r' => Key::R,
        's' => Key::S,
        't' => Key::T,
        'u' => Key::U,
        'v' => Key::V,
        'w' => Key::W,
        'x' => Key::X,
        'y' => Key::Y,
        'z' => Key::Z,
        _ => Key::A,
    }
}

pub(super) fn matches_ignore_ascii_case(value: &str, candidates: &[&str]) -> bool {
    candidates
        .iter()
        .any(|candidate| value.eq_ignore_ascii_case(candidate))
}
