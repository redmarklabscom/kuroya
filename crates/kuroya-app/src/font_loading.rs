use crate::settings_form::optional_setting_path_from_input;

use std::{
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

pub(crate) const MAX_FONT_FILE_BYTES: u64 = 16 * 1024 * 1024;
pub(crate) const MAX_FONT_STACK_FONTS: usize = 6;
const FONT_DATA_NAME_PREFIX: &str = "kuroya_";
const MAX_FONT_DATA_NAME_CHARS: usize = 96;

#[cfg(test)]
pub(crate) fn load_font_bytes(
    workspace_root: &Path,
    configured: Option<&str>,
    candidates: &[PathBuf],
) -> Option<(String, Vec<u8>)> {
    load_font_stack_bytes(workspace_root, configured, candidates, 1)
        .into_iter()
        .next()
}

pub(crate) fn load_font_stack_bytes(
    workspace_root: &Path,
    configured: Option<&str>,
    candidates: &[PathBuf],
    max_fonts: usize,
) -> Vec<(String, Vec<u8>)> {
    let max_fonts = max_fonts.min(MAX_FONT_STACK_FONTS);
    if max_fonts == 0 {
        return Vec::new();
    }

    let mut loaded = Vec::with_capacity(max_fonts);
    let mut names = Vec::with_capacity(max_fonts);
    for path in configured_font_paths(workspace_root, configured, candidates) {
        let Ok(bytes) = read_font_bytes_with_limit(&path, MAX_FONT_FILE_BYTES) else {
            continue;
        };
        if ab_glyph::FontRef::try_from_slice(&bytes).is_ok() {
            let name = unique_font_data_name(&path, &names);
            names.push(name.clone());
            loaded.push((name, bytes));
            if loaded.len() >= max_fonts {
                break;
            }
        }
    }
    loaded
}

fn read_font_bytes_with_limit(path: &Path, max_bytes: u64) -> io::Result<Vec<u8>> {
    if max_bytes == 0 {
        return std::fs::read(path);
    }

    let file = std::fs::File::open(path)?;
    let metadata = file.metadata()?;
    if metadata.is_file() && metadata.len() > max_bytes {
        return Err(font_file_size_error(path, max_bytes));
    }

    let mut reader = file.take(max_bytes.saturating_add(1));
    let capacity = if metadata.is_file() {
        usize::try_from(metadata.len().min(max_bytes.saturating_add(1))).unwrap_or(usize::MAX)
    } else {
        0
    };
    let mut bytes = Vec::with_capacity(capacity);
    reader.read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(font_file_size_error(path, max_bytes));
    }

    Ok(bytes)
}

fn font_file_size_error(path: &Path, max_bytes: u64) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!(
            "{} exceeds font file limit of {max_bytes} bytes",
            path.display()
        ),
    )
}

pub(crate) fn configured_font_paths(
    workspace_root: &Path,
    configured: Option<&str>,
    candidates: &[PathBuf],
) -> Vec<PathBuf> {
    let configured = configured.and_then(optional_setting_path_from_input);
    let mut paths = Vec::with_capacity(candidates.len() + usize::from(configured.is_some()));
    if let Some(path) = configured.as_deref() {
        push_unique_font_path(
            &mut paths,
            lexical_normalize_path(&resolve_configured_font_path(workspace_root, path)),
        );
    }
    for path in candidates {
        push_unique_font_path(&mut paths, lexical_normalize_path(path));
    }
    paths
}

fn resolve_configured_font_path(workspace_root: &Path, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        workspace_root.join(path)
    }
}

pub(crate) fn font_data_name(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("kuroya-font");
    let mut name = String::with_capacity(
        MAX_FONT_DATA_NAME_CHARS.min(FONT_DATA_NAME_PREFIX.len() + stem.len()),
    );
    name.push_str(FONT_DATA_NAME_PREFIX);
    for ch in stem.chars() {
        if name.len() >= MAX_FONT_DATA_NAME_CHARS {
            break;
        }
        name.push(if ch.is_ascii_alphanumeric() {
            ch.to_ascii_lowercase()
        } else {
            '_'
        });
    }
    name
}

fn unique_font_data_name(path: &Path, existing: &[String]) -> String {
    let base = font_data_name(path);
    if !existing.iter().any(|name| name == &base) {
        return base;
    }

    for suffix in 2usize.. {
        let candidate = suffixed_font_data_name(&base, suffix);
        if !existing.iter().any(|name| name == &candidate) {
            return candidate;
        }
    }
    unreachable!("unbounded suffix search always returns")
}

fn suffixed_font_data_name(base: &str, suffix: usize) -> String {
    let suffix = format!("_{suffix}");
    let mut name = base.to_owned();
    if name.len().saturating_add(suffix.len()) > MAX_FONT_DATA_NAME_CHARS {
        name.truncate(MAX_FONT_DATA_NAME_CHARS.saturating_sub(suffix.len()));
    }
    name.push_str(&suffix);
    name
}

fn push_unique_font_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.contains(&path) {
        paths.push(path);
    }
}

fn lexical_normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    let mut has_root = false;
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => {
                has_root = true;
                normalized.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                let can_pop_normal = normalized
                    .components()
                    .next_back()
                    .is_some_and(|component| matches!(component, Component::Normal(_)));
                if can_pop_normal {
                    normalized.pop();
                } else if !has_root {
                    normalized.push("..");
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }

    if normalized.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        normalized
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_font_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "kuroya-font-loading-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn font_reader_rejects_files_over_limit() {
        let path = temp_font_path("oversized.ttf");
        fs::write(&path, b"abcde").unwrap();

        assert_eq!(read_font_bytes_with_limit(&path, 5).unwrap(), b"abcde");
        let error = read_font_bytes_with_limit(&path, 4).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("font file limit"));

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn font_reader_zero_limit_reads_without_size_rejection() {
        let path = temp_font_path("zero-limit.ttf");
        fs::write(&path, b"abcde").unwrap();

        assert_eq!(read_font_bytes_with_limit(&path, 0).unwrap(), b"abcde");

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn font_reader_non_file_paths_are_not_reported_as_size_errors() {
        let path = temp_font_path("directory.ttf");
        fs::create_dir(&path).unwrap();

        let error = read_font_bytes_with_limit(&path, 4).unwrap_err();

        assert_ne!(error.kind(), io::ErrorKind::InvalidData);
        assert!(!error.to_string().contains("font file limit"));

        fs::remove_dir(path).unwrap();
    }

    #[test]
    fn font_reader_keeps_symlink_to_file_size_checks() {
        let target = temp_font_path("symlink-target.ttf");
        let link = temp_font_path("symlink-link.ttf");
        fs::write(&target, b"abcde").unwrap();
        if create_file_symlink(&target, &link).is_err() {
            fs::remove_file(target).unwrap();
            return;
        }

        assert_eq!(read_font_bytes_with_limit(&link, 5).unwrap(), b"abcde");
        let error = read_font_bytes_with_limit(&link, 4).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("font file limit"));

        fs::remove_file(link).unwrap();
        fs::remove_file(target).unwrap();
    }

    #[test]
    fn load_font_bytes_skips_oversized_configured_file() {
        let path = temp_font_path("configured.ttf");
        fs::write(
            &path,
            vec![b'a'; usize::try_from(MAX_FONT_FILE_BYTES + 1).unwrap()],
        )
        .unwrap();

        let loaded = load_font_bytes(Path::new("."), path.to_str(), &[]);

        assert!(loaded.is_none());

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn load_font_stack_bytes_honors_zero_limit() {
        let path = temp_font_path("configured.ttf");
        fs::write(&path, b"not a font").unwrap();

        assert!(load_font_stack_bytes(Path::new("."), path.to_str(), &[], 0).is_empty());

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn unique_font_data_names_keep_same_stem_fonts_distinct() {
        let first = Path::new("fonts/Editor.ttf");
        let second = Path::new("backup/Editor.ttf");
        let first_name = unique_font_data_name(first, &[]);
        let second_name = unique_font_data_name(second, std::slice::from_ref(&first_name));

        assert_eq!(first_name, "kuroya_editor");
        assert_eq!(second_name, "kuroya_editor_2");
    }

    #[test]
    fn font_data_names_are_sanitized_and_bounded() {
        let path = PathBuf::from(format!("{}\u{202e}.ttf", "Font-".repeat(80)));
        let name = font_data_name(&path);
        let duplicate = unique_font_data_name(&path, std::slice::from_ref(&name));

        assert!(name.starts_with("kuroya_font_"));
        assert!(!name.contains('\u{202e}'));
        assert!(name.chars().count() <= MAX_FONT_DATA_NAME_CHARS);
        assert!(duplicate.ends_with("_2"));
        assert!(duplicate.chars().count() <= MAX_FONT_DATA_NAME_CHARS);
    }

    #[test]
    fn configured_font_paths_dedupes_lexically_equivalent_paths() {
        let root = PathBuf::from("workspace");
        let candidate = root.join("fonts").join("Editor.ttf");
        let paths = configured_font_paths(
            &root,
            Some("fonts/../fonts/Editor.ttf"),
            &[
                candidate.clone(),
                root.join("fonts").join("Fallback.ttf"),
                candidate,
            ],
        );

        assert_eq!(
            paths,
            vec![
                root.join("fonts").join("Editor.ttf"),
                root.join("fonts").join("Fallback.ttf")
            ]
        );
    }

    #[test]
    fn configured_font_paths_reject_unsafe_configured_path() {
        let root = PathBuf::from("workspace");
        let candidate = root.join("fonts").join("Fallback.ttf");

        assert_eq!(
            configured_font_paths(
                &root,
                Some("fonts/\u{202e}Editor.ttf"),
                std::slice::from_ref(&candidate)
            ),
            vec![candidate.clone()]
        );
        assert_eq!(
            configured_font_paths(
                &root,
                Some("fonts/Editor\u{2028}.ttf"),
                std::slice::from_ref(&candidate)
            ),
            vec![candidate]
        );
    }

    #[test]
    fn lexical_normalize_path_preserves_stacked_relative_parents() {
        let stacked = PathBuf::from("..").join("..");
        assert_eq!(lexical_normalize_path(&stacked), stacked);

        let stacked_file = PathBuf::from("..").join("..").join("x");
        assert_eq!(lexical_normalize_path(&stacked_file), stacked_file);

        assert_eq!(
            lexical_normalize_path(
                &PathBuf::from("a")
                    .join("..")
                    .join("..")
                    .join("..")
                    .join("b")
            ),
            PathBuf::from("..").join("..").join("b")
        );
    }

    #[test]
    fn configured_font_paths_preserve_escaped_relative_setting() {
        let configured = PathBuf::from("..")
            .join("..")
            .join("fonts")
            .join("Editor.ttf");
        let paths = configured_font_paths(Path::new("."), configured.to_str(), &[]);

        assert_eq!(paths, vec![configured]);
    }

    #[test]
    fn configured_font_paths_do_not_dedupe_stacked_parent_candidates() {
        let escaped = PathBuf::from("..")
            .join("..")
            .join("fonts")
            .join("Editor.ttf");
        let local = PathBuf::from("fonts").join("Editor.ttf");
        let paths = configured_font_paths(Path::new("."), None, &[escaped.clone(), local.clone()]);

        assert_eq!(paths, vec![escaped, local]);
    }

    #[cfg(unix)]
    fn create_file_symlink(target: &Path, link: &Path) -> io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    #[cfg(windows)]
    fn create_file_symlink(target: &Path, link: &Path) -> io::Result<()> {
        std::os::windows::fs::symlink_file(target, link)
    }
}
