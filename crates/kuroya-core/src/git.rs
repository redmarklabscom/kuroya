mod branch;
mod commit;
mod diff;
mod paths;
mod status;

#[cfg(test)]
use self::branch::repository_branch_mutation_block_label;
use self::branch::{branch_name, upstream_divergence};
pub use self::branch::{
    checkout_branch, checkout_ref, create_branch, delete_branch, list_checkout_refs,
    list_local_branches, rename_branch,
};
use self::commit::git_commit_signature;
pub use self::commit::{
    clamp_git_commit_short_hash_length, commit_changes, commit_changes_with_empty_commit_option,
    commit_changes_with_options, commit_changes_with_short_hash_length,
    commit_changes_with_user_config_option, commit_staged_changes,
};
#[cfg(test)]
use self::commit::{
    guessed_git_signature_email_from, guessed_git_signature_name_from, merge_head_commits,
};
#[cfg(test)]
use self::diff::{DiffHunk, line_change_kinds, replace_hunk_lines, unified_diff_hunks};
use self::diff::{
    apply_hunk_to_new_text, apply_hunk_to_old_text, diff_hunks, diff_to_patch_text,
    line_change_kinds_with_options, unified_diff_for_texts,
};
pub use self::diff::{
    clamp_diff_context_lines, clamp_diff_hide_unchanged_regions_minimum_line_count,
    clamp_diff_hide_unchanged_regions_reveal_line_count, clamp_diff_max_computation_time_ms,
    clamp_diff_max_file_size_mb, diff_max_file_size_bytes,
    try_unified_diff_between_texts_with_options, unified_diff_between_texts,
    unified_diff_between_texts_with_options,
};
use self::paths::{
    DiscardPlan, GitRequestedPath, discover_worktree_repository, first_status_entry_path_label,
    git_path_display, git_path_label_from_path, git_status_relative_path,
    normalize_git_relative_path, status_entry_matches_requested_keys,
};
#[cfg(test)]
use self::paths::{
    GIT_PATH_LABEL_HEAD_CHARS, GIT_PATH_LABEL_OMISSION, GitDiffLabels, MAX_GIT_PATH_LABEL_CHARS,
    git_path_display_with_prefix, git_path_label, path_matches_requested_keys,
    status_path_matches_requested_keys, worktree_relative_path,
};
pub use self::status::{GitStatusCounts, GitStatusEntry};
use self::status::{GitStatusLookup, status_entries};
use anyhow::{Context, anyhow};
use git2::{
    BlameOptions, Index, IndexEntry, IndexTime, ObjectType, Oid, Repository, Sort, StashFlags,
    StatusOptions, build::CheckoutBuilder,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GitFileStatus {
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
    Conflicted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GitChangeStage {
    Staged,
    Unstaged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GitCheckoutType {
    Local,
    Remote,
    Tags,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitBranch {
    pub name: String,
    pub is_current: bool,
    pub kind: GitCheckoutType,
    pub committer_time_seconds: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitCommitSummary {
    pub oid: String,
    pub short_oid: String,
    pub summary: String,
    pub author: String,
    pub time_seconds: i64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GitTimelineDate {
    #[default]
    Committed,
    Authored,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitBlameLine {
    pub line_number: usize,
    pub short_oid: String,
    pub author: String,
    pub author_time_seconds: i64,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitStashEntry {
    pub index: usize,
    pub short_oid: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GitRemoteDivergence {
    pub incoming: usize,
    pub outgoing: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitDiffHunk {
    pub index: usize,
    pub fingerprint: u64,
    pub old_start: usize,
    pub old_lines: usize,
    pub new_start: usize,
    pub new_lines: usize,
    pub additions: usize,
    pub deletions: usize,
    pub header: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHeadDiff {
    pub diff: String,
    pub head_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitStagedDiff {
    pub diff: String,
    pub head_text: Option<String>,
    pub index_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitWorktreeDiff {
    pub diff: String,
    pub index_text: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GitSmartCommitChanges {
    #[default]
    All,
    Tracked,
}

pub const MIN_DIFF_CONTEXT_LINES: usize = 0;
pub const DEFAULT_DIFF_CONTEXT_LINES: usize = 3;
pub const MAX_DIFF_CONTEXT_LINES: usize = 200;
pub const MIN_DIFF_HIDE_UNCHANGED_REGIONS_MINIMUM_LINE_COUNT: usize = 0;
pub const DEFAULT_DIFF_HIDE_UNCHANGED_REGIONS_MINIMUM_LINE_COUNT: usize = 3;
pub const MAX_DIFF_HIDE_UNCHANGED_REGIONS_MINIMUM_LINE_COUNT: usize = 200;
pub const MIN_DIFF_HIDE_UNCHANGED_REGIONS_REVEAL_LINE_COUNT: usize = 0;
pub const DEFAULT_DIFF_HIDE_UNCHANGED_REGIONS_REVEAL_LINE_COUNT: usize = 20;
pub const MAX_DIFF_HIDE_UNCHANGED_REGIONS_REVEAL_LINE_COUNT: usize = 200;
pub const MIN_DIFF_MAX_FILE_SIZE_MB: usize = 0;
pub const DEFAULT_DIFF_MAX_FILE_SIZE_MB: usize = 50;
pub const MAX_DIFF_MAX_FILE_SIZE_MB: usize = 1024;
pub const MIN_DIFF_MAX_COMPUTATION_TIME_MS: usize = 0;
pub const DEFAULT_DIFF_MAX_COMPUTATION_TIME_MS: usize = 5_000;
pub const MAX_DIFF_MAX_COMPUTATION_TIME_MS: usize = 600_000;
pub const MIN_GIT_COMMIT_SHORT_HASH_LENGTH: usize = 7;
pub const DEFAULT_GIT_COMMIT_SHORT_HASH_LENGTH: usize = 7;
pub const MAX_GIT_COMMIT_SHORT_HASH_LENGTH: usize = 40;
const MAX_GIT_COMMIT_HISTORY_LIMIT: usize = 10_000;
pub const MIN_GIT_STATUS_LIMIT: usize = 0;
pub const DEFAULT_GIT_STATUS_LIMIT: usize = 10_000;
pub const MAX_GIT_STATUS_LIMIT: usize = 1_000_000;
pub const MIN_GIT_DETECT_SUBMODULES_LIMIT: usize = 0;
pub const DEFAULT_GIT_DETECT_SUBMODULES_LIMIT: usize = 10;
pub const MAX_GIT_DETECT_SUBMODULES_LIMIT: usize = 10_000;
pub const MIN_GIT_SIMILARITY_THRESHOLD: usize = 0;
pub const DEFAULT_GIT_SIMILARITY_THRESHOLD: usize = 50;
pub const MAX_GIT_SIMILARITY_THRESHOLD: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiffAlgorithm {
    Legacy,
    #[default]
    Advanced,
    #[serde(rename = "advanced-external")]
    AdvancedExternal,
    #[serde(rename = "advanced-wasm")]
    AdvancedWasm,
}

impl DiffAlgorithm {
    pub fn uses_advanced_diff(self) -> bool {
        !matches!(self, Self::Legacy)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffOptions {
    pub ignore_trim_whitespace: bool,
    pub algorithm: DiffAlgorithm,
    pub hide_unchanged_regions: bool,
    pub context_lines: usize,
    pub hide_unchanged_regions_minimum_line_count: usize,
    pub hide_unchanged_regions_reveal_line_count: usize,
    pub max_computation_time_ms: usize,
    pub max_file_size_bytes: usize,
}

impl Default for DiffOptions {
    fn default() -> Self {
        Self {
            ignore_trim_whitespace: false,
            algorithm: DiffAlgorithm::default(),
            hide_unchanged_regions: true,
            context_lines: DEFAULT_DIFF_CONTEXT_LINES,
            hide_unchanged_regions_minimum_line_count:
                DEFAULT_DIFF_HIDE_UNCHANGED_REGIONS_MINIMUM_LINE_COUNT,
            hide_unchanged_regions_reveal_line_count:
                DEFAULT_DIFF_HIDE_UNCHANGED_REGIONS_REVEAL_LINE_COUNT,
            max_computation_time_ms: DEFAULT_DIFF_MAX_COMPUTATION_TIME_MS,
            max_file_size_bytes: diff_max_file_size_bytes(DEFAULT_DIFF_MAX_FILE_SIZE_MB),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GitLineChangeKind {
    Added,
    Modified,
    Deleted,
}

#[derive(Debug, Clone, Default)]
pub struct GitSnapshot {
    root: Option<PathBuf>,
    branch: Option<String>,
    entries: Vec<GitStatusEntry>,
    statuses: HashMap<PathBuf, GitStatusLookup>,
    counts: GitStatusCounts,
    status_limited: bool,
    remote_divergence: Option<GitRemoteDivergence>,
}

impl GitSnapshot {
    pub fn scan(workspace_root: &Path) -> Self {
        Self::scan_with_status_limit(workspace_root, DEFAULT_GIT_STATUS_LIMIT)
    }

    pub fn scan_with_status_limit(workspace_root: &Path, status_limit: usize) -> Self {
        Self::scan_with_status_options(
            workspace_root,
            status_limit,
            false,
            true,
            DEFAULT_GIT_DETECT_SUBMODULES_LIMIT,
            DEFAULT_GIT_SIMILARITY_THRESHOLD,
        )
    }

    pub fn scan_with_status_options(
        workspace_root: &Path,
        status_limit: usize,
        ignore_submodules: bool,
        detect_submodules: bool,
        detect_submodules_limit: usize,
        similarity_threshold: usize,
    ) -> Self {
        Self::scan_with_status_options_and_parent_policy(
            workspace_root,
            status_limit,
            ignore_submodules,
            detect_submodules,
            detect_submodules_limit,
            similarity_threshold,
            true,
        )
    }

    pub fn scan_with_status_options_and_parent_policy(
        workspace_root: &Path,
        status_limit: usize,
        ignore_submodules: bool,
        detect_submodules: bool,
        detect_submodules_limit: usize,
        similarity_threshold: usize,
        open_parent_repositories: bool,
    ) -> Self {
        let Ok(repo) = scan_repository(workspace_root, open_parent_repositories) else {
            return Self::default();
        };
        let Some(workdir) = repo.workdir().map(Path::to_path_buf) else {
            return Self::default();
        };

        let submodules = git_submodule_detection(
            &repo,
            ignore_submodules,
            detect_submodules,
            detect_submodules_limit,
        );
        let mut options = StatusOptions::new();
        options
            .include_untracked(true)
            .recurse_untracked_dirs(true)
            .renames_head_to_index(true)
            .renames_index_to_workdir(true)
            .exclude_submodules(submodules.exclude_all)
            .rename_threshold(clamp_git_similarity_threshold(similarity_threshold) as u16);

        let Ok(statuses) = repo.statuses(Some(&mut options)) else {
            return Self::default();
        };

        let status_limit = clamp_git_status_limit(status_limit);
        let status_count = statuses.len();
        let entry_capacity = status_count.saturating_mul(2).min(status_limit);
        let status_capacity = status_count.min(status_limit);
        let mut entries = Vec::with_capacity(entry_capacity);
        let mut files = HashMap::with_capacity(status_capacity);
        let mut counts = GitStatusCounts::default();
        let mut emitted_statuses = 0usize;
        let mut status_limited = false;
        for (status_index, entry) in statuses.iter().enumerate() {
            let Some(relative) = entry.path().ok().and_then(git_status_relative_path) else {
                continue;
            };
            if submodules.excludes(&relative) {
                continue;
            }
            let path = workdir.join(&relative);
            let mut lookup: Option<GitStatusLookup> = None;
            for (kind, stage) in status_entries(entry.status()) {
                if emitted_statuses >= status_limit {
                    status_limited = true;
                    break;
                }
                counts.record(kind);
                emitted_statuses += 1;
                match lookup.as_mut() {
                    Some(lookup) => lookup.record(kind, stage),
                    None => lookup = Some(GitStatusLookup::new(kind, stage)),
                }
                entries.push(GitStatusEntry {
                    path: path.clone(),
                    status: kind,
                    stage,
                });
            }
            if let Some(lookup) = lookup {
                files
                    .entry(path)
                    .and_modify(|existing: &mut GitStatusLookup| existing.merge(lookup))
                    .or_insert(lookup);
            }
            if emitted_statuses >= status_limit && status_index + 1 < status_count {
                status_limited = true;
                break;
            }
        }
        sort_status_entries(&mut entries);

        Self {
            root: Some(workdir),
            branch: branch_name(&repo),
            entries,
            statuses: files,
            counts,
            status_limited,
            remote_divergence: upstream_divergence(&repo).ok().flatten(),
        }
    }

    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    pub fn branch(&self) -> Option<&str> {
        self.branch.as_deref()
    }

    pub fn status_for(&self, path: &Path) -> Option<GitFileStatus> {
        self.statuses.get(path).map(|status| status.status())
    }

    pub fn has_stage_for(&self, path: &Path, stage: GitChangeStage) -> bool {
        self.statuses
            .get(path)
            .is_some_and(|status| status.has_stage(stage))
    }

    pub fn entries_slice(&self) -> &[GitStatusEntry] {
        &self.entries
    }

    pub fn entries(&self) -> Vec<GitStatusEntry> {
        let mut entries = self.entries.clone();
        sort_status_entries(&mut entries);
        entries
    }

    pub fn counts(&self) -> GitStatusCounts {
        self.counts
    }

    pub fn status_limited(&self) -> bool {
        self.status_limited
    }

    pub fn remote_divergence(&self) -> Option<GitRemoteDivergence> {
        self.remote_divergence
    }

    pub fn len(&self) -> usize {
        self.statuses.len()
    }

    pub fn is_empty(&self) -> bool {
        self.statuses.is_empty()
    }
}

fn sort_status_entries(entries: &mut [GitStatusEntry]) {
    entries.sort_by(|left, right| {
        left.stage
            .cmp(&right.stage)
            .then_with(|| left.path.cmp(&right.path))
    });
}

fn scan_repository(
    workspace_root: &Path,
    open_parent_repositories: bool,
) -> Result<Repository, git2::Error> {
    if open_parent_repositories {
        Repository::discover(workspace_root)
    } else {
        Repository::open(workspace_root)
    }
}

#[derive(Debug, Clone, Default)]
struct GitSubmoduleDetection {
    exclude_all: bool,
    all: BTreeSet<PathBuf>,
    detected: BTreeSet<PathBuf>,
}

impl GitSubmoduleDetection {
    fn excludes(&self, relative: &Path) -> bool {
        if self.exclude_all {
            return false;
        }
        self.submodule_root(relative)
            .is_some_and(|root| !self.detected.contains(root))
    }

    fn submodule_root(&self, relative: &Path) -> Option<&PathBuf> {
        self.all.iter().find(|path| {
            relative == path.as_path()
                || relative
                    .strip_prefix(path)
                    .is_ok_and(|suffix| suffix.components().next().is_some())
        })
    }
}

fn git_submodule_detection(
    repo: &Repository,
    ignore_submodules: bool,
    detect_submodules: bool,
    detect_submodules_limit: usize,
) -> GitSubmoduleDetection {
    let detect_submodules_limit = clamp_git_detect_submodules_limit(detect_submodules_limit);
    if ignore_submodules || !detect_submodules || detect_submodules_limit == 0 {
        return GitSubmoduleDetection {
            exclude_all: true,
            ..GitSubmoduleDetection::default()
        };
    }

    let all = repo
        .submodules()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|submodule| normalize_git_relative_path(submodule.path()))
        .filter(|path| !path.as_os_str().is_empty())
        .collect::<BTreeSet<_>>();

    let detected = all
        .iter()
        .take(detect_submodules_limit)
        .cloned()
        .collect::<BTreeSet<_>>();
    GitSubmoduleDetection {
        exclude_all: false,
        all,
        detected,
    }
}

pub fn path_is_committed(workspace_root: &Path, path: &Path) -> anyhow::Result<bool> {
    let (repo, worktree) = discover_worktree_repository(workspace_root)?;
    let relative = worktree.relative_path(path)?;
    let Ok(head_tree) = repo.head().and_then(|head| head.peel_to_tree()) else {
        return Ok(false);
    };

    Ok(head_tree.get_path(&relative).is_ok())
}
pub fn list_commit_history(
    workspace_root: &Path,
    limit: usize,
) -> anyhow::Result<Vec<GitCommitSummary>> {
    list_commit_history_with_short_hash_length(
        workspace_root,
        limit,
        DEFAULT_GIT_COMMIT_SHORT_HASH_LENGTH,
    )
}

pub fn list_commit_history_with_short_hash_length(
    workspace_root: &Path,
    limit: usize,
    short_hash_length: usize,
) -> anyhow::Result<Vec<GitCommitSummary>> {
    list_commit_history_with_timeline_date(
        workspace_root,
        limit,
        short_hash_length,
        GitTimelineDate::Committed,
    )
}

pub fn list_commit_history_with_timeline_date(
    workspace_root: &Path,
    limit: usize,
    short_hash_length: usize,
    timeline_date: GitTimelineDate,
) -> anyhow::Result<Vec<GitCommitSummary>> {
    let limit = clamp_git_commit_history_limit(limit);
    if limit == 0 {
        return Ok(Vec::new());
    }

    let short_hash_length = clamp_git_commit_short_hash_length(short_hash_length);
    let repo = Repository::discover(workspace_root)?;
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(Sort::TIME | Sort::TOPOLOGICAL)?;

    let mut commits = Vec::with_capacity(limit);
    for oid in revwalk.take(limit) {
        let commit = repo.find_commit(oid?)?;
        let author = commit.author();
        let oid = commit.id().to_string();
        let time_seconds = match timeline_date {
            GitTimelineDate::Committed => commit.time().seconds(),
            GitTimelineDate::Authored => author.when().seconds(),
        };
        commits.push(GitCommitSummary {
            short_oid: short_oid(&oid, short_hash_length),
            oid,
            summary: commit
                .summary()
                .ok()
                .flatten()
                .unwrap_or("(no summary)")
                .to_owned(),
            author: author.name().unwrap_or("Unknown").to_owned(),
            time_seconds,
        });
    }

    Ok(commits)
}

pub fn unified_diff_for_commit(workspace_root: &Path, commit_ref: &str) -> anyhow::Result<String> {
    let commit_ref = commit_ref.trim();
    if commit_ref.is_empty() {
        return Err(anyhow!("commit reference cannot be empty"));
    }

    let repo = Repository::discover(workspace_root)?;
    let commit = repo.revparse_single(commit_ref)?.peel_to_commit()?;
    let new_tree = commit.tree()?;
    let old_tree = if commit.parent_count() > 0 {
        Some(commit.parent(0)?.tree()?)
    } else {
        None
    };
    let diff = repo.diff_tree_to_tree(old_tree.as_ref(), Some(&new_tree), None)?;
    diff_to_patch_text(&diff)
}

pub fn unified_diff_for_stash(workspace_root: &Path, index: usize) -> anyhow::Result<String> {
    let mut repo = Repository::discover(workspace_root)?;
    let oid = stash_oid_by_index(&mut repo, index)?;
    let commit = repo.find_commit(oid)?;
    let new_tree = commit.tree()?;
    let old_tree = if commit.parent_count() > 0 {
        Some(commit.parent(0)?.tree()?)
    } else {
        None
    };
    let diff = repo.diff_tree_to_tree(old_tree.as_ref(), Some(&new_tree), None)?;
    let mut patch = diff_to_patch_text(&diff)?;

    if commit.parent_count() > 2 {
        let untracked_tree = commit.parent(2)?.tree()?;
        let untracked_diff = repo.diff_tree_to_tree(None, Some(&untracked_tree), None)?;
        patch.push_str(&diff_to_patch_text(&untracked_diff)?);
    }

    Ok(patch)
}

pub fn blame_file(workspace_root: &Path, path: &Path) -> anyhow::Result<Vec<GitBlameLine>> {
    blame_file_with_options(workspace_root, path, false)
}

pub fn blame_file_with_options(
    workspace_root: &Path,
    path: &Path,
    ignore_whitespace: bool,
) -> anyhow::Result<Vec<GitBlameLine>> {
    blame_file_with_options_and_short_hash_length(
        workspace_root,
        path,
        ignore_whitespace,
        DEFAULT_GIT_COMMIT_SHORT_HASH_LENGTH,
    )
}

pub fn blame_file_with_options_and_short_hash_length(
    workspace_root: &Path,
    path: &Path,
    ignore_whitespace: bool,
    short_hash_length: usize,
) -> anyhow::Result<Vec<GitBlameLine>> {
    let text = read_git_text_file_with_limit(
        path,
        u64::try_from(diff_max_file_size_bytes(DEFAULT_DIFF_MAX_FILE_SIZE_MB)).unwrap_or(u64::MAX),
    )?;
    blame_file_for_text_with_options_and_short_hash_length(
        workspace_root,
        path,
        &text,
        ignore_whitespace,
        short_hash_length,
    )
}

pub fn blame_file_for_text_with_options_and_short_hash_length(
    workspace_root: &Path,
    path: &Path,
    text: &str,
    ignore_whitespace: bool,
    short_hash_length: usize,
) -> anyhow::Result<Vec<GitBlameLine>> {
    let short_hash_length = clamp_git_commit_short_hash_length(short_hash_length);
    let (repo, worktree) = discover_worktree_repository(workspace_root)?;
    let relative = worktree.relative_path(path)?;
    let line_count = text.lines().count();
    let mut options = BlameOptions::new();
    options.ignore_whitespace(ignore_whitespace);
    let blame = repo.blame_file(&relative, Some(&mut options))?;
    let mut lines = Vec::with_capacity(line_count);

    for line_number in 1..=line_count {
        let Some(hunk) = blame.get_line(line_number) else {
            continue;
        };
        let signature = hunk.final_signature();
        let author = signature
            .as_ref()
            .and_then(|signature| signature.name().ok().map(ToOwned::to_owned))
            .unwrap_or_else(|| "Unknown".to_owned());
        let author_time_seconds = signature
            .as_ref()
            .map(|signature| signature.when().seconds())
            .unwrap_or_default();
        let summary = hunk.summary()?.unwrap_or("(no summary)").to_owned();
        lines.push(GitBlameLine {
            line_number,
            short_oid: short_oid(&hunk.final_commit_id().to_string(), short_hash_length),
            author,
            author_time_seconds,
            summary,
        });
    }

    Ok(lines)
}

fn read_git_text_file_with_limit(path: &Path, max_bytes: u64) -> anyhow::Result<String> {
    if max_bytes == 0 {
        return fs::read_to_string(path)
            .with_context(|| format!("could not read {}", path.display()));
    }

    let file =
        fs::File::open(path).with_context(|| format!("could not read {}", path.display()))?;
    let metadata = file
        .metadata()
        .with_context(|| format!("could not read {}", path.display()))?;
    if metadata.is_file() && metadata.len() > max_bytes {
        anyhow::bail!("{} is larger than {max_bytes} bytes", path.display());
    }

    let mut reader = file.take(max_bytes.saturating_add(1));
    let byte_capacity = if metadata.is_file() {
        usize::try_from(metadata.len()).unwrap_or_default()
    } else {
        0
    };
    let mut bytes = Vec::with_capacity(byte_capacity);
    reader
        .read_to_end(&mut bytes)
        .with_context(|| format!("could not read {}", path.display()))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        anyhow::bail!("{} is larger than {max_bytes} bytes", path.display());
    }

    String::from_utf8(bytes).with_context(|| format!("{} is not valid UTF-8", path.display()))
}

pub fn list_stashes(workspace_root: &Path) -> anyhow::Result<Vec<GitStashEntry>> {
    list_stashes_with_short_hash_length(workspace_root, DEFAULT_GIT_COMMIT_SHORT_HASH_LENGTH)
}

pub fn list_stashes_with_short_hash_length(
    workspace_root: &Path,
    short_hash_length: usize,
) -> anyhow::Result<Vec<GitStashEntry>> {
    let mut repo = Repository::discover(workspace_root)?;
    let short_hash_length = clamp_git_commit_short_hash_length(short_hash_length);
    let mut stashes = Vec::new();
    repo.stash_foreach(|index, message, oid| {
        stashes.push(GitStashEntry {
            index,
            short_oid: short_oid(&oid.to_string(), short_hash_length),
            message: message.to_owned(),
        });
        true
    })?;
    Ok(stashes)
}

fn stash_oid_by_index(repo: &mut Repository, index: usize) -> anyhow::Result<Oid> {
    let mut found = None;
    repo.stash_foreach(|stash_index, _message, oid| {
        if stash_index == index {
            found = Some(*oid);
            return false;
        }
        true
    })?;
    found.ok_or_else(|| anyhow!("could not find git stash {index}"))
}

pub fn save_stash(workspace_root: &Path, message: &str) -> anyhow::Result<String> {
    save_stash_with_short_hash_length(
        workspace_root,
        message,
        DEFAULT_GIT_COMMIT_SHORT_HASH_LENGTH,
    )
}

pub fn save_stash_with_short_hash_length(
    workspace_root: &Path,
    message: &str,
    short_hash_length: usize,
) -> anyhow::Result<String> {
    save_stash_with_user_config_option(workspace_root, message, short_hash_length, true)
}

pub fn save_stash_with_user_config_option(
    workspace_root: &Path,
    message: &str,
    short_hash_length: usize,
    require_user_config: bool,
) -> anyhow::Result<String> {
    let message = message.trim();
    let short_hash_length = clamp_git_commit_short_hash_length(short_hash_length);
    let repo = Repository::discover(workspace_root)?;
    if repo.workdir().is_none() {
        return Err(anyhow!("bare repositories do not expose a worktree"));
    }
    let signature = git_commit_signature(&repo, require_user_config)?;
    let mut repo = repo;
    let oid = if message.is_empty() {
        repo.stash_save2(&signature, None, Some(StashFlags::INCLUDE_UNTRACKED))?
    } else {
        repo.stash_save(&signature, message, Some(StashFlags::INCLUDE_UNTRACKED))?
    };
    Ok(short_oid(&oid.to_string(), short_hash_length))
}

pub fn apply_stash(workspace_root: &Path, index: usize) -> anyhow::Result<()> {
    let mut repo = Repository::discover(workspace_root)?;
    if repo.workdir().is_none() {
        return Err(anyhow!("bare repositories do not expose a worktree"));
    }
    repo.stash_apply(index, None)?;
    Ok(())
}

pub fn pop_stash(workspace_root: &Path, index: usize) -> anyhow::Result<()> {
    let mut repo = Repository::discover(workspace_root)?;
    if repo.workdir().is_none() {
        return Err(anyhow!("bare repositories do not expose a worktree"));
    }
    repo.stash_pop(index, None)?;
    Ok(())
}

pub fn drop_stash(workspace_root: &Path, index: usize) -> anyhow::Result<()> {
    let mut repo = Repository::discover(workspace_root)?;
    repo.stash_drop(index)?;
    Ok(())
}

pub fn changed_lines_against_head(
    workspace_root: &Path,
    path: &Path,
    current_text: &str,
    max_lines: usize,
) -> anyhow::Result<BTreeSet<usize>> {
    changed_lines_against_head_with_options(
        workspace_root,
        path,
        current_text,
        max_lines,
        DiffOptions::default(),
    )
}

pub fn changed_lines_against_head_with_options(
    workspace_root: &Path,
    path: &Path,
    current_text: &str,
    max_lines: usize,
    options: DiffOptions,
) -> anyhow::Result<BTreeSet<usize>> {
    Ok(changed_line_kinds_against_head_with_options(
        workspace_root,
        path,
        current_text,
        max_lines,
        options,
    )?
    .keys()
    .copied()
    .collect())
}

pub fn changed_line_kinds_against_head(
    workspace_root: &Path,
    path: &Path,
    current_text: &str,
    max_lines: usize,
) -> anyhow::Result<BTreeMap<usize, GitLineChangeKind>> {
    changed_line_kinds_against_head_with_options(
        workspace_root,
        path,
        current_text,
        max_lines,
        DiffOptions::default(),
    )
}

pub fn changed_line_kinds_against_head_with_options(
    workspace_root: &Path,
    path: &Path,
    current_text: &str,
    max_lines: usize,
    options: DiffOptions,
) -> anyhow::Result<BTreeMap<usize, GitLineChangeKind>> {
    let (repo, worktree) = discover_worktree_repository(workspace_root)?;
    let relative = worktree.relative_path(path)?;

    let head_tree = repo.head()?.peel_to_tree()?;
    let entry = head_tree.get_path(&relative)?;
    let blob = repo.find_blob(entry.id())?;
    let head_text = std::str::from_utf8(blob.content()).unwrap_or_default();

    Ok(line_change_kinds_with_options(
        head_text,
        current_text,
        max_lines,
        options,
    ))
}

pub fn unified_diff_against_head(
    workspace_root: &Path,
    path: &Path,
    current_text: &str,
) -> anyhow::Result<String> {
    unified_diff_against_head_with_options(
        workspace_root,
        path,
        current_text,
        DiffOptions::default(),
    )
}

pub fn unified_diff_against_head_with_options(
    workspace_root: &Path,
    path: &Path,
    current_text: &str,
    options: DiffOptions,
) -> anyhow::Result<String> {
    Ok(head_diff_with_text(workspace_root, path, current_text, options)?.diff)
}

pub fn head_diff_with_text(
    workspace_root: &Path,
    path: &Path,
    current_text: &str,
    options: DiffOptions,
) -> anyhow::Result<GitHeadDiff> {
    let (repo, worktree) = discover_worktree_repository(workspace_root)?;
    let relative = worktree.relative_path(path)?;
    let head_text = head_text_for_path(&repo, &relative)?;

    let diff =
        unified_diff_for_texts(&relative, head_text.as_deref(), Some(current_text), options)?;
    Ok(GitHeadDiff { diff, head_text })
}

pub fn file_text_at_head(workspace_root: &Path, path: &Path) -> anyhow::Result<Option<String>> {
    let (repo, worktree) = discover_worktree_repository(workspace_root)?;
    let relative = worktree.relative_path(path)?;

    head_text_for_path(&repo, &relative)
}

pub fn file_text_at_index(workspace_root: &Path, path: &Path) -> anyhow::Result<Option<String>> {
    let (repo, worktree) = discover_worktree_repository(workspace_root)?;
    let relative = worktree.relative_path(path)?;

    index_text_for_path(&repo, &relative)
}

pub fn unified_diff_against_index(workspace_root: &Path, path: &Path) -> anyhow::Result<String> {
    unified_diff_against_index_with_options(workspace_root, path, DiffOptions::default())
}

pub fn unified_diff_against_index_with_options(
    workspace_root: &Path,
    path: &Path,
    options: DiffOptions,
) -> anyhow::Result<String> {
    Ok(staged_diff_with_texts(workspace_root, path, options)?.diff)
}

pub fn staged_diff_with_texts(
    workspace_root: &Path,
    path: &Path,
    options: DiffOptions,
) -> anyhow::Result<GitStagedDiff> {
    let (repo, worktree) = discover_worktree_repository(workspace_root)?;
    let relative = worktree.relative_path(path)?;
    let head_text = head_text_for_path(&repo, &relative)?;
    let index_text = index_text_for_path(&repo, &relative)?;

    let diff = unified_diff_for_texts(
        &relative,
        head_text.as_deref(),
        index_text.as_deref(),
        options,
    )?;
    Ok(GitStagedDiff {
        diff,
        head_text,
        index_text,
    })
}

pub fn unified_diff_against_worktree(
    workspace_root: &Path,
    path: &Path,
    current_text: &str,
) -> anyhow::Result<String> {
    unified_diff_against_worktree_with_options(
        workspace_root,
        path,
        current_text,
        DiffOptions::default(),
    )
}

pub fn unified_diff_against_worktree_with_options(
    workspace_root: &Path,
    path: &Path,
    current_text: &str,
    options: DiffOptions,
) -> anyhow::Result<String> {
    Ok(worktree_diff_with_index_text(workspace_root, path, current_text, options)?.diff)
}

pub fn worktree_diff_with_index_text(
    workspace_root: &Path,
    path: &Path,
    current_text: &str,
    options: DiffOptions,
) -> anyhow::Result<GitWorktreeDiff> {
    let (repo, worktree) = discover_worktree_repository(workspace_root)?;
    let relative = worktree.relative_path(path)?;
    let index_text = index_text_for_path(&repo, &relative)?;

    let diff = unified_diff_for_texts(
        &relative,
        index_text.as_deref(),
        Some(current_text),
        options,
    )?;
    Ok(GitWorktreeDiff { diff, index_text })
}

pub fn worktree_diff_hunks(
    workspace_root: &Path,
    path: &Path,
    current_text: &str,
) -> anyhow::Result<Vec<GitDiffHunk>> {
    let (repo, worktree) = discover_worktree_repository(workspace_root)?;
    let relative = worktree.relative_path(path)?;
    ensure_index_path_not_conflicted(&repo, &relative, "read git hunks for")?;
    let base_text = index_text_for_path(&repo, &relative)?;

    Ok(
        diff_hunks(base_text.as_deref().unwrap_or_default(), current_text)
            .into_iter()
            .enumerate()
            .map(|(index, hunk)| hunk.summary(index))
            .collect(),
    )
}

pub fn staged_diff_hunks(workspace_root: &Path, path: &Path) -> anyhow::Result<Vec<GitDiffHunk>> {
    let (repo, worktree) = discover_worktree_repository(workspace_root)?;
    let relative = worktree.relative_path(path)?;
    ensure_index_path_not_conflicted(&repo, &relative, "read git hunks for")?;
    let old_text = head_text_for_path(&repo, &relative)?;
    let staged_text = index_text_for_path(&repo, &relative)?;

    Ok(diff_hunks(
        old_text.as_deref().unwrap_or_default(),
        staged_text.as_deref().unwrap_or_default(),
    )
    .into_iter()
    .enumerate()
    .map(|(index, hunk)| hunk.summary(index))
    .collect())
}

pub fn stage_worktree_hunk(
    workspace_root: &Path,
    path: &Path,
    current_text: &str,
    hunk_index: usize,
    expected_fingerprint: u64,
) -> anyhow::Result<()> {
    let (repo, worktree) = discover_worktree_repository(workspace_root)?;
    let relative = worktree.relative_path(path)?;
    ensure_index_path_not_conflicted(&repo, &relative, "stage git hunk for")?;
    let base_text = index_text_for_path(&repo, &relative)?;
    let base_text = base_text.unwrap_or_default();
    let staged_text = apply_hunk_to_old_text(
        &base_text,
        current_text,
        hunk_index,
        Some(expected_fingerprint),
    )?;

    let absolute = worktree.absolute_path(&relative);
    if staged_text.is_empty() && !absolute.exists() {
        let mut index = repo.index()?;
        index.remove_path(&relative)?;
        index.write()?;
        return Ok(());
    }

    write_index_text(&repo, &relative, &staged_text)
}

pub fn unstage_staged_hunk(
    workspace_root: &Path,
    path: &Path,
    hunk_index: usize,
    expected_fingerprint: u64,
) -> anyhow::Result<()> {
    let (repo, worktree) = discover_worktree_repository(workspace_root)?;
    let relative = worktree.relative_path(path)?;
    ensure_index_path_not_conflicted(&repo, &relative, "unstage git hunk for")?;
    let head_text = head_text_for_path(&repo, &relative)?;
    let staged_text = index_text_for_path(&repo, &relative)?.unwrap_or_default();
    let updated_index_text = apply_hunk_to_new_text(
        head_text.as_deref().unwrap_or_default(),
        &staged_text,
        hunk_index,
        Some(expected_fingerprint),
    )?;

    if head_text.is_none() && updated_index_text.is_empty() {
        let mut index = repo.index()?;
        index.remove_path(&relative)?;
        index.write()?;
        Ok(())
    } else {
        write_index_text(&repo, &relative, &updated_index_text)
    }
}

pub fn discard_worktree_hunk(
    workspace_root: &Path,
    path: &Path,
    current_text: &str,
    hunk_index: usize,
    expected_fingerprint: u64,
) -> anyhow::Result<String> {
    let (repo, worktree) = discover_worktree_repository(workspace_root)?;
    let relative = worktree.relative_path(path)?;
    ensure_index_path_not_conflicted(&repo, &relative, "discard git hunk for")?;
    let base_text = index_text_for_path(&repo, &relative)?;
    let base_missing = base_text.is_none();
    let base_text = base_text.unwrap_or_default();
    let worktree_text = apply_hunk_to_new_text(
        &base_text,
        current_text,
        hunk_index,
        Some(expected_fingerprint),
    )?;
    let absolute = worktree.absolute_path(&relative);

    if base_missing && worktree_text.is_empty() {
        remove_worktree_path(&absolute)?;
        return Ok(worktree_text);
    }
    if worktree_text.is_empty() && !base_text.is_empty() && !absolute.exists() {
        return Ok(worktree_text);
    }
    fs::write(&absolute, &worktree_text)
        .with_context(|| format!("could not write {}", absolute.display()))?;
    Ok(worktree_text)
}

pub fn stage_path(workspace_root: &Path, path: &Path) -> anyhow::Result<()> {
    stage_paths(workspace_root, [path])
}

pub fn stage_paths<'a>(
    workspace_root: &Path,
    paths: impl IntoIterator<Item = &'a Path>,
) -> anyhow::Result<()> {
    let (repo, worktree) = discover_worktree_repository(workspace_root)?;
    let mut index = repo.index()?;

    for path in paths {
        let relative = worktree.relative_path(path)?;
        if worktree.absolute_path(&relative).exists() {
            index.add_path(&relative)?;
        } else {
            index.remove_path(&relative)?;
        }
        clear_index_conflict_for_path(&mut index, &relative)?;
    }

    index.write()?;
    Ok(())
}

fn clear_index_conflict_for_path(index: &mut Index, relative: &Path) -> anyhow::Result<()> {
    let target = git_path_display(relative);
    if index_has_conflict_for_key(index, &target)? {
        index
            .conflict_remove(Path::new(&target))
            .with_context(|| format!("could not mark {target} as resolved"))?;
    }
    Ok(())
}

fn ensure_index_path_not_conflicted(
    repo: &Repository,
    relative: &Path,
    operation: &str,
) -> anyhow::Result<()> {
    let mut index = repo.index()?;
    if index_has_conflict_for_path(&mut index, relative)? {
        let label =
            git_path_label_from_path(relative).unwrap_or_else(|| "selected file".to_owned());
        anyhow::bail!("cannot {operation} {label} while it has unresolved conflicts");
    }
    Ok(())
}

fn index_has_conflict_for_path(index: &mut Index, relative: &Path) -> anyhow::Result<bool> {
    if !index.has_conflicts() {
        return Ok(false);
    }

    let target = git_path_display(relative);
    index_has_conflict_for_key(index, &target)
}

fn index_has_conflict_for_key(index: &mut Index, target: &str) -> anyhow::Result<bool> {
    if !index.has_conflicts() {
        return Ok(false);
    }

    for conflict in index.conflicts()? {
        let conflict = conflict?;
        let matches_target = [
            conflict.ancestor.as_ref(),
            conflict.our.as_ref(),
            conflict.their.as_ref(),
        ]
        .into_iter()
        .flatten()
        .filter_map(|entry| std::str::from_utf8(&entry.path).ok())
        .any(|path| path == target);
        if matches_target {
            return Ok(true);
        }
    }

    Ok(false)
}

pub fn unstage_path(workspace_root: &Path, path: &Path) -> anyhow::Result<()> {
    unstage_paths(workspace_root, [path])
}

pub fn unstage_paths<'a>(
    workspace_root: &Path,
    paths: impl IntoIterator<Item = &'a Path>,
) -> anyhow::Result<()> {
    let (repo, worktree) = discover_worktree_repository(workspace_root)?;
    let pathspecs = paths
        .into_iter()
        .map(|path| {
            worktree
                .relative_path(path)
                .map(|relative| git_path_display(&relative))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let head = repo
        .head()
        .ok()
        .and_then(|head| head.peel(ObjectType::Any).ok());

    repo.reset_default(head.as_ref(), pathspecs)?;
    Ok(())
}

pub fn discard_path(workspace_root: &Path, path: &Path) -> anyhow::Result<()> {
    discard_paths(workspace_root, [path])
}

pub fn discard_paths<'a>(
    workspace_root: &Path,
    paths: impl IntoIterator<Item = &'a Path>,
) -> anyhow::Result<()> {
    let (repo, worktree) = discover_worktree_repository(workspace_root)?;
    let requested = paths
        .into_iter()
        .map(|path| worktree.relative_path(path))
        .collect::<anyhow::Result<BTreeSet<_>>>()?;
    let requested = requested
        .into_iter()
        .map(GitRequestedPath::new)
        .collect::<Vec<_>>();

    if requested.is_empty() {
        return Ok(());
    }

    let mut options = StatusOptions::new();
    options
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .renames_head_to_index(true)
        .renames_index_to_workdir(true);
    let statuses = repo.statuses(Some(&mut options))?;
    let mut plan = DiscardPlan::default();

    {
        let requested_keys = requested
            .iter()
            .map(|path| path.key.as_str())
            .collect::<BTreeSet<_>>();
        for entry in statuses.iter() {
            if !status_entry_matches_requested_keys(&entry, &requested_keys) {
                continue;
            }
            if entry.status().is_conflicted() {
                let label = first_status_entry_path_label(&entry)
                    .unwrap_or_else(|| "selected file".to_owned());
                return Err(anyhow!("cannot discard conflicted changes in {label}"));
            }
            plan.add_status_entry(
                entry.status(),
                entry.head_to_index(),
                entry.index_to_workdir(),
            );
        }
    }

    for GitRequestedPath { relative, key } in requested {
        plan.reset.insert(relative.clone());
        if !plan.matched.contains(&key) {
            plan.checkout.insert(relative);
        }
    }

    let head = repo
        .head()
        .ok()
        .and_then(|head| head.peel(ObjectType::Any).ok());

    if !plan.reset.is_empty() {
        let pathspecs = plan
            .reset
            .iter()
            .map(|path| git_path_display(path))
            .collect::<Vec<_>>();
        repo.reset_default(head.as_ref(), pathspecs)?;
    }

    if head.is_some() && !plan.checkout.is_empty() {
        let mut checkout = CheckoutBuilder::new();
        checkout.force().disable_pathspec_match(true);
        for path in &plan.checkout {
            checkout.path(git_path_display(path));
        }
        repo.checkout_head(Some(&mut checkout))?;
    }

    for relative in plan.remove {
        remove_worktree_path(&worktree.absolute_path(&relative))?;
    }

    Ok(())
}

fn head_text_for_path(repo: &Repository, relative: &Path) -> anyhow::Result<Option<String>> {
    let head_tree = match repo.head().and_then(|head| head.peel_to_tree()) {
        Ok(tree) => tree,
        Err(_) => return Ok(None),
    };
    let Ok(entry) = head_tree.get_path(relative) else {
        return Ok(None);
    };
    let blob = repo.find_blob(entry.id())?;
    Ok(Some(String::from_utf8_lossy(blob.content()).into_owned()))
}

fn index_text_for_path(repo: &Repository, relative: &Path) -> anyhow::Result<Option<String>> {
    let index = repo.index()?;
    let Some(entry) = index.get_path(relative, 0) else {
        return Ok(None);
    };
    let blob = repo.find_blob(entry.id)?;
    Ok(Some(String::from_utf8_lossy(blob.content()).into_owned()))
}

fn write_index_text(repo: &Repository, relative: &Path, text: &str) -> anyhow::Result<()> {
    let mut index = repo.index()?;
    let entry = index
        .get_path(relative, 0)
        .unwrap_or_else(|| default_index_entry(relative));
    index.add_frombuffer(&entry, text.as_bytes())?;
    index.write()?;
    Ok(())
}

fn default_index_entry(relative: &Path) -> IndexEntry {
    IndexEntry {
        ctime: IndexTime::new(0, 0),
        mtime: IndexTime::new(0, 0),
        dev: 0,
        ino: 0,
        mode: 0o100644,
        uid: 0,
        gid: 0,
        file_size: 0,
        id: Oid::ZERO_SHA1,
        flags: 0,
        flags_extended: 0,
        path: git_path_display(relative).into_bytes(),
    }
}

fn remove_worktree_path(path: &Path) -> anyhow::Result<()> {
    let Ok(metadata) = fs::metadata(path) else {
        return Ok(());
    };
    if metadata.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
    .with_context(|| format!("could not remove {}", path.display()))
}

fn clamp_git_commit_history_limit(value: usize) -> usize {
    value.min(MAX_GIT_COMMIT_HISTORY_LIMIT)
}

pub fn clamp_git_status_limit(value: usize) -> usize {
    value.clamp(MIN_GIT_STATUS_LIMIT, MAX_GIT_STATUS_LIMIT)
}

pub fn clamp_git_detect_submodules_limit(value: usize) -> usize {
    value.clamp(
        MIN_GIT_DETECT_SUBMODULES_LIMIT,
        MAX_GIT_DETECT_SUBMODULES_LIMIT,
    )
}

pub fn clamp_git_similarity_threshold(value: usize) -> usize {
    value.clamp(MIN_GIT_SIMILARITY_THRESHOLD, MAX_GIT_SIMILARITY_THRESHOLD)
}

fn short_oid(oid: &str, length: usize) -> String {
    oid.chars()
        .take(clamp_git_commit_short_hash_length(length))
        .collect()
}

#[cfg(test)]
mod tests;
