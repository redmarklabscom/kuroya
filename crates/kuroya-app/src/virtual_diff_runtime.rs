use crate::{
    KuroyaApp,
    file_io::{file_size_exceeds_limit, file_too_large_message},
    git_diff_state::DiffBufferSource,
    path_display::display_error_label_cow,
    source_control_runtime::reserve_source_control_load_request_id_state,
    ui_events::UiEvent,
};
use kuroya_core::{
    BufferId, DiffOptions, GitCommitSummary, GitStashEntry, TextSnapshot,
    try_unified_diff_between_texts_with_options, unified_diff_for_commit, unified_diff_for_stash,
};
use std::{
    fmt,
    io::{self, Read},
    path::{Path, PathBuf},
};

mod labels;

pub(crate) use labels::virtual_diff_open_detail;
use labels::{
    PreparedVirtualDiffPath, virtual_diff_commit_label, virtual_diff_file_compare_open_label,
    virtual_diff_file_compare_target, virtual_diff_open_label_owned,
    virtual_diff_open_pending_status, virtual_diff_stash_label, virtual_diff_status_text,
    virtual_diff_status_text_owned, virtual_diff_target_label, virtual_diff_target_label_owned,
};
#[cfg(test)]
pub(crate) use labels::{
    VIRTUAL_DIFF_DETAIL_MAX_CHARS, VIRTUAL_DIFF_GIT_LABEL_MAX_CHARS,
    VIRTUAL_DIFF_OPEN_LABEL_MAX_CHARS, VIRTUAL_DIFF_STATUS_MAX_CHARS,
    VIRTUAL_DIFF_TARGET_LABEL_MAX_CHARS, virtual_diff_display_text, virtual_diff_display_text_cow,
    virtual_diff_git_label, virtual_diff_git_label_cow, virtual_diff_label_pair,
};

const VIRTUAL_DIFF_READ_BUFFER_PREALLOC_MAX_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone)]
pub(crate) enum VirtualDiffOpenRequest {
    FileCompare { base_path: PathBuf, path: PathBuf },
    SavedCompare { id: BufferId, path: PathBuf },
    GitCommit { commit: GitCommitSummary },
    GitStash { stash: GitStashEntry },
}

#[derive(Debug)]
pub(crate) struct VirtualDiffOpenJob {
    request: VirtualDiffOpenRequest,
    file_compare_base_text: Option<TextSnapshot>,
    file_compare_target_text: Option<TextSnapshot>,
    working_text: Option<TextSnapshot>,
}

#[derive(Debug)]
pub(crate) enum VirtualDiffOpenOutcome {
    Open(VirtualDiffOpen),
    Status(String),
}

#[derive(Debug)]
pub(crate) struct VirtualDiffOpen {
    pub(crate) label: String,
    pub(crate) diff: String,
    pub(crate) target: String,
    pub(crate) kind: &'static str,
    pub(crate) source: Option<DiffBufferSource>,
}

impl KuroyaApp {
    pub(crate) fn spawn_virtual_diff_open(&mut self, job: VirtualDiffOpenJob) {
        let request_id = self.reserve_virtual_diff_open_request_id();
        let event_root = self.workspace.root.clone();
        let git_root = self.source_control_git_operation_root();
        let generation = self.workspace_event_generation;
        let options = self.diff_options();
        let tx = self.tx.clone();
        let request = job.request.clone();
        let detail = virtual_diff_open_detail(&request);
        self.status = virtual_diff_open_pending_status(&detail);
        self.record_async_task_started("Virtual Diff", detail);
        self.runtime.spawn_blocking(move || {
            let result = compute_virtual_diff_open(&git_root, job, options);
            let _ = crate::ui_event_channel::send_ui_event(
                &tx,
                UiEvent::VirtualDiffOpenFinished {
                    root: event_root,
                    generation,
                    request_id,
                    request,
                    result,
                },
            );
        });
    }

    pub(crate) fn reserve_virtual_diff_open_request_id(&mut self) -> u64 {
        reserve_source_control_load_request_id_state(
            &mut self.virtual_diff_open_next_request_id,
            &mut self.virtual_diff_open_active_request_id,
        )
    }

    pub(crate) fn apply_virtual_diff_open_finished(
        &mut self,
        root: PathBuf,
        generation: u64,
        request_id: u64,
        _request: VirtualDiffOpenRequest,
        result: Result<VirtualDiffOpenOutcome, String>,
    ) {
        if !self.workspace_event_is_current(&root, generation)
            || self.virtual_diff_open_active_request_id == 0
            || request_id != self.virtual_diff_open_active_request_id
        {
            return;
        }
        self.virtual_diff_open_active_request_id = 0;

        match result {
            Ok(VirtualDiffOpenOutcome::Open(open)) => {
                self.open_virtual_diff_buffer(
                    virtual_diff_open_label_owned(open.label),
                    open.diff,
                    virtual_diff_target_label_owned(open.target),
                    open.kind,
                    open.source,
                );
            }
            Ok(VirtualDiffOpenOutcome::Status(status)) | Err(status) => {
                self.status = virtual_diff_status_text(status);
            }
        }
    }
}

impl VirtualDiffOpenJob {
    #[cfg(test)]
    pub(crate) fn file_compare(base_path: PathBuf, path: PathBuf) -> Self {
        Self::file_compare_with_snapshots(base_path, path, None, None)
    }

    pub(crate) fn file_compare_with_snapshots(
        base_path: PathBuf,
        path: PathBuf,
        base_text: Option<TextSnapshot>,
        target_text: Option<TextSnapshot>,
    ) -> Self {
        Self {
            request: VirtualDiffOpenRequest::FileCompare { base_path, path },
            file_compare_base_text: base_text,
            file_compare_target_text: target_text,
            working_text: None,
        }
    }

    pub(crate) fn saved_compare(id: BufferId, path: PathBuf, working_text: TextSnapshot) -> Self {
        Self {
            request: VirtualDiffOpenRequest::SavedCompare { id, path },
            file_compare_base_text: None,
            file_compare_target_text: None,
            working_text: Some(working_text),
        }
    }

    pub(crate) fn git_commit(commit: GitCommitSummary) -> Self {
        Self {
            request: VirtualDiffOpenRequest::GitCommit { commit },
            file_compare_base_text: None,
            file_compare_target_text: None,
            working_text: None,
        }
    }

    pub(crate) fn git_stash(stash: GitStashEntry) -> Self {
        Self {
            request: VirtualDiffOpenRequest::GitStash { stash },
            file_compare_base_text: None,
            file_compare_target_text: None,
            working_text: None,
        }
    }
}

fn compute_virtual_diff_open(
    root: &Path,
    job: VirtualDiffOpenJob,
    options: DiffOptions,
) -> Result<VirtualDiffOpenOutcome, String> {
    let VirtualDiffOpenJob {
        request,
        file_compare_base_text,
        file_compare_target_text,
        working_text,
    } = job;
    match request {
        VirtualDiffOpenRequest::FileCompare { base_path, path } => compute_file_compare_diff(
            root,
            base_path,
            path,
            file_compare_base_text,
            file_compare_target_text,
            options,
        ),
        VirtualDiffOpenRequest::SavedCompare { id, path } => {
            let Some(working_text) = working_text else {
                return Err("Could not compare saved file: missing working text".to_owned());
            };
            compute_saved_compare_diff(root, id, path, working_text, options)
        }
        VirtualDiffOpenRequest::GitCommit { commit } => compute_commit_diff(root, commit),
        VirtualDiffOpenRequest::GitStash { stash } => compute_stash_diff(root, stash),
    }
}

fn compute_file_compare_diff(
    root: &Path,
    base_path: PathBuf,
    path: PathBuf,
    base_snapshot: Option<TextSnapshot>,
    target_snapshot: Option<TextSnapshot>,
    options: DiffOptions,
) -> Result<VirtualDiffOpenOutcome, String> {
    let max_bytes = options.max_file_size_bytes;
    let base_path = PreparedVirtualDiffPath::new(root, base_path);
    let path = PreparedVirtualDiffPath::new(root, path);
    let base_text =
        compare_text_for_path(&base_path.raw, &base_path.label, base_snapshot, max_bytes)?;
    let target_text = compare_text_for_path(&path.raw, &path.label, target_snapshot, max_bytes)?;
    let diff = try_unified_diff_between_texts_with_options(
        &base_path.diff_display,
        &path.diff_display,
        &base_text,
        &target_text,
        options,
    )
    .map_err(|error| {
        let error = error.to_string();
        let error = display_error_label_cow(&error);
        virtual_diff_status_text_owned(format!(
            "Could not compare {} and {}: {}",
            base_path.label,
            path.label,
            error.as_ref()
        ))
    })?;

    if diff.trim().is_empty() {
        return Ok(VirtualDiffOpenOutcome::Status(
            virtual_diff_status_text_owned(format!(
                "No differences between {}",
                virtual_diff_file_compare_target(&base_path, &path)
            )),
        ));
    }

    Ok(VirtualDiffOpenOutcome::Open(VirtualDiffOpen {
        label: virtual_diff_file_compare_open_label(&base_path, &path),
        diff,
        target: virtual_diff_file_compare_target(&base_path, &path),
        kind: "file comparison",
        source: Some(DiffBufferSource {
            path: path.raw,
            base_path: Some(base_path.raw),
            hunk_stage: None,
            saved_buffer_id: None,
        }),
    }))
}

fn compare_text_for_path(
    path: &Path,
    label: &str,
    snapshot: Option<TextSnapshot>,
    max_bytes: usize,
) -> Result<String, String> {
    let text = if let Some(snapshot) = snapshot {
        compare_text_from_snapshot(snapshot, max_bytes)
    } else {
        read_compare_text(path, max_bytes)
    };
    text.map_err(|error| {
        let error = display_error_label_cow(&error);
        virtual_diff_status_text_owned(format!("Could not read {}: {}", label, error.as_ref()))
    })
}

fn compare_text_from_snapshot(snapshot: TextSnapshot, max_bytes: usize) -> Result<String, String> {
    let bytes = u64::try_from(snapshot.len_bytes()).unwrap_or(u64::MAX);
    let max_bytes = u64::try_from(max_bytes).unwrap_or(u64::MAX);
    if file_size_exceeds_limit(bytes, max_bytes) {
        return Err(file_too_large_message(bytes, max_bytes));
    }
    if text_snapshot_contains_nul(&snapshot) {
        return Err("binary file skipped".to_owned());
    }
    Ok(snapshot.text())
}

pub(crate) fn text_snapshot_contains_nul(snapshot: &TextSnapshot) -> bool {
    snapshot.chunks().any(|chunk| chunk.as_bytes().contains(&0))
}

fn compute_saved_compare_diff(
    root: &Path,
    id: BufferId,
    path: PathBuf,
    working_text: TextSnapshot,
    options: DiffOptions,
) -> Result<VirtualDiffOpenOutcome, String> {
    let path = PreparedVirtualDiffPath::new(root, path);
    let working_bytes = u64::try_from(working_text.len_bytes()).unwrap_or(u64::MAX);
    let max_bytes = u64::try_from(options.max_file_size_bytes).unwrap_or(u64::MAX);
    if file_size_exceeds_limit(working_bytes, max_bytes) {
        return Err(virtual_diff_status_text_owned(format!(
            "Could not compare saved {}: working file is larger than {} bytes ({})",
            path.label,
            max_bytes,
            file_too_large_message(working_bytes, max_bytes)
        )));
    }
    if text_snapshot_contains_nul(&working_text) {
        return Err(virtual_diff_status_text_owned(format!(
            "Could not compare saved {}: working file is binary (binary file skipped)",
            path.label
        )));
    }

    let saved_text = match read_compare_text_with_error(&path.raw, options.max_file_size_bytes) {
        Ok(text) => text,
        Err(error) if error.is_not_found() => {
            return Ok(VirtualDiffOpenOutcome::Status(
                virtual_diff_status_text_owned(format!("No saved file for {}", path.label)),
            ));
        }
        Err(error) => {
            let error = error.to_string();
            let error = display_error_label_cow(&error);
            return Err(virtual_diff_status_text_owned(format!(
                "Could not read saved {}: {}",
                path.label,
                error.as_ref()
            )));
        }
    };
    let working_text = working_text.text();
    let diff = try_unified_diff_between_texts_with_options(
        &format!("{} (Saved)", path.diff_display),
        &format!("{} (Working Tree)", path.diff_display),
        &saved_text,
        &working_text,
        options,
    )
    .map_err(|error| {
        let error = error.to_string();
        let error = display_error_label_cow(&error);
        virtual_diff_status_text_owned(format!(
            "Could not compare saved {}: {}",
            path.label,
            error.as_ref()
        ))
    })?;

    if diff.trim().is_empty() {
        return Ok(VirtualDiffOpenOutcome::Status(
            virtual_diff_status_text_owned(format!("No unsaved changes in {}", path.label)),
        ));
    }

    Ok(VirtualDiffOpenOutcome::Open(VirtualDiffOpen {
        label: virtual_diff_open_label_owned(format!("{} (Compare with Saved)", path.label)),
        diff,
        target: virtual_diff_target_label(&path.label),
        kind: "saved comparison",
        source: Some(DiffBufferSource {
            path: path.raw,
            base_path: None,
            hunk_stage: None,
            saved_buffer_id: Some(id),
        }),
    }))
}

fn compute_commit_diff(
    root: &Path,
    commit: GitCommitSummary,
) -> Result<VirtualDiffOpenOutcome, String> {
    let commit_label = virtual_diff_commit_label(&commit);
    let diff = unified_diff_for_commit(root, &commit.oid).map_err(|error| {
        let error = error.to_string();
        let error = display_error_label_cow(&error);
        virtual_diff_status_text_owned(format!(
            "Could not diff commit {}: {}",
            commit_label,
            error.as_ref()
        ))
    })?;
    if diff.trim().is_empty() {
        return Ok(VirtualDiffOpenOutcome::Status(
            virtual_diff_status_text_owned(format!("No changes in commit {commit_label}")),
        ));
    }
    Ok(VirtualDiffOpenOutcome::Open(VirtualDiffOpen {
        label: virtual_diff_open_label_owned(format!("{commit_label} (Commit Changes)")),
        diff,
        target: virtual_diff_target_label_owned(format!("commit {commit_label}")),
        kind: "commit changes",
        source: None,
    }))
}

fn compute_stash_diff(root: &Path, stash: GitStashEntry) -> Result<VirtualDiffOpenOutcome, String> {
    let stash_ref = format!("stash@{{{}}}", stash.index);
    let diff = unified_diff_for_stash(root, stash.index).map_err(|error| {
        let error = error.to_string();
        let error = display_error_label_cow(&error);
        virtual_diff_status_text_owned(format!("Could not diff {stash_ref}: {}", error.as_ref()))
    })?;
    if diff.trim().is_empty() {
        return Ok(VirtualDiffOpenOutcome::Status(
            virtual_diff_status_text_owned(format!("No changes in {stash_ref}")),
        ));
    }
    Ok(VirtualDiffOpenOutcome::Open(VirtualDiffOpen {
        label: virtual_diff_stash_label(&stash_ref, &stash),
        diff,
        target: virtual_diff_target_label(&stash_ref),
        kind: "stash changes",
        source: None,
    }))
}

#[derive(Debug)]
struct CompareTextReadError {
    kind: CompareTextReadErrorKind,
}

#[derive(Debug)]
enum CompareTextReadErrorKind {
    Io(io::Error),
    Message(String),
}

impl CompareTextReadError {
    fn is_not_found(&self) -> bool {
        matches!(
            &self.kind,
            CompareTextReadErrorKind::Io(error) if error.kind() == io::ErrorKind::NotFound
        )
    }

    fn message(message: impl Into<String>) -> Self {
        Self {
            kind: CompareTextReadErrorKind::Message(message.into()),
        }
    }
}

impl fmt::Display for CompareTextReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            CompareTextReadErrorKind::Io(error) => error.fmt(formatter),
            CompareTextReadErrorKind::Message(message) => formatter.write_str(message),
        }
    }
}

impl From<io::Error> for CompareTextReadError {
    fn from(error: io::Error) -> Self {
        Self {
            kind: CompareTextReadErrorKind::Io(error),
        }
    }
}

pub(crate) fn read_compare_text(path: &Path, max_bytes: usize) -> Result<String, String> {
    read_compare_text_with_error(path, max_bytes).map_err(|error| error.to_string())
}

fn read_compare_text_with_error(
    path: &Path,
    max_bytes: usize,
) -> Result<String, CompareTextReadError> {
    let max_bytes = u64::try_from(max_bytes).unwrap_or(u64::MAX);
    let mut file = open_compare_read_target(path)?;
    let metadata = file.metadata()?;
    ensure_compare_read_target_is_file(path, &metadata)?;
    if file_size_exceeds_limit(metadata.len(), max_bytes) {
        return Err(CompareTextReadError::message(file_too_large_message(
            metadata.len(),
            max_bytes,
        )));
    }

    let capacity = read_compare_buffer_capacity(metadata.len(), max_bytes);
    if max_bytes == 0 {
        let mut bytes = Vec::with_capacity(capacity);
        file.read_to_end(&mut bytes)?;
        compare_text_from_bytes(bytes)
    } else {
        let mut bytes = Vec::with_capacity(capacity);
        let mut reader = file.take(max_bytes.saturating_add(1));
        reader.read_to_end(&mut bytes)?;
        let byte_len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if file_size_exceeds_limit(byte_len, max_bytes) {
            return Err(CompareTextReadError::message(file_too_large_message(
                byte_len, max_bytes,
            )));
        }
        compare_text_from_bytes(bytes)
    }
}

fn open_compare_read_target(path: &Path) -> Result<std::fs::File, CompareTextReadError> {
    match std::fs::File::open(path) {
        Ok(file) => Ok(file),
        Err(open_error) => {
            if open_error_needs_non_file_probe(&open_error) {
                reject_non_file_compare_read_target(path)?;
            }
            Err(open_error.into())
        }
    }
}

fn reject_non_file_compare_read_target(path: &Path) -> Result<(), CompareTextReadError> {
    let Ok(metadata) = std::fs::metadata(path) else {
        return Ok(());
    };
    ensure_compare_read_target_is_file(path, &metadata)
}

fn ensure_compare_read_target_is_file(
    path: &Path,
    metadata: &std::fs::Metadata,
) -> Result<(), CompareTextReadError> {
    if metadata.is_file() {
        Ok(())
    } else {
        Err(CompareTextReadError::message(format!(
            "open target is not a file: {}",
            path.display()
        )))
    }
}

fn open_error_needs_non_file_probe(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::IsADirectory | io::ErrorKind::PermissionDenied
    )
}

fn read_compare_buffer_capacity(metadata_len: u64, max_bytes: u64) -> usize {
    let prealloc_limit = if max_bytes == 0 {
        VIRTUAL_DIFF_READ_BUFFER_PREALLOC_MAX_BYTES
    } else {
        max_bytes
            .saturating_add(1)
            .min(VIRTUAL_DIFF_READ_BUFFER_PREALLOC_MAX_BYTES)
    };
    let capacity = if max_bytes > 0 && metadata_len >= max_bytes {
        max_bytes.saturating_add(1).min(prealloc_limit)
    } else {
        metadata_len.min(prealloc_limit)
    };
    usize::try_from(capacity).unwrap_or(usize::MAX)
}

fn compare_text_from_bytes(bytes: Vec<u8>) -> Result<String, CompareTextReadError> {
    if bytes.contains(&0) {
        return Err(CompareTextReadError::message("binary file skipped"));
    }
    let text = String::from_utf8(bytes)
        .map_err(|error| CompareTextReadError::message(error.to_string()))?;
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::{
        VIRTUAL_DIFF_DETAIL_MAX_CHARS, VIRTUAL_DIFF_GIT_LABEL_MAX_CHARS,
        VIRTUAL_DIFF_OPEN_LABEL_MAX_CHARS, VIRTUAL_DIFF_READ_BUFFER_PREALLOC_MAX_BYTES,
        VIRTUAL_DIFF_STATUS_MAX_CHARS, VIRTUAL_DIFF_TARGET_LABEL_MAX_CHARS, VirtualDiffOpen,
        VirtualDiffOpenJob, VirtualDiffOpenOutcome, VirtualDiffOpenRequest, compute_commit_diff,
        compute_file_compare_diff, compute_saved_compare_diff, read_compare_buffer_capacity,
        read_compare_text, virtual_diff_commit_label, virtual_diff_display_text,
        virtual_diff_display_text_cow, virtual_diff_git_label, virtual_diff_git_label_cow,
        virtual_diff_label_pair, virtual_diff_open_detail, virtual_diff_open_pending_status,
        virtual_diff_stash_label,
    };
    use crate::{
        KuroyaApp, app_startup_context::AppStartupContext,
        path_display::DISPLAY_PATH_LABEL_MAX_CHARS, terminal::TerminalPane,
    };
    use kuroya_core::{
        DiffOptions, EditorSettings, GitCommitSummary, GitStashEntry, TextBuffer, Workspace,
    };
    use std::{
        borrow::Cow,
        env, fs,
        path::{Path, PathBuf},
        time::{Instant, SystemTime, UNIX_EPOCH},
    };
    use tokio::runtime::Runtime;

    fn temp_root(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        env::temp_dir().join(format!(
            "kuroya-virtual-diff-{name}-{}-{nanos}",
            std::process::id()
        ))
    }

    fn write_file(path: &Path, text: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, text).unwrap();
    }

    fn app_for_test(root: PathBuf) -> KuroyaApp {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let settings = EditorSettings::default();
        KuroyaApp::from_startup_context(AppStartupContext {
            runtime: Runtime::new().expect("test runtime"),
            tx,
            rx,
            workspace: Workspace::new(root.clone()),
            settings: settings.clone(),
            settings_panel_draft: settings,
            settings_editor_font_path: String::new(),
            settings_ui_font_path: String::new(),
            theme_picker_selected: 0,
            saved_session: None,
            terminal: TerminalPane::new(root.clone(), 100, 12.0, 1.2),
            watcher: None,
            recent_projects: Vec::new(),
            trusted_workspaces: vec![root],
            now: Instant::now(),
            startup_timings: Vec::new(),
        })
    }

    #[test]
    fn virtual_diff_display_text_cow_borrows_clean_ascii_and_unicode_labels() {
        let ascii = "clean-label.rs";
        assert!(matches!(
            virtual_diff_display_text_cow(ascii, 64, "."),
            Cow::Borrowed(label) if label == ascii
        ));

        let unicode = "clean-\u{03bb}.rs";
        assert!(matches!(
            virtual_diff_display_text_cow(unicode, 64, "."),
            Cow::Borrowed(label) if label == unicode
        ));
    }

    #[test]
    fn virtual_diff_display_text_cow_owns_dirty_truncated_and_fallback_labels() {
        let cases = [
            ("alpha\nbeta", 64, "."),
            ("alpha\u{202e}beta", 64, "."),
            ("abcdefghijklmnopqrstuvwxyz", 12, "."),
            ("   ", 64, "fallback"),
        ];

        for (value, max_chars, fallback) in cases {
            let label = virtual_diff_display_text_cow(value, max_chars, fallback);

            assert_eq!(
                label.as_ref(),
                virtual_diff_display_text(value, max_chars, fallback)
            );
            assert!(
                matches!(label, Cow::Owned(_)),
                "expected owned label for {value:?}"
            );
        }
    }

    #[test]
    fn virtual_diff_display_text_string_wrappers_match_cow_helpers() {
        let cases = [
            ("clean-label.rs", 64, "."),
            ("clean-\u{03bb}.rs", 64, "."),
            ("alpha\nbeta", 64, "."),
            ("abcdefghijklmnopqrstuvwxyz", 12, "."),
            ("visible", 0, "."),
        ];

        for (value, max_chars, fallback) in cases {
            let label = virtual_diff_display_text_cow(value, max_chars, fallback);

            assert_eq!(
                virtual_diff_display_text(value, max_chars, fallback),
                label.as_ref()
            );
        }

        let git_cases = [
            ("abcdef123456", "commit"),
            ("abc\n123", "commit"),
            ("", "stash"),
        ];

        for (value, fallback) in git_cases {
            let label = virtual_diff_git_label_cow(value, fallback);

            assert_eq!(virtual_diff_git_label(value, fallback), label.as_ref());
        }
    }

    #[test]
    fn virtual_diff_path_label_pair_preserves_clean_display_text() {
        assert_eq!(
            virtual_diff_label_pair(
                "left.rs",
                " <-> ",
                "right.rs",
                " (Compare)",
                VIRTUAL_DIFF_OPEN_LABEL_MAX_CHARS,
            ),
            "left.rs <-> right.rs (Compare)"
        );
    }

    #[test]
    fn virtual_diff_git_label_cow_preserves_clean_display_text() {
        assert!(matches!(
            virtual_diff_git_label_cow("abcdef1", "commit"),
            Cow::Borrowed("abcdef1")
        ));
        assert_eq!(virtual_diff_git_label("abcdef1", "commit"), "abcdef1");
    }

    #[test]
    fn virtual_diff_stash_label_preserves_clean_display_text() {
        let stash = GitStashEntry {
            index: 2,
            short_oid: "abc123".to_owned(),
            message: "work in progress".to_owned(),
        };

        assert_eq!(
            virtual_diff_stash_label("stash@{2}", &stash),
            "stash@{2} abc123 (Stash Changes)"
        );
    }

    fn saved_compare_request(path: PathBuf) -> VirtualDiffOpenRequest {
        VirtualDiffOpenRequest::SavedCompare { id: 7, path }
    }

    fn diff_open_outcome(label: &str) -> Result<VirtualDiffOpenOutcome, String> {
        Ok(VirtualDiffOpenOutcome::Open(VirtualDiffOpen {
            label: label.to_owned(),
            diff: format!("diff --git a/{label} b/{label}\n"),
            target: label.to_owned(),
            kind: "test diff",
            source: None,
        }))
    }

    #[test]
    fn file_compare_diff_is_computed_with_workspace_relative_labels() {
        let root = temp_root("file-compare");
        let left = root.join("src/left.rs");
        let right = root.join("src/right.rs");
        write_file(&left, "fn main() {\n    println!(\"left\");\n}\n");
        write_file(&right, "fn main() {\n    println!(\"right\");\n}\n");

        let outcome = compute_file_compare_diff(
            &root,
            left.clone(),
            right.clone(),
            None,
            None,
            DiffOptions::default(),
        )
        .unwrap();

        match outcome {
            VirtualDiffOpenOutcome::Open(open) => {
                assert_eq!(open.kind, "file comparison");
                assert!(open.diff.contains("src/left.rs"));
                assert!(open.diff.contains("src/right.rs"));
                assert_eq!(open.source.unwrap().base_path, Some(left));
            }
            VirtualDiffOpenOutcome::Status(status) => panic!("expected diff, got {status}"),
        }

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn file_compare_diff_uses_open_buffer_snapshots_for_missing_paths() {
        let root = temp_root("file-compare-open-buffer");
        let left = root.join("src/left.rs");
        let right = root.join("src/right.rs");
        let left_snapshot =
            TextBuffer::from_text(1, Some(left.clone()), "left\n".to_owned()).text_snapshot();
        let right_snapshot =
            TextBuffer::from_text(2, Some(right.clone()), "right\n".to_owned()).text_snapshot();

        let outcome = compute_file_compare_diff(
            &root,
            left.clone(),
            right.clone(),
            Some(left_snapshot),
            Some(right_snapshot),
            DiffOptions::default(),
        )
        .unwrap();

        match outcome {
            VirtualDiffOpenOutcome::Open(open) => {
                assert_eq!(open.kind, "file comparison");
                assert!(open.diff.contains("-left"));
                assert!(open.diff.contains("+right"));
                assert_eq!(open.source.unwrap().base_path, Some(left));
            }
            VirtualDiffOpenOutcome::Status(status) => panic!("expected diff, got {status}"),
        }

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn file_compare_rejects_binary_text_target() {
        let root = temp_root("file-compare-binary");
        let left = root.join("src/left.rs");
        let right = root.join("src/binary.dat");
        write_file(&left, "plain text\n");
        if let Some(parent) = right.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&right, b"binary\0text\n").unwrap();

        let error = compute_file_compare_diff(
            &root,
            left,
            right.clone(),
            None,
            None,
            DiffOptions::default(),
        )
        .unwrap_err();

        assert!(error.contains("Could not read"));
        assert!(error.contains("binary.dat"));
        assert!(error.contains("binary file skipped"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn file_compare_rejects_binary_open_buffer_snapshot() {
        let root = temp_root("file-compare-binary-snapshot");
        let left = root.join("src/left.rs");
        let right = root.join("src/binary.dat");
        let left_snapshot =
            TextBuffer::from_text(1, Some(left.clone()), "plain text\n".to_owned()).text_snapshot();
        let right_snapshot =
            TextBuffer::from_text(2, Some(right.clone()), "binary\0text\n".to_owned())
                .text_snapshot();

        let error = compute_file_compare_diff(
            &root,
            left,
            right,
            Some(left_snapshot),
            Some(right_snapshot),
            DiffOptions::default(),
        )
        .unwrap_err();

        assert!(error.contains("Could not read"));
        assert!(error.contains("binary.dat"));
        assert!(error.contains("binary file skipped"));
    }

    #[test]
    fn read_compare_text_rejects_directories_as_non_files() {
        let root = temp_root("read-directory");
        fs::create_dir_all(&root).unwrap();

        let error = read_compare_text(&root, DiffOptions::default().max_file_size_bytes)
            .expect_err("directory should not be read as text");

        assert!(error.contains("open target is not a file"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn read_compare_buffer_capacity_caps_unlimited_preallocation() {
        assert_eq!(read_compare_buffer_capacity(3, 4), 3);
        assert_eq!(read_compare_buffer_capacity(4, 4), 5);
        assert_eq!(read_compare_buffer_capacity(100, 4), 5);
        assert_eq!(
            read_compare_buffer_capacity(VIRTUAL_DIFF_READ_BUFFER_PREALLOC_MAX_BYTES + 1024, 0),
            usize::try_from(VIRTUAL_DIFF_READ_BUFFER_PREALLOC_MAX_BYTES).unwrap()
        );
    }

    #[test]
    fn saved_compare_reports_missing_saved_file_without_panicking() {
        let root = temp_root("saved-missing");
        let path = root.join("src/missing.rs");

        let outcome = compute_saved_compare_diff(
            &root,
            9,
            path.clone(),
            TextBuffer::from_text(1, Some(path.clone()), "dirty".to_owned()).text_snapshot(),
            DiffOptions::default(),
        )
        .unwrap();

        match outcome {
            VirtualDiffOpenOutcome::Status(status) => {
                assert!(status.contains("No saved file"));
                assert!(status.contains("missing.rs"));
            }
            VirtualDiffOpenOutcome::Open(_) => panic!("expected missing-file status"),
        }
    }

    #[test]
    fn saved_compare_uses_working_snapshot_text() {
        let root = temp_root("saved-snapshot");
        let path = root.join("src/main.rs");
        write_file(&path, "saved\n");
        let mut buffer = TextBuffer::from_text(3, Some(path.clone()), "working\n".to_owned());
        let snapshot = buffer.text_snapshot();
        buffer.insert_at_cursor("newer ");

        let outcome =
            compute_saved_compare_diff(&root, 3, path, snapshot, DiffOptions::default()).unwrap();

        match outcome {
            VirtualDiffOpenOutcome::Open(open) => {
                assert!(open.diff.contains("+working"));
                assert!(!open.diff.contains("newer"));
            }
            VirtualDiffOpenOutcome::Status(status) => panic!("expected diff, got {status}"),
        }

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn saved_compare_rejects_large_working_snapshot_before_reading_saved_file() {
        let root = temp_root("saved-large-working");
        let path = root.join("src/main.rs");
        let snapshot =
            TextBuffer::from_text(1, Some(path.clone()), "dirty".to_owned()).text_snapshot();
        let options = DiffOptions {
            max_file_size_bytes: 4,
            ..DiffOptions::default()
        };

        let error = compute_saved_compare_diff(&root, 1, path, snapshot, options).unwrap_err();

        assert!(error.contains("working file is larger than 4 bytes"));
        assert!(!error.contains("No saved file"));
    }

    #[test]
    fn saved_compare_rejects_binary_working_snapshot_before_reading_saved_file() {
        let root = temp_root("saved-binary-working");
        let path = root.join("src/main.rs");
        let snapshot =
            TextBuffer::from_text(1, Some(path.clone()), "dirty\0text".to_owned()).text_snapshot();

        let error = compute_saved_compare_diff(&root, 1, path, snapshot, DiffOptions::default())
            .unwrap_err();

        assert!(error.contains("working file is binary"));
        assert!(error.contains("binary file skipped"));
        assert!(!error.contains("No saved file"));
    }

    #[test]
    fn file_compare_keeps_raw_diff_paths_while_bounding_open_labels() {
        let root = temp_root("file-compare-raw-diff-paths");
        let left_name = format!("left-{}tail.rs", "long-".repeat(32));
        let right_name = format!("right-{}tail.rs", "long-".repeat(32));
        let left = root.join("src").join(&left_name);
        let right = root.join("src").join(&right_name);
        let left_snapshot =
            TextBuffer::from_text(1, Some(left.clone()), "left\n".to_owned()).text_snapshot();
        let right_snapshot =
            TextBuffer::from_text(2, Some(right.clone()), "right\n".to_owned()).text_snapshot();

        let outcome = compute_file_compare_diff(
            &root,
            left.clone(),
            right.clone(),
            Some(left_snapshot),
            Some(right_snapshot),
            DiffOptions::default(),
        )
        .unwrap();

        match outcome {
            VirtualDiffOpenOutcome::Open(open) => {
                assert!(open.label.chars().count() <= VIRTUAL_DIFF_OPEN_LABEL_MAX_CHARS);
                assert!(open.target.chars().count() <= VIRTUAL_DIFF_TARGET_LABEL_MAX_CHARS);
                assert!(open.label.contains("..."));
                assert!(open.target.contains("..."));
                assert!(open.diff.contains(&format!("src/{left_name}")));
                assert!(open.diff.contains(&format!("src/{right_name}")));
                let source = open.source.unwrap();
                assert_eq!(source.path, right);
                assert_eq!(source.base_path, Some(left));
            }
            VirtualDiffOpenOutcome::Status(status) => panic!("expected diff, got {status}"),
        }
    }

    #[test]
    fn file_compare_status_and_pending_text_are_bounded_for_long_paths() {
        let root = temp_root("file-compare-status-bounds");
        let left_name = format!("left-{}tail.rs", "long-".repeat(32));
        let right_name = format!("right-{}tail.rs", "long-".repeat(32));
        let left = root.join("src").join(&left_name);
        let right = root.join("src").join(&right_name);
        let left_snapshot =
            TextBuffer::from_text(1, Some(left.clone()), "same\n".to_owned()).text_snapshot();
        let right_snapshot =
            TextBuffer::from_text(2, Some(right.clone()), "same\n".to_owned()).text_snapshot();
        let request = VirtualDiffOpenRequest::FileCompare {
            base_path: left.clone(),
            path: right.clone(),
        };

        let detail = virtual_diff_open_detail(&request);
        let pending = virtual_diff_open_pending_status(&detail);
        let outcome = compute_file_compare_diff(
            &root,
            left,
            right,
            Some(left_snapshot),
            Some(right_snapshot),
            DiffOptions::default(),
        )
        .unwrap();

        assert!(detail.chars().count() <= VIRTUAL_DIFF_DETAIL_MAX_CHARS);
        assert!(pending.chars().count() <= VIRTUAL_DIFF_STATUS_MAX_CHARS);
        assert!(detail.contains("..."));
        match outcome {
            VirtualDiffOpenOutcome::Status(status) => {
                assert!(status.starts_with("No differences between "));
                assert!(status.contains("..."));
                assert!(status.chars().count() <= VIRTUAL_DIFF_STATUS_MAX_CHARS);
            }
            VirtualDiffOpenOutcome::Open(_) => panic!("expected no-diff status"),
        }
    }

    #[test]
    fn virtual_diff_open_finished_without_active_request_is_ignored() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        app.status = "idle".to_owned();
        app.virtual_diff_open_active_request_id = 0;

        app.apply_virtual_diff_open_finished(
            root,
            app.workspace_event_generation,
            0,
            saved_compare_request(path),
            diff_open_outcome("src/main.rs"),
        );

        assert!(app.buffers.is_empty());
        assert!(app.virtual_buffer_labels.is_empty());
        assert_eq!(app.status, "idle");
        assert_eq!(app.virtual_diff_open_active_request_id, 0);
    }

    #[test]
    fn stale_virtual_diff_open_finished_after_newer_request_is_ignored() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        app.status = "newer request pending".to_owned();
        app.virtual_diff_open_active_request_id = 2;

        app.apply_virtual_diff_open_finished(
            root,
            app.workspace_event_generation,
            1,
            saved_compare_request(path),
            Ok(VirtualDiffOpenOutcome::Status(
                "stale diff completed".to_owned(),
            )),
        );

        assert_eq!(app.status, "newer request pending");
        assert_eq!(app.virtual_diff_open_active_request_id, 2);
    }

    #[test]
    fn stale_virtual_diff_open_finished_after_generation_change_is_ignored() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        let stale_generation = app.workspace_event_generation;
        app.workspace_event_generation = stale_generation + 1;
        app.status = "current workspace".to_owned();
        app.virtual_diff_open_active_request_id = 1;

        app.apply_virtual_diff_open_finished(
            root,
            stale_generation,
            1,
            saved_compare_request(path),
            Ok(VirtualDiffOpenOutcome::Status(
                "stale generation".to_owned(),
            )),
        );

        assert_eq!(app.status, "current workspace");
        assert_eq!(app.virtual_diff_open_active_request_id, 1);
    }

    #[test]
    fn stale_virtual_diff_open_finished_after_workspace_root_change_is_ignored() {
        let root = PathBuf::from("workspace/current");
        let stale_root = PathBuf::from("workspace/old");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        app.status = "current workspace".to_owned();
        app.virtual_diff_open_active_request_id = 1;

        app.apply_virtual_diff_open_finished(
            stale_root,
            app.workspace_event_generation,
            1,
            saved_compare_request(path),
            Ok(VirtualDiffOpenOutcome::Status("stale root".to_owned())),
        );

        assert_eq!(app.status, "current workspace");
        assert_eq!(app.virtual_diff_open_active_request_id, 1);
    }

    #[test]
    fn stale_virtual_diff_open_finished_does_not_open_buffer() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        app.status = "current diff".to_owned();
        app.virtual_diff_open_active_request_id = 2;

        app.apply_virtual_diff_open_finished(
            root,
            app.workspace_event_generation,
            1,
            saved_compare_request(path),
            diff_open_outcome("src/main.rs"),
        );

        assert!(app.buffers.is_empty());
        assert!(app.virtual_buffer_labels.is_empty());
        assert_eq!(app.status, "current diff");
        assert_eq!(app.virtual_diff_open_active_request_id, 2);
    }

    #[test]
    fn current_virtual_diff_open_finished_bounds_applied_status() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        app.virtual_diff_open_active_request_id = 1;

        app.apply_virtual_diff_open_finished(
            root,
            app.workspace_event_generation,
            1,
            saved_compare_request(path),
            Ok(VirtualDiffOpenOutcome::Status(format!(
                "completed {}",
                "status-".repeat(80)
            ))),
        );

        assert!(app.status.contains("..."));
        assert!(app.status.chars().count() <= VIRTUAL_DIFF_STATUS_MAX_CHARS);
        assert_eq!(app.virtual_diff_open_active_request_id, 0);
    }

    #[test]
    fn duplicate_current_virtual_diff_open_finished_is_ignored_after_first_apply() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        app.virtual_diff_open_active_request_id = 1;

        app.apply_virtual_diff_open_finished(
            root.clone(),
            app.workspace_event_generation,
            1,
            saved_compare_request(path.clone()),
            diff_open_outcome("src/main.rs"),
        );

        assert_eq!(app.virtual_diff_open_active_request_id, 0);
        assert_eq!(app.buffers.len(), 1);
        assert_eq!(app.virtual_buffer_labels.len(), 1);

        app.status = "first result applied".to_owned();
        app.apply_virtual_diff_open_finished(
            root,
            app.workspace_event_generation,
            1,
            saved_compare_request(path),
            Ok(VirtualDiffOpenOutcome::Status(
                "duplicate result replayed".to_owned(),
            )),
        );

        assert_eq!(app.status, "first result applied");
        assert_eq!(app.buffers.len(), 1);
        assert_eq!(app.virtual_buffer_labels.len(), 1);
    }

    #[test]
    fn virtual_diff_detail_names_request_targets() {
        let saved_detail = virtual_diff_open_detail(&VirtualDiffOpenRequest::SavedCompare {
            id: 7,
            path: PathBuf::from("src/main.rs"),
        });
        assert_eq!(saved_detail, "main.rs");
        assert_eq!(
            virtual_diff_open_detail(&VirtualDiffOpenRequest::GitCommit {
                commit: GitCommitSummary {
                    oid: "abcdef".to_owned(),
                    short_oid: "abcdef".to_owned(),
                    summary: "commit".to_owned(),
                    author: "Kuroya".to_owned(),
                    time_seconds: 0,
                },
            }),
            "commit abcdef"
        );
        assert!(matches!(
            VirtualDiffOpenJob::file_compare(PathBuf::from("a"), PathBuf::from("b")).request,
            VirtualDiffOpenRequest::FileCompare { .. }
        ));
    }

    #[test]
    fn virtual_diff_display_text_sanitizes_and_bounds_path_labels() {
        let root = temp_root("display-safe");
        let bad_name = format!("bad\n{}\u{202e}tail.rs", "very-long-component-".repeat(16));
        let path = root.join(&bad_name);

        let detail = virtual_diff_open_detail(&VirtualDiffOpenRequest::SavedCompare {
            id: 7,
            path: path.clone(),
        });

        assert!(!detail.contains('\n'));
        assert!(!detail.contains('\u{202e}'));
        assert!(detail.contains("..."));
        assert!(detail.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);

        let outcome = compute_saved_compare_diff(
            &root,
            7,
            path.clone(),
            TextBuffer::from_text(1, Some(path), "dirty".to_owned()).text_snapshot(),
            DiffOptions::default(),
        )
        .unwrap();

        match outcome {
            VirtualDiffOpenOutcome::Status(status) => {
                assert!(status.starts_with("No saved file for "));
                assert!(!status.contains('\n'));
                assert!(!status.contains('\u{202e}'));
                assert!(status.contains("..."));
                assert!(status.chars().count() <= VIRTUAL_DIFF_STATUS_MAX_CHARS);
            }
            VirtualDiffOpenOutcome::Open(_) => panic!("expected missing-file status"),
        }
    }

    #[test]
    fn virtual_diff_git_commit_display_text_sanitizes_and_bounds_labels() {
        let commit = GitCommitSummary {
            oid: "not-a-real-oid".to_owned(),
            short_oid: format!("abc\n{}\u{202e}tail", "x".repeat(160)),
            summary: "commit".to_owned(),
            author: "Kuroya".to_owned(),
            time_seconds: 0,
        };

        let detail = virtual_diff_open_detail(&VirtualDiffOpenRequest::GitCommit {
            commit: commit.clone(),
        });

        assert!(detail.starts_with("commit "));
        assert!(!detail.contains('\n'));
        assert!(!detail.contains('\u{202e}'));
        assert!(detail.contains("..."));
        assert!(detail.chars().count() <= "commit ".len() + VIRTUAL_DIFF_GIT_LABEL_MAX_CHARS);

        let root = temp_root("git-commit-display-safe");
        let error = compute_commit_diff(&root, commit).unwrap_err();

        assert!(error.starts_with("Could not diff commit "));
        assert!(!error.contains('\n'));
        assert!(!error.contains('\u{202e}'));
        assert!(error.contains("..."));
        assert!(error.chars().count() <= VIRTUAL_DIFF_STATUS_MAX_CHARS);
    }

    #[test]
    fn virtual_diff_stash_display_text_sanitizes_and_bounds_hash_labels() {
        let stash = GitStashEntry {
            index: 2,
            short_oid: format!("stash\n{}\u{202e}tail", "x".repeat(160)),
            message: "work in progress".to_owned(),
        };
        let label = virtual_diff_stash_label("stash@{2}", &stash);
        let hash_label = virtual_diff_commit_label(&GitCommitSummary {
            oid: "unused".to_owned(),
            short_oid: stash.short_oid,
            summary: String::new(),
            author: String::new(),
            time_seconds: 0,
        });

        assert!(label.starts_with("stash@{2} "));
        assert!(!label.contains('\n'));
        assert!(!label.contains('\u{202e}'));
        assert!(label.contains("..."));
        assert!(label.chars().count() <= VIRTUAL_DIFF_OPEN_LABEL_MAX_CHARS);
        assert!(hash_label.chars().count() <= VIRTUAL_DIFF_GIT_LABEL_MAX_CHARS);
    }

    #[test]
    fn file_compare_open_labels_sanitize_display_paths() {
        let root = temp_root("file-compare-display-safe");
        let left = root.join(format!("left-{}tail.rs", "long-".repeat(32)));
        let right = root.join(format!("right-{}tail.rs", "long-".repeat(32)));
        write_file(&left, "left\n");
        write_file(&right, "right\n");

        let outcome =
            compute_file_compare_diff(&root, left, right, None, None, DiffOptions::default())
                .unwrap();

        match outcome {
            VirtualDiffOpenOutcome::Open(open) => {
                assert!(open.label.chars().count() <= VIRTUAL_DIFF_OPEN_LABEL_MAX_CHARS);
                assert!(open.target.chars().count() <= VIRTUAL_DIFF_TARGET_LABEL_MAX_CHARS);
                assert!(open.label.contains("..."));
                assert!(open.target.contains("..."));
            }
            VirtualDiffOpenOutcome::Status(status) => panic!("expected diff, got {status}"),
        }

        let _ = fs::remove_dir_all(root);
    }
}
