use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

#[cfg(windows)]
use crate::computer_apps::{
    ComputerLaunchTarget, discover_windows_computer_apps, explicit_executable_launch_target,
};
#[cfg(windows)]
use crate::inspect_computer_window;
use crate::{
    ComputerApplication, ComputerButton, ComputerCapture, MAX_COMPUTER_APPLICATIONS,
    MAX_COMPUTER_WINDOWS, computer_use_target_is_forbidden, list_computer_windows,
};

pub const MAX_COMPUTER_ACCESSIBILITY_ELEMENTS: usize = 512;
pub const MAX_COMPUTER_ACCESSIBILITY_TREE_BYTES: usize = 128 * 1024;

#[cfg(windows)]
const MAX_HELPER_REQUEST_BYTES: usize = 64 * 1024;
#[cfg(windows)]
const MAX_HELPER_RESPONSE_BYTES: usize = 512 * 1024;
const MAX_ELEMENT_TEXT_BYTES: usize = 512;
const MAX_DOCUMENT_TEXT_BYTES: usize = 16 * 1024;
#[cfg(windows)]
const MAX_SELECTED_TEXT_BYTES: usize = 8 * 1024;
#[cfg(windows)]
const MAX_SELECTED_ELEMENTS: usize = 32;
#[cfg(any(windows, test))]
const MAX_BROWSER_URL_BYTES: usize = 8 * 1024;
const MAX_STALE_INPUT_WINDOWS: usize = 64;
#[cfg(windows)]
const MAX_TREE_DEPTH: usize = 32;
#[cfg(windows)]
const MAX_CHILDREN_PER_ELEMENT: usize = 128;
#[cfg(windows)]
const MAX_NATIVE_WINDOW_ENUMERATION: usize = 4_096;
#[cfg(windows)]
const HELPER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputerAccessibilityState {
    pub tree: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focused_element: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_elements: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_text: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComputerAccessibilityError {
    Unsupported,
    InvalidInput,
    Spawn,
    Supervision,
    Pipe,
    TimedOut,
    Protocol,
    WindowUnavailable,
    StaleElement,
    StaleScreenshot,
    BrowserUrlUnavailable,
    AppNotFound,
    Catalog,
    Launch,
    ForbiddenTarget,
    ActionRejected,
}

impl fmt::Display for ComputerAccessibilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Unsupported => {
                "accessibility-backed Computer Use is not supported on this platform"
            }
            Self::InvalidInput => "the accessibility request is invalid or exceeds its limit",
            Self::Spawn => "could not start the supervised Computer Use helper",
            Self::Supervision => "could not supervise the Computer Use helper",
            Self::Pipe => "the Computer Use helper disconnected",
            Self::TimedOut => "the Computer Use helper timed out and was stopped",
            Self::Protocol => "the Computer Use helper returned an invalid bounded response",
            Self::WindowUnavailable => "the selected window is unavailable to accessibility APIs",
            Self::StaleElement => {
                "the accessibility element index is stale; call get_window_state with include_text first"
            }
            Self::StaleScreenshot => {
                "the screenshot id is stale; call get_window_state with include_screenshot first"
            }
            Self::BrowserUrlUnavailable => {
                "the current browser URL could not be determined with enough confidence"
            }
            Self::AppNotFound => {
                "the app id is not installed or is not a launchable executable identifier"
            }
            Self::Catalog => "could not read the bounded installed-app catalog",
            Self::Launch => "Windows rejected the app launch request",
            Self::ForbiddenTarget => {
                "Computer Use product policy blocks the selected application"
            }
            Self::ActionRejected => "the selected accessibility element rejected the action",
        })
    }
}

impl Error for ComputerAccessibilityError {}

#[derive(Debug, Serialize, Deserialize)]
struct HelperRequest {
    id: u64,
    method: String,
    #[serde(default)]
    window_id: Option<String>,
    #[serde(default)]
    element_index: Option<usize>,
    #[serde(default)]
    mouse_button: Option<ComputerButton>,
    #[serde(default)]
    click_count: Option<u8>,
    #[serde(default)]
    value: Option<String>,
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    app_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct HelperResponse {
    id: u64,
    ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    state: Option<ComputerAccessibilityState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    applications: Option<Vec<ComputerApplication>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    browser_url: Option<String>,
    #[cfg(windows)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    launch_target: Option<ComputerLaunchTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<HelperErrorCode>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum HelperErrorCode {
    InvalidInput,
    WindowUnavailable,
    StaleElement,
    BrowserUrlUnavailable,
    AppNotFound,
    ForbiddenTarget,
    ActionRejected,
    Protocol,
}

impl From<HelperErrorCode> for ComputerAccessibilityError {
    fn from(value: HelperErrorCode) -> Self {
        match value {
            HelperErrorCode::InvalidInput => Self::InvalidInput,
            HelperErrorCode::WindowUnavailable => Self::WindowUnavailable,
            HelperErrorCode::StaleElement => Self::StaleElement,
            HelperErrorCode::BrowserUrlUnavailable => Self::BrowserUrlUnavailable,
            HelperErrorCode::AppNotFound => Self::AppNotFound,
            HelperErrorCode::ForbiddenTarget => Self::ForbiddenTarget,
            HelperErrorCode::ActionRejected => Self::ActionRejected,
            HelperErrorCode::Protocol => Self::Protocol,
        }
    }
}

pub struct ComputerUseAccessibilityClient {
    next_request_id: u64,
    latest_capture: Option<CapturedGeometry>,
    stale_input_windows: std::collections::HashSet<String>,
    all_input_windows_stale: bool,
    #[cfg(windows)]
    process: Option<WindowsHelperProcess>,
}

impl Default for ComputerUseAccessibilityClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ComputerUseAccessibilityClient {
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_request_id: 1,
            latest_capture: None,
            stale_input_windows: std::collections::HashSet::new(),
            all_input_windows_stale: false,
            #[cfg(windows)]
            process: None,
        }
    }

    pub fn remember_capture(&mut self, capture: &ComputerCapture) {
        self.clear_user_input(&capture.window.id);
        self.latest_capture = Some(CapturedGeometry {
            window_id: capture.window.id.clone(),
            screenshot_id: capture.screenshot_id.clone(),
            capture_width: capture.width,
            capture_height: capture.height,
            window_width: capture.window.width,
            window_height: capture.window.height,
        });
    }

    pub fn mark_user_input(&mut self, window_id: &str) {
        self.latest_capture = self
            .latest_capture
            .take()
            .filter(|capture| capture.window_id != window_id);
        if self.all_input_windows_stale {
            self.stale_input_windows.remove(window_id);
            return;
        }
        if self.stale_input_windows.contains(window_id) {
            return;
        }
        if self.stale_input_windows.len() >= MAX_STALE_INPUT_WINDOWS {
            self.stale_input_windows.clear();
            self.all_input_windows_stale = true;
            return;
        }
        self.stale_input_windows.insert(window_id.to_owned());
    }

    #[must_use]
    pub fn user_input_requires_refresh(&self, window_id: &str) -> bool {
        if self.all_input_windows_stale {
            !self.stale_input_windows.contains(window_id)
        } else {
            self.stale_input_windows.contains(window_id)
        }
    }

    fn clear_user_input(&mut self, window_id: &str) {
        if self.all_input_windows_stale {
            if self.stale_input_windows.len() >= MAX_STALE_INPUT_WINDOWS {
                self.stale_input_windows.clear();
            }
            self.stale_input_windows.insert(window_id.to_owned());
        } else {
            self.stale_input_windows.remove(window_id);
        }
    }

    pub fn map_screenshot_point(
        &self,
        window_id: &str,
        screenshot_id: &str,
        x: i32,
        y: i32,
    ) -> Result<(i32, i32), ComputerAccessibilityError> {
        let capture = self
            .latest_capture
            .as_ref()
            .filter(|capture| {
                capture.window_id == window_id && capture.screenshot_id == screenshot_id
            })
            .ok_or(ComputerAccessibilityError::StaleScreenshot)?;
        if x < 0
            || y < 0
            || u32::try_from(x)
                .ok()
                .is_none_or(|x| x >= capture.capture_width)
            || u32::try_from(y)
                .ok()
                .is_none_or(|y| y >= capture.capture_height)
            || capture.capture_width == 0
            || capture.capture_height == 0
            || capture.window_width == 0
            || capture.window_height == 0
        {
            return Err(ComputerAccessibilityError::InvalidInput);
        }
        let mapped_x = i64::from(x).saturating_mul(i64::from(capture.window_width))
            / i64::from(capture.capture_width);
        let mapped_y = i64::from(y).saturating_mul(i64::from(capture.window_height))
            / i64::from(capture.capture_height);
        Ok((
            i32::try_from(mapped_x.min(i64::from(capture.window_width - 1)))
                .map_err(|_| ComputerAccessibilityError::InvalidInput)?,
            i32::try_from(mapped_y.min(i64::from(capture.window_height - 1)))
                .map_err(|_| ComputerAccessibilityError::InvalidInput)?,
        ))
    }

    pub fn get_state(
        &mut self,
        window_id: &str,
    ) -> Result<ComputerAccessibilityState, ComputerAccessibilityError> {
        let response = self.request(
            "get_state",
            Some(window_id),
            None,
            None,
            None,
            None,
            None,
            None,
        )?;
        let state = response.state.ok_or(ComputerAccessibilityError::Protocol)?;
        self.clear_user_input(window_id);
        Ok(state)
    }

    pub fn browser_url(&mut self, window_id: &str) -> Result<String, ComputerAccessibilityError> {
        let response = self.request(
            "get_browser_url",
            Some(window_id),
            None,
            None,
            None,
            None,
            None,
            None,
        )?;
        response
            .browser_url
            .ok_or(ComputerAccessibilityError::Protocol)
    }

    pub fn activate_window(&mut self, window_id: &str) -> Result<(), ComputerAccessibilityError> {
        self.request(
            "activate_window",
            Some(window_id),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .map(|_| ())
    }

    pub fn click_element(
        &mut self,
        window_id: &str,
        element_index: usize,
        mouse_button: ComputerButton,
        click_count: u8,
    ) -> Result<(), ComputerAccessibilityError> {
        if click_count == 0 || click_count > 3 {
            return Err(ComputerAccessibilityError::InvalidInput);
        }
        self.request(
            "click_element",
            Some(window_id),
            Some(element_index),
            Some(mouse_button),
            Some(click_count),
            None,
            None,
            None,
        )
        .map(|_| ())
    }

    pub fn set_value(
        &mut self,
        window_id: &str,
        element_index: usize,
        value: &str,
    ) -> Result<(), ComputerAccessibilityError> {
        if value.len() > MAX_DOCUMENT_TEXT_BYTES {
            return Err(ComputerAccessibilityError::InvalidInput);
        }
        self.request(
            "set_value",
            Some(window_id),
            Some(element_index),
            None,
            None,
            Some(value),
            None,
            None,
        )
        .map(|_| ())
    }

    pub fn perform_secondary_action(
        &mut self,
        window_id: &str,
        element_index: usize,
        action: &str,
    ) -> Result<(), ComputerAccessibilityError> {
        if action.trim().is_empty() || action.len() > MAX_ELEMENT_TEXT_BYTES {
            return Err(ComputerAccessibilityError::InvalidInput);
        }
        self.request(
            "perform_secondary_action",
            Some(window_id),
            Some(element_index),
            None,
            None,
            None,
            Some(action),
            None,
        )
        .map(|_| ())
    }

    pub fn list_apps(&mut self) -> Result<Vec<ComputerApplication>, ComputerAccessibilityError> {
        #[cfg(windows)]
        let applications = self
            .request("list_apps", None, None, None, None, None, None, None)?
            .applications
            .ok_or(ComputerAccessibilityError::Protocol)?;

        #[cfg(not(windows))]
        let applications = Vec::new();

        merge_running_windows(applications)
    }

    pub fn launch_app(&mut self, app_id: &str) -> Result<(), ComputerAccessibilityError> {
        if computer_use_target_is_forbidden(app_id, "") {
            return Err(ComputerAccessibilityError::ForbiddenTarget);
        }
        #[cfg(windows)]
        {
            let target = self.resolve_launch_target(app_id)?;
            launch_windows_target(target)
        }

        #[cfg(not(windows))]
        {
            let _ = app_id;
            Err(ComputerAccessibilityError::Unsupported)
        }
    }

    pub fn validate_app_launch(&mut self, app_id: &str) -> Result<(), ComputerAccessibilityError> {
        if computer_use_target_is_forbidden(app_id, "") {
            return Err(ComputerAccessibilityError::ForbiddenTarget);
        }
        #[cfg(windows)]
        {
            self.resolve_launch_target(app_id).map(|_| ())
        }

        #[cfg(not(windows))]
        {
            let _ = app_id;
            Err(ComputerAccessibilityError::Unsupported)
        }
    }

    #[cfg(windows)]
    fn resolve_launch_target(
        &mut self,
        app_id: &str,
    ) -> Result<ComputerLaunchTarget, ComputerAccessibilityError> {
        if app_id.trim().is_empty() || app_id.len() > 32 * 1024 {
            return Err(ComputerAccessibilityError::InvalidInput);
        }
        self.request(
            "resolve_launch",
            None,
            None,
            None,
            None,
            None,
            None,
            Some(app_id),
        )?
        .launch_target
        .ok_or(ComputerAccessibilityError::Protocol)
    }

    #[allow(clippy::too_many_arguments)]
    fn request(
        &mut self,
        method: &str,
        window_id: Option<&str>,
        element_index: Option<usize>,
        mouse_button: Option<ComputerButton>,
        click_count: Option<u8>,
        value: Option<&str>,
        action: Option<&str>,
        app_id: Option<&str>,
    ) -> Result<HelperResponse, ComputerAccessibilityError> {
        if window_id.is_some_and(|id| id.is_empty() || id.len() > MAX_ELEMENT_TEXT_BYTES) {
            return Err(ComputerAccessibilityError::InvalidInput);
        }
        if app_id.is_some_and(|id| id.trim().is_empty() || id.len() > 32 * 1024) {
            return Err(ComputerAccessibilityError::InvalidInput);
        }
        let id = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1).max(1);
        let request = HelperRequest {
            id,
            method: method.to_owned(),
            window_id: window_id.map(str::to_owned),
            element_index,
            mouse_button,
            click_count,
            value: value.map(str::to_owned),
            action: action.map(str::to_owned),
            app_id: app_id.map(str::to_owned),
        };

        #[cfg(windows)]
        {
            if self.process.is_none() {
                self.process = Some(WindowsHelperProcess::spawn()?);
            }
            let response = self
                .process
                .as_mut()
                .ok_or(ComputerAccessibilityError::Spawn)?
                .exchange(&request);
            let response = match response {
                Ok(response) => response,
                Err(error) => {
                    self.process.take();
                    return Err(error);
                }
            };
            if response.id != id {
                self.process.take();
                return Err(ComputerAccessibilityError::Protocol);
            }
            if response.ok {
                return Ok(response);
            }
            Err(response
                .error
                .map(ComputerAccessibilityError::from)
                .unwrap_or(ComputerAccessibilityError::Protocol))
        }

        #[cfg(not(windows))]
        {
            let _ = request;
            Err(ComputerAccessibilityError::Unsupported)
        }
    }
}

fn merge_running_windows(
    applications: Vec<ComputerApplication>,
) -> Result<Vec<ComputerApplication>, ComputerAccessibilityError> {
    let windows = list_computer_windows().map_err(|_| ComputerAccessibilityError::Catalog)?;
    Ok(merge_running_windows_with(applications, windows))
}

fn merge_running_windows_with(
    applications: Vec<ComputerApplication>,
    windows: Vec<crate::ComputerWindow>,
) -> Vec<ComputerApplication> {
    let mut installed = std::collections::HashMap::<String, ComputerApplication>::new();
    let mut installed_order = Vec::with_capacity(applications.len().min(MAX_COMPUTER_APPLICATIONS));
    for mut application in applications.into_iter().take(MAX_COMPUTER_APPLICATIONS) {
        application.id = application.id.trim().to_owned();
        let application_key = application.id.to_lowercase();
        if application_key.is_empty() || application.id.len() > MAX_ELEMENT_TEXT_BYTES {
            continue;
        }
        application.windows.clear();
        application.is_running = false;
        installed.entry(application_key.clone()).or_insert_with(|| {
            installed_order.push(application_key);
            application
        });
    }
    let mut merged = Vec::<ComputerApplication>::with_capacity(MAX_COMPUTER_APPLICATIONS);
    let mut running_indexes = std::collections::HashMap::<String, usize>::new();
    for window in windows.into_iter().take(MAX_COMPUTER_WINDOWS) {
        let application_id = window.application_id.trim().to_owned();
        let application_key = application_id.to_lowercase();
        if application_key.is_empty() || application_id.len() > MAX_ELEMENT_TEXT_BYTES {
            continue;
        }
        let application_index = if let Some(index) = running_indexes.get(&application_key) {
            *index
        } else {
            if merged.len() >= MAX_COMPUTER_APPLICATIONS {
                continue;
            }
            let mut application =
                installed
                    .remove(&application_key)
                    .unwrap_or_else(|| ComputerApplication {
                        id: application_id,
                        display_name: (!window.application.trim().is_empty())
                            .then(|| window.application.clone()),
                        last_used_date: None,
                        use_count: None,
                        is_running: true,
                        windows: Vec::new(),
                    });
            if application.display_name.is_none() && !window.application.trim().is_empty() {
                application.display_name = Some(window.application.clone());
            }
            application.is_running = true;
            application.windows.clear();
            let index = merged.len();
            merged.push(application);
            running_indexes.insert(application_key, index);
            index
        };
        let application = &mut merged[application_index];
        application.is_running = true;
        if application.windows.len() < MAX_COMPUTER_WINDOWS {
            application.windows.push(window);
        }
    }
    for application_key in installed_order {
        if merged.len() >= MAX_COMPUTER_APPLICATIONS {
            break;
        }
        if let Some(application) = installed.remove(&application_key) {
            merged.push(application);
        }
    }
    merged
}

#[cfg(windows)]
fn launch_windows_target(target: ComputerLaunchTarget) -> Result<(), ComputerAccessibilityError> {
    use std::ffi::OsString;
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let system_root = std::env::var_os("SystemRoot")
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or(ComputerAccessibilityError::Launch)?;
    let explorer = system_root.join("explorer.exe");
    if !explorer.is_file() {
        return Err(ComputerAccessibilityError::Launch);
    }
    let argument = match target {
        ComputerLaunchTarget::Aumid(application_id) => {
            OsString::from(format!(r"shell:AppsFolder\{application_id}"))
        }
        ComputerLaunchTarget::Shortcut(path) | ComputerLaunchTarget::Executable(path) => {
            if !path.is_absolute() {
                return Err(ComputerAccessibilityError::Launch);
            }
            path.into_os_string()
        }
    };
    Command::new(explorer)
        .arg(argument)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
        .map_err(|_| ComputerAccessibilityError::Launch)
}

struct CapturedGeometry {
    window_id: String,
    screenshot_id: String,
    capture_width: u32,
    capture_height: u32,
    window_width: u32,
    window_height: u32,
}

#[cfg(windows)]
struct WindowsHelperProcess {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    responses: crossbeam_channel::Receiver<Result<Vec<u8>, ComputerAccessibilityError>>,
    reader: Option<std::thread::JoinHandle<()>>,
    _job: win32job::Job,
}

#[cfg(windows)]
impl WindowsHelperProcess {
    fn spawn() -> Result<Self, ComputerAccessibilityError> {
        use std::os::windows::io::AsRawHandle;
        use std::os::windows::process::CommandExt;
        use std::process::{Command, Stdio};
        use win32job::{ExtendedLimitInfo, Job};

        const CREATE_NO_WINDOW: u32 = 0x0800_0000;

        let executable = std::env::current_exe().map_err(|_| ComputerAccessibilityError::Spawn)?;
        let mut command = Command::new(executable);
        command
            .arg("--computer-use-helper")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW);

        let mut limits = ExtendedLimitInfo::new();
        limits.limit_kill_on_job_close();
        let job = Job::create_with_limit_info(&limits)
            .map_err(|_| ComputerAccessibilityError::Supervision)?;
        let mut child = command
            .spawn()
            .map_err(|_| ComputerAccessibilityError::Spawn)?;
        if job.assign_process(child.as_raw_handle() as isize).is_err() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(ComputerAccessibilityError::Supervision);
        }
        let stdin = child.stdin.take().ok_or(ComputerAccessibilityError::Pipe)?;
        let stdout = child
            .stdout
            .take()
            .ok_or(ComputerAccessibilityError::Pipe)?;
        let (sender, responses) = crossbeam_channel::bounded(2);
        let reader = std::thread::Builder::new()
            .name("codex-rs-computer-use-helper".to_owned())
            .spawn(move || helper_response_reader(stdout, &sender))
            .map_err(|_| ComputerAccessibilityError::Spawn)?;

        Ok(Self {
            child,
            stdin,
            responses,
            reader: Some(reader),
            _job: job,
        })
    }

    fn exchange(
        &mut self,
        request: &HelperRequest,
    ) -> Result<HelperResponse, ComputerAccessibilityError> {
        use std::io::Write;

        let encoded =
            serde_json::to_vec(request).map_err(|_| ComputerAccessibilityError::Protocol)?;
        if encoded.len() > MAX_HELPER_REQUEST_BYTES {
            return Err(ComputerAccessibilityError::InvalidInput);
        }
        self.stdin
            .write_all(&encoded)
            .and_then(|()| self.stdin.write_all(b"\n"))
            .and_then(|()| self.stdin.flush())
            .map_err(|_| ComputerAccessibilityError::Pipe)?;
        let response =
            self.responses
                .recv_timeout(HELPER_TIMEOUT)
                .map_err(|error| match error {
                    crossbeam_channel::RecvTimeoutError::Timeout => {
                        ComputerAccessibilityError::TimedOut
                    }
                    crossbeam_channel::RecvTimeoutError::Disconnected => {
                        ComputerAccessibilityError::Pipe
                    }
                })??;
        serde_json::from_slice(&response).map_err(|_| ComputerAccessibilityError::Protocol)
    }
}

#[cfg(windows)]
impl Drop for WindowsHelperProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

#[cfg(windows)]
fn helper_response_reader(
    stdout: std::process::ChildStdout,
    sender: &crossbeam_channel::Sender<Result<Vec<u8>, ComputerAccessibilityError>>,
) {
    let mut reader = std::io::BufReader::new(stdout);
    loop {
        match read_bounded_line(&mut reader, MAX_HELPER_RESPONSE_BYTES) {
            Ok(Some(line)) => {
                if sender.send(Ok(line)).is_err() {
                    return;
                }
            }
            Ok(None) => {
                let _ = sender.send(Err(ComputerAccessibilityError::Pipe));
                return;
            }
            Err(_) => {
                let _ = sender.send(Err(ComputerAccessibilityError::Protocol));
                return;
            }
        }
    }
}

#[cfg(any(windows, test))]
fn read_bounded_line(
    reader: &mut impl std::io::BufRead,
    limit: usize,
) -> std::io::Result<Option<Vec<u8>>> {
    let mut line = Vec::with_capacity(limit.min(8 * 1024));
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return if line.is_empty() {
                Ok(None)
            } else {
                Ok(Some(line))
            };
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let take = newline.map_or(available.len(), |index| index + 1);
        if line.len().saturating_add(take) > limit.saturating_add(1) {
            reader.consume(take);
            while newline.is_none() {
                let available = reader.fill_buf()?;
                if available.is_empty() {
                    break;
                }
                let next_newline = available.iter().position(|byte| *byte == b'\n');
                let drain = next_newline.map_or(available.len(), |index| index + 1);
                reader.consume(drain);
                if next_newline.is_some() {
                    break;
                }
            }
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "bounded Computer Use helper frame exceeded",
            ));
        }
        line.extend_from_slice(&available[..take]);
        reader.consume(take);
        if newline.is_some() {
            if line.last() == Some(&b'\n') {
                line.pop();
            }
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            return Ok(Some(line));
        }
    }
}

pub fn run_computer_use_helper() -> Result<(), ComputerAccessibilityError> {
    #[cfg(windows)]
    {
        windows_helper_main()
    }

    #[cfg(not(windows))]
    {
        Err(ComputerAccessibilityError::Unsupported)
    }
}

#[cfg(windows)]
fn windows_helper_main() -> Result<(), ComputerAccessibilityError> {
    use std::io::Write;

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();
    let mut session = WindowsAccessibilitySession::new()?;

    while let Some(line) = read_bounded_line(&mut reader, MAX_HELPER_REQUEST_BYTES)
        .map_err(|_| ComputerAccessibilityError::Protocol)?
    {
        let request = serde_json::from_slice::<HelperRequest>(&line);
        let response = match request {
            Ok(request) => session.handle(request),
            Err(_) => HelperResponse {
                id: 0,
                ok: false,
                state: None,
                applications: None,
                browser_url: None,
                launch_target: None,
                error: Some(HelperErrorCode::Protocol),
            },
        };
        let encoded =
            serde_json::to_vec(&response).map_err(|_| ComputerAccessibilityError::Protocol)?;
        if encoded.len() > MAX_HELPER_RESPONSE_BYTES {
            return Err(ComputerAccessibilityError::Protocol);
        }
        writer
            .write_all(&encoded)
            .and_then(|()| writer.write_all(b"\n"))
            .and_then(|()| writer.flush())
            .map_err(|_| ComputerAccessibilityError::Pipe)?;
    }
    Ok(())
}

#[cfg(windows)]
struct WindowsAccessibilitySession {
    automation: uiautomation::UIAutomation,
    cached_window_id: Option<String>,
    elements: Vec<uiautomation::UIElement>,
    application_launch_targets: std::collections::HashMap<String, ComputerLaunchTarget>,
}

#[cfg(windows)]
enum HelperPayload {
    Empty,
    State(ComputerAccessibilityState),
    Applications(Vec<ComputerApplication>),
    BrowserUrl(String),
    LaunchTarget(ComputerLaunchTarget),
}

#[cfg(windows)]
impl WindowsAccessibilitySession {
    fn new() -> Result<Self, ComputerAccessibilityError> {
        Ok(Self {
            automation: uiautomation::UIAutomation::new()
                .map_err(|_| ComputerAccessibilityError::WindowUnavailable)?,
            cached_window_id: None,
            elements: Vec::with_capacity(MAX_COMPUTER_ACCESSIBILITY_ELEMENTS),
            application_launch_targets: std::collections::HashMap::new(),
        })
    }

    fn handle(&mut self, request: HelperRequest) -> HelperResponse {
        let id = request.id;
        match self.handle_inner(request) {
            Ok(payload) => {
                let (state, applications, browser_url, launch_target) = match payload {
                    HelperPayload::Empty => (None, None, None, None),
                    HelperPayload::State(state) => (Some(state), None, None, None),
                    HelperPayload::Applications(applications) => {
                        (None, Some(applications), None, None)
                    }
                    HelperPayload::BrowserUrl(browser_url) => (None, None, Some(browser_url), None),
                    HelperPayload::LaunchTarget(target) => (None, None, None, Some(target)),
                };
                HelperResponse {
                    id,
                    ok: true,
                    state,
                    applications,
                    browser_url,
                    launch_target,
                    error: None,
                }
            }
            Err(error) => HelperResponse {
                id,
                ok: false,
                state: None,
                applications: None,
                browser_url: None,
                launch_target: None,
                error: Some(error),
            },
        }
    }

    fn handle_inner(&mut self, request: HelperRequest) -> Result<HelperPayload, HelperErrorCode> {
        match request.method.as_str() {
            "list_apps" => {
                let catalog = discover_windows_computer_apps();
                self.application_launch_targets = catalog.launch_targets;
                Ok(HelperPayload::Applications(catalog.applications))
            }
            "resolve_launch" => {
                let app_id = request
                    .app_id
                    .as_deref()
                    .ok_or(HelperErrorCode::InvalidInput)?;
                self.resolve_launch_target(app_id)
                    .map(HelperPayload::LaunchTarget)
            }
            "get_state" => self
                .get_state(
                    request
                        .window_id
                        .as_deref()
                        .ok_or(HelperErrorCode::InvalidInput)?,
                )
                .map(HelperPayload::State),
            "get_browser_url" => self
                .browser_url(
                    request
                        .window_id
                        .as_deref()
                        .ok_or(HelperErrorCode::InvalidInput)?,
                )
                .map(HelperPayload::BrowserUrl),
            "activate_window" => {
                self.activate_window(
                    request
                        .window_id
                        .as_deref()
                        .ok_or(HelperErrorCode::InvalidInput)?,
                )?;
                Ok(HelperPayload::Empty)
            }
            "click_element" => {
                let window_id = request
                    .window_id
                    .as_deref()
                    .ok_or(HelperErrorCode::InvalidInput)?;
                self.click_element(
                    window_id,
                    request.element_index.ok_or(HelperErrorCode::InvalidInput)?,
                    request.mouse_button.unwrap_or(ComputerButton::Left),
                    request.click_count.unwrap_or(1),
                )?;
                Ok(HelperPayload::Empty)
            }
            "set_value" => {
                let window_id = request
                    .window_id
                    .as_deref()
                    .ok_or(HelperErrorCode::InvalidInput)?;
                self.set_value(
                    window_id,
                    request.element_index.ok_or(HelperErrorCode::InvalidInput)?,
                    request
                        .value
                        .as_deref()
                        .ok_or(HelperErrorCode::InvalidInput)?,
                )?;
                Ok(HelperPayload::Empty)
            }
            "perform_secondary_action" => {
                let window_id = request
                    .window_id
                    .as_deref()
                    .ok_or(HelperErrorCode::InvalidInput)?;
                self.perform_secondary_action(
                    window_id,
                    request.element_index.ok_or(HelperErrorCode::InvalidInput)?,
                    request
                        .action
                        .as_deref()
                        .ok_or(HelperErrorCode::InvalidInput)?,
                )?;
                Ok(HelperPayload::Empty)
            }
            _ => Err(HelperErrorCode::InvalidInput),
        }
    }

    fn resolve_launch_target(
        &mut self,
        app_id: &str,
    ) -> Result<ComputerLaunchTarget, HelperErrorCode> {
        let normalized = app_id.trim().to_lowercase();
        if let Some(target) = self.application_launch_targets.get(&normalized) {
            return Ok(target.clone());
        }
        if let Some(target) = explicit_executable_launch_target(app_id) {
            return Ok(target);
        }
        let catalog = discover_windows_computer_apps();
        self.application_launch_targets = catalog.launch_targets;
        self.application_launch_targets
            .get(&normalized)
            .cloned()
            .ok_or(HelperErrorCode::AppNotFound)
    }

    fn get_state(
        &mut self,
        window_id: &str,
    ) -> Result<ComputerAccessibilityState, HelperErrorCode> {
        use uiautomation::types::Handle;

        self.ensure_window_allowed(window_id)?;
        self.cached_window_id = None;
        self.elements.clear();
        let raw_window_id = window_id
            .parse::<u32>()
            .map_err(|_| HelperErrorCode::InvalidInput)?;
        let handle_value =
            isize::try_from(raw_window_id).map_err(|_| HelperErrorCode::InvalidInput)?;
        let root = self
            .automation
            .element_from_handle(Handle::from(handle_value))
            .map_err(|_| HelperErrorCode::WindowUnavailable)?;
        let walker = self
            .automation
            .get_control_view_walker()
            .map_err(|_| HelperErrorCode::WindowUnavailable)?;
        let mut stack = vec![(root, 0_usize)];
        let mut tree = String::with_capacity(16 * 1024);
        let mut focused_element = None;
        let mut focused_index = None;
        let mut selected_elements = Vec::new();

        while let Some((element, depth)) = stack.pop() {
            if self.elements.len() >= MAX_COMPUTER_ACCESSIBILITY_ELEMENTS {
                break;
            }
            let index = self.elements.len();
            let summary = element_summary(&element, index, depth);
            if !push_bounded_line(
                &mut tree,
                &summary.line,
                MAX_COMPUTER_ACCESSIBILITY_TREE_BYTES,
            ) {
                break;
            }
            if summary.focused {
                focused_element = Some(summary.line.clone());
                focused_index = Some(index);
            }
            if summary.selected && selected_elements.len() < MAX_SELECTED_ELEMENTS {
                selected_elements.push(summary.line.clone());
            }
            self.elements.push(element.clone());

            if depth >= MAX_TREE_DEPTH {
                continue;
            }
            let Ok(first_child) = walker.get_first_child(&element) else {
                continue;
            };
            let mut children = vec![first_child.clone()];
            let mut current = first_child;
            while children.len() < MAX_CHILDREN_PER_ELEMENT
                && self
                    .elements
                    .len()
                    .saturating_add(stack.len())
                    .saturating_add(children.len())
                    < MAX_COMPUTER_ACCESSIBILITY_ELEMENTS
            {
                let Ok(next) = walker.get_next_sibling(&current) else {
                    break;
                };
                children.push(next.clone());
                current = next;
            }
            for child in children.into_iter().rev() {
                stack.push((child, depth + 1));
            }
        }

        let (selected_text, document_text) = focused_index
            .and_then(|index| self.elements.get(index))
            .map(focused_text)
            .unwrap_or((None, None));
        self.cached_window_id = Some(window_id.to_owned());
        Ok(ComputerAccessibilityState {
            tree,
            focused_element,
            selected_text,
            selected_elements: (!selected_elements.is_empty()).then_some(selected_elements),
            document_text,
        })
    }

    fn browser_url(&self, window_id: &str) -> Result<String, HelperErrorCode> {
        use std::collections::VecDeque;

        use uiautomation::patterns::UIValuePattern;
        use uiautomation::types::{ControlType, Handle};

        self.ensure_window_allowed(window_id)?;
        let raw_window_id = window_id
            .parse::<u32>()
            .map_err(|_| HelperErrorCode::InvalidInput)?;
        let handle_value =
            isize::try_from(raw_window_id).map_err(|_| HelperErrorCode::InvalidInput)?;
        let root = self
            .automation
            .element_from_handle(Handle::from(handle_value))
            .map_err(|_| HelperErrorCode::WindowUnavailable)?;
        let walker = self
            .automation
            .get_control_view_walker()
            .map_err(|_| HelperErrorCode::WindowUnavailable)?;
        let mut queue = VecDeque::from([(root, 0_usize)]);
        let mut visited = 0usize;
        let mut candidate = BrowserUrlCandidate::default();

        while let Some((element, depth)) = queue.pop_front() {
            if candidate
                .depth
                .is_some_and(|candidate_depth| depth > candidate_depth)
            {
                break;
            }
            visited = visited.saturating_add(1);
            if visited > MAX_COMPUTER_ACCESSIBILITY_ELEMENTS {
                return Err(HelperErrorCode::BrowserUrlUnavailable);
            }
            if element.get_control_type().ok() == Some(ControlType::Document)
                && let Ok(value_pattern) = element.get_pattern::<UIValuePattern>()
                && let Ok(value) = value_pattern.get_value()
            {
                candidate.consider(depth, &value);
            }
            if candidate.depth.is_some() || depth >= MAX_TREE_DEPTH {
                continue;
            }
            let Ok(first_child) = walker.get_first_child(&element) else {
                continue;
            };
            let mut child = first_child;
            for child_index in 0..MAX_CHILDREN_PER_ELEMENT {
                queue.push_back((child.clone(), depth + 1));
                let Ok(next) = walker.get_next_sibling(&child) else {
                    break;
                };
                if child_index + 1 == MAX_CHILDREN_PER_ELEMENT {
                    return Err(HelperErrorCode::BrowserUrlUnavailable);
                }
                child = next;
            }
            if visited.saturating_add(queue.len()) > MAX_COMPUTER_ACCESSIBILITY_ELEMENTS {
                return Err(HelperErrorCode::BrowserUrlUnavailable);
            }
        }

        candidate.finish()
    }

    fn activate_window(&self, window_id: &str) -> Result<(), HelperErrorCode> {
        use winsafe::{self as w, prelude::*};

        self.ensure_window_allowed(window_id)?;
        // Window handles are opaque and can be recycled. Rehydrate only from the
        // current bounded top-level window set instead of constructing an HWND
        // from the caller-provided number.
        let raw_window_id = window_id
            .parse::<u32>()
            .map_err(|_| HelperErrorCode::InvalidInput)?;
        let raw_window_id =
            usize::try_from(raw_window_id).map_err(|_| HelperErrorCode::InvalidInput)?;
        let mut target_window = None;
        let mut enumerated = 0usize;
        let _ = w::EnumWindows(|window| {
            enumerated = enumerated.saturating_add(1);
            if window.ptr() as usize == raw_window_id {
                target_window = Some(window);
                return false;
            }
            enumerated < MAX_NATIVE_WINDOW_ENUMERATION
        });
        let target_window = target_window.ok_or(HelperErrorCode::WindowUnavailable)?;

        if target_window.IsIconic() {
            let _ = target_window.ShowWindow(w::co::SW::RESTORE);
        }
        if w::HWND::GetForegroundWindow()
            .as_ref()
            .is_some_and(|window| window.ptr() == target_window.ptr())
        {
            return Ok(());
        }

        // Windows restricts SetForegroundWindow across input queues. The stable
        // helper temporarily joins the helper, foreground, and target queues,
        // then restores their independence before returning.
        let helper_thread_id = w::GetCurrentThreadId();
        let (target_thread_id, _) = target_window.GetWindowThreadProcessId();
        let foreground_thread_id = w::HWND::GetForegroundWindow()
            .as_ref()
            .map(|window| window.GetWindowThreadProcessId().0)
            .unwrap_or_default();
        let mut attached_thread_ids = [0u32; 2];
        let mut attached_count = 0usize;

        for thread_id in [foreground_thread_id, target_thread_id] {
            if thread_id == 0
                || thread_id == helper_thread_id
                || attached_thread_ids[..attached_count].contains(&thread_id)
            {
                continue;
            }
            if w::AttachThreadInput(helper_thread_id, thread_id, true).is_ok() {
                attached_thread_ids[attached_count] = thread_id;
                attached_count += 1;
            }
        }

        let _ = target_window.BringWindowToTop();
        let _ = target_window.SetForegroundWindow();
        let is_foreground = w::HWND::GetForegroundWindow()
            .as_ref()
            .is_some_and(|window| window.ptr() == target_window.ptr());

        for thread_id in attached_thread_ids[..attached_count].iter().rev() {
            let _ = w::AttachThreadInput(helper_thread_id, *thread_id, false);
        }

        is_foreground
            .then_some(())
            .ok_or(HelperErrorCode::ActionRejected)
    }

    fn cached_element(
        &self,
        window_id: &str,
        element_index: usize,
    ) -> Result<&uiautomation::UIElement, HelperErrorCode> {
        self.ensure_window_allowed(window_id)?;
        if self.cached_window_id.as_deref() != Some(window_id) {
            return Err(HelperErrorCode::StaleElement);
        }
        self.elements
            .get(element_index)
            .ok_or(HelperErrorCode::StaleElement)
    }

    fn ensure_window_allowed(&self, window_id: &str) -> Result<(), HelperErrorCode> {
        let window =
            inspect_computer_window(window_id).map_err(|_| HelperErrorCode::WindowUnavailable)?;
        if computer_use_target_is_forbidden(&window.application_id, &window.application) {
            Err(HelperErrorCode::ForbiddenTarget)
        } else {
            Ok(())
        }
    }

    fn click_element(
        &self,
        window_id: &str,
        element_index: usize,
        button: ComputerButton,
        click_count: u8,
    ) -> Result<(), HelperErrorCode> {
        use uiautomation::inputs::{Mouse, MouseButton};
        use uiautomation::types::Point;

        if click_count == 0 || click_count > 3 {
            return Err(HelperErrorCode::InvalidInput);
        }
        let element = self.cached_element(window_id, element_index)?;
        let point = element
            .get_clickable_point()
            .ok()
            .flatten()
            .or_else(|| {
                element.get_bounding_rectangle().ok().and_then(|rect| {
                    let width = rect.get_right().saturating_sub(rect.get_left());
                    let height = rect.get_bottom().saturating_sub(rect.get_top());
                    (width > 0 && height > 0).then(|| {
                        Point::new(
                            rect.get_left().saturating_add(width / 2),
                            rect.get_top().saturating_add(height / 2),
                        )
                    })
                })
            })
            .ok_or(HelperErrorCode::ActionRejected)?;
        let mouse_button = match button {
            ComputerButton::Left => MouseButton::LEFT,
            ComputerButton::Right => MouseButton::RIGHT,
            ComputerButton::Middle => MouseButton::MIDDLE,
        };
        let mouse = Mouse::new();
        mouse
            .move_to(&point)
            .map_err(|_| HelperErrorCode::ActionRejected)?;
        for _ in 0..click_count {
            mouse
                .click_button(mouse_button)
                .map_err(|_| HelperErrorCode::ActionRejected)?;
        }
        Ok(())
    }

    fn set_value(
        &self,
        window_id: &str,
        element_index: usize,
        value: &str,
    ) -> Result<(), HelperErrorCode> {
        use uiautomation::patterns::UIValuePattern;

        if value.len() > MAX_DOCUMENT_TEXT_BYTES {
            return Err(HelperErrorCode::InvalidInput);
        }
        let element = self.cached_element(window_id, element_index)?;
        if element.is_password().unwrap_or(false) {
            return Err(HelperErrorCode::ActionRejected);
        }
        element
            .get_pattern::<UIValuePattern>()
            .and_then(|pattern| pattern.set_value(value))
            .map_err(|_| HelperErrorCode::ActionRejected)
    }

    fn perform_secondary_action(
        &self,
        window_id: &str,
        element_index: usize,
        action: &str,
    ) -> Result<(), HelperErrorCode> {
        use uiautomation::patterns::{UIExpandCollapsePattern, UIInvokePattern};
        use uiautomation::types::ScrollAmount;

        let element = self.cached_element(window_id, element_index)?;
        match action.trim().to_ascii_lowercase().as_str() {
            "raise" | "focus" => element
                .set_focus()
                .map_err(|_| HelperErrorCode::ActionRejected),
            "invoke" | "press" => element
                .get_pattern::<UIInvokePattern>()
                .and_then(|pattern| pattern.invoke())
                .map_err(|_| HelperErrorCode::ActionRejected),
            "expand" => element
                .get_pattern::<UIExpandCollapsePattern>()
                .and_then(|pattern| pattern.expand())
                .map_err(|_| HelperErrorCode::ActionRejected),
            "collapse" => element
                .get_pattern::<UIExpandCollapsePattern>()
                .and_then(|pattern| pattern.collapse())
                .map_err(|_| HelperErrorCode::ActionRejected),
            "scroll up" => scroll_element(
                element,
                ScrollAmount::NoAmount,
                ScrollAmount::LargeDecrement,
            ),
            "scroll down" => scroll_element(
                element,
                ScrollAmount::NoAmount,
                ScrollAmount::LargeIncrement,
            ),
            "scroll left" => scroll_element(
                element,
                ScrollAmount::LargeDecrement,
                ScrollAmount::NoAmount,
            ),
            "scroll right" => scroll_element(
                element,
                ScrollAmount::LargeIncrement,
                ScrollAmount::NoAmount,
            ),
            _ => Err(HelperErrorCode::ActionRejected),
        }
    }
}

#[cfg(windows)]
fn scroll_element(
    element: &uiautomation::UIElement,
    horizontal: uiautomation::types::ScrollAmount,
    vertical: uiautomation::types::ScrollAmount,
) -> Result<(), HelperErrorCode> {
    use uiautomation::patterns::UIScrollPattern;

    element
        .get_pattern::<UIScrollPattern>()
        .and_then(|pattern| pattern.scroll(horizontal, vertical))
        .map_err(|_| HelperErrorCode::ActionRejected)
}

#[cfg(windows)]
struct ElementSummary {
    line: String,
    focused: bool,
    selected: bool,
}

#[cfg(windows)]
fn element_summary(
    element: &uiautomation::UIElement,
    index: usize,
    depth: usize,
) -> ElementSummary {
    use uiautomation::patterns::{
        UIExpandCollapsePattern, UIInvokePattern, UIScrollPattern, UISelectionItemPattern,
        UIValuePattern,
    };
    use uiautomation::types::ExpandCollapseState;

    let role = element
        .get_control_type()
        .map_or_else(|_| "Control".to_owned(), |role| format!("{role:?}"));
    let name = bounded_inline_text(
        element.get_name().unwrap_or_default(),
        MAX_ELEMENT_TEXT_BYTES,
    );
    let focused = element.has_keyboard_focus().unwrap_or(false);
    let focusable = element.is_keyboard_focusable().unwrap_or(false);
    let enabled = element.is_enabled().unwrap_or(true);
    let offscreen = element.is_offscreen().unwrap_or(false);
    let password = element.is_password().unwrap_or(false);
    let selected = element
        .get_pattern::<UISelectionItemPattern>()
        .and_then(|pattern| pattern.is_selected())
        .unwrap_or(false);
    let value = if password {
        None
    } else {
        element
            .get_pattern::<UIValuePattern>()
            .ok()
            .and_then(|pattern| pattern.get_value().ok())
            .map(|value| bounded_inline_text(value, MAX_ELEMENT_TEXT_BYTES))
            .filter(|value| !value.is_empty())
    };

    let mut actions = Vec::with_capacity(8);
    if focusable {
        actions.push("Raise");
    }
    if element.get_pattern::<UIInvokePattern>().is_ok() {
        actions.push("Invoke");
    }
    if element.get_pattern::<UIValuePattern>().is_ok() && !password {
        actions.push("Set Value");
    }
    if let Ok(pattern) = element.get_pattern::<UIExpandCollapsePattern>() {
        match pattern.get_state() {
            Ok(ExpandCollapseState::Collapsed) => actions.push("Expand"),
            Ok(ExpandCollapseState::Expanded | ExpandCollapseState::PartiallyExpanded) => {
                actions.push("Collapse");
            }
            Ok(ExpandCollapseState::LeafNode) | Err(_) => {}
        }
    }
    if let Ok(pattern) = element.get_pattern::<UIScrollPattern>() {
        if pattern.is_vertically_scrollable().unwrap_or(false) {
            actions.extend(["Scroll Up", "Scroll Down"]);
        }
        if pattern.is_horizontally_scrollable().unwrap_or(false) {
            actions.extend(["Scroll Left", "Scroll Right"]);
        }
    }

    let mut attributes = Vec::with_capacity(8);
    if focused {
        attributes.push("focused".to_owned());
    }
    if selected {
        attributes.push("selected".to_owned());
    }
    if !enabled {
        attributes.push("disabled".to_owned());
    }
    if offscreen {
        attributes.push("offscreen".to_owned());
    }
    if password {
        attributes.push("password".to_owned());
    }
    if let Ok(rect) = element.get_bounding_rectangle() {
        let width = rect.get_right().saturating_sub(rect.get_left());
        let height = rect.get_bottom().saturating_sub(rect.get_top());
        if width > 0 && height > 0 {
            attributes.push(format!(
                "bounds=({}, {}, {}, {})",
                rect.get_left(),
                rect.get_top(),
                width,
                height
            ));
        }
    }
    if !actions.is_empty() {
        attributes.push(format!("actions=[{}]", actions.join(", ")));
    }
    if let Some(value) = value {
        attributes.push(format!("value=\"{}\"", escape_tree_text(&value)));
    }

    let indent = "  ".repeat(depth.min(MAX_TREE_DEPTH));
    let mut line = format!("{indent}[{index}] {role}");
    if !name.is_empty() {
        line.push_str(" \"");
        line.push_str(&escape_tree_text(&name));
        line.push('"');
    }
    if !attributes.is_empty() {
        line.push(' ');
        line.push_str(&attributes.join(" "));
    }
    line = bounded_inline_text(line, 2 * 1024);
    ElementSummary {
        line,
        focused,
        selected,
    }
}

#[cfg(windows)]
fn focused_text(element: &uiautomation::UIElement) -> (Option<String>, Option<String>) {
    use uiautomation::patterns::{UITextPattern, UIValuePattern};

    if element.is_password().unwrap_or(false) {
        return (None, None);
    }
    if let Ok(pattern) = element.get_pattern::<UITextPattern>() {
        let selected_text = pattern
            .get_selection()
            .ok()
            .into_iter()
            .flatten()
            .take(MAX_SELECTED_ELEMENTS)
            .filter_map(|range| range.get_text(MAX_SELECTED_TEXT_BYTES as i32).ok())
            .collect::<Vec<_>>()
            .join("\n");
        let selected_text = bounded_optional_text(selected_text, MAX_SELECTED_TEXT_BYTES);
        let document_text = pattern
            .get_document_range()
            .and_then(|range| range.get_text(MAX_DOCUMENT_TEXT_BYTES as i32))
            .ok()
            .and_then(|text| bounded_optional_text(text, MAX_DOCUMENT_TEXT_BYTES));
        return (selected_text, document_text);
    }
    let document_text = element
        .get_pattern::<UIValuePattern>()
        .and_then(|pattern| pattern.get_value())
        .ok()
        .and_then(|text| bounded_optional_text(text, MAX_DOCUMENT_TEXT_BYTES));
    (None, document_text)
}

#[cfg(any(windows, test))]
#[derive(Default)]
struct BrowserUrlCandidate {
    depth: Option<usize>,
    url: Option<String>,
    ambiguous: bool,
}

#[cfg(any(windows, test))]
impl BrowserUrlCandidate {
    fn consider(&mut self, depth: usize, value: &str) {
        let Some(url) = validated_browser_url(value) else {
            return;
        };
        match self.depth {
            None => {
                self.depth = Some(depth);
                self.url = Some(url);
                self.ambiguous = false;
            }
            Some(candidate_depth) if depth < candidate_depth => {
                self.depth = Some(depth);
                self.url = Some(url);
                self.ambiguous = false;
            }
            Some(candidate_depth)
                if depth == candidate_depth && self.url.as_deref() != Some(url.as_str()) =>
            {
                self.ambiguous = true;
            }
            Some(_) => {}
        }
    }

    fn finish(self) -> Result<String, HelperErrorCode> {
        if self.ambiguous {
            return Err(HelperErrorCode::BrowserUrlUnavailable);
        }
        self.url.ok_or(HelperErrorCode::BrowserUrlUnavailable)
    }
}

#[cfg(any(windows, test))]
fn validated_browser_url(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > MAX_BROWSER_URL_BYTES
        || value.chars().any(char::is_control)
        || url::Url::parse(value).is_err()
    {
        return None;
    }
    Some(value.to_owned())
}

#[cfg(windows)]
fn bounded_optional_text(value: String, max_bytes: usize) -> Option<String> {
    let value = bounded_inline_text(value, max_bytes);
    (!value.is_empty()).then_some(value)
}

#[cfg(any(windows, test))]
fn bounded_inline_text(mut value: String, max_bytes: usize) -> String {
    value = value
        .chars()
        .map(|character| match character {
            '\r' | '\n' | '\t' => ' ',
            _ => character,
        })
        .collect();
    if value.len() <= max_bytes {
        return value;
    }
    let mut boundary = max_bytes;
    while boundary > 0 && !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    value.truncate(boundary);
    value
}

#[cfg(windows)]
fn escape_tree_text(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(any(windows, test))]
fn push_bounded_line(target: &mut String, line: &str, limit: usize) -> bool {
    let separator = usize::from(!target.is_empty());
    if target
        .len()
        .saturating_add(separator)
        .saturating_add(line.len())
        > limit
    {
        return false;
    }
    if separator != 0 {
        target.push('\n');
    }
    target.push_str(line);
    true
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Cursor};

    use super::{
        BrowserUrlCandidate, ComputerAccessibilityError, ComputerUseAccessibilityClient,
        MAX_COMPUTER_ACCESSIBILITY_TREE_BYTES, MAX_STALE_INPUT_WINDOWS, bounded_inline_text,
        merge_running_windows_with, push_bounded_line, read_bounded_line, validated_browser_url,
    };
    use crate::{ComputerApplication, ComputerCapture, ComputerWindow};

    #[test]
    fn helper_frames_are_bounded_and_drained() -> std::io::Result<()> {
        let input = Cursor::new(b"123456\nok\n");
        let mut reader = BufReader::new(input);
        assert!(read_bounded_line(&mut reader, 3).is_err());
        assert_eq!(read_bounded_line(&mut reader, 3)?, Some(b"ok".to_vec()));
        Ok(())
    }

    #[test]
    fn accessibility_text_limits_preserve_utf8_and_tree_budget() {
        assert_eq!(bounded_inline_text("abЯ".to_owned(), 3), "ab");
        let mut tree = String::new();
        assert!(push_bounded_line(
            &mut tree,
            "first",
            MAX_COMPUTER_ACCESSIBILITY_TREE_BYTES
        ));
        assert!(!push_bounded_line(&mut tree, "second", 6));
        assert_eq!(tree, "first");
    }

    #[test]
    fn browser_url_selection_is_bounded_shallow_and_unambiguous() {
        assert_eq!(
            validated_browser_url(" https://example.com/path "),
            Some("https://example.com/path".to_owned())
        );
        assert!(validated_browser_url("not a URL").is_none());
        assert!(
            validated_browser_url(&format!("https://example.com/{}", "a".repeat(8 * 1024)))
                .is_none()
        );

        let mut candidate = BrowserUrlCandidate::default();
        candidate.consider(3, "https://deeper.example/");
        candidate.consider(1, "edge://settings/");
        candidate.consider(2, "https://ignored.example/");
        assert_eq!(candidate.finish().ok().as_deref(), Some("edge://settings/"));

        let mut ambiguous = BrowserUrlCandidate::default();
        ambiguous.consider(1, "https://one.example/");
        ambiguous.consider(1, "https://two.example/");
        assert!(matches!(
            ambiguous.finish(),
            Err(super::HelperErrorCode::BrowserUrlUnavailable)
        ));
    }

    #[test]
    fn product_policy_rejects_forbidden_launches_before_spawning_the_helper() {
        let mut client = ComputerUseAccessibilityClient::new();
        assert_eq!(
            client
                .validate_app_launch(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"),
            Err(ComputerAccessibilityError::ForbiddenTarget)
        );
        assert_eq!(
            client.launch_app("OpenAI.CodexBeta_2p2nqsd0c76g0!App"),
            Err(ComputerAccessibilityError::ForbiddenTarget)
        );
    }

    #[test]
    fn running_apps_precede_the_catalog_without_reordering_installed_apps() {
        let installed = ["alpha.exe", "beta.exe", "gamma.exe"]
            .into_iter()
            .map(|id| ComputerApplication {
                id: id.to_owned(),
                display_name: Some(id.to_owned()),
                last_used_date: Some("2026-07-24".to_owned()),
                use_count: Some(0),
                is_running: false,
                windows: Vec::new(),
            })
            .collect();
        let windows = ["gamma.exe", "running-only.exe"]
            .into_iter()
            .enumerate()
            .map(|(index, id)| ComputerWindow {
                id: (index + 1).to_string(),
                process_id: (index + 1) as u32,
                application: id.to_owned(),
                application_id: id.to_owned(),
                title: id.to_owned(),
                x: 0,
                y: 0,
                width: 800,
                height: 600,
                minimized: false,
                focused: index == 0,
            })
            .collect();

        let merged = merge_running_windows_with(installed, windows);

        assert_eq!(
            merged
                .iter()
                .map(|application| application.id.as_str())
                .collect::<Vec<_>>(),
            ["gamma.exe", "running-only.exe", "alpha.exe", "beta.exe"]
        );
        assert!(merged[0].is_running);
        assert_eq!(merged[0].last_used_date.as_deref(), Some("2026-07-24"));
    }

    #[test]
    fn screenshot_ids_map_bounded_image_coordinates_back_to_the_window() {
        let mut client = ComputerUseAccessibilityClient::new();
        client.remember_capture(&ComputerCapture {
            window: ComputerWindow {
                id: "7".to_owned(),
                process_id: 1,
                application: "fixture".to_owned(),
                application_id: "fixture.exe".to_owned(),
                title: "fixture".to_owned(),
                x: 0,
                y: 0,
                width: 1_600,
                height: 1_200,
                minimized: false,
                focused: true,
            },
            screenshot_id: "screenshot-1".to_owned(),
            width: 800,
            height: 600,
            jpeg_bytes: 0,
            image_url: String::new(),
        });

        assert_eq!(
            client.map_screenshot_point("7", "screenshot-1", 400, 300),
            Ok((800, 600))
        );
        client.mark_user_input("7");
        assert!(client.user_input_requires_refresh("7"));
        assert_eq!(
            client.map_screenshot_point("7", "screenshot-1", 400, 300),
            Err(ComputerAccessibilityError::StaleScreenshot)
        );
        assert_eq!(
            client.map_screenshot_point("7", "stale", 400, 300),
            Err(ComputerAccessibilityError::StaleScreenshot)
        );
    }

    #[test]
    fn stale_input_overflow_stays_fail_closed_for_unrefreshed_windows() {
        let mut client = ComputerUseAccessibilityClient::new();
        for index in 0..=MAX_STALE_INPUT_WINDOWS {
            client.mark_user_input(&index.to_string());
        }

        assert!(client.user_input_requires_refresh("0"));
        assert!(client.user_input_requires_refresh("unseen"));
        client.clear_user_input("0");
        assert!(!client.user_input_requires_refresh("0"));
        assert!(client.user_input_requires_refresh("unseen"));
        client.mark_user_input("0");
        assert!(client.user_input_requires_refresh("0"));
    }
}
