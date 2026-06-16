use eframe::egui::{Key, Modifiers, Sense};

pub(crate) fn editor_click_drag_sense_for_tab_index(tab_index: i64) -> Sense {
    if tab_index < 0 {
        Sense::CLICK | Sense::DRAG
    } else {
        Sense::click_and_drag()
    }
}

pub(crate) fn editor_tab_focus_mode_should_release_focus(
    tab_focus_mode: bool,
    key: Key,
    modifiers: Modifiers,
) -> bool {
    tab_focus_mode && key == Key::Tab && !modifiers.ctrl && !modifiers.command && !modifiers.alt
}

#[cfg(test)]
mod tests {
    use super::{
        editor_click_drag_sense_for_tab_index, editor_tab_focus_mode_should_release_focus,
    };
    use eframe::egui::{Key, Modifiers};

    #[test]
    fn editor_tab_index_controls_keyboard_focusability() {
        let skipped = editor_click_drag_sense_for_tab_index(-1);
        assert!(skipped.interactive());
        assert!(!skipped.is_focusable());

        let focusable = editor_click_drag_sense_for_tab_index(0);
        assert!(focusable.interactive());
        assert!(focusable.is_focusable());

        let ordered = editor_click_drag_sense_for_tab_index(2);
        assert!(ordered.interactive());
        assert!(ordered.is_focusable());
    }

    #[test]
    fn editor_tab_focus_mode_releases_only_plain_or_shift_tab() {
        assert!(editor_tab_focus_mode_should_release_focus(
            true,
            Key::Tab,
            Modifiers::NONE
        ));
        assert!(editor_tab_focus_mode_should_release_focus(
            true,
            Key::Tab,
            Modifiers {
                shift: true,
                ..Modifiers::NONE
            }
        ));
        assert!(!editor_tab_focus_mode_should_release_focus(
            false,
            Key::Tab,
            Modifiers::NONE
        ));
        assert!(!editor_tab_focus_mode_should_release_focus(
            true,
            Key::Enter,
            Modifiers::NONE
        ));
        assert!(!editor_tab_focus_mode_should_release_focus(
            true,
            Key::Tab,
            Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            }
        ));
    }
}
