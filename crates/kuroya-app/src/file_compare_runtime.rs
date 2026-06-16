use crate::{
    KuroyaApp,
    file_runtime::file_path_open_buffer_or_known_openable,
    path_display::{display_path_label_cow, sanitized_display_label_cow},
    virtual_diff_runtime::{VirtualDiffOpenJob, text_snapshot_contains_nul},
    workspace_state::paths_match_exact_or_lexically,
};
use kuroya_core::{BufferId, TextBuffer, TextSnapshot};
use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

const FILE_COMPARE_STATUS_MAX_CHARS: usize = 240;

impl KuroyaApp {
    pub(crate) fn select_active_file_for_compare(&mut self) {
        let Some(path) = self.active_file_or_diff_source_path("select for compare") else {
            return;
        };
        self.select_file_for_compare(path);
    }

    pub(crate) fn compare_active_file_with_selected(&mut self) {
        let Some(path) = self.active_file_or_diff_source_path("compare with selected") else {
            return;
        };
        self.compare_file_with_selected(path);
    }

    pub(crate) fn compare_active_file_with_saved(&mut self) {
        let Some(id) = self.active else {
            self.status = "No active file to compare with saved".to_owned();
            return;
        };
        self.open_buffer_saved_comparison(id);
    }

    pub(crate) fn select_file_for_compare(&mut self, path: PathBuf) {
        if !self.file_compare_target_is_available(&path) {
            self.explorer_compare_path = None;
            self.status = unavailable_file_compare_status(&path);
            return;
        }

        self.status = selected_file_compare_status(&path);
        self.explorer_compare_path = Some(path);
    }

    pub(crate) fn compare_file_with_selected(&mut self, path: PathBuf) {
        let Some(base_path) = self.explorer_compare_path.clone() else {
            self.status = "No selected file to compare".to_owned();
            return;
        };
        if !self.file_compare_target_is_available(&base_path) {
            self.explorer_compare_path = None;
            self.status = stale_selected_file_compare_status(&base_path);
            return;
        }

        if file_compare_paths_match(&base_path, &path) {
            self.status = same_file_compare_status(&path);
            return;
        }

        if !self.file_compare_target_is_available(&path) {
            self.status = unavailable_file_compare_status(&path);
            return;
        }

        if let Some(id) = self.file_comparison_buffer_id(&base_path, &path) {
            self.set_active_buffer(id);
            let base_label = display_path_label_cow(&base_path);
            let target_label = display_path_label_cow(&path);
            self.status =
                file_compare_already_open_status(base_label.as_ref(), target_label.as_ref());
            return;
        }
        self.open_file_comparison(base_path, path);
    }

    pub(crate) fn open_file_comparison(&mut self, base_path: PathBuf, path: PathBuf) {
        let comparison = match self.prepare_file_comparison(base_path, path) {
            Ok(comparison) => comparison,
            Err(status) => {
                self.status = status;
                return;
            }
        };
        self.open_prepared_file_comparison(comparison);
    }

    fn open_prepared_file_comparison(&mut self, comparison: PreparedFileComparison) {
        let PreparedFileComparison { base, target } = comparison;
        self.spawn_virtual_diff_open(VirtualDiffOpenJob::file_compare_with_snapshots(
            base.raw_path,
            target.raw_path,
            base.text,
            target.text,
        ));
    }

    pub(crate) fn swap_active_diff_sides(&mut self) {
        let Some(id) = self.active else {
            self.status = "No active comparison to swap".to_owned();
            return;
        };
        self.swap_diff_sides(id);
    }

    pub(crate) fn swap_diff_sides(&mut self, id: BufferId) {
        let Some(source) = self.diff_buffer_sources.get(&id) else {
            self.status = "No comparison to swap".to_owned();
            return;
        };
        let Some(base_path) = source.base_path.clone() else {
            self.status = "Only file comparisons can swap sides".to_owned();
            return;
        };
        let path = source.path.clone();
        self.open_file_comparison(path, base_path);
    }

    pub(crate) fn open_buffer_saved_comparison(&mut self, id: BufferId) {
        let max_bytes = self.diff_options().max_file_size_bytes;
        let Some(input) = self
            .buffer(id)
            .map(|buffer| saved_compare_working_snapshot(buffer, max_bytes))
        else {
            self.status = "No file-backed buffer to compare with saved".to_owned();
            return;
        };
        let Some((path, working_text)) = input.unwrap_or_else(|status| {
            self.status = status;
            None
        }) else {
            return;
        };
        self.spawn_virtual_diff_open(VirtualDiffOpenJob::saved_compare(id, path, working_text));
    }

    pub(crate) fn saved_diff_buffer_id(
        &self,
        saved_buffer_id: Option<BufferId>,
        path: &Path,
    ) -> Option<BufferId> {
        saved_buffer_id
            .filter(|id| {
                self.buffer(*id)
                    .and_then(|buffer| buffer.path())
                    .is_some_and(|candidate| file_compare_paths_match(candidate, path))
            })
            .or_else(|| self.buffer_by_lexical_path(path).map(|buffer| buffer.id()))
    }

    fn prepare_file_comparison(
        &self,
        base_path: PathBuf,
        path: PathBuf,
    ) -> Result<PreparedFileComparison, String> {
        if file_compare_paths_match(&base_path, &path) {
            return Err(same_file_compare_status(&path));
        }

        let base = self.prepare_file_compare_target(base_path)?;
        let target = self.prepare_file_compare_target(path)?;
        Ok(PreparedFileComparison { base, target })
    }

    fn prepare_file_compare_target(
        &self,
        path: PathBuf,
    ) -> Result<PreparedFileCompareTarget, String> {
        prepare_file_compare_target(&self.buffers, self.index.files(), path, Path::is_file)
    }

    fn file_compare_target_is_available(&self, path: &Path) -> bool {
        file_path_open_buffer_or_known_openable(
            &self.buffers,
            self.index.files(),
            path,
            Path::is_file,
        )
    }

    fn file_comparison_buffer_id(&self, base_path: &Path, path: &Path) -> Option<BufferId> {
        self.diff_buffer_sources.iter().find_map(|(id, source)| {
            file_compare_source_matches(source, base_path, path).then_some(*id)
        })
    }
}

fn selected_file_compare_status(path: &Path) -> String {
    let label = display_path_label_cow(path);
    selected_file_compare_status_for_label(label.as_ref())
}

fn selected_file_compare_status_for_label(label: &str) -> String {
    file_compare_status_text_owned(format!("Selected {label} for compare"))
}

fn same_file_compare_status(path: &Path) -> String {
    let path_label = display_path_label_cow(path);
    file_compare_status_text_owned(format!(
        "Select a different file to compare with {}",
        path_label.as_ref()
    ))
}

fn stale_selected_file_compare_status(path: &Path) -> String {
    let path_label = display_path_label_cow(path);
    file_compare_status_text_owned(format!(
        "Selected file is no longer available for compare: {}",
        path_label.as_ref()
    ))
}

fn unavailable_file_compare_status(path: &Path) -> String {
    let path_label = display_path_label_cow(path);
    file_compare_status_text_owned(format!(
        "Cannot compare {}: file is not available",
        path_label.as_ref()
    ))
}

fn file_compare_already_open_status(base_label: &str, target_label: &str) -> String {
    file_compare_status_text_owned(format!(
        "Comparison already open for {base_label} and {target_label}"
    ))
}

fn saved_compare_too_large_status(path: &Path, max_bytes: usize) -> String {
    let path_label = display_path_label_cow(path);
    file_compare_status_text_owned(format!(
        "{} is larger than {} bytes",
        path_label.as_ref(),
        max_bytes
    ))
}

fn saved_compare_binary_status(path: &Path) -> String {
    let path_label = display_path_label_cow(path);
    file_compare_status_text_owned(format!(
        "{} is binary and cannot be compared with saved",
        path_label.as_ref()
    ))
}

#[cfg(test)]
fn file_compare_status_text(value: impl AsRef<str>) -> String {
    file_compare_status_text_cow(value.as_ref()).into_owned()
}

fn file_compare_status_text_owned(value: String) -> String {
    let sanitized = {
        let raw = value.as_str();
        match file_compare_status_text_cow(raw) {
            Cow::Borrowed(label) => {
                let borrowed_original =
                    !raw.is_empty() && label.as_ptr() == raw.as_ptr() && label.len() == raw.len();
                if borrowed_original {
                    None
                } else {
                    Some(label.to_owned())
                }
            }
            Cow::Owned(label) => Some(label),
        }
    };

    sanitized.unwrap_or(value)
}

fn file_compare_status_text_cow(value: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(
        value,
        FILE_COMPARE_STATUS_MAX_CHARS,
        "File compare status unavailable",
    )
}

struct PreparedFileComparison {
    base: PreparedFileCompareTarget,
    target: PreparedFileCompareTarget,
}

struct PreparedFileCompareTarget {
    raw_path: PathBuf,
    text: Option<TextSnapshot>,
}

fn prepare_file_compare_target(
    buffers: &[TextBuffer],
    indexed_files: &[PathBuf],
    path: PathBuf,
    path_is_file: impl FnOnce(&Path) -> bool,
) -> Result<PreparedFileCompareTarget, String> {
    let text = file_compare_open_buffer_snapshot(buffers, &path);
    if text.is_none()
        && !file_path_open_buffer_or_known_openable(buffers, indexed_files, &path, path_is_file)
    {
        return Err(unavailable_file_compare_status(&path));
    }
    Ok(PreparedFileCompareTarget {
        raw_path: path,
        text,
    })
}

fn file_compare_open_buffer_snapshot(buffers: &[TextBuffer], path: &Path) -> Option<TextSnapshot> {
    buffers
        .iter()
        .find(|buffer| buffer.path().is_some_and(|candidate| candidate == path))
        .or_else(|| {
            buffers.iter().find(|buffer| {
                buffer
                    .path()
                    .is_some_and(|candidate| file_compare_paths_match(candidate, path))
            })
        })
        .map(TextBuffer::text_snapshot)
}

fn file_compare_paths_match(left: &Path, right: &Path) -> bool {
    paths_match_exact_or_lexically(left, right)
}

fn file_compare_source_matches(
    source: &crate::git_diff_state::DiffBufferSource,
    base_path: &Path,
    path: &Path,
) -> bool {
    if source.hunk_stage.is_some() || source.saved_buffer_id.is_some() {
        return false;
    }

    source.base_path.as_ref().is_some_and(|source_base| {
        file_compare_paths_match(source_base, base_path)
            && file_compare_paths_match(&source.path, path)
    })
}

pub(crate) fn saved_compare_working_snapshot(
    buffer: &TextBuffer,
    max_bytes: usize,
) -> Result<Option<(PathBuf, TextSnapshot)>, String> {
    let Some(path) = buffer.path() else {
        return Ok(None);
    };
    let working_bytes = buffer.len_bytes();
    if max_bytes > 0 && working_bytes > max_bytes {
        return Err(saved_compare_too_large_status(path, max_bytes));
    }
    let snapshot = buffer.text_snapshot();
    if text_snapshot_contains_nul(&snapshot) {
        return Err(saved_compare_binary_status(path));
    }
    Ok(Some((path.clone(), snapshot)))
}

#[cfg(test)]
mod tests {
    use super::{
        FILE_COMPARE_STATUS_MAX_CHARS, file_compare_already_open_status, file_compare_paths_match,
        file_compare_source_matches, file_compare_status_text, file_compare_status_text_cow,
        file_compare_status_text_owned, prepare_file_compare_target, same_file_compare_status,
        saved_compare_binary_status, saved_compare_too_large_status,
        saved_compare_working_snapshot, selected_file_compare_status,
        stale_selected_file_compare_status, unavailable_file_compare_status,
    };
    use crate::git_diff_state::DiffBufferSource;
    use crate::path_display::{DISPLAY_PATH_LABEL_MAX_CHARS, display_path_label_cow};
    use kuroya_core::{GitChangeStage, TextBuffer};
    use std::{
        borrow::Cow,
        path::{Path, PathBuf},
    };

    #[test]
    fn file_compare_statuses_preserve_normal_wording() {
        let path = Path::new("workspace/src/main.rs");

        assert_eq!(
            selected_file_compare_status(path),
            "Selected main.rs for compare"
        );
        assert_eq!(
            same_file_compare_status(path),
            "Select a different file to compare with main.rs"
        );
        assert_eq!(
            stale_selected_file_compare_status(path),
            "Selected file is no longer available for compare: main.rs"
        );
        assert_eq!(
            unavailable_file_compare_status(path),
            "Cannot compare main.rs: file is not available"
        );
        assert_eq!(
            file_compare_already_open_status("base.rs", "target.rs"),
            "Comparison already open for base.rs and target.rs"
        );
        assert_eq!(
            saved_compare_too_large_status(path, 3),
            "main.rs is larger than 3 bytes"
        );
        assert_eq!(
            saved_compare_binary_status(path),
            "main.rs is binary and cannot be compared with saved"
        );
    }

    #[test]
    fn file_compare_status_text_cow_borrows_clean_ascii_and_unicode() {
        assert!(matches!(
            file_compare_status_text_cow("Selected main.rs for compare"),
            Cow::Borrowed("Selected main.rs for compare")
        ));

        let unicode = "Selected clean-\u{03bb}.rs for compare";
        match file_compare_status_text_cow(unicode) {
            Cow::Borrowed(label) => assert_eq!(label, unicode),
            Cow::Owned(label) => panic!("expected borrowed status, got {label:?}"),
        }
    }

    #[test]
    fn file_compare_status_text_cow_owns_dirty_truncated_and_fallback_values() {
        let dirty = file_compare_status_text_cow("alpha\nbeta\u{202e}gamma");
        assert_eq!(dirty.as_ref(), "alpha betagamma");
        assert!(matches!(dirty, Cow::Owned(_)));

        let long = format!("status-{}", "x".repeat(FILE_COMPARE_STATUS_MAX_CHARS * 2));
        let truncated = file_compare_status_text_cow(&long);
        assert!(truncated.contains("..."));
        assert!(truncated.chars().count() <= FILE_COMPARE_STATUS_MAX_CHARS);
        assert!(matches!(truncated, Cow::Owned(_)));

        let fallback = file_compare_status_text_cow("\n\u{202e}\u{2066}");
        assert_eq!(fallback.as_ref(), "File compare status unavailable");
        assert!(matches!(fallback, Cow::Owned(_)));
    }

    #[test]
    fn file_compare_status_text_wrappers_match_cow_output() {
        let clean = "Selected main.rs for compare";
        assert_eq!(
            file_compare_status_text(clean),
            file_compare_status_text_cow(clean).into_owned()
        );

        let dirty = "alpha\nbeta";
        assert_eq!(
            file_compare_status_text(dirty),
            file_compare_status_text_cow(dirty).into_owned()
        );

        let owned_clean = "Comparison already open for base.rs and target.rs".to_owned();
        let owned_clean_ptr = owned_clean.as_ptr();
        let owned_status = file_compare_status_text_owned(owned_clean);
        assert_eq!(
            owned_status,
            "Comparison already open for base.rs and target.rs"
        );
        assert_eq!(owned_status.as_ptr(), owned_clean_ptr);

        assert_eq!(
            file_compare_status_text_owned("alpha\nbeta".to_owned()),
            file_compare_status_text("alpha\nbeta")
        );
    }

    #[test]
    fn compare_statuses_sanitize_and_bound_path_labels() {
        let path = Path::new("workspace").join(format!(
            "bad\n{}\u{202e}tail.rs",
            "very-long-component-".repeat(16)
        ));

        let selected = selected_file_compare_status(&path);
        let same_file = same_file_compare_status(&path);

        assert!(selected.starts_with("Selected bad "));
        assert!(!selected.contains('\n'));
        assert!(!selected.contains('\u{202e}'));
        assert!(selected.contains("..."));
        assert!(
            selected.chars().count()
                <= "Selected ".chars().count()
                    + DISPLAY_PATH_LABEL_MAX_CHARS
                    + " for compare".chars().count()
        );

        assert!(same_file.starts_with("Select a different file to compare with bad "));
        assert!(!same_file.contains('\n'));
        assert!(!same_file.contains('\u{202e}'));
        assert!(same_file.contains("..."));
        assert!(
            same_file.chars().count()
                <= "Select a different file to compare with ".chars().count()
                    + DISPLAY_PATH_LABEL_MAX_CHARS
        );
    }

    #[test]
    fn saved_compare_working_snapshot_checks_size_before_cloning_text() {
        let buffer = TextBuffer::from_text(
            1,
            Some(PathBuf::from("workspace/src/large.rs")),
            "abcdef".to_owned(),
        );

        let error = saved_compare_working_snapshot(&buffer, 3).unwrap_err();

        assert!(error.contains("large.rs"));
        assert!(error.contains("3 bytes"));
    }

    #[test]
    fn saved_compare_working_snapshot_rejects_binary_open_buffer() {
        let buffer = TextBuffer::from_text(
            1,
            Some(PathBuf::from("workspace/src/binary.dat")),
            "binary\0text".to_owned(),
        );

        let error = saved_compare_working_snapshot(&buffer, 128).unwrap_err();

        assert!(error.contains("binary.dat"));
        assert!(error.contains("binary"));
        assert!(error.chars().count() <= FILE_COMPARE_STATUS_MAX_CHARS);
    }

    #[test]
    fn saved_compare_working_snapshot_size_error_sanitizes_and_bounds_path_label() {
        let path = Path::new("workspace").join(format!(
            "huge\n{}\u{2066}tail.rs",
            "very-long-component-".repeat(16)
        ));
        let buffer = TextBuffer::from_text(1, Some(path), "abcdef".to_owned());

        let error = saved_compare_working_snapshot(&buffer, 3).unwrap_err();

        assert!(error.starts_with("huge "));
        assert!(!error.contains('\n'));
        assert!(!error.contains('\u{2066}'));
        assert!(error.contains("..."));
        assert!(error.ends_with(" is larger than 3 bytes"));
        assert!(
            error.chars().count()
                <= DISPLAY_PATH_LABEL_MAX_CHARS + " is larger than 3 bytes".chars().count()
        );
    }

    #[test]
    fn unavailable_compare_statuses_sanitize_and_bound_path_labels() {
        let path = Path::new("workspace").join(format!(
            "gone\n{}\u{2066}tail.rs",
            "very-long-component-".repeat(16)
        ));

        let unavailable = unavailable_file_compare_status(&path);
        let stale = stale_selected_file_compare_status(&path);

        assert!(unavailable.starts_with("Cannot compare gone "));
        assert!(!unavailable.contains('\n'));
        assert!(!unavailable.contains('\u{2066}'));
        assert!(unavailable.contains("..."));
        assert!(
            unavailable.chars().count()
                <= "Cannot compare ".chars().count()
                    + DISPLAY_PATH_LABEL_MAX_CHARS
                    + ": file is not available".chars().count()
        );

        assert!(stale.starts_with("Selected file is no longer available for compare: gone "));
        assert!(!stale.contains('\n'));
        assert!(!stale.contains('\u{2066}'));
        assert!(stale.contains("..."));
        assert!(
            stale.chars().count()
                <= "Selected file is no longer available for compare: "
                    .chars()
                    .count()
                    + DISPLAY_PATH_LABEL_MAX_CHARS
        );
    }

    #[test]
    fn prepare_file_compare_target_preserves_raw_path_while_preparing_safe_label() {
        let raw_path = Path::new("workspace").join(format!(
            "raw\n{}\u{202e}tail.rs",
            "very-long-component-".repeat(8)
        ));
        let buffer = TextBuffer::from_text(1, Some(raw_path.clone()), "abc".to_owned());

        let target = prepare_file_compare_target(&[buffer], &[], raw_path.clone(), |_| false)
            .expect("open buffer target");

        assert_eq!(target.raw_path, raw_path);
        assert_eq!(target.text.expect("snapshot").text(), "abc");
        let label = display_path_label_cow(&target.raw_path);
        assert!(label.starts_with("raw "));
        assert!(!label.contains('\n'));
        assert!(!label.contains('\u{202e}'));
        let status = selected_file_compare_status(&target.raw_path);
        assert!(status.starts_with("Selected raw "));
        assert!(!status.contains('\n'));
        assert!(!status.contains('\u{202e}'));
        assert_eq!(target.raw_path, raw_path);
    }

    #[test]
    fn prepare_file_compare_target_uses_lexical_open_buffer_without_rewriting_path() {
        let buffer_path = PathBuf::from("workspace/src/main.rs");
        let request_path = PathBuf::from("workspace/src/../src/main.rs");
        let buffer = TextBuffer::from_text(1, Some(buffer_path), "abc".to_owned());

        let target = prepare_file_compare_target(&[buffer], &[], request_path.clone(), |_| false)
            .expect("lexical open buffer target");

        assert_eq!(target.raw_path, request_path);
        assert_eq!(target.text.expect("snapshot").text(), "abc");
    }

    #[test]
    fn prepare_file_compare_target_accepts_indexed_path_without_filesystem_probe() {
        let path = PathBuf::from("workspace/src/main.rs");
        let indexed = [path.clone()];

        let target = prepare_file_compare_target(&[], &indexed, path.clone(), |_| {
            panic!("indexed compare target should not probe filesystem")
        })
        .expect("indexed target");

        assert_eq!(target.raw_path, path);
        assert!(target.text.is_none());
    }

    #[test]
    fn prepare_file_compare_target_rejects_unavailable_paths() {
        let path = PathBuf::from("workspace/src/missing.rs");

        let Err(error) = prepare_file_compare_target(&[], &[], path, |_| false) else {
            panic!("expected unavailable compare target");
        };

        assert_eq!(error, "Cannot compare missing.rs: file is not available");
    }

    #[test]
    fn compare_paths_match_lexically_equivalent_targets() {
        assert!(file_compare_paths_match(
            Path::new("workspace/src/../src/main.rs"),
            Path::new("workspace/src/main.rs"),
        ));
        assert!(!file_compare_paths_match(
            Path::new("workspace/src/main.rs"),
            Path::new("workspace/src/lib.rs"),
        ));
    }

    #[test]
    fn file_compare_source_matches_lexically_equivalent_raw_paths() {
        let source = DiffBufferSource {
            path: PathBuf::from("workspace/src/target.rs"),
            base_path: Some(PathBuf::from("workspace/src/base.rs")),
            hunk_stage: None,
            saved_buffer_id: None,
        };

        assert!(file_compare_source_matches(
            &source,
            Path::new("workspace/src/../src/base.rs"),
            Path::new("workspace/src/../src/target.rs"),
        ));
    }

    #[test]
    fn file_compare_source_rejects_non_file_compare_sources() {
        let source = DiffBufferSource {
            path: PathBuf::from("workspace/src/target.rs"),
            base_path: Some(PathBuf::from("workspace/src/base.rs")),
            hunk_stage: Some(GitChangeStage::Unstaged),
            saved_buffer_id: None,
        };

        assert!(!file_compare_source_matches(
            &source,
            Path::new("workspace/src/base.rs"),
            Path::new("workspace/src/target.rs"),
        ));
    }

    #[test]
    fn saved_compare_working_snapshot_returns_snapshot_within_limit() {
        let mut buffer = TextBuffer::from_text(
            1,
            Some(PathBuf::from("workspace/src/main.rs")),
            "abc".to_owned(),
        );

        let (path, text) = saved_compare_working_snapshot(&buffer, 3)
            .unwrap()
            .expect("file-backed buffer");
        buffer.insert_at_cursor("dirty");

        assert_eq!(path, PathBuf::from("workspace/src/main.rs"));
        assert_eq!(text.text(), "abc");
    }

    #[test]
    fn saved_compare_working_snapshot_preserves_raw_path_for_compare() {
        let raw_path = Path::new("workspace").join(format!(
            "raw\n{}\u{202e}tail.rs",
            "very-long-component-".repeat(8)
        ));
        let buffer = TextBuffer::from_text(1, Some(raw_path.clone()), "abc".to_owned());

        let (path, text) = saved_compare_working_snapshot(&buffer, 3)
            .unwrap()
            .expect("file-backed buffer");

        assert_eq!(path, raw_path);
        assert_eq!(text.text(), "abc");
        let status = saved_compare_too_large_status(&path, 2);
        assert!(status.starts_with("raw "));
        assert!(!status.contains('\n'));
        assert!(!status.contains('\u{202e}'));
        assert_eq!(path, raw_path);
    }
}
