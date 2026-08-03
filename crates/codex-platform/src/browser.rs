use std::collections::{HashMap, HashSet, VecDeque};
use std::error::Error;
use std::ffi::OsString;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Cursor, Read};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use codex_core::{BrowserPermissionResource, BrowserPermissionValue, BrowserPermissionsState};
use crossbeam_channel::{Receiver, Sender, TryRecvError, TrySendError};
use image::ImageDecoder as _;
use image::codecs::jpeg::JpegDecoder;
use serde_json::{Value, json};
use tungstenite::client::client_with_config;
use tungstenite::protocol::WebSocketConfig;
use tungstenite::{Error as WebSocketError, HandshakeError, Message, WebSocket};
use url::Url;

use crate::browser_agent::{
    BrowserAgentBridge, BrowserAgentNotification, BrowserAgentRequest, BrowserAgentRpcError,
};

#[cfg(unix)]
use rustix::process::{Pid, Signal, kill_process_group};
#[cfg(unix)]
use std::os::unix::{fs::PermissionsExt, process::CommandExt};
#[cfg(windows)]
use std::os::windows::io::AsRawHandle;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
use win32job::{ExtendedLimitInfo, Job};

pub const BROWSER_EVENT_CAPACITY: usize = 64;
pub const MAX_BROWSER_CONTEXT_ID_BYTES: usize = 256;
pub const MAX_BROWSER_FRAME_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_BROWSER_TABS: usize = 16;
pub const MAX_BROWSER_TITLE_BYTES: usize = 512;
pub const MAX_BROWSER_URL_BYTES: usize = 8 * 1024;

const BROWSER_COMMAND_CAPACITY: usize = 64;
const BROWSER_COMMANDS_PER_TICK: usize = 8;
const BROWSER_EVENTS_PER_TICK: usize = 8;
const BROWSER_AGENT_EVENT_CAPACITY: usize = 128;
const BROWSER_TICK: Duration = Duration::from_millis(25);
const CDP_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const CDP_IO_TIMEOUT: Duration = Duration::from_millis(25);
const CDP_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const CDP_SHUTDOWN_REQUESTED: &str = "Browser shutdown requested";
const DEVTOOLS_PORT_TIMEOUT: Duration = Duration::from_secs(10);
const GRACEFUL_BROWSER_EXIT: Duration = Duration::from_secs(1);
const MAX_CDP_MESSAGE_BYTES: usize = 8 * 1024 * 1024;
const MAX_CDP_PENDING_EVENT_BYTES: usize = 16 * 1024 * 1024;
const MAX_BROWSER_KEY_BYTES: usize = 64;
const MAX_BROWSER_TEXT_BYTES: usize = 256;
const MAX_CACHED_AGENT_EXPRESSIONS: usize = 32;
const MAX_CACHED_AGENT_EXPRESSION_BYTES: usize = 1024 * 1024;
const MAX_AGENT_CACHE_KEY_BYTES: usize = 256;
const MAX_AGENT_CHILD_SESSIONS: usize = BROWSER_COMMAND_CAPACITY;
const MAX_BROWSER_DOWNLOAD_DIRECTORY_ENTRIES: usize = 32;
const MAX_BROWSER_DOWNLOAD_FILENAME_ATTEMPTS: u32 = 10_000;
const MAX_BROWSER_DOWNLOAD_FILENAME_BYTES: usize = 240;
const MAX_BROWSER_DOWNLOAD_FRAMES: usize = 64;
const MAX_BROWSER_DOWNLOAD_ID_BYTES: usize = 256;
const MAX_BROWSER_DOWNLOAD_PATH_BYTES: usize = 4 * 1024;
const MAX_BROWSER_DOWNLOAD_HISTORY: usize = 200;
const MAX_BROWSER_DOWNLOAD_WEBUI_RESOURCES: usize = 32;
#[cfg(any(target_os = "linux", test))]
const MAX_XDG_USER_DIRS_BYTES: usize = 16 * 1024;
const MAX_DEVTOOLS_ACTIVE_PORT_BYTES: u64 = 4 * 1024;
const MAX_DEVTOOLS_PATH_BYTES: usize = 512;
const MAX_ERROR_BYTES: usize = 2 * 1024;
const MAX_BROWSER_AGENT_ID_BYTES: usize = 256;
const BROWSER_DOWNLOAD_GRANT_TIMEOUT: Duration = Duration::from_secs(10);
const BROWSER_DOWNLOAD_CONTROL_TIMEOUT: Duration = Duration::from_secs(5);
const BROWSER_VISIBILITY_CAPABILITY_DESCRIPTION: &str = "Use to show or hide the browser to the user, and to determine the browser's current visibility. Keep browser work in the background unless the user asks to see it or live viewing is useful. When the browser should be visible, call set(true).";
const BROWSER_VIEWPORT_CAPABILITY_DESCRIPTION: &str = "Controls an explicit browser viewport override for responsive or device-size testing. Use it when a task calls for specific dimensions or breakpoint validation; otherwise leave it unset so the browser uses its normal viewport. Reset temporary overrides before finishing unless the user asked to keep them.";
const BROWSER_PAGE_ASSETS_CAPABILITY_DESCRIPTION: &str = "List assets already observed in the current page state and bundle selected assets into a temporary local artifact.";
const MAX_VIEWPORT_HEIGHT: u32 = 1_080;
const MAX_VIEWPORT_WIDTH: u32 = 1_920;
const MIN_VIEWPORT_HEIGHT: u32 = 240;
const MIN_VIEWPORT_WIDTH: u32 = 320;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone)]
pub struct BrowserConfig {
    pub context_id: String,
    pub download_dir: Option<PathBuf>,
    pub executable: Option<PathBuf>,
    pub profile_dir: PathBuf,
    pub prompt_for_user_downloads: bool,
    pub permissions: BrowserPermissionsState,
    pub initial_url: Option<String>,
    pub viewport_width: u32,
    pub viewport_height: u32,
}

impl BrowserConfig {
    #[must_use]
    pub fn new(profile_dir: PathBuf, context_id: String) -> Self {
        Self {
            context_id,
            download_dir: None,
            executable: None,
            profile_dir,
            prompt_for_user_downloads: false,
            permissions: BrowserPermissionsState::default(),
            initial_url: None,
            viewport_width: 1_280,
            viewport_height: 800,
        }
    }

    #[must_use]
    pub fn with_executable(mut self, executable: Option<PathBuf>) -> Self {
        self.executable = executable;
        self
    }

    #[must_use]
    pub fn with_download_dir(mut self, download_dir: Option<PathBuf>) -> Self {
        self.download_dir = download_dir;
        self
    }

    #[must_use]
    pub fn with_prompt_for_user_downloads(mut self, prompt: bool) -> Self {
        self.prompt_for_user_downloads = prompt;
        self
    }

    #[must_use]
    pub fn with_permissions(mut self, permissions: BrowserPermissionsState) -> Self {
        self.permissions = permissions.normalized();
        self
    }

    #[must_use]
    pub fn with_initial_url(mut self, initial_url: Option<String>) -> Self {
        self.initial_url = initial_url;
        self
    }

    #[must_use]
    pub fn with_viewport(mut self, width: u32, height: u32) -> Self {
        self.viewport_width = width.clamp(MIN_VIEWPORT_WIDTH, MAX_VIEWPORT_WIDTH);
        self.viewport_height = height.clamp(MIN_VIEWPORT_HEIGHT, MAX_VIEWPORT_HEIGHT);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserTab {
    pub id: String,
    pub url: String,
    pub title: String,
    pub loading: bool,
    pub can_go_back: bool,
    pub can_go_forward: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserMouseButton {
    Left,
    Middle,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserKeyInput {
    pub key: String,
    pub text: Option<String>,
    pub alt: bool,
    pub control: bool,
    pub meta: bool,
    pub shift: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserDownloadStatus {
    Started,
    InProgress,
    Paused,
    Failed,
    Canceled,
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserDownload {
    pub can_cancel: bool,
    pub can_pause: bool,
    pub can_resume: bool,
    pub context_id: String,
    pub file_exists: bool,
    pub filename: String,
    pub id: String,
    pub path: PathBuf,
    pub received_bytes: u64,
    pub started_at_ms: u64,
    pub status: BrowserDownloadStatus,
    pub total_bytes: u64,
    pub updated_at_ms: u64,
    pub url: String,
    pub user_initiated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BrowserFamily {
    Chrome,
    Edge,
}

impl BrowserFamily {
    fn from_executable(executable: &Path) -> Self {
        if executable
            .file_stem()
            .and_then(std::ffi::OsStr::to_str)
            .is_some_and(|name| name.eq_ignore_ascii_case("msedge"))
        {
            Self::Edge
        } else {
            Self::Chrome
        }
    }

    const fn downloads_url(self) -> &'static str {
        match self {
            Self::Chrome => "chrome://downloads/",
            Self::Edge => "edge://downloads/all",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BrowserDownloadControl {
    Pause,
    Resume,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowserEvent {
    Ready {
        executable: PathBuf,
    },
    TabsChanged {
        context_id: String,
        tabs: Vec<BrowserTab>,
        active_tab_id: String,
    },
    Frame {
        context_id: String,
        tab_id: String,
        jpeg: Vec<u8>,
        width: u32,
        height: u32,
    },
    VisibilityRequested {
        context_id: String,
        visible: bool,
    },
    DownloadChanged(BrowserDownload),
    DownloadSaveRequested {
        directory: PathBuf,
        filename: String,
        id: String,
    },
    DownloadRemoved {
        id: String,
    },
    OperationFailed(String),
    Failed(String),
    Exited,
}

#[derive(Debug)]
pub enum BrowserError {
    MissingBrowser,
    Profile(io::Error),
    AgentBridge(io::Error),
    Thread(io::Error),
}

impl fmt::Display for BrowserError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingBrowser => formatter
                .write_str("Chrome, Edge, or Chromium is required for the native Browser panel"),
            Self::Profile(_) => formatter.write_str("could not prepare the Browser profile"),
            Self::AgentBridge(_) => formatter.write_str("could not start the Browser agent bridge"),
            Self::Thread(_) => formatter.write_str("could not start the Browser supervisor"),
        }
    }
}

impl Error for BrowserError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Profile(error) | Self::AgentBridge(error) | Self::Thread(error) => Some(error),
            Self::MissingBrowser => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserCommandError {
    InvalidTab,
    InvalidUrl,
    InvalidInput,
    InvalidPath,
    QueueFull,
    Disconnected,
}

impl fmt::Display for BrowserCommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTab => formatter.write_str("the Browser tab identifier is invalid"),
            Self::InvalidUrl => formatter.write_str("Browser URLs must use http:// or https://"),
            Self::InvalidInput => formatter.write_str("the Browser input is invalid"),
            Self::InvalidPath => formatter.write_str("the Browser path is invalid"),
            Self::QueueFull => formatter.write_str("the Browser command queue is full"),
            Self::Disconnected => formatter.write_str("the Browser is disconnected"),
        }
    }
}

impl Error for BrowserCommandError {}

pub(crate) enum BrowserCommand {
    ActivateContext(String),
    Navigate {
        context_id: String,
        url: String,
    },
    Back(String),
    Forward(String),
    Reload(String),
    Stop(String),
    OpenTab {
        context_id: String,
        url: Option<String>,
    },
    SelectTab {
        context_id: String,
        tab_id: String,
    },
    CloseTab {
        context_id: String,
        tab_id: String,
    },
    Resize {
        width: u32,
        height: u32,
    },
    SyncSurfaceState {
        context_id: Option<String>,
        visible: bool,
    },
    Click {
        context_id: String,
        x: u32,
        y: u32,
        button: BrowserMouseButton,
    },
    Scroll {
        context_id: String,
        x: u32,
        y: u32,
        delta_x: i32,
        delta_y: i32,
    },
    Key {
        context_id: String,
        input: BrowserKeyInput,
    },
    SetDownloadDirectory(PathBuf),
    SetPromptForUserDownloads(bool),
    SetPermissions(BrowserPermissionsState),
    SetDownloadDestination {
        id: String,
        path: Option<PathBuf>,
    },
    CancelDownload(String),
    PauseDownload(String),
    ResumeDownload(String),
    OpenDownload(String),
    RemoveDownload(String),
    ShowDownloadInFolder(String),
    ShowDownloadsFolder,
    AgentRpc {
        request: BrowserAgentRequest,
        response: Sender<Result<Value, BrowserAgentRpcError>>,
    },
    Shutdown,
}

pub struct BrowserSession {
    commands: Sender<BrowserCommand>,
    events: Receiver<BrowserEvent>,
    latest_frame: LatestBrowserFrame,
    agent_bridge: BrowserAgentBridge,
    shutdown_requested: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

#[derive(Clone, Default)]
struct LatestBrowserFrame {
    frame: Arc<Mutex<Option<BrowserEvent>>>,
}

struct BrowserUiEvents {
    control: Sender<BrowserEvent>,
    latest_frame: LatestBrowserFrame,
    shutdown_requested: Arc<AtomicBool>,
}

impl LatestBrowserFrame {
    fn replace(&self, frame: BrowserEvent) {
        debug_assert!(matches!(frame, BrowserEvent::Frame { .. }));
        *self
            .frame
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(frame);
    }

    fn take(&self) -> Option<BrowserEvent> {
        self.frame
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
    }
}

fn try_recv_browser_event(
    events: &Receiver<BrowserEvent>,
    latest_frame: &LatestBrowserFrame,
) -> Result<Option<BrowserEvent>, BrowserCommandError> {
    match events.try_recv() {
        Ok(event) => Ok(Some(event)),
        Err(TryRecvError::Empty) => Ok(latest_frame.take()),
        Err(TryRecvError::Disconnected) => latest_frame
            .take()
            .map(Some)
            .ok_or(BrowserCommandError::Disconnected),
    }
}

impl BrowserSession {
    pub fn spawn(config: BrowserConfig) -> Result<Self, BrowserError> {
        let executable = config
            .executable
            .clone()
            .filter(|path| is_browser_executable(path))
            .or_else(resolve_browser_binary)
            .ok_or(BrowserError::MissingBrowser)?;
        fs::create_dir_all(&config.profile_dir).map_err(BrowserError::Profile)?;
        let current_dir = std::env::current_dir().map_err(BrowserError::Profile)?;
        let mut download_dir = config
            .download_dir
            .clone()
            .unwrap_or_else(|| config.profile_dir.join("downloads"));
        if !download_dir.is_absolute() {
            download_dir = current_dir.join(download_dir);
        }
        let mut download_staging_root = config.profile_dir.join("download-staging");
        if !download_staging_root.is_absolute() {
            download_staging_root = current_dir.join(download_staging_root);
        }
        fs::create_dir_all(&download_dir).map_err(BrowserError::Profile)?;
        fs::create_dir_all(&download_staging_root).map_err(BrowserError::Profile)?;

        let (command_sender, command_receiver) =
            crossbeam_channel::bounded(BROWSER_COMMAND_CAPACITY);
        let (event_sender, event_receiver) = crossbeam_channel::bounded(BROWSER_EVENT_CAPACITY);
        let shutdown_requested = Arc::new(AtomicBool::new(false));
        let latest_frame = LatestBrowserFrame::default();
        let runtime_ui_events = BrowserUiEvents {
            control: event_sender.clone(),
            latest_frame: latest_frame.clone(),
            shutdown_requested: Arc::clone(&shutdown_requested),
        };
        let (agent_event_sender, agent_event_receiver) =
            crossbeam_channel::bounded(BROWSER_AGENT_EVENT_CAPACITY);
        let agent_bridge = BrowserAgentBridge::spawn(command_sender.clone(), agent_event_receiver)
            .map_err(BrowserError::AgentBridge)?;
        let thread = thread::Builder::new()
            .name("codex-browser-supervisor".to_owned())
            .spawn(move || {
                if let Err(error) = run_browser(
                    executable,
                    config,
                    download_dir,
                    download_staging_root,
                    command_receiver,
                    runtime_ui_events,
                    agent_event_sender,
                ) {
                    let _ = event_sender.try_send(BrowserEvent::Failed(bounded_error(error)));
                }
                let _ = event_sender.try_send(BrowserEvent::Exited);
            })
            .map_err(BrowserError::Thread)?;

        Ok(Self {
            commands: command_sender,
            events: event_receiver,
            latest_frame,
            agent_bridge,
            shutdown_requested,
            thread: Some(thread),
        })
    }

    #[must_use]
    pub fn agent_endpoint(&self) -> &str {
        self.agent_bridge.endpoint()
    }

    pub fn activate_context(&self, context_id: &str) -> Result<(), BrowserCommandError> {
        let context_id = checked_context_id(context_id)?;
        send_command(&self.commands, BrowserCommand::ActivateContext(context_id))
    }

    pub fn navigate(&self, context_id: &str, url: &str) -> Result<(), BrowserCommandError> {
        let context_id = checked_context_id(context_id)?;
        let url = checked_navigation_url(url)?;
        send_command(&self.commands, BrowserCommand::Navigate { context_id, url })
    }

    pub fn back(&self, context_id: &str) -> Result<(), BrowserCommandError> {
        send_command(
            &self.commands,
            BrowserCommand::Back(checked_context_id(context_id)?),
        )
    }

    pub fn forward(&self, context_id: &str) -> Result<(), BrowserCommandError> {
        send_command(
            &self.commands,
            BrowserCommand::Forward(checked_context_id(context_id)?),
        )
    }

    pub fn reload(&self, context_id: &str) -> Result<(), BrowserCommandError> {
        send_command(
            &self.commands,
            BrowserCommand::Reload(checked_context_id(context_id)?),
        )
    }

    pub fn stop(&self, context_id: &str) -> Result<(), BrowserCommandError> {
        send_command(
            &self.commands,
            BrowserCommand::Stop(checked_context_id(context_id)?),
        )
    }

    pub fn open_tab(&self, context_id: &str, url: Option<&str>) -> Result<(), BrowserCommandError> {
        let context_id = checked_context_id(context_id)?;
        let url = url.map(checked_navigation_url).transpose()?;
        send_command(&self.commands, BrowserCommand::OpenTab { context_id, url })
    }

    pub fn select_tab(&self, context_id: &str, tab_id: &str) -> Result<(), BrowserCommandError> {
        let context_id = checked_context_id(context_id)?;
        let tab_id = checked_tab_id(tab_id)?;
        send_command(
            &self.commands,
            BrowserCommand::SelectTab { context_id, tab_id },
        )
    }

    pub fn close_tab(&self, context_id: &str, tab_id: &str) -> Result<(), BrowserCommandError> {
        let context_id = checked_context_id(context_id)?;
        let tab_id = checked_tab_id(tab_id)?;
        send_command(
            &self.commands,
            BrowserCommand::CloseTab { context_id, tab_id },
        )
    }

    pub fn resize(&self, width: u32, height: u32) -> Result<(), BrowserCommandError> {
        send_command(
            &self.commands,
            BrowserCommand::Resize {
                width: width.clamp(MIN_VIEWPORT_WIDTH, MAX_VIEWPORT_WIDTH),
                height: height.clamp(MIN_VIEWPORT_HEIGHT, MAX_VIEWPORT_HEIGHT),
            },
        )
    }

    pub fn sync_surface_state(
        &self,
        context_id: Option<&str>,
        visible: bool,
    ) -> Result<(), BrowserCommandError> {
        let context_id = context_id.map(checked_context_id).transpose()?;
        send_command(
            &self.commands,
            BrowserCommand::SyncSurfaceState {
                visible: visible && context_id.is_some(),
                context_id,
            },
        )
    }

    pub fn click(
        &self,
        context_id: &str,
        x: u32,
        y: u32,
        button: BrowserMouseButton,
    ) -> Result<(), BrowserCommandError> {
        send_command(
            &self.commands,
            BrowserCommand::Click {
                context_id: checked_context_id(context_id)?,
                x,
                y,
                button,
            },
        )
    }

    pub fn scroll(
        &self,
        context_id: &str,
        x: u32,
        y: u32,
        delta_x: i32,
        delta_y: i32,
    ) -> Result<(), BrowserCommandError> {
        send_command(
            &self.commands,
            BrowserCommand::Scroll {
                context_id: checked_context_id(context_id)?,
                x,
                y,
                delta_x: delta_x.clamp(-10_000, 10_000),
                delta_y: delta_y.clamp(-10_000, 10_000),
            },
        )
    }

    pub fn key(
        &self,
        context_id: &str,
        mut input: BrowserKeyInput,
    ) -> Result<(), BrowserCommandError> {
        input.key = input.key.trim().to_owned();
        if input.key.is_empty()
            || input.key.len() > MAX_BROWSER_KEY_BYTES
            || input.key.chars().any(char::is_control)
            || input.text.as_ref().is_some_and(|text| {
                text.is_empty() || text.len() > MAX_BROWSER_TEXT_BYTES || text.contains('\0')
            })
        {
            return Err(BrowserCommandError::InvalidInput);
        }
        send_command(
            &self.commands,
            BrowserCommand::Key {
                context_id: checked_context_id(context_id)?,
                input,
            },
        )
    }

    pub fn set_download_directory(&self, path: &Path) -> Result<(), BrowserCommandError> {
        send_command(
            &self.commands,
            BrowserCommand::SetDownloadDirectory(checked_download_path(path)?),
        )
    }

    pub fn set_prompt_for_user_downloads(&self, prompt: bool) -> Result<(), BrowserCommandError> {
        send_command(
            &self.commands,
            BrowserCommand::SetPromptForUserDownloads(prompt),
        )
    }

    pub fn set_permissions(
        &self,
        permissions: BrowserPermissionsState,
    ) -> Result<(), BrowserCommandError> {
        send_command(
            &self.commands,
            BrowserCommand::SetPermissions(permissions.normalized()),
        )
    }

    pub fn set_download_destination(
        &self,
        id: &str,
        path: Option<&Path>,
    ) -> Result<(), BrowserCommandError> {
        send_command(
            &self.commands,
            BrowserCommand::SetDownloadDestination {
                id: checked_download_id(id)?,
                path: path.map(checked_download_path).transpose()?,
            },
        )
    }

    pub fn cancel_download(&self, id: &str) -> Result<(), BrowserCommandError> {
        send_command(
            &self.commands,
            BrowserCommand::CancelDownload(checked_download_id(id)?),
        )
    }

    pub fn pause_download(&self, id: &str) -> Result<(), BrowserCommandError> {
        send_command(
            &self.commands,
            BrowserCommand::PauseDownload(checked_download_id(id)?),
        )
    }

    pub fn resume_download(&self, id: &str) -> Result<(), BrowserCommandError> {
        send_command(
            &self.commands,
            BrowserCommand::ResumeDownload(checked_download_id(id)?),
        )
    }

    pub fn open_download(&self, id: &str) -> Result<(), BrowserCommandError> {
        send_command(
            &self.commands,
            BrowserCommand::OpenDownload(checked_download_id(id)?),
        )
    }

    pub fn remove_download(&self, id: &str) -> Result<(), BrowserCommandError> {
        send_command(
            &self.commands,
            BrowserCommand::RemoveDownload(checked_download_id(id)?),
        )
    }

    pub fn show_download_in_folder(&self, id: &str) -> Result<(), BrowserCommandError> {
        send_command(
            &self.commands,
            BrowserCommand::ShowDownloadInFolder(checked_download_id(id)?),
        )
    }

    pub fn show_downloads_folder(&self) -> Result<(), BrowserCommandError> {
        send_command(&self.commands, BrowserCommand::ShowDownloadsFolder)
    }

    pub fn try_recv_event(&self) -> Result<Option<BrowserEvent>, BrowserCommandError> {
        try_recv_browser_event(&self.events, &self.latest_frame)
    }

    pub fn shutdown(&mut self) {
        self.shutdown_requested.store(true, Ordering::Release);
        self.agent_bridge.shutdown();
        let _ = self.commands.try_send(BrowserCommand::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for BrowserSession {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[must_use]
pub fn resolve_browser_binary() -> Option<PathBuf> {
    if let Some(configured) = std::env::var_os("CODEX_RS_BROWSER_BIN")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .filter(|path| is_browser_executable(path))
    {
        return Some(configured);
    }

    #[cfg(windows)]
    {
        let relative_candidates = [
            Path::new("Google")
                .join("Chrome")
                .join("Application")
                .join("chrome.exe"),
            Path::new("Microsoft")
                .join("Edge")
                .join("Application")
                .join("msedge.exe"),
            Path::new("Chromium").join("Application").join("chrome.exe"),
        ];
        let roots = [
            std::env::var_os("ProgramW6432").map(PathBuf::from),
            std::env::var_os("ProgramFiles").map(PathBuf::from),
            std::env::var_os("ProgramFiles(x86)").map(PathBuf::from),
            std::env::var_os("LOCALAPPDATA").map(PathBuf::from),
        ];
        for relative in relative_candidates {
            if let Some(candidate) = roots
                .iter()
                .flatten()
                .map(|root| root.join(&relative))
                .find(|candidate| candidate.is_file())
            {
                return Some(candidate);
            }
        }
        find_path_executable(&["chrome.exe", "msedge.exe", "chromium.exe"])
    }

    #[cfg(target_os = "linux")]
    {
        find_path_executable(&[
            "google-chrome-stable",
            "google-chrome",
            "chromium",
            "chromium-browser",
            "microsoft-edge-stable",
            "microsoft-edge",
        ])
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    {
        None
    }
}

#[must_use]
pub fn default_browser_download_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .map(|path| path.join("Downloads"))
    }

    #[cfg(target_os = "linux")]
    {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute());
        linux_user_dirs_config_path(
            std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .as_deref(),
            home.as_deref(),
        )
        .and_then(|path| read_xdg_download_dir(&path, home.as_deref()))
        .or_else(|| home.map(|path| path.join("Downloads")))
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    {
        None
    }
}

#[cfg(any(target_os = "linux", test))]
fn linux_user_dirs_config_path(config_home: Option<&Path>, home: Option<&Path>) -> Option<PathBuf> {
    config_home
        .filter(|path| path.is_absolute())
        .map(|path| path.join("user-dirs.dirs"))
        .or_else(|| home.map(|path| path.join(".config").join("user-dirs.dirs")))
}

#[cfg(any(target_os = "linux", test))]
fn read_xdg_download_dir(path: &Path, home: Option<&Path>) -> Option<PathBuf> {
    if !fs::metadata(path).ok()?.is_file() {
        return None;
    }
    let file = File::open(path).ok()?;
    let mut contents = Vec::with_capacity(MAX_XDG_USER_DIRS_BYTES.min(4 * 1024));
    file.take((MAX_XDG_USER_DIRS_BYTES + 1) as u64)
        .read_to_end(&mut contents)
        .ok()?;
    if contents.len() > MAX_XDG_USER_DIRS_BYTES {
        return None;
    }
    parse_xdg_download_dir(&contents, home)
}

#[cfg(any(target_os = "linux", test))]
fn parse_xdg_download_dir(contents: &[u8], home: Option<&Path>) -> Option<PathBuf> {
    let contents = std::str::from_utf8(contents).ok()?;
    let mut offset = 0_usize;
    for raw_line in contents.split_inclusive('\n') {
        let line = raw_line.trim_end_matches('\n');
        let leading = line.len().saturating_sub(line.trim_start().len());
        let line = &line[leading..];
        let Some(value) = line.strip_prefix("XDG_DOWNLOAD_DIR=") else {
            offset = offset.saturating_add(raw_line.len());
            continue;
        };
        let value_offset = offset + leading + line.len().saturating_sub(value.len());
        let raw_value = &contents[value_offset..];
        let expands_home = raw_value.starts_with("\"$HOME\"") || raw_value.starts_with("\"$HOME/");
        let value = decode_xdg_double_quoted(raw_value)?;
        let path = if expands_home && value == "$HOME" {
            home?.to_path_buf()
        } else if expands_home && value.starts_with("$HOME/") {
            let relative = value.strip_prefix("$HOME/")?;
            home?.join(relative)
        } else {
            PathBuf::from(value)
        };
        return safe_absolute_path(path);
    }
    None
}

#[cfg(any(target_os = "linux", test))]
fn decode_xdg_double_quoted(value: &str) -> Option<String> {
    let mut characters = value.chars();
    (characters.next()? == '"').then_some(())?;
    let mut decoded = String::with_capacity(value.len().min(256));
    while let Some(character) = characters.next() {
        match character {
            '"' => {
                let trailing = characters
                    .by_ref()
                    .take_while(|character| *character != '\n')
                    .collect::<String>();
                let trailing = trailing.trim();
                return (trailing.is_empty() || trailing.starts_with('#')).then_some(decoded);
            }
            '\\' => match characters.next()? {
                escaped @ ('\\' | '"' | '$' | '`') => decoded.push(escaped),
                '\n' => {}
                '\r' if characters.next()? == '\n' => {}
                _ => return None,
            },
            character if character == '\n' || character == '\r' || character.is_control() => {
                return None;
            }
            character => decoded.push(character),
        }
    }
    None
}

#[cfg(any(target_os = "linux", test))]
fn safe_absolute_path(path: PathBuf) -> Option<PathBuf> {
    path.is_absolute()
        .then_some(path)
        .filter(|path| {
            !path
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
        })
        .filter(|path| {
            path.to_str()
                .is_some_and(|value| !value.chars().any(char::is_control))
        })
}

fn find_path_executable(names: &[&str]) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    find_path_executable_in(std::env::split_paths(&path), names)
}

fn find_path_executable_in(
    directories: impl IntoIterator<Item = PathBuf>,
    names: &[&str],
) -> Option<PathBuf> {
    for directory in directories {
        if let Some(candidate) = names
            .iter()
            .map(|name| directory.join(name))
            .find(|candidate| is_browser_executable(candidate))
        {
            return Some(candidate);
        }
    }
    None
}

fn is_browser_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        path.metadata()
            .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
    }

    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

fn send_command(
    sender: &Sender<BrowserCommand>,
    command: BrowserCommand,
) -> Result<(), BrowserCommandError> {
    sender.try_send(command).map_err(|error| match error {
        TrySendError::Full(_) => BrowserCommandError::QueueFull,
        TrySendError::Disconnected(_) => BrowserCommandError::Disconnected,
    })
}

fn checked_tab_id(tab_id: &str) -> Result<String, BrowserCommandError> {
    let tab_id = tab_id.trim();
    if tab_id.is_empty() || tab_id.len() > 256 || tab_id.chars().any(char::is_control) {
        return Err(BrowserCommandError::InvalidTab);
    }
    Ok(tab_id.to_owned())
}

fn checked_context_id(context_id: &str) -> Result<String, BrowserCommandError> {
    let context_id = context_id.trim();
    if context_id.is_empty()
        || context_id.len() > MAX_BROWSER_CONTEXT_ID_BYTES
        || context_id.chars().any(char::is_control)
    {
        return Err(BrowserCommandError::InvalidTab);
    }
    Ok(context_id.to_owned())
}

fn checked_download_id(id: &str) -> Result<String, BrowserCommandError> {
    let id = id.trim();
    if id.is_empty() || id.len() > MAX_BROWSER_DOWNLOAD_ID_BYTES || id.chars().any(char::is_control)
    {
        return Err(BrowserCommandError::InvalidInput);
    }
    Ok(id.to_owned())
}

fn checked_download_path(path: &Path) -> Result<PathBuf, BrowserCommandError> {
    let Some(value) = path.to_str() else {
        return Err(BrowserCommandError::InvalidPath);
    };
    if !path.is_absolute()
        || path.as_os_str().is_empty()
        || value.len() > MAX_BROWSER_DOWNLOAD_PATH_BYTES
        || value.contains('\0')
    {
        return Err(BrowserCommandError::InvalidPath);
    }
    Ok(path.to_path_buf())
}

fn checked_navigation_url(url: &str) -> Result<String, BrowserCommandError> {
    let url = url.trim();
    if url.len() > MAX_BROWSER_URL_BYTES || url.chars().any(char::is_control) {
        return Err(BrowserCommandError::InvalidUrl);
    }
    if url.is_empty() || url.eq_ignore_ascii_case("about:blank") {
        return Ok("about:blank".to_owned());
    }
    let Some((scheme, remainder)) = url.split_once("://") else {
        return Err(BrowserCommandError::InvalidUrl);
    };
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return Err(BrowserCommandError::InvalidUrl);
    }
    let authority = remainder
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .trim();
    if authority.is_empty() || authority.chars().any(char::is_whitespace) {
        return Err(BrowserCommandError::InvalidUrl);
    }
    Ok(url.to_owned())
}

#[must_use]
pub fn normalize_browser_origin(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > MAX_BROWSER_URL_BYTES
        || value.chars().any(char::is_control)
    {
        return None;
    }
    let candidate = if value.contains("://") {
        value.to_owned()
    } else {
        format!("https://{value}")
    };
    let url = Url::parse(&candidate).ok()?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return None;
    }
    let origin = url.origin().ascii_serialization();
    (origin != "null" && origin.len() <= MAX_BROWSER_URL_BYTES).then_some(origin)
}

#[must_use]
pub fn browser_permission_for_url(
    permissions: &BrowserPermissionsState,
    url: &str,
    resource: BrowserPermissionResource,
) -> BrowserPermissionValue {
    let Some((origin, host)) = browser_origin_parts(url) else {
        return BrowserPermissionValue::Default;
    };
    permissions.permission_matching(resource, |pattern| {
        browser_origin_pattern_matches(pattern, &origin, &host)
    })
}

fn browser_origin_parts(value: &str) -> Option<(String, String)> {
    let url = Url::parse(value).ok()?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return None;
    }
    let origin = url.origin().ascii_serialization();
    let host = origin.split_once("://")?.1.to_owned();
    Some((origin, host))
}

fn browser_origin_pattern_matches(pattern: &str, origin: &str, host: &str) -> bool {
    let target = if pattern.contains("://") {
        origin
    } else {
        host
    };
    wildcard_pattern_matches(pattern, target)
}

fn wildcard_pattern_matches(pattern: &str, value: &str) -> bool {
    #[derive(Clone, Copy)]
    enum Token {
        Star,
        Literal(char),
    }

    let mut tokens = Vec::with_capacity(pattern.chars().count());
    let mut characters = pattern.chars();
    while let Some(character) = characters.next() {
        match character {
            '*' => tokens.push(Token::Star),
            '\\' => tokens.push(Token::Literal(characters.next().unwrap_or('\\'))),
            character => tokens.push(Token::Literal(character)),
        }
    }
    let value = value.chars().collect::<Vec<_>>();
    let (mut token_index, mut value_index) = (0, 0);
    let mut last_star = None;
    let mut last_star_value_index = 0;
    while value_index < value.len() {
        match tokens.get(token_index) {
            Some(Token::Literal(expected)) if *expected == value[value_index] => {
                token_index += 1;
                value_index += 1;
            }
            Some(Token::Star) => {
                last_star = Some(token_index);
                token_index += 1;
                last_star_value_index = value_index;
            }
            _ => {
                let Some(star_index) = last_star else {
                    return false;
                };
                last_star_value_index += 1;
                value_index = last_star_value_index;
                token_index = star_index + 1;
            }
        }
    }
    tokens[token_index..]
        .iter()
        .all(|token| matches!(token, Token::Star))
}

fn run_browser(
    executable: PathBuf,
    config: BrowserConfig,
    download_dir: PathBuf,
    download_staging_root: PathBuf,
    commands: Receiver<BrowserCommand>,
    ui_events: BrowserUiEvents,
    agent_events: Sender<BrowserAgentNotification>,
) -> Result<(), String> {
    let shutdown_requested = Arc::clone(&ui_events.shutdown_requested);
    let browser_family = BrowserFamily::from_executable(&executable);
    let marker_before = read_devtools_marker(&config.profile_dir).ok();
    let mut command = browser_command(&executable, &config);
    let mut child = command
        .spawn()
        .map_err(|error| format!("could not launch the native Browser: {error}"))?;
    let job = create_browser_job(&mut child)?;
    let mut pending_commands = VecDeque::with_capacity(BROWSER_COMMAND_CAPACITY);
    let mut runtime = None;
    let result = (|| {
        let Some(endpoint) = wait_for_devtools_endpoint(
            &mut child,
            &config.profile_dir,
            marker_before.as_ref(),
            &commands,
            &shutdown_requested,
            &mut pending_commands,
        )?
        else {
            return Ok(());
        };
        if shutdown_requested.load(Ordering::Acquire) {
            return Ok(());
        }
        runtime = Some(BrowserRuntime::connect(
            endpoint,
            browser_family,
            &config,
            download_dir,
            download_staging_root,
            ui_events,
            agent_events,
        )?);
        if shutdown_requested.load(Ordering::Acquire) {
            return Ok(());
        }
        let context_id =
            checked_context_id(&config.context_id).map_err(|error| error.to_string())?;
        let Some(runtime) = runtime.as_mut() else {
            return Err("Browser runtime was unavailable during bootstrap".to_owned());
        };
        runtime.bootstrap(&context_id, config.initial_url.as_deref())?;
        if shutdown_requested.load(Ordering::Acquire) {
            return Ok(());
        }
        runtime.emit(BrowserEvent::Ready {
            executable: executable.clone(),
        });
        runtime.emit_tabs(&context_id);
        runtime.run(&mut child, &commands, &shutdown_requested, pending_commands)
    })();
    graceful_browser_exit(&mut child, runtime.as_mut().map(|runtime| &mut runtime.cdp));
    release_browser_job(job);
    if shutdown_requested.load(Ordering::Acquire) {
        Ok(())
    } else {
        result
    }
}

fn browser_command(executable: &Path, config: &BrowserConfig) -> Command {
    let mut command = Command::new(executable);
    let mut profile = OsString::from("--user-data-dir=");
    profile.push(&config.profile_dir);
    command
        .arg(profile)
        .args([
            "--headless=new",
            "--remote-debugging-address=127.0.0.1",
            "--remote-debugging-port=0",
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-default-apps",
            "--hide-scrollbars",
        ])
        .arg(format!(
            "--window-size={},{}",
            config.viewport_width, config.viewport_height
        ))
        .arg("about:blank")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    #[cfg(unix)]
    command.process_group(0);
    command
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DevToolsEndpoint {
    port: u16,
    path: String,
}

impl DevToolsEndpoint {
    fn websocket_url(&self) -> String {
        format!("ws://127.0.0.1:{}{}", self.port, self.path)
    }
}

fn wait_for_devtools_endpoint(
    child: &mut Child,
    profile_dir: &Path,
    marker_before: Option<&DevToolsEndpoint>,
    commands: &Receiver<BrowserCommand>,
    shutdown_requested: &AtomicBool,
    pending_commands: &mut VecDeque<BrowserCommand>,
) -> Result<Option<DevToolsEndpoint>, String> {
    let deadline = Instant::now() + DEVTOOLS_PORT_TIMEOUT;
    loop {
        if shutdown_requested.load(Ordering::Acquire) {
            return Ok(None);
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("could not inspect Browser startup: {error}"))?
        {
            return Err(format!(
                "the native Browser exited during startup ({status})"
            ));
        }
        while let Ok(command) = commands.try_recv() {
            if matches!(command, BrowserCommand::Shutdown) {
                return Ok(None);
            }
            if pending_commands.len() < BROWSER_COMMAND_CAPACITY {
                pending_commands.push_back(command);
            }
        }
        if let Ok(endpoint) = read_devtools_marker(profile_dir)
            && marker_before != Some(&endpoint)
        {
            return Ok(Some(endpoint));
        }
        if Instant::now() >= deadline {
            return Err("timed out waiting for the native Browser debugging endpoint".to_owned());
        }
        thread::sleep(BROWSER_TICK);
    }
}

fn next_browser_command(
    shutdown_requested: &AtomicBool,
    commands: &Receiver<BrowserCommand>,
    pending_commands: &mut VecDeque<BrowserCommand>,
) -> Result<Option<BrowserCommand>, TryRecvError> {
    if shutdown_requested.load(Ordering::Acquire) {
        return Ok(Some(BrowserCommand::Shutdown));
    }
    if let Some(command) = pending_commands.pop_front() {
        return Ok(Some(command));
    }
    match commands.try_recv() {
        Ok(command) => Ok(Some(command)),
        Err(TryRecvError::Empty) => Ok(None),
        Err(error @ TryRecvError::Disconnected) => Err(error),
    }
}

fn read_devtools_marker(profile_dir: &Path) -> Result<DevToolsEndpoint, String> {
    let path = profile_dir.join("DevToolsActivePort");
    let mut file =
        File::open(path).map_err(|error| format!("could not open DevToolsActivePort: {error}"))?;
    let mut bytes = Vec::with_capacity(MAX_DEVTOOLS_ACTIVE_PORT_BYTES as usize);
    file.by_ref()
        .take(MAX_DEVTOOLS_ACTIVE_PORT_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("could not read DevToolsActivePort: {error}"))?;
    if bytes.len() == MAX_DEVTOOLS_ACTIVE_PORT_BYTES as usize {
        return Err("DevToolsActivePort exceeded its size limit".to_owned());
    }
    let text =
        std::str::from_utf8(&bytes).map_err(|_| "DevToolsActivePort was not UTF-8".to_owned())?;
    parse_devtools_marker(text)
}

fn parse_devtools_marker(text: &str) -> Result<DevToolsEndpoint, String> {
    let mut lines = text.lines();
    let port = lines
        .next()
        .ok_or_else(|| "DevToolsActivePort did not contain a port".to_owned())?
        .trim()
        .parse::<u16>()
        .map_err(|_| "DevToolsActivePort contained an invalid port".to_owned())?;
    if port == 0 {
        return Err("DevToolsActivePort contained an invalid port".to_owned());
    }
    let path = lines
        .next()
        .ok_or_else(|| "DevToolsActivePort did not contain a WebSocket path".to_owned())?
        .trim();
    if path.len() > MAX_DEVTOOLS_PATH_BYTES
        || !path.starts_with("/devtools/browser/")
        || path.chars().any(char::is_control)
    {
        return Err("DevToolsActivePort contained an invalid WebSocket path".to_owned());
    }
    Ok(DevToolsEndpoint {
        port,
        path: path.to_owned(),
    })
}

#[cfg(windows)]
type BrowserJob = Job;

#[cfg(unix)]
struct BrowserJob {
    process_group: Pid,
}

#[cfg(unix)]
impl Drop for BrowserJob {
    fn drop(&mut self) {
        let _ = kill_process_group(self.process_group, Signal::KILL);
    }
}

#[cfg(not(any(windows, unix)))]
struct BrowserJob;

fn release_browser_job(_job: BrowserJob) {}

#[cfg(windows)]
fn create_browser_job(child: &mut Child) -> Result<BrowserJob, String> {
    let mut limits = ExtendedLimitInfo::new();
    limits.limit_kill_on_job_close();
    let job = Job::create_with_limit_info(&limits)
        .map_err(|error| format!("could not create a Browser Job Object: {error}"))?;
    if let Err(error) = job.assign_process(child.as_raw_handle() as isize) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(format!(
            "could not assign the Browser to its Job Object: {error}"
        ));
    }
    Ok(job)
}

#[cfg(unix)]
fn create_browser_job(child: &mut Child) -> Result<BrowserJob, String> {
    let process_group = i32::try_from(child.id())
        .ok()
        .and_then(Pid::from_raw)
        .ok_or_else(|| "Browser child did not expose a process id".to_owned())?;
    Ok(BrowserJob { process_group })
}

#[cfg(not(any(windows, unix)))]
fn create_browser_job(_child: &mut Child) -> Result<BrowserJob, String> {
    Ok(BrowserJob)
}

struct CdpClient {
    socket: WebSocket<TcpStream>,
    next_id: u64,
    pending_events: PendingCdpEvents,
    shutdown_requested: Arc<AtomicBool>,
}

struct PendingCdpEvent {
    value: Value,
    frame_bytes: usize,
}

struct PendingCdpEvents {
    events: VecDeque<PendingCdpEvent>,
    retained_bytes: usize,
}

impl PendingCdpEvents {
    fn new() -> Self {
        Self {
            events: VecDeque::with_capacity(BROWSER_EVENT_CAPACITY),
            retained_bytes: 0,
        }
    }

    fn push(&mut self, value: Value, frame_bytes: usize) -> Vec<Value> {
        let mut evicted = Vec::new();
        if frame_bytes > MAX_CDP_PENDING_EVENT_BYTES {
            evicted.push(value);
            return evicted;
        }
        while self.events.len() >= BROWSER_EVENT_CAPACITY
            || self.retained_bytes.saturating_add(frame_bytes) > MAX_CDP_PENDING_EVENT_BYTES
        {
            let Some(event) = self.events.pop_front() else {
                break;
            };
            self.retained_bytes = self.retained_bytes.saturating_sub(event.frame_bytes);
            evicted.push(event.value);
        }
        self.retained_bytes = self.retained_bytes.saturating_add(frame_bytes);
        self.events
            .push_back(PendingCdpEvent { value, frame_bytes });
        evicted
    }

    fn pop(&mut self) -> Option<Value> {
        let event = self.events.pop_front()?;
        self.retained_bytes = self.retained_bytes.saturating_sub(event.frame_bytes);
        Some(event.value)
    }
}

impl CdpClient {
    fn connect(
        endpoint: &DevToolsEndpoint,
        shutdown_requested: &Arc<AtomicBool>,
    ) -> Result<Self, String> {
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), endpoint.port);
        let stream = connect_cdp_stream(address, shutdown_requested)?;
        stream
            .set_write_timeout(Some(CDP_REQUEST_TIMEOUT))
            .map_err(|error| format!("could not configure Browser writes: {error}"))?;
        stream
            .set_nonblocking(true)
            .map_err(|error| format!("could not configure Browser reads: {error}"))?;
        let config = WebSocketConfig::default()
            .max_message_size(Some(MAX_CDP_MESSAGE_BYTES))
            .max_frame_size(Some(MAX_CDP_MESSAGE_BYTES));
        let mut socket = match client_with_config(endpoint.websocket_url(), stream, Some(config)) {
            Ok((mut socket, _)) => {
                socket
                    .get_mut()
                    .set_nonblocking(false)
                    .map_err(|error| format!("could not configure Browser reads: {error}"))?;
                socket
            }
            Err(HandshakeError::Interrupted(handshake)) => {
                handshake_cdp_client(handshake, shutdown_requested)?
            }
            Err(HandshakeError::Failure(error)) => {
                return Err(format!(
                    "could not open the Browser debugging channel: {error}"
                ));
            }
        };
        socket
            .get_mut()
            .set_read_timeout(Some(CDP_IO_TIMEOUT))
            .map_err(|error| format!("could not configure Browser event reads: {error}"))?;
        Ok(Self {
            socket,
            next_id: 1,
            pending_events: PendingCdpEvents::new(),
            shutdown_requested: Arc::clone(shutdown_requested),
        })
    }

    fn shutdown_requested(&self) -> bool {
        self.shutdown_requested.load(Ordering::Acquire)
    }

    fn request(
        &mut self,
        method: &str,
        params: Value,
        session_id: Option<&str>,
    ) -> Result<Value, String> {
        if self.shutdown_requested() {
            return Err(CDP_SHUTDOWN_REQUESTED.to_owned());
        }
        let id = self.send_request(method, params, session_id)?;

        let deadline = Instant::now() + CDP_REQUEST_TIMEOUT;
        loop {
            if self.shutdown_requested() {
                return Err(CDP_SHUTDOWN_REQUESTED.to_owned());
            }
            if Instant::now() >= deadline {
                return Err(format!("Browser command {method} timed out"));
            }
            match self.read_value()? {
                Some((value, _)) if value.get("id").and_then(Value::as_u64) == Some(id) => {
                    if let Some(error) = value.get("error") {
                        let message = error
                            .get("message")
                            .and_then(Value::as_str)
                            .unwrap_or("Browser command failed");
                        return Err(bounded_text(message, MAX_ERROR_BYTES));
                    }
                    return Ok(value.get("result").cloned().unwrap_or(Value::Null));
                }
                Some((value, frame_bytes)) if value.get("method").is_some() => {
                    let evicted = self.pending_events.push(value, frame_bytes);
                    self.acknowledge_evicted_screencast_frames(evicted)?;
                }
                Some(_) | None => {}
            }
        }
    }

    fn send_request(
        &mut self,
        method: &str,
        params: Value,
        session_id: Option<&str>,
    ) -> Result<u64, String> {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        let mut request = json!({
            "id": id,
            "method": method,
            "params": params,
        });
        if let Some(session_id) = session_id {
            request["sessionId"] = Value::String(session_id.to_owned());
        }
        let serialized = serde_json::to_string(&request)
            .map_err(|error| format!("could not encode Browser command: {error}"))?;
        if serialized.len() > MAX_CDP_MESSAGE_BYTES {
            return Err("Browser command exceeded its size limit".to_owned());
        }
        self.socket
            .send(Message::text(serialized))
            .map_err(|error| format!("could not send Browser command: {error}"))?;
        Ok(id)
    }

    fn acknowledge_evicted_screencast_frames(&mut self, events: Vec<Value>) -> Result<(), String> {
        for event in events {
            let Some((session_id, frame_session_id)) = screencast_frame_ack(&event) else {
                continue;
            };
            self.send_request(
                "Page.screencastFrameAck",
                json!({"sessionId": frame_session_id}),
                Some(session_id),
            )?;
        }
        Ok(())
    }

    fn close_browser(&mut self) {
        if self
            .socket
            .get_mut()
            .set_write_timeout(Some(CDP_IO_TIMEOUT))
            .is_err()
        {
            return;
        }
        let _ = self.send_request("Browser.close", json!({}), None);
        let _ = self
            .socket
            .get_mut()
            .set_write_timeout(Some(CDP_REQUEST_TIMEOUT));
    }
}

fn connect_cdp_stream(
    address: SocketAddr,
    shutdown_requested: &AtomicBool,
) -> Result<TcpStream, String> {
    let deadline = Instant::now() + CDP_CONNECT_TIMEOUT;
    loop {
        if shutdown_requested.load(Ordering::Acquire) {
            return Err(CDP_SHUTDOWN_REQUESTED.to_owned());
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(
                "could not connect to the Browser debugging endpoint: timed out".to_owned(),
            );
        }
        match TcpStream::connect_timeout(&address, remaining.min(CDP_IO_TIMEOUT)) {
            Ok(stream) => return Ok(stream),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) if error.kind() == io::ErrorKind::TimedOut => {}
            Err(error) => {
                return Err(format!(
                    "could not connect to the Browser debugging endpoint: {error}"
                ));
            }
        }
    }
}

fn handshake_cdp_client(
    mut handshake: tungstenite::handshake::MidHandshake<tungstenite::ClientHandshake<TcpStream>>,
    shutdown_requested: &AtomicBool,
) -> Result<WebSocket<TcpStream>, String> {
    let deadline = Instant::now() + CDP_CONNECT_TIMEOUT;
    loop {
        if shutdown_requested.load(Ordering::Acquire) {
            return Err(CDP_SHUTDOWN_REQUESTED.to_owned());
        }
        if Instant::now() >= deadline {
            return Err("could not open the Browser debugging channel: timed out".to_owned());
        }
        match handshake.handshake() {
            Ok((mut socket, _)) => {
                socket
                    .get_mut()
                    .set_nonblocking(false)
                    .map_err(|error| format!("could not configure Browser reads: {error}"))?;
                return Ok(socket);
            }
            Err(HandshakeError::Interrupted(next)) => {
                handshake = next;
                thread::sleep(CDP_IO_TIMEOUT);
            }
            Err(HandshakeError::Failure(error)) => {
                return Err(format!(
                    "could not open the Browser debugging channel: {error}"
                ));
            }
        }
    }
}

fn screencast_frame_ack(event: &Value) -> Option<(&str, u64)> {
    if event.get("method").and_then(Value::as_str) != Some("Page.screencastFrame") {
        return None;
    }
    Some((
        event.get("sessionId")?.as_str()?,
        event.get("params")?.get("sessionId")?.as_u64()?,
    ))
}

impl CdpClient {
    fn poll_event(&mut self) -> Result<Option<Value>, String> {
        if let Some(event) = self.pending_events.pop() {
            return Ok(Some(event));
        }
        loop {
            match self.read_value()? {
                Some((value, _)) if value.get("method").is_some() => return Ok(Some(value)),
                Some(_) => {}
                None => return Ok(None),
            }
        }
    }

    fn read_value(&mut self) -> Result<Option<(Value, usize)>, String> {
        match self.socket.read() {
            Ok(Message::Text(text)) => {
                if text.len() > MAX_CDP_MESSAGE_BYTES {
                    return Err("Browser event exceeded its size limit".to_owned());
                }
                let frame_bytes = text.len();
                serde_json::from_str(text.as_str())
                    .map(|value| Some((value, frame_bytes)))
                    .map_err(|error| format!("could not decode Browser event: {error}"))
            }
            Ok(Message::Ping(payload)) => {
                self.socket
                    .send(Message::Pong(payload))
                    .map_err(|error| format!("could not answer Browser heartbeat: {error}"))?;
                Ok(None)
            }
            Ok(Message::Pong(_) | Message::Frame(_)) => Ok(None),
            Ok(Message::Close(_)) => Err("the Browser debugging channel closed".to_owned()),
            Ok(Message::Binary(_)) => Err("the Browser sent an unexpected binary event".to_owned()),
            Err(WebSocketError::Io(error))
                if matches!(
                    error.kind(),
                    io::ErrorKind::TimedOut
                        | io::ErrorKind::WouldBlock
                        | io::ErrorKind::Interrupted
                ) =>
            {
                Ok(None)
            }
            Err(WebSocketError::ConnectionClosed | WebSocketError::AlreadyClosed) => {
                Err("the Browser debugging channel closed".to_owned())
            }
            Err(error) => Err(format!("could not read Browser event: {error}")),
        }
    }
}

fn cdp_runtime_value(
    cdp: &mut CdpClient,
    session_id: &str,
    expression: &str,
) -> Result<Value, String> {
    if expression.len() > MAX_CDP_MESSAGE_BYTES {
        return Err("Browser Downloads command exceeded its size limit".to_owned());
    }
    let result = cdp.request(
        "Runtime.evaluate",
        json!({
            "awaitPromise": true,
            "expression": expression,
            "returnByValue": true,
        }),
        Some(session_id),
    )?;
    if let Some(description) = result
        .pointer("/exceptionDetails/exception/description")
        .and_then(Value::as_str)
    {
        return Err(bounded_text(description, MAX_ERROR_BYTES));
    }
    result
        .pointer("/result/value")
        .cloned()
        .ok_or_else(|| "Browser Downloads did not return a value".to_owned())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BrowserTabOrigin {
    User,
    Agent,
}

struct BrowserTabMark {
    status: String,
    turn_id: String,
}

struct BrowserTabRuntime {
    agent_id: u64,
    context_id: String,
    origin: BrowserTabOrigin,
    mark: Option<BrowserTabMark>,
    public: BrowserTab,
    session_id: String,
}

fn browser_agent_tab_matches(tab: &BrowserTabRuntime, context_id: &str, agent_tab_id: u64) -> bool {
    tab.context_id == context_id && tab.agent_id == agent_tab_id
}

fn cached_agent_expression_key(
    context_id: String,
    agent_tab_id: u64,
    cache_key: String,
) -> (String, u64, String) {
    (context_id, agent_tab_id, cache_key)
}

struct BrowserAgentChildSession {
    agent_tab_id: u64,
    target_id: String,
    parent_session_id: String,
}

fn agent_tab_id_for_cdp_session<'a>(
    mut root_sessions: impl Iterator<Item = (&'a str, u64)>,
    child_sessions: &HashMap<String, BrowserAgentChildSession>,
    session_id: &str,
) -> Option<u64> {
    root_sessions
        .find_map(|(root_session_id, agent_tab_id)| {
            (root_session_id == session_id).then_some(agent_tab_id)
        })
        .or_else(|| {
            child_sessions
                .get(session_id)
                .map(|child| child.agent_tab_id)
        })
}

fn record_agent_child_session<'a>(
    root_sessions: impl Iterator<Item = (&'a str, u64)>,
    child_sessions: &mut HashMap<String, BrowserAgentChildSession>,
    parent_session_id: &str,
    child_session_id: &str,
    target_id: &str,
) {
    let Some(agent_tab_id) =
        agent_tab_id_for_cdp_session(root_sessions, child_sessions, parent_session_id)
    else {
        return;
    };
    if !is_bounded_agent_identifier(child_session_id)
        || !is_bounded_agent_identifier(target_id)
        || child_session_id == parent_session_id
        || child_sessions.contains_key(child_session_id)
        || child_sessions.len() >= MAX_AGENT_CHILD_SESSIONS
    {
        return;
    }
    child_sessions.insert(
        child_session_id.to_owned(),
        BrowserAgentChildSession {
            agent_tab_id,
            target_id: target_id.to_owned(),
            parent_session_id: parent_session_id.to_owned(),
        },
    );
}

fn remove_agent_child_session<'a>(
    root_sessions: impl Iterator<Item = (&'a str, u64)>,
    child_sessions: &mut HashMap<String, BrowserAgentChildSession>,
    parent_session_id: &str,
    child_session_id: &str,
) -> bool {
    let Some(parent_agent_tab_id) =
        agent_tab_id_for_cdp_session(root_sessions, child_sessions, parent_session_id)
    else {
        return false;
    };
    let Some(child) = child_sessions.get(child_session_id) else {
        return false;
    };
    if child.parent_session_id != parent_session_id || child.agent_tab_id != parent_agent_tab_id {
        return false;
    }

    let mut removed_sessions = VecDeque::from([child_session_id.to_owned()]);
    while let Some(parent_session_id) = removed_sessions.pop_front() {
        child_sessions.remove(&parent_session_id);
        removed_sessions.extend(
            child_sessions
                .iter()
                .filter(|(_, child)| child.parent_session_id == parent_session_id)
                .map(|(session_id, _)| session_id.to_owned()),
        );
    }
    true
}

enum BrowserDownloadGrantSource {
    Agent {
        agent_tab_id: u64,
        expected_url: String,
    },
    User,
}

struct BrowserDownloadGrant {
    context_id: String,
    directory: PathBuf,
    expires_at: Instant,
    frame_ids: HashSet<String>,
    source: BrowserDownloadGrantSource,
}

struct BrowserDownloadRuntime {
    agent_session: bool,
    can_resume: bool,
    completed_source: Option<PathBuf>,
    context_id: String,
    default_destination: PathBuf,
    destination: Option<PathBuf>,
    overwrite_destination: bool,
    paused: bool,
    received_bytes: u64,
    save_prompt_pending: bool,
    staging_directory: PathBuf,
    staging_path: PathBuf,
    started_at_ms: u64,
    total_bytes: u64,
    url: String,
    user_initiated: bool,
}

struct BrowserRuntime {
    browser_family: BrowserFamily,
    cdp: CdpClient,
    ui_events: BrowserUiEvents,
    agent_events: Sender<BrowserAgentNotification>,
    tabs: Vec<BrowserTabRuntime>,
    active_tab_ids: HashMap<String, String>,
    agent_child_sessions: HashMap<String, BrowserAgentChildSession>,
    cached_agent_expressions: HashMap<(String, u64, String), String>,
    cached_agent_expression_order: VecDeque<(String, u64, String)>,
    active_context_id: String,
    active_index: usize,
    next_agent_tab_id: u64,
    permissions: BrowserPermissionsState,
    download_dir: PathBuf,
    download_staging_root: PathBuf,
    prompt_for_user_downloads: bool,
    next_download_directory_id: u64,
    pending_download_grant: Option<BrowserDownloadGrant>,
    downloads: HashMap<String, BrowserDownloadRuntime>,
    download_history: VecDeque<BrowserDownload>,
    reserved_download_paths: HashSet<PathBuf>,
    download_behavior_enabled: Option<bool>,
    surface_context_id: Option<String>,
    surface_visible: bool,
    pending_visibility_context_ids: HashSet<String>,
    surface_viewport_width: u32,
    surface_viewport_height: u32,
    viewport_overridden: bool,
    viewport_width: u32,
    viewport_height: u32,
}

impl BrowserRuntime {
    fn connect(
        endpoint: DevToolsEndpoint,
        browser_family: BrowserFamily,
        config: &BrowserConfig,
        download_dir: PathBuf,
        download_staging_root: PathBuf,
        ui_events: BrowserUiEvents,
        agent_events: Sender<BrowserAgentNotification>,
    ) -> Result<Self, String> {
        Ok(Self {
            browser_family,
            cdp: CdpClient::connect(&endpoint, &ui_events.shutdown_requested)?,
            ui_events,
            agent_events,
            tabs: Vec::with_capacity(4),
            active_tab_ids: HashMap::new(),
            agent_child_sessions: HashMap::new(),
            cached_agent_expressions: HashMap::new(),
            cached_agent_expression_order: VecDeque::new(),
            active_context_id: String::new(),
            active_index: 0,
            next_agent_tab_id: 1,
            permissions: config.permissions.clone().normalized(),
            download_dir,
            download_staging_root,
            prompt_for_user_downloads: config.prompt_for_user_downloads,
            next_download_directory_id: 1,
            pending_download_grant: None,
            downloads: HashMap::new(),
            download_history: VecDeque::with_capacity(MAX_BROWSER_DOWNLOAD_HISTORY),
            reserved_download_paths: HashSet::new(),
            download_behavior_enabled: None,
            surface_context_id: None,
            surface_visible: false,
            pending_visibility_context_ids: HashSet::new(),
            surface_viewport_width: config
                .viewport_width
                .clamp(MIN_VIEWPORT_WIDTH, MAX_VIEWPORT_WIDTH),
            surface_viewport_height: config
                .viewport_height
                .clamp(MIN_VIEWPORT_HEIGHT, MAX_VIEWPORT_HEIGHT),
            viewport_overridden: false,
            viewport_width: config
                .viewport_width
                .clamp(MIN_VIEWPORT_WIDTH, MAX_VIEWPORT_WIDTH),
            viewport_height: config
                .viewport_height
                .clamp(MIN_VIEWPORT_HEIGHT, MAX_VIEWPORT_HEIGHT),
        })
    }

    fn bootstrap(&mut self, context_id: &str, initial_url: Option<&str>) -> Result<(), String> {
        self.cdp
            .request("Target.setDiscoverTargets", json!({"discover": true}), None)?;
        self.set_download_behavior(false, None)?;
        let targets = self.cdp.request("Target.getTargets", json!({}), None)?;
        let existing_target_id = targets
            .get("targetInfos")
            .and_then(Value::as_array)
            .and_then(|targets| {
                targets.iter().find_map(|target| {
                    (target.get("type").and_then(Value::as_str) == Some("page"))
                        .then(|| {
                            target
                                .get("targetId")
                                .and_then(Value::as_str)
                                .map(str::to_owned)
                        })
                        .flatten()
                })
            });
        let target_id = match existing_target_id {
            Some(target_id) => target_id,
            None => self.create_target("about:blank")?,
        };
        self.active_context_id = context_id.to_owned();
        self.attach_target(context_id, target_id, BrowserTabOrigin::User)?;
        self.active_tab_ids
            .insert(context_id.to_owned(), self.tabs[0].public.id.clone());
        self.start_screencast(0)?;
        if let Some(url) = initial_url {
            self.navigate(context_id, url)?;
        } else {
            self.refresh_tab(0)?;
        }
        Ok(())
    }

    fn run(
        &mut self,
        child: &mut Child,
        commands: &Receiver<BrowserCommand>,
        shutdown_requested: &AtomicBool,
        mut pending_commands: VecDeque<BrowserCommand>,
    ) -> Result<(), String> {
        loop {
            if shutdown_requested.load(Ordering::Acquire) {
                return Ok(());
            }
            self.expire_download_grant()?;
            if let Some(status) = child
                .try_wait()
                .map_err(|error| format!("could not inspect the Browser process: {error}"))?
            {
                return Err(format!("the native Browser exited unexpectedly ({status})"));
            }

            for _ in 0..BROWSER_COMMANDS_PER_TICK {
                let command =
                    match next_browser_command(shutdown_requested, commands, &mut pending_commands)
                    {
                        Ok(command) => command,
                        Err(TryRecvError::Disconnected) => return Ok(()),
                        Err(TryRecvError::Empty) => None,
                    };
                let Some(command) = command else {
                    break;
                };
                if !self.handle_command(command)? {
                    return Ok(());
                }
            }

            for _ in 0..BROWSER_EVENTS_PER_TICK {
                let Some(event) = self.cdp.poll_event()? else {
                    break;
                };
                self.handle_event(event)?;
            }
        }
    }

    fn handle_command(&mut self, command: BrowserCommand) -> Result<bool, String> {
        match command {
            BrowserCommand::ActivateContext(context_id) => {
                self.activate_context(&context_id)?;
            }
            BrowserCommand::Navigate { context_id, url } => self.navigate(&context_id, &url)?,
            BrowserCommand::Back(context_id) => {
                self.activate_context(&context_id)?;
                self.navigate_history(-1)?;
            }
            BrowserCommand::Forward(context_id) => {
                self.activate_context(&context_id)?;
                self.navigate_history(1)?;
            }
            BrowserCommand::Reload(context_id) => {
                self.activate_context(&context_id)?;
                let session_id = self.active_tab()?.session_id.clone();
                self.set_loading(true);
                self.cdp
                    .request("Page.reload", json!({}), Some(&session_id))?;
            }
            BrowserCommand::Stop(context_id) => {
                self.activate_context(&context_id)?;
                let session_id = self.active_tab()?.session_id.clone();
                self.cdp
                    .request("Page.stopLoading", json!({}), Some(&session_id))?;
                self.set_loading(false);
            }
            BrowserCommand::OpenTab { context_id, url } => {
                self.open_tab(&context_id, url.as_deref(), BrowserTabOrigin::User)?;
            }
            BrowserCommand::SelectTab { context_id, tab_id } => {
                self.select_tab(&context_id, &tab_id)?;
            }
            BrowserCommand::CloseTab { context_id, tab_id } => {
                self.close_tab(&context_id, &tab_id)?;
            }
            BrowserCommand::Resize { width, height } => self.resize(width, height)?,
            BrowserCommand::SyncSurfaceState {
                context_id,
                visible,
            } => self.sync_surface_state(context_id, visible),
            BrowserCommand::Click {
                context_id,
                x,
                y,
                button,
            } => self.click(&context_id, x, y, button)?,
            BrowserCommand::Scroll {
                context_id,
                x,
                y,
                delta_x,
                delta_y,
            } => self.scroll(&context_id, x, y, delta_x, delta_y)?,
            BrowserCommand::Key { context_id, input } => self.key(&context_id, &input)?,
            BrowserCommand::SetDownloadDirectory(path) => {
                if let Err(error) = self.set_download_directory(path) {
                    self.emit(BrowserEvent::OperationFailed(bounded_error(error)));
                }
            }
            BrowserCommand::SetPromptForUserDownloads(prompt) => {
                self.prompt_for_user_downloads = prompt;
            }
            BrowserCommand::SetPermissions(permissions) => {
                self.permissions = permissions.normalized();
            }
            BrowserCommand::SetDownloadDestination { id, path } => {
                if let Err(error) = self.set_download_destination(&id, path) {
                    self.emit(BrowserEvent::OperationFailed(bounded_error(error)));
                }
            }
            BrowserCommand::CancelDownload(id) => {
                if let Err(error) = self.cancel_download(&id) {
                    self.emit(BrowserEvent::OperationFailed(bounded_error(error)));
                }
            }
            BrowserCommand::PauseDownload(id) => {
                if let Err(error) = self.pause_download(&id) {
                    self.emit(BrowserEvent::OperationFailed(bounded_error(error)));
                }
            }
            BrowserCommand::ResumeDownload(id) => {
                if let Err(error) = self.resume_download(&id) {
                    self.emit(BrowserEvent::OperationFailed(bounded_error(error)));
                }
            }
            BrowserCommand::OpenDownload(id) => {
                if let Err(error) = self.open_download(&id) {
                    self.emit(BrowserEvent::OperationFailed(bounded_error(error)));
                }
            }
            BrowserCommand::RemoveDownload(id) => {
                if let Err(error) = self.remove_download(&id) {
                    self.emit(BrowserEvent::OperationFailed(bounded_error(error)));
                }
            }
            BrowserCommand::ShowDownloadInFolder(id) => {
                if let Err(error) = self.show_download_in_folder(&id) {
                    self.emit(BrowserEvent::OperationFailed(bounded_error(error)));
                }
            }
            BrowserCommand::ShowDownloadsFolder => {
                if let Err(error) = open_platform_path(&self.download_dir, false) {
                    self.emit(BrowserEvent::OperationFailed(bounded_error(error)));
                }
            }
            BrowserCommand::AgentRpc { request, response } => {
                let _ = response.try_send(self.handle_agent_request(request));
            }
            BrowserCommand::Shutdown => return Ok(false),
        }
        Ok(true)
    }

    fn handle_event(&mut self, event: Value) -> Result<(), String> {
        let method = event
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let session_id = event
            .get("sessionId")
            .and_then(Value::as_str)
            .map(str::to_owned);
        self.record_agent_target_event(&event);
        self.forward_agent_cdp_event(&event);
        match method {
            "Browser.downloadWillBegin" => {
                self.handle_download_will_begin(event.get("params").unwrap_or(&Value::Null))?;
            }
            "Browser.downloadProgress" => {
                self.handle_download_progress(event.get("params").unwrap_or(&Value::Null))?;
            }
            "Page.screencastFrame" => {
                if session_id.as_deref() != Some(self.active_tab()?.session_id.as_str()) {
                    return Ok(());
                }
                let params = event.get("params").unwrap_or(&Value::Null);
                let frame_session_id =
                    params
                        .get("sessionId")
                        .and_then(Value::as_u64)
                        .ok_or_else(|| {
                            "Browser frame did not include an acknowledgement id".to_owned()
                        })?;
                self.cdp.request(
                    "Page.screencastFrameAck",
                    json!({"sessionId": frame_session_id}),
                    session_id.as_deref(),
                )?;
                if let Some(data) = params.get("data").and_then(Value::as_str) {
                    self.emit_frame(data)?;
                }
            }
            "Page.frameNavigated" => {
                let params = event.get("params").unwrap_or(&Value::Null);
                let frame = params.get("frame").unwrap_or(&Value::Null);
                if frame.get("parentId").is_none()
                    && let Some(url) = frame.get("url").and_then(Value::as_str)
                    && let Some(index) = self.tab_index_for_session(session_id.as_deref())
                {
                    self.tabs[index].public.url = bounded_text(url, MAX_BROWSER_URL_BYTES);
                    let context_id = self.tabs[index].context_id.clone();
                    self.emit_tabs(&context_id);
                }
            }
            "Page.loadEventFired" => {
                if let Some(index) = self.tab_index_for_session(session_id.as_deref()) {
                    self.tabs[index].public.loading = false;
                    self.refresh_tab(index)?;
                    let context_id = self.tabs[index].context_id.clone();
                    self.emit_tabs(&context_id);
                }
            }
            "Page.lifecycleEvent" => {}
            "Target.targetInfoChanged" => {
                if let Some(info) = event
                    .get("params")
                    .and_then(|params| params.get("targetInfo"))
                    && let Some(target_id) = info.get("targetId").and_then(Value::as_str)
                    && let Some(index) = self.tab_index(target_id)
                {
                    if let Some(url) = info.get("url").and_then(Value::as_str) {
                        self.tabs[index].public.url = bounded_text(url, MAX_BROWSER_URL_BYTES);
                    }
                    if let Some(title) = info.get("title").and_then(Value::as_str) {
                        self.tabs[index].public.title =
                            bounded_text(title, MAX_BROWSER_TITLE_BYTES);
                    }
                    let context_id = self.tabs[index].context_id.clone();
                    self.emit_tabs(&context_id);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_agent_request(
        &mut self,
        request: BrowserAgentRequest,
    ) -> Result<Value, BrowserAgentRpcError> {
        match request.method.as_str() {
            "ping" => Ok(Value::String("pong".to_owned())),
            "getInfo" => self.agent_get_info(&request.params),
            "getTabs" => self.agent_get_tabs(&request.params),
            "getUserTabs" => self.agent_get_user_tabs(&request.params),
            "createTab" => self.agent_create_tab(&request.params),
            "claimUserTab" => self.agent_claim_user_tab(&request.params),
            "focusTab" => self.agent_focus_tab(&request.params),
            "nameSession" => self.agent_name_session(&request.params),
            "attach" | "detach" => self.agent_validate_tab(&request.params),
            "attachTarget" => self.agent_attach_target(&request.params),
            "detachTarget" => self.agent_detach_target(&request.params),
            "executeCdp" => self.agent_execute_cdp(&request.params),
            "executeCdpWithCachedExpression" => {
                self.agent_execute_cdp_with_cached_expression(&request.params)
            }
            "moveMouse" => self.agent_move_mouse(&request.params),
            "finalizeTabs" => self.agent_finalize_tabs(&request.params),
            "markTab" => self.agent_mark_tab(&request.params),
            "turnEnded" => self.agent_turn_ended(&request.params),
            "allowDownload" => self.agent_allow_download(&request.params),
            "getUserHistory" => Err(BrowserAgentRpcError::method_not_found("getUserHistory")),
            "executeUnhandledCommand" => self.agent_execute_unhandled_command(&request.params),
            method => Err(BrowserAgentRpcError::method_not_found(method)),
        }
    }

    fn agent_get_info(&self, params: &Value) -> Result<Value, BrowserAgentRpcError> {
        let context_id = self.agent_context_id(params)?;
        Ok(json!({
            "apiSupportOverrides": {
                "BrowserUser.claimTab": true,
                "Tab.markDeliverable": true,
                "Tab.markHandoff": true,
                "Tabs.finalize": true
            },
            "name": "Codex In-app Browser",
            "version": env!("CARGO_PKG_VERSION"),
            "type": "iab",
            "capabilities": {
                "browser": [
                    {
                        "id": "visibility",
                        "description": BROWSER_VISIBILITY_CAPABILITY_DESCRIPTION
                    },
                    {
                        "id": "viewport",
                        "description": BROWSER_VIEWPORT_CAPABILITY_DESCRIPTION
                    }
                ],
                "tab": [
                    {
                        "id": "pageAssets",
                        "description": BROWSER_PAGE_ASSETS_CAPABILITY_DESCRIPTION
                    }
                ]
            },
            "metadata": {
                "codexSessionId": context_id
            }
        }))
    }

    fn agent_execute_unhandled_command(
        &mut self,
        params: &Value,
    ) -> Result<Value, BrowserAgentRpcError> {
        let context_id = self.agent_context_id(params)?;
        checked_agent_browser_id(params)?;
        let command = params
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| BrowserAgentRpcError::request("Browser command requires a type"))?;
        match command {
            "browser_visibility_get" => Ok(json!({
                "visible": self.surface_context_id.as_deref() == Some(context_id.as_str())
                    && (self.surface_visible
                        || self.pending_visibility_context_ids.contains(&context_id))
            })),
            "browser_visibility_set" => {
                let visible = params
                    .get("visible")
                    .and_then(Value::as_bool)
                    .ok_or_else(|| {
                        BrowserAgentRpcError::request(
                            "Browser visibility command requires a boolean visible",
                        )
                    })?;
                self.ui_events
                    .control
                    .try_send(BrowserEvent::VisibilityRequested {
                        context_id: context_id.clone(),
                        visible,
                    })
                    .map_err(|_| {
                        BrowserAgentRpcError::request("Browser UI event queue is unavailable")
                    })?;
                if visible && self.surface_context_id.as_deref() == Some(context_id.as_str()) {
                    self.pending_visibility_context_ids.insert(context_id);
                } else {
                    self.pending_visibility_context_ids.remove(&context_id);
                    if self.surface_context_id.as_deref() == Some(context_id.as_str()) && !visible {
                        self.surface_visible = false;
                    }
                }
                Ok(json!({}))
            }
            "browser_viewport_set" => {
                let (width, height) = checked_agent_viewport(params)?;
                self.set_agent_viewport(width, height)
                    .map_err(BrowserAgentRpcError::request)?;
                Ok(json!({}))
            }
            "browser_viewport_reset" => {
                self.reset_agent_viewport()
                    .map_err(BrowserAgentRpcError::request)?;
                Ok(json!({}))
            }
            command => Err(BrowserAgentRpcError::request(format!(
                "Codex In-app Browser does not support command \"{command}\""
            ))),
        }
    }

    fn agent_get_tabs(&self, params: &Value) -> Result<Value, BrowserAgentRpcError> {
        let context_id = self.agent_context_id(params)?;
        Ok(Value::Array(
            self.tabs
                .iter()
                .filter(|tab| tab.context_id == context_id)
                .map(|tab| self.serialize_agent_tab(tab))
                .collect(),
        ))
    }

    fn agent_get_user_tabs(&self, params: &Value) -> Result<Value, BrowserAgentRpcError> {
        let context_id = self.agent_context_id(params)?;
        Ok(Value::Array(
            self.tabs
                .iter()
                .filter(|tab| tab.context_id == context_id && tab.origin == BrowserTabOrigin::User)
                .map(|tab| {
                    json!({
                        "id": tab.agent_id,
                        "providerTabId": tab.public.id,
                        "title": tab.public.title,
                        "url": tab.public.url,
                    })
                })
                .collect(),
        ))
    }

    fn agent_create_tab(&mut self, params: &Value) -> Result<Value, BrowserAgentRpcError> {
        let context_id = self.agent_context_id(params)?;
        self.open_tab(&context_id, None, BrowserTabOrigin::Agent)
            .map_err(BrowserAgentRpcError::request)?;
        let tab = self
            .tabs
            .get(self.active_index)
            .ok_or_else(|| BrowserAgentRpcError::request("Browser tab is no longer available"))?;
        Ok(self.serialize_agent_tab(tab))
    }

    fn agent_claim_user_tab(&mut self, params: &Value) -> Result<Value, BrowserAgentRpcError> {
        let context_id = self.agent_context_id(params)?;
        let agent_tab_id = agent_tab_id(params)?;
        let index = self
            .agent_tab_index(&context_id, agent_tab_id)
            .ok_or_else(|| BrowserAgentRpcError::request(format!("Unknown tab: {agent_tab_id}")))?;
        let target_id = self.tabs[index].public.id.clone();
        self.select_tab(&context_id, &target_id)
            .map_err(BrowserAgentRpcError::request)?;
        let tab = self
            .tabs
            .get(index)
            .ok_or_else(|| BrowserAgentRpcError::request("Browser tab is no longer available"))?;
        Ok(self.serialize_agent_tab(tab))
    }

    fn agent_focus_tab(&mut self, params: &Value) -> Result<Value, BrowserAgentRpcError> {
        let context_id = self.agent_context_id(params)?;
        let agent_tab_id = agent_tab_id(params)?;
        let index = self
            .agent_tab_index(&context_id, agent_tab_id)
            .ok_or_else(|| BrowserAgentRpcError::request(format!("Unknown tab: {agent_tab_id}")))?;
        let target_id = self.tabs[index].public.id.clone();
        self.select_tab(&context_id, &target_id)
            .map_err(BrowserAgentRpcError::request)?;
        Ok(Value::Null)
    }

    fn agent_name_session(&self, params: &Value) -> Result<Value, BrowserAgentRpcError> {
        self.agent_context_id(params)?;
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|name| {
                !name.is_empty()
                    && name.len() <= MAX_BROWSER_TITLE_BYTES
                    && !name.chars().any(char::is_control)
            })
            .ok_or_else(|| BrowserAgentRpcError::request("nameSession requires a name"))?;
        let _ = name;
        Ok(Value::Null)
    }

    fn agent_validate_tab(&self, params: &Value) -> Result<Value, BrowserAgentRpcError> {
        let context_id = self.agent_context_id(params)?;
        let agent_tab_id = agent_tab_id(params)?;
        if self.agent_tab_index(&context_id, agent_tab_id).is_none() {
            return Err(BrowserAgentRpcError::request(format!(
                "Unknown tab: {agent_tab_id}"
            )));
        }
        Ok(Value::Null)
    }

    fn agent_attach_target(&mut self, params: &Value) -> Result<Value, BrowserAgentRpcError> {
        let context_id = self.agent_context_id(params)?;
        let agent_tab_id = agent_tab_id(params)?;
        let target_id = checked_agent_target_id(params.get("targetId"))?;
        let index = self
            .agent_tab_index(&context_id, agent_tab_id)
            .ok_or_else(|| BrowserAgentRpcError::request(format!("Unknown tab: {agent_tab_id}")))?;
        if self.tabs[index].public.id == target_id {
            return Ok(Value::Null);
        }
        self.agent_child_sessions
            .values()
            .any(|child| child.agent_tab_id == agent_tab_id && child.target_id == target_id)
            .then_some(Value::Null)
            .ok_or_else(|| {
                BrowserAgentRpcError::request("Debugger target does not belong to this Browser tab")
            })
    }

    fn agent_detach_target(&mut self, params: &Value) -> Result<Value, BrowserAgentRpcError> {
        let context_id = self.agent_context_id(params)?;
        let agent_tab_id = agent_tab_id(params)?;
        let target_id = checked_agent_target_id(params.get("targetId"))?;
        let index = self
            .agent_tab_index(&context_id, agent_tab_id)
            .ok_or_else(|| BrowserAgentRpcError::request(format!("Unknown tab: {agent_tab_id}")))?;
        if self.tabs[index].public.id == target_id {
            return Ok(Value::Null);
        }
        let child_session = self
            .agent_child_sessions
            .iter()
            .find_map(|(session_id, child)| {
                (child.agent_tab_id == agent_tab_id && child.target_id == target_id)
                    .then(|| session_id.clone())
            })
            .ok_or_else(|| BrowserAgentRpcError::request("Debugger target is not attached"))?;
        let parent_session = self
            .agent_child_sessions
            .get(&child_session)
            .map(|child| child.parent_session_id.clone())
            .ok_or_else(|| BrowserAgentRpcError::request("Debugger target is not attached"))?;
        self.cdp
            .request(
                "Target.detachFromTarget",
                json!({"sessionId": child_session}),
                Some(&parent_session),
            )
            .map_err(BrowserAgentRpcError::request)?;
        self.remove_agent_child_session(&parent_session, &child_session);
        Ok(Value::Null)
    }

    fn agent_execute_cdp(&mut self, params: &Value) -> Result<Value, BrowserAgentRpcError> {
        let context_id = self.agent_context_id(params)?;
        let target = params
            .get("target")
            .and_then(Value::as_object)
            .ok_or_else(|| BrowserAgentRpcError::request("executeCdp requires a target"))?;
        let agent_tab_id = target
            .get("tabId")
            .and_then(Value::as_u64)
            .filter(|tab_id| *tab_id > 0)
            .ok_or_else(|| BrowserAgentRpcError::request("executeCdp requires a numeric tabId"))?;
        let index = self
            .agent_tab_index(&context_id, agent_tab_id)
            .ok_or_else(|| BrowserAgentRpcError::request(format!("Unknown tab: {agent_tab_id}")))?;
        let method = params
            .get("method")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|method| {
                !method.is_empty() && method.len() <= 128 && !method.chars().any(char::is_control)
            })
            .ok_or_else(|| BrowserAgentRpcError::request("executeCdp requires a method"))?;
        if !allowed_agent_cdp_method(method) {
            return Err(BrowserAgentRpcError::request(format!(
                "CDP method is not available to Browser agents: {method}"
            )));
        }

        let mut command_params = params
            .get("commandParams")
            .cloned()
            .unwrap_or_else(|| json!({}));
        if command_params.is_null() {
            command_params = json!({});
        }
        if !command_params.is_object() {
            return Err(BrowserAgentRpcError::request(
                "executeCdp commandParams must be an object",
            ));
        }
        if method == "Page.navigate" {
            let url = command_params
                .get("url")
                .and_then(Value::as_str)
                .ok_or_else(|| BrowserAgentRpcError::request("Page.navigate requires a URL"))
                .and_then(|url| {
                    checked_navigation_url(url)
                        .map_err(|error| BrowserAgentRpcError::request(error.to_string()))
                })?;
            if let Some(command_params) = command_params.as_object_mut() {
                command_params.insert("url".to_owned(), Value::String(url));
            }
        }
        let permission_url = if method == "Page.navigate" {
            command_params
                .get("url")
                .and_then(Value::as_str)
                .unwrap_or_default()
        } else {
            self.tabs[index].public.url.as_str()
        };
        self.ensure_agent_site_permission(
            permission_url,
            BrowserPermissionResource::Browse,
            "browsing",
        )?;
        if method == "DOM.setFileInputFiles" {
            self.ensure_agent_site_permission(
                permission_url,
                BrowserPermissionResource::Upload,
                "uploads",
            )?;
        }

        let cdp_session = self.agent_cdp_session(index, target)?;
        let root_command = matches!(
            method,
            "Browser.getVersion" | "Browser.getBrowserCommandLine"
        );
        self.cdp
            .request(
                method,
                command_params,
                (!root_command).then_some(cdp_session.as_str()),
            )
            .map_err(BrowserAgentRpcError::request)
    }

    fn agent_execute_cdp_with_cached_expression(
        &mut self,
        params: &Value,
    ) -> Result<Value, BrowserAgentRpcError> {
        let context_id = self.agent_context_id(params)?;
        let agent_tab_id = params
            .get("target")
            .and_then(Value::as_object)
            .and_then(|target| target.get("tabId"))
            .and_then(Value::as_u64)
            .filter(|tab_id| *tab_id > 0)
            .ok_or_else(|| BrowserAgentRpcError::request("executeCdp requires a numeric tabId"))?;
        if self.agent_tab_index(&context_id, agent_tab_id).is_none() {
            return Err(BrowserAgentRpcError::request(format!(
                "Unknown tab: {agent_tab_id}"
            )));
        }
        let cache_key = params
            .get("expressionCacheKey")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|key| {
                !key.is_empty()
                    && key.len() <= MAX_AGENT_CACHE_KEY_BYTES
                    && !key.chars().any(char::is_control)
            })
            .ok_or_else(|| {
                BrowserAgentRpcError::request(
                    "executeCdpWithCachedExpression requires an expressionCacheKey",
                )
            })?
            .to_owned();
        let cache_key = cached_agent_expression_key(context_id, agent_tab_id, cache_key);
        let supplied_expression = params
            .get("commandParams")
            .and_then(Value::as_object)
            .and_then(|command_params| command_params.get("expression"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        if supplied_expression
            .as_ref()
            .is_some_and(|expression| expression.len() > MAX_CACHED_AGENT_EXPRESSION_BYTES)
        {
            return Err(BrowserAgentRpcError::request(
                "Cached CDP expression exceeded its size limit",
            ));
        }
        if let Some(expression) = supplied_expression {
            self.store_agent_expression(cache_key.clone(), expression);
        }
        let Some(expression) = self.cached_agent_expressions.get(&cache_key).cloned() else {
            return Ok(json!({"kind": "cache-miss"}));
        };
        let mut request = params.clone();
        let request_object = request
            .as_object_mut()
            .ok_or_else(|| BrowserAgentRpcError::request("Cached CDP request was invalid"))?;
        let command_params = request_object
            .entry("commandParams")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or_else(|| {
                BrowserAgentRpcError::request("executeCdp commandParams must be an object")
            })?;
        command_params.insert("expression".to_owned(), Value::String(expression));
        let result = self.agent_execute_cdp(&request)?;
        Ok(json!({"kind": "executed", "result": result}))
    }

    fn agent_move_mouse(&mut self, params: &Value) -> Result<Value, BrowserAgentRpcError> {
        let context_id = self.agent_context_id(params)?;
        let agent_tab_id = agent_tab_id(params)?;
        let index = self
            .agent_tab_index(&context_id, agent_tab_id)
            .ok_or_else(|| BrowserAgentRpcError::request(format!("Unknown tab: {agent_tab_id}")))?;
        let x = finite_coordinate(params.get("x"), self.viewport_width)?;
        let y = finite_coordinate(params.get("y"), self.viewport_height)?;
        let session_id = self.tabs[index].session_id.clone();
        self.cdp
            .request(
                "Input.dispatchMouseEvent",
                json!({"type": "mouseMoved", "x": x, "y": y}),
                Some(&session_id),
            )
            .map_err(BrowserAgentRpcError::request)?;
        Ok(Value::Null)
    }

    fn agent_allow_download(&mut self, params: &Value) -> Result<Value, BrowserAgentRpcError> {
        let context_id = self.agent_context_id(params)?;
        let agent_tab_id = agent_tab_id(params)?;
        let index = self
            .agent_tab_index(&context_id, agent_tab_id)
            .ok_or_else(|| BrowserAgentRpcError::request(format!("Unknown tab: {agent_tab_id}")))?;
        let url = checked_download_url(params.get("url").and_then(Value::as_str))?;
        self.ensure_agent_site_permission(
            &self.tabs[index].public.url,
            BrowserPermissionResource::Download,
            "downloads",
        )?;
        self.ensure_agent_site_permission(&url, BrowserPermissionResource::Download, "downloads")?;
        self.expire_download_grant()
            .map_err(BrowserAgentRpcError::request)?;
        if let Some(grant) = self.pending_download_grant.as_ref() {
            if grant.context_id == context_id
                && matches!(
                    &grant.source,
                    BrowserDownloadGrantSource::Agent {
                        agent_tab_id: pending_tab_id,
                        expected_url,
                    } if *pending_tab_id == agent_tab_id && expected_url == &url
                )
            {
                return Ok(Value::Null);
            }
            return Err(BrowserAgentRpcError::request(
                "Another Browser download approval is still pending",
            ));
        }
        let session_id = self.tabs[index].session_id.clone();
        let frame_tree = self
            .cdp
            .request("Page.getFrameTree", json!({}), Some(&session_id))
            .map_err(BrowserAgentRpcError::request)?;
        let frame_ids = download_frame_ids(&frame_tree);
        if frame_ids.is_empty() {
            return Err(BrowserAgentRpcError::request(
                "Browser tab did not expose a download frame",
            ));
        }

        let directory = self
            .create_download_directory()
            .map_err(BrowserAgentRpcError::request)?;
        self.set_download_behavior(true, Some(&directory))
            .map_err(BrowserAgentRpcError::request)?;
        self.pending_download_grant = Some(BrowserDownloadGrant {
            context_id,
            directory,
            expires_at: Instant::now() + BROWSER_DOWNLOAD_GRANT_TIMEOUT,
            frame_ids,
            source: BrowserDownloadGrantSource::Agent {
                agent_tab_id,
                expected_url: url,
            },
        });
        Ok(Value::Null)
    }

    fn ensure_agent_site_permission(
        &self,
        url: &str,
        resource: BrowserPermissionResource,
        action: &str,
    ) -> Result<(), BrowserAgentRpcError> {
        if browser_permission_for_url(&self.permissions, url, BrowserPermissionResource::Browse)
            == BrowserPermissionValue::Block
            || browser_permission_for_url(&self.permissions, url, resource)
                == BrowserPermissionValue::Block
        {
            let origin = normalize_browser_origin(url).unwrap_or_else(|| "this site".to_owned());
            return Err(BrowserAgentRpcError::request(format!(
                "Browser site permissions block {action} on {origin}"
            )));
        }
        Ok(())
    }

    fn agent_finalize_tabs(&mut self, params: &Value) -> Result<Value, BrowserAgentRpcError> {
        let context_id = self.agent_context_id(params)?;
        let keep = params
            .get("keep")
            .and_then(Value::as_array)
            .ok_or_else(|| BrowserAgentRpcError::request("finalizeTabs requires a keep array"))?;
        if keep.len() > MAX_BROWSER_TABS {
            return Err(BrowserAgentRpcError::request(
                "finalizeTabs keep array exceeded its limit",
            ));
        }
        let keep_ids: HashSet<u64> = keep
            .iter()
            .filter_map(|entry| entry.get("tabId").and_then(Value::as_u64))
            .collect();
        let close_ids: Vec<String> = self
            .tabs
            .iter()
            .filter(|tab| {
                tab.context_id == context_id
                    && tab.origin == BrowserTabOrigin::Agent
                    && !keep_ids.contains(&tab.agent_id)
            })
            .map(|tab| tab.public.id.clone())
            .collect();
        for tab_id in close_ids {
            self.close_tab(&context_id, &tab_id)
                .map_err(BrowserAgentRpcError::request)?;
        }
        Ok(Value::Null)
    }

    fn agent_mark_tab(&mut self, params: &Value) -> Result<Value, BrowserAgentRpcError> {
        let context_id = self.agent_context_id(params)?;
        let agent_tab_id = agent_tab_id(params)?;
        let status = params
            .get("status")
            .and_then(Value::as_str)
            .filter(|status| matches!(*status, "handoff" | "deliverable"))
            .ok_or_else(|| BrowserAgentRpcError::request("markTab received an invalid status"))?
            .to_owned();
        let turn_id = params
            .get("turn_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|turn_id| {
                !turn_id.is_empty()
                    && turn_id.len() <= MAX_BROWSER_CONTEXT_ID_BYTES
                    && !turn_id.chars().any(char::is_control)
            })
            .ok_or_else(|| BrowserAgentRpcError::request("markTab requires a turn_id"))?
            .to_owned();
        let index = self
            .agent_tab_index(&context_id, agent_tab_id)
            .ok_or_else(|| BrowserAgentRpcError::request(format!("Unknown tab: {agent_tab_id}")))?;
        self.tabs[index].mark = Some(BrowserTabMark { status, turn_id });
        Ok(Value::Null)
    }

    fn agent_turn_ended(&mut self, params: &Value) -> Result<Value, BrowserAgentRpcError> {
        let context_id = self.agent_context_id(params)?;
        let turn_id = params
            .get("turn_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|turn_id| !turn_id.is_empty())
            .ok_or_else(|| BrowserAgentRpcError::request("turnEnded requires a turn_id"))?;
        let close_ids: Vec<String> = self
            .tabs
            .iter()
            .filter(|tab| {
                tab.context_id == context_id
                    && tab.origin == BrowserTabOrigin::Agent
                    && !tab.mark.as_ref().is_some_and(|mark| {
                        mark.turn_id == turn_id
                            && matches!(mark.status.as_str(), "handoff" | "deliverable")
                    })
            })
            .map(|tab| tab.public.id.clone())
            .collect();
        for tab_id in close_ids {
            self.close_tab(&context_id, &tab_id)
                .map_err(BrowserAgentRpcError::request)?;
        }
        for tab in self
            .tabs
            .iter_mut()
            .filter(|tab| tab.context_id == context_id)
        {
            if tab
                .mark
                .as_ref()
                .is_some_and(|mark| mark.turn_id == turn_id)
            {
                tab.mark = None;
            }
        }
        Ok(Value::Null)
    }

    fn agent_context_id(&self, params: &Value) -> Result<String, BrowserAgentRpcError> {
        let context_id = params
            .get("session_id")
            .and_then(Value::as_str)
            .ok_or_else(|| BrowserAgentRpcError::request("Missing required browser session_id"))
            .and_then(|context_id| {
                checked_context_id(context_id)
                    .map_err(|error| BrowserAgentRpcError::request(error.to_string()))
            })?;
        if !self.tabs.iter().any(|tab| tab.context_id == context_id) {
            return Err(BrowserAgentRpcError::request(format!(
                "Browser session is unavailable: {context_id}"
            )));
        }
        Ok(context_id)
    }

    fn agent_tab_index(&self, context_id: &str, agent_tab_id: u64) -> Option<usize> {
        self.tabs
            .iter()
            .position(|tab| browser_agent_tab_matches(tab, context_id, agent_tab_id))
    }

    fn serialize_agent_tab(&self, tab: &BrowserTabRuntime) -> Value {
        json!({
            "id": tab.agent_id,
            "title": tab.public.title,
            "active": self.active_tab_ids.get(&tab.context_id) == Some(&tab.public.id),
            "url": tab.public.url,
        })
    }

    fn agent_cdp_session(
        &self,
        tab_index: usize,
        target: &serde_json::Map<String, Value>,
    ) -> Result<String, BrowserAgentRpcError> {
        let tab = self
            .tabs
            .get(tab_index)
            .ok_or_else(|| BrowserAgentRpcError::request("Browser tab is no longer available"))?;
        if let Some(session_id) = target.get("sessionId").and_then(Value::as_str) {
            if session_id == tab.session_id
                || self
                    .agent_child_sessions
                    .get(session_id)
                    .is_some_and(|child| child.agent_tab_id == tab.agent_id)
            {
                return Ok(session_id.to_owned());
            }
            return Err(BrowserAgentRpcError::request(
                "Debugger session does not belong to this Browser tab",
            ));
        }
        if let Some(target_id) = target.get("targetId").and_then(Value::as_str) {
            if target_id == tab.public.id {
                return Ok(tab.session_id.clone());
            }
            return self
                .agent_child_sessions
                .iter()
                .find_map(|(session_id, child)| {
                    (child.agent_tab_id == tab.agent_id && child.target_id == target_id)
                        .then(|| session_id.clone())
                })
                .ok_or_else(|| {
                    BrowserAgentRpcError::request(
                        "Debugger target does not belong to this Browser tab",
                    )
                });
        }
        Ok(tab.session_id.clone())
    }

    fn store_agent_expression(&mut self, key: (String, u64, String), expression: String) {
        if self.cached_agent_expressions.contains_key(&key) {
            self.cached_agent_expression_order
                .retain(|cached| cached != &key);
        } else if self.cached_agent_expressions.len() >= MAX_CACHED_AGENT_EXPRESSIONS
            && let Some(oldest) = self.cached_agent_expression_order.pop_front()
        {
            self.cached_agent_expressions.remove(&oldest);
        }
        self.cached_agent_expression_order.push_back(key.clone());
        self.cached_agent_expressions.insert(key, expression);
    }

    fn record_agent_target_event(&mut self, event: &Value) {
        let method = event.get("method").and_then(Value::as_str);
        let parent_session = event.get("sessionId").and_then(Value::as_str);
        if method == Some("Target.detachedFromTarget") {
            if let (Some(parent_session), Some(child_session)) = (
                parent_session,
                event
                    .get("params")
                    .and_then(|params| params.get("sessionId"))
                    .and_then(Value::as_str),
            ) {
                let root_sessions = self
                    .tabs
                    .iter()
                    .map(|tab| (tab.session_id.as_str(), tab.agent_id));
                remove_agent_child_session(
                    root_sessions,
                    &mut self.agent_child_sessions,
                    parent_session,
                    child_session,
                );
            }
            return;
        }
        if method != Some("Target.attachedToTarget") {
            return;
        }
        let Some(parent_session) = parent_session else {
            return;
        };
        let params = event.get("params").unwrap_or(&Value::Null);
        let Some(child_session) = params.get("sessionId").and_then(Value::as_str) else {
            return;
        };
        let Some(target_id) = params
            .get("targetInfo")
            .and_then(|target| target.get("targetId"))
            .and_then(Value::as_str)
        else {
            return;
        };
        let root_sessions = self
            .tabs
            .iter()
            .map(|tab| (tab.session_id.as_str(), tab.agent_id));
        record_agent_child_session(
            root_sessions,
            &mut self.agent_child_sessions,
            parent_session,
            child_session,
            target_id,
        );
    }

    fn remove_agent_child_session(&mut self, parent_session_id: &str, child_session_id: &str) {
        let root_sessions = self
            .tabs
            .iter()
            .map(|tab| (tab.session_id.as_str(), tab.agent_id));
        remove_agent_child_session(
            root_sessions,
            &mut self.agent_child_sessions,
            parent_session_id,
            child_session_id,
        );
    }

    fn forward_agent_cdp_event(&self, event: &Value) {
        let Some(method) = event.get("method").and_then(Value::as_str) else {
            return;
        };
        if method == "Page.screencastFrame" {
            return;
        }
        let session_id = event.get("sessionId").and_then(Value::as_str);
        let route = session_id
            .and_then(|session_id| {
                self.tab_index_for_session(Some(session_id))
                    .map(|index| {
                        (
                            self.tabs[index].context_id.clone(),
                            json!({"tabId": self.tabs[index].agent_id}),
                        )
                    })
                    .or_else(|| {
                        self.agent_child_sessions.get(session_id).and_then(|child| {
                            self.tabs
                                .iter()
                                .find(|tab| tab.agent_id == child.agent_tab_id)
                                .map(|tab| {
                                    (
                                        tab.context_id.clone(),
                                        json!({
                                            "tabId": tab.agent_id,
                                            "sessionId": session_id,
                                            "targetId": child.target_id,
                                        }),
                                    )
                                })
                        })
                    })
            })
            .or_else(|| {
                event
                    .pointer("/params/targetInfo/targetId")
                    .and_then(Value::as_str)
                    .and_then(|target_id| self.tab_index(target_id))
                    .map(|index| {
                        (
                            self.tabs[index].context_id.clone(),
                            json!({"tabId": self.tabs[index].agent_id}),
                        )
                    })
            });
        let Some((context_id, source)) = route else {
            return;
        };
        self.emit_agent_notification(
            &context_id,
            "onCDPEvent",
            json!({
                "source": source,
                "method": method,
                "params": event.get("params").cloned().unwrap_or_else(|| json!({})),
            }),
        );
    }

    fn emit_agent_notification(&self, context_id: &str, method: &str, params: Value) {
        let _ = self.agent_events.try_send(BrowserAgentNotification {
            context_id: context_id.to_owned(),
            method: method.to_owned(),
            params,
        });
    }

    fn emit_agent_download_notification(
        &self,
        context_id: &str,
        id: &str,
        status: &str,
        path: &Path,
        url: &str,
    ) {
        self.emit_agent_notification(
            context_id,
            "onDownloadChange",
            json!({
                "filename": bounded_text(
                    path.to_string_lossy().as_ref(),
                    MAX_BROWSER_DOWNLOAD_PATH_BYTES
                ),
                "id": id,
                "session_id": context_id,
                "status": status,
                "url": url,
            }),
        );
    }

    fn record_download(&mut self, download: BrowserDownload) {
        if let Some(index) = self
            .download_history
            .iter()
            .position(|entry| entry.id == download.id)
        {
            self.download_history.remove(index);
        }
        self.download_history.push_front(download);
        self.download_history.truncate(MAX_BROWSER_DOWNLOAD_HISTORY);
    }

    fn reserve_download_path(&mut self, suggested_filename: &str) -> Result<PathBuf, String> {
        let filename = safe_download_filename(suggested_filename);
        let path = Path::new(&filename);
        let stem = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .filter(|stem| !stem.is_empty())
            .unwrap_or_else(|| "download".to_owned());
        let extension = path
            .extension()
            .map(|extension| extension.to_string_lossy().into_owned());
        for sequence in 0..MAX_BROWSER_DOWNLOAD_FILENAME_ATTEMPTS {
            let filename = if sequence == 0 {
                filename.clone()
            } else if let Some(extension) = extension.as_deref() {
                format!("{stem} ({sequence}).{extension}")
            } else {
                format!("{stem} ({sequence})")
            };
            let candidate = self.download_dir.join(filename);
            if !candidate.exists() && !self.reserved_download_paths.contains(&candidate) {
                self.reserved_download_paths.insert(candidate.clone());
                return Ok(candidate);
            }
        }
        Err("The Browser could not reserve a unique download filename.".to_owned())
    }

    fn create_download_directory(&mut self) -> Result<PathBuf, String> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        for _ in 0..MAX_BROWSER_DOWNLOAD_DIRECTORY_ENTRIES {
            let sequence = self.next_download_directory_id;
            self.next_download_directory_id = self.next_download_directory_id.saturating_add(1);
            let directory = self
                .download_staging_root
                .join(format!("{timestamp:x}-{sequence:x}"));
            match fs::create_dir(&directory) {
                Ok(()) => return Ok(directory),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => {
                    return Err(format!(
                        "could not prepare the Browser download directory: {error}"
                    ));
                }
            }
        }
        Err("could not allocate a unique Browser download directory".to_owned())
    }

    fn set_download_directory(&mut self, path: PathBuf) -> Result<(), String> {
        checked_download_path(&path).map_err(|error| error.to_string())?;
        fs::create_dir_all(&path)
            .map_err(|error| format!("Unable to change the downloads folder: {error}"))?;
        if !path.is_dir() {
            return Err("Unable to change the downloads folder.".to_owned());
        }
        self.download_dir = path;
        Ok(())
    }

    fn set_download_destination(&mut self, id: &str, path: Option<PathBuf>) -> Result<(), String> {
        if let Some(path) = path.as_ref() {
            checked_download_path(path).map_err(|error| error.to_string())?;
            if path.file_name().is_none()
                || path.is_dir()
                || path.parent().is_none_or(|parent| !parent.is_dir())
            {
                return Err("The selected Browser download path is unavailable.".to_owned());
            }
        }

        let Some(mut download) = self.downloads.remove(id) else {
            return Err("The Browser download is no longer waiting for a location.".to_owned());
        };
        if !download.user_initiated || !download.save_prompt_pending {
            self.downloads.insert(id.to_owned(), download);
            return Err("The Browser download is not waiting for a location.".to_owned());
        }

        let Some(path) = path else {
            download.save_prompt_pending = false;
            if download.completed_source.take().is_some() {
                let reserved_path = browser_download_path(&download).to_path_buf();
                self.reserved_download_paths.remove(&reserved_path);
                self.emit_download_change(id, BrowserDownloadStatus::Canceled, &download);
                let _ = fs::remove_dir_all(&download.staging_directory);
                if self.pending_download_grant.is_none() && self.downloads.is_empty() {
                    self.set_download_behavior(false, None)?;
                }
                return Ok(());
            }
            let result = self
                .cdp
                .request("Browser.cancelDownload", json!({"guid": id}), None)
                .map(|_| ());
            self.downloads.insert(id.to_owned(), download);
            return result;
        };

        if path != download.default_destination && self.reserved_download_paths.contains(&path) {
            self.downloads.insert(id.to_owned(), download);
            return Err("Another Browser download already reserved that path.".to_owned());
        }
        self.reserved_download_paths
            .remove(&download.default_destination);
        self.reserved_download_paths.insert(path.clone());
        download.destination = Some(path);
        download.save_prompt_pending = false;
        download.overwrite_destination = true;
        if let Some(source) = download.completed_source.take() {
            self.finish_completed_download(id, download, &source)?;
        } else {
            let status = if download.paused {
                BrowserDownloadStatus::Paused
            } else if download.received_bytes == 0 {
                BrowserDownloadStatus::Started
            } else {
                BrowserDownloadStatus::InProgress
            };
            self.emit_download_change(id, status, &download);
            self.downloads.insert(id.to_owned(), download);
        }
        Ok(())
    }

    fn finish_completed_download(
        &mut self,
        id: &str,
        download: BrowserDownloadRuntime,
        source: &Path,
    ) -> Result<(), String> {
        let destination = download
            .destination
            .as_deref()
            .ok_or_else(|| "The Browser download location was not selected.".to_owned())?;
        let status =
            match move_download_to_destination(source, destination, download.overwrite_destination)
            {
                Ok(()) => BrowserDownloadStatus::Complete,
                Err(_) => BrowserDownloadStatus::Failed,
            };
        self.reserved_download_paths.remove(destination);
        self.emit_download_change(id, status, &download);
        let _ = fs::remove_dir_all(&download.staging_directory);
        if self.pending_download_grant.is_none() && self.downloads.is_empty() {
            self.set_download_behavior(false, None)?;
        }
        Ok(())
    }

    fn cancel_download(&mut self, id: &str) -> Result<(), String> {
        if self.downloads.get(id).is_some_and(|download| {
            download.user_initiated
                && download.save_prompt_pending
                && download.destination.is_none()
        }) {
            return self.set_download_destination(id, None);
        }
        if !self.downloads.contains_key(id) {
            return Err("The Browser download is no longer active.".to_owned());
        }
        self.cdp
            .request("Browser.cancelDownload", json!({"guid": id}), None)?;
        Ok(())
    }

    fn pause_download(&mut self, id: &str) -> Result<(), String> {
        let Some(mut download) = self.downloads.remove(id) else {
            return Err("The Browser download is no longer active.".to_owned());
        };
        if download.paused || download.completed_source.is_some() {
            self.downloads.insert(id.to_owned(), download);
            return Err("download-not-pausable".to_owned());
        }
        match control_download_transfer(
            &mut self.cdp,
            self.browser_family,
            &download,
            BrowserDownloadControl::Pause,
        ) {
            Ok(can_resume) => {
                download.paused = true;
                download.can_resume = can_resume;
                self.emit_download_change(id, BrowserDownloadStatus::Paused, &download);
                self.downloads.insert(id.to_owned(), download);
                Ok(())
            }
            Err(error) => {
                self.downloads.insert(id.to_owned(), download);
                Err(error)
            }
        }
    }

    fn resume_download(&mut self, id: &str) -> Result<(), String> {
        let Some(mut download) = self.downloads.remove(id) else {
            return Err("The Browser download is no longer active.".to_owned());
        };
        if !download.paused || !download.can_resume {
            self.downloads.insert(id.to_owned(), download);
            return Err("download-not-resumable".to_owned());
        }
        match control_download_transfer(
            &mut self.cdp,
            self.browser_family,
            &download,
            BrowserDownloadControl::Resume,
        ) {
            Ok(_) => {
                download.paused = false;
                download.can_resume = false;
                self.emit_download_change(id, BrowserDownloadStatus::InProgress, &download);
                self.downloads.insert(id.to_owned(), download);
                Ok(())
            }
            Err(error) => {
                self.downloads.insert(id.to_owned(), download);
                Err(error)
            }
        }
    }

    fn open_download(&self, id: &str) -> Result<(), String> {
        let download = self
            .download_history
            .iter()
            .find(|download| download.id == id)
            .ok_or_else(|| "The Browser download could not be found.".to_owned())?;
        if download.status != BrowserDownloadStatus::Complete || !download.path.is_file() {
            return Err("The downloaded file is no longer available.".to_owned());
        }
        open_platform_path(&download.path, false)
    }

    fn show_download_in_folder(&self, id: &str) -> Result<(), String> {
        let download = self
            .download_history
            .iter()
            .find(|download| download.id == id)
            .ok_or_else(|| "The Browser download could not be found.".to_owned())?;
        if download.status != BrowserDownloadStatus::Complete || !download.path.is_file() {
            return Err("The downloaded file is no longer available.".to_owned());
        }
        open_platform_path(&download.path, true)
    }

    fn remove_download(&mut self, id: &str) -> Result<(), String> {
        if self.downloads.contains_key(id) {
            return Err("Stop the Browser download before removing it.".to_owned());
        }
        let Some(index) = self
            .download_history
            .iter()
            .position(|download| download.id == id)
        else {
            return Err("The Browser download could not be found.".to_owned());
        };
        self.download_history.remove(index);
        self.emit(BrowserEvent::DownloadRemoved { id: id.to_owned() });
        Ok(())
    }

    fn set_download_behavior(
        &mut self,
        enabled: bool,
        directory: Option<&Path>,
    ) -> Result<(), String> {
        if !enabled && self.download_behavior_enabled == Some(false) {
            return Ok(());
        }
        let params = if enabled {
            let directory = directory
                .and_then(Path::to_str)
                .ok_or_else(|| "Browser download directory was not valid UTF-8".to_owned())?;
            if directory.len() > MAX_BROWSER_DOWNLOAD_PATH_BYTES || directory.contains('\0') {
                return Err("Browser download directory exceeded its limits".to_owned());
            }
            json!({
                "behavior": "allow",
                "downloadPath": directory,
                "eventsEnabled": true,
            })
        } else {
            json!({
                "behavior": "deny",
                "eventsEnabled": true,
            })
        };
        self.cdp
            .request("Browser.setDownloadBehavior", params, None)?;
        self.download_behavior_enabled = Some(enabled);
        Ok(())
    }

    fn expire_download_grant(&mut self) -> Result<(), String> {
        let expired = self
            .pending_download_grant
            .as_ref()
            .is_some_and(|grant| Instant::now() >= grant.expires_at);
        if expired {
            self.pending_download_grant = None;
            if self.downloads.is_empty() {
                self.set_download_behavior(false, None)?;
            }
        }
        Ok(())
    }

    fn revoke_download_grant_for_tab(&mut self, agent_tab_id: u64) -> Result<(), String> {
        if self.pending_download_grant.as_ref().is_some_and(|grant| {
            matches!(
                &grant.source,
                BrowserDownloadGrantSource::Agent {
                    agent_tab_id: pending_tab_id,
                    ..
                } if *pending_tab_id == agent_tab_id
            )
        }) {
            self.pending_download_grant = None;
            if self.downloads.is_empty() {
                self.set_download_behavior(false, None)?;
            }
        }
        Ok(())
    }

    fn prepare_user_download_grant(&mut self, context_id: &str) -> Result<(), String> {
        self.expire_download_grant()?;
        if let Some(grant) = self.pending_download_grant.as_mut() {
            if grant.context_id == context_id
                && matches!(&grant.source, BrowserDownloadGrantSource::User)
            {
                grant.expires_at = Instant::now() + BROWSER_DOWNLOAD_GRANT_TIMEOUT;
            }
            return Ok(());
        }

        let session_id = self.active_tab()?.session_id.clone();
        let frame_tree = self
            .cdp
            .request("Page.getFrameTree", json!({}), Some(&session_id))?;
        let frame_ids = download_frame_ids(&frame_tree);
        if frame_ids.is_empty() {
            return Ok(());
        }
        let directory = self.download_staging_root.join("user");
        fs::create_dir_all(&directory)
            .map_err(|error| format!("could not prepare user Browser downloads: {error}"))?;
        self.set_download_behavior(true, Some(&directory))?;
        self.pending_download_grant = Some(BrowserDownloadGrant {
            context_id: context_id.to_owned(),
            directory,
            expires_at: Instant::now() + BROWSER_DOWNLOAD_GRANT_TIMEOUT,
            frame_ids,
            source: BrowserDownloadGrantSource::User,
        });
        Ok(())
    }

    fn handle_download_will_begin(&mut self, params: &Value) -> Result<(), String> {
        let guid = checked_download_event_text(
            params.get("guid").and_then(Value::as_str),
            MAX_BROWSER_DOWNLOAD_ID_BYTES,
            "Browser download did not include a valid identifier",
        )?;
        let url = checked_download_event_text(
            params.get("url").and_then(Value::as_str),
            MAX_BROWSER_URL_BYTES,
            "Browser download did not include a valid URL",
        )?;
        let frame_id = checked_download_event_text(
            params.get("frameId").and_then(Value::as_str),
            MAX_BROWSER_DOWNLOAD_ID_BYTES,
            "Browser download did not include a valid frame identifier",
        )?;
        let matches_grant = self.pending_download_grant.as_ref().is_some_and(|grant| {
            grant.frame_ids.contains(&frame_id)
                && match &grant.source {
                    BrowserDownloadGrantSource::Agent { expected_url, .. } => expected_url == &url,
                    BrowserDownloadGrantSource::User => true,
                }
        });
        if !matches_grant {
            self.cdp
                .request("Browser.cancelDownload", json!({"guid": guid}), None)?;
            return Ok(());
        }

        let grant = self
            .pending_download_grant
            .take()
            .ok_or_else(|| "Browser download approval disappeared".to_owned())?;
        let agent_session = matches!(&grant.source, BrowserDownloadGrantSource::Agent { .. });
        let user_initiated = matches!(&grant.source, BrowserDownloadGrantSource::User);
        if self.downloads.len() >= MAX_BROWSER_TABS {
            self.cdp
                .request("Browser.cancelDownload", json!({"guid": guid}), None)?;
            if agent_session {
                self.emit_agent_download_notification(
                    &grant.context_id,
                    &guid,
                    "failed",
                    &self.download_dir,
                    &url,
                );
            }
            self.emit(BrowserEvent::OperationFailed(
                "The Browser download limit was reached.".to_owned(),
            ));
            if self.downloads.is_empty() {
                self.set_download_behavior(false, None)?;
            }
            return Ok(());
        }

        let suggested_filename = params
            .get("suggestedFilename")
            .and_then(Value::as_str)
            .map(safe_download_filename)
            .unwrap_or_else(|| "download".to_owned());
        let default_destination = self.reserve_download_path(&suggested_filename)?;
        let save_prompt_pending = user_initiated && self.prompt_for_user_downloads;
        let destination = (!save_prompt_pending).then(|| default_destination.clone());
        let staging_path = grant.directory.join(&suggested_filename);
        let started_at_ms = unix_time_ms();
        let download = BrowserDownloadRuntime {
            agent_session,
            can_resume: false,
            completed_source: None,
            context_id: grant.context_id,
            default_destination,
            destination,
            overwrite_destination: false,
            paused: false,
            received_bytes: 0,
            save_prompt_pending,
            staging_directory: grant.directory,
            staging_path,
            started_at_ms,
            total_bytes: 0,
            url,
            user_initiated,
        };
        self.emit_download_change(&guid, BrowserDownloadStatus::Started, &download);
        self.downloads.insert(guid.clone(), download);
        if save_prompt_pending {
            self.emit(BrowserEvent::DownloadSaveRequested {
                directory: self.download_dir.clone(),
                filename: suggested_filename,
                id: guid,
            });
        }
        Ok(())
    }

    fn handle_download_progress(&mut self, params: &Value) -> Result<(), String> {
        let guid = checked_download_event_text(
            params.get("guid").and_then(Value::as_str),
            MAX_BROWSER_DOWNLOAD_ID_BYTES,
            "Browser download progress did not include a valid identifier",
        )?;
        let Some(state) = params.get("state").and_then(Value::as_str) else {
            return Err("Browser download progress did not include a state".to_owned());
        };
        if !self.downloads.contains_key(&guid) {
            if state == "inProgress" {
                self.cdp
                    .request("Browser.cancelDownload", json!({"guid": guid}), None)?;
            }
            return Ok(());
        }

        match state {
            "inProgress" => {
                let Some(mut download) = self.downloads.remove(&guid) else {
                    return Ok(());
                };
                download.received_bytes = download_event_bytes(params, "receivedBytes");
                download.total_bytes = download_event_bytes(params, "totalBytes");
                let status = if download.paused {
                    BrowserDownloadStatus::Paused
                } else {
                    BrowserDownloadStatus::InProgress
                };
                self.emit_download_change(&guid, status, &download);
                self.downloads.insert(guid, download);
            }
            "completed" | "canceled" => {
                let Some(mut download) = self.downloads.remove(&guid) else {
                    return Ok(());
                };
                download.received_bytes = download_event_bytes(params, "receivedBytes");
                download.total_bytes = download_event_bytes(params, "totalBytes");
                if state == "completed" {
                    let source = resolve_completed_download_path(
                        &download,
                        params.get("filePath").and_then(Value::as_str),
                    );
                    if download.save_prompt_pending && download.destination.is_none() {
                        download.paused = false;
                        download.can_resume = false;
                        download.completed_source = Some(source);
                        self.emit_download_change(
                            &guid,
                            BrowserDownloadStatus::InProgress,
                            &download,
                        );
                        self.downloads.insert(guid, download);
                    } else if download.destination.is_some() {
                        self.finish_completed_download(&guid, download, &source)?;
                    } else {
                        let reserved_path = browser_download_path(&download).to_path_buf();
                        self.reserved_download_paths.remove(&reserved_path);
                        self.emit_download_change(
                            &guid,
                            BrowserDownloadStatus::Canceled,
                            &download,
                        );
                        let _ = fs::remove_dir_all(&download.staging_directory);
                    }
                } else {
                    let reserved_path = browser_download_path(&download).to_path_buf();
                    self.reserved_download_paths.remove(&reserved_path);
                    self.emit_download_change(&guid, BrowserDownloadStatus::Canceled, &download);
                    let _ = fs::remove_dir_all(&download.staging_directory);
                }
                if self.pending_download_grant.is_none() && self.downloads.is_empty() {
                    self.set_download_behavior(false, None)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn emit_download_change(
        &mut self,
        guid: &str,
        status: BrowserDownloadStatus,
        download: &BrowserDownloadRuntime,
    ) {
        let updated_at_ms = unix_time_ms();
        let destination = browser_download_path(download);
        let active = matches!(
            status,
            BrowserDownloadStatus::Started
                | BrowserDownloadStatus::InProgress
                | BrowserDownloadStatus::Paused
        );
        let item = BrowserDownload {
            can_cancel: active,
            can_pause: active
                && status != BrowserDownloadStatus::Paused
                && download.completed_source.is_none(),
            can_resume: status == BrowserDownloadStatus::Paused && download.can_resume,
            context_id: download.context_id.clone(),
            file_exists: status == BrowserDownloadStatus::Complete && destination.is_file(),
            filename: destination
                .file_name()
                .map(|filename| filename.to_string_lossy().into_owned())
                .unwrap_or_else(|| "download".to_owned()),
            id: guid.to_owned(),
            path: destination.to_path_buf(),
            received_bytes: download.received_bytes,
            started_at_ms: download.started_at_ms,
            status,
            total_bytes: download.total_bytes,
            updated_at_ms,
            url: download.url.clone(),
            user_initiated: download.user_initiated,
        };
        self.record_download(item.clone());
        if download.agent_session {
            self.emit_agent_download_notification(
                &download.context_id,
                guid,
                browser_agent_download_status(status),
                destination,
                &download.url,
            );
        }
        self.emit(BrowserEvent::DownloadChanged(item));
    }

    fn navigate(&mut self, context_id: &str, url: &str) -> Result<(), String> {
        self.activate_context(context_id)?;
        let url = checked_navigation_url(url).map_err(|error| error.to_string())?;
        let session_id = self.active_tab()?.session_id.clone();
        self.set_loading(true);
        self.cdp
            .request("Page.navigate", json!({"url": url}), Some(&session_id))?;
        Ok(())
    }

    fn navigate_history(&mut self, delta: isize) -> Result<(), String> {
        let session_id = self.active_tab()?.session_id.clone();
        let history =
            self.cdp
                .request("Page.getNavigationHistory", json!({}), Some(&session_id))?;
        let current = history
            .get("currentIndex")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let entries = history
            .get("entries")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let target_index = current.saturating_add(delta as i64);
        if target_index < 0 || target_index >= entries.len() as i64 {
            self.refresh_tab(self.active_index)?;
            let context_id = self.active_context_id.clone();
            self.emit_tabs(&context_id);
            return Ok(());
        }
        let entry_id = entries[target_index as usize]
            .get("id")
            .and_then(Value::as_i64)
            .ok_or_else(|| "Browser history entry did not include an id".to_owned())?;
        self.set_loading(true);
        self.cdp.request(
            "Page.navigateToHistoryEntry",
            json!({"entryId": entry_id}),
            Some(&session_id),
        )?;
        Ok(())
    }

    fn open_tab(
        &mut self,
        context_id: &str,
        url: Option<&str>,
        origin: BrowserTabOrigin,
    ) -> Result<(), String> {
        if self.tabs.len() >= MAX_BROWSER_TABS {
            return Err(format!(
                "Browser supports at most {MAX_BROWSER_TABS} open tabs"
            ));
        }
        if !self.tabs.is_empty() {
            self.stop_screencast(self.active_index)?;
        }
        let target_id = self.create_target("about:blank")?;
        self.attach_target(context_id, target_id, origin)?;
        self.active_index = self.tabs.len().saturating_sub(1);
        self.active_context_id = context_id.to_owned();
        self.active_tab_ids.insert(
            context_id.to_owned(),
            self.tabs[self.active_index].public.id.clone(),
        );
        self.start_screencast(self.active_index)?;
        if let Some(url) = url {
            self.navigate(context_id, url)?;
        } else {
            self.refresh_tab(self.active_index)?;
        }
        self.emit_tabs(context_id);
        Ok(())
    }

    fn select_tab(&mut self, context_id: &str, tab_id: &str) -> Result<(), String> {
        let Some(index) = self.tab_index_in_context(context_id, tab_id) else {
            return Err("Browser tab is no longer available".to_owned());
        };
        if index == self.active_index {
            return Ok(());
        }
        self.stop_screencast(self.active_index)?;
        self.active_index = index;
        self.active_context_id = context_id.to_owned();
        self.active_tab_ids
            .insert(context_id.to_owned(), tab_id.to_owned());
        self.cdp
            .request("Target.activateTarget", json!({"targetId": tab_id}), None)?;
        self.start_screencast(index)?;
        self.refresh_tab(index)?;
        self.emit_tabs(context_id);
        Ok(())
    }

    fn close_tab(&mut self, context_id: &str, tab_id: &str) -> Result<(), String> {
        let Some(index) = self.tab_index_in_context(context_id, tab_id) else {
            return Ok(());
        };
        let agent_tab_id = self.tabs[index].agent_id;
        let was_active = index == self.active_index;
        if was_active {
            self.stop_screencast(index)?;
        }
        self.revoke_download_grant_for_tab(agent_tab_id)?;
        self.emit_agent_notification(context_id, "onCDPDetach", json!({"tabId": agent_tab_id}));
        self.cdp
            .request("Target.closeTarget", json!({"targetId": tab_id}), None)?;
        self.tabs.remove(index);
        self.agent_child_sessions
            .retain(|_, child| child.agent_tab_id != agent_tab_id);
        if index < self.active_index {
            self.active_index = self.active_index.saturating_sub(1);
        }
        let replacement = self
            .tabs
            .iter()
            .position(|tab| tab.context_id == context_id);
        if replacement.is_none() {
            let target_id = self.create_target("about:blank")?;
            self.attach_target(context_id, target_id, BrowserTabOrigin::User)?;
        } else {
            self.active_tab_ids.remove(context_id);
        }
        if was_active {
            self.active_context_id.clear();
            self.activate_context(context_id)?;
        } else if let Some(index) = self
            .tabs
            .iter()
            .position(|tab| tab.context_id == context_id)
        {
            self.active_tab_ids
                .insert(context_id.to_owned(), self.tabs[index].public.id.clone());
        }
        self.emit_tabs(context_id);
        Ok(())
    }

    fn activate_context(&mut self, context_id: &str) -> Result<(), String> {
        if self.active_context_id == context_id
            && self
                .tabs
                .get(self.active_index)
                .is_some_and(|tab| tab.context_id == context_id)
        {
            return Ok(());
        }
        let requested_tab_id = self.active_tab_ids.get(context_id).cloned();
        let index = requested_tab_id
            .as_deref()
            .and_then(|tab_id| self.tab_index_in_context(context_id, tab_id))
            .or_else(|| {
                self.tabs
                    .iter()
                    .position(|tab| tab.context_id == context_id)
            });
        let Some(index) = index else {
            return self.open_tab(context_id, None, BrowserTabOrigin::User);
        };
        if !self.tabs.is_empty() {
            self.stop_screencast(self.active_index)?;
        }
        self.active_index = index;
        self.active_context_id = context_id.to_owned();
        let tab_id = self.tabs[index].public.id.clone();
        self.active_tab_ids
            .insert(context_id.to_owned(), tab_id.clone());
        self.cdp
            .request("Target.activateTarget", json!({"targetId": tab_id}), None)?;
        self.start_screencast(index)?;
        self.refresh_tab(index)?;
        self.emit_tabs(context_id);
        Ok(())
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<(), String> {
        self.surface_viewport_width = width.clamp(MIN_VIEWPORT_WIDTH, MAX_VIEWPORT_WIDTH);
        self.surface_viewport_height = height.clamp(MIN_VIEWPORT_HEIGHT, MAX_VIEWPORT_HEIGHT);
        if self.viewport_overridden {
            return Ok(());
        }
        self.apply_viewport(self.surface_viewport_width, self.surface_viewport_height)
    }

    fn sync_surface_state(&mut self, context_id: Option<String>, visible: bool) {
        self.surface_context_id = context_id;
        self.surface_visible = visible && self.surface_context_id.is_some();
        if let Some(context_id) = self.surface_context_id.as_ref() {
            self.pending_visibility_context_ids.remove(context_id);
        }
    }

    fn set_agent_viewport(&mut self, width: u32, height: u32) -> Result<(), String> {
        self.viewport_overridden = true;
        self.apply_viewport(width, height)
    }

    fn reset_agent_viewport(&mut self) -> Result<(), String> {
        self.viewport_overridden = false;
        self.apply_viewport(self.surface_viewport_width, self.surface_viewport_height)
    }

    fn apply_viewport(&mut self, width: u32, height: u32) -> Result<(), String> {
        self.viewport_width = width;
        self.viewport_height = height;
        for index in 0..self.tabs.len() {
            self.set_viewport(index)?;
        }
        self.stop_screencast(self.active_index)?;
        self.start_screencast(self.active_index)?;
        Ok(())
    }

    fn click(
        &mut self,
        context_id: &str,
        x: u32,
        y: u32,
        button: BrowserMouseButton,
    ) -> Result<(), String> {
        self.activate_context(context_id)?;
        if button == BrowserMouseButton::Left {
            self.prepare_user_download_grant(context_id)?;
        }
        let session_id = self.active_tab()?.session_id.clone();
        let x = x.min(self.viewport_width.saturating_sub(1));
        let y = y.min(self.viewport_height.saturating_sub(1));
        let button = match button {
            BrowserMouseButton::Left => "left",
            BrowserMouseButton::Middle => "middle",
            BrowserMouseButton::Right => "right",
        };
        for event_type in ["mousePressed", "mouseReleased"] {
            self.cdp.request(
                "Input.dispatchMouseEvent",
                json!({
                    "type": event_type,
                    "x": x,
                    "y": y,
                    "button": button,
                    "clickCount": 1,
                }),
                Some(&session_id),
            )?;
        }
        Ok(())
    }

    fn scroll(
        &mut self,
        context_id: &str,
        x: u32,
        y: u32,
        delta_x: i32,
        delta_y: i32,
    ) -> Result<(), String> {
        self.activate_context(context_id)?;
        let session_id = self.active_tab()?.session_id.clone();
        self.cdp.request(
            "Input.dispatchMouseEvent",
            json!({
                "type": "mouseWheel",
                "x": x.min(self.viewport_width.saturating_sub(1)),
                "y": y.min(self.viewport_height.saturating_sub(1)),
                "deltaX": delta_x,
                "deltaY": delta_y,
            }),
            Some(&session_id),
        )?;
        Ok(())
    }

    fn key(&mut self, context_id: &str, input: &BrowserKeyInput) -> Result<(), String> {
        self.activate_context(context_id)?;
        let session_id = self.active_tab()?.session_id.clone();
        if let Some(text) = input.text.as_deref()
            && !input.alt
            && !input.control
            && !input.meta
            && !text.chars().any(char::is_control)
        {
            self.cdp
                .request("Input.insertText", json!({"text": text}), Some(&session_id))?;
            return Ok(());
        }
        let key = cdp_key_name(&input.key);
        if matches!(key.as_str(), "Enter" | " ") {
            self.prepare_user_download_grant(context_id)?;
        }
        let modifiers = u8::from(input.alt)
            | (u8::from(input.control) << 1)
            | (u8::from(input.meta) << 2)
            | (u8::from(input.shift) << 3);
        self.cdp.request(
            "Input.dispatchKeyEvent",
            json!({
                "type": "keyDown",
                "key": &key,
                "modifiers": modifiers,
            }),
            Some(&session_id),
        )?;
        self.cdp.request(
            "Input.dispatchKeyEvent",
            json!({
                "type": "keyUp",
                "key": &key,
                "modifiers": modifiers,
            }),
            Some(&session_id),
        )?;
        Ok(())
    }

    fn create_target(&mut self, url: &str) -> Result<String, String> {
        let result = self
            .cdp
            .request("Target.createTarget", json!({"url": url}), None)?;
        result
            .get("targetId")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| "Browser did not return a tab identifier".to_owned())
    }

    fn attach_target(
        &mut self,
        context_id: &str,
        target_id: String,
        origin: BrowserTabOrigin,
    ) -> Result<(), String> {
        let result = self.cdp.request(
            "Target.attachToTarget",
            json!({"targetId": target_id, "flatten": true}),
            None,
        )?;
        let session_id = result
            .get("sessionId")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| "Browser did not return a tab session".to_owned())?;
        self.cdp.request(
            "Target.setAutoAttach",
            json!({
                "autoAttach": true,
                "waitForDebuggerOnStart": false,
                "flatten": true,
            }),
            Some(&session_id),
        )?;
        self.cdp
            .request("Page.enable", json!({}), Some(&session_id))?;
        self.cdp.request(
            "Page.setLifecycleEventsEnabled",
            json!({"enabled": true}),
            Some(&session_id),
        )?;
        let agent_id = self.next_agent_tab_id;
        self.next_agent_tab_id = self.next_agent_tab_id.saturating_add(1);
        self.tabs.push(BrowserTabRuntime {
            agent_id,
            context_id: context_id.to_owned(),
            origin,
            mark: None,
            public: BrowserTab {
                id: target_id,
                url: String::new(),
                title: String::new(),
                loading: false,
                can_go_back: false,
                can_go_forward: false,
            },
            session_id,
        });
        self.set_viewport(self.tabs.len().saturating_sub(1))?;
        Ok(())
    }

    fn set_viewport(&mut self, index: usize) -> Result<(), String> {
        let session_id = self
            .tabs
            .get(index)
            .ok_or_else(|| "Browser tab is no longer available".to_owned())?
            .session_id
            .clone();
        self.cdp.request(
            "Emulation.setDeviceMetricsOverride",
            json!({
                "width": self.viewport_width,
                "height": self.viewport_height,
                "deviceScaleFactor": 1,
                "mobile": false,
            }),
            Some(&session_id),
        )?;
        Ok(())
    }

    fn start_screencast(&mut self, index: usize) -> Result<(), String> {
        let session_id = self
            .tabs
            .get(index)
            .ok_or_else(|| "Browser tab is no longer available".to_owned())?
            .session_id
            .clone();
        self.cdp.request(
            "Page.startScreencast",
            json!({
                "format": "jpeg",
                "quality": 80,
                "maxWidth": self.viewport_width,
                "maxHeight": self.viewport_height,
                "everyNthFrame": 1,
            }),
            Some(&session_id),
        )?;
        Ok(())
    }

    fn stop_screencast(&mut self, index: usize) -> Result<(), String> {
        let Some(tab) = self.tabs.get(index) else {
            return Ok(());
        };
        let session_id = tab.session_id.clone();
        self.cdp
            .request("Page.stopScreencast", json!({}), Some(&session_id))?;
        Ok(())
    }

    fn refresh_tab(&mut self, index: usize) -> Result<(), String> {
        let session_id = self
            .tabs
            .get(index)
            .ok_or_else(|| "Browser tab is no longer available".to_owned())?
            .session_id
            .clone();
        let history =
            self.cdp
                .request("Page.getNavigationHistory", json!({}), Some(&session_id))?;
        let current = history
            .get("currentIndex")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let entries = history
            .get("entries")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let tab = &mut self.tabs[index].public;
        tab.can_go_back = current > 0;
        tab.can_go_forward = current >= 0 && (current as usize + 1) < entries.len();
        if let Some(entry) = entries.get(current.max(0) as usize) {
            if let Some(url) = entry.get("url").and_then(Value::as_str) {
                tab.url = bounded_text(url, MAX_BROWSER_URL_BYTES);
            }
            if let Some(title) = entry.get("title").and_then(Value::as_str) {
                tab.title = bounded_text(title, MAX_BROWSER_TITLE_BYTES);
            }
        }
        Ok(())
    }

    fn emit_frame(&self, encoded: &str) -> Result<(), String> {
        if encoded.len() > MAX_BROWSER_FRAME_BYTES.saturating_mul(4).div_ceil(3) + 4 {
            return Err("Browser frame exceeded its size limit".to_owned());
        }
        let jpeg = BASE64
            .decode(encoded)
            .map_err(|_| "Browser frame was not valid base64".to_owned())?;
        if jpeg.len() > MAX_BROWSER_FRAME_BYTES {
            return Err("Browser frame exceeded its size limit".to_owned());
        }
        let decoder = JpegDecoder::new(Cursor::new(&jpeg))
            .map_err(|_| "Browser frame was not a valid JPEG".to_owned())?;
        let (width, height) = decoder.dimensions();
        if width == 0 || height == 0 || width > MAX_VIEWPORT_WIDTH || height > MAX_VIEWPORT_HEIGHT {
            return Err("Browser frame dimensions exceeded their limits".to_owned());
        }
        let tab_id = self.active_tab()?.public.id.clone();
        let context_id = self.active_tab()?.context_id.clone();
        self.ui_events.latest_frame.replace(BrowserEvent::Frame {
            context_id,
            tab_id,
            jpeg,
            width,
            height,
        });
        Ok(())
    }

    fn set_loading(&mut self, loading: bool) {
        if let Some(tab) = self.tabs.get_mut(self.active_index) {
            tab.public.loading = loading;
        }
        let context_id = self.active_context_id.clone();
        self.emit_tabs(&context_id);
    }

    fn emit_tabs(&self, context_id: &str) {
        let active_tab_id = self.active_tab_ids.get(context_id).cloned().or_else(|| {
            self.tabs
                .iter()
                .find(|tab| tab.context_id == context_id)
                .map(|tab| tab.public.id.clone())
        });
        let Some(active_tab_id) = active_tab_id else {
            return;
        };
        self.emit(BrowserEvent::TabsChanged {
            context_id: context_id.to_owned(),
            tabs: self
                .tabs
                .iter()
                .filter(|tab| tab.context_id == context_id)
                .map(|tab| tab.public.clone())
                .collect(),
            active_tab_id,
        });
    }

    fn emit(&self, event: BrowserEvent) {
        let _ = self.ui_events.control.try_send(event);
    }

    fn active_tab(&self) -> Result<&BrowserTabRuntime, String> {
        self.tabs
            .get(self.active_index)
            .ok_or_else(|| "Browser tab is no longer available".to_owned())
    }

    fn tab_index(&self, target_id: &str) -> Option<usize> {
        self.tabs.iter().position(|tab| tab.public.id == target_id)
    }

    fn tab_index_in_context(&self, context_id: &str, target_id: &str) -> Option<usize> {
        self.tabs
            .iter()
            .position(|tab| tab.context_id == context_id && tab.public.id == target_id)
    }

    fn tab_index_for_session(&self, session_id: Option<&str>) -> Option<usize> {
        let session_id = session_id?;
        self.tabs
            .iter()
            .position(|tab| tab.session_id == session_id)
    }
}

fn agent_tab_id(params: &Value) -> Result<u64, BrowserAgentRpcError> {
    params
        .get("tabId")
        .and_then(Value::as_u64)
        .filter(|tab_id| *tab_id > 0)
        .ok_or_else(|| BrowserAgentRpcError::request("Browser request requires an integer tabId"))
}

fn checked_agent_browser_id(params: &Value) -> Result<&str, BrowserAgentRpcError> {
    params
        .get("browser_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| {
            !id.is_empty()
                && id.len() <= MAX_BROWSER_AGENT_ID_BYTES
                && !id.chars().any(char::is_control)
        })
        .ok_or_else(|| BrowserAgentRpcError::request("Browser command requires a browser_id"))
}

fn checked_agent_viewport(params: &Value) -> Result<(u32, u32), BrowserAgentRpcError> {
    let width = params
        .get("width")
        .and_then(Value::as_u64)
        .and_then(|width| u32::try_from(width).ok())
        .filter(|width| (MIN_VIEWPORT_WIDTH..=MAX_VIEWPORT_WIDTH).contains(width))
        .ok_or_else(|| {
            BrowserAgentRpcError::request(format!(
                "Browser viewport width must be between {MIN_VIEWPORT_WIDTH} and {MAX_VIEWPORT_WIDTH}"
            ))
        })?;
    let height = params
        .get("height")
        .and_then(Value::as_u64)
        .and_then(|height| u32::try_from(height).ok())
        .filter(|height| (MIN_VIEWPORT_HEIGHT..=MAX_VIEWPORT_HEIGHT).contains(height))
        .ok_or_else(|| {
            BrowserAgentRpcError::request(format!(
                "Browser viewport height must be between {MIN_VIEWPORT_HEIGHT} and {MAX_VIEWPORT_HEIGHT}"
            ))
        })?;
    Ok((width, height))
}

fn checked_download_url(value: Option<&str>) -> Result<String, BrowserAgentRpcError> {
    value
        .map(str::trim)
        .filter(|url| {
            !url.is_empty()
                && url.len() <= MAX_BROWSER_URL_BYTES
                && !url.chars().any(char::is_control)
        })
        .map(str::to_owned)
        .ok_or_else(|| BrowserAgentRpcError::request("allowDownload requires a non-empty url"))
}

fn checked_download_event_text(
    value: Option<&str>,
    max_bytes: usize,
    error: &str,
) -> Result<String, String> {
    value
        .filter(|value| {
            !value.is_empty() && value.len() <= max_bytes && !value.chars().any(char::is_control)
        })
        .map(str::to_owned)
        .ok_or_else(|| error.to_owned())
}

fn download_frame_ids(frame_tree: &Value) -> HashSet<String> {
    let mut ids = HashSet::with_capacity(4);
    let mut pending = Vec::with_capacity(4);
    if let Some(root) = frame_tree.get("frameTree") {
        pending.push(root);
    }
    while let Some(node) = pending.pop() {
        if ids.len() >= MAX_BROWSER_DOWNLOAD_FRAMES {
            break;
        }
        if let Some(id) = node
            .pointer("/frame/id")
            .and_then(Value::as_str)
            .filter(|id| {
                !id.is_empty()
                    && id.len() <= MAX_BROWSER_DOWNLOAD_ID_BYTES
                    && !id.chars().any(char::is_control)
            })
        {
            ids.insert(id.to_owned());
        }
        let remaining = MAX_BROWSER_DOWNLOAD_FRAMES
            .saturating_sub(ids.len())
            .saturating_sub(pending.len());
        if remaining == 0 {
            continue;
        }
        if let Some(children) = node.get("childFrames").and_then(Value::as_array) {
            pending.extend(children.iter().take(remaining).rev());
        }
    }
    ids
}

fn safe_download_filename(value: &str) -> String {
    let filename = value.rsplit(['/', '\\']).next().unwrap_or_default().trim();
    let mut sanitized =
        String::with_capacity(filename.len().min(MAX_BROWSER_DOWNLOAD_FILENAME_BYTES));
    for character in filename.chars() {
        if character.is_control() || matches!(character, '<' | '>' | ':' | '"' | '|' | '?' | '*') {
            if sanitized.len() < MAX_BROWSER_DOWNLOAD_FILENAME_BYTES {
                sanitized.push('_');
            }
            continue;
        }
        if sanitized.len().saturating_add(character.len_utf8())
            > MAX_BROWSER_DOWNLOAD_FILENAME_BYTES
        {
            break;
        }
        sanitized.push(character);
    }
    if sanitized.is_empty() || matches!(sanitized.as_str(), "." | "..") {
        "download".to_owned()
    } else {
        sanitized
    }
}

fn control_download_transfer(
    cdp: &mut CdpClient,
    browser_family: BrowserFamily,
    download: &BrowserDownloadRuntime,
    control: BrowserDownloadControl,
) -> Result<bool, String> {
    let target = cdp.request(
        "Target.createTarget",
        json!({"url": browser_family.downloads_url()}),
        None,
    )?;
    let target_id = target
        .get("targetId")
        .and_then(Value::as_str)
        .ok_or_else(|| "Browser did not create its Downloads controller".to_owned())?
        .to_owned();
    let result = (|| {
        let attached = cdp.request(
            "Target.attachToTarget",
            json!({"targetId": &target_id, "flatten": true}),
            None,
        )?;
        let session_id = attached
            .get("sessionId")
            .and_then(Value::as_str)
            .ok_or_else(|| "Browser did not attach its Downloads controller".to_owned())?
            .to_owned();
        cdp.request("Page.enable", json!({}), Some(&session_id))?;
        cdp.request("Runtime.enable", json!({}), Some(&session_id))?;

        let expression = match browser_family {
            BrowserFamily::Chrome => chrome_download_control_expression(download, control)?,
            BrowserFamily::Edge => {
                let ready_deadline = Instant::now() + BROWSER_DOWNLOAD_CONTROL_TIMEOUT;
                loop {
                    if cdp_runtime_value(
                        cdp,
                        &session_id,
                        "document.querySelector('downloads-full-page-app')?.shadowRoot?.querySelector('downloads-list') != null",
                    )? == json!(true)
                    {
                        break;
                    }
                    if Instant::now() >= ready_deadline {
                        return Err(browser_download_control_error(control));
                    }
                    thread::sleep(Duration::from_millis(20));
                }
                let tree = cdp.request("Page.getResourceTree", json!({}), Some(&session_id))?;
                let module_urls = tree
                    .pointer("/frameTree/resources")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|resource| resource.get("url").and_then(Value::as_str))
                    .filter(|url| url.starts_with("edge://downloads/") && url.ends_with(".js"))
                    .take(MAX_BROWSER_DOWNLOAD_WEBUI_RESOURCES)
                    .map(str::to_owned)
                    .collect::<Vec<_>>();
                edge_download_control_expression(download, control, &module_urls)?
            }
        };
        let value = cdp_runtime_value(cdp, &session_id, &expression)?;
        browser_download_control_result(&value, control)
    })();
    let _ = cdp.request("Target.closeTarget", json!({"targetId": &target_id}), None);
    result
}

fn browser_download_control_error(control: BrowserDownloadControl) -> String {
    match control {
        BrowserDownloadControl::Pause => "download-not-pausable",
        BrowserDownloadControl::Resume => "download-not-resumable",
    }
    .to_owned()
}

fn chrome_download_control_expression(
    download: &BrowserDownloadRuntime,
    control: BrowserDownloadControl,
) -> Result<String, String> {
    let input = serde_json::to_string(&json!({
        "control": match control {
            BrowserDownloadControl::Pause => "pause",
            BrowserDownloadControl::Resume => "resume",
        },
        "path": download.staging_path.to_string_lossy(),
        "timeoutMs": BROWSER_DOWNLOAD_CONTROL_TIMEOUT.as_millis() as u64,
        "url": download.url,
    }))
    .map_err(|error| format!("could not encode Browser download control: {error}"))?;
    let mut expression = String::with_capacity(input.len().saturating_add(2_500));
    expression.push_str(
        r#"(async (input) => {
const sleep = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));
const failure = () => ({ok: false, error: input.control === 'pause' ? 'download-not-pausable' : 'download-not-resumable'});
const deadline = Date.now() + input.timeoutMs;
let manager = null;
let item = null;
const findItem = () => {
    const items = manager?.items_ ?? [];
    return items.find(candidate => {
        const candidateUrl = typeof candidate.url === 'string' ? candidate.url : candidate.url?.url;
        return candidateUrl === input.url || candidate.filePath === input.path;
    });
};
while (Date.now() < deadline) {
    manager = document.querySelector('downloads-manager');
    item = findItem();
    if (manager?.mojoHandler_ != null && item != null) break;
    await sleep(20);
}
if (manager?.mojoHandler_ == null || item == null) return failure();
if (input.control === 'pause') {
    if (Boolean(item.resume)) return failure();
    await manager.mojoHandler_.pause(item.id);
    const settleDeadline = Date.now() + input.timeoutMs;
    while (Date.now() < settleDeadline) {
        item = findItem();
        if (item != null && Boolean(item.resume)) return {ok: true, canResume: true};
        await sleep(20);
    }
    return failure();
}
if (!Boolean(item.resume)) return failure();
await manager.mojoHandler_.resume(item.id);
const settleDeadline = Date.now() + input.timeoutMs;
while (Date.now() < settleDeadline) {
    item = findItem();
    if (item != null && !Boolean(item.resume)) return {ok: true, canResume: false};
    await sleep(20);
}
return failure();
})("#,
    );
    expression.push_str(&input);
    expression.push(')');
    Ok(expression)
}

fn edge_download_control_expression(
    download: &BrowserDownloadRuntime,
    control: BrowserDownloadControl,
    module_urls: &[String],
) -> Result<String, String> {
    let input = serde_json::to_string(&json!({
        "control": match control {
            BrowserDownloadControl::Pause => "pause",
            BrowserDownloadControl::Resume => "resume",
        },
        "moduleUrls": module_urls,
        "path": download.staging_path.to_string_lossy(),
        "timeoutMs": BROWSER_DOWNLOAD_CONTROL_TIMEOUT.as_millis() as u64,
    }))
    .map_err(|error| format!("could not encode Edge download control: {error}"))?;
    let mut expression = String::with_capacity(input.len().saturating_add(3_500));
    expression.push_str(
        r#"(async (input) => {
const sleep = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));
const failure = () => ({ok: false, error: input.control === 'pause' ? 'download-not-pausable' : 'download-not-resumable'});
let service = null;
for (const url of input.moduleUrls) {
    try {
        const module = await import(url);
        service = Object.values(module).find(value =>
            value != null &&
            typeof value === 'object' &&
            typeof value.getHandler === 'function' &&
            typeof value.refreshDownloadList === 'function'
        );
        if (service != null) break;
    } catch {}
}
if (service == null) return failure();
const list = document.querySelector('downloads-full-page-app')?.shadowRoot?.querySelector('downloads-list');
if (list == null) return failure();
const items = () => [...(list._downloads ?? [])].flatMap(group => [...(group.downloads ?? group._downloads ?? [])]);
const findItem = () => items().find(candidate => {
    try {
        return new URL(candidate.icon ?? candidate._icon).searchParams.get('path') === input.path;
    } catch {
        return false;
    }
});
const canResume = item => {
    const actions = item?.actions ?? item?._actions;
    return [...(actions?.trailing ?? [])].some(action => (action.type ?? action._type) === 'resume');
};
const refresh = async () => {
    await service.refreshDownloadList(false);
    await sleep(25);
    return findItem();
};
const deadline = Date.now() + input.timeoutMs;
let item = null;
while (Date.now() < deadline) {
    item = await refresh();
    if (item != null) break;
}
if (item == null) return failure();
const handler = await service.getHandler();
if (input.control === 'pause') {
    if (canResume(item)) return failure();
    await handler.pause(item.id);
    const settleDeadline = Date.now() + input.timeoutMs;
    while (Date.now() < settleDeadline) {
        item = await refresh();
        if (item != null && canResume(item)) return {ok: true, canResume: true};
    }
    return failure();
}
if (!canResume(item)) return failure();
await handler.resume(item.id);
const settleDeadline = Date.now() + input.timeoutMs;
while (Date.now() < settleDeadline) {
    item = await refresh();
    if (item != null && !canResume(item) && (item.state ?? item._state) === 'in_progress') {
        return {ok: true, canResume: false};
    }
}
return failure();
})("#,
    );
    expression.push_str(&input);
    expression.push(')');
    Ok(expression)
}

fn browser_download_control_result(
    value: &Value,
    control: BrowserDownloadControl,
) -> Result<bool, String> {
    if value.get("ok").and_then(Value::as_bool) == Some(true) {
        let can_resume = value
            .get("canResume")
            .and_then(Value::as_bool)
            .ok_or_else(|| browser_download_control_error(control))?;
        return match (control, can_resume) {
            (BrowserDownloadControl::Pause, true) | (BrowserDownloadControl::Resume, false) => {
                Ok(can_resume)
            }
            _ => Err(browser_download_control_error(control)),
        };
    }
    Err(value
        .get("error")
        .and_then(Value::as_str)
        .map(|error| bounded_text(error, MAX_ERROR_BYTES))
        .unwrap_or_else(|| browser_download_control_error(control)))
}

fn browser_download_path(download: &BrowserDownloadRuntime) -> &Path {
    download
        .destination
        .as_deref()
        .unwrap_or(&download.default_destination)
}

fn resolve_completed_download_path(
    download: &BrowserDownloadRuntime,
    event_path: Option<&str>,
) -> PathBuf {
    if let Some(path) = event_path
        .filter(|path| path.len() <= MAX_BROWSER_DOWNLOAD_PATH_BYTES && !path.contains('\0'))
        .map(PathBuf::from)
        .filter(|path| {
            path.parent() == Some(download.staging_directory.as_path()) && path.is_file()
        })
    {
        return path;
    }
    if download.staging_path.is_file() {
        return download.staging_path.clone();
    }
    let Ok(entries) = fs::read_dir(&download.staging_directory) else {
        return download.staging_path.clone();
    };
    let mut candidates = Vec::new();
    for entry in entries
        .take(MAX_BROWSER_DOWNLOAD_DIRECTORY_ENTRIES)
        .flatten()
    {
        let path = entry.path();
        if entry.file_type().is_ok_and(|kind| kind.is_file())
            && path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_none_or(|extension| !extension.eq_ignore_ascii_case("crdownload"))
        {
            candidates.push(path);
        }
    }
    candidates.sort();
    candidates
        .into_iter()
        .next()
        .unwrap_or_else(|| download.staging_path.clone())
}

fn move_download_to_destination(
    source: &Path,
    destination: &Path,
    overwrite: bool,
) -> Result<(), String> {
    if source == destination {
        return source
            .is_file()
            .then_some(())
            .ok_or_else(|| "The completed Browser download was missing.".to_owned());
    }
    if !source.is_file() || (!overwrite && destination.exists()) {
        return Err("The completed Browser download could not be finalized.".to_owned());
    }

    if !destination.exists() && fs::hard_link(source, destination).is_ok() {
        let _ = fs::remove_file(source);
        return Ok(());
    }

    let mut input = File::open(source)
        .map_err(|error| format!("could not open the completed Browser download: {error}"))?;
    let temporary = reserve_download_replacement_path(destination)?;
    let mut output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|error| format!("could not create the Browser download: {error}"))?;
    if let Err(error) = io::copy(&mut input, &mut output).and_then(|_| output.sync_all()) {
        drop(output);
        let _ = fs::remove_file(&temporary);
        return Err(format!("could not finalize the Browser download: {error}"));
    }
    drop(output);
    if let Err(error) = replace_download_destination(&temporary, destination, overwrite) {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    let _ = fs::remove_file(source);
    Ok(())
}

fn reserve_download_replacement_path(destination: &Path) -> Result<PathBuf, String> {
    let parent = destination
        .parent()
        .ok_or_else(|| "The Browser download destination has no parent.".to_owned())?;
    let filename = destination
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "download".to_owned());
    for sequence in 1..=MAX_BROWSER_DOWNLOAD_FILENAME_ATTEMPTS {
        let candidate = parent.join(format!(".{filename}.codexrs-{sequence}.tmp"));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err("The Browser could not reserve a temporary download path.".to_owned())
}

fn replace_download_destination(
    temporary: &Path,
    destination: &Path,
    overwrite: bool,
) -> Result<(), String> {
    if !destination.exists() {
        return fs::rename(temporary, destination)
            .map_err(|error| format!("could not install the Browser download: {error}"));
    }
    if !overwrite {
        return Err("The completed Browser download could not be finalized.".to_owned());
    }

    let parent = destination
        .parent()
        .ok_or_else(|| "The Browser download destination has no parent.".to_owned())?;
    let filename = destination
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "download".to_owned());
    let backup = (1..=MAX_BROWSER_DOWNLOAD_FILENAME_ATTEMPTS)
        .map(|sequence| parent.join(format!(".{filename}.codexrs-{sequence}.backup")))
        .find(|candidate| !candidate.exists())
        .ok_or_else(|| "The Browser could not reserve a download backup path.".to_owned())?;
    fs::rename(destination, &backup)
        .map_err(|error| format!("could not preserve the existing download: {error}"))?;
    if let Err(error) = fs::rename(temporary, destination) {
        let _ = fs::rename(&backup, destination);
        return Err(format!("could not replace the Browser download: {error}"));
    }
    let _ = fs::remove_file(backup);
    Ok(())
}

fn download_event_bytes(params: &Value, key: &str) -> u64 {
    params.get(key).and_then(Value::as_u64).unwrap_or_else(|| {
        params
            .get(key)
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite() && *value >= 0.0)
            .map_or(0, |value| value.min(u64::MAX as f64).round() as u64)
    })
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

const fn browser_agent_download_status(status: BrowserDownloadStatus) -> &'static str {
    match status {
        BrowserDownloadStatus::Started => "started",
        BrowserDownloadStatus::InProgress => "in_progress",
        BrowserDownloadStatus::Paused => "in_progress",
        BrowserDownloadStatus::Failed => "failed",
        BrowserDownloadStatus::Canceled => "canceled",
        BrowserDownloadStatus::Complete => "complete",
    }
}

fn open_platform_path(path: &Path, reveal: bool) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("The Browser download path was not absolute.".to_owned());
    }
    #[cfg(windows)]
    let mut command = {
        let mut command = Command::new("explorer.exe");
        if reveal && path.is_file() {
            command.arg("/select,").arg(path);
        } else {
            command.arg(path);
        }
        command.creation_flags(CREATE_NO_WINDOW);
        command
    };
    #[cfg(target_os = "linux")]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(if reveal && path.is_file() {
            path.parent().unwrap_or(path)
        } else {
            path
        });
        command
    };
    #[cfg(not(any(windows, target_os = "linux")))]
    let mut command = {
        let _ = reveal;
        return Err("Opening Browser downloads is not supported on this platform.".to_owned());
    };

    crate::process::spawn_detached_bounded(&mut command)
        .map_err(|error| format!("could not open the Browser download: {error}"))
}

fn checked_agent_target_id(value: Option<&Value>) -> Result<String, BrowserAgentRpcError> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|target_id| is_bounded_agent_identifier(target_id))
        .map(str::to_owned)
        .ok_or_else(|| BrowserAgentRpcError::request("Browser request requires a targetId"))
}

fn is_bounded_agent_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_BROWSER_AGENT_ID_BYTES
        && !value.chars().any(char::is_control)
}

fn finite_coordinate(value: Option<&Value>, extent: u32) -> Result<f64, BrowserAgentRpcError> {
    value
        .and_then(Value::as_f64)
        .filter(|coordinate| coordinate.is_finite())
        .map(|coordinate| coordinate.clamp(0.0, extent.saturating_sub(1) as f64))
        .ok_or_else(|| BrowserAgentRpcError::request("Browser cursor coordinate was invalid"))
}

fn allowed_agent_cdp_method(method: &str) -> bool {
    if matches!(
        method,
        "Browser.getVersion" | "Browser.getBrowserCommandLine"
    ) {
        return true;
    }
    let Some((domain, command)) = method.split_once('.') else {
        return false;
    };
    if domain == "Target" {
        return false;
    }
    if domain == "Page" && matches!(command, "startScreencast" | "stopScreencast") {
        return false;
    }
    matches!(
        domain,
        "Accessibility"
            | "Animation"
            | "Audits"
            | "CacheStorage"
            | "CSS"
            | "DOM"
            | "DOMDebugger"
            | "DOMSnapshot"
            | "DOMStorage"
            | "Database"
            | "Debugger"
            | "Emulation"
            | "Fetch"
            | "HeadlessExperimental"
            | "IO"
            | "IndexedDB"
            | "Input"
            | "LayerTree"
            | "Log"
            | "Media"
            | "Memory"
            | "Network"
            | "Overlay"
            | "Page"
            | "Performance"
            | "PerformanceTimeline"
            | "Profiler"
            | "Runtime"
            | "Schema"
            | "Security"
            | "ServiceWorker"
            | "Storage"
            | "WebAudio"
            | "WebAuthn"
    )
}

fn graceful_browser_exit(child: &mut Child, cdp: Option<&mut CdpClient>) {
    if let Some(cdp) = cdp {
        cdp.close_browser();
    }
    let deadline = Instant::now() + GRACEFUL_BROWSER_EXIT;
    while Instant::now() < deadline {
        if child.try_wait().ok().flatten().is_some() {
            return;
        }
        thread::sleep(BROWSER_TICK);
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn bounded_error(error: String) -> String {
    bounded_text(&error, MAX_ERROR_BYTES)
}

fn cdp_key_name(key: &str) -> String {
    match key.to_ascii_lowercase().as_str() {
        "enter" => "Enter",
        "tab" => "Tab",
        "backspace" => "Backspace",
        "delete" => "Delete",
        "escape" | "esc" => "Escape",
        "space" => " ",
        "left" | "arrowleft" => "ArrowLeft",
        "right" | "arrowright" => "ArrowRight",
        "up" | "arrowup" => "ArrowUp",
        "down" | "arrowdown" => "ArrowDown",
        "home" => "Home",
        "end" => "End",
        "pageup" => "PageUp",
        "pagedown" => "PageDown",
        other => return other.to_owned(),
    }
    .to_owned()
}

fn bounded_text(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    value[..end].to_owned()
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, VecDeque};
    use std::error::Error;
    use std::fs;
    use std::io::{self, Read as _, Write as _};
    use std::net::TcpListener;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    #[cfg(unix)]
    use std::os::unix::process::CommandExt;
    use std::path::{Path, PathBuf};
    #[cfg(unix)]
    use std::process::{Command, Stdio};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    use codex_core::{
        BrowserPermissionResource, BrowserPermissionValue, BrowserPermissionsState,
        BrowserSitePermission,
    };
    use crossbeam_channel::bounded;
    use serde_json::json;

    use super::{
        BROWSER_COMMAND_CAPACITY, BrowserAgentChildSession, BrowserCommand, BrowserCommandError,
        BrowserConfig, BrowserDownloadControl, BrowserDownloadRuntime, BrowserEvent, BrowserFamily,
        BrowserTab, BrowserTabOrigin, BrowserTabRuntime, CDP_SHUTDOWN_REQUESTED, CdpClient,
        DevToolsEndpoint, LatestBrowserFrame, MAX_AGENT_CHILD_SESSIONS, MAX_BROWSER_URL_BYTES,
        MAX_CDP_PENDING_EVENT_BYTES, PendingCdpEvents, agent_tab_id_for_cdp_session,
        allowed_agent_cdp_method, browser_agent_tab_matches, browser_command,
        browser_origin_pattern_matches, browser_permission_for_url, cached_agent_expression_key,
        cdp_key_name, checked_agent_browser_id, checked_agent_viewport, checked_download_url,
        checked_navigation_url, control_download_transfer, create_browser_job,
        graceful_browser_exit, move_download_to_destination, next_browser_command,
        normalize_browser_origin, parse_devtools_marker, record_agent_child_session,
        remove_agent_child_session, resolve_browser_binary, safe_download_filename,
        screencast_frame_ack, try_recv_browser_event, wait_for_devtools_endpoint,
    };
    use super::{
        MAX_XDG_USER_DIRS_BYTES, decode_xdg_double_quoted, linux_user_dirs_config_path,
        parse_xdg_download_dir, read_xdg_download_dir,
    };
    #[cfg(unix)]
    use super::{find_path_executable_in, is_browser_executable, release_browser_job};

    fn runtime_value(
        cdp: &mut CdpClient,
        session_id: &str,
        expression: &str,
    ) -> Result<serde_json::Value, io::Error> {
        let result = cdp
            .request(
                "Runtime.evaluate",
                json!({
                    "awaitPromise": true,
                    "expression": expression,
                    "returnByValue": true,
                }),
                Some(session_id),
            )
            .map_err(io::Error::other)?;
        if let Some(description) = result
            .pointer("/exceptionDetails/exception/description")
            .and_then(serde_json::Value::as_str)
        {
            return Err(io::Error::other(description.to_owned()));
        }
        result
            .pointer("/result/value")
            .cloned()
            .ok_or_else(|| io::Error::other("Chromium did not return a WebUI value"))
    }

    #[cfg(unix)]
    #[test]
    fn unix_browser_resolver_requires_an_executable_file() -> Result<(), Box<dyn Error>> {
        let root = std::env::temp_dir().join(format!(
            "codexrs-browser-resolver-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        ));
        let result = (|| {
            let first = root.join("first");
            let second = root.join("second");
            fs::create_dir_all(&first)?;
            fs::create_dir_all(&second)?;
            let non_executable = first.join("chromium");
            let executable = second.join("chromium");
            fs::write(&non_executable, b"not executable")?;
            fs::write(&executable, b"executable")?;

            let mut non_executable_permissions = fs::metadata(&non_executable)?.permissions();
            non_executable_permissions.set_mode(0o644);
            fs::set_permissions(&non_executable, non_executable_permissions)?;
            let mut executable_permissions = fs::metadata(&executable)?.permissions();
            executable_permissions.set_mode(0o100);
            fs::set_permissions(&executable, executable_permissions)?;

            assert!(!is_browser_executable(&first));
            assert!(!is_browser_executable(&non_executable));
            assert!(is_browser_executable(&executable));
            assert_eq!(
                find_path_executable_in(vec![first, second], &["chromium"]),
                Some(executable)
            );
            Ok::<(), io::Error>(())
        })();
        let _ = fs::remove_dir_all(&root);
        result?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn unix_browser_job_kills_same_group_descendants() -> Result<(), Box<dyn Error>> {
        let marker = std::env::temp_dir().join(format!(
            "codexrs-browser-job-{}-{}.marker",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        ));
        let _ = fs::remove_file(&marker);
        let mut command = Command::new("sh");
        command
            .args([
                "-c",
                "trap '' HUP; (sleep 1; printf x > \"$CODEXRS_BROWSER_JOB_MARKER\") & exit",
            ])
            .env("CODEXRS_BROWSER_JOB_MARKER", &marker)
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = command.spawn()?;
        let job = create_browser_job(&mut child).map_err(io::Error::other)?;
        assert!(child.wait()?.success());

        release_browser_job(job);
        thread::sleep(Duration::from_millis(1_200));
        let descendant_ran = marker.exists();
        let _ = fs::remove_file(&marker);
        assert!(
            !descendant_ran,
            "a Browser process-group descendant ran after its job was dropped"
        );
        Ok(())
    }

    #[test]
    fn browser_frames_coalesce_without_displacing_control_events() {
        let (sender, receiver) = bounded(1);
        let latest_frame = LatestBrowserFrame::default();
        latest_frame.replace(BrowserEvent::Frame {
            context_id: "task".to_owned(),
            tab_id: "tab".to_owned(),
            jpeg: vec![1],
            width: 1,
            height: 1,
        });
        latest_frame.replace(BrowserEvent::Frame {
            context_id: "task".to_owned(),
            tab_id: "tab".to_owned(),
            jpeg: vec![2],
            width: 1,
            height: 1,
        });
        assert!(sender.try_send(BrowserEvent::Exited).is_ok());
        drop(sender);

        assert_eq!(
            try_recv_browser_event(&receiver, &latest_frame),
            Ok(Some(BrowserEvent::Exited))
        );
        assert_eq!(
            try_recv_browser_event(&receiver, &latest_frame),
            Ok(Some(BrowserEvent::Frame {
                context_id: "task".to_owned(),
                tab_id: "tab".to_owned(),
                jpeg: vec![2],
                width: 1,
                height: 1,
            }))
        );
        assert_eq!(
            try_recv_browser_event(&receiver, &latest_frame),
            Err(BrowserCommandError::Disconnected)
        );
    }

    #[test]
    fn linux_xdg_download_dir_accepts_literal_and_home_paths() {
        let home = std::env::temp_dir().join("codexrs-home");
        let literal = std::env::temp_dir().join("codexrs-downloads");
        let home_value = "XDG_DOWNLOAD_DIR=\"$HOME/Downloads\"\n".to_owned();
        let literal = literal.to_string_lossy().replace('\\', "/");
        let literal_value = format!("XDG_DOWNLOAD_DIR=\"{literal}\"\n");
        assert_eq!(
            parse_xdg_download_dir(home_value.as_bytes(), Some(&home)),
            Some(home.join("Downloads"))
        );
        assert_eq!(
            parse_xdg_download_dir(literal_value.as_bytes(), Some(&home)),
            Some(PathBuf::from(literal))
        );
    }

    #[test]
    fn linux_xdg_download_dir_decodes_safe_shell_escapes_and_continuations() {
        assert_eq!(
            decode_xdg_double_quoted("\"/srv/\\\\folder/\\\"quoted\\\"/\\$cash/\\`tick\\`\""),
            Some("/srv/\\folder/\"quoted\"/$cash/`tick`".to_owned())
        );

        let home = std::env::temp_dir().join("codexrs-home");
        assert_eq!(
            parse_xdg_download_dir(b"XDG_DOWNLOAD_DIR=\"$HOME/Down\\\nloads\"\n", Some(&home)),
            Some(home.join("Downloads"))
        );
        assert_eq!(
            parse_xdg_download_dir(
                b"# generated\r\nXDG_DOWNLOAD_DIR=\"\\$HOME/Downloads\"\r\n",
                Some(&home),
            ),
            None
        );
    }

    #[test]
    fn linux_xdg_download_dir_rejects_unsafe_or_unresolvable_values() {
        let home = std::env::temp_dir().join("codexrs-home");
        assert_eq!(
            parse_xdg_download_dir(b"XDG_DOWNLOAD_DIR=\"Downloads\"\n", Some(&home)),
            None
        );
        assert_eq!(
            parse_xdg_download_dir(b"XDG_DOWNLOAD_DIR=\"$HOME/../other\"\n", Some(&home)),
            None
        );
        assert_eq!(
            parse_xdg_download_dir(b"XDG_DOWNLOAD_DIR=\"$HOME/Downloads\"\n", None),
            None
        );
    }

    #[test]
    fn linux_xdg_download_dir_uses_absolute_xdg_config_or_home_config() {
        let home = std::env::temp_dir().join("codexrs-home");
        let config_home = std::env::temp_dir().join("codexrs-config");
        assert_eq!(
            linux_user_dirs_config_path(Some(&config_home), Some(&home)),
            Some(config_home.join("user-dirs.dirs"))
        );
        assert_eq!(
            linux_user_dirs_config_path(None, Some(&home)),
            Some(home.join(".config").join("user-dirs.dirs"))
        );
        assert_eq!(
            linux_user_dirs_config_path(Some(Path::new("relative-config")), Some(&home)),
            Some(home.join(".config").join("user-dirs.dirs"))
        );
        assert_eq!(linux_user_dirs_config_path(None, None), None);
    }

    #[test]
    fn linux_xdg_download_dir_bounds_config_reads() -> Result<(), Box<dyn Error>> {
        let path = std::env::temp_dir().join(format!(
            "codexrs-user-dirs-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        ));
        fs::write(&path, vec![b'x'; MAX_XDG_USER_DIRS_BYTES + 1])?;
        let result = read_xdg_download_dir(&path, Some(&std::env::temp_dir()));
        fs::remove_file(&path)?;
        assert_eq!(result, None);
        Ok(())
    }

    #[test]
    fn linux_xdg_download_dir_rejects_directories() -> Result<(), Box<dyn Error>> {
        let path = std::env::temp_dir().join(format!(
            "codexrs-user-dirs-directory-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        ));
        fs::create_dir(&path)?;
        let result = read_xdg_download_dir(&path, Some(&std::env::temp_dir()));
        fs::remove_dir(&path)?;
        assert_eq!(result, None);
        Ok(())
    }

    #[test]
    fn navigation_is_limited_to_web_urls_and_blank_pages() {
        assert_eq!(
            checked_navigation_url(" https://example.com/path "),
            Ok("https://example.com/path".to_owned())
        );
        assert_eq!(
            checked_navigation_url("about:blank"),
            Ok("about:blank".to_owned())
        );
        assert_eq!(
            checked_navigation_url("file:///etc/passwd"),
            Err(BrowserCommandError::InvalidUrl)
        );
        assert_eq!(
            checked_navigation_url("javascript:alert(1)"),
            Err(BrowserCommandError::InvalidUrl)
        );
        assert_eq!(
            checked_navigation_url(&format!(
                "https://example.com/{}",
                "x".repeat(MAX_BROWSER_URL_BYTES)
            )),
            Err(BrowserCommandError::InvalidUrl)
        );
    }

    #[test]
    fn site_permissions_normalize_origins_and_apply_block_before_allow() {
        assert_eq!(
            normalize_browser_origin("Example.COM:443/path?secret=1"),
            Some("https://example.com".to_owned())
        );
        assert_eq!(normalize_browser_origin("file:///tmp/private"), None);
        assert_eq!(
            normalize_browser_origin("https://user:secret@example.com/private"),
            None
        );
        assert!(browser_origin_pattern_matches(
            "*.example.com",
            "https://docs.example.com",
            "docs.example.com"
        ));
        assert!(!browser_origin_pattern_matches(
            r"\*.example.com",
            "https://docs.example.com",
            "docs.example.com"
        ));

        let permissions = BrowserPermissionsState {
            sites: vec![
                BrowserSitePermission {
                    origin: "*.example.com".to_owned(),
                    browse: BrowserPermissionValue::Block,
                    download: BrowserPermissionValue::Default,
                    upload: BrowserPermissionValue::Default,
                    full_cdp: BrowserPermissionValue::Default,
                },
                BrowserSitePermission {
                    origin: "https://docs.example.com".to_owned(),
                    browse: BrowserPermissionValue::Allow,
                    download: BrowserPermissionValue::Block,
                    upload: BrowserPermissionValue::Default,
                    full_cdp: BrowserPermissionValue::Default,
                },
            ],
            ..BrowserPermissionsState::default()
        }
        .normalized();
        assert_eq!(
            browser_permission_for_url(
                &permissions,
                "https://docs.example.com/private?token=hidden",
                BrowserPermissionResource::Browse
            ),
            BrowserPermissionValue::Block
        );
        assert_eq!(
            browser_permission_for_url(
                &permissions,
                "https://docs.example.com/file.zip",
                BrowserPermissionResource::Download
            ),
            BrowserPermissionValue::Block
        );
    }

    #[test]
    fn devtools_marker_accepts_only_the_loopback_browser_endpoint_shape() {
        assert_eq!(
            parse_devtools_marker("43123\n/devtools/browser/abc-123\n"),
            Ok(DevToolsEndpoint {
                port: 43_123,
                path: "/devtools/browser/abc-123".to_owned(),
            })
        );
        assert!(parse_devtools_marker("0\n/devtools/browser/abc\n").is_err());
        assert!(parse_devtools_marker("43123\n/devtools/page/abc\n").is_err());
        assert!(parse_devtools_marker("43123\nws://remote/devtools/browser/abc\n").is_err());
    }

    #[test]
    fn gpui_navigation_keys_map_to_cdp_key_names() {
        assert_eq!(cdp_key_name("left"), "ArrowLeft");
        assert_eq!(cdp_key_name("ENTER"), "Enter");
        assert_eq!(cdp_key_name("a"), "a");
    }

    #[test]
    fn shutdown_flag_outranks_a_full_browser_command_queue() {
        let (sender, receiver) = bounded(1);
        let mut pending_commands = VecDeque::new();
        let shutdown_requested = AtomicBool::new(false);
        assert!(
            sender
                .try_send(BrowserCommand::SetPromptForUserDownloads(true))
                .is_ok()
        );
        shutdown_requested.store(true, Ordering::Release);

        assert!(matches!(
            next_browser_command(&shutdown_requested, &receiver, &mut pending_commands),
            Ok(Some(BrowserCommand::Shutdown))
        ));
        assert!(matches!(
            receiver.try_recv(),
            Ok(BrowserCommand::SetPromptForUserDownloads(true))
        ));
    }

    #[test]
    fn agent_focus_tab_rejects_a_tab_from_another_context() {
        let task_a_tab = BrowserTabRuntime {
            agent_id: 1,
            context_id: "task-a".to_owned(),
            origin: BrowserTabOrigin::Agent,
            mark: None,
            public: BrowserTab {
                id: "tab-a".to_owned(),
                url: String::new(),
                title: String::new(),
                loading: false,
                can_go_back: false,
                can_go_forward: false,
            },
            session_id: "session-a".to_owned(),
        };
        let task_b_tab = BrowserTabRuntime {
            agent_id: 2,
            context_id: "task-b".to_owned(),
            origin: BrowserTabOrigin::Agent,
            mark: None,
            public: BrowserTab {
                id: "tab-b".to_owned(),
                url: String::new(),
                title: String::new(),
                loading: false,
                can_go_back: false,
                can_go_forward: false,
            },
            session_id: "session-b".to_owned(),
        };

        assert!(browser_agent_tab_matches(&task_a_tab, "task-a", 1));
        assert!(!browser_agent_tab_matches(&task_b_tab, "task-a", 2));
    }

    #[test]
    fn cached_agent_expressions_are_scoped_to_context_and_tab() {
        let task_a_key =
            cached_agent_expression_key("task-a".to_owned(), 1, "shared-expression-key".to_owned());
        let task_b_key =
            cached_agent_expression_key("task-b".to_owned(), 2, "shared-expression-key".to_owned());
        let expressions = HashMap::from([(task_a_key, "document.title".to_owned())]);

        assert!(!expressions.contains_key(&task_b_key));
    }

    #[test]
    fn scoped_agent_child_sessions_inherit_only_owned_parent_sessions() {
        let root_sessions =
            HashMap::from([("root-a".to_owned(), 1_u64), ("root-b".to_owned(), 2_u64)]);
        let mut child_sessions = HashMap::<String, BrowserAgentChildSession>::new();

        record_agent_child_session(
            root_sessions
                .iter()
                .map(|(session_id, tab_id)| (session_id.as_str(), *tab_id)),
            &mut child_sessions,
            "unscoped-parent",
            "unowned-child",
            "unowned-target",
        );
        assert!(child_sessions.is_empty());

        record_agent_child_session(
            root_sessions
                .iter()
                .map(|(session_id, tab_id)| (session_id.as_str(), *tab_id)),
            &mut child_sessions,
            "root-a",
            "child-a",
            "target-a",
        );
        record_agent_child_session(
            root_sessions
                .iter()
                .map(|(session_id, tab_id)| (session_id.as_str(), *tab_id)),
            &mut child_sessions,
            "child-a",
            "nested-child-a",
            "nested-target-a",
        );
        record_agent_child_session(
            root_sessions
                .iter()
                .map(|(session_id, tab_id)| (session_id.as_str(), *tab_id)),
            &mut child_sessions,
            "root-b",
            "child-b",
            "target-b",
        );

        assert_eq!(
            agent_tab_id_for_cdp_session(
                root_sessions
                    .iter()
                    .map(|(session_id, tab_id)| (session_id.as_str(), *tab_id)),
                &child_sessions,
                "nested-child-a",
            ),
            Some(1)
        );
        assert_eq!(
            child_sessions
                .get("child-b")
                .map(|child| child.agent_tab_id),
            Some(2)
        );

        assert!(!remove_agent_child_session(
            root_sessions
                .iter()
                .map(|(session_id, tab_id)| (session_id.as_str(), *tab_id)),
            &mut child_sessions,
            "root-b",
            "child-a",
        ));
        assert!(child_sessions.contains_key("child-a"));
        assert!(remove_agent_child_session(
            root_sessions
                .iter()
                .map(|(session_id, tab_id)| (session_id.as_str(), *tab_id)),
            &mut child_sessions,
            "root-a",
            "child-a",
        ));
        assert!(!child_sessions.contains_key("child-a"));
        assert!(!child_sessions.contains_key("nested-child-a"));
        assert!(child_sessions.contains_key("child-b"));
    }

    #[test]
    fn scoped_agent_child_sessions_fail_closed_at_their_cap() {
        let root_sessions = HashMap::from([("root".to_owned(), 1_u64)]);
        let mut child_sessions = HashMap::<String, BrowserAgentChildSession>::new();
        for index in 0..=MAX_AGENT_CHILD_SESSIONS {
            record_agent_child_session(
                root_sessions
                    .iter()
                    .map(|(session_id, tab_id)| (session_id.as_str(), *tab_id)),
                &mut child_sessions,
                "root",
                &format!("child-{index}"),
                &format!("target-{index}"),
            );
        }

        assert_eq!(child_sessions.len(), MAX_AGENT_CHILD_SESSIONS);
        assert!(!child_sessions.contains_key(&format!("child-{MAX_AGENT_CHILD_SESSIONS}")));
    }

    #[test]
    fn raw_agent_cdp_rejects_target_domain_commands() {
        for method in [
            "Target.activateTarget",
            "Target.attachToTarget",
            "Target.closeTarget",
            "Target.detachFromTarget",
            "Target.getTargetInfo",
            "Target.getTargets",
            "Target.setAutoAttach",
            "Target.setDiscoverTargets",
        ] {
            assert!(!allowed_agent_cdp_method(method), "{method}");
        }
        assert!(allowed_agent_cdp_method("Page.navigate"));
        assert!(allowed_agent_cdp_method("Browser.getVersion"));
    }

    #[test]
    fn cdp_pending_events_evict_oldest_to_stay_within_byte_budget() {
        let mut events = PendingCdpEvents::new();
        let quarter_budget = MAX_CDP_PENDING_EVENT_BYTES / 4;
        let half_budget = MAX_CDP_PENDING_EVENT_BYTES / 2;

        assert!(
            events
                .push(json!({"method": "first"}), quarter_budget)
                .is_empty()
        );
        assert!(
            events
                .push(json!({"method": "second"}), half_budget)
                .is_empty()
        );
        let evicted = events.push(json!({"method": "third"}), half_budget);

        assert_eq!(events.retained_bytes, MAX_CDP_PENDING_EVENT_BYTES);
        assert_eq!(evicted, vec![json!({"method": "first"})]);
        assert_eq!(events.pop(), Some(json!({"method": "second"})));
        assert_eq!(events.pop(), Some(json!({"method": "third"})));
        assert_eq!(events.retained_bytes, 0);
        assert_eq!(events.pop(), None);
    }

    #[test]
    fn evicted_screencast_frames_extract_ack_details_only() {
        assert_eq!(
            screencast_frame_ack(&json!({
                "method": "Page.screencastFrame",
                "sessionId": "tab-session",
                "params": {"sessionId": 42},
            })),
            Some(("tab-session", 42)),
        );
        assert_eq!(
            screencast_frame_ack(&json!({"method": "Runtime.consoleAPICalled"})),
            None,
        );
    }

    #[test]
    fn cdp_request_stops_waiting_when_startup_shutdown_is_requested() -> Result<(), Box<dyn Error>>
    {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        let (request_received_sender, request_received_receiver) = bounded(1);
        let (finish_sender, finish_receiver) = bounded(1);
        let server = thread::spawn(move || {
            let Ok((connection, _)) = listener.accept() else {
                return false;
            };
            let Ok(mut socket) = tungstenite::accept(connection) else {
                return false;
            };
            if socket.read().is_err() || request_received_sender.send(()).is_err() {
                return false;
            }
            finish_receiver.recv_timeout(Duration::from_secs(2)).is_ok()
        });
        let shutdown_requested = Arc::new(AtomicBool::new(false));
        let shutdown_setter = {
            let shutdown_requested = Arc::clone(&shutdown_requested);
            thread::spawn(move || {
                if request_received_receiver
                    .recv_timeout(Duration::from_secs(2))
                    .is_ok()
                {
                    shutdown_requested.store(true, Ordering::Release);
                }
            })
        };
        let endpoint = DevToolsEndpoint {
            port,
            path: "/devtools/browser/test".to_owned(),
        };
        let mut cdp = CdpClient::connect(&endpoint, &shutdown_requested)?;

        let started = Instant::now();
        let result = cdp.request("Runtime.enable", json!({}), None);
        let elapsed = started.elapsed();
        let _ = finish_sender.send(());
        if shutdown_setter.join().is_err() || !matches!(server.join(), Ok(true)) {
            return Err("CDP shutdown fixture failed".into());
        }

        assert_eq!(result, Err(CDP_SHUTDOWN_REQUESTED.to_owned()));
        assert!(
            elapsed < Duration::from_secs(1),
            "CDP request waited for its normal timeout after shutdown"
        );
        Ok(())
    }

    #[test]
    fn cdp_connect_stops_waiting_when_handshake_shutdown_is_requested() -> Result<(), Box<dyn Error>>
    {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        let (request_received_sender, request_received_receiver) = bounded(1);
        let (finish_sender, finish_receiver) = bounded(1);
        let server = thread::spawn(move || {
            let Ok((mut connection, _)) = listener.accept() else {
                return false;
            };
            let mut request = [0_u8; 1024];
            if connection.read(&mut request).is_err() || request_received_sender.send(()).is_err() {
                return false;
            }
            finish_receiver.recv_timeout(Duration::from_secs(2)).is_ok()
        });
        let shutdown_requested = Arc::new(AtomicBool::new(false));
        let shutdown_setter = {
            let shutdown_requested = Arc::clone(&shutdown_requested);
            thread::spawn(move || {
                if request_received_receiver
                    .recv_timeout(Duration::from_secs(2))
                    .is_ok()
                {
                    shutdown_requested.store(true, Ordering::Release);
                }
            })
        };
        let endpoint = DevToolsEndpoint {
            port,
            path: "/devtools/browser/test".to_owned(),
        };

        let started = Instant::now();
        let result = CdpClient::connect(&endpoint, &shutdown_requested);
        let elapsed = started.elapsed();
        let _ = finish_sender.send(());
        if shutdown_setter.join().is_err() || !matches!(server.join(), Ok(true)) {
            return Err("CDP handshake shutdown fixture failed".into());
        }

        assert_eq!(result.map(|_| ()), Err(CDP_SHUTDOWN_REQUESTED.to_owned()));
        assert!(
            elapsed < Duration::from_secs(1),
            "CDP connect waited for the handshake timeout after shutdown"
        );
        Ok(())
    }

    #[test]
    fn cdp_close_does_not_wait_for_a_response_during_shutdown() -> Result<(), Box<dyn Error>> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        let (close_received_sender, close_received_receiver) = bounded(1);
        let (finish_sender, finish_receiver) = bounded(1);
        let server = thread::spawn(move || {
            let Ok((connection, _)) = listener.accept() else {
                return false;
            };
            let Ok(mut socket) = tungstenite::accept(connection) else {
                return false;
            };
            if socket.read().is_err() || close_received_sender.send(()).is_err() {
                return false;
            }
            finish_receiver.recv_timeout(Duration::from_secs(2)).is_ok()
        });
        let shutdown_requested = Arc::new(AtomicBool::new(false));
        let endpoint = DevToolsEndpoint {
            port,
            path: "/devtools/browser/test".to_owned(),
        };
        let mut cdp = CdpClient::connect(&endpoint, &shutdown_requested)?;
        shutdown_requested.store(true, Ordering::Release);

        let started = Instant::now();
        cdp.close_browser();
        assert!(
            close_received_receiver
                .recv_timeout(Duration::from_secs(1))
                .is_ok(),
            "Browser.close was not sent during shutdown"
        );
        let elapsed = started.elapsed();
        let _ = finish_sender.send(());
        if !matches!(server.join(), Ok(true)) {
            return Err("CDP close shutdown fixture failed".into());
        }

        assert!(
            elapsed < Duration::from_secs(1),
            "Browser.close waited for a CDP response during shutdown"
        );
        Ok(())
    }

    #[test]
    fn download_grants_preserve_exact_bounded_urls() {
        assert_eq!(
            checked_download_url(Some("blob:https://example.com/download-id"))
                .map_err(|error| error.message),
            Ok("blob:https://example.com/download-id".to_owned())
        );
        assert!(checked_download_url(Some("")).is_err());
        assert!(checked_download_url(Some("https://example.com/\0secret")).is_err());
    }

    #[test]
    fn suggested_download_names_cannot_escape_the_owned_directory() {
        assert_eq!(safe_download_filename(r"..\..\report?.pdf"), "report_.pdf");
        assert_eq!(safe_download_filename("../../"), "download");
        assert_eq!(safe_download_filename(""), "download");
    }

    #[test]
    fn completed_download_move_never_overwrites_an_existing_file()
    -> Result<(), Box<dyn std::error::Error>> {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "codexrs-browser-download-move-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&directory)?;
        let source = directory.join("source.txt");
        let destination = directory.join("destination.txt");
        fs::write(&source, b"new")?;
        fs::write(&destination, b"existing")?;

        assert!(move_download_to_destination(&source, &destination, false).is_err());
        assert_eq!(fs::read(&source)?, b"new");
        assert_eq!(fs::read(&destination)?, b"existing");

        fs::remove_file(&destination)?;
        move_download_to_destination(&source, &destination, false)?;
        assert!(!source.exists());
        assert_eq!(fs::read(&destination)?, b"new");
        fs::remove_file(&destination)?;
        fs::remove_dir(&directory)?;
        Ok(())
    }

    #[test]
    fn explicit_download_destination_replaces_only_the_selected_file()
    -> Result<(), Box<dyn std::error::Error>> {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "codexrs-browser-download-replace-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&directory)?;
        let source = directory.join("source.txt");
        let destination = directory.join("destination.txt");
        let untouched = directory.join("untouched.txt");
        fs::write(&source, b"new")?;
        fs::write(&destination, b"existing")?;
        fs::write(&untouched, b"keep")?;

        move_download_to_destination(&source, &destination, true)?;

        assert!(!source.exists());
        assert_eq!(fs::read(&destination)?, b"new");
        assert_eq!(fs::read(&untouched)?, b"keep");
        assert_eq!(fs::read_dir(&directory)?.count(), 2);

        fs::remove_file(&destination)?;
        fs::remove_file(&untouched)?;
        fs::remove_dir(&directory)?;
        Ok(())
    }

    #[test]
    #[ignore = "requires an installed Chromium browser"]
    fn live_chromium_downloads_webui_controls_active_download()
    -> Result<(), Box<dyn std::error::Error>> {
        let executable =
            resolve_browser_binary().ok_or("Chrome, Edge, or Chromium was not installed")?;
        let browser_family = BrowserFamily::from_executable(&executable);
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let profile_dir = std::env::temp_dir().join(format!(
            "codexrs-browser-download-control-{}-{nonce}",
            std::process::id()
        ));
        let download_dir = profile_dir.join("downloads");
        fs::create_dir_all(&download_dir)?;

        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let stop_server = Arc::new(AtomicBool::new(false));
        let server_stop = Arc::clone(&stop_server);
        let server = thread::spawn(move || -> io::Result<()> {
            let deadline = std::time::Instant::now() + Duration::from_secs(30);
            'accept: loop {
                if server_stop.load(Ordering::Relaxed) {
                    return Ok(());
                }
                match listener.accept() {
                    Ok((mut connection, _)) => {
                        connection.set_read_timeout(Some(Duration::from_secs(2)))?;
                        connection.set_write_timeout(Some(Duration::from_secs(2)))?;
                        let mut request = [0_u8; 4 * 1024];
                        let read = match connection.read(&mut request) {
                            Ok(0) => continue,
                            Ok(read) => read,
                            Err(error)
                                if matches!(
                                    error.kind(),
                                    io::ErrorKind::TimedOut
                                        | io::ErrorKind::WouldBlock
                                        | io::ErrorKind::Interrupted
                                ) =>
                            {
                                continue;
                            }
                            Err(error) => return Err(error),
                        };
                        let request = String::from_utf8_lossy(&request[..read]);
                        if request.starts_with("GET /page ") {
                            let page = b"<a id=\"download\" href=\"/slow.bin\">download</a>";
                            connection.write_all(
                                format!(
                                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                    page.len()
                                )
                                .as_bytes(),
                            )?;
                            connection.write_all(page)?;
                            continue;
                        }
                        if !request.starts_with("GET /slow.bin ") {
                            connection.write_all(
                                b"HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\n",
                            )?;
                            continue;
                        }
                        const TOTAL_BYTES: usize = 32 * 1024 * 1024;
                        const CHUNK_BYTES: usize = 64 * 1024;
                        let range_start = request.lines().find_map(|line| {
                            line.strip_prefix("Range: bytes=")
                                .or_else(|| line.strip_prefix("range: bytes="))
                                .and_then(|range| range.split_once('-').map(|(start, _)| start))
                                .and_then(|start| start.parse::<usize>().ok())
                                .filter(|start| *start < TOTAL_BYTES)
                        });
                        let offset = range_start.unwrap_or(0);
                        let remaining = TOTAL_BYTES - offset;
                        let (status, content_range) = if range_start.is_some() {
                            (
                                "206 Partial Content",
                                format!(
                                    "Content-Range: bytes {offset}-{}/{TOTAL_BYTES}\r\n",
                                    TOTAL_BYTES - 1
                                ),
                            )
                        } else {
                            ("200 OK", String::new())
                        };
                        let response = format!(
                            "HTTP/1.1 {status}\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=\"slow.bin\"\r\nAccept-Ranges: bytes\r\nETag: \"codexrs-slow-fixture\"\r\n{content_range}Content-Length: {remaining}\r\nConnection: close\r\n\r\n"
                        );
                        if connection.write_all(response.as_bytes()).is_err() {
                            continue;
                        }
                        let chunk = [0x5a_u8; CHUNK_BYTES];
                        let mut written = 0_usize;
                        while written < remaining {
                            if server_stop.load(Ordering::Relaxed) {
                                return Ok(());
                            }
                            let count = (remaining - written).min(CHUNK_BYTES);
                            if connection.write_all(&chunk[..count]).is_err() {
                                continue 'accept;
                            }
                            written += count;
                            thread::sleep(Duration::from_millis(12));
                        }
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        if std::time::Instant::now() >= deadline {
                            return Err(io::Error::new(
                                io::ErrorKind::TimedOut,
                                "Chromium did not request the slow download",
                            ));
                        }
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => return Err(error),
                }
            }
        });

        let config = BrowserConfig::new(profile_dir.clone(), "download-control".to_owned())
            .with_executable(Some(executable));
        let mut child = browser_command(
            config
                .executable
                .as_ref()
                .ok_or("Browser executable disappeared")?,
            &config,
        )
        .spawn()?;
        let job = create_browser_job(&mut child).map_err(io::Error::other)?;
        let (_commands, command_receiver) = bounded(BROWSER_COMMAND_CAPACITY);
        let mut pending_commands = VecDeque::new();
        let shutdown_requested = Arc::new(AtomicBool::new(false));
        let endpoint = wait_for_devtools_endpoint(
            &mut child,
            &profile_dir,
            None,
            &command_receiver,
            shutdown_requested.as_ref(),
            &mut pending_commands,
        )
        .map_err(io::Error::other)?
        .ok_or("Chromium stopped before publishing its DevTools endpoint")?;
        let mut cdp =
            CdpClient::connect(&endpoint, &shutdown_requested).map_err(io::Error::other)?;

        let probe = (|| -> Result<(), Box<dyn std::error::Error>> {
            cdp.request("Target.setDiscoverTargets", json!({"discover": true}), None)
                .map_err(io::Error::other)?;
            cdp.request(
                "Browser.setDownloadBehavior",
                json!({
                    "behavior": "allow",
                    "downloadPath": download_dir,
                    "eventsEnabled": true,
                }),
                None,
            )
            .map_err(io::Error::other)?;
            let download_url = format!("http://{address}/slow.bin");
            let download_target = cdp
                .request("Target.createTarget", json!({"url": "about:blank"}), None)
                .map_err(io::Error::other)?;
            let download_target_id = download_target
                .get("targetId")
                .and_then(serde_json::Value::as_str)
                .ok_or("Chromium did not create the download target")?;
            let download_attached = cdp
                .request(
                    "Target.attachToTarget",
                    json!({"targetId": download_target_id, "flatten": true}),
                    None,
                )
                .map_err(io::Error::other)?;
            let download_session_id = download_attached
                .get("sessionId")
                .and_then(serde_json::Value::as_str)
                .ok_or("Chromium did not attach the download target")?;
            cdp.request("Page.enable", json!({}), Some(download_session_id))
                .map_err(io::Error::other)?;
            cdp.request("Runtime.enable", json!({}), Some(download_session_id))
                .map_err(io::Error::other)?;
            let page_navigation = cdp
                .request(
                    "Page.navigate",
                    json!({"url": format!("http://{address}/page")}),
                    Some(download_session_id),
                )
                .map_err(io::Error::other)?;
            let page_deadline = std::time::Instant::now() + Duration::from_secs(5);
            loop {
                if runtime_value(
                    &mut cdp,
                    download_session_id,
                    "document.querySelector('#download') != null",
                )? == json!(true)
                {
                    break;
                }
                if std::time::Instant::now() >= page_deadline {
                    let snapshot = runtime_value(
                        &mut cdp,
                        download_session_id,
                        "({href: location.href, body: document.body?.innerHTML ?? '', ready: document.readyState})",
                    )?;
                    return Err(format!(
                        "Chromium did not load the download fixture page: navigate={page_navigation}, page={snapshot}"
                    )
                    .into());
                }
                thread::sleep(Duration::from_millis(20));
            }
            cdp.request(
                "Runtime.evaluate",
                json!({
                    "expression": "document.querySelector('#download').click()",
                    "returnByValue": true,
                    "userGesture": true,
                }),
                Some(download_session_id),
            )
            .map_err(io::Error::other)?;

            let download_deadline = std::time::Instant::now() + Duration::from_secs(10);
            let mut observed_methods = Vec::new();
            let guid = loop {
                if std::time::Instant::now() >= download_deadline {
                    return Err(format!(
                        "Chromium did not start the slow download; observed {observed_methods:?}"
                    )
                    .into());
                }
                let Some(event) = cdp.poll_event().map_err(io::Error::other)? else {
                    continue;
                };
                if observed_methods.len() < 32
                    && let Some(method) = event.get("method").and_then(serde_json::Value::as_str)
                {
                    observed_methods.push(method.to_owned());
                }
                if event.get("method").and_then(serde_json::Value::as_str)
                    == Some("Browser.downloadWillBegin")
                    && let Some(guid) = event
                        .pointer("/params/guid")
                        .and_then(serde_json::Value::as_str)
                {
                    break guid.to_owned();
                }
            };

            let staging_path = download_dir.join("slow.bin");
            let download = BrowserDownloadRuntime {
                agent_session: false,
                can_resume: false,
                completed_source: None,
                context_id: "download-control".to_owned(),
                default_destination: staging_path.clone(),
                destination: Some(staging_path.clone()),
                overwrite_destination: false,
                paused: false,
                received_bytes: 0,
                save_prompt_pending: false,
                staging_directory: download_dir.clone(),
                staging_path,
                started_at_ms: 0,
                total_bytes: 32 * 1024 * 1024,
                url: download_url,
                user_initiated: true,
            };
            let current_size = || -> u64 {
                fs::read_dir(&download_dir)
                    .ok()
                    .into_iter()
                    .flatten()
                    .take(16)
                    .filter_map(Result::ok)
                    .filter_map(|entry| entry.metadata().ok())
                    .map(|metadata| metadata.len())
                    .max()
                    .unwrap_or(0)
            };
            let data_deadline = std::time::Instant::now() + Duration::from_secs(5);
            while current_size() == 0 {
                if std::time::Instant::now() >= data_deadline {
                    let entries = fs::read_dir(&download_dir)
                        .ok()
                        .into_iter()
                        .flatten()
                        .take(16)
                        .filter_map(Result::ok)
                        .map(|entry| {
                            (
                                entry.file_name().to_string_lossy().into_owned(),
                                entry.metadata().map(|metadata| metadata.len()).ok(),
                            )
                        })
                        .collect::<Vec<_>>();
                    return Err(format!(
                        "Chromium did not write active download {guid}; files={entries:?}"
                    )
                    .into());
                }
                thread::sleep(Duration::from_millis(20));
            }

            assert!(control_download_transfer(
                &mut cdp,
                browser_family,
                &download,
                BrowserDownloadControl::Pause,
            )?);
            let settled_size = if browser_family == BrowserFamily::Edge {
                thread::sleep(Duration::from_millis(800));
                let paused_size = current_size();
                thread::sleep(Duration::from_millis(400));
                let settled_size = current_size();
                if paused_size != settled_size {
                    return Err(format!(
                        "Downloads controller did not pause {guid}: {paused_size} -> {settled_size}"
                    )
                    .into());
                }
                Some(settled_size)
            } else {
                None
            };

            assert!(!control_download_transfer(
                &mut cdp,
                browser_family,
                &download,
                BrowserDownloadControl::Resume,
            )?);
            if let Some(settled_size) = settled_size {
                let resumed_deadline = std::time::Instant::now() + Duration::from_secs(5);
                while current_size() <= settled_size {
                    if std::time::Instant::now() >= resumed_deadline {
                        return Err(format!("Downloads controller did not resume {guid}").into());
                    }
                    thread::sleep(Duration::from_millis(20));
                }
            }
            Ok(())
        })();

        stop_server.store(true, Ordering::Relaxed);
        graceful_browser_exit(&mut child, Some(&mut cdp));
        super::release_browser_job(job);
        match server.join() {
            Ok(result) if probe.is_ok() => result?,
            Ok(_) => {}
            Err(_) if probe.is_ok() => return Err("download probe server panicked".into()),
            Err(_) => {}
        }
        let _ = fs::remove_dir_all(&profile_dir);
        probe
    }

    #[test]
    fn agent_viewport_commands_require_exact_bounded_dimensions() {
        let command = json!({
            "browser_id": "browser-1",
            "width": 320,
            "height": 240,
        });
        assert_eq!(checked_agent_browser_id(&command).ok(), Some("browser-1"));
        assert_eq!(checked_agent_viewport(&command).ok(), Some((320, 240)));
        assert!(
            checked_agent_viewport(&json!({
                "browser_id": "browser-1",
                "width": 319,
                "height": 240,
            }))
            .is_err()
        );
        assert!(
            checked_agent_viewport(&json!({
                "browser_id": "browser-1",
                "width": 320,
                "height": 1081,
            }))
            .is_err()
        );
        assert!(
            checked_agent_viewport(&json!({
                "browser_id": "browser-1",
                "width": 800.5,
                "height": 600,
            }))
            .is_err()
        );
        assert!(checked_agent_browser_id(&json!({"browser_id": ""})).is_err());
    }
}
