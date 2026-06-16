const OPTIONAL_SETTING_PATH_MAX_CHARS: usize = 1024;

pub(crate) fn optional_setting_path_from_input(input: &str) -> Option<String> {
    valid_optional_setting_path(input).map(str::to_owned)
}

pub(crate) fn optional_setting_path_to_input(value: &Option<String>) -> String {
    value
        .as_deref()
        .and_then(valid_optional_setting_path)
        .map(str::to_owned)
        .unwrap_or_default()
}

pub(crate) fn optional_setting_path_input_matches(input: &str, value: &Option<String>) -> bool {
    match value.as_deref().and_then(valid_optional_setting_path) {
        Some(path) => input == path,
        None => input.is_empty(),
    }
}

fn valid_optional_setting_path(input: &str) -> Option<&str> {
    let trimmed = input.trim();
    if trimmed.is_empty()
        || trimmed
            .chars()
            .nth(OPTIONAL_SETTING_PATH_MAX_CHARS)
            .is_some()
        || contains_path_hidden_or_control_char(trimmed)
    {
        None
    } else {
        Some(trimmed)
    }
}

fn contains_path_hidden_or_control_char(value: &str) -> bool {
    if value.is_ascii() {
        value.bytes().any(|byte| byte.is_ascii_control())
    } else {
        value.chars().any(|ch| {
            ch.is_control() || matches!(ch, '\u{2028}' | '\u{2029}') || is_hidden_format_control(ch)
        })
    }
}

fn is_hidden_format_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{061C}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2066}'..='\u{2069}'
            | '\u{FEFF}'
    )
}

#[cfg(test)]
mod tests {
    use super::{
        OPTIONAL_SETTING_PATH_MAX_CHARS, optional_setting_path_from_input,
        optional_setting_path_input_matches, optional_setting_path_to_input,
    };

    #[test]
    fn optional_setting_path_rejects_embedded_control_characters() {
        assert_eq!(optional_setting_path_from_input("fonts/Inter.ttf\0"), None);
        assert_eq!(
            optional_setting_path_from_input("fonts/Inter.ttf\nfonts/Other.ttf"),
            None
        );
        assert_eq!(
            optional_setting_path_to_input(&Some("fonts/\tbad.ttf".to_owned())),
            ""
        );
    }

    #[test]
    fn optional_setting_path_rejects_hidden_format_controls_and_huge_paths() {
        assert_eq!(
            optional_setting_path_from_input("fonts/\u{202e}Inter.ttf"),
            None
        );
        assert_eq!(
            optional_setting_path_from_input("fonts/Inter\u{2028}.ttf"),
            None
        );
        assert_eq!(
            optional_setting_path_to_input(&Some("fonts/Inter\u{200f}.ttf".to_owned())),
            ""
        );
        assert_eq!(
            optional_setting_path_to_input(&Some("fonts/\u{feff}Inter.ttf".to_owned())),
            ""
        );

        let huge = format!("{}{}", "a".repeat(OPTIONAL_SETTING_PATH_MAX_CHARS), ".ttf");
        assert_eq!(optional_setting_path_from_input(&huge), None);
    }

    #[test]
    fn optional_setting_path_trims_stored_values_for_panel_input() {
        assert_eq!(
            optional_setting_path_to_input(&Some(" fonts/Inter-Regular.ttf ".to_owned())),
            "fonts/Inter-Regular.ttf"
        );
    }

    #[test]
    fn optional_setting_path_input_match_uses_normalized_stored_value() {
        assert!(optional_setting_path_input_matches(
            "fonts/Inter-Regular.ttf",
            &Some(" fonts/Inter-Regular.ttf ".to_owned())
        ));
        assert!(optional_setting_path_input_matches(
            "",
            &Some("fonts/\tbad.ttf".to_owned())
        ));
        assert!(!optional_setting_path_input_matches(
            " fonts/Inter-Regular.ttf ",
            &Some("fonts/Inter-Regular.ttf".to_owned())
        ));
    }
}
