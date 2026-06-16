use crate::{
    path_display::display_error_label_cow, settings_form::optional_setting_path_from_input,
};

use std::{
    path::{Path, PathBuf},
    process::Command as ProcessCommand,
};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

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

#[cfg(windows)]
fn pick_font_file(initial_dir: &Path) -> Result<Option<PathBuf>, String> {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const SCRIPT: &str = r#"
param([string]$InitialDirectory)
Add-Type -AssemblyName System.Windows.Forms
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$dialog = New-Object System.Windows.Forms.OpenFileDialog
$dialog.Title = 'Choose font file'
$dialog.Filter = 'Font files (*.ttf;*.otf)|*.ttf;*.otf|All files (*.*)|*.*'
$dialog.CheckFileExists = $true
$dialog.Multiselect = $false
if ($InitialDirectory -and [System.IO.Directory]::Exists($InitialDirectory)) {
    $dialog.InitialDirectory = $InitialDirectory
}
$result = $dialog.ShowDialog()
if ($result -eq [System.Windows.Forms.DialogResult]::OK) {
    Write-Output $dialog.FileName
}
"#;
    let initial_dir = initial_dir.display().to_string();

    let output = ProcessCommand::new("powershell.exe")
        .args([
            "-NoProfile",
            "-STA",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            SCRIPT,
            &initial_dir,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|error| font_chooser_failure_status(&error.to_string()))?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(if error.is_empty() {
            "Could not open font chooser".to_owned()
        } else {
            font_chooser_failure_status(&error)
        });
    }

    let selected = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if selected.is_empty() {
        Ok(None)
    } else {
        Ok(Some(PathBuf::from(selected)))
    }
}

#[cfg(not(windows))]
fn pick_font_file(_initial_dir: &Path) -> Result<Option<PathBuf>, String> {
    Err("Font chooser is only available on Windows in this build".to_owned())
}

fn font_chooser_failure_status(error: &str) -> String {
    let error = display_error_label_cow(error);
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
