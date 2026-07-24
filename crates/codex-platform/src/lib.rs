use std::env;
use std::error::Error;
use std::fmt;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::time::Duration;

mod app_server;
mod computer_use;
mod git;
mod process;
mod terminal;

pub use app_server::{
    AppServerClient, AppServerConfig, AppServerConnection, AppServerError, AppServerEvent,
    CodexHome, CodexHomeKind, DEFAULT_THREAD_PAGE_LIMIT, MAX_THREAD_PAGE_LIMIT,
};
pub use computer_use::{
    ComputerButton, ComputerCapture, ComputerKey, ComputerUseError, ComputerWindow,
    MAX_COMPUTER_CAPTURE_BYTES, MAX_COMPUTER_TEXT_BYTES, MAX_COMPUTER_WINDOWS,
    capture_computer_window, click_computer_window, inspect_computer_window, list_computer_windows,
    move_over_computer_window, press_computer_key, scroll_computer_window,
    type_into_computer_window,
};
pub use git::{
    GitBranch, GitDiff, GitError, GitFile, GitFileKind, GitSnapshot, GitWorktree,
    MAX_GIT_BRANCH_BYTES, MAX_GIT_BRANCHES, MAX_GIT_DIFF_BYTES, MAX_GIT_FILES, MAX_GIT_WORKTREES,
    create_worktree, diff as git_diff, snapshot as git_snapshot, stage as git_stage, switch_branch,
    unstage as git_unstage,
};
pub use process::ProcessError;
pub use terminal::{
    MAX_TERMINAL_INPUT_BYTES, TERMINAL_EVENT_CAPACITY, TerminalCommandError, TerminalConfig,
    TerminalError, TerminalEvent, TerminalSession,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataDirectoryError {
    HomeUnavailable,
}

impl fmt::Display for DataDirectoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HomeUnavailable => {
                formatter.write_str("the operating-system data directory is unavailable")
            }
        }
    }
}

impl Error for DataDirectoryError {}

pub fn codexrs_data_dir() -> Result<PathBuf, DataDirectoryError> {
    if let Some(configured) = env::var_os("CODEX_RS_DATA_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
    {
        return Ok(configured);
    }

    #[cfg(windows)]
    {
        return env::var_os("LOCALAPPDATA")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .map(|path| path.join("codexRS"))
            .ok_or(DataDirectoryError::HomeUnavailable);
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(data_home) = env::var_os("XDG_DATA_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
        {
            return Ok(data_home.join("codexRS"));
        }
        return env::var_os("HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .map(|path| path.join(".local").join("share").join("codexRS"))
            .ok_or(DataDirectoryError::HomeUnavailable);
    }

    #[allow(unreachable_code)]
    Err(DataDirectoryError::HomeUnavailable)
}

/// Resolves the official Codex CLI without introducing a Node runtime
/// dependency. On Windows, the native executable shipped by the official npm
/// package is preferred because packaged WindowsApps binaries may be visible
/// through PATH while denying direct child-process execution.
#[must_use]
pub fn resolve_codex_binary(explicit: Option<PathBuf>) -> PathBuf {
    if let Some(explicit) = explicit.filter(|path| !path.as_os_str().is_empty()) {
        return explicit;
    }
    if let Some(configured) = env::var_os("CODEX_RS_CODEX_BIN")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
    {
        return configured;
    }
    #[cfg(windows)]
    if let Some(app_data) = env::var_os("APPDATA") {
        let candidate = windows_npm_codex_candidate(PathBuf::from(app_data));
        if candidate.is_file() {
            return candidate;
        }
    }
    PathBuf::from(if cfg!(windows) { "codex.exe" } else { "codex" })
}

#[cfg(windows)]
fn windows_npm_codex_candidate(app_data: PathBuf) -> PathBuf {
    let (package, target) = if cfg!(target_arch = "aarch64") {
        ("codex-win32-arm64", "aarch64-pc-windows-msvc")
    } else {
        ("codex-win32-x64", "x86_64-pc-windows-msvc")
    };
    app_data
        .join("npm")
        .join("node_modules")
        .join("@openai")
        .join("codex")
        .join("node_modules")
        .join("@openai")
        .join(package)
        .join("vendor")
        .join(target)
        .join("codex")
        .join("codex.exe")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimePolicy {
    pub git_debounce: Duration,
    pub max_parallel_git_processes: NonZeroUsize,
    pub graceful_shutdown_timeout: Duration,
}

impl Default for RuntimePolicy {
    fn default() -> Self {
        Self {
            git_debounce: Duration::from_millis(300),
            max_parallel_git_processes: NonZeroUsize::MIN,
            graceful_shutdown_timeout: Duration::from_secs(3),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimePolicy;
    #[cfg(windows)]
    use super::windows_npm_codex_candidate;
    #[cfg(windows)]
    use std::path::PathBuf;

    #[test]
    fn default_policy_prevents_parallel_git_storms() {
        assert_eq!(RuntimePolicy::default().max_parallel_git_processes.get(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn npm_codex_candidate_points_to_the_native_vendor_binary() {
        assert_eq!(
            windows_npm_codex_candidate(PathBuf::from(r"C:\Users\dev\AppData\Roaming")),
            PathBuf::from(
                r"C:\Users\dev\AppData\Roaming\npm\node_modules\@openai\codex\node_modules\@openai\codex-win32-x64\vendor\x86_64-pc-windows-msvc\codex\codex.exe"
            )
        );
    }
}
