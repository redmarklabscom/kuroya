use crate::lsp_text_positions::buffer_position_to_lsp_utf16_column;
use crate::workspace_trust::{trusted_workspace_paths_match, workspace_path_contains_lexically};
use kuroya_core::{BufferId, TextBuffer};
use std::path::Path;

pub(crate) fn workspace_event_matches(current_root: &Path, event_root: &Path) -> bool {
    trusted_workspace_paths_match(current_root, event_root)
}

pub(crate) fn background_request_matches(request_id: u64, active_request_id: u64) -> bool {
    request_id == active_request_id
}

pub(crate) fn background_workspace_event_matches(
    current_root: &Path,
    event_root: &Path,
    request_id: u64,
    active_request_id: u64,
) -> bool {
    workspace_event_matches(current_root, event_root)
        && background_request_matches(request_id, active_request_id)
}

pub(crate) fn lsp_event_path_is_current(workspace_root: &Path, path: &Path) -> bool {
    workspace_path_contains_lexically(workspace_root, path)
}

pub(crate) fn paths_match_lexically(left: &Path, right: &Path) -> bool {
    trusted_workspace_paths_match(left, right)
}

pub(crate) fn active_buffer_path_matches(active: Option<&TextBuffer>, path: &Path) -> bool {
    active
        .and_then(TextBuffer::path)
        .is_some_and(|active_path| paths_match_lexically(active_path, path))
}

pub(crate) fn active_buffer_path_version_matches(
    active: Option<&TextBuffer>,
    path: &Path,
    version: u64,
) -> bool {
    active.is_some_and(|buffer| {
        buffer.version() == version
            && buffer
                .path()
                .is_some_and(|active_path| paths_match_lexically(active_path, path))
    })
}

pub(crate) fn active_buffer_lsp_position_matches(
    active: Option<&TextBuffer>,
    path: &Path,
    version: u64,
    line: usize,
    one_based_column: usize,
) -> bool {
    active.is_some_and(|buffer| {
        if buffer.version() != version {
            return false;
        }
        let cursor = buffer.cursor_position();
        if cursor.line != line {
            return false;
        }
        if !buffer
            .path()
            .is_some_and(|active_path| paths_match_lexically(active_path, path))
        {
            return false;
        }
        let Some(lsp_column) =
            buffer_position_to_lsp_utf16_column(buffer, cursor.line, cursor.column)
        else {
            return false;
        };
        lsp_column.saturating_add(1) == one_based_column
    })
}

pub(crate) fn buffer_id_path_version_matches(
    buffers: &[TextBuffer],
    id: BufferId,
    path: &Path,
    version: u64,
) -> bool {
    buffers.iter().any(|buffer| {
        buffer.id() == id
            && buffer.version() == version
            && buffer
                .path()
                .is_some_and(|candidate| paths_match_lexically(candidate, path))
    })
}

#[cfg(test)]
mod tests {
    use super::{
        active_buffer_lsp_position_matches, active_buffer_path_matches,
        active_buffer_path_version_matches, background_request_matches,
        background_workspace_event_matches, buffer_id_path_version_matches,
        lsp_event_path_is_current, paths_match_lexically, workspace_event_matches,
    };
    use kuroya_core::TextBuffer;
    use std::path::{Path, PathBuf};

    #[test]
    fn background_request_matches_only_active_request() {
        assert!(background_request_matches(7, 7));
        assert!(!background_request_matches(6, 7));
        assert!(!background_request_matches(8, 7));
    }

    #[test]
    fn background_workspace_event_matches_root_and_active_request() {
        let root = Path::new("workspace");
        assert!(background_workspace_event_matches(root, root, 3, 3));
        assert!(!background_workspace_event_matches(
            root,
            Path::new("other"),
            3,
            3
        ));
        assert!(!background_workspace_event_matches(root, root, 2, 3));
    }

    #[test]
    fn workspace_event_matches_lexically_equivalent_roots() {
        assert!(workspace_event_matches(
            Path::new("workspace/current"),
            Path::new("workspace/current/src/..")
        ));
        assert!(!workspace_event_matches(
            Path::new("workspace/current"),
            Path::new("workspace/current/../old")
        ));
    }

    #[test]
    fn lsp_event_path_is_current_rejects_lexical_parent_escape() {
        let root = Path::new("workspace/current");

        assert!(lsp_event_path_is_current(
            root,
            Path::new("workspace/current/src/main.rs")
        ));
        assert!(lsp_event_path_is_current(
            root,
            Path::new("workspace/current/./src/../main.rs")
        ));
        assert!(!lsp_event_path_is_current(
            root,
            Path::new("workspace/current/../old/src/main.rs")
        ));
    }

    #[test]
    fn paths_match_lexically_accepts_equivalent_paths() {
        assert!(paths_match_lexically(
            Path::new("workspace/current/src/main.rs"),
            Path::new("workspace/current/src/../src/main.rs")
        ));
        assert!(!paths_match_lexically(
            Path::new("workspace/current/src/main.rs"),
            Path::new("workspace/current/../old/src/main.rs")
        ));
    }

    #[test]
    fn active_buffer_lsp_position_matches_path_version_and_cursor() {
        let path = PathBuf::from("src/main.rs");
        let mut buffer =
            TextBuffer::from_text(1, Some(path.clone()), "let value = 1;\nvalue\n".to_owned());
        buffer.set_single_cursor(buffer.line_column_to_char(1, 5));
        let version = buffer.version();

        assert!(active_buffer_lsp_position_matches(
            Some(&buffer),
            &path,
            version,
            1,
            6
        ));
        assert!(!active_buffer_lsp_position_matches(
            Some(&buffer),
            &path,
            version,
            1,
            5
        ));
        assert!(!active_buffer_lsp_position_matches(
            Some(&buffer),
            &path,
            version,
            0,
            6
        ));
        assert!(!active_buffer_lsp_position_matches(
            Some(&buffer),
            Path::new("src/lib.rs"),
            version,
            1,
            6
        ));
        assert!(!active_buffer_lsp_position_matches(
            Some(&buffer),
            &path,
            version + 1,
            1,
            6
        ));

        buffer.set_single_cursor(buffer.line_column_to_char(0, 4));
        assert!(!active_buffer_lsp_position_matches(
            Some(&buffer),
            &path,
            version,
            1,
            6
        ));
    }

    #[test]
    fn lsp_buffer_guards_match_lexically_equivalent_paths() {
        let path = PathBuf::from("workspace/src/main.rs");
        let equivalent_path = PathBuf::from("workspace/src/./main.rs");
        let mut buffer = TextBuffer::from_text(7, Some(path), "let value = 1;\nvalue\n".to_owned());
        buffer.set_single_cursor(buffer.line_column_to_char(1, 5));
        let version = buffer.version();

        assert!(active_buffer_path_matches(Some(&buffer), &equivalent_path));
        assert!(active_buffer_path_version_matches(
            Some(&buffer),
            &equivalent_path,
            version
        ));
        assert!(active_buffer_lsp_position_matches(
            Some(&buffer),
            &equivalent_path,
            version,
            1,
            6
        ));
        assert!(buffer_id_path_version_matches(
            std::slice::from_ref(&buffer),
            7,
            &equivalent_path,
            version
        ));
    }

    #[test]
    fn active_buffer_lsp_position_matches_utf16_columns() {
        let path = PathBuf::from("src/main.rs");
        let mut buffer = TextBuffer::from_text(1, Some(path.clone()), "😀alpha\n".to_owned());
        buffer.set_single_cursor(buffer.line_column_to_char(0, 1));
        let version = buffer.version();

        assert!(active_buffer_lsp_position_matches(
            Some(&buffer),
            &path,
            version,
            0,
            3
        ));
        assert!(!active_buffer_lsp_position_matches(
            Some(&buffer),
            &path,
            version,
            0,
            2
        ));
    }
}
