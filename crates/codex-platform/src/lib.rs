use std::env;
use std::error::Error;
use std::fmt;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::time::Duration;
#[cfg(windows)]
use std::{
    fs,
    fs::File,
    io::{self, Read},
};

#[cfg(windows)]
use sha2::{Digest, Sha256};

mod app_server;
mod artifacts;
mod browser;
mod browser_agent;
#[cfg(windows)]
mod computer_apps;
mod computer_interruption;
mod computer_overlay;
mod computer_use;
mod computer_use_helper;
mod git;
mod github;
mod process;
mod terminal;

pub use app_server::{
    AppServerClient, AppServerConfig, AppServerConnection, AppServerError, AppServerEvent,
    CodexHome, CodexHomeKind, DEFAULT_THREAD_PAGE_LIMIT, MAX_THREAD_PAGE_LIMIT,
};
pub use artifacts::{
    ArtifactError, ArtifactFileKind, ArtifactFilePreview, MAX_ARTIFACT_PATH_BYTES,
    MAX_ARTIFACT_PREVIEW_BYTES, MAX_ARTIFACT_TEXT_BYTES, inspect_artifact, inspect_workspace_file,
    is_supported_artifact_path, open_workspace_path, read_artifact_image, reveal_artifact,
};
pub use browser::{
    BROWSER_EVENT_CAPACITY, BrowserCommandError, BrowserConfig, BrowserDownload,
    BrowserDownloadStatus, BrowserError, BrowserEvent, BrowserKeyInput, BrowserMouseButton,
    BrowserSession, BrowserTab, MAX_BROWSER_CONTEXT_ID_BYTES, MAX_BROWSER_FRAME_BYTES,
    MAX_BROWSER_TABS, MAX_BROWSER_TITLE_BYTES, MAX_BROWSER_URL_BYTES, browser_permission_for_url,
    default_browser_download_dir, normalize_browser_origin, resolve_browser_binary,
};
pub use computer_interruption::{ComputerUseInterruptionMonitor, ComputerUseTurnKey};
pub use computer_overlay::{
    ComputerUseOverlayTarget, ComputerUseSystemOverlay, run_computer_use_overlay_helper,
};
pub use computer_use::{
    ComputerApplication, ComputerButton, ComputerCapture, ComputerKey, ComputerUseError,
    ComputerWindow, MAX_COMPUTER_APPLICATIONS, MAX_COMPUTER_CAPTURE_BYTES, MAX_COMPUTER_TEXT_BYTES,
    MAX_COMPUTER_WINDOWS, capture_computer_window, click_computer_window,
    computer_use_target_is_forbidden, drag_computer_window, inspect_computer_window,
    list_computer_windows, move_over_computer_window, press_computer_key, scroll_computer_window,
    type_into_computer_window,
};
pub use computer_use_helper::{
    ComputerAccessibilityError, ComputerAccessibilityState, ComputerUseAccessibilityClient,
    MAX_COMPUTER_ACCESSIBILITY_ELEMENTS, MAX_COMPUTER_ACCESSIBILITY_TREE_BYTES,
    run_computer_use_helper,
};
pub use git::{
    GitBranch, GitBranchDiff, GitBranchMutationOutcome, GitDiff, GitError, GitFile, GitFileKind,
    GitReviewCommit, GitSnapshot, GitWorktree, MAX_GIT_BRANCH_BYTES, MAX_GIT_BRANCHES,
    MAX_GIT_COMMIT_CONTEXT_BYTES, MAX_GIT_COMMIT_MESSAGE_CHARS, MAX_GIT_CONFLICT_PATHS,
    MAX_GIT_DIFF_BYTES, MAX_GIT_FILES, MAX_GIT_PULL_REQUEST_CONTEXT_BYTES, MAX_GIT_REVIEW_BRANCHES,
    MAX_GIT_REVIEW_COMMITS, MAX_GIT_WORKTREES, MAX_MANAGED_WORKTREE_DIFF_BYTES, ManagedWorktree,
    branch_diff as git_branch_diff, commit as git_commit, commit_diff,
    commit_message_diff as git_commit_message_diff, create_branch, create_managed_worktree,
    create_managed_worktree_cancellable, create_worktree, diff as git_diff,
    pull_request_context as git_pull_request_context, push as git_push, snapshot as git_snapshot,
    stage as git_stage, stage_all as git_stage_all, switch_branch, uncommitted_diff,
    unstage as git_unstage, unstage_all as git_unstage_all,
};
pub use github::{
    GitHubCheckStatus, GitHubCiStatus, GitHubCliAvailability, GitHubCreatePullRequest,
    GitHubCreatedPullRequest, GitHubError, GitHubPullRequest, GitHubPullRequestActivity,
    GitHubPullRequestActivityKind, GitHubPullRequestCheck, GitHubPullRequestDetail,
    GitHubPullRequestDiff, GitHubPullRequestIdentity, GitHubPullRequestLifecycle,
    GitHubPullRequestMergeMethod, GitHubPullRequestRelationship, GitHubPullRequestReviewEvent,
    GitHubPullRequestReviewState, GitHubPullRequestSearchFilters, GitHubPullRequestSearchPage,
    GitHubPullRequestState, GitHubPullRequestStatus, GitHubPullRequestSummary, GitHubUser,
    cli_availability as github_cli_availability, create_pull_request as github_create_pull_request,
    merge_pull_request as github_merge_pull_request,
    post_pull_request_comment as github_post_pull_request_comment,
    pull_request_detail as github_pull_request_detail,
    pull_request_diff as github_pull_request_diff,
    pull_request_status as github_pull_request_status,
    search_pull_requests as github_search_pull_requests,
    set_pull_request_review_state as github_set_pull_request_review_state,
    submit_pull_request_review as github_submit_pull_request_review,
    update_pull_request_body as github_update_pull_request_body,
    update_pull_request_title as github_update_pull_request_title,
};
pub use process::ProcessError;
pub use terminal::{
    MAX_TERMINAL_INPUT_BYTES, TERMINAL_EVENT_CAPACITY, TerminalCommandError, TerminalConfig,
    TerminalError, TerminalEvent, TerminalSession, available_terminal_shells,
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

pub const MAX_DESKTOP_WORK_AREAS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DesktopWorkArea {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl DesktopWorkArea {
    #[cfg(any(windows, test))]
    fn new(x: f32, y: f32, width: f32, height: f32) -> Option<Self> {
        (x.is_finite()
            && y.is_finite()
            && width.is_finite()
            && height.is_finite()
            && width > 0.0
            && height > 0.0)
            .then_some(Self {
                x,
                y,
                width,
                height,
            })
    }

    #[must_use]
    pub fn intersects(self, x: f32, y: f32, width: f32, height: f32) -> bool {
        self.x < x + width
            && self.x + self.width > x
            && self.y < y + height
            && self.y + self.height > y
    }
}

#[cfg(windows)]
#[must_use]
pub fn desktop_work_areas() -> Vec<DesktopWorkArea> {
    use winsafe::{self as w, prelude::*};

    let mut work_areas = Vec::with_capacity(4);
    let _ = w::HDC::NULL.EnumDisplayMonitors(None, |monitor, _, _| {
        if work_areas.len() >= MAX_DESKTOP_WORK_AREAS {
            return false;
        }
        let mut info = w::MONITORINFOEX::default();
        if monitor.GetMonitorInfo(&mut info).is_ok() {
            let scale_factor = xcap::Monitor::from_point(
                info.rcMonitor.left.saturating_add(1),
                info.rcMonitor.top.saturating_add(1),
            )
            .and_then(|monitor| monitor.scale_factor())
            .ok()
            .filter(|scale| scale.is_finite() && *scale > 0.0)
            .unwrap_or(1.0);
            let width = info.rcWork.right.saturating_sub(info.rcWork.left) as f32 / scale_factor;
            let height = info.rcWork.bottom.saturating_sub(info.rcWork.top) as f32 / scale_factor;
            if let Some(work_area) = DesktopWorkArea::new(
                info.rcWork.left as f32 / scale_factor,
                info.rcWork.top as f32 / scale_factor,
                width,
                height,
            ) {
                work_areas.push(work_area);
            }
        }
        true
    });
    work_areas
}

#[cfg(not(windows))]
#[must_use]
pub fn desktop_work_areas() -> Vec<DesktopWorkArea> {
    Vec::new()
}

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
/// package is preferred. When the exact stable Desktop package is installed,
/// its signed, hash-pinned CLI is copied into the codexRS-owned runtime cache
/// because WindowsApps denies direct child-process execution.
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
    if let Some(candidate) = windows_stable_codex_cache_candidate() {
        return candidate;
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
fn windows_stable_codex_cache_candidate() -> Option<PathBuf> {
    let reference = codex_core::stable_reference();
    let destination = codexrs_data_dir()
        .ok()?
        .join("runtime")
        .join(format!("codex-{}.exe", reference.cli_version));
    if sha256_matches(&destination, reference.cli_sha256).unwrap_or(false) {
        return Some(destination);
    }

    let program_files = env::var_os("ProgramW6432")
        .or_else(|| env::var_os("ProgramFiles"))
        .map(PathBuf::from)?;
    let source = windows_packaged_codex_candidate(program_files);
    if !source.is_file() || !sha256_matches(&source, reference.cli_sha256).ok()? {
        return None;
    }

    fs::create_dir_all(destination.parent()?).ok()?;
    fs::copy(&source, &destination).ok()?;
    sha256_matches(&destination, reference.cli_sha256)
        .ok()
        .filter(|matches| *matches)
        .map(|_| destination)
}

#[cfg(windows)]
fn windows_packaged_codex_candidate(program_files: PathBuf) -> PathBuf {
    let reference = codex_core::stable_reference();
    program_files
        .join("WindowsApps")
        .join(format!(
            "{}_{}_{}__2p2nqsd0c76g0",
            reference.package_name, reference.package_version, reference.architecture
        ))
        .join("app")
        .join("resources")
        .join("codex.exe")
}

#[cfg(windows)]
fn sha256_matches(path: &std::path::Path, expected: &str) -> io::Result<bool> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let actual = hasher.finalize();
    Ok(actual
        .iter()
        .zip(expected.as_bytes().chunks_exact(2))
        .all(|(actual, expected)| {
            u8::from_str_radix(std::str::from_utf8(expected).unwrap_or_default(), 16)
                .is_ok_and(|expected| *actual == expected)
        })
        && expected.len() == actual.len() * 2)
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
    use super::{DesktopWorkArea, RuntimePolicy};
    #[cfg(windows)]
    use super::{sha256_matches, windows_npm_codex_candidate, windows_packaged_codex_candidate};
    #[cfg(windows)]
    use std::fs;
    #[cfg(windows)]
    use std::path::PathBuf;

    #[test]
    fn default_policy_prevents_parallel_git_storms() {
        assert_eq!(RuntimePolicy::default().max_parallel_git_processes.get(), 1);
    }

    #[test]
    fn desktop_work_area_uses_strict_rectangle_intersection() {
        let Some(work_area) = DesktopWorkArea::new(0.0, 0.0, 1_920.0, 1_040.0) else {
            panic!("valid work area must construct");
        };

        assert!(work_area.intersects(1_800.0, 900.0, 480.0, 600.0));
        assert!(!work_area.intersects(1_920.0, 0.0, 480.0, 600.0));
        assert!(!work_area.intersects(0.0, 1_040.0, 480.0, 600.0));
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

    #[cfg(windows)]
    #[test]
    fn packaged_codex_candidate_points_to_the_pinned_stable_cli() {
        assert_eq!(
            windows_packaged_codex_candidate(PathBuf::from(r"C:\Program Files")),
            PathBuf::from(
                r"C:\Program Files\WindowsApps\OpenAI.Codex_26.721.3996.0_x64__2p2nqsd0c76g0\app\resources\codex.exe"
            )
        );
    }

    #[cfg(windows)]
    #[test]
    fn sha256_verification_is_streamed_and_exact() -> Result<(), Box<dyn std::error::Error>> {
        let fixture =
            std::env::temp_dir().join(format!("codexrs-sha256-{}.txt", std::process::id()));
        fs::write(&fixture, b"codexRS")?;
        let matches = sha256_matches(
            &fixture,
            "0d844d17ac96b938796777831e5ec4703184f81a2acbddf851addd2c5d7fb8d7",
        )?;
        let _ = fs::remove_file(&fixture);
        assert!(matches);
        Ok(())
    }
}
