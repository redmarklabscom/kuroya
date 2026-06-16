use kuroya_core::LspServerConfig;
use std::path::Path;

pub fn can_use_server_for_path(config: &LspServerConfig, root: &Path, path: &Path) -> bool {
    can_use_server_for_path_with_probe(config, root, path, Path::exists)
}

fn can_use_server_for_path_with_probe(
    config: &LspServerConfig,
    root: &Path,
    path: &Path,
    mut path_exists: impl FnMut(&Path) -> bool,
) -> bool {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    let mut root_child_cache = RootChildProbeCache::default();
    if path.parent() == Some(root) {
        let exists = root_child_cache.get_or_probe(root, file_name, &mut path_exists);
        if exists {
            return true;
        }
    }

    for marker in &config.root_markers {
        let exists = root_child_cache.get_or_probe(root, marker, &mut path_exists);
        if exists {
            return true;
        }
    }

    root_child_cache.get_or_probe(root, file_name, path_exists)
}

#[derive(Default)]
struct RootChildProbeCache<'a> {
    entries: Vec<(&'a str, bool)>,
}

impl<'a> RootChildProbeCache<'a> {
    fn get_or_probe(
        &mut self,
        root: &Path,
        child: &'a str,
        mut path_exists: impl FnMut(&Path) -> bool,
    ) -> bool {
        if let Some((_, exists)) = self
            .entries
            .iter()
            .find(|(cached_child, _)| *cached_child == child)
        {
            return *exists;
        }

        let exists = root_child_exists(root, child, &mut path_exists);
        self.entries.push((child, exists));
        exists
    }
}

fn root_child_exists(root: &Path, child: &str, mut path_exists: impl FnMut(&Path) -> bool) -> bool {
    let mut candidate = root.to_path_buf();
    candidate.push(child);
    path_exists(&candidate)
}

#[cfg(test)]
mod tests {
    use super::can_use_server_for_path_with_probe;
    use kuroya_core::LspServerConfig;
    use std::{cell::RefCell, path::PathBuf};

    #[test]
    fn nested_project_file_checks_root_markers_before_source_name() {
        let config = config_with_markers(&["Cargo.toml", "rust-project.json"]);
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let probes = RefCell::new(Vec::new());

        assert!(can_use_server_for_path_with_probe(
            &config,
            &root,
            &path,
            |candidate| {
                probes.borrow_mut().push(candidate.to_path_buf());
                candidate == root.join("Cargo.toml")
            },
        ));

        assert_eq!(probes.into_inner(), vec![root.join("Cargo.toml")]);
    }

    #[test]
    fn root_project_file_keeps_fast_source_file_probe() {
        let config = config_with_markers(&["Cargo.toml"]);
        let root = PathBuf::from("workspace");
        let path = root.join("main.rs");
        let probes = RefCell::new(Vec::new());

        assert!(can_use_server_for_path_with_probe(
            &config,
            &root,
            &path,
            |candidate| {
                probes.borrow_mut().push(candidate.to_path_buf());
                candidate == root.join("main.rs")
            },
        ));

        assert_eq!(probes.into_inner(), vec![root.join("main.rs")]);
    }

    #[test]
    fn markerless_nested_file_falls_back_to_root_source_name() {
        let config = config_with_markers(&[]);
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let probes = RefCell::new(Vec::new());

        assert!(can_use_server_for_path_with_probe(
            &config,
            &root,
            &path,
            |candidate| {
                probes.borrow_mut().push(candidate.to_path_buf());
                candidate == root.join("main.rs")
            },
        ));

        assert_eq!(probes.into_inner(), vec![root.join("main.rs")]);
    }

    #[test]
    fn nested_file_name_marker_is_not_probed_again_as_fallback() {
        let config = config_with_markers(&["main.rs"]);
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let probes = RefCell::new(Vec::new());

        assert!(!can_use_server_for_path_with_probe(
            &config,
            &root,
            &path,
            |candidate| {
                probes.borrow_mut().push(candidate.to_path_buf());
                false
            },
        ));

        assert_eq!(probes.into_inner(), vec![root.join("main.rs")]);
    }

    #[test]
    fn duplicate_root_markers_reuse_first_probe_before_fallback() {
        let config = config_with_markers(&["Cargo.toml", "Cargo.toml"]);
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let probes = RefCell::new(Vec::new());

        assert!(can_use_server_for_path_with_probe(
            &config,
            &root,
            &path,
            |candidate| {
                probes.borrow_mut().push(candidate.to_path_buf());
                candidate == root.join("main.rs")
            },
        ));

        assert_eq!(
            probes.into_inner(),
            vec![root.join("Cargo.toml"), root.join("main.rs")]
        );
    }

    #[test]
    fn duplicate_root_file_name_markers_reuse_fallback_probe() {
        let config = config_with_markers(&["main.rs", "main.rs"]);
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let probes = RefCell::new(Vec::new());

        assert!(!can_use_server_for_path_with_probe(
            &config,
            &root,
            &path,
            |candidate| {
                probes.borrow_mut().push(candidate.to_path_buf());
                false
            },
        ));

        assert_eq!(probes.into_inner(), vec![root.join("main.rs")]);
    }

    #[test]
    fn root_file_marker_reuses_fast_probe_before_other_markers() {
        let config = config_with_markers(&["main.rs", "Cargo.toml"]);
        let root = PathBuf::from("workspace");
        let path = root.join("main.rs");
        let probes = RefCell::new(Vec::new());

        assert!(can_use_server_for_path_with_probe(
            &config,
            &root,
            &path,
            |candidate| {
                probes.borrow_mut().push(candidate.to_path_buf());
                candidate == root.join("Cargo.toml")
            },
        ));

        assert_eq!(
            probes.into_inner(),
            vec![root.join("main.rs"), root.join("Cargo.toml")]
        );
    }

    fn config_with_markers(markers: &[&str]) -> LspServerConfig {
        LspServerConfig {
            language: "rust".to_owned(),
            command: "rust-analyzer".to_owned(),
            args: Vec::new(),
            root_markers: markers.iter().map(|marker| (*marker).to_owned()).collect(),
        }
    }
}
