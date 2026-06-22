use crate::native_paths::normalize_native_path;
#[cfg(not(test))]
use crate::persistence_storage::app_state_path;
use crate::persistence_storage::{app_state_dir, atomic_write, read_file_bytes_with_limit};
use crate::workspace_trust::trusted_workspace_paths_match;

pub(crate) use crate::persistence_models::{
    APP_STATE_RECENT_PROJECTS_MAX, APP_STATE_TRUSTED_WORKSPACES_MAX,
};
pub(crate) use crate::persistence_workspace_snapshots::{
    load_latest_workspace_snapshot, save_workspace_snapshot,
};
pub use crate::{
    persistence_models::{
        AppState, BufferFoldState, BufferHistoryState, BufferSelectionState, BufferViewState,
        PaneBufferViewState, PersistedClosedFileEntry, PersistedFoldRange,
        PersistedNavigationLocation, PersistedSession, PersistedSourceControlSortMode,
        PersistedSourceControlViewMode, PersistedTerminalProcessStatus, PersistedTerminalSession,
        RecoveredBuffer, RecoveredBufferHistoryState, RecoveredBufferViewState,
        SkippedRecoveredBuffer,
    },
    persistence_session::{save_session, save_session_async},
};
use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(test)]
use crate::persistence_storage::state_dir;

const APP_STATE_MAX_BYTES: u64 = 128 * 1024;
const EMPTY_STARTUP_WORKSPACE_DIR_NAME: &str = "empty-workspace";

impl AppState {
    pub fn load() -> anyhow::Result<Self> {
        #[cfg(test)]
        {
            load_app_state_from_path(&test_app_state_path())
        }
        #[cfg(not(test))]
        {
            load_app_state_from_path(&app_state_path())
        }
    }

    pub fn save(&self) -> anyhow::Result<()> {
        #[cfg(test)]
        {
            self.save_to_path(&test_app_state_path())
        }
        #[cfg(not(test))]
        {
            self.save_to_path(&app_state_path())
        }
    }

    pub(crate) fn save_to_path(&self, path: &Path) -> anyhow::Result<()> {
        save_app_state_to_path(path, self)
    }
}

#[cfg(test)]
fn test_app_state_path() -> PathBuf {
    let thread_id = format!("{:?}", std::thread::current().id())
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>();
    std::env::temp_dir()
        .join(format!(
            "kuroya-app-state-test-{}-{thread_id}",
            std::process::id()
        ))
        .join("state.json")
}

pub fn normalize_recent_projects(projects: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    normalize_path_list_by(
        projects
            .into_iter()
            .map(normalize_native_path)
            .filter(|path| !is_empty_startup_recent_project(path)),
        APP_STATE_RECENT_PROJECTS_MAX,
        trusted_workspace_paths_match,
    )
}

pub fn normalize_trusted_workspaces(workspaces: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    normalize_path_list_by(
        workspaces.into_iter().map(normalize_native_path),
        APP_STATE_TRUSTED_WORKSPACES_MAX,
        trusted_workspace_paths_match,
    )
}

fn is_empty_startup_recent_project(path: &Path) -> bool {
    trusted_workspace_paths_match(
        path,
        &app_state_dir().join(EMPTY_STARTUP_WORKSPACE_DIR_NAME),
    )
}

fn normalize_path_list_by(
    paths: impl IntoIterator<Item = PathBuf>,
    max: usize,
    mut matches: impl FnMut(&Path, &Path) -> bool,
) -> Vec<PathBuf> {
    let mut normalized = Vec::new();

    for path in paths {
        if path.as_os_str().is_empty()
            || normalized
                .iter()
                .any(|candidate: &PathBuf| matches(candidate, &path))
        {
            continue;
        }
        normalized.push(path);
        if normalized.len() == max {
            break;
        }
    }

    normalized
}

fn load_app_state_from_path(path: &Path) -> anyhow::Result<AppState> {
    let bytes = match read_file_bytes_with_limit(path, APP_STATE_MAX_BYTES) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(AppState::default()),
        Err(error) if error.kind() == ErrorKind::InvalidData => {
            quarantine_corrupt_app_state(path)?;
            return Ok(AppState::default());
        }
        Err(error) => return Err(error.into()),
    };
    let mut state: AppState = match serde_json::from_slice(&bytes) {
        Ok(state) => state,
        Err(_) => {
            quarantine_corrupt_app_state(path)?;
            return Ok(AppState::default());
        }
    };
    state.recent_projects = normalize_recent_projects(state.recent_projects);
    state.trusted_workspaces = normalize_trusted_workspaces(state.trusted_workspaces);
    if let Some(vim) = &mut state.vim {
        vim.sanitize();
        crate::editor_vim_key_events::sanitize_vim_settings_for_runtime(vim);
    }
    Ok(state)
}

fn save_app_state_to_path(path: &Path, state: &AppState) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let state = AppState {
        recent_projects: normalize_recent_projects(state.recent_projects.clone()),
        trusted_workspaces: normalize_trusted_workspaces(state.trusted_workspaces.clone()),
        vim_keybindings: state.vim_keybindings,
        vim: state.vim.clone().map(|mut vim| {
            vim.sanitize();
            crate::editor_vim_key_events::sanitize_vim_settings_for_runtime(&mut vim);
            vim
        }),
    };
    let bytes = serde_json::to_vec_pretty(&state)?;
    atomic_write(path, &bytes)?;
    Ok(())
}

fn quarantine_corrupt_app_state(path: &Path) -> anyhow::Result<PathBuf> {
    let quarantine = corrupt_app_state_path(path);
    std::fs::rename(path, &quarantine)?;
    Ok(quarantine)
}

fn corrupt_app_state_path(path: &Path) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("state.json");
    path.with_file_name(format!(
        "{file_name}.corrupt.{}.{}",
        std::process::id(),
        unique
    ))
}

#[cfg(test)]
mod tests;
