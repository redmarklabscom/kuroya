use super::ExplorerEntryKind;
use crate::workspace_trust::{
    trusted_workspace_paths_match, workspace_path_contains_lexically,
    workspace_path_stays_within_root_lexically,
};
use std::path::{Component, Path, PathBuf};

pub(crate) fn explorer_kind_for_path(path: &Path) -> ExplorerEntryKind {
    if path.is_dir() {
        ExplorerEntryKind::Folder
    } else {
        ExplorerEntryKind::File
    }
}

pub(crate) fn workspace_child_path(
    root: &Path,
    parent: &Path,
    input: &str,
) -> Result<PathBuf, String> {
    if !workspace_path_stays_within_root_lexically(root, parent) {
        return Err("Explorer target must stay inside the workspace".to_owned());
    }

    let relative = relative_child_path(input)?;
    let target = workspace_child_target_path(parent, relative);
    if !workspace_path_stays_within_root_lexically(root, &target) {
        return Err("Explorer target must stay inside the workspace".to_owned());
    }
    Ok(target)
}

fn workspace_child_target_path(parent: &Path, relative: PathBuf) -> PathBuf {
    if parent == Path::new(".") {
        relative
    } else {
        parent.join(relative)
    }
}

fn relative_child_path(input: &str) -> Result<PathBuf, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Name is empty".to_owned());
    }

    let candidate = PathBuf::from(trimmed);
    if candidate.is_absolute() {
        return Err("Use a relative name inside the workspace".to_owned());
    }

    let mut clean = PathBuf::new();
    for component in candidate.components() {
        match component {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err("Names cannot escape the workspace".to_owned());
            }
        }
    }

    if clean.as_os_str().is_empty() {
        return Err("Name is empty".to_owned());
    }

    Ok(clean)
}

pub(crate) fn retarget_path_prefix(
    path: &Path,
    old_prefix: &Path,
    new_prefix: &Path,
) -> Option<PathBuf> {
    if !workspace_path_contains_lexically(old_prefix, path) {
        return None;
    }

    let old_prefix = lexical_normalize_path(old_prefix)?;
    if old_prefix.as_os_str().is_empty() {
        return None;
    }
    let path = lexical_normalize_path(path)?;
    let suffix = lexical_suffix_after_prefix(&path, &old_prefix);
    if suffix.as_os_str().is_empty() {
        Some(new_prefix.to_path_buf())
    } else {
        Some(new_prefix.join(suffix))
    }
}

pub(crate) fn path_matches_kind(path: &Path, target: &Path, kind: ExplorerEntryKind) -> bool {
    match kind {
        ExplorerEntryKind::File => trusted_workspace_paths_match(path, target),
        ExplorerEntryKind::Folder => workspace_path_contains_lexically(target, path),
    }
}

fn lexical_suffix_after_prefix(path: &Path, prefix: &Path) -> PathBuf {
    let mut suffix = PathBuf::new();
    for component in path.components().skip(prefix.components().count()) {
        suffix.push(component.as_os_str());
    }
    suffix
}

fn lexical_normalize_path(path: &Path) -> Option<PathBuf> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return None;
                }
            }
            Component::Normal(component) => normalized.push(component),
        }
    }
    Some(normalized)
}

#[cfg(test)]
mod tests {
    use super::workspace_child_path;
    use std::path::{Path, PathBuf};

    #[test]
    fn workspace_child_path_rejects_parent_reentry_paths() {
        let root = PathBuf::from("workspace").join("current");
        let parent = root.join("..").join("current").join("src");

        assert_eq!(
            workspace_child_path(&root, &parent, "main.rs").unwrap_err(),
            "Explorer target must stay inside the workspace"
        );
    }

    #[test]
    fn workspace_child_path_preserves_raw_parent_path_for_valid_targets() {
        let root = PathBuf::from("workspace");
        let parent = root.join("src").join(".").join("nested");

        let child = workspace_child_path(&root, &parent, "main.rs").unwrap();

        assert_eq!(child, parent.join("main.rs"));
        let child = child.to_string_lossy();
        assert!(child.contains("./nested") || child.contains(".\\nested"));
    }

    #[test]
    fn workspace_child_path_keeps_current_directory_workspace_output_compact() {
        assert_eq!(
            workspace_child_path(Path::new("."), Path::new("."), "src/main.rs").unwrap(),
            PathBuf::from("src").join("main.rs")
        );
    }
}
