use crate::{
    KuroyaApp,
    file_io::{file_size_exceeds_limit, file_too_large_message, read_utf8_text_file_with_limit},
    git_diff_view::hunk_patch_from_unified_diff,
    path_display::{display_error_label_cow, display_path_label_cow, sanitized_display_label_cow},
    source_control_hunk_runtime::hunk_stage_label,
    ui_events::UiEvent,
    workspace_state::paths_match_exact_or_lexically,
};
use eframe::egui::Context;
use kuroya_core::{
    DiffOptions, GitChangeStage, GitCommitSummary, GitStashEntry, TextBuffer, TextSnapshot,
    unified_diff_against_index_with_options, unified_diff_against_worktree_with_options,
    unified_diff_for_commit, unified_diff_for_stash,
};
use std::{
    borrow::Cow,
    fmt::Write as _,
    path::{Path, PathBuf},
};

const SOURCE_CONTROL_PATCH_COMMIT_ID_DISPLAY_MAX_CHARS: usize = 64;

#[derive(Debug, Clone)]
pub(crate) enum SourceControlPatchCopyRequest {
    File {
        path: PathBuf,
        stage: GitChangeStage,
    },
    Stage {
        stage: GitChangeStage,
    },
    All,
    Hunk {
        path: PathBuf,
        stage: GitChangeStage,
        hunk_index: usize,
    },
    Commit {
        commit: GitCommitSummary,
    },
    Stash {
        stash: GitStashEntry,
    },
}

#[derive(Debug)]
pub(crate) enum SourceControlPatchCopyOutcome {
    Patch { text: String, file_count: usize },
    Empty,
}

#[derive(Debug)]
pub(crate) enum SourceControlPatchCopyInput {
    File {
        path: PathBuf,
        stage: GitChangeStage,
        worktree_text: Option<SourceControlPatchText>,
    },
}

#[derive(Debug)]
pub(crate) enum SourceControlPatchText {
    Snapshot(TextSnapshot),
    File(PathBuf),
    Deleted,
    TooLarge { bytes: usize },
}

impl KuroyaApp {
    pub(crate) fn spawn_source_control_patch_copy(
        &mut self,
        request: SourceControlPatchCopyRequest,
        inputs: Vec<SourceControlPatchCopyInput>,
    ) {
        let request_id = reserve_source_control_patch_copy_request_id_state(
            &mut self.source_control_patch_copy_next_request_id,
            &mut self.source_control_patch_copy_active_request_id,
        );
        let event_root = self.workspace.root.clone();
        let git_root = self.source_control_git_operation_root();
        let generation = self.workspace_event_generation;
        let options = self.diff_options();
        let tx = self.tx.clone();
        let (pending_status, task_detail) = {
            let labels = SourceControlPatchCopyStatusLabels::new(&request);
            let pending_status = labels.pending_status();
            (pending_status, labels.into_detail())
        };
        self.status = pending_status;
        self.record_async_task_started("Git Patch Copy", task_detail);
        self.runtime.spawn_blocking(move || {
            let result = compute_source_control_patch_copy(&git_root, &request, options, inputs);
            let _ = crate::ui_event_channel::send_ui_event(
                &tx,
                UiEvent::GitPatchCopyFinished {
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

    pub(crate) fn apply_source_control_patch_copy_finished(
        &mut self,
        ctx: Option<&Context>,
        root: PathBuf,
        operation_root: PathBuf,
        generation: u64,
        request_id: u64,
        request: SourceControlPatchCopyRequest,
        result: Result<SourceControlPatchCopyOutcome, String>,
    ) {
        if !self.workspace_event_is_current(&root, generation)
            || !self.source_control_git_operation_root_matches(&operation_root)
            || request_id != self.source_control_patch_copy_active_request_id
        {
            return;
        }
        self.source_control_patch_copy_active_request_id = 0;

        let labels = SourceControlPatchCopyStatusLabels::new(&request);
        self.status = match result {
            Ok(SourceControlPatchCopyOutcome::Empty) => labels.empty_status(),
            Ok(SourceControlPatchCopyOutcome::Patch { text, file_count }) => {
                if let Some(ctx) = ctx {
                    ctx.copy_text(text);
                    labels.success_status(file_count)
                } else {
                    labels.failure_status("clipboard context unavailable")
                }
            }
            Err(error) => labels.failure_status(&error),
        };
    }
}

impl SourceControlPatchCopyInput {
    pub(crate) fn staged(path: PathBuf) -> Self {
        Self::File {
            path,
            stage: GitChangeStage::Staged,
            worktree_text: None,
        }
    }

    pub(crate) fn unstaged(path: PathBuf, worktree_text: SourceControlPatchText) -> Self {
        Self::File {
            path,
            stage: GitChangeStage::Unstaged,
            worktree_text: Some(worktree_text),
        }
    }
}

impl SourceControlPatchText {
    pub(crate) fn open_buffer(buffer: &TextBuffer, max_bytes: usize) -> Self {
        let bytes = buffer.len_bytes();
        if source_control_patch_text_exceeds_max_bytes(bytes, max_bytes) {
            Self::TooLarge { bytes }
        } else {
            Self::Snapshot(buffer.text_snapshot())
        }
    }

    fn load(self, max_bytes: usize) -> Result<String, String> {
        match self {
            Self::Snapshot(text) => {
                let bytes = text.len_bytes();
                if source_control_patch_text_exceeds_max_bytes(bytes, max_bytes) {
                    return Err(source_control_patch_text_too_large_message(
                        bytes, max_bytes,
                    ));
                }
                Ok(text.text())
            }
            Self::Deleted => Ok(String::new()),
            Self::File(path) => read_utf8_text_file_with_limit(&path, max_bytes),
            Self::TooLarge { bytes } => Err(source_control_patch_text_too_large_message(
                bytes, max_bytes,
            )),
        }
    }
}

fn source_control_patch_text_exceeds_max_bytes(bytes: usize, max_bytes: usize) -> bool {
    file_size_exceeds_limit(
        u64::try_from(bytes).unwrap_or(u64::MAX),
        u64::try_from(max_bytes).unwrap_or(u64::MAX),
    )
}

fn source_control_patch_text_too_large_message(bytes: usize, max_bytes: usize) -> String {
    file_too_large_message(
        u64::try_from(bytes).unwrap_or(u64::MAX),
        u64::try_from(max_bytes).unwrap_or(u64::MAX),
    )
}

fn reserve_source_control_patch_copy_request_id_state(
    next_request_id: &mut u64,
    active_request_id: &mut u64,
) -> u64 {
    let mut request_id = next_source_control_patch_copy_request_id(*next_request_id);
    if request_id == *active_request_id && *active_request_id != 0 {
        request_id = next_source_control_patch_copy_request_id(request_id);
    }
    *next_request_id = request_id;
    *active_request_id = request_id;
    request_id
}

fn next_source_control_patch_copy_request_id(current: u64) -> u64 {
    match current.wrapping_add(1) {
        0 => 1,
        request_id => request_id,
    }
}

fn compute_source_control_patch_copy(
    root: &Path,
    request: &SourceControlPatchCopyRequest,
    options: DiffOptions,
    inputs: Vec<SourceControlPatchCopyInput>,
) -> Result<SourceControlPatchCopyOutcome, String> {
    match request {
        SourceControlPatchCopyRequest::File { path, stage } => {
            let Some(worktree_text) =
                source_control_patch_worktree_text_for_target(inputs, path, *stage)?
            else {
                return Ok(SourceControlPatchCopyOutcome::Empty);
            };
            patch_outcome_for_diff(
                file_patch_for_file(root, path, *stage, worktree_text, options)?,
                1,
            )
        }
        SourceControlPatchCopyRequest::Hunk {
            path,
            stage,
            hunk_index,
        } => {
            let Some(worktree_text) =
                source_control_patch_worktree_text_for_target(inputs, path, *stage)?
            else {
                return Ok(SourceControlPatchCopyOutcome::Empty);
            };
            let diff = file_patch_for_file(root, path, *stage, worktree_text, options)?;
            let Some(patch) = hunk_patch_from_unified_diff(&diff, *hunk_index) else {
                return Ok(SourceControlPatchCopyOutcome::Empty);
            };
            patch_outcome_for_diff(patch, 1)
        }
        SourceControlPatchCopyRequest::Stage { .. } | SourceControlPatchCopyRequest::All => {
            let mut joined = String::new();
            let mut count = 0;
            for input in inputs {
                let SourceControlPatchCopyInput::File {
                    path,
                    stage,
                    worktree_text,
                } = input;
                let patch = file_patch_for_file(root, &path, stage, worktree_text, options)
                    .map_err(|error| source_control_patch_file_error(&path, &error))?;
                if append_unified_patch(&mut joined, &patch) {
                    count += 1;
                }
            }
            patch_outcome_for_diff(joined, count)
        }
        SourceControlPatchCopyRequest::Commit { commit } => {
            let oid = source_control_patch_commit_target(&commit.oid)?;
            patch_outcome_for_diff(
                unified_diff_for_commit(root, oid).map_err(|error| error.to_string())?,
                1,
            )
        }
        SourceControlPatchCopyRequest::Stash { stash } => patch_outcome_for_diff(
            unified_diff_for_stash(root, stash.index).map_err(|error| error.to_string())?,
            1,
        ),
    }
}

fn source_control_patch_worktree_text_for_target(
    inputs: Vec<SourceControlPatchCopyInput>,
    target_path: &Path,
    target_stage: GitChangeStage,
) -> Result<Option<Option<SourceControlPatchText>>, String> {
    let mut matched = None;
    for input in inputs {
        let SourceControlPatchCopyInput::File {
            path,
            stage,
            worktree_text,
        } = input;
        if !source_control_patch_input_target_matches(&path, stage, target_path, target_stage) {
            continue;
        }

        if matched.is_some() {
            return Err(source_control_patch_ambiguous_target_error(
                target_path,
                target_stage,
            ));
        }
        matched = Some(worktree_text);
    }
    Ok(matched)
}

fn source_control_patch_input_target_matches(
    input_path: &Path,
    input_stage: GitChangeStage,
    target_path: &Path,
    target_stage: GitChangeStage,
) -> bool {
    input_stage == target_stage && paths_match_exact_or_lexically(input_path, target_path)
}

fn file_patch_for_file(
    root: &Path,
    path: &Path,
    stage: GitChangeStage,
    worktree_text: Option<SourceControlPatchText>,
    options: DiffOptions,
) -> Result<String, String> {
    match stage {
        GitChangeStage::Staged => unified_diff_against_index_with_options(root, path, options)
            .map_err(|error| error.to_string()),
        GitChangeStage::Unstaged => {
            let text = worktree_text
                .ok_or_else(|| "missing worktree text source".to_owned())?
                .load(options.max_file_size_bytes)?;
            unified_diff_against_worktree_with_options(root, path, &text, options)
                .map_err(|error| error.to_string())
        }
    }
}

fn append_unified_patch(joined: &mut String, patch: &str) -> bool {
    let patch = patch.trim_end();
    if patch.trim().is_empty() {
        return false;
    }
    if !joined.is_empty() {
        joined.reserve(patch.len() + 1);
        joined.push('\n');
    } else {
        joined.reserve(patch.len());
    }
    joined.push_str(patch);
    true
}

fn patch_outcome_for_diff(
    text: String,
    file_count: usize,
) -> Result<SourceControlPatchCopyOutcome, String> {
    if text.trim().is_empty() {
        Ok(SourceControlPatchCopyOutcome::Empty)
    } else {
        Ok(SourceControlPatchCopyOutcome::Patch { text, file_count })
    }
}

pub(crate) fn source_control_patch_copy_detail(request: &SourceControlPatchCopyRequest) -> String {
    SourceControlPatchCopyStatusLabels::new(request).into_detail()
}

#[cfg(test)]
pub(crate) fn source_control_patch_copy_pending_status(
    request: &SourceControlPatchCopyRequest,
) -> String {
    SourceControlPatchCopyStatusLabels::new(request).pending_status()
}

#[cfg(test)]
pub(crate) fn source_control_patch_copy_success_status_for_request(
    request: &SourceControlPatchCopyRequest,
    file_count: usize,
) -> String {
    SourceControlPatchCopyStatusLabels::new(request).success_status(file_count)
}

#[cfg(test)]
pub(crate) fn source_control_patch_copy_empty_status_for_request(
    request: &SourceControlPatchCopyRequest,
) -> String {
    SourceControlPatchCopyStatusLabels::new(request).empty_status()
}

#[cfg(test)]
pub(crate) fn source_control_patch_copy_failure_status_for_request(
    request: &SourceControlPatchCopyRequest,
    error: &str,
) -> String {
    SourceControlPatchCopyStatusLabels::new(request).failure_status(error)
}

struct SourceControlPatchCopyStatusLabels<'a> {
    request: &'a SourceControlPatchCopyRequest,
    path_label: Option<Cow<'a, str>>,
    commit_id_label: Option<Cow<'a, str>>,
    stash_ref_label: Option<String>,
}

impl<'a> SourceControlPatchCopyStatusLabels<'a> {
    fn new(request: &'a SourceControlPatchCopyRequest) -> Self {
        let path_label = match request {
            SourceControlPatchCopyRequest::File { path, .. }
            | SourceControlPatchCopyRequest::Hunk { path, .. } => {
                Some(source_control_patch_path_label_cow(path))
            }
            _ => None,
        };
        let commit_id_label = match request {
            SourceControlPatchCopyRequest::Commit { commit } => {
                Some(source_control_patch_commit_id_label_cow(&commit.short_oid))
            }
            _ => None,
        };
        let stash_ref_label = match request {
            SourceControlPatchCopyRequest::Stash { stash } => {
                Some(source_control_patch_stash_ref(stash))
            }
            _ => None,
        };
        Self {
            request,
            path_label,
            commit_id_label,
            stash_ref_label,
        }
    }

    fn path_label(&self) -> &str {
        self.path_label
            .as_deref()
            .expect("patch copy file and hunk requests have a display path label")
    }

    fn commit_id_label(&self) -> &str {
        self.commit_id_label
            .as_deref()
            .expect("patch copy commit requests have a display commit id label")
    }

    fn stash_ref_label(&self) -> &str {
        self.stash_ref_label
            .as_deref()
            .expect("patch copy stash requests have a display stash ref label")
    }

    #[cfg(test)]
    fn detail(&self) -> String {
        match self.request {
            SourceControlPatchCopyRequest::File { .. }
            | SourceControlPatchCopyRequest::Hunk { .. } => self.path_label().to_owned(),
            SourceControlPatchCopyRequest::Stage { stage } => {
                source_control_patch_group_label(*stage).to_owned()
            }
            SourceControlPatchCopyRequest::All => "all changes".to_owned(),
            SourceControlPatchCopyRequest::Commit { .. } => {
                format!("commit {}", self.commit_id_label())
            }
            SourceControlPatchCopyRequest::Stash { .. } => self.stash_ref_label().to_owned(),
        }
    }

    fn into_detail(self) -> String {
        match self.request {
            SourceControlPatchCopyRequest::File { .. }
            | SourceControlPatchCopyRequest::Hunk { .. } => self
                .path_label
                .expect("patch copy file and hunk requests have a display path label")
                .into_owned(),
            SourceControlPatchCopyRequest::Stage { stage } => {
                source_control_patch_group_label(*stage).to_owned()
            }
            SourceControlPatchCopyRequest::All => "all changes".to_owned(),
            SourceControlPatchCopyRequest::Commit { .. } => {
                let commit_id_label = self
                    .commit_id_label
                    .expect("patch copy commit requests have a display commit id label");
                format!("commit {}", commit_id_label.as_ref())
            }
            SourceControlPatchCopyRequest::Stash { .. } => self
                .stash_ref_label
                .expect("patch copy stash requests have a display stash ref label"),
        }
    }

    fn pending_status(&self) -> String {
        match self.request {
            SourceControlPatchCopyRequest::File { stage, .. } => format!(
                "Preparing {}patch for {}",
                source_control_patch_stage_prefix(*stage),
                self.path_label()
            ),
            SourceControlPatchCopyRequest::Stage { stage } => {
                format!(
                    "Preparing {} patch",
                    source_control_patch_group_label(*stage)
                )
            }
            SourceControlPatchCopyRequest::All => "Preparing all changes patch".to_owned(),
            SourceControlPatchCopyRequest::Hunk {
                stage, hunk_index, ..
            } => format!(
                "Preparing {}hunk {hunk_index} patch for {}",
                source_control_patch_stage_prefix(*stage),
                self.path_label()
            ),
            SourceControlPatchCopyRequest::Commit { .. } => {
                format!("Preparing patch for commit {}", self.commit_id_label())
            }
            SourceControlPatchCopyRequest::Stash { .. } => {
                format!("Preparing patch for {}", self.stash_ref_label())
            }
        }
    }

    fn success_status(&self, file_count: usize) -> String {
        match self.request {
            SourceControlPatchCopyRequest::File { stage, .. } => format!(
                "Copied {}patch for {}",
                source_control_patch_stage_prefix(*stage),
                self.path_label()
            ),
            SourceControlPatchCopyRequest::Stage { stage } => {
                let stage_label = source_control_patch_group_label(*stage);
                let noun = if file_count == 1 { "file" } else { "files" };
                format!("Copied {stage_label} patch for {file_count} {noun}")
            }
            SourceControlPatchCopyRequest::All => {
                let noun = if file_count == 1 { "change" } else { "changes" };
                format!("Copied all changes patch for {file_count} {noun}")
            }
            SourceControlPatchCopyRequest::Hunk {
                stage, hunk_index, ..
            } => format!(
                "Copied {} hunk {hunk_index} patch for {}",
                hunk_stage_label(*stage),
                self.path_label()
            ),
            SourceControlPatchCopyRequest::Commit { .. } => {
                format!("Copied patch for commit {}", self.commit_id_label())
            }
            SourceControlPatchCopyRequest::Stash { .. } => {
                format!("Copied patch for {}", self.stash_ref_label())
            }
        }
    }

    fn empty_status(&self) -> String {
        match self.request {
            SourceControlPatchCopyRequest::File { stage, .. } => format!(
                "No {}patch to copy for {}",
                source_control_patch_stage_prefix(*stage),
                self.path_label()
            ),
            SourceControlPatchCopyRequest::Stage { stage } => {
                let stage_label = source_control_patch_group_label(*stage);
                format!("No {stage_label} patch to copy")
            }
            SourceControlPatchCopyRequest::All => "No changes patch to copy".to_owned(),
            SourceControlPatchCopyRequest::Hunk {
                stage, hunk_index, ..
            } => format!(
                "No {} hunk {hunk_index} patch to copy for {}",
                hunk_stage_label(*stage),
                self.path_label()
            ),
            SourceControlPatchCopyRequest::Commit { .. } => {
                format!("No patch to copy for commit {}", self.commit_id_label())
            }
            SourceControlPatchCopyRequest::Stash { .. } => {
                format!("No patch to copy for {}", self.stash_ref_label())
            }
        }
    }

    fn failure_status(&self, error: &str) -> String {
        let error_label = source_control_patch_error_label_cow(error);
        match self.request {
            SourceControlPatchCopyRequest::File { stage, .. } => format!(
                "Could not copy {}patch for {}: {}",
                source_control_patch_stage_prefix(*stage),
                self.path_label(),
                error_label.as_ref()
            ),
            SourceControlPatchCopyRequest::Stage { stage } => {
                let stage_label = source_control_patch_group_label(*stage);
                format!(
                    "Could not copy {stage_label} patch: {}",
                    error_label.as_ref()
                )
            }
            SourceControlPatchCopyRequest::All => {
                format!("Could not copy all changes patch: {}", error_label.as_ref())
            }
            SourceControlPatchCopyRequest::Hunk {
                stage, hunk_index, ..
            } => format!(
                "Could not copy {} hunk {hunk_index} patch for {}: {}",
                hunk_stage_label(*stage),
                self.path_label(),
                error_label.as_ref()
            ),
            SourceControlPatchCopyRequest::Commit { .. } => {
                format!(
                    "Could not copy patch for commit {}: {}",
                    self.commit_id_label(),
                    error_label.as_ref()
                )
            }
            SourceControlPatchCopyRequest::Stash { .. } => {
                format!(
                    "Could not copy patch for {}: {}",
                    self.stash_ref_label(),
                    error_label.as_ref()
                )
            }
        }
    }
}

fn source_control_patch_stash_ref(stash: &GitStashEntry) -> String {
    let mut stash_ref = String::with_capacity(source_control_patch_stash_ref_len(stash.index));
    push_source_control_patch_stash_ref(&mut stash_ref, stash.index);
    stash_ref
}

fn push_source_control_patch_stash_ref(output: &mut String, index: usize) {
    output.push_str("stash@{");
    let _ = write!(output, "{index}");
    output.push('}');
}

fn source_control_patch_stash_ref_len(index: usize) -> usize {
    "stash@{}".len() + source_control_patch_usize_decimal_len(index)
}

fn source_control_patch_usize_decimal_len(mut value: usize) -> usize {
    let mut len = 1;
    while value >= 10 {
        value /= 10;
        len += 1;
    }
    len
}

#[cfg(test)]
fn source_control_patch_commit_id_label(short_oid: &str) -> String {
    source_control_patch_commit_id_label_cow(short_oid).into_owned()
}

fn source_control_patch_commit_id_label_cow(short_oid: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(
        short_oid,
        SOURCE_CONTROL_PATCH_COMMIT_ID_DISPLAY_MAX_CHARS,
        "commit",
    )
}

fn source_control_patch_stage_prefix(stage: GitChangeStage) -> &'static str {
    match stage {
        GitChangeStage::Staged => "staged ",
        GitChangeStage::Unstaged => "",
    }
}

fn source_control_patch_path_label_cow(path: &Path) -> Cow<'_, str> {
    display_path_label_cow(path)
}

fn source_control_patch_error_label_cow(error: &str) -> Cow<'_, str> {
    display_error_label_cow(error)
}

fn source_control_patch_file_error(path: &Path, error: &str) -> String {
    let path_label = source_control_patch_path_label_cow(path);
    let error_label = source_control_patch_error_label_cow(error);
    format!("{}: {}", path_label.as_ref(), error_label.as_ref())
}

fn source_control_patch_ambiguous_target_error(path: &Path, stage: GitChangeStage) -> String {
    let path_label = source_control_patch_path_label_cow(path);
    format!(
        "multiple {}source-control rows match {}",
        source_control_patch_stage_prefix(stage),
        path_label.as_ref()
    )
}

fn source_control_patch_commit_target(oid: &str) -> Result<&str, String> {
    let oid = oid.trim();
    if source_control_patch_commit_oid_is_full_hex(oid) {
        Ok(oid)
    } else {
        Err("commit target is not a full object id".to_owned())
    }
}

fn source_control_patch_commit_oid_is_full_hex(oid: &str) -> bool {
    matches!(oid.len(), 40 | 64) && oid.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn source_control_patch_group_label(stage: GitChangeStage) -> &'static str {
    match stage {
        GitChangeStage::Staged => "staged",
        GitChangeStage::Unstaged => "unstaged",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SOURCE_CONTROL_PATCH_COMMIT_ID_DISPLAY_MAX_CHARS, SourceControlPatchCopyInput,
        SourceControlPatchCopyOutcome, SourceControlPatchCopyRequest,
        SourceControlPatchCopyStatusLabels, SourceControlPatchText,
        compute_source_control_patch_copy, patch_outcome_for_diff,
        source_control_patch_commit_id_label, source_control_patch_commit_id_label_cow,
        source_control_patch_commit_target, source_control_patch_copy_detail,
        source_control_patch_copy_empty_status_for_request,
        source_control_patch_copy_failure_status_for_request,
        source_control_patch_copy_pending_status,
        source_control_patch_copy_success_status_for_request,
        source_control_patch_input_target_matches, source_control_patch_worktree_text_for_target,
    };
    use crate::{
        KuroyaApp,
        app_startup_context::AppStartupContext,
        path_display::{DISPLAY_ERROR_LABEL_MAX_CHARS, DISPLAY_PATH_LABEL_MAX_CHARS},
        terminal::TerminalPane,
    };
    use kuroya_core::{
        DiffOptions, EditorSettings, GitChangeStage, GitCommitSummary, GitStashEntry, TextBuffer,
        Workspace,
    };
    use std::{
        borrow::Cow,
        env, fs,
        path::{MAIN_SEPARATOR, PathBuf},
        time::{Instant, SystemTime, UNIX_EPOCH},
    };
    use tokio::runtime::Runtime;

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        env::temp_dir().join(format!(
            "kuroya-source-control-patch-{name}-{}-{nanos}",
            std::process::id()
        ))
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

    fn patch_request(path: &str) -> SourceControlPatchCopyRequest {
        SourceControlPatchCopyRequest::File {
            path: PathBuf::from(path),
            stage: GitChangeStage::Unstaged,
        }
    }

    fn hostile_display_path() -> PathBuf {
        PathBuf::from("workspace/src")
            .join(format!("bad\n{}\u{202e}tail.rs", "very-long-".repeat(32)))
    }

    fn hostile_error() -> String {
        format!(
            "first line\nsecond line \u{202e}\u{2066}{}",
            "detail-".repeat(DISPLAY_ERROR_LABEL_MAX_CHARS)
        )
    }

    fn assert_display_safe(label: &str) {
        assert!(!label.contains('\n'), "{label:?}");
        assert!(!label.contains('\r'), "{label:?}");
        assert!(!label.contains('\u{202e}'), "{label:?}");
        assert!(!label.contains('\u{2066}'), "{label:?}");
        assert!(!label.contains('\u{2069}'), "{label:?}");
    }

    #[test]
    fn stale_patch_copy_finished_after_newer_request_is_ignored() {
        let root = PathBuf::from("workspace");
        let mut app = app_for_test(root.clone());
        app.status = "newer patch pending".to_owned();
        app.source_control_patch_copy_active_request_id = 2;

        app.apply_source_control_patch_copy_finished(
            None,
            root.clone(),
            root,
            app.workspace_event_generation,
            1,
            patch_request("src/main.rs"),
            Ok(SourceControlPatchCopyOutcome::Empty),
        );

        assert_eq!(app.status, "newer patch pending");
    }

    #[test]
    fn stale_patch_copy_finished_after_generation_change_is_ignored() {
        let root = PathBuf::from("workspace");
        let mut app = app_for_test(root.clone());
        let stale_generation = app.workspace_event_generation;
        app.workspace_event_generation = stale_generation + 1;
        app.status = "current workspace".to_owned();
        app.source_control_patch_copy_active_request_id = 1;

        app.apply_source_control_patch_copy_finished(
            None,
            root.clone(),
            root,
            stale_generation,
            1,
            patch_request("src/main.rs"),
            Ok(SourceControlPatchCopyOutcome::Empty),
        );

        assert_eq!(app.status, "current workspace");
    }

    #[test]
    fn stale_patch_copy_finished_after_operation_root_change_is_ignored() {
        let root = PathBuf::from("workspace");
        let stale_operation_root = root.join("old-repo");
        let mut app = app_for_test(root.clone());
        app.status = "current operation root".to_owned();
        app.source_control_patch_copy_active_request_id = 1;

        app.apply_source_control_patch_copy_finished(
            None,
            root,
            stale_operation_root,
            app.workspace_event_generation,
            1,
            patch_request("src/main.rs"),
            Ok(SourceControlPatchCopyOutcome::Empty),
        );

        assert_eq!(app.status, "current operation root");
        assert_eq!(app.source_control_patch_copy_active_request_id, 1);
    }

    #[test]
    fn current_patch_copy_finished_applies_status() {
        let root = PathBuf::from("workspace");
        let mut app = app_for_test(root.clone());
        app.source_control_patch_copy_active_request_id = 7;

        app.apply_source_control_patch_copy_finished(
            None,
            root.clone(),
            root,
            app.workspace_event_generation,
            7,
            patch_request("src/main.rs"),
            Ok(SourceControlPatchCopyOutcome::Patch {
                text: "diff --git a/src/main.rs b/src/main.rs\n".to_owned(),
                file_count: 1,
            }),
        );

        assert_eq!(
            app.status,
            "Could not copy patch for main.rs: clipboard context unavailable"
        );
        assert_eq!(app.source_control_patch_copy_active_request_id, 0);
    }

    #[test]
    fn duplicate_current_patch_copy_finished_is_ignored_after_first_apply() {
        let root = PathBuf::from("workspace");
        let mut app = app_for_test(root.clone());
        app.source_control_patch_copy_active_request_id = 7;

        app.apply_source_control_patch_copy_finished(
            None,
            root.clone(),
            root.clone(),
            app.workspace_event_generation,
            7,
            patch_request("src/main.rs"),
            Ok(SourceControlPatchCopyOutcome::Empty),
        );
        app.status = "after first patch completion".to_owned();

        app.apply_source_control_patch_copy_finished(
            None,
            root.clone(),
            root,
            app.workspace_event_generation,
            7,
            patch_request("src/main.rs"),
            Ok(SourceControlPatchCopyOutcome::Patch {
                text: "diff --git a/src/main.rs b/src/main.rs\n".to_owned(),
                file_count: 1,
            }),
        );

        assert_eq!(app.status, "after first patch completion");
    }

    #[test]
    fn patch_copy_request_ids_wrap_to_nonzero_and_skip_active_id() {
        let mut next_request_id = u64::MAX;
        let mut active_request_id = 0;

        let request_id = super::reserve_source_control_patch_copy_request_id_state(
            &mut next_request_id,
            &mut active_request_id,
        );

        assert_eq!(request_id, 1);
        assert_eq!(next_request_id, 1);
        assert_eq!(active_request_id, 1);

        next_request_id = u64::MAX;
        active_request_id = 1;
        let request_id = super::reserve_source_control_patch_copy_request_id_state(
            &mut next_request_id,
            &mut active_request_id,
        );

        assert_eq!(request_id, 2);
        assert_eq!(next_request_id, 2);
        assert_eq!(active_request_id, 2);
    }

    #[test]
    fn patch_text_file_respects_size_limit_before_reading() {
        let path = temp_path("oversize.txt");
        fs::write(&path, "too large").unwrap();

        let error = SourceControlPatchText::File(path.clone())
            .load(3)
            .unwrap_err();

        assert!(error.contains("file is too large to open"));
        assert!(error.contains("9 B"));
        assert!(error.contains("3 B"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn patch_text_open_buffer_respects_size_limit_before_text_clone() {
        let buffer = TextBuffer::from_text(
            1,
            Some(PathBuf::from("workspace/src/large.rs")),
            "abcdef".to_owned(),
        );

        let error = SourceControlPatchText::open_buffer(&buffer, 3)
            .load(3)
            .unwrap_err();

        assert!(error.contains("file is too large to open"));
        assert!(error.contains("6 B"));
        assert!(error.contains("3 B"));
    }

    #[test]
    fn patch_text_open_buffer_uses_snapshot_for_allowed_text() {
        let buffer = TextBuffer::from_text(
            1,
            Some(PathBuf::from("workspace/src/main.rs")),
            "abcdef".to_owned(),
        );

        let text = SourceControlPatchText::open_buffer(&buffer, 99);

        match text {
            SourceControlPatchText::Snapshot(snapshot) => assert_eq!(snapshot.text(), "abcdef"),
            other => panic!("expected snapshot text source, got {other:?}"),
        }
    }

    #[test]
    fn patch_copy_file_request_ignores_mismatched_input_path_before_loading() {
        let input = SourceControlPatchCopyInput::File {
            path: PathBuf::from("workspace/src/other.rs"),
            stage: GitChangeStage::Unstaged,
            worktree_text: None,
        };

        let outcome = compute_source_control_patch_copy(
            PathBuf::from("workspace").as_path(),
            &SourceControlPatchCopyRequest::File {
                path: PathBuf::from("workspace/src/main.rs"),
                stage: GitChangeStage::Unstaged,
            },
            DiffOptions::default(),
            vec![input],
        )
        .unwrap();

        assert!(matches!(outcome, SourceControlPatchCopyOutcome::Empty));
    }

    #[test]
    fn patch_copy_hunk_request_ignores_mismatched_input_stage_before_loading() {
        let input = SourceControlPatchCopyInput::File {
            path: PathBuf::from("workspace/src/main.rs"),
            stage: GitChangeStage::Unstaged,
            worktree_text: None,
        };

        let outcome = compute_source_control_patch_copy(
            PathBuf::from("workspace").as_path(),
            &SourceControlPatchCopyRequest::Hunk {
                path: PathBuf::from("workspace/src/main.rs"),
                stage: GitChangeStage::Staged,
                hunk_index: 0,
            },
            DiffOptions::default(),
            vec![input],
        )
        .unwrap();

        assert!(matches!(outcome, SourceControlPatchCopyOutcome::Empty));
    }

    #[test]
    fn patch_copy_target_matching_preserves_raw_request_path_alias() {
        let raw_marker = format!("{MAIN_SEPARATOR}.{MAIN_SEPARATOR}");
        let raw_request_path = PathBuf::from(format!(
            "workspace{MAIN_SEPARATOR}src{MAIN_SEPARATOR}.{MAIN_SEPARATOR}main.rs"
        ));
        let input_path = PathBuf::from(format!(
            "workspace{MAIN_SEPARATOR}src{MAIN_SEPARATOR}main.rs"
        ));
        assert!(
            raw_request_path
                .as_os_str()
                .to_string_lossy()
                .contains(&raw_marker),
            "{raw_request_path:?}"
        );
        assert!(source_control_patch_input_target_matches(
            &input_path,
            GitChangeStage::Unstaged,
            &raw_request_path,
            GitChangeStage::Unstaged,
        ));

        let worktree_text = source_control_patch_worktree_text_for_target(
            vec![SourceControlPatchCopyInput::unstaged(
                input_path,
                SourceControlPatchText::Deleted,
            )],
            &raw_request_path,
            GitChangeStage::Unstaged,
        )
        .unwrap()
        .expect("lexically matching input should be accepted");

        assert!(matches!(
            worktree_text,
            Some(SourceControlPatchText::Deleted)
        ));
        assert!(
            raw_request_path
                .as_os_str()
                .to_string_lossy()
                .contains(&raw_marker),
            "{raw_request_path:?}"
        );
    }

    #[test]
    fn patch_copy_target_resolution_rejects_ambiguous_matching_rows() {
        let target_path = PathBuf::from(format!(
            "workspace{MAIN_SEPARATOR}src{MAIN_SEPARATOR}.{MAIN_SEPARATOR}main.rs"
        ));
        let input_path = PathBuf::from(format!(
            "workspace{MAIN_SEPARATOR}src{MAIN_SEPARATOR}main.rs"
        ));
        let aliased_input_path = PathBuf::from(format!(
            "workspace{MAIN_SEPARATOR}src{MAIN_SEPARATOR}.{MAIN_SEPARATOR}main.rs"
        ));

        let error = source_control_patch_worktree_text_for_target(
            vec![
                SourceControlPatchCopyInput::unstaged(input_path, SourceControlPatchText::Deleted),
                SourceControlPatchCopyInput::unstaged(
                    aliased_input_path,
                    SourceControlPatchText::TooLarge { bytes: 99 },
                ),
            ],
            &target_path,
            GitChangeStage::Unstaged,
        )
        .unwrap_err();

        assert_display_safe(&error);
        assert!(
            error.contains("multiple source-control rows match"),
            "{error}"
        );
        assert!(error.contains("main.rs"), "{error}");
    }

    #[test]
    fn patch_copy_hunk_request_rejects_ambiguous_target_before_loading_text() {
        let path = PathBuf::from("workspace/src/main.rs");

        let error = compute_source_control_patch_copy(
            PathBuf::from("workspace").as_path(),
            &SourceControlPatchCopyRequest::Hunk {
                path: path.clone(),
                stage: GitChangeStage::Unstaged,
                hunk_index: 0,
            },
            DiffOptions::default(),
            vec![
                SourceControlPatchCopyInput::unstaged(
                    path.clone(),
                    SourceControlPatchText::TooLarge { bytes: 99 },
                ),
                SourceControlPatchCopyInput::unstaged(path, SourceControlPatchText::Deleted),
            ],
        )
        .unwrap_err();

        assert_display_safe(&error);
        assert!(
            error.contains("multiple source-control rows match"),
            "{error}"
        );
        assert!(!error.contains("file is too large"), "{error}");
    }

    #[test]
    fn patch_copy_commit_request_rejects_non_full_oid_before_repo_lookup() {
        let request = SourceControlPatchCopyRequest::Commit {
            commit: GitCommitSummary {
                oid: "HEAD~1".to_owned(),
                short_oid: "HEAD~1".to_owned(),
                summary: "ambiguous ref".to_owned(),
                author: "Ada".to_owned(),
                time_seconds: 10,
            },
        };

        let error = compute_source_control_patch_copy(
            PathBuf::from("workspace").as_path(),
            &request,
            DiffOptions::default(),
            Vec::new(),
        )
        .unwrap_err();

        assert_eq!(error, "commit target is not a full object id");
    }

    #[test]
    fn patch_copy_commit_target_accepts_full_hex_oids_only() {
        let sha1 = "a".repeat(40);
        let sha256 = "b".repeat(64);

        assert_eq!(source_control_patch_commit_target(&sha1).unwrap(), sha1);
        assert_eq!(source_control_patch_commit_target(&sha256).unwrap(), sha256);
        assert!(source_control_patch_commit_target("abcdef").is_err());
        assert!(source_control_patch_commit_target("HEAD").is_err());
        assert!(source_control_patch_commit_target(&format!("{}g", "a".repeat(39))).is_err());
    }

    #[test]
    fn patch_copy_detail_and_pending_status_sanitize_path_labels_only() {
        let path = hostile_display_path();
        let request = SourceControlPatchCopyRequest::Hunk {
            path: path.clone(),
            stage: GitChangeStage::Unstaged,
            hunk_index: 3,
        };

        let detail = source_control_patch_copy_detail(&request);
        let pending = source_control_patch_copy_pending_status(&request);

        assert_display_safe(&detail);
        assert_display_safe(&pending);
        assert!(detail.contains("..."), "{detail}");
        assert!(pending.contains("..."), "{pending}");
        assert!(detail.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);
        match &request {
            SourceControlPatchCopyRequest::Hunk {
                path: request_path,
                hunk_index,
                ..
            } => {
                assert_eq!(request_path, &path);
                assert_eq!(*hunk_index, 3);
                assert!(request_path.to_string_lossy().contains('\n'));
                assert!(request_path.to_string_lossy().contains('\u{202e}'));
            }
            other => panic!("expected hunk request, got {other:?}"),
        }
    }

    #[test]
    fn patch_copy_status_labels_reuse_display_path_without_rewriting_request() {
        let path = hostile_display_path();
        let request = SourceControlPatchCopyRequest::Hunk {
            path: path.clone(),
            stage: GitChangeStage::Unstaged,
            hunk_index: 4,
        };

        let labels = SourceControlPatchCopyStatusLabels::new(&request);
        let cached_path_label = labels
            .path_label
            .clone()
            .expect("hunk request should cache a path label");
        let detail = labels.detail();
        let pending = labels.pending_status();
        let success = labels.success_status(1);
        let empty = labels.empty_status();
        let failure = labels.failure_status(&hostile_error());

        assert_eq!(detail, cached_path_label);
        for status in [&pending, &success, &empty, &failure] {
            assert_display_safe(status);
            assert!(status.contains(cached_path_label.as_ref()), "{status}");
        }
        assert_display_safe(&cached_path_label);
        assert!(cached_path_label.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);
        match &request {
            SourceControlPatchCopyRequest::Hunk {
                path: request_path,
                hunk_index,
                ..
            } => {
                assert_eq!(request_path, &path);
                assert_eq!(*hunk_index, 4);
                assert!(request_path.to_string_lossy().contains('\n'));
                assert!(request_path.to_string_lossy().contains('\u{202e}'));
            }
            other => panic!("expected hunk request, got {other:?}"),
        }
    }

    #[test]
    fn patch_copy_status_labels_reuse_stash_ref_label() {
        let request = SourceControlPatchCopyRequest::Stash {
            stash: GitStashEntry {
                index: 42,
                short_oid: "abcdef12".to_owned(),
                message: "WIP".to_owned(),
            },
        };

        let labels = SourceControlPatchCopyStatusLabels::new(&request);
        let cached_stash_ref = labels
            .stash_ref_label
            .clone()
            .expect("stash request should cache a stash ref label");

        assert_eq!(cached_stash_ref, "stash@{42}");
        assert_eq!(labels.detail(), cached_stash_ref);
        assert_eq!(labels.pending_status(), "Preparing patch for stash@{42}");
        assert_eq!(labels.success_status(1), "Copied patch for stash@{42}");
        assert_eq!(labels.empty_status(), "No patch to copy for stash@{42}");
        assert_eq!(
            labels.failure_status("diff failed"),
            "Could not copy patch for stash@{42}: diff failed"
        );
    }

    #[test]
    fn patch_copy_failure_status_sanitizes_path_and_error_labels() {
        let path = hostile_display_path();
        let error = hostile_error();
        let request = SourceControlPatchCopyRequest::File {
            path,
            stage: GitChangeStage::Unstaged,
        };

        let status = source_control_patch_copy_failure_status_for_request(&request, &error);

        assert_display_safe(&status);
        assert!(status.starts_with("Could not copy patch for "));
        assert!(status.contains("..."), "{status}");
        assert!(
            status.chars().count()
                <= "Could not copy patch for : ".len()
                    + DISPLAY_PATH_LABEL_MAX_CHARS
                    + DISPLAY_ERROR_LABEL_MAX_CHARS
        );
        assert!(error.contains('\n'));
        assert!(error.contains('\u{202e}'));
    }

    #[test]
    fn patch_copy_commit_id_label_cow_borrows_clean_display_ids() {
        let ascii = "abcdef123456";
        assert!(matches!(
            source_control_patch_commit_id_label_cow(ascii),
            Cow::Borrowed(label) if label == ascii
        ));

        let unicode = "feature-\u{03c0}";
        match source_control_patch_commit_id_label_cow(unicode) {
            Cow::Borrowed(label) => assert_eq!(label, unicode),
            Cow::Owned(label) => {
                panic!("expected borrowed clean Unicode commit label, got {label:?}")
            }
        }
    }

    #[test]
    fn patch_copy_commit_id_label_cow_owns_dirty_truncated_and_fallback_ids() {
        match source_control_patch_commit_id_label_cow("abc\n123") {
            Cow::Owned(label) => assert_eq!(label, "abc 123"),
            Cow::Borrowed(label) => panic!("expected owned dirty commit label, got {label:?}"),
        }

        let long = "a".repeat(SOURCE_CONTROL_PATCH_COMMIT_ID_DISPLAY_MAX_CHARS + 1);
        match source_control_patch_commit_id_label_cow(&long) {
            Cow::Owned(label) => {
                assert!(label.contains("..."), "{label}");
                assert!(label.chars().count() <= SOURCE_CONTROL_PATCH_COMMIT_ID_DISPLAY_MAX_CHARS);
            }
            Cow::Borrowed(label) => panic!("expected owned truncated commit label, got {label:?}"),
        }

        match source_control_patch_commit_id_label_cow("\n\u{202e}") {
            Cow::Owned(label) => assert_eq!(label, "commit"),
            Cow::Borrowed(label) => panic!("expected owned fallback commit label, got {label:?}"),
        }
    }

    #[test]
    fn patch_copy_commit_id_label_string_wrapper_matches_cow_output() {
        let long = "a".repeat(SOURCE_CONTROL_PATCH_COMMIT_ID_DISPLAY_MAX_CHARS + 1);
        for short_oid in [
            "abcdef123456",
            "feature-\u{03c0}",
            "abc\n123",
            "\n\u{202e}",
            long.as_str(),
        ] {
            assert_eq!(
                source_control_patch_commit_id_label(short_oid),
                source_control_patch_commit_id_label_cow(short_oid).as_ref()
            );
        }
    }

    #[test]
    fn patch_copy_commit_statuses_sanitize_display_id_without_changing_request() {
        let raw_short_oid = format!("12\u{202e}\n34{}", "a".repeat(120));
        let request = SourceControlPatchCopyRequest::Commit {
            commit: GitCommitSummary {
                oid: format!("{raw_short_oid}{}", "b".repeat(120)),
                short_oid: raw_short_oid.clone(),
                summary: "raw commit".to_owned(),
                author: "Ada".to_owned(),
                time_seconds: 10,
            },
        };

        let detail = source_control_patch_copy_detail(&request);
        let pending = source_control_patch_copy_pending_status(&request);
        let success = source_control_patch_copy_success_status_for_request(&request, 1);
        let empty = source_control_patch_copy_empty_status_for_request(&request);
        let failure =
            source_control_patch_copy_failure_status_for_request(&request, &hostile_error());

        for status in [&detail, &pending, &success, &empty, &failure] {
            assert_display_safe(status);
            assert!(status.contains("12 34"), "{status}");
            assert!(status.contains("..."), "{status}");
        }
        assert!(
            detail.chars().count()
                <= "commit ".chars().count() + SOURCE_CONTROL_PATCH_COMMIT_ID_DISPLAY_MAX_CHARS
        );
        match &request {
            SourceControlPatchCopyRequest::Commit { commit } => {
                assert_eq!(commit.short_oid, raw_short_oid);
                assert!(commit.short_oid.contains('\n'));
                assert!(commit.short_oid.contains('\u{202e}'));
            }
            other => panic!("expected commit request, got {other:?}"),
        }
    }

    #[test]
    fn patch_copy_stage_per_file_errors_sanitize_path_and_error_text() {
        let path = hostile_display_path();
        let input = SourceControlPatchCopyInput::File {
            path: path.clone(),
            stage: GitChangeStage::Unstaged,
            worktree_text: None,
        };

        let error = compute_source_control_patch_copy(
            PathBuf::from("workspace").as_path(),
            &SourceControlPatchCopyRequest::Stage {
                stage: GitChangeStage::Unstaged,
            },
            DiffOptions::default(),
            vec![input],
        )
        .unwrap_err();

        assert_display_safe(&error);
        assert!(error.contains("missing worktree text source"), "{error}");
        assert!(error.contains("..."), "{error}");
        assert!(
            error.chars().count()
                <= DISPLAY_PATH_LABEL_MAX_CHARS + DISPLAY_ERROR_LABEL_MAX_CHARS + 2
        );
        assert!(path.to_string_lossy().contains('\n'));
        assert!(path.to_string_lossy().contains('\u{202e}'));
    }

    #[test]
    fn patch_copy_outcome_preserves_raw_patch_text() {
        let raw_patch = "diff --git a/bad b/bad\n+first line\n+second \u{202e} line\n".to_owned();

        let outcome = patch_outcome_for_diff(raw_patch.clone(), 1).unwrap();

        match outcome {
            SourceControlPatchCopyOutcome::Patch { text, file_count } => {
                assert_eq!(text, raw_patch);
                assert_eq!(file_count, 1);
            }
            other => panic!("expected patch outcome, got {other:?}"),
        }
    }
}
