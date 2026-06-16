use std::borrow::Cow;

use crate::path_display::sanitized_display_label_cow;
use serde::{Deserialize, Serialize};

const PANEL_STATUS_LABEL_MAX_CHARS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PanelDockSide {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum PanelPlacement {
    #[default]
    DockedRight,
    Floating,
    DockedLeft,
}

impl<'de> Deserialize<'de> for PanelPlacement {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let placement = match Option::<String>::deserialize(deserializer)?.as_deref() {
            Some("dockedLeft") => Self::DockedLeft,
            Some("dockedRight") => Self::DockedRight,
            Some("floating") => Self::Floating,
            _ => Self::default(),
        };
        Ok(placement)
    }
}

impl PanelPlacement {
    pub(crate) fn cycle(self) -> Self {
        match self {
            Self::DockedRight => Self::Floating,
            Self::Floating => Self::DockedLeft,
            Self::DockedLeft => Self::DockedRight,
        }
    }

    pub(crate) fn dock_side(self) -> Option<PanelDockSide> {
        match self {
            Self::DockedRight => Some(PanelDockSide::Right),
            Self::DockedLeft => Some(PanelDockSide::Left),
            Self::Floating => None,
        }
    }

    pub(crate) fn is_floating(self) -> bool {
        matches!(self, Self::Floating)
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::DockedRight => "right dock",
            Self::Floating => "floating window",
            Self::DockedLeft => "left dock",
        }
    }
}

pub(crate) fn cycle_panel_placement(
    open: &mut bool,
    placement: &mut PanelPlacement,
    panel_name: &str,
) -> String {
    *open = true;
    *placement = placement.cycle();
    let panel_name = panel_status_label_cow(panel_name);
    let placement_label = placement.label();
    let mut status =
        String::with_capacity(panel_name.len() + " panel moved to ".len() + placement_label.len());
    status.push_str(panel_name.as_ref());
    status.push_str(" panel moved to ");
    status.push_str(placement_label);
    status
}

#[cfg(test)]
fn panel_status_label(label: &str) -> String {
    panel_status_label_cow(label).into_owned()
}

fn panel_status_label_cow(label: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(label, PANEL_STATUS_LABEL_MAX_CHARS, "Unknown")
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::{
        PANEL_STATUS_LABEL_MAX_CHARS, PanelPlacement, cycle_panel_placement, panel_status_label,
        panel_status_label_cow,
    };

    #[test]
    fn cycle_panel_placement_sanitizes_and_bounds_status_label() {
        let mut open = false;
        let mut placement = PanelPlacement::DockedRight;
        let status = cycle_panel_placement(
            &mut open,
            &mut placement,
            &format!("Panel\n{}\u{202e}", "very-long-name-".repeat(16)),
        );

        assert!(open);
        assert_eq!(placement, PanelPlacement::Floating);
        assert!(!status.contains('\n'));
        assert!(!status.contains('\u{202e}'));
        assert!(status.contains("..."));
        assert!(
            status
                .trim_end_matches(" panel moved to floating window")
                .chars()
                .count()
                <= PANEL_STATUS_LABEL_MAX_CHARS
        );
    }

    #[test]
    fn cycle_panel_placement_uses_fallback_for_blank_control_label() {
        let mut open = false;
        let mut placement = PanelPlacement::DockedLeft;

        assert_eq!(
            cycle_panel_placement(&mut open, &mut placement, "\n\u{202e}"),
            "Unknown panel moved to right dock"
        );
    }

    #[test]
    fn panel_status_label_cow_borrows_clean_ascii_and_unicode() {
        assert_cow_borrows_original(panel_status_label_cow("Search"), "Search");

        let unicode = "Panel-\u{03bb}";
        assert_cow_borrows_original(panel_status_label_cow(unicode), unicode);
    }

    #[test]
    fn panel_status_label_cow_owns_dirty_truncated_and_fallback_output() {
        assert_cow_owned_eq(panel_status_label_cow("Panel\n\u{202e}Name"), "Panel Name");

        let long = format!(
            "Panel-{}-finish",
            "segment-".repeat(PANEL_STATUS_LABEL_MAX_CHARS)
        );
        match panel_status_label_cow(&long) {
            Cow::Owned(label) => {
                assert!(label.starts_with("Panel-"), "{label:?}");
                assert!(label.contains("..."), "{label:?}");
                assert!(label.ends_with("-finish"), "{label:?}");
                assert_eq!(label.chars().count(), PANEL_STATUS_LABEL_MAX_CHARS);
            }
            Cow::Borrowed(label) => panic!("expected owned label, got borrowed {label:?}"),
        }

        assert_cow_owned_eq(panel_status_label_cow("\n\u{202e}\u{0007}"), "Unknown");
    }

    #[test]
    fn panel_status_label_wrapper_and_cycle_status_match_cow_helper() {
        let long = format!(
            "Panel-{}-finish",
            "segment-".repeat(PANEL_STATUS_LABEL_MAX_CHARS)
        );
        for value in [
            "Search",
            "Panel-\u{03bb}",
            "Panel\n\u{202e}Name",
            long.as_str(),
            "\n\u{202e}\u{0007}",
        ] {
            assert_eq!(
                panel_status_label(value),
                panel_status_label_cow(value).into_owned()
            );
        }

        let mut open = false;
        let mut placement = PanelPlacement::DockedRight;
        let label = panel_status_label_cow("Panel\n\u{202e}Name").into_owned();

        assert_eq!(
            cycle_panel_placement(&mut open, &mut placement, "Panel\n\u{202e}Name"),
            format!("{label} panel moved to floating window")
        );
        assert!(open);
        assert_eq!(placement, PanelPlacement::Floating);
    }

    fn assert_cow_borrows_original(label: Cow<'_, str>, original: &str) {
        match label {
            Cow::Borrowed(borrowed) => {
                assert_eq!(borrowed, original);
                assert_eq!(borrowed.as_ptr(), original.as_ptr());
                assert_eq!(borrowed.len(), original.len());
            }
            Cow::Owned(owned) => panic!("expected borrowed label, got owned {owned:?}"),
        }
    }

    fn assert_cow_owned_eq(label: Cow<'_, str>, expected: &str) {
        match label {
            Cow::Owned(owned) => assert_eq!(owned, expected),
            Cow::Borrowed(borrowed) => panic!("expected owned label, got borrowed {borrowed:?}"),
        }
    }
}
