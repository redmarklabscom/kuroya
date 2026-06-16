use crate::workspace_paths::{
    lexical_normalize, normalize_child_path, normalize_child_path_with_normalized_root,
};
use anyhow::{Context, anyhow};
use git2::{DiffDelta, Repository, Status};
use std::{
    borrow::Borrow,
    collections::{BTreeSet, VecDeque},
    path::{Path, PathBuf},
};

pub(super) const MAX_GIT_PATH_LABEL_CHARS: usize = 240;
pub(super) const GIT_PATH_LABEL_HEAD_CHARS: usize = 160;
pub(super) const GIT_PATH_LABEL_TAIL_CHARS: usize = 64;
pub(super) const GIT_PATH_LABEL_OMISSION: &str = "...";

pub(super) fn git_path_display(path: &Path) -> String {
    let mut display = String::new();
    push_git_path_display(&mut display, path);
    display
}

#[cfg(test)]
pub(super) fn git_path_display_with_prefix(prefix: &str, path: &Path) -> String {
    let mut display = String::from(prefix);
    push_git_path_display(&mut display, path);
    display
}

fn push_git_path_display(display: &mut String, path: &Path) {
    let mut has_component = false;
    for component in path.components() {
        if has_component {
            display.push('/');
        }
        display.push_str(&component.as_os_str().to_string_lossy());
        has_component = true;
    }
}

pub(super) struct GitDiffLabels {
    pub(super) old_display_label: String,
    pub(super) new_display_label: String,
    pub(super) old_git_label: String,
    pub(super) new_git_label: String,
}

impl GitDiffLabels {
    pub(super) fn for_relative_path(relative: &Path) -> Self {
        let display_label = git_display_label_from_path(relative);
        Self::for_display_labels(display_label.clone(), display_label)
    }

    pub(super) fn for_displays(old_display: &str, new_display: &str) -> Self {
        Self::for_display_labels(
            git_display_label(old_display),
            git_display_label(new_display),
        )
    }

    fn for_display_labels(old_display_label: String, new_display_label: String) -> Self {
        Self {
            old_git_label: git_label_with_prefix("a/", &old_display_label),
            new_git_label: git_label_with_prefix("b/", &new_display_label),
            old_display_label,
            new_display_label,
        }
    }

    pub(super) fn old_file_label(&self, old_text: Option<&str>) -> &str {
        if old_text.is_some() {
            &self.old_git_label
        } else {
            "/dev/null"
        }
    }

    pub(super) fn new_file_label(&self, new_text: Option<&str>) -> &str {
        if new_text.is_some() {
            &self.new_git_label
        } else {
            "/dev/null"
        }
    }

    pub(super) fn push_diff_header(&self, diff: &mut String) {
        diff.push_str("diff --git ");
        diff.push_str(&self.old_git_label);
        diff.push(' ');
        diff.push_str(&self.new_git_label);
        diff.push('\n');
    }
}

fn git_label_with_prefix(prefix: &str, display: &str) -> String {
    let mut label = GitPathLabelBuilder::new(
        prefix
            .len()
            .saturating_add(display.len())
            .min(MAX_GIT_PATH_LABEL_CHARS),
    );
    for ch in prefix.chars().chain(display.chars()) {
        label.push(ch);
    }
    label
        .finish()
        .unwrap_or_else(|| prefix.trim_end_matches('/').to_owned())
}

pub(super) fn push_diff_file_header(diff: &mut String, marker: &str, label: &str) {
    diff.push_str(marker);
    diff.push(' ');
    diff.push_str(label);
    diff.push('\n');
}

#[derive(Debug, Clone)]
pub(super) struct GitWorktreePathContext {
    workdir: PathBuf,
    normalized_workdir: PathBuf,
    normalized_workdir_component_count: usize,
}

impl GitWorktreePathContext {
    fn new(workdir: &Path) -> Self {
        let normalized_workdir = lexical_normalize(workdir);
        let normalized_workdir_component_count = normalized_workdir.components().count();
        Self {
            workdir: workdir.to_path_buf(),
            normalized_workdir,
            normalized_workdir_component_count,
        }
    }

    pub(super) fn for_repo(repo: &Repository) -> anyhow::Result<Self> {
        let workdir = repo
            .workdir()
            .ok_or_else(|| anyhow!("bare repositories do not expose a worktree"))?;
        Ok(Self::new(workdir))
    }

    pub(super) fn relative_path(&self, path: &Path) -> anyhow::Result<PathBuf> {
        if let Ok(relative) = path.strip_prefix(&self.workdir) {
            let relative = normalize_worktree_relative_path(relative).with_context(|| {
                format!("{} is outside {}", path.display(), self.workdir.display())
            })?;
            return ensure_named_worktree_relative_path(relative, path, &self.workdir);
        }

        let normalized_child =
            normalize_child_path_with_normalized_root(&self.normalized_workdir, path)
                .with_context(|| {
                    format!("{} is outside {}", path.display(), self.workdir.display())
                })?;
        let relative = path_relative_to_normalized_root(
            &self.normalized_workdir,
            self.normalized_workdir_component_count,
            &normalized_child,
        );
        ensure_named_worktree_relative_path(relative, path, &self.workdir)
    }

    pub(super) fn absolute_path(&self, relative: &Path) -> PathBuf {
        self.workdir.join(relative)
    }
}

fn ensure_named_worktree_relative_path(
    relative: PathBuf,
    path: &Path,
    workdir: &Path,
) -> anyhow::Result<PathBuf> {
    if relative.as_os_str().is_empty() {
        return Err(anyhow!(
            "{} resolves to the worktree root {}",
            path.display(),
            workdir.display()
        ));
    }
    Ok(relative)
}

pub(super) fn discover_worktree_repository(
    workspace_root: &Path,
) -> anyhow::Result<(Repository, GitWorktreePathContext)> {
    let repo = Repository::discover(workspace_root)?;
    let worktree = GitWorktreePathContext::for_repo(&repo)?;
    Ok((repo, worktree))
}

#[cfg(test)]
pub(super) fn worktree_relative_path(workdir: &Path, path: &Path) -> anyhow::Result<PathBuf> {
    GitWorktreePathContext::new(workdir).relative_path(path)
}

fn normalize_worktree_relative_path(path: &Path) -> Option<PathBuf> {
    let normalized = normalize_child_path(Path::new("."), path)?;
    if normalized == Path::new(".") {
        Some(PathBuf::new())
    } else {
        Some(normalized)
    }
}

pub(super) fn normalize_git_relative_path(path: &Path) -> Option<PathBuf> {
    let normalized = normalize_worktree_relative_path(path)?;
    (!normalized.as_os_str().is_empty()).then_some(normalized)
}

pub(super) fn git_status_relative_path(raw: &str) -> Option<PathBuf> {
    if raw.is_empty() || raw.contains('\0') {
        return None;
    }
    normalize_git_relative_path(Path::new(raw))
}

fn path_relative_to_normalized_root(
    root: &Path,
    root_component_count: usize,
    path: &Path,
) -> PathBuf {
    if root == Path::new(".") {
        return if path == Path::new(".") {
            PathBuf::new()
        } else {
            path.to_path_buf()
        };
    }
    let relative: PathBuf = path.components().skip(root_component_count).collect();
    if relative.as_os_str().is_empty() {
        PathBuf::new()
    } else {
        relative
    }
}

#[derive(Debug, Default)]
pub(super) struct DiscardPlan {
    pub(super) matched: BTreeSet<String>,
    pub(super) reset: BTreeSet<PathBuf>,
    pub(super) checkout: BTreeSet<PathBuf>,
    pub(super) remove: BTreeSet<PathBuf>,
}

#[derive(Debug)]
pub(super) struct GitRequestedPath {
    pub(super) relative: PathBuf,
    pub(super) key: String,
}

impl GitRequestedPath {
    pub(super) fn new(relative: PathBuf) -> Self {
        let key = git_path_display(&relative);
        Self { relative, key }
    }
}

impl DiscardPlan {
    pub(super) fn add_status_entry(
        &mut self,
        status: Status,
        head_to_index: Option<DiffDelta<'_>>,
        index_to_workdir: Option<DiffDelta<'_>>,
    ) {
        if let Some(delta) = head_to_index.as_ref() {
            self.add_delta_paths(delta);
            if status.contains(Status::INDEX_NEW) {
                self.remove_delta_new_path(delta);
            } else if status.contains(Status::INDEX_RENAMED) {
                self.remove_delta_new_path(delta);
                self.checkout_delta_old_path(delta);
            } else if status.intersects(
                Status::INDEX_MODIFIED | Status::INDEX_DELETED | Status::INDEX_TYPECHANGE,
            ) {
                self.checkout_delta_old_path(delta);
            }
        }

        if let Some(delta) = index_to_workdir.as_ref() {
            self.add_delta_paths(delta);
            if status.contains(Status::WT_NEW) {
                self.remove_delta_path(delta);
            } else if status.contains(Status::WT_RENAMED) {
                self.remove_delta_new_path(delta);
                self.checkout_delta_old_path(delta);
            } else if status
                .intersects(Status::WT_MODIFIED | Status::WT_DELETED | Status::WT_TYPECHANGE)
            {
                self.checkout_delta_old_path(delta);
            }
        }
    }

    fn add_delta_paths(&mut self, delta: &DiffDelta<'_>) {
        self.add_reset_path(delta.old_file().path());
        self.add_reset_path(delta.new_file().path());
    }

    fn checkout_delta_old_path(&mut self, delta: &DiffDelta<'_>) {
        self.add_checkout_path(delta.old_file().path());
    }

    fn remove_delta_new_path(&mut self, delta: &DiffDelta<'_>) {
        self.add_remove_path(delta.new_file().path());
    }

    fn remove_delta_path(&mut self, delta: &DiffDelta<'_>) {
        self.add_remove_path(delta.old_file().path().or_else(|| delta.new_file().path()));
    }

    fn add_reset_path(&mut self, path: Option<&Path>) {
        if let Some(path) = path.and_then(normalize_git_relative_path) {
            self.matched.insert(git_path_display(&path));
            self.reset.insert(path);
        }
    }

    fn add_checkout_path(&mut self, path: Option<&Path>) {
        if let Some(path) = path.and_then(normalize_git_relative_path) {
            self.checkout.insert(path);
        }
    }

    fn add_remove_path(&mut self, path: Option<&Path>) {
        if let Some(path) = path.and_then(normalize_git_relative_path) {
            self.remove.insert(path);
        }
    }
}

pub(super) fn status_entry_matches_requested_keys<T>(
    entry: &git2::StatusEntry<'_>,
    requested_keys: &BTreeSet<T>,
) -> bool
where
    T: Borrow<str> + Ord,
{
    if let Ok(path) = entry.path()
        && status_path_matches_requested_keys(path, requested_keys)
    {
        return true;
    }

    delta_matches_requested_keys(entry.head_to_index(), requested_keys)
        || delta_matches_requested_keys(entry.index_to_workdir(), requested_keys)
}

fn delta_matches_requested_keys<T>(
    delta: Option<DiffDelta<'_>>,
    requested_keys: &BTreeSet<T>,
) -> bool
where
    T: Borrow<str> + Ord,
{
    let Some(delta) = delta else {
        return false;
    };

    path_matches_requested_keys(delta.old_file().path(), requested_keys)
        || path_matches_requested_keys(delta.new_file().path(), requested_keys)
}

pub(super) fn status_path_matches_requested_keys<T>(raw: &str, requested_keys: &BTreeSet<T>) -> bool
where
    T: Borrow<str> + Ord,
{
    let Some(relative) = git_status_relative_path(raw) else {
        return false;
    };
    if requested_keys.contains(raw) {
        return true;
    }

    let key = git_path_display(&relative);
    requested_keys.contains(key.as_str())
}

pub(super) fn path_matches_requested_keys<T>(
    path: Option<&Path>,
    requested_keys: &BTreeSet<T>,
) -> bool
where
    T: Borrow<str> + Ord,
{
    let Some(path) = path else {
        return false;
    };
    let Some(relative) = normalize_git_relative_path(path) else {
        return false;
    };
    if let Some(path) = path.to_str()
        && requested_keys.contains(path)
    {
        return true;
    }

    let key = git_path_display(&relative);
    requested_keys.contains(key.as_str())
}

pub(super) fn first_status_entry_path_label(entry: &git2::StatusEntry<'_>) -> Option<String> {
    if let Ok(path) = entry.path() {
        return git_path_label(path);
    }

    first_delta_path_label(entry.head_to_index())
        .or_else(|| first_delta_path_label(entry.index_to_workdir()))
}

fn first_delta_path_label(delta: Option<DiffDelta<'_>>) -> Option<String> {
    let delta = delta?;
    delta
        .old_file()
        .path()
        .or_else(|| delta.new_file().path())
        .and_then(git_path_label_from_path)
}

fn git_display_label_from_path(path: &Path) -> String {
    git_path_label_from_path(path).unwrap_or_else(|| "selected file".to_owned())
}

fn git_display_label(raw: &str) -> String {
    git_path_label(raw).unwrap_or_else(|| "selected file".to_owned())
}

pub(super) fn git_path_label_from_path(path: &Path) -> Option<String> {
    let mut label = GitPathLabelBuilder::new(0);
    let mut has_component = false;
    for component in path.components() {
        if has_component {
            label.push('/');
        }
        label.push_lossy(component.as_os_str());
        has_component = true;
    }
    label.finish()
}

pub(super) fn git_path_label(raw: &str) -> Option<String> {
    let mut label = GitPathLabelBuilder::new(raw.len().min(MAX_GIT_PATH_LABEL_CHARS));
    for ch in raw.chars() {
        label.push(ch);
    }
    label.finish()
}

struct GitPathLabelBuilder {
    label: String,
    tail: VecDeque<char>,
    chars: usize,
}

impl GitPathLabelBuilder {
    fn new(capacity: usize) -> Self {
        debug_assert!(
            GIT_PATH_LABEL_HEAD_CHARS + GIT_PATH_LABEL_OMISSION.len() + GIT_PATH_LABEL_TAIL_CHARS
                <= MAX_GIT_PATH_LABEL_CHARS
        );

        Self {
            label: String::with_capacity(capacity),
            tail: VecDeque::with_capacity(GIT_PATH_LABEL_TAIL_CHARS),
            chars: 0,
        }
    }

    fn push_lossy(&mut self, raw: &std::ffi::OsStr) {
        for ch in raw.to_string_lossy().chars() {
            self.push(ch);
        }
    }

    fn push(&mut self, ch: char) {
        if git_path_label_char_is_hidden(ch) {
            return;
        }

        self.chars += 1;
        if self.chars <= MAX_GIT_PATH_LABEL_CHARS {
            self.label.push(ch);
        }

        if self.tail.len() == GIT_PATH_LABEL_TAIL_CHARS {
            self.tail.pop_front();
        }
        self.tail.push_back(ch);
    }

    fn finish(self) -> Option<String> {
        if self.chars == 0 {
            return None;
        }

        if self.chars <= MAX_GIT_PATH_LABEL_CHARS {
            return Some(self.label);
        }

        let mut bounded = self
            .label
            .chars()
            .take(GIT_PATH_LABEL_HEAD_CHARS)
            .collect::<String>();
        bounded.push_str(GIT_PATH_LABEL_OMISSION);
        bounded.extend(self.tail);
        Some(bounded)
    }
}

fn git_path_label_char_is_hidden(ch: char) -> bool {
    ch.is_control()
        || matches!(
            ch,
            '\u{061C}'
                | '\u{200B}'..='\u{200F}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{206F}'
                | '\u{FEFF}'
        )
}

pub(super) fn extend_status_entry_paths(
    entry: &git2::StatusEntry<'_>,
    paths: &mut BTreeSet<PathBuf>,
) {
    if let Ok(path) = entry.path()
        && let Some(path) = git_status_relative_path(path)
    {
        paths.insert(path);
    }
    extend_delta_paths(entry.head_to_index(), paths);
    extend_delta_paths(entry.index_to_workdir(), paths);
}

fn extend_delta_paths(delta: Option<DiffDelta<'_>>, paths: &mut BTreeSet<PathBuf>) {
    if let Some(delta) = delta {
        if let Some(path) = delta
            .old_file()
            .path()
            .and_then(normalize_git_relative_path)
        {
            paths.insert(path);
        }
        if let Some(path) = delta
            .new_file()
            .path()
            .and_then(normalize_git_relative_path)
        {
            paths.insert(path);
        }
    }
}
