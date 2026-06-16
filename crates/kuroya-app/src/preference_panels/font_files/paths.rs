use crate::settings_form::optional_setting_path_from_input;

use std::path::{Path, PathBuf};

pub(super) fn font_dialog_initial_dir(workspace_root: &Path, current: &str) -> PathBuf {
    let Some(trimmed) = optional_setting_path_from_input(current) else {
        return workspace_root.to_path_buf();
    };

    let path = PathBuf::from(trimmed);
    let path = if path.is_absolute() {
        path
    } else {
        workspace_root.join(path)
    };
    if path.is_dir() {
        path
    } else {
        path.parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| workspace_root.to_path_buf())
    }
}

pub(super) fn setting_path_for_selected_font(workspace_root: &Path, selected: &Path) -> String {
    let root = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| workspace_root.to_path_buf());
    let selected = selected
        .canonicalize()
        .unwrap_or_else(|_| selected.to_path_buf());
    selected
        .strip_prefix(&root)
        .unwrap_or(&selected)
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::{font_dialog_initial_dir, setting_path_for_selected_font};
    use std::path::Path;

    #[test]
    fn font_dialog_initial_dir_uses_workspace_for_empty_path() {
        assert_eq!(
            font_dialog_initial_dir(Path::new("workspace"), ""),
            Path::new("workspace")
        );
    }

    #[test]
    fn font_dialog_initial_dir_uses_workspace_for_unsafe_path() {
        assert_eq!(
            font_dialog_initial_dir(Path::new("workspace"), "fonts/\u{202e}Editor.ttf"),
            Path::new("workspace")
        );
    }

    #[test]
    fn selected_workspace_font_is_stored_relative_to_workspace() {
        let workspace =
            std::env::temp_dir().join(format!("kuroya-font-picker-{}", std::process::id()));
        let font_dir = workspace.join("fonts");
        let font = font_dir.join("Editor.ttf");
        std::fs::create_dir_all(&font_dir).unwrap();
        std::fs::write(&font, b"font").unwrap();

        assert_eq!(
            setting_path_for_selected_font(&workspace, &font),
            Path::new("fonts").join("Editor.ttf").display().to_string()
        );

        let _ = std::fs::remove_dir_all(workspace);
    }
}
