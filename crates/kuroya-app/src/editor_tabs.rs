use std::borrow::Cow;

use crate::path_display::{DISPLAY_PATH_LABEL_MAX_CHARS, sanitized_display_label_cow};

pub(crate) fn buffer_tab_display_name(name: &str) -> String {
    buffer_tab_display_name_cow(name, DISPLAY_PATH_LABEL_MAX_CHARS).into_owned()
}

pub(crate) fn buffer_tab_label_from_display_name(
    display_name: &str,
    dirty: bool,
    changed_on_disk: bool,
    read_only: bool,
) -> String {
    let prefix = buffer_tab_status_prefix(dirty, changed_on_disk, read_only);
    let display_name = buffer_tab_display_name_with_prefix_budget(display_name, prefix);

    let mut label = String::with_capacity(prefix.len() + display_name.len());
    label.push_str(prefix);
    label.push_str(display_name.as_ref());
    label
}

pub(crate) fn buffer_tab_close_tooltip(name: &str) -> String {
    const PREFIX: &str = "Close ";

    let name = buffer_tab_display_name_cow(
        name,
        DISPLAY_PATH_LABEL_MAX_CHARS.saturating_sub(PREFIX.len()),
    );
    let mut tooltip = String::with_capacity(PREFIX.len() + name.len());
    tooltip.push_str(PREFIX);
    tooltip.push_str(name.as_ref());
    tooltip
}

fn buffer_tab_status_prefix(dirty: bool, changed_on_disk: bool, read_only: bool) -> &'static str {
    match (changed_on_disk, dirty, read_only) {
        (false, false, false) => "",
        (true, false, false) => "! ",
        (false, true, false) => "* ",
        (false, false, true) => "RO ",
        (true, true, false) => "! * ",
        (true, false, true) => "! RO ",
        (false, true, true) => "* RO ",
        (true, true, true) => "! * RO ",
    }
}

fn buffer_tab_display_name_cow(name: &str, max_chars: usize) -> Cow<'_, str> {
    sanitized_display_label_cow(name, max_chars, "Untitled")
}

fn buffer_tab_display_name_with_prefix_budget<'a>(
    display_name: &'a str,
    prefix: &str,
) -> Cow<'a, str> {
    let max_chars = DISPLAY_PATH_LABEL_MAX_CHARS.saturating_sub(prefix.len());
    if buffer_tab_display_name_is_clean_and_bounded(display_name, max_chars) {
        Cow::Borrowed(display_name)
    } else {
        buffer_tab_display_name_cow(display_name, max_chars)
    }
}

fn buffer_tab_display_name_is_clean_and_bounded(display_name: &str, max_chars: usize) -> bool {
    if display_name.is_empty() || max_chars == 0 {
        return false;
    }
    if display_name.is_ascii() {
        let bytes = display_name.as_bytes();
        return bytes.len() <= max_chars
            && !bytes.first().is_some_and(u8::is_ascii_whitespace)
            && !bytes.last().is_some_and(u8::is_ascii_whitespace)
            && bytes.iter().all(|byte| *byte >= b' ' && *byte != b'\x7f');
    }

    let mut chars = 0usize;
    let mut first = None;
    let mut last = None;
    for ch in display_name.chars() {
        if ch.is_control()
            || matches!(
                ch,
                '\u{00ad}'
                    | '\u{061c}'
                    | '\u{180e}'
                    | '\u{200b}'..='\u{200f}'
                    | '\u{2028}'..='\u{202e}'
                    | '\u{2060}'..='\u{206f}'
                    | '\u{feff}'
            )
        {
            return false;
        }
        first.get_or_insert(ch);
        last = Some(ch);
        chars += 1;
        if chars > max_chars {
            return false;
        }
    }

    !first.is_some_and(char::is_whitespace) && !last.is_some_and(char::is_whitespace)
}

#[cfg(test)]
pub(crate) fn buffer_tab_label(
    name: &str,
    dirty: bool,
    changed_on_disk: bool,
    read_only: bool,
) -> String {
    let prefix = buffer_tab_status_prefix(dirty, changed_on_disk, read_only);
    let display_name = buffer_tab_display_name_with_prefix_budget(name, prefix);

    let mut label = String::with_capacity(prefix.len() + display_name.len());
    label.push_str(prefix);
    label.push_str(display_name.as_ref());
    label
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::{
        buffer_tab_close_tooltip, buffer_tab_display_name, buffer_tab_display_name_cow,
        buffer_tab_display_name_is_clean_and_bounded, buffer_tab_label,
        buffer_tab_label_from_display_name,
    };
    use crate::path_display::DISPLAY_PATH_LABEL_MAX_CHARS;

    #[test]
    fn buffer_tab_display_name_cow_borrows_clean_ascii_and_unicode_names() {
        assert!(matches!(
            buffer_tab_display_name_cow("clean.rs", DISPLAY_PATH_LABEL_MAX_CHARS),
            Cow::Borrowed("clean.rs")
        ));

        let unicode = "clean-\u{03bb}.rs";
        match buffer_tab_display_name_cow(unicode, DISPLAY_PATH_LABEL_MAX_CHARS) {
            Cow::Borrowed(label) => assert_eq!(label, unicode),
            Cow::Owned(label) => panic!("expected borrowed label, got {label:?}"),
        }
    }

    #[test]
    fn buffer_tab_display_name_cow_owns_dirty_truncated_and_fallback_labels() {
        let long = format!("main-{}.rs", "x".repeat(DISPLAY_PATH_LABEL_MAX_CHARS * 2));
        let names = [
            "  clean.rs  ",
            "bad\nname\u{202e}",
            long.as_str(),
            "\n\u{202e}",
        ];

        for name in names {
            let label = buffer_tab_display_name_cow(name, DISPLAY_PATH_LABEL_MAX_CHARS);

            assert_eq!(label.as_ref(), buffer_tab_display_name(name));
            assert!(
                matches!(&label, Cow::Owned(_)),
                "expected owned label for {name:?}"
            );
        }
    }

    #[test]
    fn buffer_tab_string_wrappers_match_cow_helpers() {
        let names = ["clean.rs", "bad\nname\u{202e}", "\n\u{202e}"];
        for name in names {
            assert_eq!(
                buffer_tab_display_name(name),
                buffer_tab_display_name_cow(name, DISPLAY_PATH_LABEL_MAX_CHARS).into_owned()
            );
        }

        let label = buffer_tab_label_from_display_name("main.rs", true, false, true);
        assert_eq!(label, buffer_tab_label("main.rs", true, false, true));

        let tooltip = buffer_tab_close_tooltip("main.rs");
        assert_eq!(tooltip, "Close main.rs");
    }

    #[test]
    fn buffer_tab_display_name_budget_fast_path_accepts_only_clean_bounded_names() {
        assert!(buffer_tab_display_name_is_clean_and_bounded("main.rs", 12));
        assert!(buffer_tab_display_name_is_clean_and_bounded(
            "clean-\u{03bb}.rs",
            16
        ));

        for name in [
            "",
            " main.rs",
            "main.rs ",
            "bad\nname",
            "bad\u{202e}name",
            "clean-\u{03bb}.rs",
        ] {
            assert!(!buffer_tab_display_name_is_clean_and_bounded(name, 6));
        }
    }

    #[test]
    fn buffer_tab_label_sanitizes_and_bounds_display_name() {
        let label = buffer_tab_label(
            &format!(
                "bad\nname\u{202e}{}",
                "x".repeat(DISPLAY_PATH_LABEL_MAX_CHARS * 2)
            ),
            false,
            false,
            false,
        );

        assert!(!label.contains('\n'));
        assert!(!label.contains('\u{202e}'));
        assert!(label.contains("..."));
        assert!(label.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);
    }

    #[test]
    fn buffer_tab_label_preserves_markers_while_bounding_name() {
        let label = buffer_tab_label(
            &format!("main-{}.rs", "x".repeat(DISPLAY_PATH_LABEL_MAX_CHARS * 2)),
            true,
            true,
            true,
        );

        assert!(label.starts_with("! * RO "));
        let display_name = label.trim_start_matches("! * RO ");
        assert!(display_name.contains("..."));
        assert!(display_name.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);
        assert!(label.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);
    }

    #[test]
    fn buffer_tab_display_name_falls_back_for_control_only_names() {
        assert_eq!(buffer_tab_display_name("\n\u{202e}"), "Untitled");
    }

    #[test]
    fn buffer_tab_close_tooltip_sanitizes_and_bounds_name() {
        let tooltip = buffer_tab_close_tooltip(&format!(
            "bad\nname\u{202e}{}",
            "x".repeat(DISPLAY_PATH_LABEL_MAX_CHARS * 2)
        ));

        assert!(tooltip.starts_with("Close "));
        assert!(!tooltip.contains('\n'));
        assert!(!tooltip.contains('\u{202e}'));
        assert!(tooltip.contains("..."));
        assert!(tooltip.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);
    }
}
