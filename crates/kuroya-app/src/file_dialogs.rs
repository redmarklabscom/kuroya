use crate::{
    KuroyaApp,
    app_startup_context::terminal_root_for_workspace,
    native_paths::normalize_native_path,
    path_display::{
        DISPLAY_PATH_LABEL_MAX_CHARS, display_error_label_cow, sanitized_display_label_cow,
    },
    ui_events::UiEvent,
};
use kuroya_core::Command as AppCommand;
use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

impl KuroyaApp {
    pub(crate) fn begin_open_workspace(&mut self) {
        if self.open_workspace_picker_in_flight {
            self.status = "Workspace folder picker is already open".to_owned();
            return;
        }
        self.open_workspace_open = false;
        self.open_workspace_path.clear();
        self.open_workspace_picker_in_flight = true;
        self.open_workspace_picker_request_id =
            next_open_workspace_picker_request_id(self.open_workspace_picker_request_id);
        let initial_dir = if self.workspace_placeholder {
            terminal_root_for_workspace(&self.workspace.root)
        } else {
            self.workspace.root.clone()
        };
        self.spawn_open_workspace_picker(self.open_workspace_picker_request_id, initial_dir);
        self.status = "Choose a workspace folder".to_owned();
    }

    pub(crate) fn apply_open_workspace_picked(&mut self, request_id: u64, path: PathBuf) {
        if !self.open_workspace_picker_event_matches(request_id) {
            return;
        }
        self.open_workspace_picker_in_flight = false;
        match resolve_workspace_path_candidate(
            &path,
            workspace_path_is_dir,
            canonicalize_workspace_path,
        ) {
            Ok(path) => {
                self.open_workspace_open = false;
                self.command_bus.push(AppCommand::OpenWorkspace(path));
            }
            Err(error) => {
                self.status = error;
            }
        }
    }

    pub(crate) fn apply_open_workspace_picker_canceled(&mut self, request_id: u64) {
        if !self.open_workspace_picker_event_matches(request_id) {
            return;
        }
        self.open_workspace_picker_in_flight = false;
        self.status = "Workspace open canceled".to_owned();
    }

    pub(crate) fn apply_open_workspace_picker_failed(&mut self, request_id: u64, error: String) {
        if !self.open_workspace_picker_event_matches(request_id) {
            return;
        }
        self.open_workspace_picker_in_flight = false;
        self.status = error;
    }

    fn open_workspace_picker_event_matches(&self, request_id: u64) -> bool {
        self.open_workspace_picker_in_flight && self.open_workspace_picker_request_id == request_id
    }

    fn spawn_open_workspace_picker(&mut self, request_id: u64, initial_dir: PathBuf) {
        let tx = self.tx.clone();
        self.runtime.spawn_blocking(move || {
            let event = match pick_workspace_folder(&initial_dir) {
                Ok(Some(path)) => UiEvent::OpenWorkspacePicked { request_id, path },
                Ok(None) => UiEvent::OpenWorkspacePickerCanceled { request_id },
                Err(error) => UiEvent::OpenWorkspacePickerFailed { request_id, error },
            };
            let _ = crate::ui_event_channel::send_critical_ui_event(&tx, event);
        });
    }
}

#[cfg(test)]
fn resolve_workspace_path_input(
    input: &str,
    current_dir: &Path,
    path_is_dir: impl FnOnce(&Path) -> Result<bool, String>,
    canonicalize: impl FnOnce(&Path) -> Result<PathBuf, String>,
) -> Result<PathBuf, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Workspace path is empty".to_owned());
    }

    let path = workspace_path_candidate(trimmed, current_dir);
    resolve_workspace_path_candidate_with_label(
        &path,
        Path::new(trimmed),
        path_is_dir,
        canonicalize,
    )
}

fn resolve_workspace_path_candidate(
    path: &Path,
    path_is_dir: impl FnOnce(&Path) -> Result<bool, String>,
    canonicalize: impl FnOnce(&Path) -> Result<PathBuf, String>,
) -> Result<PathBuf, String> {
    resolve_workspace_path_candidate_with_label(path, path, path_is_dir, canonicalize)
}

fn resolve_workspace_path_candidate_with_label(
    path: &Path,
    display_path: &Path,
    path_is_dir: impl FnOnce(&Path) -> Result<bool, String>,
    canonicalize: impl FnOnce(&Path) -> Result<PathBuf, String>,
) -> Result<PathBuf, String> {
    if path.as_os_str().is_empty() {
        return Err("Workspace path is empty".to_owned());
    }

    if !path_is_dir(path)? {
        return Err(open_workspace_not_folder_status(display_path));
    }
    canonicalize(path)
}

#[cfg(test)]
fn workspace_path_candidate(input: &str, current_dir: &Path) -> PathBuf {
    let path = PathBuf::from(input);
    if path.is_absolute() {
        path
    } else {
        current_dir.join(path)
    }
}

fn workspace_path_is_dir(path: &Path) -> Result<bool, String> {
    std::fs::metadata(path)
        .map(|metadata| metadata.is_dir())
        .map_err(|error| open_workspace_failure_status(path, &error.to_string()))
}

fn canonicalize_workspace_path(path: &Path) -> Result<PathBuf, String> {
    std::fs::canonicalize(path)
        .map(normalize_native_path)
        .map_err(|error| open_workspace_failure_status(path, &error.to_string()))
}

fn pick_workspace_folder(initial_dir: &Path) -> Result<Option<PathBuf>, String> {
    Ok(rfd::FileDialog::new()
        .set_title("Choose workspace folder")
        .set_directory(initial_dir)
        .pick_folder()
        .map(normalize_native_path))
}

fn open_workspace_failure_status(path: &Path, error: &str) -> String {
    let path_label = open_workspace_path_label_cow(path);
    let error_label = display_error_label_cow(error);
    format!(
        "Could not open workspace {}: {}",
        path_label.as_ref(),
        error_label.as_ref()
    )
}

fn open_workspace_not_folder_status(path: &Path) -> String {
    let path_label = open_workspace_path_label_cow(path);
    format!("Workspace path must be a folder: {path_label}")
}

fn open_workspace_path_label(path: &Path) -> String {
    let display = path.display().to_string();
    if let Cow::Owned(label) =
        sanitized_display_label_cow(&display, DISPLAY_PATH_LABEL_MAX_CHARS, ".")
    {
        label
    } else {
        display
    }
}

fn open_workspace_path_label_cow(path: &Path) -> Cow<'_, str> {
    if let Some(display) = path.as_os_str().to_str() {
        return sanitized_display_label_cow(display, DISPLAY_PATH_LABEL_MAX_CHARS, ".");
    }

    Cow::Owned(open_workspace_path_label(path))
}

fn next_open_workspace_picker_request_id(current: u64) -> u64 {
    match current.wrapping_add(1) {
        0 => 1,
        request_id => request_id,
    }
}

#[cfg(test)]
mod tests {
    use super::next_open_workspace_picker_request_id;
    use super::{
        DISPLAY_PATH_LABEL_MAX_CHARS, open_workspace_failure_status,
        open_workspace_not_folder_status, open_workspace_path_label, open_workspace_path_label_cow,
        resolve_workspace_path_candidate, resolve_workspace_path_input,
    };
    use crate::path_display::DISPLAY_ERROR_LABEL_MAX_CHARS;
    use std::{
        borrow::Cow,
        cell::Cell,
        path::{Path, PathBuf},
    };

    #[test]
    fn file_dialog_open_workspace_failure_status_sanitizes_and_bounds_path_and_error() {
        let path = Path::new("workspace").join(format!(
            "bad\n{}\u{202e}tail",
            "very-long-component-".repeat(16)
        ));
        let status = open_workspace_failure_status(
            &path,
            &format!(
                "first line\nsecond line \u{2066}{}",
                "detail-".repeat(DISPLAY_ERROR_LABEL_MAX_CHARS * 2)
            ),
        );

        assert!(status.starts_with("Could not open workspace workspace"));
        assert!(!status.contains('\n'));
        assert!(!status.contains('\u{202e}'));
        assert!(!status.contains('\u{2066}'));
        assert!(status.contains("..."));
        assert!(
            status.chars().count()
                <= "Could not open workspace : ".chars().count()
                    + DISPLAY_PATH_LABEL_MAX_CHARS
                    + DISPLAY_ERROR_LABEL_MAX_CHARS
        );
    }

    #[test]
    fn file_dialog_open_workspace_statuses_fall_back_for_blank_display_text() {
        assert_eq!(
            open_workspace_failure_status(Path::new("\n\u{202e}\u{0007}"), "\n\u{2066}"),
            "Could not open workspace .: unknown error"
        );
        assert_eq!(
            open_workspace_not_folder_status(Path::new("\n\u{202e}\u{0007}")),
            "Workspace path must be a folder: ."
        );
    }

    #[test]
    fn file_dialog_open_workspace_path_label_cow_borrows_clean_ascii_and_unicode_display_text() {
        let ascii = Path::new("workspace").join("src").join("main.rs");
        match open_workspace_path_label_cow(&ascii) {
            Cow::Borrowed(label) => assert_eq!(label, ascii.display().to_string()),
            Cow::Owned(label) => panic!("expected borrowed clean ASCII label, got {label:?}"),
        }
        assert_eq!(
            open_workspace_path_label(&ascii),
            ascii.display().to_string()
        );

        let unicode = Path::new("workspace").join("clean-\u{03bb}");
        match open_workspace_path_label_cow(&unicode) {
            Cow::Borrowed(label) => assert_eq!(label, unicode.display().to_string()),
            Cow::Owned(label) => panic!("expected borrowed clean unicode label, got {label:?}"),
        }
        assert_eq!(
            open_workspace_path_label(&unicode),
            unicode.display().to_string()
        );
    }

    #[test]
    fn file_dialog_open_workspace_path_label_sanitizes_truncates_and_falls_back() {
        let path = Path::new("workspace").join(format!(
            "bad\n{}\u{202e}tail",
            "very-long-component-".repeat(16)
        ));

        let label = open_workspace_path_label_cow(&path);

        assert!(matches!(&label, Cow::Owned(_)));
        assert!(!label.contains('\n'));
        assert!(!label.contains('\u{202e}'));
        assert!(label.contains("..."));
        assert!(label.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);

        let fallback = open_workspace_path_label_cow(Path::new("\n\u{202e}\u{0007}"));
        assert!(matches!(&fallback, Cow::Owned(_)));
        assert_eq!(fallback.as_ref(), ".");
        assert_eq!(
            open_workspace_path_label(Path::new("\n\u{202e}\u{0007}")),
            "."
        );
    }

    #[test]
    fn file_dialog_open_workspace_status_wording_is_unchanged_for_clean_labels() {
        let path = Path::new("workspace");

        assert_eq!(
            open_workspace_failure_status(path, "access denied"),
            "Could not open workspace workspace: access denied"
        );
        assert_eq!(
            open_workspace_not_folder_status(path),
            "Workspace path must be a folder: workspace"
        );
    }

    #[test]
    fn file_dialog_resolve_workspace_path_checks_directory_before_canonicalize() {
        let current_dir = PathBuf::from("workspace");
        let candidate = current_dir.join("src");
        let canonical = PathBuf::from("canonical/workspace/src");
        let is_dir_calls = Cell::new(0usize);
        let canonicalize_calls = Cell::new(0usize);

        let resolved = resolve_workspace_path_input(
            " src ",
            &current_dir,
            |path| {
                is_dir_calls.set(is_dir_calls.get() + 1);
                assert_eq!(path, candidate.as_path());
                Ok(true)
            },
            |path| {
                canonicalize_calls.set(canonicalize_calls.get() + 1);
                assert_eq!(path, candidate.as_path());
                Ok(canonical.clone())
            },
        )
        .expect("directory should resolve");

        assert_eq!(resolved, canonical);
        assert_eq!(is_dir_calls.get(), 1);
        assert_eq!(canonicalize_calls.get(), 1);
    }

    #[test]
    fn file_dialog_resolve_workspace_path_preserves_raw_candidate_when_label_is_sanitized() {
        let current_dir = PathBuf::from("workspace");
        let raw_input = "dirty\nfolder\u{202e}name";
        let candidate = current_dir.join(raw_input);
        let canonical = PathBuf::from("canonical").join(raw_input);

        let resolved = resolve_workspace_path_input(
            raw_input,
            &current_dir,
            |path| {
                assert_eq!(path, candidate.as_path());
                Ok(true)
            },
            |path| {
                assert_eq!(path, candidate.as_path());
                Ok(canonical.clone())
            },
        )
        .expect("raw candidate should resolve");

        assert_eq!(resolved, canonical);

        let error = resolve_workspace_path_input(
            raw_input,
            &current_dir,
            |path| {
                assert_eq!(path, candidate.as_path());
                Ok(false)
            },
            |_| panic!("non-folders must not be canonicalized"),
        )
        .unwrap_err();

        assert_eq!(error, "Workspace path must be a folder: dirty foldername");
    }

    #[test]
    fn file_dialog_resolve_workspace_path_rejects_non_folder_without_canonicalize_probe() {
        let current_dir = PathBuf::from("workspace");
        let candidate = current_dir.join("README.md");
        let canonicalize_calls = Cell::new(0usize);

        let error = resolve_workspace_path_input(
            "README.md",
            &current_dir,
            |path| {
                assert_eq!(path, candidate.as_path());
                Ok(false)
            },
            |_| {
                canonicalize_calls.set(canonicalize_calls.get() + 1);
                Ok(PathBuf::from("should-not-resolve"))
            },
        )
        .unwrap_err();

        assert_eq!(error, "Workspace path must be a folder: README.md");
        assert_eq!(canonicalize_calls.get(), 0);
    }

    #[test]
    fn file_dialog_resolve_workspace_picked_path_canonicalizes_folder() {
        let picked = PathBuf::from("workspace").join("selected");
        let canonical = PathBuf::from("canonical")
            .join("workspace")
            .join("selected");
        let is_dir_calls = Cell::new(0usize);
        let canonicalize_calls = Cell::new(0usize);

        let resolved = resolve_workspace_path_candidate(
            &picked,
            |path| {
                is_dir_calls.set(is_dir_calls.get() + 1);
                assert_eq!(path, picked.as_path());
                Ok(true)
            },
            |path| {
                canonicalize_calls.set(canonicalize_calls.get() + 1);
                assert_eq!(path, picked.as_path());
                Ok(canonical.clone())
            },
        )
        .expect("picked folder should resolve");

        assert_eq!(resolved, canonical);
        assert_eq!(is_dir_calls.get(), 1);
        assert_eq!(canonicalize_calls.get(), 1);
    }

    #[test]
    fn file_dialog_picked_path_rejects_stale_non_folder_without_canonicalize_probe() {
        let picked = PathBuf::from("workspace").join("stale");
        let canonicalize_calls = Cell::new(0usize);

        let error = resolve_workspace_path_candidate(
            &picked,
            |path| {
                assert_eq!(path, picked.as_path());
                Ok(false)
            },
            |_| {
                canonicalize_calls.set(canonicalize_calls.get() + 1);
                Ok(PathBuf::from("should-not-resolve"))
            },
        )
        .unwrap_err();

        assert_eq!(error, open_workspace_not_folder_status(&picked));
        assert_eq!(canonicalize_calls.get(), 0);
    }

    #[test]
    fn file_dialog_workspace_picker_request_ids_skip_zero_after_wrap() {
        assert_eq!(next_open_workspace_picker_request_id(0), 1);
        assert_eq!(next_open_workspace_picker_request_id(41), 42);
        assert_eq!(next_open_workspace_picker_request_id(u64::MAX), 1);
    }
}
