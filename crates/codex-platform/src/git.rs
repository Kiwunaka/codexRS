use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::process::{BoundedOutput, ProcessError, run_bounded};

pub const MAX_GIT_FILES: usize = 2_000;
pub const MAX_GIT_BRANCHES: usize = 500;
pub const MAX_GIT_WORKTREES: usize = 100;
pub const MAX_GIT_DIFF_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_GIT_BRANCH_BYTES: usize = 1_024;

const MAX_GIT_METADATA_BYTES: usize = 2 * 1024 * 1024;
const MAX_GIT_STDERR_BYTES: usize = 16 * 1024;
const GIT_TIMEOUT: Duration = Duration::from_secs(15);

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
pub struct GitSnapshot {
    pub repository_root: PathBuf,
    pub branch: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub files: Vec<GitFile>,
    pub branches: Vec<GitBranch>,
    pub worktrees: Vec<GitWorktree>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitDiff {
    pub text: String,
    pub truncated: bool,
}

#[derive(Debug)]
pub enum GitError {
    Process(ProcessError),
    InvalidRepository,
    InvalidOutput,
    PathOutsideRepository,
    InvalidBranchName,
    InvalidWorktreePath,
}

impl fmt::Display for GitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Process(error) => error.fmt(formatter),
            Self::InvalidRepository => {
                formatter.write_str("working directory is not a Git repository")
            }
            Self::InvalidOutput => formatter.write_str("Git returned malformed output"),
            Self::PathOutsideRepository => {
                formatter.write_str("path is outside the selected Git repository")
            }
            Self::InvalidBranchName => formatter.write_str("Git branch name is invalid"),
            Self::InvalidWorktreePath => {
                formatter.write_str("worktree path must be an absolute path outside the repository")
            }
        }
    }
}

impl Error for GitError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Process(error) => Some(error),
            Self::InvalidRepository
            | Self::InvalidOutput
            | Self::PathOutsideRepository
            | Self::InvalidBranchName
            | Self::InvalidWorktreePath => None,
        }
    }
}

impl From<ProcessError> for GitError {
    fn from(error: ProcessError) -> Self {
        Self::Process(error)
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
    let (branch, ahead, behind, mut files) = parse_status(&status_output.stdout)?;
    let mut truncated = status_output.stdout_truncated || files.len() > MAX_GIT_FILES;
    files.truncate(MAX_GIT_FILES);

    let branch_output = git_output(
        &root,
        [
            "for-each-ref",
            "--format=%(refname:short)%09%(objectname:short)%09%(HEAD)",
            "refs/heads",
        ],
        MAX_GIT_METADATA_BYTES,
        GIT_TIMEOUT,
    )?;
    let mut branches = parse_branches(&branch_output.stdout);
    truncated |= branch_output.stdout_truncated || branches.len() > MAX_GIT_BRANCHES;
    branches.truncate(MAX_GIT_BRANCHES);

    let worktree_output = git_output(
        &root,
        ["worktree", "list", "--porcelain"],
        MAX_GIT_METADATA_BYTES,
        GIT_TIMEOUT,
    )?;
    let mut worktrees = parse_worktrees(&worktree_output.stdout);
    truncated |= worktree_output.stdout_truncated || worktrees.len() > MAX_GIT_WORKTREES;
    worktrees.truncate(MAX_GIT_WORKTREES);

    Ok(GitSnapshot {
        repository_root: root,
        branch,
        ahead,
        behind,
        files,
        branches,
        worktrees,
        truncated,
    })
}

pub fn diff(root: &Path, path: &Path) -> Result<GitDiff, GitError> {
    let relative = repository_relative_path(root, path)?;
    let budget = MAX_GIT_DIFF_BYTES / 2;
    let staged = git_output_with_path(
        root,
        [
            "diff",
            "--cached",
            "--no-ext-diff",
            "--no-color",
            "--unified=3",
        ],
        &relative,
        budget,
    )?;
    let unstaged = git_output_with_path(
        root,
        ["diff", "--no-ext-diff", "--no-color", "--unified=3"],
        &relative,
        budget,
    )?;
    let mut text = String::new();
    if !staged.stdout.is_empty() {
        text.push_str("## Staged\n\n");
        text.push_str(&String::from_utf8_lossy(&staged.stdout));
    }
    if !unstaged.stdout.is_empty() {
        if !text.is_empty() {
            text.push_str("\n\n");
        }
        text.push_str("## Working tree\n\n");
        text.push_str(&String::from_utf8_lossy(&unstaged.stdout));
    }
    Ok(GitDiff {
        text,
        truncated: staged.stdout_truncated || unstaged.stdout_truncated,
    })
}

pub fn stage(root: &Path, path: &Path) -> Result<(), GitError> {
    let relative = repository_relative_path(root, path)?;
    git_output_with_path(root, ["add"], &relative, 64 * 1024)?;
    Ok(())
}

pub fn unstage(root: &Path, path: &Path) -> Result<(), GitError> {
    let relative = repository_relative_path(root, path)?;
    git_output_with_path(root, ["reset", "-q"], &relative, 64 * 1024)?;
    Ok(())
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

pub fn switch_branch(root: &Path, branch: &str) -> Result<(), GitError> {
    validate_branch(root, branch)?;
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(root)
        .arg("switch")
        .arg("--")
        .arg(branch);
    run_bounded(
        &mut command,
        MAX_GIT_METADATA_BYTES,
        MAX_GIT_STDERR_BYTES,
        Duration::from_secs(60),
    )?;
    Ok(())
}

fn valid_worktree_path(root: &Path, worktree_path: &Path) -> bool {
    worktree_path.is_absolute() && !worktree_path.starts_with(root)
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

fn parse_status(bytes: &[u8]) -> Result<(Option<String>, u32, u32, Vec<GitFile>), GitError> {
    let mut branch = None;
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
                });
            }
            b'?' => files.push(GitFile {
                path: git_path(record.get(2..).unwrap_or_default()),
                old_path: None,
                kind: GitFileKind::Untracked,
                staged: false,
                unstaged: true,
            }),
            b'!' | b'#' => {}
            _ => return Err(GitError::InvalidOutput),
        }
    }
    Ok((branch, ahead, behind, files))
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
    use std::path::{Path, PathBuf};

    use super::{
        GitFileKind, parse_branches, parse_status, parse_worktrees, repository_relative_path,
        valid_worktree_path,
    };

    #[test]
    fn parses_porcelain_v2_without_line_based_path_reads() -> Result<(), super::GitError> {
        let input = b"# branch.head main\0# branch.ab +2 -1\0\
            1 .M N... 100644 100644 100644 abc def src/lib.rs\0\
            ? notes with spaces.md\0";
        let (branch, ahead, behind, files) = parse_status(input)?;

        assert_eq!(branch.as_deref(), Some("main"));
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
        assert!(!valid_worktree_path(root, &root.join("worktrees/feature")));
        assert!(!valid_worktree_path(root, Path::new("../repo-feature")));
    }

    #[test]
    fn inspects_the_checkout_from_a_nested_crate_directory() -> Result<(), super::GitError> {
        let snapshot = super::snapshot(Path::new(env!("CARGO_MANIFEST_DIR")))?;

        assert!(snapshot.repository_root.join("Cargo.toml").is_file());
        assert!(!snapshot.worktrees.is_empty());
        Ok(())
    }
}
