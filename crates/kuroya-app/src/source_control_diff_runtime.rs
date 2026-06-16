use crate::{
    KuroyaApp,
    file_io::{file_size_exceeds_limit, file_too_large_message, read_utf8_text_file_with_limit},
    git_diff_state::DiffBufferSource,
    git_diff_view::{
        hunk_start_lines_in_unified_diff, source_control_hunk_diff_open_missing_status,
        source_control_hunk_diff_open_success_status,
    },
    path_display::{
        DISPLAY_PATH_LABEL_MAX_CHARS, display_error_label_cow, display_path_label_cow,
        sanitized_display_label_cow,
    },
    ui_events::UiEvent,
    virtual_diff_runtime::VirtualDiffOpen,
};
use kuroya_core::{
    DiffOptions, GitChangeStage, TextBuffer, TextSnapshot,
    clamp_diff_render_side_by_side_inline_breakpoint, clamp_diff_split_view_default_ratio,
    file_text_at_head, head_diff_with_text, staged_diff_with_texts, worktree_diff_with_index_text,
};
use std::{
    borrow::Cow,
    fmt::Display,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub(crate) enum SourceControlDiffOpenRequest {
    Worktree {
        path: PathBuf,
        focus_hunk: Option<usize>,
    },
    Staged {
        path: PathBuf,
        focus_hunk: Option<usize>,
    },
    Head {
        path: PathBuf,
    },
}

#[derive(Debug)]
pub(crate) struct SourceControlDiffOpenJob {
    request: SourceControlDiffOpenRequest,
    worktree_text: Option<SourceControlDiffText>,
    prepare_side_by_side: bool,
}

#[derive(Debug)]
pub(crate) enum SourceControlDiffOpenOutcome {
    Open(Box<SourceControlDiffOpen>),
    Status(String),
}

#[derive(Debug)]
pub(crate) struct SourceControlDiffOpen {
    open: VirtualDiffOpen,
    focus_hunk: Option<usize>,
    side_by_side: Option<SourceControlSideBySideOpen>,
}

#[derive(Debug)]
pub(crate) struct SourceControlSideBySideOpen {
    base: SourceControlSideBySideBuffer,
    source: SourceControlSideBySideBuffer,
    target: String,
    kind: &'static str,
}

#[derive(Debug)]
pub(crate) enum SourceControlSideBySideBuffer {
    Virtual {
        label: String,
        path: PathBuf,
        target: String,
        text: String,
    },
    Worktree {
        path: PathBuf,
    },
}

#[derive(Debug)]
pub(crate) enum SourceControlDiffText {
    Snapshot(TextSnapshot),
    File(PathBuf),
    Deleted,
    TooLarge { bytes: usize },
}

#[derive(Debug, Clone)]
struct SourceControlDiffPathLabels {
    diff: String,
    target: String,
}

impl SourceControlDiffPathLabels {
    fn new(path: &Path) -> Self {
        let file_name = path.file_name().and_then(|name| name.to_str());
        let target = file_name
            .map(source_control_diff_target_display_label)
            .unwrap_or_else(|| source_control_diff_target_label(path));
        let diff = file_name
            .map(source_control_diff_display_label)
            .unwrap_or_else(|| target.clone());

        Self { diff, target }
    }

    fn diff_title(&self, suffix: &str) -> String {
        source_control_diff_title_label(&self.diff, suffix)
    }

    fn target_label(&self) -> String {
        self.target.clone()
    }
}

impl KuroyaApp {
    pub(crate) fn spawn_source_control_diff_open(&mut self, job: SourceControlDiffOpenJob) {
        let request_id = self.reserve_source_control_diff_open_request_id();
        self.spawn_source_control_diff_open_with_request_id(job, request_id);
    }

    pub(crate) fn reserve_source_control_diff_open_request_id(&mut self) -> u64 {
        reserve_source_control_diff_open_request_id_state(
            &mut self.source_control_diff_open_next_request_id,
            &mut self.source_control_diff_open_active_request_id,
        )
    }

    pub(crate) fn spawn_source_control_diff_open_with_request_id(
        &mut self,
        mut job: SourceControlDiffOpenJob,
        request_id: u64,
    ) {
        let event_root = self.workspace.root.clone();
        let git_root = self.source_control_git_operation_root();
        let generation = self.workspace_event_generation;
        let options = self.diff_options();
        let tx = self.tx.clone();
        let request = job.request.clone();
        let detail = source_control_diff_open_detail(&request);
        job.prepare_side_by_side &= source_control_diff_opens_side_by_side(
            self.settings.diff_render_side_by_side,
            self.settings.diff_only_show_accessible_viewer,
            true,
            self.settings.diff_use_inline_view_when_space_is_limited,
            self.settings.diff_render_side_by_side_inline_breakpoint,
            self.active_editor_pane_width(),
        );
        self.set_git_progress_status(source_control_diff_open_pending_status_for_detail(&detail));
        self.record_async_task_started("Git Diff Open", detail);
        self.runtime.spawn_blocking(move || {
            let result = compute_source_control_diff_open(&git_root, job, options);
            let _ = crate::ui_event_channel::send_ui_event(
                &tx,
                UiEvent::SourceControlDiffOpenFinished {
                    root: event_root,
                    operation_root: git_root,
                    generation,
                    request_id,
                    request,
                    result,
                },
            );
        });
    }

    pub(crate) fn apply_source_control_diff_open_finished(
        &mut self,
        root: PathBuf,
        operation_root: PathBuf,
        generation: u64,
        request_id: u64,
        _request: SourceControlDiffOpenRequest,
        result: Result<SourceControlDiffOpenOutcome, String>,
    ) {
        if !self.source_control_diff_open_finished_is_current(
            &root,
            &operation_root,
            generation,
            request_id,
        ) {
            return;
        }

        match result {
            Ok(SourceControlDiffOpenOutcome::Open(open)) => {
                if source_control_diff_opens_side_by_side(
                    self.settings.diff_render_side_by_side,
                    self.settings.diff_only_show_accessible_viewer,
                    open.side_by_side.is_some(),
                    self.settings.diff_use_inline_view_when_space_is_limited,
                    self.settings.diff_render_side_by_side_inline_breakpoint,
                    self.active_editor_pane_width(),
                ) {
                    if let Some(side_by_side) = open.side_by_side {
                        self.open_source_control_side_by_side_diff(
                            open.open.diff,
                            open.focus_hunk,
                            side_by_side,
                        );
                        return;
                    }
                }
                let hunk_source = open.focus_hunk.and_then(|hunk_index| {
                    open.open.source.as_ref().and_then(|source| {
                        source
                            .hunk_stage
                            .map(|stage| (source.path.clone(), stage, hunk_index))
                    })
                });
                self.open_virtual_diff_buffer(
                    open.open.label,
                    open.open.diff,
                    open.open.target,
                    open.open.kind,
                    open.open.source,
                );
                let Some(id) = self.active else {
                    return;
                };
                if let Some((path, stage, hunk_index)) = hunk_source {
                    if let Some((label, line)) = self.focus_diff_hunk(id, hunk_index) {
                        self.status = source_control_hunk_diff_open_success_status(
                            stage, &label, hunk_index, line,
                        );
                    } else {
                        self.status =
                            source_control_hunk_diff_open_missing_status(stage, &path, hunk_index);
                    }
                }
            }
            Ok(SourceControlDiffOpenOutcome::Status(status)) | Err(status) => {
                self.status = status;
            }
        }
    }

    fn source_control_diff_open_finished_is_current(
        &self,
        root: &Path,
        operation_root: &Path,
        generation: u64,
        request_id: u64,
    ) -> bool {
        self.workspace_event_is_current(root, generation)
            && self.source_control_git_operation_root_matches(operation_root)
            && request_id == self.source_control_diff_open_active_request_id
    }
}

impl SourceControlDiffOpenJob {
    pub(crate) fn worktree(
        path: PathBuf,
        text: SourceControlDiffText,
        focus_hunk: Option<usize>,
    ) -> Self {
        Self {
            request: SourceControlDiffOpenRequest::Worktree { path, focus_hunk },
            worktree_text: Some(text),
            prepare_side_by_side: true,
        }
    }

    pub(crate) fn staged(path: PathBuf, focus_hunk: Option<usize>) -> Self {
        Self {
            request: SourceControlDiffOpenRequest::Staged { path, focus_hunk },
            worktree_text: None,
            prepare_side_by_side: true,
        }
    }

    pub(crate) fn head(path: PathBuf, text: SourceControlDiffText) -> Self {
        Self {
            request: SourceControlDiffOpenRequest::Head { path },
            worktree_text: Some(text),
            prepare_side_by_side: true,
        }
    }
}

impl SourceControlDiffText {
    pub(crate) fn open_buffer(buffer: &TextBuffer, max_bytes: usize) -> Self {
        let bytes = buffer.len_bytes();
        if source_control_diff_text_exceeds_max_bytes(bytes, max_bytes) {
            Self::TooLarge { bytes }
        } else {
            Self::Snapshot(buffer.text_snapshot())
        }
    }

    fn load(self, max_bytes: usize) -> Result<String, String> {
        match self {
            Self::Snapshot(text) => {
                let bytes = text.len_bytes();
                if source_control_diff_text_exceeds_max_bytes(bytes, max_bytes) {
                    return Err(source_control_diff_text_too_large_message(bytes, max_bytes));
                }
                Ok(text.text())
            }
            Self::Deleted => Ok(String::new()),
            Self::File(path) => read_source_control_diff_text(&path, max_bytes),
            Self::TooLarge { bytes } => {
                Err(source_control_diff_text_too_large_message(bytes, max_bytes))
            }
        }
    }

    fn load_with_worktree_source_availability(
        self,
        max_bytes: usize,
    ) -> Result<(String, bool), String> {
        let worktree_source_available = self.worktree_source_available();
        self.load(max_bytes)
            .map(|text| (text, worktree_source_available))
    }

    fn worktree_source_available(&self) -> bool {
        matches!(self, Self::Snapshot(_) | Self::File(_))
    }
}

fn source_control_diff_text_exceeds_max_bytes(bytes: usize, max_bytes: usize) -> bool {
    file_size_exceeds_limit(
        u64::try_from(bytes).unwrap_or(u64::MAX),
        u64::try_from(max_bytes).unwrap_or(u64::MAX),
    )
}

fn source_control_diff_text_too_large_message(bytes: usize, max_bytes: usize) -> String {
    file_too_large_message(
        u64::try_from(bytes).unwrap_or(u64::MAX),
        u64::try_from(max_bytes).unwrap_or(u64::MAX),
    )
}

fn reserve_source_control_diff_open_request_id_state(
    next_request_id: &mut u64,
    active_request_id: &mut u64,
) -> u64 {
    let mut request_id = next_source_control_diff_open_request_id(*next_request_id);
    if request_id == *active_request_id && *active_request_id != 0 {
        request_id = next_source_control_diff_open_request_id(request_id);
    }
    *next_request_id = request_id;
    *active_request_id = request_id;
    request_id
}

fn next_source_control_diff_open_request_id(current: u64) -> u64 {
    match current.wrapping_add(1) {
        0 => 1,
        request_id => request_id,
    }
}

fn source_control_diff_path_label_cow(path: &Path) -> Cow<'_, str> {
    display_path_label_cow(path)
}

fn source_control_diff_path_label(path: &Path) -> String {
    source_control_diff_path_label_cow(path).into_owned()
}

fn source_control_diff_error_label_cow(error: &str) -> Cow<'_, str> {
    display_error_label_cow(error)
}

#[cfg(test)]
fn source_control_diff_error_label(error: impl Display) -> String {
    let error = error.to_string();
    source_control_diff_error_label_cow(&error).into_owned()
}

fn source_control_diff_target_label(path: &Path) -> String {
    source_control_diff_path_label(path)
}

fn source_control_diff_target_display_label_cow<'a>(target: &'a str) -> Cow<'a, str> {
    sanitized_display_label_cow(target, DISPLAY_PATH_LABEL_MAX_CHARS, ".")
}

fn source_control_diff_target_display_label(target: &str) -> String {
    source_control_diff_target_display_label_cow(target).into_owned()
}

fn source_control_diff_display_label_cow<'a>(label: &'a str) -> Cow<'a, str> {
    sanitized_display_label_cow(label, DISPLAY_PATH_LABEL_MAX_CHARS, "diff")
}

fn source_control_diff_display_label(label: &str) -> String {
    source_control_diff_display_label_cow(label).into_owned()
}

fn source_control_diff_display_label_owned(label: String) -> String {
    let owned_label = {
        let raw = label.as_str();
        match source_control_diff_display_label_cow(raw) {
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

    match owned_label {
        Some(label) => label,
        None => label,
    }
}

fn source_control_diff_title_label(label: &str, suffix: &str) -> String {
    let suffix = format!(" ({suffix})");
    let suffix_chars = suffix.chars().count();
    if suffix_chars >= DISPLAY_PATH_LABEL_MAX_CHARS {
        return source_control_diff_display_label_owned(format!("{label}{suffix}"));
    }

    let label_chars = DISPLAY_PATH_LABEL_MAX_CHARS - suffix_chars;
    format!(
        "{}{suffix}",
        sanitized_display_label_cow(label, label_chars, "diff")
    )
}

fn source_control_diff_missing_worktree_text_status(path: &Path) -> String {
    format!(
        "Could not diff {}: missing worktree text",
        source_control_diff_path_label_cow(path)
    )
}

fn source_control_head_missing_worktree_text_status(path: &Path) -> String {
    format!(
        "Could not compare {} with HEAD: missing worktree text",
        source_control_diff_path_label_cow(path)
    )
}

fn source_control_diff_read_failure_status(path: &Path, error: impl Display) -> String {
    let error = error.to_string();
    format!(
        "Could not read {}: {}",
        source_control_diff_path_label_cow(path),
        source_control_diff_error_label_cow(&error)
    )
}

fn source_control_worktree_diff_failure_status(path: &Path, error: impl Display) -> String {
    let error = error.to_string();
    format!(
        "Could not diff {}: {}",
        source_control_diff_path_label_cow(path),
        source_control_diff_error_label_cow(&error)
    )
}

fn source_control_staged_diff_failure_status(path: &Path, error: impl Display) -> String {
    let error = error.to_string();
    format!(
        "Could not diff staged {}: {}",
        source_control_diff_path_label_cow(path),
        source_control_diff_error_label_cow(&error)
    )
}

fn source_control_head_compare_failure_status(path: &Path, error: impl Display) -> String {
    let error = error.to_string();
    format!(
        "Could not compare {} with HEAD: {}",
        source_control_diff_path_label_cow(path),
        source_control_diff_error_label_cow(&error)
    )
}

fn source_control_no_git_changes_status(path: &Path) -> String {
    format!(
        "No git changes in {}",
        source_control_diff_path_label_cow(path)
    )
}

fn source_control_no_staged_changes_status(path: &Path) -> String {
    format!(
        "No staged changes in {}",
        source_control_diff_path_label_cow(path)
    )
}

fn source_control_no_head_changes_status(path: &Path) -> String {
    format!(
        "No HEAD changes in {}",
        source_control_diff_path_label_cow(path)
    )
}

fn source_control_side_by_side_open_status(kind: &str, target: &str) -> String {
    let target = source_control_diff_target_display_label_cow(target);
    source_control_side_by_side_open_status_for_label(kind, target.as_ref())
}

fn source_control_side_by_side_open_status_for_label(kind: &str, target: &str) -> String {
    format!("Opened side-by-side {} for {}", kind, target)
}

fn source_control_head_side_open_failure_status(path: &Path, error: impl Display) -> String {
    let error = error.to_string();
    format!(
        "Could not open HEAD side for {}: {}",
        source_control_diff_path_label_cow(path),
        source_control_diff_error_label_cow(&error)
    )
}

#[cfg(test)]
fn source_control_index_and_head_side_open_failure_status(
    path: &Path,
    index_error: impl Display,
    head_error: impl Display,
) -> String {
    let index_error = index_error.to_string();
    let head_error = head_error.to_string();
    format!(
        "Could not open index side for {}: {}; HEAD: {}",
        source_control_diff_path_label_cow(path),
        source_control_diff_error_label_cow(&index_error),
        source_control_diff_error_label_cow(&head_error)
    )
}

fn compute_source_control_diff_open(
    root: &Path,
    job: SourceControlDiffOpenJob,
    options: DiffOptions,
) -> Result<SourceControlDiffOpenOutcome, String> {
    match job.request {
        SourceControlDiffOpenRequest::Worktree { path, focus_hunk } => {
            let Some(text) = job.worktree_text else {
                return Err(source_control_diff_missing_worktree_text_status(&path));
            };
            compute_worktree_diff_open(
                root,
                path,
                text,
                focus_hunk,
                options,
                job.prepare_side_by_side,
            )
        }
        SourceControlDiffOpenRequest::Staged { path, focus_hunk } => {
            compute_staged_diff_open(root, path, focus_hunk, options, job.prepare_side_by_side)
        }
        SourceControlDiffOpenRequest::Head { path } => {
            let Some(text) = job.worktree_text else {
                return Err(source_control_head_missing_worktree_text_status(&path));
            };
            compute_head_diff_open(root, path, text, options, job.prepare_side_by_side)
        }
    }
}

fn compute_worktree_diff_open(
    root: &Path,
    path: PathBuf,
    text: SourceControlDiffText,
    focus_hunk: Option<usize>,
    options: DiffOptions,
    prepare_side_by_side: bool,
) -> Result<SourceControlDiffOpenOutcome, String> {
    let (text, worktree_source_available) = text
        .load_with_worktree_source_availability(options.max_file_size_bytes)
        .map_err(|error| source_control_diff_read_failure_status(&path, error))?;
    let diff = worktree_diff_with_index_text(root, &path, &text, options)
        .map_err(|error| source_control_worktree_diff_failure_status(&path, error))?;
    if diff.diff.trim().is_empty() {
        return Ok(SourceControlDiffOpenOutcome::Status(
            source_control_no_git_changes_status(&path),
        ));
    }
    let labels = SourceControlDiffPathLabels::new(&path);
    let side_by_side = source_control_side_by_side_open_if_needed(prepare_side_by_side, || {
        worktree_side_by_side_open_with_labels(
            root,
            &path,
            focus_hunk,
            &text,
            worktree_source_available,
            diff.index_text,
            &labels,
        )
    });
    Ok(SourceControlDiffOpenOutcome::Open(Box::new(
        SourceControlDiffOpen {
            open: VirtualDiffOpen {
                label: labels.diff_title("Changes"),
                diff: diff.diff,
                target: labels.target_label(),
                kind: "changes",
                source: Some(DiffBufferSource {
                    path,
                    base_path: None,
                    hunk_stage: Some(GitChangeStage::Unstaged),
                    saved_buffer_id: None,
                }),
            },
            focus_hunk,
            side_by_side,
        },
    )))
}

fn compute_staged_diff_open(
    root: &Path,
    path: PathBuf,
    focus_hunk: Option<usize>,
    options: DiffOptions,
    prepare_side_by_side: bool,
) -> Result<SourceControlDiffOpenOutcome, String> {
    let diff = staged_diff_with_texts(root, &path, options)
        .map_err(|error| source_control_staged_diff_failure_status(&path, error))?;
    if diff.diff.trim().is_empty() {
        return Ok(SourceControlDiffOpenOutcome::Status(
            source_control_no_staged_changes_status(&path),
        ));
    }
    let labels = SourceControlDiffPathLabels::new(&path);
    let side_by_side = source_control_side_by_side_open_if_needed(prepare_side_by_side, || {
        Ok(staged_side_by_side_open_with_labels(
            &path,
            diff.head_text,
            diff.index_text,
            &labels,
        ))
    });
    Ok(SourceControlDiffOpenOutcome::Open(Box::new(
        SourceControlDiffOpen {
            open: VirtualDiffOpen {
                label: labels.diff_title("Staged Changes"),
                diff: diff.diff,
                target: labels.target_label(),
                kind: "staged changes",
                source: Some(DiffBufferSource {
                    path,
                    base_path: None,
                    hunk_stage: Some(GitChangeStage::Staged),
                    saved_buffer_id: None,
                }),
            },
            focus_hunk,
            side_by_side,
        },
    )))
}

fn compute_head_diff_open(
    root: &Path,
    path: PathBuf,
    text: SourceControlDiffText,
    options: DiffOptions,
    prepare_side_by_side: bool,
) -> Result<SourceControlDiffOpenOutcome, String> {
    let (text, worktree_source_available) = text
        .load_with_worktree_source_availability(options.max_file_size_bytes)
        .map_err(|error| source_control_diff_read_failure_status(&path, error))?;
    let diff = head_diff_with_text(root, &path, &text, options)
        .map_err(|error| source_control_head_compare_failure_status(&path, error))?;
    if diff.diff.trim().is_empty() {
        return Ok(SourceControlDiffOpenOutcome::Status(
            source_control_no_head_changes_status(&path),
        ));
    }
    let labels = SourceControlDiffPathLabels::new(&path);
    let side_by_side = source_control_side_by_side_open_if_needed(prepare_side_by_side, || {
        Ok(head_side_by_side_open_with_labels(
            &path,
            &text,
            worktree_source_available,
            diff.head_text,
            &labels,
        ))
    });
    Ok(SourceControlDiffOpenOutcome::Open(Box::new(
        SourceControlDiffOpen {
            open: VirtualDiffOpen {
                label: labels.diff_title("Compare with HEAD"),
                diff: diff.diff,
                target: labels.target_label(),
                kind: "HEAD changes",
                source: Some(DiffBufferSource {
                    path,
                    base_path: None,
                    hunk_stage: None,
                    saved_buffer_id: None,
                }),
            },
            focus_hunk: None,
            side_by_side,
        },
    )))
}

impl KuroyaApp {
    fn open_source_control_side_by_side_diff(
        &mut self,
        diff: String,
        focus_hunk: Option<usize>,
        side_by_side: SourceControlSideBySideOpen,
    ) {
        let (base_line, source_line) = focus_hunk
            .and_then(|hunk| hunk_start_lines_in_unified_diff(&diff, hunk))
            .unwrap_or((1, 1));
        let base_pane = self.active_pane;
        let _base_id = self.open_source_control_side_by_side_buffer(
            base_pane,
            side_by_side.base,
            base_line,
            false,
        );
        let source_pane = self.insert_editor_pane_right(None);
        self.apply_source_control_diff_split_ratio(
            base_pane,
            source_pane,
            self.settings.diff_split_view_default_ratio,
        );
        let _source_id = self.open_source_control_side_by_side_buffer(
            source_pane,
            side_by_side.source,
            source_line,
            true,
        );
        self.status =
            source_control_side_by_side_open_status(side_by_side.kind, &side_by_side.target);
    }

    fn apply_source_control_diff_split_ratio(
        &mut self,
        base_pane: crate::workspace_state::PaneId,
        source_pane: crate::workspace_state::PaneId,
        split_ratio: f32,
    ) {
        let Some(base_index) = self.panes.iter().position(|pane| pane.id == base_pane) else {
            return;
        };
        let Some(source_index) = self.panes.iter().position(|pane| pane.id == source_pane) else {
            return;
        };
        let total_weight = self.panes[base_index].weight + self.panes[source_index].weight;
        let (base_weight, source_weight) =
            source_control_diff_split_weights(total_weight, split_ratio);
        self.panes[base_index].weight = base_weight;
        self.panes[source_index].weight = source_weight;
        self.normalize_pane_weights();
    }

    fn open_source_control_side_by_side_buffer(
        &mut self,
        pane_id: crate::workspace_state::PaneId,
        buffer: SourceControlSideBySideBuffer,
        line: usize,
        activate: bool,
    ) -> Option<kuroya_core::BufferId> {
        match buffer {
            SourceControlSideBySideBuffer::Virtual {
                label,
                path,
                target,
                text,
            } => {
                let id = self.open_virtual_revision_buffer_in_pane(
                    pane_id,
                    label,
                    path,
                    text,
                    target,
                    "diff side",
                    activate,
                );
                self.apply_file_jump(id, line, 1);
                Some(id)
            }
            SourceControlSideBySideBuffer::Worktree { path } => {
                self.open_worktree_diff_side_in_pane(pane_id, path, line, activate)
            }
        }
    }

    fn open_worktree_diff_side_in_pane(
        &mut self,
        pane_id: crate::workspace_state::PaneId,
        path: PathBuf,
        line: usize,
        activate: bool,
    ) -> Option<kuroya_core::BufferId> {
        if let Some(id) = self.buffer_by_path(&path).map(TextBuffer::id) {
            if activate {
                self.set_active_buffer_in_pane(pane_id, id);
            } else {
                self.assign_buffer_to_pane(pane_id, id);
            }
            self.apply_file_jump(id, line, 1);
            return Some(id);
        }

        self.pending_pane_paths.insert(pane_id, path.clone());
        self.pending_file_jump = Some(crate::transient_state::FileJump::char(
            path.clone(),
            line,
            1,
        ));
        self.active_pane = pane_id;
        if activate {
            self.focused_pane = Some(pane_id);
        }
        self.spawn_open_file_with_activation(path, activate);
        None
    }

    fn active_editor_pane_width(&self) -> Option<f32> {
        if self.editor_content_width <= 0.0 || !self.editor_content_width.is_finite() {
            return None;
        }
        self.panes
            .iter()
            .find(|pane| pane.id == self.active_pane)
            .map(|pane| (self.editor_content_width * pane.weight).max(0.0))
    }
}

pub(crate) fn source_control_diff_opens_side_by_side(
    render_side_by_side: bool,
    only_show_accessible_viewer: bool,
    side_by_side_available: bool,
    use_inline_view_when_space_is_limited: bool,
    inline_breakpoint: usize,
    available_width: Option<f32>,
) -> bool {
    if !render_side_by_side || only_show_accessible_viewer || !side_by_side_available {
        return false;
    }
    let inline_breakpoint = clamp_diff_render_side_by_side_inline_breakpoint(inline_breakpoint);
    if use_inline_view_when_space_is_limited
        && available_width
            .filter(|width| width.is_finite())
            .is_some_and(|width| width < inline_breakpoint as f32)
    {
        return false;
    }
    true
}

pub(crate) fn source_control_diff_split_weights(total_weight: f32, split_ratio: f32) -> (f32, f32) {
    let total_weight = if total_weight.is_finite() && total_weight > 0.0 {
        total_weight
    } else {
        1.0
    };
    if total_weight <= 0.02 {
        return (total_weight * 0.5, total_weight * 0.5);
    }

    let ratio = clamp_diff_split_view_default_ratio(split_ratio);
    let base_weight = (total_weight * ratio).clamp(0.01, total_weight - 0.01);
    (base_weight, total_weight - base_weight)
}

fn source_control_side_by_side_open_if_needed<T>(
    prepare_side_by_side: bool,
    open: impl FnOnce() -> Result<T, String>,
) -> Option<T> {
    if prepare_side_by_side {
        open().ok()
    } else {
        None
    }
}

#[cfg(test)]
fn worktree_side_by_side_open(
    root: &Path,
    path: &Path,
    focus_hunk: Option<usize>,
    text: &str,
    worktree_source_available: bool,
    index_text: Option<String>,
) -> Result<SourceControlSideBySideOpen, String> {
    let labels = SourceControlDiffPathLabels::new(path);
    worktree_side_by_side_open_with_labels(
        root,
        path,
        focus_hunk,
        text,
        worktree_source_available,
        index_text,
        &labels,
    )
}

fn worktree_side_by_side_open_with_labels(
    root: &Path,
    path: &Path,
    focus_hunk: Option<usize>,
    text: &str,
    worktree_source_available: bool,
    index_text: Option<String>,
    labels: &SourceControlDiffPathLabels,
) -> Result<SourceControlSideBySideOpen, String> {
    let (base_text, base_name) = worktree_base_text(root, path, index_text)?;
    let source =
        worktree_side_by_side_source_with_labels(path, text, worktree_source_available, labels);
    let kind = if focus_hunk.is_some() {
        "hunk changes"
    } else {
        "changes"
    };

    Ok(SourceControlSideBySideOpen {
        base: SourceControlSideBySideBuffer::Virtual {
            label: labels.diff_title(base_name),
            path: path.to_path_buf(),
            target: labels.target_label(),
            text: base_text.unwrap_or_default(),
        },
        source,
        target: labels.target_label(),
        kind,
    })
}

#[cfg(test)]
fn staged_side_by_side_open(
    path: &Path,
    head_text: Option<String>,
    index_text: Option<String>,
) -> SourceControlSideBySideOpen {
    let labels = SourceControlDiffPathLabels::new(path);
    staged_side_by_side_open_with_labels(path, head_text, index_text, &labels)
}

fn staged_side_by_side_open_with_labels(
    path: &Path,
    head_text: Option<String>,
    index_text: Option<String>,
    labels: &SourceControlDiffPathLabels,
) -> SourceControlSideBySideOpen {
    SourceControlSideBySideOpen {
        base: SourceControlSideBySideBuffer::Virtual {
            label: labels.diff_title("HEAD"),
            path: path.to_path_buf(),
            target: labels.target_label(),
            text: head_text.unwrap_or_default(),
        },
        source: SourceControlSideBySideBuffer::Virtual {
            label: labels.diff_title("Index"),
            path: path.to_path_buf(),
            target: labels.target_label(),
            text: index_text.unwrap_or_default(),
        },
        target: labels.target_label(),
        kind: "staged changes",
    }
}

#[cfg(test)]
fn head_side_by_side_open(
    path: &Path,
    text: &str,
    worktree_source_available: bool,
    head_text: Option<String>,
) -> SourceControlSideBySideOpen {
    let labels = SourceControlDiffPathLabels::new(path);
    head_side_by_side_open_with_labels(path, text, worktree_source_available, head_text, &labels)
}

fn head_side_by_side_open_with_labels(
    path: &Path,
    text: &str,
    worktree_source_available: bool,
    head_text: Option<String>,
    labels: &SourceControlDiffPathLabels,
) -> SourceControlSideBySideOpen {
    SourceControlSideBySideOpen {
        base: SourceControlSideBySideBuffer::Virtual {
            label: labels.diff_title("HEAD"),
            path: path.to_path_buf(),
            target: labels.target_label(),
            text: head_text.unwrap_or_default(),
        },
        source: worktree_side_by_side_source_with_labels(
            path,
            text,
            worktree_source_available,
            labels,
        ),
        target: labels.target_label(),
        kind: "HEAD changes",
    }
}

#[cfg(test)]
fn worktree_side_by_side_source(
    path: &Path,
    text: &str,
    worktree_source_available: bool,
) -> SourceControlSideBySideBuffer {
    let labels = SourceControlDiffPathLabels::new(path);
    worktree_side_by_side_source_with_labels(path, text, worktree_source_available, &labels)
}

fn worktree_side_by_side_source_with_labels(
    path: &Path,
    text: &str,
    worktree_source_available: bool,
    labels: &SourceControlDiffPathLabels,
) -> SourceControlSideBySideBuffer {
    if worktree_source_available {
        SourceControlSideBySideBuffer::Worktree {
            path: path.to_path_buf(),
        }
    } else {
        SourceControlSideBySideBuffer::Virtual {
            label: labels.diff_title("Working Tree"),
            path: path.to_path_buf(),
            target: labels.target_label(),
            text: text.to_owned(),
        }
    }
}

fn worktree_base_text(
    root: &Path,
    path: &Path,
    index_text: Option<String>,
) -> Result<(Option<String>, &'static str), String> {
    match index_text {
        Some(text) => Ok((Some(text), "Index")),
        None => file_text_at_head(root, path)
            .map(|text| (text, "HEAD"))
            .map_err(|error| source_control_head_side_open_failure_status(path, error)),
    }
}

fn read_source_control_diff_text(path: &Path, max_bytes: usize) -> Result<String, String> {
    read_utf8_text_file_with_limit(path, max_bytes)
}

pub(crate) fn source_control_diff_open_detail(request: &SourceControlDiffOpenRequest) -> String {
    match request {
        SourceControlDiffOpenRequest::Worktree { path, focus_hunk } => {
            diff_detail("worktree", path, *focus_hunk)
        }
        SourceControlDiffOpenRequest::Staged { path, focus_hunk } => {
            diff_detail("staged", path, *focus_hunk)
        }
        SourceControlDiffOpenRequest::Head { path } => {
            format!("HEAD {}", source_control_diff_path_label_cow(path))
        }
    }
}

#[cfg(test)]
fn source_control_diff_open_pending_status(request: &SourceControlDiffOpenRequest) -> String {
    source_control_diff_open_pending_status_for_detail(&source_control_diff_open_detail(request))
}

fn source_control_diff_open_pending_status_for_detail(detail: &str) -> String {
    format!("Preparing {detail}")
}

fn diff_detail(kind: &str, path: &Path, focus_hunk: Option<usize>) -> String {
    match focus_hunk {
        Some(hunk_index) => format!(
            "{kind} {} hunk {}",
            source_control_diff_path_label_cow(path),
            hunk_index + 1
        ),
        None => format!("{kind} {}", source_control_diff_path_label_cow(path)),
    }
}

#[cfg(test)]
mod tests;
