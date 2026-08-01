use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(windows)]
use std::{
    ffi::OsString,
    os::windows::ffi::{OsStrExt, OsStringExt},
};

use crate::process::{BoundedOutput, ProcessError, run_bounded, run_bounded_cancelable};

pub const MAX_GIT_FILES: usize = 2_000;
pub const MAX_GIT_BRANCHES: usize = 500;
pub const MAX_GIT_WORKTREES: usize = 100;
pub const MAX_GIT_REVIEW_BRANCHES: usize = 30;
pub const MAX_GIT_REVIEW_COMMITS: usize = 30;
pub const MAX_GIT_DIFF_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_GIT_BRANCH_BYTES: usize = 1_024;
pub const MAX_GIT_CONFLICT_PATHS: usize = 100;
pub const MAX_GIT_COMMIT_MESSAGE_CHARS: usize = 4_000;
pub const MAX_GIT_COMMIT_CONTEXT_BYTES: usize = 20 * 1024;
pub const MAX_GIT_PULL_REQUEST_CONTEXT_BYTES: usize = 30 * 1024;
pub const MAX_MANAGED_WORKTREE_DIFF_BYTES: usize = 20 * 1024 * 1024;

const MAX_GIT_METADATA_BYTES: usize = 2 * 1024 * 1024;
const MAX_GIT_STDERR_BYTES: usize = 16 * 1024;
const MAX_GIT_PATH_BYTES: usize = 8 * 1024;
const MAX_GIT_REVIEW_COMMIT_SUBJECT_BYTES: usize = 512;
const MAX_GIT_REVIEW_COMMIT_MESSAGE_BYTES: usize = 8 * 1024;
const MAX_GIT_REVIEW_LOG_BYTES: usize = 320 * 1024;
const MAX_GIT_COMMIT_CONTEXT_FILES: usize = 128;
const GIT_TIMEOUT: Duration = Duration::from_secs(15);
const GIT_PUSH_TIMEOUT: Duration = Duration::from_secs(60);
static MANAGED_WORKTREE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitFileKind {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    Untracked,
    Conflicted,
    TypeChanged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitFile {
    pub path: PathBuf,
    pub old_path: Option<PathBuf>,
    pub kind: GitFileKind,
    pub staged: bool,
    pub unstaged: bool,
    pub staged_additions: u32,
    pub staged_deletions: u32,
    pub unstaged_additions: u32,
    pub unstaged_deletions: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitBranch {
    pub name: String,
    pub commit: String,
    pub current: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitWorktree {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub commit: Option<String>,
    pub bare: bool,
    pub detached: bool,
    pub locked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitReviewCommit {
    pub sha: String,
    pub subject: String,
    pub message: String,
    pub committed_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitSnapshot {
    pub repository_root: PathBuf,
    pub branch: Option<String>,
    pub default_branch: Option<String>,
    pub review_default_base: Option<String>,
    pub upstream_ref: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub files: Vec<GitFile>,
    pub branches: Vec<GitBranch>,
    pub review_branches: Vec<String>,
    pub worktrees: Vec<GitWorktree>,
    pub commits: Vec<GitReviewCommit>,
    pub truncated: bool,
}

type ParsedGitStatus = (Option<String>, Option<String>, u32, u32, Vec<GitFile>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitDiff {
    pub text: String,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitBranchDiff {
    pub base_sha: String,
    pub text: String,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedWorktree {
    pub git_root: PathBuf,
    pub workspace_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitBranchMutationOutcome {
    Switched,
    Blocked {
        paths: Vec<PathBuf>,
        truncated: bool,
    },
}

#[derive(Debug)]
pub enum GitError {
    Process(ProcessError),
    Io(std::io::Error),
    InvalidRepository,
    InvalidOutput,
    InvalidReference,
    PathOutsideRepository,
    InvalidBranchName,
    InvalidWorktreePath,
    ManagedWorktreePathUnavailable,
    Cancelled,
    WorkingTreeDiffTooLarge,
    InvalidCommitMessage,
    NoCurrentBranch,
    NoRemote,
}

impl fmt::Display for GitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Process(error) => error.fmt(formatter),
            Self::Io(error) => error.fmt(formatter),
            Self::InvalidRepository => {
                formatter.write_str("working directory is not a Git repository")
            }
            Self::InvalidOutput => formatter.write_str("Git returned malformed output"),
            Self::InvalidReference => formatter.write_str("Git reference is invalid"),
            Self::PathOutsideRepository => {
                formatter.write_str("path is outside the selected Git repository")
            }
            Self::InvalidBranchName => formatter.write_str("Git branch name is invalid"),
            Self::InvalidWorktreePath => {
                formatter.write_str("worktree path must be an absolute path outside the repository")
            }
            Self::ManagedWorktreePathUnavailable => {
                formatter.write_str("could not allocate a managed worktree path")
            }
            Self::Cancelled => formatter.write_str("worktree creation was cancelled"),
            Self::WorkingTreeDiffTooLarge => write!(
                formatter,
                "working tree diff exceeds the {MAX_MANAGED_WORKTREE_DIFF_BYTES}-byte limit"
            ),
            Self::InvalidCommitMessage => formatter.write_str("Git commit message is invalid"),
            Self::NoCurrentBranch => formatter.write_str("Git has no current branch to push"),
            Self::NoRemote => formatter.write_str("no Git remote is configured for push"),
        }
    }
}

impl Error for GitError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Process(error) => Some(error),
            Self::Io(error) => Some(error),
            Self::InvalidRepository
            | Self::InvalidOutput
            | Self::InvalidReference
            | Self::PathOutsideRepository
            | Self::InvalidBranchName
            | Self::InvalidWorktreePath
            | Self::ManagedWorktreePathUnavailable
            | Self::Cancelled
            | Self::WorkingTreeDiffTooLarge
            | Self::InvalidCommitMessage
            | Self::NoCurrentBranch
            | Self::NoRemote => None,
        }
    }
}

impl From<ProcessError> for GitError {
    fn from(error: ProcessError) -> Self {
        Self::Process(error)
    }
}

impl From<std::io::Error> for GitError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

pub fn snapshot(start: &Path) -> Result<GitSnapshot, GitError> {
    let root_output = git_output(
        start,
        ["rev-parse", "--show-toplevel"],
        64 * 1024,
        GIT_TIMEOUT,
    )
    .map_err(|error| match error {
        GitError::Process(ProcessError::Exit { .. }) => GitError::InvalidRepository,
        other => other,
    })?;
    let root_text = String::from_utf8(root_output.stdout).map_err(|_| GitError::InvalidOutput)?;
    let root = PathBuf::from(root_text.trim());
    if root.as_os_str().is_empty() {
        return Err(GitError::InvalidRepository);
    }

    let status_output = git_output(
        &root,
        ["status", "--porcelain=v2", "--branch", "-z"],
        MAX_GIT_METADATA_BYTES,
        GIT_TIMEOUT,
    )?;
    let (branch, upstream_ref, ahead, behind, mut files) = parse_status(&status_output.stdout)?;
    let mut truncated = status_output.stdout_truncated || files.len() > MAX_GIT_FILES;
    files.truncate(MAX_GIT_FILES);

    let staged_stats_output = git_output(
        &root,
        [
            "diff",
            "--cached",
            "--numstat",
            "--no-renames",
            "-z",
            "--no-ext-diff",
        ],
        MAX_GIT_METADATA_BYTES,
        GIT_TIMEOUT,
    )?;
    let unstaged_stats_output = git_output(
        &root,
        ["diff", "--numstat", "--no-renames", "-z", "--no-ext-diff"],
        MAX_GIT_METADATA_BYTES,
        GIT_TIMEOUT,
    )?;
    let staged_stats = parse_numstat(&staged_stats_output.stdout);
    let unstaged_stats = parse_numstat(&unstaged_stats_output.stdout);
    truncated |= staged_stats_output.stdout_truncated
        || unstaged_stats_output.stdout_truncated
        || numstat_exceeds_limit(&staged_stats_output.stdout)
        || numstat_exceeds_limit(&unstaged_stats_output.stdout);
    apply_numstat(&mut files, &staged_stats, true);
    apply_numstat(&mut files, &unstaged_stats, false);

    let branch_output = git_output(
        &root,
        [
            "for-each-ref",
            "--count=501",
            "--sort=-committerdate",
            "--format=%(refname:short)%09%(objectname:short)%09%(HEAD)",
            "refs/heads",
        ],
        MAX_GIT_METADATA_BYTES,
        GIT_TIMEOUT,
    )?;
    let mut branches = parse_branches(&branch_output.stdout);
    truncated |= branch_output.stdout_truncated || branches.len() > MAX_GIT_BRANCHES;
    branches.truncate(MAX_GIT_BRANCHES);
    let default_branch = default_branch(&root, branch.as_deref(), &branches)?;
    let review_default_base =
        review_default_base(&root, branch.as_deref(), default_branch.as_deref())?;
    let review_branches = branches
        .iter()
        .filter(|candidate| !candidate.current)
        .take(MAX_GIT_REVIEW_BRANCHES)
        .map(|candidate| candidate.name.clone())
        .collect();

    let worktree_output = git_output(
        &root,
        ["worktree", "list", "--porcelain"],
        MAX_GIT_METADATA_BYTES,
        GIT_TIMEOUT,
    )?;
    let mut worktrees = parse_worktrees(&worktree_output.stdout);
    truncated |= worktree_output.stdout_truncated || worktrees.len() > MAX_GIT_WORKTREES;
    worktrees.truncate(MAX_GIT_WORKTREES);
    let (commits, commits_truncated) =
        review_commits(&root, branch.as_deref(), default_branch.as_deref())?;
    truncated |= commits_truncated;

    Ok(GitSnapshot {
        repository_root: root,
        branch,
        default_branch,
        review_default_base,
        upstream_ref,
        ahead,
        behind,
        files,
        branches,
        review_branches,
        worktrees,
        commits,
        truncated,
    })
}

pub fn diff(root: &Path, path: &Path, staged: bool) -> Result<GitDiff, GitError> {
    let relative = repository_relative_path(root, path)?;
    let output = if staged {
        git_output_with_path(
            root,
            [
                "diff",
                "--cached",
                "--no-ext-diff",
                "--no-color",
                "--unified=3",
            ],
            &relative,
            MAX_GIT_DIFF_BYTES,
        )?
    } else {
        git_output_with_path(
            root,
            ["diff", "--no-ext-diff", "--no-color", "--unified=3"],
            &relative,
            MAX_GIT_DIFF_BYTES,
        )?
    };
    if !staged && output.stdout.is_empty() && is_untracked(root, &relative)? {
        return untracked_diff(root, &relative);
    }
    Ok(GitDiff {
        text: String::from_utf8_lossy(&output.stdout).into_owned(),
        truncated: output.stdout_truncated,
    })
}

pub fn uncommitted_diff(root: &Path) -> Result<GitDiff, GitError> {
    let head_exists = optional_git_output(
        root,
        ["rev-parse", "--verify", "--quiet", "HEAD"],
        128,
        GIT_TIMEOUT,
    )?
    .is_some();
    let tracked = if head_exists {
        git_output(
            root,
            ["diff", "HEAD", "--no-ext-diff", "--no-color", "--unified=3"],
            MAX_GIT_DIFF_BYTES,
            GIT_TIMEOUT,
        )?
    } else {
        git_output(
            root,
            [
                "diff",
                "--cached",
                "--no-ext-diff",
                "--no-color",
                "--unified=3",
            ],
            MAX_GIT_DIFF_BYTES,
            GIT_TIMEOUT,
        )?
    };
    let mut text = String::from_utf8_lossy(&tracked.stdout).into_owned();
    let mut truncated = tracked.stdout_truncated;

    if !head_exists && !truncated {
        let unstaged = git_output(
            root,
            ["diff", "--no-ext-diff", "--no-color", "--unified=3"],
            MAX_GIT_DIFF_BYTES.saturating_sub(text.len()),
            GIT_TIMEOUT,
        )?;
        push_bounded_text(
            &mut text,
            &String::from_utf8_lossy(&unstaged.stdout),
            MAX_GIT_DIFF_BYTES,
            &mut truncated,
        );
        truncated |= unstaged.stdout_truncated;
    }

    append_untracked_diffs(root, &mut text, &mut truncated)?;

    Ok(GitDiff { text, truncated })
}

pub fn branch_diff(root: &Path, base_ref: &str) -> Result<GitBranchDiff, GitError> {
    let base_ref = base_ref.trim();
    if base_ref.is_empty()
        || base_ref.len() > MAX_GIT_BRANCH_BYTES
        || base_ref.starts_with('-')
        || base_ref.chars().any(char::is_control)
    {
        return Err(GitError::InvalidReference);
    }

    let merge_base = git_output(root, ["merge-base", "HEAD", base_ref], 128, GIT_TIMEOUT)?;
    let base_sha = String::from_utf8(merge_base.stdout)
        .map_err(|_| GitError::InvalidOutput)?
        .trim()
        .to_owned();
    if !(7..=64).contains(&base_sha.len()) || !base_sha.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(GitError::InvalidOutput);
    }

    let tracked = git_output(
        root,
        [
            "diff",
            base_sha.as_str(),
            "--no-ext-diff",
            "--no-color",
            "--unified=3",
        ],
        MAX_GIT_DIFF_BYTES,
        GIT_TIMEOUT,
    )?;
    let mut text = String::from_utf8_lossy(&tracked.stdout).into_owned();
    let mut truncated = tracked.stdout_truncated;
    append_untracked_diffs(root, &mut text, &mut truncated)?;
    Ok(GitBranchDiff {
        base_sha,
        text,
        truncated,
    })
}

pub fn commit_diff(root: &Path, sha: &str) -> Result<GitDiff, GitError> {
    if !(7..=64).contains(&sha.len()) || !sha.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(GitError::InvalidOutput);
    }
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(root)
        .arg("show")
        .arg("--format=")
        .arg("--no-ext-diff")
        .arg("--no-color")
        .arg("--unified=3")
        .arg(sha)
        .arg("--");
    let output = run_bounded(
        &mut command,
        MAX_GIT_DIFF_BYTES,
        MAX_GIT_STDERR_BYTES,
        GIT_TIMEOUT,
    )?;
    Ok(GitDiff {
        text: String::from_utf8_lossy(&output.stdout).into_owned(),
        truncated: output.stdout_truncated,
    })
}

pub fn stage(root: &Path, path: &Path) -> Result<(), GitError> {
    let relative = repository_relative_path(root, path)?;
    git_output_with_path(root, ["add"], &relative, 64 * 1024)?;
    Ok(())
}

pub fn stage_all(root: &Path) -> Result<(), GitError> {
    git_output(root, ["add", "--all"], 64 * 1024, GIT_TIMEOUT)?;
    Ok(())
}

pub fn unstage(root: &Path, path: &Path) -> Result<(), GitError> {
    let relative = repository_relative_path(root, path)?;
    git_output_with_path(root, ["reset", "-q"], &relative, 64 * 1024)?;
    Ok(())
}

pub fn unstage_all(root: &Path) -> Result<(), GitError> {
    git_output(root, ["reset", "-q"], 64 * 1024, GIT_TIMEOUT)?;
    Ok(())
}

pub fn commit(root: &Path, message: &str, include_unstaged: bool) -> Result<(), GitError> {
    let message = message.trim();
    if message.is_empty()
        || message.chars().count() > MAX_GIT_COMMIT_MESSAGE_CHARS
        || message.contains('\0')
    {
        return Err(GitError::InvalidCommitMessage);
    }
    if include_unstaged {
        stage_all(root)?;
    }
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(root)
        .arg("commit")
        .arg("-m")
        .arg(message);
    run_bounded(
        &mut command,
        MAX_GIT_METADATA_BYTES,
        MAX_GIT_STDERR_BYTES,
        Duration::from_secs(60),
    )?;
    Ok(())
}

pub fn push(root: &Path, force: bool) -> Result<(), GitError> {
    let branch_output = git_output(
        root,
        ["branch", "--show-current"],
        MAX_GIT_BRANCH_BYTES,
        GIT_TIMEOUT,
    )?;
    let branch = String::from_utf8(branch_output.stdout).map_err(|_| GitError::InvalidOutput)?;
    let branch = branch.trim();
    if branch.is_empty() {
        return Err(GitError::NoCurrentBranch);
    }
    validate_branch(root, branch)?;

    let remote = push_remote(root, branch)?.ok_or(GitError::NoRemote)?;
    let has_upstream = optional_git_output(
        root,
        [
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            "@{upstream}",
        ],
        MAX_GIT_BRANCH_BYTES,
        GIT_TIMEOUT,
    )?
    .is_some_and(|output| !output.stdout.is_empty());

    let mut command = Command::new("git");
    command.arg("-C").arg(root).arg("push").arg("--porcelain");
    if force {
        command.arg("--force-with-lease");
    }
    let refspec = (!has_upstream).then(|| format!("HEAD:refs/heads/{branch}"));
    if refspec.is_some() {
        command.arg("-u");
    }
    command.arg(remote);
    if let Some(refspec) = refspec {
        command.arg(refspec);
    }
    run_bounded(
        &mut command,
        MAX_GIT_METADATA_BYTES,
        MAX_GIT_STDERR_BYTES,
        GIT_PUSH_TIMEOUT,
    )?;
    Ok(())
}

pub fn pull_request_context(
    root: &Path,
    base_branch: &str,
    include_uncommitted: bool,
) -> Result<GitDiff, GitError> {
    validate_branch(root, base_branch)?;
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(root)
        .arg("diff")
        .arg("--no-ext-diff")
        .arg("--no-color")
        .arg("--unified=3")
        .arg(format!("{base_branch}...HEAD"));
    let committed = run_bounded(
        &mut command,
        MAX_GIT_PULL_REQUEST_CONTEXT_BYTES,
        MAX_GIT_STDERR_BYTES,
        GIT_TIMEOUT,
    )?;
    let mut text = String::from_utf8_lossy(&committed.stdout).into_owned();
    let mut truncated = committed.stdout_truncated;
    if include_uncommitted && !truncated {
        let uncommitted = commit_message_diff(root, true)?;
        if !uncommitted.text.trim().is_empty() {
            if !text.is_empty() {
                push_bounded_text(
                    &mut text,
                    "\n",
                    MAX_GIT_PULL_REQUEST_CONTEXT_BYTES,
                    &mut truncated,
                );
            }
            push_bounded_text(
                &mut text,
                &uncommitted.text,
                MAX_GIT_PULL_REQUEST_CONTEXT_BYTES,
                &mut truncated,
            );
        }
        truncated |= uncommitted.truncated;
    }
    Ok(GitDiff { text, truncated })
}

pub fn commit_message_diff(root: &Path, include_unstaged: bool) -> Result<GitDiff, GitError> {
    let mut text = String::new();
    let mut truncated = false;
    let staged = git_output(
        root,
        [
            "diff",
            "--cached",
            "--no-ext-diff",
            "--no-color",
            "--unified=3",
        ],
        MAX_GIT_COMMIT_CONTEXT_BYTES,
        GIT_TIMEOUT,
    )?;
    push_bounded_text(
        &mut text,
        &String::from_utf8_lossy(&staged.stdout),
        MAX_GIT_COMMIT_CONTEXT_BYTES,
        &mut truncated,
    );
    truncated |= staged.stdout_truncated;

    if include_unstaged && !truncated {
        let unstaged = git_output(
            root,
            ["diff", "--no-ext-diff", "--no-color", "--unified=3"],
            MAX_GIT_COMMIT_CONTEXT_BYTES.saturating_sub(text.len()),
            GIT_TIMEOUT,
        )?;
        push_bounded_text(
            &mut text,
            &String::from_utf8_lossy(&unstaged.stdout),
            MAX_GIT_COMMIT_CONTEXT_BYTES,
            &mut truncated,
        );
        truncated |= unstaged.stdout_truncated;
    }

    if include_unstaged && !truncated {
        let untracked = git_output(
            root,
            ["ls-files", "--others", "--exclude-standard", "-z"],
            MAX_GIT_METADATA_BYTES,
            GIT_TIMEOUT,
        )?;
        let mut untracked_paths = untracked
            .stdout
            .split(|byte| *byte == 0)
            .filter(|path| !path.is_empty());
        for path in untracked_paths.by_ref().take(MAX_GIT_COMMIT_CONTEXT_FILES) {
            let remaining = MAX_GIT_COMMIT_CONTEXT_BYTES.saturating_sub(text.len());
            if remaining == 0 {
                truncated = true;
                break;
            }
            let diff = untracked_diff_with_limit(root, &git_path(path), remaining)?;
            push_bounded_text(
                &mut text,
                &diff.text,
                MAX_GIT_COMMIT_CONTEXT_BYTES,
                &mut truncated,
            );
            truncated |= diff.truncated;
            if truncated {
                break;
            }
        }
        truncated |= untracked_paths.next().is_some();
        truncated |= untracked.stdout_truncated;
    }

    Ok(GitDiff { text, truncated })
}

pub fn create_worktree(
    root: &Path,
    worktree_path: &Path,
    branch: &str,
    create_branch: bool,
) -> Result<(), GitError> {
    if !valid_worktree_path(root, worktree_path) {
        return Err(GitError::InvalidWorktreePath);
    }
    validate_branch(root, branch)?;
    let mut command = Command::new("git");
    command.arg("-C").arg(root).arg("worktree").arg("add");
    if create_branch {
        command.arg("-b").arg(branch).arg("--").arg(worktree_path);
    } else {
        command.arg("--").arg(worktree_path).arg(branch);
    }
    run_bounded(
        &mut command,
        MAX_GIT_METADATA_BYTES,
        MAX_GIT_STDERR_BYTES,
        Duration::from_secs(60),
    )?;
    Ok(())
}

pub fn create_managed_worktree(
    workspace_root: &Path,
    worktrees_root: &Path,
) -> Result<ManagedWorktree, GitError> {
    create_managed_worktree_inner(workspace_root, worktrees_root, None)
}

pub fn create_managed_worktree_cancellable(
    workspace_root: &Path,
    worktrees_root: &Path,
    cancellation: &AtomicBool,
) -> Result<ManagedWorktree, GitError> {
    create_managed_worktree_inner(workspace_root, worktrees_root, Some(cancellation))
}

fn create_managed_worktree_inner(
    workspace_root: &Path,
    worktrees_root: &Path,
    cancellation: Option<&AtomicBool>,
) -> Result<ManagedWorktree, GitError> {
    check_managed_worktree_cancellation(cancellation)?;
    let workspace_root = managed_worktree_canonicalize(workspace_root)?;
    let repository_root = managed_worktree_repository_root(&workspace_root, cancellation)?;
    let relative_workspace = workspace_root
        .strip_prefix(&repository_root)
        .map_err(|_| GitError::PathOutsideRepository)?;
    if !worktrees_root.is_absolute() {
        return Err(GitError::InvalidWorktreePath);
    }

    fs::create_dir_all(worktrees_root)?;
    let canonical_worktrees_root = managed_worktree_canonicalize(worktrees_root)?;
    if canonical_worktrees_root.starts_with(&repository_root) {
        return Err(GitError::InvalidWorktreePath);
    }
    let tracked_diff = managed_worktree_diff(&repository_root, cancellation)?;
    let untracked =
        managed_worktree_untracked_files(&repository_root, tracked_diff.len(), cancellation)?;
    check_managed_worktree_cancellation(cancellation)?;

    let allocation_root = allocate_managed_worktree_root(&canonical_worktrees_root)?;
    let repository_name = repository_root
        .file_name()
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| std::ffi::OsStr::new("repository"));
    let git_root = allocation_root.join(repository_name);
    if !valid_worktree_path(&repository_root, &git_root) {
        return Err(GitError::InvalidWorktreePath);
    }

    let result = (|| {
        let patch_path = allocation_root.join(".codexrs-working-tree.patch");
        let patch_guard = TemporaryPatch::new(patch_path, &tracked_diff)?;
        check_managed_worktree_cancellation(cancellation)?;

        let mut create = Command::new("git");
        create
            .arg("-C")
            .arg(&repository_root)
            .args(["worktree", "add", "--detach"])
            .arg("--")
            .arg(&git_root)
            .arg("HEAD");
        run_managed_worktree_command(
            &mut create,
            MAX_GIT_METADATA_BYTES,
            Duration::from_secs(60),
            cancellation,
        )?;

        if !tracked_diff.is_empty() {
            let mut check = Command::new("git");
            check
                .arg("-C")
                .arg(&git_root)
                .args(["apply", "--check", "--binary", "--whitespace=nowarn", "--"])
                .arg(patch_guard.path());
            run_managed_worktree_command(
                &mut check,
                MAX_GIT_METADATA_BYTES,
                Duration::from_secs(60),
                cancellation,
            )?;

            let mut apply = Command::new("git");
            apply
                .arg("-C")
                .arg(&git_root)
                .args(["apply", "--binary", "--whitespace=nowarn", "--"])
                .arg(patch_guard.path());
            run_managed_worktree_command(
                &mut apply,
                MAX_GIT_METADATA_BYTES,
                Duration::from_secs(60),
                cancellation,
            )?;
        }
        copy_managed_worktree_untracked_files(
            &repository_root,
            &git_root,
            &untracked,
            cancellation,
        )?;
        let workspace_root = git_root.join(relative_workspace);
        fs::create_dir_all(&workspace_root)?;
        check_managed_worktree_cancellation(cancellation)?;

        Ok(ManagedWorktree {
            workspace_root,
            git_root: git_root.clone(),
        })
    })();

    if result.is_err() {
        cleanup_incomplete_managed_worktree(
            &repository_root,
            &canonical_worktrees_root,
            &allocation_root,
            &git_root,
        );
    }
    result
}

fn allocate_managed_worktree_root(worktrees_root: &Path) -> Result<PathBuf, GitError> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| GitError::ManagedWorktreePathUnavailable)?
        .as_nanos() as u64;
    for _ in 0..32 {
        let sequence = MANAGED_WORKTREE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let candidate = worktrees_root.join(format!("{:08x}", timestamp ^ sequence));
        match fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
    }
    Err(GitError::ManagedWorktreePathUnavailable)
}

fn managed_worktree_repository_root(
    workspace_root: &Path,
    cancellation: Option<&AtomicBool>,
) -> Result<PathBuf, GitError> {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(workspace_root)
        .args(["rev-parse", "--show-toplevel"]);
    let output = run_managed_worktree_command(&mut command, 64 * 1024, GIT_TIMEOUT, cancellation)
        .map_err(|error| match error {
        GitError::Process(ProcessError::Exit { .. }) => GitError::InvalidRepository,
        other => other,
    })?;
    let root_text = String::from_utf8(output.stdout).map_err(|_| GitError::InvalidOutput)?;
    let root = PathBuf::from(root_text.trim());
    if root.as_os_str().is_empty() {
        return Err(GitError::InvalidRepository);
    }
    managed_worktree_canonicalize(&root)
}

fn managed_worktree_diff(
    repository_root: &Path,
    cancellation: Option<&AtomicBool>,
) -> Result<Vec<u8>, GitError> {
    let mut command = Command::new("git");
    command.arg("-C").arg(repository_root).args([
        "diff",
        "--binary",
        "--full-index",
        "--no-ext-diff",
        "--no-textconv",
        "HEAD",
        "--",
        ".",
    ]);
    let output = run_managed_worktree_command(
        &mut command,
        MAX_MANAGED_WORKTREE_DIFF_BYTES,
        Duration::from_secs(60),
        cancellation,
    )?;
    if output.stdout_truncated {
        return Err(GitError::WorkingTreeDiffTooLarge);
    }
    Ok(output.stdout)
}

fn managed_worktree_untracked_files(
    repository_root: &Path,
    tracked_diff_bytes: usize,
    cancellation: Option<&AtomicBool>,
) -> Result<Vec<PathBuf>, GitError> {
    let mut paths = managed_worktree_git_paths(
        repository_root,
        &["ls-files", "--others", "--exclude-standard", "-z"],
        cancellation,
    )?;
    paths.extend(managed_worktree_git_paths(
        repository_root,
        &[
            "ls-files",
            "--others",
            "--ignored",
            "--exclude-standard",
            "-z",
            "--",
            ":(glob)**/AGENTS.override.md",
        ],
        cancellation,
    )?);
    if repository_root.join(".worktreeinclude").is_file() {
        let ignored = managed_worktree_git_paths(
            repository_root,
            &[
                "ls-files",
                "--others",
                "--ignored",
                "--exclude-standard",
                "-z",
            ],
            cancellation,
        )?
        .into_iter()
        .collect::<HashSet<_>>();
        let selected = managed_worktree_git_paths(
            repository_root,
            &[
                "ls-files",
                "--others",
                "--ignored",
                "--exclude-from=.worktreeinclude",
                "-z",
            ],
            cancellation,
        )?;
        paths.extend(selected.into_iter().filter(|path| ignored.contains(path)));
    }
    paths.sort();
    paths.dedup();

    let canonical_root = fs::canonicalize(repository_root)?;
    let mut total_bytes = tracked_diff_bytes;
    let mut files = Vec::new();
    for relative in paths {
        check_managed_worktree_cancellation(cancellation)?;
        if files.len() == MAX_GIT_FILES || relative.to_string_lossy().len() > MAX_GIT_PATH_BYTES {
            return Err(GitError::WorkingTreeDiffTooLarge);
        }
        if !relative.components().all(|component| {
            matches!(
                component,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        }) {
            return Err(GitError::PathOutsideRepository);
        }
        let source = fs::canonicalize(repository_root.join(&relative))?;
        if !source.starts_with(&canonical_root) || !source.is_file() {
            return Err(GitError::PathOutsideRepository);
        }
        total_bytes = total_bytes
            .checked_add(source.metadata()?.len() as usize)
            .ok_or(GitError::WorkingTreeDiffTooLarge)?;
        if total_bytes > MAX_MANAGED_WORKTREE_DIFF_BYTES {
            return Err(GitError::WorkingTreeDiffTooLarge);
        }
        files.push(relative);
    }
    Ok(files)
}

fn managed_worktree_git_paths(
    repository_root: &Path,
    args: &[&str],
    cancellation: Option<&AtomicBool>,
) -> Result<Vec<PathBuf>, GitError> {
    let mut command = Command::new("git");
    command.arg("-C").arg(repository_root).args(args);
    let output = run_managed_worktree_command(
        &mut command,
        MAX_GIT_METADATA_BYTES,
        GIT_TIMEOUT,
        cancellation,
    )?;
    if output.stdout_truncated {
        return Err(GitError::WorkingTreeDiffTooLarge);
    }
    Ok(output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(git_path)
        .collect())
}

fn copy_managed_worktree_untracked_files(
    source_root: &Path,
    destination_root: &Path,
    files: &[PathBuf],
    cancellation: Option<&AtomicBool>,
) -> Result<(), GitError> {
    let canonical_source_root = fs::canonicalize(source_root)?;
    let canonical_destination_root = fs::canonicalize(destination_root)?;
    for relative in files {
        check_managed_worktree_cancellation(cancellation)?;
        let source = fs::canonicalize(source_root.join(relative))?;
        if !source.starts_with(&canonical_source_root) || !source.is_file() {
            return Err(GitError::PathOutsideRepository);
        }
        let destination = destination_root.join(relative);
        let parent = destination
            .parent()
            .ok_or(GitError::PathOutsideRepository)?;
        fs::create_dir_all(parent)?;
        let canonical_parent = fs::canonicalize(parent)?;
        if !canonical_parent.starts_with(&canonical_destination_root) || destination.exists() {
            return Err(GitError::PathOutsideRepository);
        }
        fs::copy(source, destination)?;
    }
    Ok(())
}

fn run_managed_worktree_command(
    command: &mut Command,
    stdout_limit: usize,
    timeout: Duration,
    cancellation: Option<&AtomicBool>,
) -> Result<BoundedOutput, GitError> {
    let result = if let Some(cancellation) = cancellation {
        run_bounded_cancelable(
            command,
            stdout_limit,
            MAX_GIT_STDERR_BYTES,
            timeout,
            cancellation,
        )
    } else {
        run_bounded(command, stdout_limit, MAX_GIT_STDERR_BYTES, timeout)
    };
    match result {
        Err(ProcessError::Cancelled) => Err(GitError::Cancelled),
        result => Ok(result?),
    }
}

fn check_managed_worktree_cancellation(cancellation: Option<&AtomicBool>) -> Result<(), GitError> {
    if cancellation.is_some_and(|cancellation| cancellation.load(Ordering::Acquire)) {
        Err(GitError::Cancelled)
    } else {
        Ok(())
    }
}

fn managed_worktree_canonicalize(path: &Path) -> Result<PathBuf, GitError> {
    let canonical = fs::canonicalize(path)?;
    Ok(managed_worktree_git_path(canonical))
}

#[cfg(windows)]
fn managed_worktree_git_path(path: PathBuf) -> PathBuf {
    let encoded = path.as_os_str().encode_wide().collect::<Vec<_>>();
    const VERBATIM_PREFIX: [u16; 4] = [b'\\' as u16, b'\\' as u16, b'?' as u16, b'\\' as u16];
    const UNC_PREFIX: [u16; 4] = [b'U' as u16, b'N' as u16, b'C' as u16, b'\\' as u16];
    let Some(remainder) = encoded.strip_prefix(&VERBATIM_PREFIX) else {
        return path;
    };
    if let Some(unc) = remainder.strip_prefix(&UNC_PREFIX) {
        let normalized = [b'\\' as u16, b'\\' as u16]
            .into_iter()
            .chain(unc.iter().copied())
            .collect::<Vec<_>>();
        return PathBuf::from(OsString::from_wide(&normalized));
    }
    if remainder.len() >= 3
        && ((b'A' as u16..=b'Z' as u16).contains(&remainder[0])
            || (b'a' as u16..=b'z' as u16).contains(&remainder[0]))
        && remainder[1] == b':' as u16
        && matches!(remainder[2], value if value == b'\\' as u16 || value == b'/' as u16)
    {
        return PathBuf::from(OsString::from_wide(remainder));
    }
    path
}

#[cfg(not(windows))]
fn managed_worktree_git_path(path: PathBuf) -> PathBuf {
    path
}

fn cleanup_incomplete_managed_worktree(
    repository_root: &Path,
    worktrees_root: &Path,
    allocation_root: &Path,
    git_root: &Path,
) {
    if allocation_root.parent() != Some(worktrees_root)
        || !git_root.starts_with(allocation_root)
        || allocation_root.file_name().is_none()
    {
        return;
    }
    if git_root.exists() {
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(repository_root)
            .args(["worktree", "remove", "--force"])
            .arg("--")
            .arg(git_root);
        let _ = run_bounded(
            &mut command,
            MAX_GIT_METADATA_BYTES,
            MAX_GIT_STDERR_BYTES,
            Duration::from_secs(60),
        );
    }
    if allocation_root.exists() {
        let _ = fs::remove_dir_all(allocation_root);
    }
}

struct TemporaryPatch {
    path: PathBuf,
}

impl TemporaryPatch {
    fn new(path: PathBuf, bytes: &[u8]) -> Result<Self, GitError> {
        fs::write(&path, bytes)?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TemporaryPatch {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub fn switch_branch(root: &Path, branch: &str) -> Result<GitBranchMutationOutcome, GitError> {
    validate_branch(root, branch)?;
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(root)
        .arg("switch")
        .arg("--")
        .arg(branch);
    run_branch_mutation(&mut command)
}

pub fn create_branch(root: &Path, branch: &str) -> Result<GitBranchMutationOutcome, GitError> {
    validate_branch(root, branch)?;
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(root)
        .arg("switch")
        .arg("-c")
        .arg(branch);
    run_branch_mutation(&mut command)
}

fn run_branch_mutation(command: &mut Command) -> Result<GitBranchMutationOutcome, GitError> {
    match run_bounded(
        command,
        MAX_GIT_METADATA_BYTES,
        MAX_GIT_STDERR_BYTES,
        Duration::from_secs(60),
    ) {
        Ok(_) => Ok(GitBranchMutationOutcome::Switched),
        Err(ProcessError::Exit { status, stderr }) => {
            if let Some((paths, truncated)) = parse_checkout_conflicts(&stderr) {
                Ok(GitBranchMutationOutcome::Blocked { paths, truncated })
            } else {
                Err(GitError::Process(ProcessError::Exit { status, stderr }))
            }
        }
        Err(error) => Err(GitError::Process(error)),
    }
}

fn valid_worktree_path(root: &Path, worktree_path: &Path) -> bool {
    let Some(root) = normalized_absolute_path(root) else {
        return false;
    };
    let Some(worktree_path) = normalized_absolute_path(worktree_path) else {
        return false;
    };
    #[cfg(windows)]
    let Some(root) = canonicalize_windows_path(&root) else {
        return false;
    };
    #[cfg(windows)]
    let Some(worktree_path) = canonicalize_windows_path(&worktree_path) else {
        return false;
    };
    !worktree_path.starts_with(root)
}

#[cfg(windows)]
fn canonicalize_windows_path(path: &Path) -> Option<PathBuf> {
    let mut ancestor = path;
    let mut missing = Vec::<OsString>::new();
    loop {
        match fs::canonicalize(ancestor) {
            Ok(mut canonical) => {
                for component in missing.into_iter().rev() {
                    canonical.push(component);
                }
                return Some(canonical);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing.push(ancestor.file_name()?.to_owned());
                ancestor = ancestor.parent()?;
            }
            Err(_) => return None,
        }
    }
}

fn normalized_absolute_path(path: &Path) -> Option<PathBuf> {
    if !path.is_absolute() {
        return None;
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::Prefix(_) | std::path::Component::RootDir => {
                normalized.push(component.as_os_str());
            }
            std::path::Component::CurDir => {}
            std::path::Component::Normal(component) => normalized.push(component),
            std::path::Component::ParentDir => {
                if normalized.file_name().is_some() {
                    normalized.pop();
                } else {
                    return None;
                }
            }
        }
    }
    Some(normalized)
}

fn push_remote(root: &Path, branch: &str) -> Result<Option<String>, GitError> {
    let remote_key = format!("branch.{branch}.remote");
    let configured_remote = optional_git_output(
        root,
        ["config", "--get", remote_key.as_str()],
        MAX_GIT_BRANCH_BYTES,
        GIT_TIMEOUT,
    )?
    .and_then(|output| String::from_utf8(output.stdout).ok())
    .map(|remote| remote.trim().to_owned())
    .filter(|remote| !remote.is_empty());
    if configured_remote.is_some() {
        return Ok(configured_remote);
    }
    let remotes = git_output(root, ["remote"], MAX_GIT_METADATA_BYTES, GIT_TIMEOUT)?;
    Ok(String::from_utf8(remotes.stdout)
        .map_err(|_| GitError::InvalidOutput)?
        .lines()
        .map(str::trim)
        .find(|remote| !remote.is_empty())
        .map(str::to_owned))
}

fn default_branch(
    root: &Path,
    branch: Option<&str>,
    branches: &[GitBranch],
) -> Result<Option<String>, GitError> {
    if let Some(remote) = branch
        .map(|branch| push_remote(root, branch))
        .transpose()?
        .flatten()
    {
        let remote_head = format!("refs/remotes/{remote}/HEAD");
        if let Some(output) = optional_git_output(
            root,
            ["symbolic-ref", "--quiet", "--short", remote_head.as_str()],
            MAX_GIT_BRANCH_BYTES,
            GIT_TIMEOUT,
        )? {
            let value = String::from_utf8(output.stdout).map_err(|_| GitError::InvalidOutput)?;
            let prefix = format!("{remote}/");
            if let Some(default_branch) = value.trim().strip_prefix(&prefix)
                && !default_branch.is_empty()
            {
                return Ok(Some(default_branch.to_owned()));
            }
        }
    }
    Ok(["main", "master"]
        .into_iter()
        .find(|candidate| branches.iter().any(|branch| branch.name == *candidate))
        .map(str::to_owned))
}

fn review_default_base(
    root: &Path,
    branch: Option<&str>,
    default_branch: Option<&str>,
) -> Result<Option<String>, GitError> {
    let Some(default_branch) = default_branch else {
        return Ok(None);
    };
    let remote = branch
        .map(|branch| push_remote(root, branch))
        .transpose()?
        .flatten();
    if let Some(remote) = remote {
        let remote_ref = format!("refs/remotes/{remote}/{default_branch}");
        if optional_git_output(
            root,
            ["rev-parse", "--verify", "--quiet", remote_ref.as_str()],
            128,
            GIT_TIMEOUT,
        )?
        .is_some()
        {
            return Ok(Some(format!("{remote}/{default_branch}")));
        }
    }
    Ok(Some(default_branch.to_owned()))
}

fn review_commits(
    root: &Path,
    branch: Option<&str>,
    default_branch: Option<&str>,
) -> Result<(Vec<GitReviewCommit>, bool), GitError> {
    let (Some(branch), Some(default_branch)) = (branch, default_branch) else {
        return Ok((Vec::new(), false));
    };
    if branch == default_branch {
        return Ok((Vec::new(), false));
    }

    let local_ref = format!("refs/heads/{default_branch}");
    let base_ref = if optional_git_output(
        root,
        ["rev-parse", "--verify", "--quiet", local_ref.as_str()],
        128,
        GIT_TIMEOUT,
    )?
    .is_some()
    {
        Some(local_ref)
    } else if let Some(remote) = push_remote(root, branch)? {
        let remote_ref = format!("refs/remotes/{remote}/{default_branch}");
        optional_git_output(
            root,
            ["rev-parse", "--verify", "--quiet", remote_ref.as_str()],
            128,
            GIT_TIMEOUT,
        )?
        .map(|_| remote_ref)
    } else {
        None
    };
    let Some(base_ref) = base_ref else {
        return Ok((Vec::new(), false));
    };

    let range = format!("{base_ref}..HEAD");
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(root)
        .arg("log")
        .arg(format!("--max-count={}", MAX_GIT_REVIEW_COMMITS + 1))
        .arg("--format=%H%x00%ct%x00%s%x00%B%x00")
        .arg(range);
    let output = run_bounded(
        &mut command,
        MAX_GIT_REVIEW_LOG_BYTES,
        MAX_GIT_STDERR_BYTES,
        GIT_TIMEOUT,
    )?;
    let mut fields = output.stdout.split(|byte| *byte == 0);
    let mut commits = Vec::new();
    while let (Some(sha), Some(committed_at), Some(subject), Some(message)) =
        (fields.next(), fields.next(), fields.next(), fields.next())
    {
        let sha = String::from_utf8_lossy(sha).trim().to_owned();
        let committed_at = String::from_utf8_lossy(committed_at)
            .trim()
            .parse::<i64>()
            .ok();
        if sha.is_empty()
            || sha.len() > 64
            || !sha.bytes().all(|byte| byte.is_ascii_hexdigit())
            || committed_at.is_none()
        {
            continue;
        }
        commits.push(GitReviewCommit {
            sha,
            subject: bounded_git_text(subject, MAX_GIT_REVIEW_COMMIT_SUBJECT_BYTES),
            message: bounded_git_text(message, MAX_GIT_REVIEW_COMMIT_MESSAGE_BYTES),
            committed_at: committed_at.unwrap_or_default(),
        });
    }
    let truncated = output.stdout_truncated || commits.len() > MAX_GIT_REVIEW_COMMITS;
    commits.truncate(MAX_GIT_REVIEW_COMMITS);
    Ok((commits, truncated))
}

fn bounded_git_text(bytes: &[u8], limit: usize) -> String {
    let mut text = String::new();
    let mut truncated = false;
    push_bounded_text(
        &mut text,
        &String::from_utf8_lossy(bytes),
        limit,
        &mut truncated,
    );
    text.trim().to_owned()
}

fn validate_branch(root: &Path, branch: &str) -> Result<(), GitError> {
    if branch.is_empty()
        || branch.len() > MAX_GIT_BRANCH_BYTES
        || branch.starts_with('-')
        || branch.chars().any(char::is_control)
    {
        return Err(GitError::InvalidBranchName);
    }
    git_output(
        root,
        ["check-ref-format", "--branch", branch],
        64 * 1024,
        GIT_TIMEOUT,
    )
    .map_err(|error| match error {
        GitError::Process(ProcessError::Exit { .. }) => GitError::InvalidBranchName,
        other => other,
    })?;
    Ok(())
}

fn parse_checkout_conflicts(stderr: &str) -> Option<(Vec<PathBuf>, bool)> {
    const TRACKED_MARKER: &str =
        "Your local changes to the following files would be overwritten by checkout:";
    const UNTRACKED_MARKER: &str =
        "The following untracked working tree files would be overwritten by checkout:";

    let mut collecting = false;
    let mut found_marker = false;
    let mut truncated = false;
    let mut paths = Vec::new();
    for line in stderr.lines() {
        let trimmed = line.trim();
        if trimmed.contains(TRACKED_MARKER) || trimmed.contains(UNTRACKED_MARKER) {
            collecting = true;
            found_marker = true;
            continue;
        }
        if !collecting {
            continue;
        }
        if trimmed.is_empty() {
            continue;
        }
        if !line.starts_with(char::is_whitespace)
            || trimmed.starts_with("Please ")
            || trimmed.eq_ignore_ascii_case("Aborting")
        {
            collecting = false;
            continue;
        }
        if trimmed.len() > MAX_GIT_PATH_BYTES {
            truncated = true;
            continue;
        }
        if paths.len() == MAX_GIT_CONFLICT_PATHS {
            truncated = true;
            continue;
        }
        paths.push(PathBuf::from(trimmed));
    }
    found_marker.then_some((paths, truncated))
}

fn git_output<const N: usize>(
    cwd: &Path,
    args: [&str; N],
    stdout_limit: usize,
    timeout: Duration,
) -> Result<BoundedOutput, GitError> {
    let mut command = Command::new("git");
    command.arg("-C").arg(cwd).args(args);
    Ok(run_bounded(
        &mut command,
        stdout_limit,
        MAX_GIT_STDERR_BYTES,
        timeout,
    )?)
}

fn optional_git_output<const N: usize>(
    cwd: &Path,
    args: [&str; N],
    stdout_limit: usize,
    timeout: Duration,
) -> Result<Option<BoundedOutput>, GitError> {
    match git_output(cwd, args, stdout_limit, timeout) {
        Ok(output) => Ok(Some(output)),
        Err(GitError::Process(ProcessError::Exit { .. })) => Ok(None),
        Err(error) => Err(error),
    }
}

fn git_output_with_path<const N: usize>(
    root: &Path,
    args: [&str; N],
    path: &Path,
    stdout_limit: usize,
) -> Result<BoundedOutput, GitError> {
    let mut command = Command::new("git");
    command.arg("-C").arg(root).args(args).arg("--").arg(path);
    Ok(run_bounded(
        &mut command,
        stdout_limit,
        MAX_GIT_STDERR_BYTES,
        GIT_TIMEOUT,
    )?)
}

fn is_untracked(root: &Path, path: &Path) -> Result<bool, GitError> {
    let output = git_output_with_path(
        root,
        ["ls-files", "--others", "--exclude-standard", "-z"],
        path,
        MAX_GIT_PATH_BYTES,
    )?;
    Ok(!output.stdout.is_empty())
}

fn untracked_diff(root: &Path, relative: &Path) -> Result<GitDiff, GitError> {
    untracked_diff_with_limit(root, relative, MAX_GIT_DIFF_BYTES)
}

fn untracked_diff_with_limit(
    root: &Path,
    relative: &Path,
    limit: usize,
) -> Result<GitDiff, GitError> {
    let canonical_root = fs::canonicalize(root)?;
    let canonical_path = fs::canonicalize(root.join(relative))?;
    if !canonical_path.starts_with(&canonical_root) || !canonical_path.is_file() {
        return Err(GitError::PathOutsideRepository);
    }

    let mut bytes = Vec::with_capacity(limit.saturating_add(1));
    File::open(canonical_path)?
        .take(limit.saturating_add(1) as u64)
        .read_to_end(&mut bytes)?;
    let input_truncated = bytes.len() > limit;
    bytes.truncate(limit);
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return Ok(GitDiff {
            text: String::new(),
            truncated: input_truncated,
        });
    };

    let path = escaped_git_path(relative);
    let line_count = text.lines().count();
    let mut output = String::new();
    let mut output_truncated = false;
    push_bounded_text(
        &mut output,
        &format!(
            "diff --git a/{path} b/{path}\nnew file mode 100644\n--- /dev/null\n+++ b/{path}\n"
        ),
        limit,
        &mut output_truncated,
    );
    if line_count > 0 && !output_truncated {
        push_bounded_text(
            &mut output,
            &format!("@@ -0,0 +1,{line_count} @@\n"),
            limit,
            &mut output_truncated,
        );
        for line in text.lines() {
            push_bounded_text(&mut output, "+", limit, &mut output_truncated);
            push_bounded_text(&mut output, line, limit, &mut output_truncated);
            push_bounded_text(&mut output, "\n", limit, &mut output_truncated);
            if output_truncated {
                break;
            }
        }
        if !input_truncated && !text.is_empty() && !text.ends_with('\n') {
            push_bounded_text(
                &mut output,
                "\\ No newline at end of file\n",
                limit,
                &mut output_truncated,
            );
        }
    }
    Ok(GitDiff {
        text: output,
        truncated: input_truncated || output_truncated,
    })
}

fn append_untracked_diffs(
    root: &Path,
    text: &mut String,
    truncated: &mut bool,
) -> Result<(), GitError> {
    if *truncated {
        return Ok(());
    }
    let untracked = git_output(
        root,
        ["ls-files", "--others", "--exclude-standard", "-z"],
        MAX_GIT_METADATA_BYTES,
        GIT_TIMEOUT,
    )?;
    let mut paths = untracked
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty());
    for path in paths.by_ref().take(MAX_GIT_FILES) {
        let remaining = MAX_GIT_DIFF_BYTES.saturating_sub(text.len());
        if remaining == 0 {
            *truncated = true;
            break;
        }
        if !text.is_empty() {
            push_bounded_text(text, "\n", MAX_GIT_DIFF_BYTES, truncated);
        }
        let diff = untracked_diff_with_limit(root, &git_path(path), remaining)?;
        push_bounded_text(text, &diff.text, MAX_GIT_DIFF_BYTES, truncated);
        *truncated |= diff.truncated;
        if *truncated {
            break;
        }
    }
    *truncated |= paths.next().is_some() || untracked.stdout_truncated;
    Ok(())
}

fn escaped_git_path(path: &Path) -> String {
    let mut escaped = String::new();
    for character in path.to_string_lossy().chars() {
        for escaped_character in character.escape_default() {
            if escaped.len() + escaped_character.len_utf8() > MAX_GIT_PATH_BYTES {
                return escaped;
            }
            escaped.push(escaped_character);
        }
    }
    escaped
}

fn push_bounded_text(output: &mut String, value: &str, limit: usize, truncated: &mut bool) {
    if *truncated {
        return;
    }
    let remaining = limit.saturating_sub(output.len());
    if value.len() <= remaining {
        output.push_str(value);
        return;
    }
    let mut end = remaining.min(value.len());
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    output.push_str(&value[..end]);
    *truncated = true;
}

fn repository_relative_path(root: &Path, path: &Path) -> Result<PathBuf, GitError> {
    if path.is_relative() {
        if path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir | std::path::Component::RootDir
            )
        }) {
            return Err(GitError::PathOutsideRepository);
        }
        return Ok(path.to_path_buf());
    }
    path.strip_prefix(root)
        .map(Path::to_path_buf)
        .map_err(|_| GitError::PathOutsideRepository)
}

fn parse_status(bytes: &[u8]) -> Result<ParsedGitStatus, GitError> {
    let mut branch = None;
    let mut upstream_ref = None;
    let mut ahead = 0;
    let mut behind = 0;
    let mut files = Vec::new();
    let mut records = bytes.split(|byte| *byte == 0).peekable();
    while let Some(record) = records.next() {
        if record.is_empty() {
            continue;
        }
        if record.starts_with(b"# branch.head ") {
            let value = String::from_utf8_lossy(&record[14..]).into_owned();
            if value != "(detached)" {
                branch = Some(value);
            }
            continue;
        }
        if record.starts_with(b"# branch.ab ") {
            let value = String::from_utf8_lossy(&record[12..]);
            for part in value.split_whitespace() {
                if let Some(value) = part.strip_prefix('+') {
                    ahead = value.parse().unwrap_or(0);
                } else if let Some(value) = part.strip_prefix('-') {
                    behind = value.parse().unwrap_or(0);
                }
            }
            continue;
        }
        if record.starts_with(b"# branch.upstream ") {
            let value = String::from_utf8_lossy(&record[18..]).into_owned();
            if !value.is_empty() {
                upstream_ref = Some(value);
            }
            continue;
        }
        match record[0] {
            b'1' => {
                let fields = record.splitn(9, |byte| *byte == b' ').collect::<Vec<_>>();
                if fields.len() != 9 {
                    return Err(GitError::InvalidOutput);
                }
                files.push(status_file(fields[1], fields[8], None));
            }
            b'2' => {
                let fields = record.splitn(10, |byte| *byte == b' ').collect::<Vec<_>>();
                if fields.len() != 10 {
                    return Err(GitError::InvalidOutput);
                }
                let old_path = records.next().filter(|path| !path.is_empty());
                files.push(status_file(fields[1], fields[9], old_path));
            }
            b'u' => {
                let fields = record.splitn(11, |byte| *byte == b' ').collect::<Vec<_>>();
                if fields.len() != 11 {
                    return Err(GitError::InvalidOutput);
                }
                files.push(GitFile {
                    path: git_path(fields[10]),
                    old_path: None,
                    kind: GitFileKind::Conflicted,
                    staged: true,
                    unstaged: true,
                    staged_additions: 0,
                    staged_deletions: 0,
                    unstaged_additions: 0,
                    unstaged_deletions: 0,
                });
            }
            b'?' => files.push(GitFile {
                path: git_path(record.get(2..).unwrap_or_default()),
                old_path: None,
                kind: GitFileKind::Untracked,
                staged: false,
                unstaged: true,
                staged_additions: 0,
                staged_deletions: 0,
                unstaged_additions: 0,
                unstaged_deletions: 0,
            }),
            b'!' | b'#' => {}
            _ => return Err(GitError::InvalidOutput),
        }
    }
    Ok((branch, upstream_ref, ahead, behind, files))
}

fn status_file(xy: &[u8], path: &[u8], old_path: Option<&[u8]>) -> GitFile {
    let staged_code = xy.first().copied().unwrap_or(b'.');
    let unstaged_code = xy.get(1).copied().unwrap_or(b'.');
    let code = if unstaged_code != b'.' {
        unstaged_code
    } else {
        staged_code
    };
    GitFile {
        path: git_path(path),
        old_path: old_path.map(git_path),
        kind: match code {
            b'A' => GitFileKind::Added,
            b'D' => GitFileKind::Deleted,
            b'R' => GitFileKind::Renamed,
            b'C' => GitFileKind::Copied,
            b'T' => GitFileKind::TypeChanged,
            b'U' => GitFileKind::Conflicted,
            _ => GitFileKind::Modified,
        },
        staged: staged_code != b'.',
        unstaged: unstaged_code != b'.',
        staged_additions: 0,
        staged_deletions: 0,
        unstaged_additions: 0,
        unstaged_deletions: 0,
    }
}

fn parse_numstat(bytes: &[u8]) -> HashMap<PathBuf, (u32, u32)> {
    bytes
        .split(|byte| *byte == 0)
        .filter_map(|record| {
            let mut fields = record.splitn(3, |byte| *byte == b'\t');
            let additions = parse_numstat_count(fields.next()?)?;
            let deletions = parse_numstat_count(fields.next()?)?;
            let path = fields.next()?;
            (!path.is_empty()).then(|| (git_path(path), (additions, deletions)))
        })
        .take(MAX_GIT_FILES)
        .collect()
}

fn numstat_exceeds_limit(bytes: &[u8]) -> bool {
    bytes
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        .nth(MAX_GIT_FILES)
        .is_some()
}

fn parse_numstat_count(bytes: &[u8]) -> Option<u32> {
    if bytes == b"-" {
        return Some(0);
    }
    std::str::from_utf8(bytes).ok()?.parse().ok()
}

fn apply_numstat(files: &mut [GitFile], stats: &HashMap<PathBuf, (u32, u32)>, staged: bool) {
    for file in files {
        let Some(&(additions, deletions)) = stats.get(&file.path) else {
            continue;
        };
        if staged {
            file.staged_additions = additions;
            file.staged_deletions = deletions;
        } else {
            file.unstaged_additions = additions;
            file.unstaged_deletions = deletions;
        }
    }
}

fn parse_branches(bytes: &[u8]) -> Vec<GitBranch> {
    String::from_utf8_lossy(bytes)
        .lines()
        .filter_map(|line| {
            let mut fields = line.splitn(3, '\t');
            Some(GitBranch {
                name: fields.next()?.to_owned(),
                commit: fields.next()?.to_owned(),
                current: fields.next()?.trim() == "*",
            })
        })
        .collect()
}

fn parse_worktrees(bytes: &[u8]) -> Vec<GitWorktree> {
    let mut worktrees = Vec::new();
    let mut current: Option<GitWorktree> = None;
    for line in String::from_utf8_lossy(bytes).lines() {
        if line.is_empty() {
            if let Some(worktree) = current.take() {
                worktrees.push(worktree);
            }
        } else if let Some(path) = line.strip_prefix("worktree ") {
            if let Some(worktree) = current.take() {
                worktrees.push(worktree);
            }
            current = Some(GitWorktree {
                path: PathBuf::from(path),
                branch: None,
                commit: None,
                bare: false,
                detached: false,
                locked: false,
            });
        } else if let Some(worktree) = current.as_mut() {
            if let Some(commit) = line.strip_prefix("HEAD ") {
                worktree.commit = Some(commit.to_owned());
            } else if let Some(branch) = line.strip_prefix("branch refs/heads/") {
                worktree.branch = Some(branch.to_owned());
            } else if line == "bare" {
                worktree.bare = true;
            } else if line == "detached" {
                worktree.detached = true;
            } else if line.starts_with("locked") {
                worktree.locked = true;
            }
        }
    }
    if let Some(worktree) = current {
        worktrees.push(worktree);
    }
    worktrees
}

fn git_path(bytes: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    use super::{
        GitFileKind, MAX_GIT_COMMIT_CONTEXT_BYTES, MAX_GIT_CONFLICT_PATHS, branch_diff, commit,
        commit_diff, commit_message_diff, parse_branches, parse_checkout_conflicts, parse_numstat,
        parse_status, parse_worktrees, pull_request_context, push, repository_relative_path,
        snapshot, uncommitted_diff, untracked_diff, valid_worktree_path,
    };

    struct TemporaryDirectory(PathBuf);

    impl Drop for TemporaryDirectory {
        fn drop(&mut self) {
            if self.0.starts_with(std::env::temp_dir()) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
    }

    fn run_git(root: &Path, args: &[&str]) -> Result<String, super::GitError> {
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()?;
        if !output.status.success() {
            return Err(super::GitError::InvalidOutput);
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    fn temporary_git_repository(prefix: &str) -> Result<TemporaryDirectory, super::GitError> {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| super::GitError::InvalidOutput)?
            .as_nanos();
        let root = std::env::temp_dir().join(format!("{prefix}-{}-{unique}", std::process::id()));
        fs::create_dir(&root)?;
        let directory = TemporaryDirectory(root);
        run_git(&directory.0, &["init", "-q"])?;
        run_git(
            &directory.0,
            &["config", "user.email", "codexrs@example.invalid"],
        )?;
        run_git(&directory.0, &["config", "user.name", "codexRS Tests"])?;
        fs::write(directory.0.join("tracked.txt"), "base\n")?;
        run_git(&directory.0, &["add", "--", "tracked.txt"])?;
        run_git(&directory.0, &["commit", "-q", "-m", "Initial commit"])?;
        Ok(directory)
    }

    #[test]
    fn parses_porcelain_v2_without_line_based_path_reads() -> Result<(), super::GitError> {
        let input = b"# branch.head main\0# branch.upstream origin/main\0# branch.ab +2 -1\0\
            1 .M N... 100644 100644 100644 abc def src/lib.rs\0\
            ? notes with spaces.md\0";
        let (branch, upstream_ref, ahead, behind, files) = parse_status(input)?;

        assert_eq!(branch.as_deref(), Some("main"));
        assert_eq!(upstream_ref.as_deref(), Some("origin/main"));
        assert_eq!((ahead, behind), (2, 1));
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, PathBuf::from("src/lib.rs"));
        assert_eq!(files[0].kind, GitFileKind::Modified);
        assert_eq!(files[1].path, PathBuf::from("notes with spaces.md"));
        assert_eq!(files[1].kind, GitFileKind::Untracked);
        Ok(())
    }

    #[test]
    fn parses_bounded_branch_and_worktree_metadata() {
        let branches = parse_branches(b"main\tabc123\t*\nfeature/x\tdef456\t \n");
        assert_eq!(branches.len(), 2);
        assert!(branches[0].current);

        let worktrees = parse_worktrees(
            b"worktree C:/repo\nHEAD abc\nbranch refs/heads/main\n\n\
              worktree C:/repo-wt\nHEAD def\ndetached\nlocked reason\n\n",
        );
        assert_eq!(worktrees.len(), 2);
        assert_eq!(worktrees[0].branch.as_deref(), Some("main"));
        assert!(worktrees[1].detached);
        assert!(worktrees[1].locked);
    }

    #[test]
    fn checkout_conflicts_are_bounded_and_keep_utf8_paths() -> Result<(), super::GitError> {
        let mut stderr = String::from(
            "error: Your local changes to the following files would be overwritten by checkout:\n",
        );
        stderr.push_str("\tREADME.md\n\tпапка/файл.rs\n");
        for index in 0..MAX_GIT_CONFLICT_PATHS {
            stderr.push_str(&format!("\tsrc/file-{index}.rs\n"));
        }
        stderr.push_str("Please commit your changes or stash them before you switch branches.\n");
        stderr.push_str("Aborting\n");

        let (paths, truncated) =
            parse_checkout_conflicts(&stderr).ok_or(super::GitError::InvalidOutput)?;

        assert_eq!(paths[0], PathBuf::from("README.md"));
        assert_eq!(paths[1], PathBuf::from("папка/файл.rs"));
        assert_eq!(paths.len(), MAX_GIT_CONFLICT_PATHS);
        assert!(truncated);
        Ok(())
    }

    #[test]
    fn parses_text_and_binary_numstat_without_line_based_paths() {
        let stats = parse_numstat(
            b"12\t3\tsrc/file with spaces.rs\0-\t-\tassets/image with tabs\tand spaces.png\0",
        );

        assert_eq!(
            stats.get(&PathBuf::from("src/file with spaces.rs")),
            Some(&(12, 3))
        );
        assert_eq!(
            stats.get(&PathBuf::from("assets/image with tabs\tand spaces.png")),
            Some(&(0, 0))
        );
    }

    #[test]
    fn untracked_text_diff_is_bounded_and_uses_an_added_file_patch() -> Result<(), super::GitError>
    {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| super::GitError::InvalidOutput)?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "codexrs-untracked-diff-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&root)?;
        let directory = TemporaryDirectory(root);
        fs::write(directory.0.join("new file.txt"), "first\nsecond\n")?;

        let diff = untracked_diff(&directory.0, Path::new("new file.txt"))?;

        assert!(!diff.truncated);
        assert!(diff.text.contains("--- /dev/null"));
        assert!(diff.text.contains("+++ b/new file.txt"));
        assert!(diff.text.contains("+first\n+second\n"));
        Ok(())
    }

    #[test]
    fn untracked_text_diff_marks_a_missing_final_newline() -> Result<(), super::GitError> {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| super::GitError::InvalidOutput)?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "codexrs-untracked-no-final-newline-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&root)?;
        let directory = TemporaryDirectory(root);
        fs::write(directory.0.join("new file.txt"), "first\nsecond")?;

        let diff = untracked_diff(&directory.0, Path::new("new file.txt"))?;

        assert!(!diff.truncated);
        assert!(
            diff.text
                .ends_with("+first\n+second\n\\ No newline at end of file\n")
        );
        Ok(())
    }

    #[test]
    fn untracked_empty_file_diff_has_no_final_newline_marker() -> Result<(), super::GitError> {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| super::GitError::InvalidOutput)?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "codexrs-untracked-empty-file-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&root)?;
        let directory = TemporaryDirectory(root);
        fs::write(directory.0.join("empty.txt"), [])?;

        let diff = untracked_diff(&directory.0, Path::new("empty.txt"))?;

        assert!(!diff.truncated);
        assert_eq!(
            diff.text,
            "diff --git a/empty.txt b/empty.txt\nnew file mode 100644\n--- /dev/null\n+++ b/empty.txt\n"
        );
        Ok(())
    }

    #[test]
    fn commit_context_and_include_unstaged_commit_are_bounded_and_isolated()
    -> Result<(), super::GitError> {
        let directory = temporary_git_repository("codexrs-commit")?;
        fs::write(directory.0.join("tracked.txt"), "staged\n")?;
        run_git(&directory.0, &["add", "--", "tracked.txt"])?;
        fs::write(directory.0.join("tracked.txt"), "staged\nunstaged\n")?;
        fs::write(directory.0.join("new file.txt"), "new context\n")?;

        let context = commit_message_diff(&directory.0, true)?;

        assert!(context.text.len() <= MAX_GIT_COMMIT_CONTEXT_BYTES);
        assert!(context.text.contains("+staged"));
        assert!(context.text.contains("+unstaged"));
        assert!(context.text.contains("new file mode"));
        commit(&directory.0, "Capture native Git changes", true)?;
        assert!(run_git(&directory.0, &["status", "--porcelain"])?.is_empty());
        assert_eq!(
            run_git(&directory.0, &["log", "-1", "--pretty=%s"])?.trim(),
            "Capture native Git changes"
        );
        Ok(())
    }

    #[test]
    fn review_sources_keep_branch_commits_and_working_tree_changes_separate()
    -> Result<(), super::GitError> {
        let directory = temporary_git_repository("codexrs-review-sources")?;
        let base_branch = run_git(&directory.0, &["branch", "--show-current"])?
            .trim()
            .to_owned();
        let base_sha = run_git(&directory.0, &["rev-parse", "HEAD"])?
            .trim()
            .to_owned();
        run_git(
            &directory.0,
            &["switch", "-q", "-c", "feature/review-sources"],
        )?;
        fs::write(directory.0.join("feature.txt"), "committed feature\n")?;
        run_git(&directory.0, &["add", "--", "feature.txt"])?;
        run_git(
            &directory.0,
            &[
                "commit",
                "-q",
                "-m",
                "Add committed feature",
                "-m",
                "Review source body",
            ],
        )?;
        let commit_sha = run_git(&directory.0, &["rev-parse", "HEAD"])?
            .trim()
            .to_owned();

        fs::write(directory.0.join("tracked.txt"), "working tree change\n")?;
        fs::write(directory.0.join("untracked.txt"), "untracked change\n")?;

        let repository = snapshot(&directory.0)?;
        assert_eq!(repository.commits.len(), 1);
        assert_eq!(repository.commits[0].sha, commit_sha);
        assert_eq!(repository.commits[0].subject, "Add committed feature");
        assert!(repository.commits[0].message.contains("Review source body"));
        assert!(repository.commits[0].committed_at > 0);
        assert_eq!(
            repository.review_default_base.as_deref(),
            Some(base_branch.as_str())
        );
        assert_eq!(
            repository.review_branches,
            std::slice::from_ref(&base_branch)
        );

        let working_tree = uncommitted_diff(&directory.0)?;
        assert!(!working_tree.truncated);
        assert!(working_tree.text.contains("+working tree change"));
        assert!(working_tree.text.contains("+++ b/untracked.txt"));
        assert!(working_tree.text.contains("+untracked change"));

        let committed = commit_diff(&directory.0, &commit_sha)?;
        assert!(!committed.truncated);
        assert!(committed.text.contains("+++ b/feature.txt"));
        assert!(committed.text.contains("+committed feature"));
        assert!(!committed.text.contains("working tree change"));
        assert!(!committed.text.contains("untracked change"));

        let branch = branch_diff(&directory.0, &base_branch)?;
        assert_eq!(branch.base_sha, base_sha);
        assert!(!branch.truncated);
        assert!(branch.text.contains("+committed feature"));
        assert!(branch.text.contains("+working tree change"));
        assert!(branch.text.contains("+++ b/untracked.txt"));
        assert!(branch.text.contains("+untracked change"));
        assert!(branch_diff(&directory.0, "\ninvalid").is_err());
        Ok(())
    }

    #[test]
    fn push_uses_the_existing_upstream_and_publishes_a_new_branch() -> Result<(), super::GitError> {
        let directory = temporary_git_repository("codexrs-push")?;
        let remote = directory.0.join("remote.git");
        let remote_text = remote.to_string_lossy().into_owned();
        run_git(&directory.0, &["init", "-q", "--bare", &remote_text])?;
        run_git(&directory.0, &["remote", "add", "origin", &remote_text])?;
        let initial_branch = run_git(&directory.0, &["branch", "--show-current"])?
            .trim()
            .to_owned();
        run_git(
            &directory.0,
            &["push", "-q", "-u", "origin", &initial_branch],
        )?;

        fs::write(directory.0.join("tracked.txt"), "pushed\n")?;
        run_git(&directory.0, &["add", "--", "tracked.txt"])?;
        run_git(&directory.0, &["commit", "-q", "-m", "Push tracked change"])?;
        push(&directory.0, false)?;
        assert_eq!(
            run_git(
                &remote,
                &[
                    "log",
                    "-1",
                    "--pretty=%s",
                    &format!("refs/heads/{initial_branch}")
                ]
            )?
            .trim(),
            "Push tracked change"
        );

        run_git(&directory.0, &["switch", "-q", "-c", "feature/published"])?;
        fs::write(directory.0.join("feature.txt"), "published\n")?;
        run_git(&directory.0, &["add", "--", "feature.txt"])?;
        run_git(
            &directory.0,
            &["commit", "-q", "-m", "Publish feature branch"],
        )?;
        push(&directory.0, false)?;
        assert_eq!(
            run_git(
                &directory.0,
                &[
                    "rev-parse",
                    "--abbrev-ref",
                    "--symbolic-full-name",
                    "@{upstream}"
                ]
            )?
            .trim(),
            "origin/feature/published"
        );
        assert_eq!(
            run_git(
                &remote,
                &["log", "-1", "--pretty=%s", "refs/heads/feature/published"]
            )?
            .trim(),
            "Publish feature branch"
        );
        Ok(())
    }

    #[test]
    fn pull_request_context_is_bounded_to_the_base_branch_and_local_changes()
    -> Result<(), super::GitError> {
        let directory = temporary_git_repository("codexrs-pr-context")?;
        let base_branch = run_git(&directory.0, &["branch", "--show-current"])?
            .trim()
            .to_owned();
        run_git(&directory.0, &["switch", "-q", "-c", "feature/native-pr"])?;
        fs::write(directory.0.join("feature.txt"), "committed\n")?;
        run_git(&directory.0, &["add", "--", "feature.txt"])?;
        run_git(&directory.0, &["commit", "-q", "-m", "Add feature"])?;
        fs::write(directory.0.join("tracked.txt"), "local\n")?;

        let context = pull_request_context(&directory.0, &base_branch, true)?;

        assert!(context.text.contains("+committed"));
        assert!(context.text.contains("+local"));
        assert!(context.text.len() <= super::MAX_GIT_PULL_REQUEST_CONTEXT_BYTES);
        Ok(())
    }

    #[test]
    fn rejects_paths_that_escape_the_repository() {
        assert!(repository_relative_path(Path::new("C:/repo"), Path::new("../secret")).is_err());
    }

    #[test]
    fn worktrees_require_an_absolute_sibling_path() {
        let root = if cfg!(windows) {
            Path::new("C:/repo")
        } else {
            Path::new("/repo")
        };
        let sibling = if cfg!(windows) {
            Path::new("C:/repo-feature")
        } else {
            Path::new("/repo-feature")
        };
        assert!(valid_worktree_path(root, sibling));
        assert!(valid_worktree_path(
            root,
            &root.join("..").join("repo-feature")
        ));
        assert!(!valid_worktree_path(root, &root.join("worktrees/feature")));
        assert!(!valid_worktree_path(
            root,
            &root
                .parent()
                .unwrap_or(root)
                .join("repo-alias")
                .join("..")
                .join(root.file_name().unwrap_or_default())
                .join("worktrees/feature")
        ));
        assert!(!valid_worktree_path(root, Path::new("../repo-feature")));
    }

    #[cfg(windows)]
    #[test]
    fn windows_worktree_paths_resolve_filesystem_aliases() -> Result<(), super::GitError> {
        let directory = temporary_git_repository("codexrs-worktree-path-alias")?;
        let root = directory.0.join("Répo");
        fs::create_dir(&root)?;

        let verbatim_root = fs::canonicalize(&root)?;
        assert!(!valid_worktree_path(&root, &verbatim_root.join("feature")));
        assert!(!valid_worktree_path(&verbatim_root, &root.join("feature")));

        let case_alias = directory.0.join("répo");
        if fs::canonicalize(&case_alias).is_ok() {
            assert!(!valid_worktree_path(&root, &case_alias.join("feature")));
        }
        assert!(valid_worktree_path(
            &root,
            &directory.0.join("Répo-feature")
        ));
        Ok(())
    }

    #[test]
    fn managed_worktree_preserves_the_bounded_working_tree_and_nested_workspace()
    -> Result<(), super::GitError> {
        let repository = temporary_git_repository("codexrs-managed-worktree-source")?;
        fs::create_dir_all(repository.0.join("crates/native"))?;
        fs::write(
            repository.0.join("crates/native/lib.rs"),
            b"pub fn base() {}\n",
        )?;
        fs::write(
            repository.0.join(".gitignore"),
            b".secret/\nAGENTS.override.md\n",
        )?;
        fs::write(repository.0.join(".worktreeinclude"), b".secret/**\n")?;
        run_git(&repository.0, &["add", "."])?;
        run_git(
            &repository.0,
            &["commit", "-q", "-m", "Add nested workspace"],
        )?;

        fs::write(repository.0.join("tracked.txt"), b"working tree\n")?;
        fs::write(
            repository.0.join("crates/native/untracked.bin"),
            [0_u8, 1, 2, 3, 255],
        )?;
        fs::create_dir_all(repository.0.join(".secret"))?;
        fs::write(repository.0.join(".secret/config.bin"), [9_u8, 8, 7])?;
        fs::write(
            repository.0.join("crates/native/AGENTS.override.md"),
            b"# Worktree instructions\n",
        )?;

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| super::GitError::InvalidOutput)?
            .as_nanos();
        let worktrees = TemporaryDirectory(std::env::temp_dir().join(format!(
            "codexrs-managed-worktree-root-{}-{unique}",
            std::process::id()
        )));
        fs::create_dir_all(&worktrees.0)?;

        let managed =
            super::create_managed_worktree(&repository.0.join("crates/native"), &worktrees.0)?;
        let canonical_worktrees = super::managed_worktree_canonicalize(&worktrees.0)?;
        assert!(managed.git_root.starts_with(canonical_worktrees));
        assert_eq!(
            managed.workspace_root,
            managed.git_root.join("crates/native")
        );
        assert_eq!(
            String::from_utf8_lossy(&fs::read(managed.git_root.join("tracked.txt"))?)
                .replace("\r\n", "\n"),
            "working tree\n"
        );
        assert_eq!(
            fs::read(managed.workspace_root.join("untracked.bin"))?,
            [0_u8, 1, 2, 3, 255]
        );
        assert_eq!(
            fs::read(managed.git_root.join(".secret/config.bin"))?,
            [9_u8, 8, 7]
        );
        assert_eq!(
            fs::read(managed.workspace_root.join("AGENTS.override.md"))?,
            b"# Worktree instructions\n"
        );
        assert_eq!(
            run_git(&managed.git_root, &["rev-parse", "--abbrev-ref", "HEAD"])?.trim(),
            "HEAD"
        );
        Ok(())
    }

    #[test]
    fn cancelled_managed_worktree_cleans_only_its_fresh_allocation() -> Result<(), super::GitError>
    {
        let repository = temporary_git_repository("codexrs-managed-worktree-cancelled")?;
        let destination = repository.0.with_extension("cancelled-worktrees");
        let destination_guard = TemporaryDirectory(destination.clone());
        fs::create_dir(&destination)?;
        let contents = vec![b'x'; 16 * 1024];
        for index in 0..1_000 {
            fs::write(
                repository.0.join(format!("untracked-{index:04}.bin")),
                &contents,
            )?;
        }
        let cancellation = Arc::new(AtomicBool::new(false));
        let worker_cancellation = Arc::clone(&cancellation);
        let worker_repository = repository.0.clone();
        let worker_destination = destination.clone();
        let worker = thread::spawn(move || {
            super::create_managed_worktree_cancellable(
                &worker_repository,
                &worker_destination,
                &worker_cancellation,
            )
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        while fs::read_dir(&destination)?.next().is_none() {
            assert!(
                !worker.is_finished() && Instant::now() < deadline,
                "worktree allocation should remain observable until cancellation"
            );
            thread::yield_now();
        }
        cancellation.store(true, Ordering::Release);
        let Ok(result) = worker.join() else {
            panic!("worktree worker should not panic");
        };

        assert!(matches!(result, Err(super::GitError::Cancelled)));
        assert_eq!(fs::read_dir(&destination)?.count(), 0);
        assert!(repository.0.join("tracked.txt").is_file());
        drop(destination_guard);
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn managed_worktree_git_paths_drop_only_supported_verbatim_prefixes() {
        assert_eq!(
            super::managed_worktree_git_path(PathBuf::from(
                r"\\?\E:\workspace with spaces\worktree"
            )),
            PathBuf::from(r"E:\workspace with spaces\worktree")
        );
        assert_eq!(
            super::managed_worktree_git_path(PathBuf::from(r"\\?\UNC\server\share\worktree")),
            PathBuf::from(r"\\server\share\worktree")
        );
        assert_eq!(
            super::managed_worktree_git_path(PathBuf::from(r"\\?\Volume{1234-5678}\worktree")),
            PathBuf::from(r"\\?\Volume{1234-5678}\worktree")
        );
    }

    #[test]
    fn inspects_the_checkout_from_a_nested_crate_directory() -> Result<(), super::GitError> {
        let snapshot = super::snapshot(Path::new(env!("CARGO_MANIFEST_DIR")))?;

        assert!(snapshot.repository_root.join("Cargo.toml").is_file());
        assert!(!snapshot.worktrees.is_empty());
        Ok(())
    }
}
