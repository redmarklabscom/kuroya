use eframe::egui;
use std::path::Path;

use crate::workspace_trust::workspace_path_stays_within_root_lexically;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PathCopyKind {
    Absolute,
    Relative,
}

const PATH_COPY_STATUS_VALUE_MAX_CHARS: usize = 180;
const PATH_COPY_STATUS_TRUNCATION: &str = "...";

pub(crate) fn copy_path_to_clipboard(
    ctx: &egui::Context,
    root: &Path,
    path: &Path,
    kind: PathCopyKind,
) -> String {
    let text = path_copy_text(root, path, kind);
    let status = path_copy_status(&text, kind);
    ctx.copy_text(text);
    status
}

pub(crate) fn path_copy_text(root: &Path, path: &Path, kind: PathCopyKind) -> String {
    let target = match kind {
        PathCopyKind::Absolute => path,
        PathCopyKind::Relative => relative_path_copy_target(root, path).unwrap_or(path),
    };
    path_copy_target_text(target)
}

fn relative_path_copy_target<'a>(root: &Path, path: &'a Path) -> Option<&'a Path> {
    if !workspace_path_stays_within_root_lexically(root, path) {
        return None;
    }

    path.strip_prefix(root)
        .ok()
        .filter(|relative| !relative.as_os_str().is_empty())
}

fn path_copy_target_text(path: &Path) -> String {
    path.to_str()
        .map(str::to_owned)
        .unwrap_or_else(|| path.display().to_string())
}

pub(crate) fn path_copy_status(text: &str, kind: PathCopyKind) -> String {
    let text = path_copy_status_value(text);
    let prefix = match kind {
        PathCopyKind::Absolute => "Copied path ",
        PathCopyKind::Relative => "Copied relative path ",
    };
    let mut status = String::with_capacity(prefix.len() + text.len());
    status.push_str(prefix);
    status.push_str(&text);
    status
}

fn path_copy_status_value(text: &str) -> String {
    let value = single_line_path_copy_status_value(text, PATH_COPY_STATUS_VALUE_MAX_CHARS);
    if value.is_empty() {
        ".".to_owned()
    } else {
        value
    }
}

fn single_line_path_copy_status_value(text: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    if is_simple_path_copy_status_text(text, max_chars) {
        return text.to_owned();
    }

    let mut output = String::with_capacity(text.len().min(max_chars));
    let mut chars = 0usize;
    let mut pending_space = false;
    let mut truncated = false;

    for ch in text.trim().chars() {
        if chars >= max_chars {
            truncated = true;
            break;
        }
        if is_path_copy_status_format_control(ch) {
            continue;
        }
        if ch.is_control() || ch.is_whitespace() {
            pending_space = !output.is_empty();
            continue;
        }
        if pending_space {
            output.push(' ');
            chars += 1;
            pending_space = false;
            if chars >= max_chars {
                truncated = true;
                break;
            }
        }
        output.push(ch);
        chars += 1;
    }

    if truncated {
        path_copy_status_truncated_value(&output, max_chars)
    } else {
        output
    }
}

fn is_simple_path_copy_status_text(text: &str, max_chars: usize) -> bool {
    if text.is_empty() || text.len() > max_chars {
        return false;
    }

    let bytes = text.as_bytes();
    if bytes.first() == Some(&b' ') || bytes.last() == Some(&b' ') {
        return false;
    }

    let mut previous_space = false;
    for &byte in bytes {
        if !(b' '..=b'~').contains(&byte) {
            return false;
        }
        let is_space = byte == b' ';
        if is_space && previous_space {
            return false;
        }
        previous_space = is_space;
    }

    true
}

fn path_copy_status_truncated_value(value: &str, max_chars: usize) -> String {
    let marker_len = PATH_COPY_STATUS_TRUNCATION.chars().count();
    if max_chars <= marker_len {
        return PATH_COPY_STATUS_TRUNCATION
            .chars()
            .take(max_chars)
            .collect();
    }

    let keep = max_chars - marker_len;
    let mut output = String::with_capacity(max_chars.min(value.len()));
    output.extend(value.chars().take(keep));
    while output.ends_with(char::is_whitespace) {
        output.pop();
    }
    output.push_str(PATH_COPY_STATUS_TRUNCATION);
    output
}

fn is_path_copy_status_format_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{00ad}'
            | '\u{061c}'
            | '\u{180e}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{206f}'
            | '\u{feff}'
    )
}

#[cfg(test)]
mod tests {
    use super::{
        PATH_COPY_STATUS_VALUE_MAX_CHARS, PathCopyKind, path_copy_status, path_copy_text,
        single_line_path_copy_status_value,
    };
    use std::path::PathBuf;

    #[test]
    fn path_copy_text_supports_absolute_and_workspace_relative_paths() {
        let root = PathBuf::from("C:/repo");
        let path = root.join("src").join("main.rs");

        assert_eq!(
            path_copy_text(&root, &path, PathCopyKind::Absolute),
            path.display().to_string()
        );
        assert_eq!(
            path_copy_text(&root, &path, PathCopyKind::Relative),
            PathBuf::from("src").join("main.rs").display().to_string()
        );
    }

    #[test]
    fn path_copy_text_falls_back_to_absolute_when_outside_root() {
        let root = PathBuf::from("C:/repo");
        let path = PathBuf::from("D:/other/main.rs");

        assert_eq!(
            path_copy_text(&root, &path, PathCopyKind::Relative),
            path.display().to_string()
        );
    }

    #[test]
    fn path_copy_text_falls_back_to_raw_absolute_when_relative_path_escapes_root() {
        let root = PathBuf::from("workspace/current");
        let path = root.join("..").join("old").join("secret.rs");

        assert_eq!(
            path_copy_text(&root, &path, PathCopyKind::Relative),
            path.display().to_string()
        );
    }

    #[test]
    fn path_copy_text_preserves_raw_relative_path_when_walk_stays_inside_root() {
        let root = PathBuf::from("workspace/current");
        let path = root.join("src").join("..").join("main.rs");

        assert_eq!(
            path_copy_text(&root, &path, PathCopyKind::Relative),
            PathBuf::from("src")
                .join("..")
                .join("main.rs")
                .display()
                .to_string()
        );
    }

    #[test]
    fn path_copy_text_keeps_utf8_filename_less_paths_without_display_fallback() {
        let root = PathBuf::from("/");
        let path = PathBuf::from("/");

        assert_eq!(
            path_copy_text(&root, &path, PathCopyKind::Absolute),
            path.to_str().unwrap()
        );
        assert_eq!(
            path_copy_text(&root, &path, PathCopyKind::Relative),
            path.to_str().unwrap()
        );
    }

    #[test]
    fn path_copy_status_names_absolute_and_relative_actions() {
        assert_eq!(
            path_copy_status("C:/repo/src/main.rs", PathCopyKind::Absolute),
            "Copied path C:/repo/src/main.rs"
        );
        assert_eq!(
            path_copy_status("C:/repo/My File.rs", PathCopyKind::Absolute),
            "Copied path C:/repo/My File.rs"
        );
        assert_eq!(
            path_copy_status("src/main.rs", PathCopyKind::Relative),
            "Copied relative path src/main.rs"
        );
    }

    #[test]
    fn path_copy_status_value_keeps_clean_paths_and_sanitizes_spacing() {
        assert_eq!(
            single_line_path_copy_status_value(
                "C:/repo/My File.rs",
                PATH_COPY_STATUS_VALUE_MAX_CHARS
            ),
            "C:/repo/My File.rs"
        );
        assert_eq!(
            single_line_path_copy_status_value(
                " C:/repo/My  File.rs ",
                PATH_COPY_STATUS_VALUE_MAX_CHARS
            ),
            "C:/repo/My File.rs"
        );
        assert_eq!(
            single_line_path_copy_status_value(
                " \nC:/repo/My File.rs\t ",
                PATH_COPY_STATUS_VALUE_MAX_CHARS
            ),
            "C:/repo/My File.rs"
        );
    }

    #[test]
    fn path_copy_status_is_single_line_bounded_and_sanitized() {
        let status = path_copy_status(
            &format!(" C:/repo/src/\u{202e}\nmain.rs{}", "x".repeat(220)),
            PathCopyKind::Absolute,
        );

        assert!(status.starts_with("Copied path C:/repo/src/ main.rs"));
        assert!(status.ends_with("..."));
        assert!(!status.contains('\n'));
        assert!(!status.contains('\u{202e}'));
        assert!(
            status.strip_prefix("Copied path ").unwrap().chars().count()
                <= PATH_COPY_STATUS_VALUE_MAX_CHARS
        );
        assert_eq!(
            path_copy_status("\u{202e}\n", PathCopyKind::Relative),
            "Copied relative path ."
        );
    }

    #[test]
    fn path_copy_status_strips_hidden_format_controls_without_rewriting_clipboard_text() {
        let unsafe_path =
            "src/\u{00ad}\u{200b}alpha\u{200c}\u{200d}beta\u{180e}\u{feff}\u{2066}gamma\u{2069}.rs";

        assert_eq!(
            path_copy_status(unsafe_path, PathCopyKind::Relative),
            "Copied relative path src/alphabetagamma.rs"
        );
    }

    #[test]
    fn path_copy_status_strips_full_invisible_format_control_block() {
        let unsafe_path =
            "src/\u{2060}alpha\u{206a}\u{206b}beta\u{206c}\u{206d}\u{206e}\u{206f}.rs";

        assert_eq!(
            path_copy_status(unsafe_path, PathCopyKind::Relative),
            "Copied relative path src/alphabeta.rs"
        );
    }

    #[test]
    fn path_copy_text_preserves_raw_clipboard_text_for_unsafe_paths() {
        let root = PathBuf::from("C:/repo");
        let path = root
            .join("src")
            .join("unsafe\u{00ad}\u{180e}\u{200b}\u{202e}\u{feff}name.rs");

        assert_eq!(
            path_copy_text(&root, &path, PathCopyKind::Relative),
            PathBuf::from("src")
                .join("unsafe\u{00ad}\u{180e}\u{200b}\u{202e}\u{feff}name.rs")
                .display()
                .to_string()
        );
    }
}
