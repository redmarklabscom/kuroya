use super::paths::{GitWorktreePathContext, extend_status_entry_paths};
use super::status::status_entries;
use super::{
    DEFAULT_GIT_COMMIT_SHORT_HASH_LENGTH, GitChangeStage, GitSmartCommitChanges,
    MAX_GIT_COMMIT_SHORT_HASH_LENGTH, MIN_GIT_COMMIT_SHORT_HASH_LENGTH, short_oid,
};
use anyhow::{Context, anyhow};
use git2::{Oid, Repository, RepositoryState, Signature, Status, StatusOptions};
use std::{collections::BTreeSet, env, fs, path::Path};

pub fn commit_staged_changes(workspace_root: &Path, message: &str) -> anyhow::Result<String> {
    commit_changes(workspace_root, message, None)
}

pub fn commit_changes(
    workspace_root: &Path,
    message: &str,
    smart_commit_changes: Option<GitSmartCommitChanges>,
) -> anyhow::Result<String> {
    commit_changes_with_short_hash_length(
        workspace_root,
        message,
        smart_commit_changes,
        DEFAULT_GIT_COMMIT_SHORT_HASH_LENGTH,
    )
}

pub fn commit_changes_with_short_hash_length(
    workspace_root: &Path,
    message: &str,
    smart_commit_changes: Option<GitSmartCommitChanges>,
    short_hash_length: usize,
) -> anyhow::Result<String> {
    commit_changes_with_options(
        workspace_root,
        message,
        smart_commit_changes,
        short_hash_length,
        false,
    )
}

pub fn commit_changes_with_options(
    workspace_root: &Path,
    message: &str,
    smart_commit_changes: Option<GitSmartCommitChanges>,
    short_hash_length: usize,
    sign_off: bool,
) -> anyhow::Result<String> {
    commit_changes_with_empty_commit_option(
        workspace_root,
        message,
        smart_commit_changes,
        short_hash_length,
        sign_off,
        false,
    )
}

pub fn commit_changes_with_empty_commit_option(
    workspace_root: &Path,
    message: &str,
    smart_commit_changes: Option<GitSmartCommitChanges>,
    short_hash_length: usize,
    sign_off: bool,
    allow_empty: bool,
) -> anyhow::Result<String> {
    commit_changes_with_user_config_option(
        workspace_root,
        message,
        smart_commit_changes,
        short_hash_length,
        sign_off,
        allow_empty,
        true,
    )
}

pub fn commit_changes_with_user_config_option(
    workspace_root: &Path,
    message: &str,
    smart_commit_changes: Option<GitSmartCommitChanges>,
    short_hash_length: usize,
    sign_off: bool,
    allow_empty: bool,
    require_user_config: bool,
) -> anyhow::Result<String> {
    let message = message.trim();
    if message.is_empty() {
        return Err(anyhow!("commit message cannot be empty"));
    }

    let short_hash_length = clamp_git_commit_short_hash_length(short_hash_length);
    let repo = Repository::discover(workspace_root)?;
    if repo.workdir().is_none() {
        return Err(anyhow!("bare repositories do not expose a worktree"));
    }

    if smart_commit_changes.is_some()
        && repo.state() != RepositoryState::Merge
        && !repo_has_staged_changes(&repo)?
    {
        let staged = stage_smart_commit_changes(&repo, smart_commit_changes.unwrap_or_default())?;
        if !staged {
            return Err(anyhow!("no changes to smart commit"));
        }
    }

    commit_current_index(
        &repo,
        message,
        short_hash_length,
        sign_off,
        allow_empty,
        require_user_config,
    )
}

fn commit_current_index(
    repo: &Repository,
    message: &str,
    short_hash_length: usize,
    sign_off: bool,
    allow_empty: bool,
    require_user_config: bool,
) -> anyhow::Result<String> {
    let mut index = repo.index()?;
    if index.has_conflicts() {
        return Err(anyhow!("cannot commit while the index has conflicts"));
    }

    let state = repo.state();
    let parents = commit_parents_for_repository_state(repo, state)?;
    let tree_id = index.write_tree()?;
    if !is_merge_commit_state(state)
        && !allow_empty
        && parents
            .first()
            .is_some_and(|parent| parent.tree_id() == tree_id)
    {
        return Err(anyhow!("no staged changes to commit"));
    }

    let tree = repo.find_tree(tree_id)?;
    let signature = git_commit_signature(repo, require_user_config)?;
    let message = if sign_off {
        commit_message_with_sign_off(message, &signature)
    } else {
        message.to_owned()
    };
    let parent_refs = parents.iter().collect::<Vec<_>>();
    let oid = repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        &message,
        &tree,
        &parent_refs,
    )?;
    if is_merge_commit_state(state) {
        repo.cleanup_state()
            .context("created merge commit but could not clean up merge state")?;
    }
    Ok(short_oid(&oid.to_string(), short_hash_length))
}

fn commit_parents_for_repository_state<'repo>(
    repo: &'repo Repository,
    state: RepositoryState,
) -> anyhow::Result<Vec<git2::Commit<'repo>>> {
    if !is_merge_commit_state(state) {
        return Ok(repo
            .head()
            .ok()
            .and_then(|head| head.peel_to_commit().ok())
            .into_iter()
            .collect());
    }

    let head = repo
        .head()
        .context("could not read HEAD")?
        .peel_to_commit()
        .context("could not resolve HEAD commit")?;
    let mut parents = vec![head];
    parents.extend(merge_head_commits(repo)?);
    Ok(parents)
}

pub(super) fn merge_head_commits<'repo>(
    repo: &'repo Repository,
) -> anyhow::Result<Vec<git2::Commit<'repo>>> {
    let merge_head_path = repo.path().join("MERGE_HEAD");
    let merge_head = fs::read_to_string(&merge_head_path)
        .with_context(|| format!("could not read {}", merge_head_path.display()))?;
    let mut commits = Vec::with_capacity(merge_head.lines().count());
    for (line_index, line) in merge_head.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let oid = Oid::from_str(line).with_context(|| {
            format!(
                "could not parse {} line {}",
                merge_head_path.display(),
                line_index + 1
            )
        })?;
        commits.push(
            repo.find_commit(oid)
                .with_context(|| format!("could not find merge parent {oid}"))?,
        );
    }
    if commits.is_empty() {
        return Err(anyhow!("MERGE_HEAD does not list any merge parents"));
    }
    Ok(commits)
}

fn is_merge_commit_state(state: RepositoryState) -> bool {
    state == RepositoryState::Merge
}

pub(super) fn git_commit_signature(
    repo: &Repository,
    require_user_config: bool,
) -> anyhow::Result<Signature<'static>> {
    match repo.signature() {
        Ok(signature) => Ok(signature),
        Err(error) if !require_user_config => guessed_git_signature()
            .with_context(|| format!("could not read git author identity: {error}")),
        Err(error) => Err(error).context("could not read git author identity"),
    }
}

fn guessed_git_signature() -> anyhow::Result<Signature<'static>> {
    let name = guessed_git_signature_name();
    let email = guessed_git_signature_email(&name);
    Signature::now(&name, &email).context("could not create guessed git author identity")
}

fn guessed_git_signature_name() -> String {
    guessed_git_signature_name_from(|key| env::var(key).ok())
}

fn guessed_git_signature_email(name: &str) -> String {
    guessed_git_signature_email_from(name, |key| env::var(key).ok())
}

pub(super) fn guessed_git_signature_name_from(
    get_value: impl Fn(&str) -> Option<String>,
) -> String {
    first_clean_value(
        &["GIT_AUTHOR_NAME", "GIT_COMMITTER_NAME", "USERNAME", "USER"],
        &get_value,
    )
    .unwrap_or_else(|| "Kuroya".to_owned())
}

pub(super) fn guessed_git_signature_email_from(
    name: &str,
    get_value: impl Fn(&str) -> Option<String>,
) -> String {
    first_clean_value(
        &["GIT_AUTHOR_EMAIL", "GIT_COMMITTER_EMAIL", "EMAIL"],
        &get_value,
    )
    .unwrap_or_else(|| {
        let user = clean_email_component(name).unwrap_or_else(|| "kuroya".to_owned());
        let host = first_clean_value(&["COMPUTERNAME", "HOSTNAME"], &get_value)
            .and_then(|host| clean_email_component(&host))
            .unwrap_or_else(|| "localhost".to_owned());
        format!("{user}@{host}")
    })
}

fn first_clean_value(keys: &[&str], get_value: &impl Fn(&str) -> Option<String>) -> Option<String> {
    keys.iter()
        .filter_map(|key| get_value(key))
        .find_map(|value| clean_signature_component(&value))
}

fn clean_signature_component(value: &str) -> Option<String> {
    let cleaned = value
        .trim()
        .chars()
        .filter(|ch| !matches!(ch, '<' | '>' | '\n' | '\r'))
        .collect::<String>();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

fn clean_email_component(value: &str) -> Option<String> {
    let cleaned = clean_signature_component(value)?
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches(|ch| matches!(ch, '.' | '_' | '-'))
        .to_owned();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

fn commit_message_with_sign_off(message: &str, signature: &Signature<'_>) -> String {
    let name = signature.name().unwrap_or_default().trim();
    let email = signature.email().unwrap_or_default().trim();
    if name.is_empty() || email.is_empty() {
        return message.to_owned();
    }

    let trailer = format!("Signed-off-by: {name} <{email}>");
    if message
        .lines()
        .any(|line| line.trim().eq_ignore_ascii_case(&trailer))
    {
        return message.to_owned();
    }

    format!("{}\n\n{}", message.trim_end(), trailer)
}

fn repo_has_staged_changes(repo: &Repository) -> anyhow::Result<bool> {
    let mut options = StatusOptions::new();
    options
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .renames_head_to_index(true)
        .renames_index_to_workdir(true);
    let statuses = repo.statuses(Some(&mut options))?;
    Ok(statuses.iter().any(|entry| {
        status_entries(entry.status())
            .into_iter()
            .any(|(_, stage)| stage == GitChangeStage::Staged)
    }))
}

fn stage_smart_commit_changes(
    repo: &Repository,
    smart_commit_changes: GitSmartCommitChanges,
) -> anyhow::Result<bool> {
    let worktree = GitWorktreePathContext::for_repo(repo)?;
    let mut options = StatusOptions::new();
    options
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .renames_head_to_index(true)
        .renames_index_to_workdir(true);
    let statuses = repo.statuses(Some(&mut options))?;
    let mut paths = BTreeSet::new();

    for entry in statuses.iter() {
        let status = entry.status();
        if status.is_conflicted() || !smart_commit_status_included(status, smart_commit_changes) {
            continue;
        }
        extend_status_entry_paths(&entry, &mut paths);
    }

    if paths.is_empty() {
        return Ok(false);
    }

    let mut index = repo.index()?;
    for relative in paths {
        if worktree.absolute_path(&relative).exists() {
            index.add_path(&relative)?;
        } else {
            index.remove_path(&relative)?;
        }
    }
    index.write()?;
    Ok(true)
}

fn smart_commit_status_included(
    status: Status,
    smart_commit_changes: GitSmartCommitChanges,
) -> bool {
    if !status.intersects(
        Status::WT_NEW
            | Status::WT_MODIFIED
            | Status::WT_DELETED
            | Status::WT_RENAMED
            | Status::WT_TYPECHANGE,
    ) {
        return false;
    }
    smart_commit_changes == GitSmartCommitChanges::All || !status.contains(Status::WT_NEW)
}

pub fn clamp_git_commit_short_hash_length(value: usize) -> usize {
    value.clamp(
        MIN_GIT_COMMIT_SHORT_HASH_LENGTH,
        MAX_GIT_COMMIT_SHORT_HASH_LENGTH,
    )
}
