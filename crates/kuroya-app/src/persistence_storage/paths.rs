use std::{
    env,
    ffi::OsStr,
    io::ErrorKind,
    path::{Component, Path, PathBuf},
};

#[cfg(not(test))]
const APP_STATE_FILE_NAME: &str = "state.json";
const STATE_DIR_NAME: &str = ".kuroya";
const SESSION_FILE_NAME: &str = "session.json";
const PROJECT_INDEX_CACHE_FILE_NAME: &str = "project-index.json";
const SESSION_SNAPSHOTS_DIR_NAME: &str = "snapshots";
const WORKSPACE_SNAPSHOTS_DIR_NAME: &str = "workspace-snapshots";
const WORKSPACE_STATE_BUCKET_DIR_NAME: &str = "workspaces";
const WORKSPACE_STATE_BUCKET_FALLBACK_LABEL: &str = "workspace";
const WORKSPACE_STATE_BUCKET_LABEL_MAX_BYTES: usize = 48;
const WORKSPACE_STATE_COMPONENT_MAX_CHARS: usize = 120;
const WORKSPACE_STATE_COMPONENT_MAX_BYTES: usize = 240;
const WORKSPACE_STATE_HASH_OFFSET: u64 = 0xcbf29ce484222325;
const WORKSPACE_STATE_HASH_PRIME: u64 = 0x100000001b3;

#[cfg(not(test))]
pub(crate) fn app_state_path() -> PathBuf {
    app_state_dir().join(APP_STATE_FILE_NAME)
}

pub(crate) fn state_dir(workspace_root: &Path) -> PathBuf {
    let normalized = normalize_workspace_root_for_storage(workspace_root);
    if workspace_root_needs_external_state_dir(&normalized) {
        return external_workspace_state_dir(&normalized);
    }

    normalized.join(STATE_DIR_NAME)
}

pub(crate) fn session_path(workspace_root: &Path) -> PathBuf {
    workspace_storage_path(workspace_root, SESSION_FILE_NAME)
}

pub(crate) fn project_index_cache_path(workspace_root: &Path) -> PathBuf {
    workspace_storage_path(workspace_root, PROJECT_INDEX_CACHE_FILE_NAME)
}

pub(crate) fn session_snapshots_dir(workspace_root: &Path) -> PathBuf {
    workspace_storage_path(workspace_root, SESSION_SNAPSHOTS_DIR_NAME)
}

pub(crate) fn workspace_snapshots_dir(workspace_root: &Path) -> PathBuf {
    workspace_storage_path(workspace_root, WORKSPACE_SNAPSHOTS_DIR_NAME)
}

fn workspace_storage_path(workspace_root: &Path, storage_name: &str) -> PathBuf {
    state_dir(workspace_root).join(storage_name)
}

fn normalize_workspace_root_for_storage(workspace_root: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    let mut normal_components = 0usize;
    let mut anchored = false;

    for component in workspace_root.components() {
        match component {
            Component::Prefix(prefix) => {
                normalized.push(prefix.as_os_str());
                anchored = true;
                normal_components = 0;
            }
            Component::RootDir => {
                normalized.push(component.as_os_str());
                anchored = true;
                normal_components = 0;
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if normal_components > 0 {
                    normalized.pop();
                    normal_components -= 1;
                } else if !anchored {
                    normalized.push(component.as_os_str());
                }
            }
            Component::Normal(component) => {
                normalized.push(component);
                normal_components += 1;
            }
        }
    }

    normalized
}

fn workspace_root_needs_external_state_dir(workspace_root: &Path) -> bool {
    workspace_root.as_os_str().is_empty()
        || workspace_root_has_unsafe_storage_component(workspace_root)
        || workspace_root_is_existing_non_directory(workspace_root)
}

fn workspace_root_has_unsafe_storage_component(workspace_root: &Path) -> bool {
    workspace_root
        .components()
        .any(|component| match component {
            Component::Normal(component) => storage_component_needs_external_guard(component),
            Component::Prefix(_)
            | Component::RootDir
            | Component::CurDir
            | Component::ParentDir => false,
        })
}

fn storage_component_needs_external_guard(component: &OsStr) -> bool {
    let text = component.to_string_lossy();
    text.is_empty()
        || text.len() > WORKSPACE_STATE_COMPONENT_MAX_BYTES
        || text
            .chars()
            .take(WORKSPACE_STATE_COMPONENT_MAX_CHARS + 1)
            .count()
            > WORKSPACE_STATE_COMPONENT_MAX_CHARS
        || text.chars().any(is_unsafe_storage_component_char)
        || storage_component_has_windows_unsafe_label(&text)
}

fn is_unsafe_storage_component_char(ch: char) -> bool {
    ch.is_control()
        || matches!(
            ch,
            '\u{061c}'
                | '\u{200b}'..='\u{200f}'
                | '\u{2028}'..='\u{202e}'
                | '\u{2060}'..='\u{206f}'
                | '\u{feff}'
        )
}

fn storage_component_has_windows_unsafe_label(text: &str) -> bool {
    storage_component_has_trailing_windows_trim_char(text)
        || storage_component_has_reserved_windows_label(text)
}

fn storage_component_has_trailing_windows_trim_char(text: &str) -> bool {
    matches!(text.as_bytes().last(), Some(b' ' | b'.'))
}

fn storage_component_has_reserved_windows_label(text: &str) -> bool {
    let stem = text.split('.').next().unwrap_or(text);
    stem.eq_ignore_ascii_case("con")
        || stem.eq_ignore_ascii_case("prn")
        || stem.eq_ignore_ascii_case("aux")
        || stem.eq_ignore_ascii_case("nul")
        || storage_component_has_reserved_windows_numbered_label(stem, "com")
        || storage_component_has_reserved_windows_numbered_label(stem, "lpt")
}

fn storage_component_has_reserved_windows_numbered_label(stem: &str, prefix: &str) -> bool {
    stem.len() == 4
        && stem
            .get(..3)
            .is_some_and(|stem_prefix| stem_prefix.eq_ignore_ascii_case(prefix))
        && matches!(stem.as_bytes()[3], b'1'..=b'9')
}

fn workspace_root_is_existing_non_directory(workspace_root: &Path) -> bool {
    match std::fs::metadata(workspace_root) {
        Ok(metadata) => !metadata.is_dir(),
        Err(error) if error.kind() == ErrorKind::NotFound => false,
        Err(_) => false,
    }
}

fn external_workspace_state_dir(workspace_root: &Path) -> PathBuf {
    app_state_dir()
        .join(WORKSPACE_STATE_BUCKET_DIR_NAME)
        .join(workspace_state_bucket_name(workspace_root))
}

fn workspace_state_bucket_name(workspace_root: &Path) -> String {
    let label = workspace_state_bucket_label(workspace_root);
    let hash = workspace_state_hash(workspace_root);
    format!("{label}-{hash:016x}")
}

fn workspace_state_bucket_label(workspace_root: &Path) -> String {
    let mut label = String::with_capacity(WORKSPACE_STATE_BUCKET_LABEL_MAX_BYTES);
    let raw_label = workspace_root
        .file_name()
        .filter(|label| !label.is_empty())
        .unwrap_or_else(|| OsStr::new(WORKSPACE_STATE_BUCKET_FALLBACK_LABEL));

    for ch in raw_label.to_string_lossy().chars() {
        let Some(ch) = workspace_state_bucket_label_char(ch) else {
            continue;
        };
        if label.len() >= WORKSPACE_STATE_BUCKET_LABEL_MAX_BYTES {
            break;
        }
        label.push(ch);
    }

    guard_workspace_state_bucket_label(&mut label);
    if label.is_empty() {
        WORKSPACE_STATE_BUCKET_FALLBACK_LABEL.to_owned()
    } else {
        label
    }
}

fn workspace_state_bucket_label_char(ch: char) -> Option<char> {
    if is_unsafe_storage_component_char(ch) {
        return None;
    }
    if ch.is_ascii_alphanumeric() {
        return Some(ch.to_ascii_lowercase());
    }
    if matches!(ch, '-' | '_' | '.') {
        return Some(ch);
    }
    Some('_')
}

fn guard_workspace_state_bucket_label(label: &mut String) {
    while label.ends_with('.') {
        label.pop();
        label.push('_');
    }
    while label.starts_with('.') {
        label.remove(0);
    }
    if storage_component_has_reserved_windows_label(label) {
        label.insert(0, '_');
    }
}

fn workspace_state_hash(workspace_root: &Path) -> u64 {
    let mut hash = WORKSPACE_STATE_HASH_OFFSET;
    for byte in workspace_root.to_string_lossy().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(WORKSPACE_STATE_HASH_PRIME);
    }
    hash
}

pub(crate) fn app_state_dir() -> PathBuf {
    if let Some(path) = env::var_os("KUROYA_STATE_DIR") {
        return PathBuf::from(path);
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(path) = env::var_os("APPDATA").or_else(|| env::var_os("LOCALAPPDATA")) {
            return PathBuf::from(path).join("Kuroya");
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = home_dir() {
            return home
                .join("Library")
                .join("Application Support")
                .join("Kuroya");
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if let Some(path) = env::var_os("XDG_DATA_HOME") {
            return PathBuf::from(path).join("Kuroya");
        }
        if let Some(home) = home_dir() {
            return home.join(".local").join("share").join("Kuroya");
        }
    }

    env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".kuroya")
        .join("app-state")
}

#[cfg(not(target_os = "windows"))]
fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        ffi::OsStr,
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_path(name: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "kuroya-persistence-paths-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn workspace_storage_paths_share_one_normalized_state_dir() {
        let workspace = PathBuf::from("workspace").join(".").join("src").join("..");
        let state = PathBuf::from("workspace").join(STATE_DIR_NAME);

        assert_eq!(state_dir(&workspace), state);
        assert_eq!(session_path(&workspace), state.join(SESSION_FILE_NAME));
        assert_eq!(
            project_index_cache_path(&workspace),
            state.join(PROJECT_INDEX_CACHE_FILE_NAME)
        );
        assert_eq!(
            session_snapshots_dir(&workspace),
            state.join(SESSION_SNAPSHOTS_DIR_NAME)
        );
        assert_eq!(
            workspace_snapshots_dir(&workspace),
            state.join(WORKSPACE_SNAPSHOTS_DIR_NAME)
        );
    }

    #[test]
    fn workspace_root_normalization_preserves_leading_parent_components() {
        let workspace = PathBuf::from("..")
            .join("outside")
            .join("workspace")
            .join("..");

        assert_eq!(
            state_dir(&workspace),
            PathBuf::from("..").join("outside").join(STATE_DIR_NAME)
        );
    }

    #[test]
    fn unsafe_workspace_root_uses_bounded_external_state_bucket() {
        let raw_name = format!("raw\n\u{202e}workspace{}", "x".repeat(160));
        let workspace = PathBuf::from("workspace")
            .join("..")
            .join(&raw_name)
            .join(".");
        let state = state_dir(&workspace);
        let state_text = state.as_os_str().to_string_lossy();
        let bucket = state.file_name().unwrap().to_string_lossy();

        assert_eq!(
            state
                .parent()
                .and_then(Path::file_name)
                .and_then(OsStr::to_str),
            Some(WORKSPACE_STATE_BUCKET_DIR_NAME)
        );
        assert!(!state_text.contains('\n'));
        assert!(!state_text.contains('\u{202e}'));
        assert!(!state_text.contains(&"x".repeat(160)));
        assert!(bucket.len() <= WORKSPACE_STATE_BUCKET_LABEL_MAX_BYTES + 1 + 16);
        assert!(
            bucket
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
        );
    }

    #[test]
    fn file_shaped_workspace_root_uses_external_state_bucket() {
        let workspace = temp_path("file-root");
        fs::write(&workspace, b"not a directory").unwrap();

        let state = state_dir(&workspace);

        assert!(!state.starts_with(&workspace));
        assert_eq!(
            state
                .parent()
                .and_then(Path::file_name)
                .and_then(OsStr::to_str),
            Some(WORKSPACE_STATE_BUCKET_DIR_NAME)
        );

        fs::remove_file(workspace).unwrap();
    }
}
