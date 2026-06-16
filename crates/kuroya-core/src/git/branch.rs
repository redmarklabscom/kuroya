use super::{
    DEFAULT_GIT_COMMIT_SHORT_HASH_LENGTH, GitBranch, GitCheckoutType, GitRemoteDivergence,
    short_oid,
};
use anyhow::{Context, anyhow};
use git2::{BranchType, Oid, Repository, RepositoryState, build::CheckoutBuilder};
use std::path::Path;

pub(super) fn branch_name(repo: &Repository) -> Option<String> {
    let head = repo.head().ok()?;
    if head.is_branch() {
        return head.shorthand().ok().map(ToOwned::to_owned);
    }

    head.target()
        .map(|oid| short_oid(&oid.to_string(), DEFAULT_GIT_COMMIT_SHORT_HASH_LENGTH))
}

pub(super) fn upstream_divergence(
    repo: &Repository,
) -> anyhow::Result<Option<GitRemoteDivergence>> {
    let head = repo.head()?;
    if !head.is_branch() {
        return Ok(None);
    }
    let Ok(branch_name) = head.shorthand() else {
        return Ok(None);
    };
    let branch = repo.find_branch(branch_name, BranchType::Local)?;
    let Ok(upstream) = branch.upstream() else {
        return Ok(None);
    };
    let Some(local_oid) = branch.get().target() else {
        return Ok(None);
    };
    let Some(upstream_oid) = upstream.get().target() else {
        return Ok(None);
    };
    let (outgoing, incoming) = repo.graph_ahead_behind(local_oid, upstream_oid)?;
    Ok(Some(GitRemoteDivergence { incoming, outgoing }))
}

pub fn list_local_branches(workspace_root: &Path) -> anyhow::Result<Vec<GitBranch>> {
    list_checkout_refs(workspace_root, &[GitCheckoutType::Local])
}

pub fn list_checkout_refs(
    workspace_root: &Path,
    checkout_types: &[GitCheckoutType],
) -> anyhow::Result<Vec<GitBranch>> {
    let repo = Repository::discover(workspace_root)?;
    let current = branch_name(&repo);
    let head_is_branch = repo.head().ok().is_some_and(|head| head.is_branch());
    let head_target = repo.head().ok().and_then(|head| head.target());
    let include_local = checkout_types.contains(&GitCheckoutType::Local);
    let include_remote = checkout_types.contains(&GitCheckoutType::Remote);
    let include_tags = checkout_types.contains(&GitCheckoutType::Tags);
    let mut branches = Vec::new();

    if include_local {
        for branch in repo.branches(Some(BranchType::Local))? {
            let (branch, _) = branch?;
            let Some(name) = branch.name()? else {
                continue;
            };
            branches.push(GitBranch {
                name: name.to_owned(),
                is_current: current.as_deref() == Some(name),
                kind: GitCheckoutType::Local,
                committer_time_seconds: branch_committer_time_seconds(&branch),
            });
        }
    }

    if include_remote {
        for branch in repo.branches(Some(BranchType::Remote))? {
            let (branch, _) = branch?;
            let Some(name) = branch.name()? else {
                continue;
            };
            if name.ends_with("/HEAD") {
                continue;
            }
            branches.push(GitBranch {
                name: name.to_owned(),
                is_current: false,
                kind: GitCheckoutType::Remote,
                committer_time_seconds: branch_committer_time_seconds(&branch),
            });
        }
    }

    if include_tags {
        let tag_names = repo.tag_names(None)?;
        branches.reserve(tag_names.iter().count());
        for name in tag_names.iter() {
            let Ok(Some(name)) = name else {
                continue;
            };
            let Ok(commit) = repo
                .revparse_single(&format!("refs/tags/{name}"))
                .and_then(|object| object.peel_to_commit())
            else {
                continue;
            };
            branches.push(GitBranch {
                name: name.to_owned(),
                is_current: !head_is_branch && head_target == Some(commit.id()),
                kind: GitCheckoutType::Tags,
                committer_time_seconds: commit.time().seconds(),
            });
        }
    }

    branches.sort_by(|left, right| {
        right
            .is_current
            .cmp(&left.is_current)
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(branches)
}

fn branch_committer_time_seconds(branch: &git2::Branch<'_>) -> i64 {
    branch
        .get()
        .peel_to_commit()
        .map(|commit| commit.time().seconds())
        .unwrap_or_default()
}

pub fn checkout_branch(workspace_root: &Path, branch_name: &str) -> anyhow::Result<()> {
    checkout_ref(workspace_root, branch_name, GitCheckoutType::Local)
}

pub fn checkout_ref(
    workspace_root: &Path,
    ref_name: &str,
    kind: GitCheckoutType,
) -> anyhow::Result<()> {
    match kind {
        GitCheckoutType::Local => checkout_local_branch(workspace_root, ref_name),
        GitCheckoutType::Remote => checkout_remote_branch(workspace_root, ref_name),
        GitCheckoutType::Tags => checkout_tag(workspace_root, ref_name),
    }
}

fn checkout_local_branch(workspace_root: &Path, branch_name: &str) -> anyhow::Result<()> {
    let branch_name = branch_name.trim();
    if branch_name.is_empty() {
        return Err(anyhow!("branch name cannot be empty"));
    }

    let repo = Repository::discover(workspace_root)?;
    if repo.workdir().is_none() {
        return Err(anyhow!("bare repositories do not expose a worktree"));
    }
    ensure_repository_allows_branch_mutation(&repo)?;

    let branch = repo
        .find_branch(branch_name, BranchType::Local)
        .with_context(|| format!("could not find local branch {branch_name}"))?;
    let branch_ref = branch.get();
    let ref_name = branch_ref.name()?.to_owned();
    let target = branch_ref.peel_to_commit()?;
    let mut checkout = CheckoutBuilder::new();
    checkout.safe();
    repo.checkout_tree(target.as_object(), Some(&mut checkout))?;
    repo.set_head(&ref_name)?;
    Ok(())
}

fn checkout_remote_branch(workspace_root: &Path, branch_name: &str) -> anyhow::Result<()> {
    let branch_name = branch_name.trim();
    if branch_name.is_empty() {
        return Err(anyhow!("branch name cannot be empty"));
    }

    let repo = Repository::discover(workspace_root)?;
    if repo.workdir().is_none() {
        return Err(anyhow!("bare repositories do not expose a worktree"));
    }
    ensure_repository_allows_branch_mutation(&repo)?;

    let remote_branch = repo
        .find_branch(branch_name, BranchType::Remote)
        .with_context(|| format!("could not find remote branch {branch_name}"))?;
    let commit = remote_branch.get().peel_to_commit()?;
    let local_name = remote_branch_local_name(branch_name);
    if repo.find_branch(&local_name, BranchType::Local).is_ok() {
        return checkout_local_branch(workspace_root, &local_name);
    }
    let mut branch = repo
        .branch(&local_name, &commit, false)
        .with_context(|| format!("could not create local branch {local_name}"))?;
    let _ = branch.set_upstream(Some(branch_name));
    match checkout_local_branch(workspace_root, &local_name) {
        Ok(()) => Ok(()),
        Err(error) => {
            rollback_created_local_branch(&repo, &local_name, commit.id());
            Err(error)
        }
    }
}

fn rollback_created_local_branch(repo: &Repository, local_name: &str, expected_target: Oid) {
    if branch_name(repo).as_deref() == Some(local_name) {
        return;
    }

    let Ok(mut branch) = repo.find_branch(local_name, BranchType::Local) else {
        return;
    };
    if branch.get().target() == Some(expected_target) {
        let _ = branch.delete();
    }
}

fn checkout_tag(workspace_root: &Path, tag_name: &str) -> anyhow::Result<()> {
    let tag_name = tag_name.trim();
    if tag_name.is_empty() {
        return Err(anyhow!("tag name cannot be empty"));
    }

    let repo = Repository::discover(workspace_root)?;
    if repo.workdir().is_none() {
        return Err(anyhow!("bare repositories do not expose a worktree"));
    }
    ensure_repository_allows_branch_mutation(&repo)?;

    let commit = repo
        .revparse_single(&format!("refs/tags/{tag_name}"))
        .with_context(|| format!("could not find tag {tag_name}"))?
        .peel_to_commit()?;
    let mut checkout = CheckoutBuilder::new();
    checkout.safe();
    repo.checkout_tree(commit.as_object(), Some(&mut checkout))?;
    repo.set_head_detached(commit.id())?;
    Ok(())
}

fn remote_branch_local_name(branch_name: &str) -> String {
    branch_name
        .split_once('/')
        .map(|(_, name)| name)
        .unwrap_or(branch_name)
        .to_owned()
}

pub fn create_branch(workspace_root: &Path, branch_name: &str) -> anyhow::Result<()> {
    let branch_name = branch_name.trim();
    if branch_name.is_empty() {
        return Err(anyhow!("branch name cannot be empty"));
    }

    let repo = Repository::discover(workspace_root)?;
    if repo.workdir().is_none() {
        return Err(anyhow!("bare repositories do not expose a worktree"));
    }
    ensure_repository_allows_branch_mutation(&repo)?;
    if repo.find_branch(branch_name, BranchType::Local).is_ok() {
        return Err(anyhow!("branch {branch_name} already exists"));
    }

    let head = repo.head()?.peel_to_commit()?;
    let branch = repo
        .branch(branch_name, &head, false)
        .with_context(|| format!("could not create local branch {branch_name}"))?;
    let ref_name = branch.get().name()?.to_owned();
    drop(branch);
    let target_id = head.id();
    let result = (|| -> anyhow::Result<()> {
        let mut checkout = CheckoutBuilder::new();
        checkout.safe();
        repo.checkout_tree(head.as_object(), Some(&mut checkout))?;
        repo.set_head(&ref_name)?;
        Ok(())
    })();
    if let Err(error) = result {
        rollback_created_local_branch(&repo, branch_name, target_id);
        return Err(error);
    }
    Ok(())
}

fn ensure_repository_allows_branch_mutation(repo: &Repository) -> anyhow::Result<()> {
    if let Some(operation) = repository_branch_mutation_block_label(repo.state()) {
        return Err(anyhow!(
            "cannot switch or create branches while {operation} is in progress"
        ));
    }
    Ok(())
}

pub(super) fn repository_branch_mutation_block_label(
    state: RepositoryState,
) -> Option<&'static str> {
    match state {
        RepositoryState::Clean => None,
        RepositoryState::Merge => Some("merge"),
        RepositoryState::Revert | RepositoryState::RevertSequence => Some("revert"),
        RepositoryState::CherryPick | RepositoryState::CherryPickSequence => Some("cherry-pick"),
        RepositoryState::Bisect => Some("bisect"),
        RepositoryState::Rebase
        | RepositoryState::RebaseInteractive
        | RepositoryState::RebaseMerge => Some("rebase"),
        RepositoryState::ApplyMailbox => Some("apply"),
        RepositoryState::ApplyMailboxOrRebase => Some("apply or rebase"),
    }
}

pub fn delete_branch(workspace_root: &Path, name: &str) -> anyhow::Result<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err(anyhow!("branch name cannot be empty"));
    }

    let repo = Repository::discover(workspace_root)?;
    if repo.workdir().is_none() {
        return Err(anyhow!("bare repositories do not expose a worktree"));
    }
    if branch_name(&repo).as_deref() == Some(name) {
        return Err(anyhow!("cannot delete the current branch {name}"));
    }

    let mut branch = repo
        .find_branch(name, BranchType::Local)
        .with_context(|| format!("could not find local branch {name}"))?;
    branch
        .delete()
        .with_context(|| format!("could not delete local branch {name}"))?;
    Ok(())
}

pub fn rename_branch(workspace_root: &Path, old_name: &str, new_name: &str) -> anyhow::Result<()> {
    let old_name = old_name.trim();
    let new_name = new_name.trim();
    if old_name.is_empty() || new_name.is_empty() {
        return Err(anyhow!("branch name cannot be empty"));
    }
    if old_name == new_name {
        return Err(anyhow!("branch already has name {new_name}"));
    }

    let repo = Repository::discover(workspace_root)?;
    if repo.workdir().is_none() {
        return Err(anyhow!("bare repositories do not expose a worktree"));
    }
    if repo.find_branch(new_name, BranchType::Local).is_ok() {
        return Err(anyhow!("branch {new_name} already exists"));
    }

    let mut branch = repo
        .find_branch(old_name, BranchType::Local)
        .with_context(|| format!("could not find local branch {old_name}"))?;
    branch
        .rename(new_name, false)
        .with_context(|| format!("could not rename local branch {old_name} to {new_name}"))?;
    Ok(())
}
