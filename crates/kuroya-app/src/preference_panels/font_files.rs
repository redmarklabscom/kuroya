use crate::{native_paths::normalize_native_path, settings_form::optional_setting_path_from_input};

use std::path::{Path, PathBuf};

mod paths;

use paths::{font_dialog_initial_dir, setting_path_for_selected_font};

pub(super) fn choose_font_file(
    workspace_root: &Path,
    current: &str,
) -> Result<Option<String>, String> {
    let initial_dir = font_dialog_initial_dir(workspace_root, current);
    let Some(path) = pick_font_file(&initial_dir)? else {
        return Ok(None);
    };
    validated_selected_font_setting_path(workspace_root, &path).map(Some)
}

fn validated_selected_font_setting_path(
    workspace_root: &Path,
    selected: &Path,
) -> Result<String, String> {
    let setting_path = setting_path_for_selected_font(workspace_root, selected);
    optional_setting_path_from_input(&setting_path)
        .ok_or_else(|| "Selected font path contains unsupported characters".to_owned())
}

fn pick_font_file(initial_dir: &Path) -> Result<Option<PathBuf>, String> {
    Ok(rfd::FileDialog::new()
        .set_title("Choose font file")
        .add_filter("Font files", &["ttf", "otf"])
        .add_filter("All files", &["*"])
        .set_directory(initial_dir)
        .pick_file()
        .map(normalize_native_path))
}

#[cfg(test)]
fn font_chooser_failure_status(error: &str) -> String {
    let error = crate::path_display::display_error_label_cow(error);
    format!("Could not open font chooser: {}", error.as_ref())
}

#[cfg(test)]
mod tests {
    use super::{font_chooser_failure_status, validated_selected_font_setting_path};
    use crate::path_display::DISPLAY_ERROR_LABEL_MAX_CHARS;
    use std::path::Path;

    #[test]
    fn font_chooser_failure_status_sanitizes_and_bounds_error_detail() {
        let status = font_chooser_failure_status(&format!(
            "first line\nsecond line \u{202e}{}",
            "font-detail-".repeat(DISPLAY_ERROR_LABEL_MAX_CHARS * 2)
        ));

        assert!(status.starts_with("Could not open font chooser: first line "));
        assert!(!status.contains('\n'));
        assert!(!status.contains('\u{202e}'));
        assert!(status.contains("..."));
        assert!(
            status.chars().count()
                <= "Could not open font chooser: ".chars().count() + DISPLAY_ERROR_LABEL_MAX_CHARS
        );
    }

    #[test]
    fn font_chooser_failure_status_falls_back_for_blank_error_detail() {
        assert_eq!(
            font_chooser_failure_status("\n\u{202e}\u{0007}"),
            "Could not open font chooser: unknown error"
        );
    }

    #[test]
    fn selected_font_setting_path_rejects_unsafe_display_text() {
        let error = validated_selected_font_setting_path(
            Path::new("workspace"),
            Path::new("workspace/fonts/\u{202e}Editor.ttf"),
        )
        .unwrap_err();

        assert_eq!(error, "Selected font path contains unsupported characters");
    }
}
