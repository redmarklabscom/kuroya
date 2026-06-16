use crate::{
    path_display::sanitized_display_label_cow, ui_text::count_label,
    workspace_trust::workspace_path_contains_lexically,
};
use kuroya_core::{SearchMatch, SearchResult, SearchStats};
use std::{
    borrow::Cow,
    collections::HashSet,
    fmt::Write as _,
    path::{Component, Path, PathBuf},
};

pub(super) const MAX_RESULT_PATH_CHARS: usize = 96;
pub(super) const MAX_RESULT_PREVIEW_CHARS: usize = 160;
pub(super) const MAX_QUERY_LABEL_CHARS: usize = 48;
pub(super) const MAX_PROJECT_SEARCH_SUMMARY_FILE_SCAN: usize = 512;

pub(super) fn project_search_empty_stats_detail(stats: SearchStats) -> Option<String> {
    let skipped_files = stats.skipped_files();
    match (stats.searched_files, skipped_files) {
        (0, 0) => None,
        (searched_files, 0) => Some(count_label(
            searched_files,
            "file searched",
            "files searched",
        )),
        (0, skipped_files) => {
            let skipped_label = count_label(skipped_files, "file", "files");
            let mut detail = String::with_capacity(skipped_label.len() + 8);
            let _ = write!(detail, "{} skipped", skipped_label);
            Some(detail)
        }
        (searched_files, skipped_files) => {
            let mut detail = count_label(searched_files, "file searched", "files searched");
            let _ = write!(
                detail,
                ", {} skipped",
                count_label(skipped_files, "file", "files")
            );
            Some(detail)
        }
    }
}

pub(super) fn project_search_result_summary(
    result: &SearchResult,
    results_match_query: bool,
) -> Option<String> {
    if result.matches.is_empty() {
        return None;
    }

    let matched_files = project_search_visible_matched_file_count(result);
    let matches_label = count_label(result.matches.len(), "match", "matches");
    let files_label = project_search_matched_files_label(matched_files);
    let mut summary = String::with_capacity(matches_label.len() + files_label.len() + 48);
    let _ = write!(summary, "{} in {}", matches_label, files_label);
    if result.stats.searched_files > 0 {
        let _ = write!(
            summary,
            " from {}",
            count_label(
                result.stats.searched_files,
                "file searched",
                "files searched"
            )
        );
    }
    let skipped_files = result.stats.skipped_files();
    if skipped_files > 0 {
        let _ = write!(
            summary,
            " - {} skipped",
            count_label(skipped_files, "file", "files")
        );
    }
    if result.truncated {
        summary.push_str(" - truncated");
    }
    if !results_match_query {
        summary.push_str(" - previous search");
    }
    Some(summary)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ProjectSearchMatchedFileCount {
    pub(super) count: usize,
    pub(super) exact: bool,
}

impl ProjectSearchMatchedFileCount {
    fn exact(count: usize) -> Self {
        Self { count, exact: true }
    }

    fn lower_bound(count: usize) -> Self {
        Self {
            count,
            exact: false,
        }
    }
}

pub(super) fn project_search_matched_files_label(
    matched_files: ProjectSearchMatchedFileCount,
) -> String {
    let files_label = count_label(matched_files.count, "file", "files");
    if matched_files.exact {
        return files_label;
    }

    let mut label = String::with_capacity(files_label.len() + "at least ".len());
    label.push_str("at least ");
    label.push_str(&files_label);
    label
}

pub(super) fn project_search_visible_matched_file_count(
    result: &SearchResult,
) -> ProjectSearchMatchedFileCount {
    if result.stats.matched_files > 0 {
        return ProjectSearchMatchedFileCount::exact(result.stats.matched_files);
    }

    let scanned_matches = result
        .matches
        .len()
        .min(MAX_PROJECT_SEARCH_SUMMARY_FILE_SCAN);
    let mut visible_paths = HashSet::with_capacity(scanned_matches);
    for result_match in result.matches.iter().take(scanned_matches) {
        visible_paths.insert(result_match.path.as_path());
    }
    let count = visible_paths.len().max(1);
    if scanned_matches < result.matches.len() {
        ProjectSearchMatchedFileCount::lower_bound(count)
    } else {
        ProjectSearchMatchedFileCount::exact(count)
    }
}

#[cfg(test)]
pub(super) fn project_search_visible_matched_files(result: &SearchResult) -> usize {
    project_search_visible_matched_file_count(result).count
}

#[cfg(test)]
pub(super) fn project_search_result_row_label(
    workspace_root: &Path,
    result_match: &SearchMatch,
) -> String {
    let path = project_search_result_path_label_cow(workspace_root, &result_match.path);
    let preview = project_search_result_preview_label_cow(&result_match.preview);
    project_search_result_row_label_with_display_fragments(
        result_match,
        path.as_ref(),
        preview.as_ref(),
    )
}

pub(super) fn project_search_result_row_label_with_display_fragments(
    result_match: &SearchMatch,
    path_label: &str,
    preview_label: &str,
) -> String {
    let mut label = String::with_capacity(project_search_result_row_label_capacity(
        result_match,
        path_label,
        preview_label,
    ));
    label.push_str(path_label);
    label.push(':');
    let _ = write!(label, "{}", result_match.line);
    label.push(':');
    let _ = write!(label, "{}", result_match.column);
    label.push_str("  ");
    label.push_str(preview_label);
    label
}

fn project_search_result_row_label_capacity(
    result_match: &SearchMatch,
    path_label: &str,
    preview_label: &str,
) -> usize {
    path_label.len()
        + decimal_digit_count(result_match.line)
        + decimal_digit_count(result_match.column)
        + preview_label.len()
        + ":".len()
        + ":".len()
        + "  ".len()
}

fn decimal_digit_count(mut value: usize) -> usize {
    let mut digits = 1;
    while value >= 10 {
        digits += 1;
        value /= 10;
    }
    digits
}

#[cfg(test)]
pub(super) fn project_search_result_preview_label(preview: &str) -> String {
    project_search_result_preview_label_cow(preview).into_owned()
}

pub(super) fn project_search_result_preview_label_cow(preview: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(preview, MAX_RESULT_PREVIEW_CHARS, "")
}

#[cfg(test)]
pub(super) fn project_search_result_path_label(workspace_root: &Path, path: &Path) -> String {
    project_search_result_path_label_cow(workspace_root, path).into_owned()
}

pub(super) fn project_search_result_path_label_cow<'a>(
    workspace_root: &Path,
    path: &'a Path,
) -> Cow<'a, str> {
    if let Ok(relative) = path.strip_prefix(workspace_root)
        && project_search_relative_path_is_clean(relative)
    {
        return project_search_result_path_label_from_lossy(relative.to_string_lossy());
    }

    if let Some(relative) = project_search_workspace_relative_path(workspace_root, path) {
        return Cow::Owned(
            project_search_result_path_label_from_lossy(relative.to_string_lossy()).into_owned(),
        );
    }

    project_search_result_path_label_from_lossy(path.to_string_lossy())
}

fn project_search_result_path_label_from_lossy(relative: Cow<'_, str>) -> Cow<'_, str> {
    match relative {
        Cow::Borrowed(relative) => {
            sanitized_display_label_cow(relative, MAX_RESULT_PATH_CHARS, ".")
        }
        Cow::Owned(relative) => {
            match sanitized_display_label_cow(&relative, MAX_RESULT_PATH_CHARS, ".") {
                Cow::Borrowed(borrowed)
                    if borrowed.as_ptr() == relative.as_ptr()
                        && borrowed.len() == relative.len() =>
                {
                    Cow::Owned(relative)
                }
                Cow::Borrowed(borrowed) => Cow::Owned(borrowed.to_owned()),
                Cow::Owned(label) => Cow::Owned(label),
            }
        }
    }
}

fn project_search_workspace_relative_path(workspace_root: &Path, path: &Path) -> Option<PathBuf> {
    let workspace_root = project_search_lexically_normalize_path(workspace_root);
    let path = project_search_lexically_normalize_path(path);
    if let Ok(relative) = path.strip_prefix(&workspace_root) {
        return Some(relative.to_path_buf());
    }

    if !workspace_path_contains_lexically(&workspace_root, &path) {
        return None;
    }

    let root_components = workspace_root.components().count();
    let mut relative = PathBuf::new();
    for component in path.components().skip(root_components) {
        relative.push(component.as_os_str());
    }
    Some(relative)
}

fn project_search_relative_path_is_clean(path: &Path) -> bool {
    !path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::Prefix(_) | Component::RootDir
        )
    })
}

fn project_search_lexically_normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    let mut has_root = false;
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => {
                normalized.push(prefix.as_os_str());
                has_root = false;
            }
            Component::RootDir => {
                normalized.push(component.as_os_str());
                has_root = true;
            }
            Component::CurDir => {}
            Component::ParentDir => {
                let can_pop = matches!(
                    normalized.components().next_back(),
                    Some(Component::Normal(_))
                );
                if can_pop {
                    normalized.pop();
                } else if !has_root {
                    normalized.push("..");
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
}

#[cfg(test)]
pub(super) fn project_search_query_label(query: &str) -> String {
    project_search_query_label_cow(query).into_owned()
}

pub(super) fn project_search_query_label_cow(query: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(query, MAX_QUERY_LABEL_CHARS, "search text")
}
