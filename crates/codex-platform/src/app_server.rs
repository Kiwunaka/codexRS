use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io::{self, BufReader, Read, Write};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use codex_protocol::{
    AppsListParams, AppsListResponse, AppsReadParams, AppsReadResponse, CancelLoginAccountParams,
    CancelLoginAccountResponse, ClientInfo, ClientNotification, ClientRequest,
    ConfigBatchWriteParams, ConfigReadParams, ConfigReadResponse, ConfigRequirementsReadResponse,
    ConfigWriteResponse, DEFAULT_COMMAND_CHANNEL_CAPACITY, DEFAULT_EVENT_CHANNEL_CAPACITY,
    DEFAULT_MAX_FRAME_BYTES, DEFAULT_MESSAGE_CHANNEL_CAPACITY, FeedbackUploadParams,
    FeedbackUploadResponse, GetAccountParams, GetAccountRateLimitsResponse, GetAccountResponse,
    GetAuthStatusParams, GetAuthStatusResponse, GitDiffToRemoteParams, GitDiffToRemoteResponse,
    HooksListParams, HooksListResponse, IncomingMessage, InitializeCapabilities, InitializeParams,
    InitializeResponse, ListMcpServerStatusParams, ListMcpServerStatusResponse, LoginAccountParams,
    LoginAccountResponse, LogoutAccountResponse, MAX_INTERLEAVED_MESSAGES_PER_REQUEST,
    MAX_PENDING_REQUESTS, MarketplaceAddParams, MarketplaceAddResponse, MarketplaceRemoveParams,
    MarketplaceRemoveResponse, MarketplaceUpgradeParams, MarketplaceUpgradeResponse,
    McpResourceReadParams, McpResourceReadResponse, McpServerOauthLoginParams,
    McpServerOauthLoginResponse, ModelListParams, ModelListResponse, PermissionProfileListParams,
    PermissionProfileListResponse, PluginInstallParams, PluginInstallResponse, PluginListParams,
    PluginListResponse, PluginReadParams, PluginReadResponse, PluginUninstallParams, ProtocolError,
    SkillsConfigWriteParams, SkillsConfigWriteResponse, SkillsListParams, SkillsListResponse,
    ThreadArchiveParams, ThreadBackgroundTerminalsCleanParams,
    ThreadBackgroundTerminalsCleanResponse, ThreadBackgroundTerminalsListParams,
    ThreadBackgroundTerminalsListResponse, ThreadBackgroundTerminalsTerminateParams,
    ThreadBackgroundTerminalsTerminateResponse, ThreadCompactStartParams,
    ThreadCompactStartResponse, ThreadDeleteParams, ThreadForkParams, ThreadForkResponse,
    ThreadGoalClearParams, ThreadGoalClearResponse, ThreadGoalGetParams, ThreadGoalGetResponse,
    ThreadGoalSetParams, ThreadGoalSetResponse, ThreadItemsListParams, ThreadItemsListResponse,
    ThreadListParams, ThreadListResponse, ThreadLoadedListParams, ThreadLoadedListResponse,
    ThreadReadParams, ThreadReadResponse, ThreadResumeParams, ThreadResumeResponse,
    ThreadRollbackParams, ThreadRollbackResponse, ThreadSearchParams, ThreadSearchResponse,
    ThreadSetNameParams, ThreadSettingsUpdateParams, ThreadSettingsUpdateResponse,
    ThreadShellCommandParams, ThreadShellCommandResponse, ThreadStartParams, ThreadStartResponse,
    ThreadTurnsListParams, ThreadTurnsListResponse, ThreadUnarchiveParams, ThreadUnarchiveResponse,
    TurnInterruptParams, TurnStartParams, TurnStartResponse, TurnSteerParams, decode_incoming,
    decode_result, encode_error_response, encode_json_line, encode_success_response,
    encode_unsupported_request, read_bounded_frame,
};
use crossbeam_channel::{Receiver as CrossbeamReceiver, Sender as CrossbeamSender, TrySendError};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

#[cfg(windows)]
use std::os::windows::io::AsRawHandle;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
use win32job::{ExtendedLimitInfo, Job, JobError};

pub const DEFAULT_THREAD_PAGE_LIMIT: u32 = 20;
pub const MAX_THREAD_PAGE_LIMIT: u32 = 100;
pub const MAX_APP_READ_ITEMS: usize = 100;

const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(3);
const ROUTER_TICK: Duration = Duration::from_millis(25);
const EVENT_BACKPRESSURE_TIMEOUT: Duration = Duration::from_millis(100);
const SHUTDOWN_POLL_INTERVAL: Duration = Duration::from_millis(20);

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexHomeKind {
    Default,
    Configured,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexHome {
    path: PathBuf,
    kind: CodexHomeKind,
}

impl CodexHome {
    pub fn resolve(explicit: Option<PathBuf>) -> Result<Self, AppServerError> {
        let configured = explicit.or_else(|| nonempty_environment_value("CODEX_HOME"));
        let selected = match configured {
            Some(path) => path,
            None => default_codex_home_path()?,
        };

        let metadata = fs::metadata(&selected).map_err(|_| AppServerError::CodexHomeUnavailable)?;
        if !metadata.is_dir() {
            return Err(AppServerError::CodexHomeNotDirectory);
        }
        let path = fs::canonicalize(&selected).map_err(|_| AppServerError::CodexHomeUnavailable)?;

        let kind = default_codex_home_path()
            .ok()
            .and_then(|default| fs::canonicalize(default).ok())
            .map_or(CodexHomeKind::Configured, |default| {
                if paths_match(&path, &default) {
                    CodexHomeKind::Default
                } else {
                    CodexHomeKind::Configured
                }
            });

        Ok(Self { path, kind })
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub const fn kind(&self) -> CodexHomeKind {
        self.kind
    }
}

fn nonempty_environment_value(name: &str) -> Option<PathBuf> {
    env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn default_codex_home_path() -> Result<PathBuf, AppServerError> {
    #[cfg(windows)]
    let home = env::var_os("USERPROFILE");
    #[cfg(not(windows))]
    let home = env::var_os("HOME");

    home.filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .map(|path| path.join(".codex"))
        .ok_or(AppServerError::HomeEnvironmentUnavailable)
}

#[cfg(windows)]
fn paths_match(left: &Path, right: &Path) -> bool {
    left.as_os_str()
        .to_string_lossy()
        .eq_ignore_ascii_case(&right.as_os_str().to_string_lossy())
}

#[cfg(not(windows))]
fn paths_match(left: &Path, right: &Path) -> bool {
    left == right
}

#[derive(Debug, Clone)]
pub struct AppServerConfig {
    pub codex_binary: PathBuf,
    pub codex_home: CodexHome,
    pub request_timeout: Duration,
    pub shutdown_timeout: Duration,
    pub max_frame_bytes: NonZeroUsize,
}

impl AppServerConfig {
    #[must_use]
    pub fn new(codex_binary: PathBuf, codex_home: CodexHome) -> Self {
        Self {
            codex_binary,
            codex_home,
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
            shutdown_timeout: DEFAULT_SHUTDOWN_TIMEOUT,
            max_frame_bytes: NonZeroUsize::new(DEFAULT_MAX_FRAME_BYTES)
                .unwrap_or(NonZeroUsize::MIN),
        }
    }
}

#[derive(Debug)]
pub enum AppServerError {
    HomeEnvironmentUnavailable,
    CodexHomeUnavailable,
    CodexHomeNotDirectory,
    CodexHomeMismatch,
    Spawn(io::Error),
    MissingPipe(&'static str),
    BackgroundThread(io::Error),
    BackgroundThreadPanicked,
    Transport(io::Error),
    Protocol(ProtocolError),
    TransportClosed,
    CommandQueueFull,
    EventQueueOverloaded,
    TooManyPendingRequests,
    RequestTimedOut(&'static str),
    RequestFailed {
        code: i64,
    },
    UnexpectedResponseId,
    TooManyInterleavedMessages,
    RequestIdExhausted,
    AlreadyInitialized,
    NotInitialized,
    InvalidPageLimit {
        requested: u32,
        maximum: u32,
    },
    #[cfg(windows)]
    Job(JobError),
}

impl fmt::Display for AppServerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HomeEnvironmentUnavailable => {
                formatter.write_str("the user home directory is unavailable")
            }
            Self::CodexHomeUnavailable => {
                formatter.write_str("CODEX_HOME does not exist or cannot be opened")
            }
            Self::CodexHomeNotDirectory => formatter.write_str("CODEX_HOME is not a directory"),
            Self::CodexHomeMismatch => {
                formatter.write_str("app-server reported a different CODEX_HOME")
            }
            Self::Spawn(_) => formatter.write_str("could not start codex app-server"),
            Self::MissingPipe(name) => write!(formatter, "app-server {name} pipe is unavailable"),
            Self::BackgroundThread(_) => {
                formatter.write_str("could not start an app-server I/O thread")
            }
            Self::BackgroundThreadPanicked => {
                formatter.write_str("an app-server I/O thread stopped unexpectedly")
            }
            Self::Transport(_) => formatter.write_str("app-server transport failed"),
            Self::Protocol(error) => error.fmt(formatter),
            Self::TransportClosed => formatter.write_str("app-server closed its transport"),
            Self::CommandQueueFull => {
                formatter.write_str("app-server command queue is temporarily full")
            }
            Self::EventQueueOverloaded => {
                formatter.write_str("app-server event queue exceeded its bounded capacity")
            }
            Self::TooManyPendingRequests => {
                formatter.write_str("too many app-server requests are already pending")
            }
            Self::RequestTimedOut(method) => {
                write!(formatter, "app-server request `{method}` timed out")
            }
            Self::RequestFailed { code } => {
                write!(
                    formatter,
                    "app-server rejected the request with code {code}"
                )
            }
            Self::UnexpectedResponseId => {
                formatter.write_str("app-server returned an unexpected response id")
            }
            Self::TooManyInterleavedMessages => {
                formatter.write_str("app-server exceeded the per-request message budget")
            }
            Self::RequestIdExhausted => formatter.write_str("app-server request ids are exhausted"),
            Self::AlreadyInitialized => {
                formatter.write_str("app-server connection is already initialized")
            }
            Self::NotInitialized => formatter.write_str("app-server connection is not initialized"),
            Self::InvalidPageLimit { requested, maximum } => {
                write!(
                    formatter,
                    "thread page size {requested} is outside the allowed range 1..={maximum}"
                )
            }
            #[cfg(windows)]
            Self::Job(_) => formatter.write_str("Windows Job Object setup failed"),
        }
    }
}

impl Error for AppServerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Spawn(error) | Self::BackgroundThread(error) | Self::Transport(error) => {
                Some(error)
            }
            Self::Protocol(error) => Some(error),
            #[cfg(windows)]
            Self::Job(error) => Some(error),
            Self::HomeEnvironmentUnavailable
            | Self::CodexHomeUnavailable
            | Self::CodexHomeNotDirectory
            | Self::CodexHomeMismatch
            | Self::MissingPipe(_)
            | Self::BackgroundThreadPanicked
            | Self::TransportClosed
            | Self::CommandQueueFull
            | Self::EventQueueOverloaded
            | Self::TooManyPendingRequests
            | Self::RequestTimedOut(_)
            | Self::RequestFailed { .. }
            | Self::UnexpectedResponseId
            | Self::TooManyInterleavedMessages
            | Self::RequestIdExhausted
            | Self::AlreadyInitialized
            | Self::NotInitialized
            | Self::InvalidPageLimit { .. } => None,
        }
    }
}

impl From<ProtocolError> for AppServerError {
    fn from(error: ProtocolError) -> Self {
        Self::Protocol(error)
    }
}

#[cfg(windows)]
impl From<JobError> for AppServerError {
    fn from(error: JobError) -> Self {
        Self::Job(error)
    }
}

pub struct AppServerClient {
    config: AppServerConfig,
    child: ManagedChild,
    stdin: Option<ChildStdin>,
    messages: Option<Receiver<ReaderEvent>>,
    stdout_thread: Option<JoinHandle<()>>,
    stderr_thread: Option<JoinHandle<()>>,
    next_request_id: u64,
    initialized: bool,
    closed: bool,
}

impl fmt::Debug for AppServerClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AppServerClient")
            .field("codex_home_kind", &self.config.codex_home.kind())
            .field("next_request_id", &self.next_request_id)
            .field("initialized", &self.initialized)
            .field("closed", &self.closed)
            .finish_non_exhaustive()
    }
}

impl AppServerClient {
    pub fn spawn(config: AppServerConfig) -> Result<Self, AppServerError> {
        let mut command = Command::new(&config.codex_binary);
        command
            .arg("app-server")
            .env("CODEX_HOME", config.codex_home.path())
            .env("CODEX_SQLITE_HOME", config.codex_home.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        command.creation_flags(CREATE_NO_WINDOW);

        let mut child = ManagedChild::spawn(&mut command)?;
        let stdin = child
            .take_stdin()
            .ok_or(AppServerError::MissingPipe("stdin"))?;
        let stdout = child
            .take_stdout()
            .ok_or(AppServerError::MissingPipe("stdout"))?;
        let stderr = child
            .take_stderr()
            .ok_or(AppServerError::MissingPipe("stderr"))?;

        let (sender, receiver) = mpsc::sync_channel(DEFAULT_MESSAGE_CHANNEL_CAPACITY);
        let max_frame_bytes = config.max_frame_bytes;
        let stdout_thread = thread::Builder::new()
            .name("codex-app-server-stdout".to_owned())
            .spawn(move || stdout_reader(stdout, sender, max_frame_bytes))
            .map_err(AppServerError::BackgroundThread)?;
        let stderr_thread = thread::Builder::new()
            .name("codex-app-server-stderr".to_owned())
            .spawn(move || drain_stderr(stderr))
            .map_err(AppServerError::BackgroundThread)?;

        Ok(Self {
            config,
            child,
            stdin: Some(stdin),
            messages: Some(receiver),
            stdout_thread: Some(stdout_thread),
            stderr_thread: Some(stderr_thread),
            next_request_id: 0,
            initialized: false,
            closed: false,
        })
    }

    pub fn initialize(
        &mut self,
        client_info: ClientInfo,
    ) -> Result<InitializeResponse, AppServerError> {
        self.initialize_with_capabilities(client_info, None)
    }

    pub fn initialize_with_capabilities(
        &mut self,
        client_info: ClientInfo,
        capabilities: Option<InitializeCapabilities>,
    ) -> Result<InitializeResponse, AppServerError> {
        if self.initialized {
            return Err(AppServerError::AlreadyInitialized);
        }

        let response = self.request(
            "initialize",
            InitializeParams {
                client_info,
                capabilities,
            },
        )?;
        self.verify_reported_home(&response)?;
        self.write_message(&ClientNotification {
            method: "initialized",
        })?;
        self.initialized = true;
        Ok(response)
    }

    pub fn list_threads_state_db_only(
        &mut self,
        limit: u32,
    ) -> Result<ThreadListResponse, AppServerError> {
        if !self.initialized {
            return Err(AppServerError::NotInitialized);
        }
        if !(1..=MAX_THREAD_PAGE_LIMIT).contains(&limit) {
            return Err(AppServerError::InvalidPageLimit {
                requested: limit,
                maximum: MAX_THREAD_PAGE_LIMIT,
            });
        }

        self.request("thread/list", ThreadListParams::state_db_page(limit))
    }

    pub fn shutdown(&mut self) -> Result<(), AppServerError> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;
        drop(self.stdin.take());

        let shutdown_result = self.child.shutdown(self.config.shutdown_timeout);
        drop(self.messages.take());
        let stdout_result = join_background_thread(self.stdout_thread.take());
        let stderr_result = join_background_thread(self.stderr_thread.take());

        shutdown_result?;
        stdout_result?;
        stderr_result
    }

    fn request<P, R>(&mut self, method: &'static str, params: P) -> Result<R, AppServerError>
    where
        P: Serialize,
        R: DeserializeOwned,
    {
        if self.closed {
            return Err(AppServerError::TransportClosed);
        }
        let id = self.next_request_id;
        self.next_request_id = self
            .next_request_id
            .checked_add(1)
            .ok_or(AppServerError::RequestIdExhausted)?;
        self.write_message(&ClientRequest {
            method,
            id,
            params: Some(params),
        })?;

        let deadline = Instant::now() + self.config.request_timeout;
        let mut interleaved = 0_usize;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(AppServerError::RequestTimedOut(method));
            }
            let event = self
                .messages
                .as_ref()
                .ok_or(AppServerError::TransportClosed)?
                .recv_timeout(remaining);
            let event = match event {
                Ok(event) => event,
                Err(RecvTimeoutError::Timeout) => {
                    return Err(AppServerError::RequestTimedOut(method));
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(AppServerError::TransportClosed);
                }
            };

            match event {
                ReaderEvent::Message(Ok(IncomingMessage::Success {
                    id: response_id,
                    result,
                })) => {
                    if response_id.as_u64() != Some(id) {
                        return Err(AppServerError::UnexpectedResponseId);
                    }
                    return decode_result(result).map_err(AppServerError::from);
                }
                ReaderEvent::Message(Ok(IncomingMessage::Failure {
                    id: response_id,
                    code,
                })) => {
                    if response_id.as_u64() != Some(id) {
                        return Err(AppServerError::UnexpectedResponseId);
                    }
                    return Err(AppServerError::RequestFailed { code });
                }
                ReaderEvent::Message(Ok(IncomingMessage::Request { id: request_id, .. })) => {
                    interleaved = add_interleaved(interleaved)?;
                    let frame =
                        encode_unsupported_request(&request_id, self.config.max_frame_bytes)?;
                    self.write_frame(&frame)?;
                }
                ReaderEvent::Message(Ok(IncomingMessage::Notification { .. })) => {
                    interleaved = add_interleaved(interleaved)?;
                }
                ReaderEvent::Message(Err(error)) => return Err(error.into()),
                ReaderEvent::Eof => return Err(AppServerError::TransportClosed),
            }
        }
    }

    fn write_message<T: Serialize>(&mut self, message: &T) -> Result<(), AppServerError> {
        let frame = encode_json_line(message, self.config.max_frame_bytes)?;
        self.write_frame(&frame)
    }

    fn write_frame(&mut self, frame: &[u8]) -> Result<(), AppServerError> {
        let stdin = self.stdin.as_mut().ok_or(AppServerError::TransportClosed)?;
        stdin.write_all(frame).map_err(AppServerError::Transport)?;
        stdin.flush().map_err(AppServerError::Transport)
    }

    fn verify_reported_home(&self, response: &InitializeResponse) -> Result<(), AppServerError> {
        let reported = fs::canonicalize(&response.codex_home)
            .map_err(|_| AppServerError::CodexHomeMismatch)?;
        if paths_match(self.config.codex_home.path(), &reported) {
            Ok(())
        } else {
            Err(AppServerError::CodexHomeMismatch)
        }
    }
}

impl Drop for AppServerClient {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

/// An event emitted independently of request responses.
///
/// The receiver is deliberately bounded. Notifications may be coalesced under
/// sustained backpressure, but server-initiated requests are either delivered
/// promptly or rejected back to app-server with an overload error.
#[derive(Debug)]
pub enum AppServerEvent {
    Notification {
        method: String,
        params: Value,
    },
    Request {
        id: Value,
        method: String,
        params: Value,
    },
    NotificationsDropped {
        count: usize,
    },
    Disconnected,
}

enum RouterCommand {
    Request {
        id: u64,
        method: &'static str,
        frame: Vec<u8>,
        deadline: Instant,
        reply: CrossbeamSender<Result<Value, AppServerError>>,
    },
    Frame(Vec<u8>),
    Shutdown(CrossbeamSender<Result<(), AppServerError>>),
}

struct PendingRequest {
    method: &'static str,
    deadline: Instant,
    reply: CrossbeamSender<Result<Value, AppServerError>>,
}

enum RoutedReaderEvent {
    Message(Result<IncomingMessage, ProtocolError>),
    Eof,
}

/// Long-lived, multiplexed app-server connection suitable for a desktop UI.
///
/// Requests can be issued concurrently from background tasks. A single router
/// owns the process stdin and pending-request table, so responses may arrive
/// out of order without blocking notification and approval delivery.
pub struct AppServerConnection {
    config: AppServerConfig,
    commands: CrossbeamSender<RouterCommand>,
    events: CrossbeamReceiver<AppServerEvent>,
    next_request_id: AtomicU64,
    initialized: AtomicBool,
    closed: AtomicBool,
    router_thread: Option<JoinHandle<()>>,
}

impl fmt::Debug for AppServerConnection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AppServerConnection")
            .field("codex_home_kind", &self.config.codex_home.kind())
            .field(
                "next_request_id",
                &self.next_request_id.load(Ordering::Relaxed),
            )
            .field("initialized", &self.initialized.load(Ordering::Acquire))
            .field("closed", &self.closed.load(Ordering::Acquire))
            .finish_non_exhaustive()
    }
}

impl AppServerConnection {
    pub fn spawn(config: AppServerConfig) -> Result<Self, AppServerError> {
        let mut command = Command::new(&config.codex_binary);
        command
            .arg("app-server")
            .env("CODEX_HOME", config.codex_home.path())
            .env("CODEX_SQLITE_HOME", config.codex_home.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        command.creation_flags(CREATE_NO_WINDOW);

        let mut child = ManagedChild::spawn(&mut command)?;
        let stdin = child
            .take_stdin()
            .ok_or(AppServerError::MissingPipe("stdin"))?;
        let stdout = child
            .take_stdout()
            .ok_or(AppServerError::MissingPipe("stdout"))?;
        let stderr = child
            .take_stderr()
            .ok_or(AppServerError::MissingPipe("stderr"))?;

        let (reader_sender, reader_receiver) =
            crossbeam_channel::bounded(DEFAULT_MESSAGE_CHANNEL_CAPACITY);
        let max_frame_bytes = config.max_frame_bytes;
        let stdout_thread = thread::Builder::new()
            .name("codex-app-server-router-stdout".to_owned())
            .spawn(move || routed_stdout_reader(stdout, reader_sender, max_frame_bytes))
            .map_err(AppServerError::BackgroundThread)?;
        let stderr_thread = thread::Builder::new()
            .name("codex-app-server-router-stderr".to_owned())
            .spawn(move || drain_stderr(stderr))
            .map_err(AppServerError::BackgroundThread)?;

        let (command_sender, command_receiver) =
            crossbeam_channel::bounded(DEFAULT_COMMAND_CHANNEL_CAPACITY);
        let (event_sender, event_receiver) =
            crossbeam_channel::bounded(DEFAULT_EVENT_CHANNEL_CAPACITY);
        let shutdown_timeout = config.shutdown_timeout;
        let router_thread = thread::Builder::new()
            .name("codex-app-server-router".to_owned())
            .spawn(move || {
                route_app_server(
                    child,
                    stdin,
                    command_receiver,
                    reader_receiver,
                    event_sender,
                    stdout_thread,
                    stderr_thread,
                    max_frame_bytes,
                    shutdown_timeout,
                );
            })
            .map_err(AppServerError::BackgroundThread)?;

        Ok(Self {
            config,
            commands: command_sender,
            events: event_receiver,
            next_request_id: AtomicU64::new(0),
            initialized: AtomicBool::new(false),
            closed: AtomicBool::new(false),
            router_thread: Some(router_thread),
        })
    }

    pub fn initialize(
        &self,
        client_info: ClientInfo,
    ) -> Result<InitializeResponse, AppServerError> {
        self.initialize_with_capabilities(client_info, None)
    }

    pub fn initialize_with_capabilities(
        &self,
        client_info: ClientInfo,
        capabilities: Option<InitializeCapabilities>,
    ) -> Result<InitializeResponse, AppServerError> {
        if self.initialized.load(Ordering::Acquire) {
            return Err(AppServerError::AlreadyInitialized);
        }

        let response = self.request(
            "initialize",
            InitializeParams {
                client_info,
                capabilities,
            },
        )?;
        self.verify_reported_home(&response)?;
        self.notify(&ClientNotification {
            method: "initialized",
        })?;
        self.initialized.store(true, Ordering::Release);
        Ok(response)
    }

    pub fn request<P, R>(&self, method: &'static str, params: P) -> Result<R, AppServerError>
    where
        P: Serialize,
        R: DeserializeOwned,
    {
        self.request_optional(method, Some(params))
    }

    fn request_without_params<R>(&self, method: &'static str) -> Result<R, AppServerError>
    where
        R: DeserializeOwned,
    {
        self.request_optional::<(), R>(method, None)
    }

    fn request_optional<P, R>(
        &self,
        method: &'static str,
        params: Option<P>,
    ) -> Result<R, AppServerError>
    where
        P: Serialize,
        R: DeserializeOwned,
    {
        if self.closed.load(Ordering::Acquire) {
            return Err(AppServerError::TransportClosed);
        }

        let id = self.allocate_request_id()?;
        let frame = encode_json_line(
            &ClientRequest { method, id, params },
            self.config.max_frame_bytes,
        )?;
        let (reply_sender, reply_receiver) = crossbeam_channel::bounded(1);
        let deadline = Instant::now() + self.config.request_timeout;
        self.commands
            .send_timeout(
                RouterCommand::Request {
                    id,
                    method,
                    frame,
                    deadline,
                    reply: reply_sender,
                },
                self.config.request_timeout.min(EVENT_BACKPRESSURE_TIMEOUT),
            )
            .map_err(|error| match error {
                crossbeam_channel::SendTimeoutError::Timeout(_) => AppServerError::CommandQueueFull,
                crossbeam_channel::SendTimeoutError::Disconnected(_) => {
                    AppServerError::TransportClosed
                }
            })?;

        let result = reply_receiver
            .recv_deadline(deadline + ROUTER_TICK)
            .map_err(|error| match error {
                crossbeam_channel::RecvTimeoutError::Timeout => {
                    AppServerError::RequestTimedOut(method)
                }
                crossbeam_channel::RecvTimeoutError::Disconnected => {
                    AppServerError::TransportClosed
                }
            })??;
        decode_result(result).map_err(AppServerError::from)
    }

    pub fn notify<T: Serialize>(&self, notification: &T) -> Result<(), AppServerError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(AppServerError::TransportClosed);
        }
        let frame = encode_json_line(notification, self.config.max_frame_bytes)?;
        self.send_frame_command(frame)
    }

    pub fn respond_success<T: Serialize>(
        &self,
        id: &Value,
        result: &T,
    ) -> Result<(), AppServerError> {
        let frame = encode_success_response(id, result, self.config.max_frame_bytes)?;
        self.send_frame_command(frame)
    }

    pub fn respond_error(
        &self,
        id: &Value,
        code: i64,
        message: &'static str,
    ) -> Result<(), AppServerError> {
        let frame = encode_error_response(id, code, message, self.config.max_frame_bytes)?;
        self.send_frame_command(frame)
    }

    pub fn try_recv_event(&self) -> Result<Option<AppServerEvent>, AppServerError> {
        match self.events.try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(crossbeam_channel::TryRecvError::Empty) => Ok(None),
            Err(crossbeam_channel::TryRecvError::Disconnected) => {
                Err(AppServerError::TransportClosed)
            }
        }
    }

    pub fn recv_event_timeout(
        &self,
        timeout: Duration,
    ) -> Result<Option<AppServerEvent>, AppServerError> {
        match self.events.recv_timeout(timeout) {
            Ok(event) => Ok(Some(event)),
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => Ok(None),
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                Err(AppServerError::TransportClosed)
            }
        }
    }

    pub fn list_threads_state_db_only(
        &self,
        limit: u32,
    ) -> Result<ThreadListResponse, AppServerError> {
        self.require_initialized()?;
        validate_page_limit(limit)?;
        self.request("thread/list", ThreadListParams::state_db_page(limit))
    }

    pub fn list_threads(
        &self,
        params: ThreadListParams,
    ) -> Result<ThreadListResponse, AppServerError> {
        self.require_initialized()?;
        validate_page_limit(params.limit)?;
        if !params.use_state_db_only {
            return Err(AppServerError::Protocol(ProtocolError::InvalidEnvelope(
                "thread/list must use state DB only",
            )));
        }
        self.request("thread/list", params)
    }

    pub fn list_loaded_threads(
        &self,
        params: ThreadLoadedListParams,
    ) -> Result<ThreadLoadedListResponse, AppServerError> {
        self.require_initialized()?;
        validate_page_limit(params.limit)?;
        self.request("thread/loaded/list", params)
    }

    pub fn search_threads(
        &self,
        params: ThreadSearchParams,
    ) -> Result<ThreadSearchResponse, AppServerError> {
        self.require_initialized()?;
        if let Some(limit) = params.limit {
            validate_page_limit(limit)?;
        }
        self.request("thread/search", params)
    }

    pub fn read_thread(
        &self,
        params: ThreadReadParams,
    ) -> Result<ThreadReadResponse, AppServerError> {
        self.require_initialized()?;
        self.request("thread/read", params)
    }

    pub fn list_thread_turns(
        &self,
        params: ThreadTurnsListParams,
    ) -> Result<ThreadTurnsListResponse, AppServerError> {
        self.require_initialized()?;
        validate_page_limit(params.limit)?;
        self.request("thread/turns/list", params)
    }

    pub fn list_thread_items(
        &self,
        params: ThreadItemsListParams,
    ) -> Result<ThreadItemsListResponse, AppServerError> {
        self.require_initialized()?;
        validate_page_limit(params.limit)?;
        self.request("thread/items/list", params)
    }

    pub fn list_background_terminals(
        &self,
        params: ThreadBackgroundTerminalsListParams,
    ) -> Result<ThreadBackgroundTerminalsListResponse, AppServerError> {
        self.require_initialized()?;
        if let Some(limit) = params.limit {
            validate_page_limit(limit)?;
        }
        self.request("thread/backgroundTerminals/list", params)
    }

    pub fn terminate_background_terminal(
        &self,
        params: ThreadBackgroundTerminalsTerminateParams,
    ) -> Result<ThreadBackgroundTerminalsTerminateResponse, AppServerError> {
        self.require_initialized()?;
        self.request("thread/backgroundTerminals/terminate", params)
    }

    pub fn clean_background_terminals(
        &self,
        params: ThreadBackgroundTerminalsCleanParams,
    ) -> Result<ThreadBackgroundTerminalsCleanResponse, AppServerError> {
        self.require_initialized()?;
        self.request("thread/backgroundTerminals/clean", params)
    }

    pub fn start_thread(
        &self,
        params: ThreadStartParams,
    ) -> Result<ThreadStartResponse, AppServerError> {
        self.require_initialized()?;
        self.request("thread/start", params)
    }

    pub fn fork_thread(
        &self,
        params: ThreadForkParams,
    ) -> Result<ThreadForkResponse, AppServerError> {
        self.require_initialized()?;
        self.request("thread/fork", params)
    }

    pub fn rollback_thread(
        &self,
        params: ThreadRollbackParams,
    ) -> Result<ThreadRollbackResponse, AppServerError> {
        self.require_initialized()?;
        self.request("thread/rollback", params)
    }

    pub fn compact_thread(
        &self,
        params: ThreadCompactStartParams,
    ) -> Result<ThreadCompactStartResponse, AppServerError> {
        self.require_initialized()?;
        self.request("thread/compact/start", params)
    }

    pub fn run_thread_shell_command(
        &self,
        params: ThreadShellCommandParams,
    ) -> Result<ThreadShellCommandResponse, AppServerError> {
        self.require_initialized()?;
        self.request("thread/shellCommand", params)
    }

    pub fn archive_thread(&self, params: ThreadArchiveParams) -> Result<Value, AppServerError> {
        self.require_initialized()?;
        self.request("thread/archive", params)
    }

    pub fn unarchive_thread(
        &self,
        params: ThreadUnarchiveParams,
    ) -> Result<ThreadUnarchiveResponse, AppServerError> {
        self.require_initialized()?;
        self.request("thread/unarchive", params)
    }

    pub fn delete_thread(&self, params: ThreadDeleteParams) -> Result<Value, AppServerError> {
        self.require_initialized()?;
        self.request("thread/delete", params)
    }

    pub fn set_thread_name(&self, params: ThreadSetNameParams) -> Result<Value, AppServerError> {
        self.require_initialized()?;
        self.request("thread/name/set", params)
    }

    pub fn set_thread_goal(
        &self,
        params: ThreadGoalSetParams,
    ) -> Result<ThreadGoalSetResponse, AppServerError> {
        self.require_initialized()?;
        self.request("thread/goal/set", params)
    }

    pub fn get_thread_goal(
        &self,
        params: ThreadGoalGetParams,
    ) -> Result<ThreadGoalGetResponse, AppServerError> {
        self.require_initialized()?;
        self.request("thread/goal/get", params)
    }

    pub fn clear_thread_goal(
        &self,
        params: ThreadGoalClearParams,
    ) -> Result<ThreadGoalClearResponse, AppServerError> {
        self.require_initialized()?;
        self.request("thread/goal/clear", params)
    }

    pub fn resume_thread(
        &self,
        params: ThreadResumeParams,
    ) -> Result<ThreadResumeResponse, AppServerError> {
        self.require_initialized()?;
        if params
            .initial_turns_page
            .as_ref()
            .is_some_and(|page| !(1..=MAX_THREAD_PAGE_LIMIT).contains(&page.limit))
        {
            return Err(AppServerError::InvalidPageLimit {
                requested: params
                    .initial_turns_page
                    .as_ref()
                    .map_or(0, |page| page.limit),
                maximum: MAX_THREAD_PAGE_LIMIT,
            });
        }
        self.request("thread/resume", params)
    }

    pub fn update_thread_settings(
        &self,
        params: ThreadSettingsUpdateParams,
    ) -> Result<ThreadSettingsUpdateResponse, AppServerError> {
        self.require_initialized()?;
        self.request("thread/settings/update", params)
    }

    pub fn start_turn(&self, params: TurnStartParams) -> Result<TurnStartResponse, AppServerError> {
        self.require_initialized()?;
        self.request("turn/start", params)
    }

    pub fn steer_turn(&self, params: TurnSteerParams) -> Result<Value, AppServerError> {
        self.require_initialized()?;
        self.request("turn/steer", params)
    }

    pub fn interrupt_turn(&self, params: TurnInterruptParams) -> Result<Value, AppServerError> {
        self.require_initialized()?;
        self.request("turn/interrupt", params)
    }

    pub fn list_models(
        &self,
        params: ModelListParams,
    ) -> Result<ModelListResponse, AppServerError> {
        self.require_initialized()?;
        if let Some(limit) = params.limit {
            validate_page_limit(limit)?;
        }
        self.request("model/list", params)
    }

    pub fn list_permission_profiles(
        &self,
        params: PermissionProfileListParams,
    ) -> Result<PermissionProfileListResponse, AppServerError> {
        self.require_initialized()?;
        if let Some(limit) = params.limit {
            validate_page_limit(limit)?;
        }
        self.request("permissionProfile/list", params)
    }

    pub fn read_config_requirements(
        &self,
    ) -> Result<ConfigRequirementsReadResponse, AppServerError> {
        self.require_initialized()?;
        self.request_without_params("configRequirements/read")
    }

    pub fn read_account(
        &self,
        params: GetAccountParams,
    ) -> Result<GetAccountResponse, AppServerError> {
        self.require_initialized()?;
        self.request("account/read", params)
    }

    pub fn read_account_rate_limits(&self) -> Result<GetAccountRateLimitsResponse, AppServerError> {
        self.require_initialized()?;
        self.request_without_params("account/rateLimits/read")
    }

    pub fn get_auth_status(
        &self,
        params: GetAuthStatusParams,
    ) -> Result<GetAuthStatusResponse, AppServerError> {
        self.require_initialized()?;
        self.request("getAuthStatus", params)
    }

    pub fn git_diff_to_remote(
        &self,
        params: GitDiffToRemoteParams,
    ) -> Result<GitDiffToRemoteResponse, AppServerError> {
        self.require_initialized()?;
        self.request("gitDiffToRemote", params)
    }

    pub fn start_account_login(
        &self,
        params: LoginAccountParams,
    ) -> Result<LoginAccountResponse, AppServerError> {
        self.require_initialized()?;
        self.request("account/login/start", params)
    }

    pub fn cancel_account_login(
        &self,
        params: CancelLoginAccountParams,
    ) -> Result<CancelLoginAccountResponse, AppServerError> {
        self.require_initialized()?;
        self.request("account/login/cancel", params)
    }

    pub fn logout_account(&self) -> Result<LogoutAccountResponse, AppServerError> {
        self.require_initialized()?;
        self.request_without_params("account/logout")
    }

    pub fn upload_feedback(
        &self,
        params: FeedbackUploadParams,
    ) -> Result<FeedbackUploadResponse, AppServerError> {
        self.require_initialized()?;
        self.request("feedback/upload", params)
    }

    pub fn read_config(
        &self,
        params: ConfigReadParams,
    ) -> Result<ConfigReadResponse, AppServerError> {
        self.require_initialized()?;
        self.request("config/read", params)
    }

    pub fn batch_write_config(
        &self,
        params: ConfigBatchWriteParams,
    ) -> Result<ConfigWriteResponse, AppServerError> {
        self.require_initialized()?;
        self.request("config/batchWrite", params)
    }

    pub fn list_plugins(
        &self,
        params: PluginListParams,
    ) -> Result<PluginListResponse, AppServerError> {
        self.require_initialized()?;
        self.request("plugin/list", params)
    }

    pub fn add_marketplace(
        &self,
        params: MarketplaceAddParams,
    ) -> Result<MarketplaceAddResponse, AppServerError> {
        self.require_initialized()?;
        self.request("marketplace/add", params)
    }

    pub fn remove_marketplace(
        &self,
        params: MarketplaceRemoveParams,
    ) -> Result<MarketplaceRemoveResponse, AppServerError> {
        self.require_initialized()?;
        self.request("marketplace/remove", params)
    }

    pub fn upgrade_marketplaces(
        &self,
        params: MarketplaceUpgradeParams,
    ) -> Result<MarketplaceUpgradeResponse, AppServerError> {
        self.require_initialized()?;
        self.request("marketplace/upgrade", params)
    }

    pub fn read_plugin(
        &self,
        params: PluginReadParams,
    ) -> Result<PluginReadResponse, AppServerError> {
        self.require_initialized()?;
        self.request("plugin/read", params)
    }

    pub fn list_skills(
        &self,
        params: SkillsListParams,
    ) -> Result<SkillsListResponse, AppServerError> {
        self.require_initialized()?;
        self.request("skills/list", params)
    }

    pub fn write_skill_config(
        &self,
        params: SkillsConfigWriteParams,
    ) -> Result<SkillsConfigWriteResponse, AppServerError> {
        self.require_initialized()?;
        self.request("skills/config/write", params)
    }

    pub fn list_hooks(&self, params: HooksListParams) -> Result<HooksListResponse, AppServerError> {
        self.require_initialized()?;
        self.request("hooks/list", params)
    }

    pub fn install_plugin(
        &self,
        params: PluginInstallParams,
    ) -> Result<PluginInstallResponse, AppServerError> {
        self.require_initialized()?;
        self.request("plugin/install", params)
    }

    pub fn uninstall_plugin(&self, params: PluginUninstallParams) -> Result<Value, AppServerError> {
        self.require_initialized()?;
        self.request("plugin/uninstall", params)
    }

    pub fn list_apps(&self, params: AppsListParams) -> Result<AppsListResponse, AppServerError> {
        self.require_initialized()?;
        if !(1..=MAX_THREAD_PAGE_LIMIT).contains(&params.limit) {
            return Err(AppServerError::InvalidPageLimit {
                requested: params.limit,
                maximum: MAX_THREAD_PAGE_LIMIT,
            });
        }
        self.request("app/list", params)
    }

    pub fn read_apps(&self, params: AppsReadParams) -> Result<AppsReadResponse, AppServerError> {
        self.require_initialized()?;
        if params.app_ids.len() > MAX_APP_READ_ITEMS {
            return Err(AppServerError::InvalidPageLimit {
                requested: u32::try_from(params.app_ids.len()).unwrap_or(u32::MAX),
                maximum: MAX_APP_READ_ITEMS as u32,
            });
        }
        self.request("app/read", params)
    }

    pub fn list_mcp_server_status(
        &self,
        params: ListMcpServerStatusParams,
    ) -> Result<ListMcpServerStatusResponse, AppServerError> {
        self.require_initialized()?;
        if !(1..=MAX_THREAD_PAGE_LIMIT).contains(&params.limit) {
            return Err(AppServerError::InvalidPageLimit {
                requested: params.limit,
                maximum: MAX_THREAD_PAGE_LIMIT,
            });
        }
        self.request("mcpServerStatus/list", params)
    }

    pub fn read_mcp_resource(
        &self,
        params: McpResourceReadParams,
    ) -> Result<McpResourceReadResponse, AppServerError> {
        self.require_initialized()?;
        self.request("mcpServer/resource/read", params)
    }

    pub fn login_mcp_server(
        &self,
        params: McpServerOauthLoginParams,
    ) -> Result<McpServerOauthLoginResponse, AppServerError> {
        self.require_initialized()?;
        self.request("mcpServer/oauth/login", params)
    }

    pub fn reload_mcp_servers(&self) -> Result<(), AppServerError> {
        self.require_initialized()?;
        self.request_without_params::<Value>("config/mcpServer/reload")
            .map(|_| ())
    }

    pub fn shutdown(&mut self) -> Result<(), AppServerError> {
        if self.closed.swap(true, Ordering::AcqRel) {
            return Ok(());
        }

        let (reply_sender, reply_receiver) = crossbeam_channel::bounded(1);
        let send_result = self.commands.send_timeout(
            RouterCommand::Shutdown(reply_sender),
            EVENT_BACKPRESSURE_TIMEOUT,
        );
        let shutdown_result = match send_result {
            Ok(()) => reply_receiver
                .recv_timeout(self.config.shutdown_timeout + Duration::from_secs(1))
                .map_err(|_| AppServerError::TransportClosed)?,
            Err(_) => Err(AppServerError::TransportClosed),
        };
        let join_result = match self.router_thread.take() {
            Some(thread) => thread
                .join()
                .map_err(|_| AppServerError::BackgroundThreadPanicked),
            None => Ok(()),
        };

        shutdown_result?;
        join_result
    }

    fn send_frame_command(&self, frame: Vec<u8>) -> Result<(), AppServerError> {
        self.commands
            .send_timeout(
                RouterCommand::Frame(frame),
                self.config.request_timeout.min(EVENT_BACKPRESSURE_TIMEOUT),
            )
            .map_err(|error| match error {
                crossbeam_channel::SendTimeoutError::Timeout(_) => AppServerError::CommandQueueFull,
                crossbeam_channel::SendTimeoutError::Disconnected(_) => {
                    AppServerError::TransportClosed
                }
            })
    }

    fn allocate_request_id(&self) -> Result<u64, AppServerError> {
        self.next_request_id
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map_err(|_| AppServerError::RequestIdExhausted)
    }

    fn require_initialized(&self) -> Result<(), AppServerError> {
        if self.initialized.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err(AppServerError::NotInitialized)
        }
    }

    fn verify_reported_home(&self, response: &InitializeResponse) -> Result<(), AppServerError> {
        let reported = fs::canonicalize(&response.codex_home)
            .map_err(|_| AppServerError::CodexHomeMismatch)?;
        if paths_match(self.config.codex_home.path(), &reported) {
            Ok(())
        } else {
            Err(AppServerError::CodexHomeMismatch)
        }
    }
}

impl Drop for AppServerConnection {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn validate_page_limit(limit: u32) -> Result<(), AppServerError> {
    if (1..=MAX_THREAD_PAGE_LIMIT).contains(&limit) {
        Ok(())
    } else {
        Err(AppServerError::InvalidPageLimit {
            requested: limit,
            maximum: MAX_THREAD_PAGE_LIMIT,
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn route_app_server(
    mut child: ManagedChild,
    stdin: ChildStdin,
    commands: CrossbeamReceiver<RouterCommand>,
    incoming: CrossbeamReceiver<RoutedReaderEvent>,
    events: CrossbeamSender<AppServerEvent>,
    stdout_thread: JoinHandle<()>,
    stderr_thread: JoinHandle<()>,
    max_frame_bytes: NonZeroUsize,
    shutdown_timeout: Duration,
) {
    let mut stdin = Some(stdin);
    let mut pending = HashMap::<u64, PendingRequest>::new();
    let mut dropped_notifications = 0_usize;
    let mut running = true;

    while running {
        expire_pending_requests(&mut pending);

        crossbeam_channel::select! {
            recv(commands) -> command => {
                match command {
                    Ok(RouterCommand::Request { id, method, frame, deadline, reply }) => {
                        if pending.len() >= MAX_PENDING_REQUESTS {
                            let _ = reply.try_send(Err(AppServerError::TooManyPendingRequests));
                            continue;
                        }
                        pending.insert(id, PendingRequest { method, deadline, reply });
                        if let Err(error) = write_router_frame(&mut stdin, &frame) {
                            if let Some(request) = pending.remove(&id) {
                                let _ = request.reply.try_send(Err(error));
                            }
                            fail_pending_requests(&mut pending);
                            running = false;
                        }
                    }
                    Ok(RouterCommand::Frame(frame)) => {
                        if write_router_frame(&mut stdin, &frame).is_err() {
                            fail_pending_requests(&mut pending);
                            running = false;
                        }
                    }
                    Ok(RouterCommand::Shutdown(reply)) => {
                        fail_pending_requests(&mut pending);
                        drop(stdin.take());
                        let result = child.shutdown(shutdown_timeout);
                        let stdout_result = join_background_thread(Some(stdout_thread));
                        let stderr_result = join_background_thread(Some(stderr_thread));
                        let result = result.and(stdout_result).and(stderr_result);
                        let _ = reply.try_send(result);
                        let _ = events.try_send(AppServerEvent::Disconnected);
                        return;
                    }
                    Err(_) => {
                        fail_pending_requests(&mut pending);
                        running = false;
                    }
                }
            }
            recv(incoming) -> event => {
                match event {
                    Ok(RoutedReaderEvent::Message(Ok(message))) => {
                        if handle_routed_message(
                            message,
                            &mut pending,
                            &events,
                            &mut stdin,
                            max_frame_bytes,
                            &mut dropped_notifications,
                        ).is_err() {
                            fail_pending_requests(&mut pending);
                            running = false;
                        }
                    }
                    Ok(RoutedReaderEvent::Message(Err(_))) | Ok(RoutedReaderEvent::Eof) | Err(_) => {
                        fail_pending_requests(&mut pending);
                        running = false;
                    }
                }
            }
            default(ROUTER_TICK) => {}
        }
    }

    drop(stdin.take());
    let _ = child.shutdown(shutdown_timeout);
    let _ = join_background_thread(Some(stdout_thread));
    let _ = join_background_thread(Some(stderr_thread));
    let _ = events.try_send(AppServerEvent::Disconnected);
}

fn handle_routed_message(
    message: IncomingMessage,
    pending: &mut HashMap<u64, PendingRequest>,
    events: &CrossbeamSender<AppServerEvent>,
    stdin: &mut Option<ChildStdin>,
    max_frame_bytes: NonZeroUsize,
    dropped_notifications: &mut usize,
) -> Result<(), AppServerError> {
    match message {
        IncomingMessage::Success { id, result } => {
            let Some(id) = id.as_u64() else {
                return Err(AppServerError::UnexpectedResponseId);
            };
            if let Some(request) = pending.remove(&id) {
                let _ = request.reply.try_send(Ok(result));
            }
        }
        IncomingMessage::Failure { id, code } => {
            let Some(id) = id.as_u64() else {
                return Err(AppServerError::UnexpectedResponseId);
            };
            if let Some(request) = pending.remove(&id) {
                let _ = request
                    .reply
                    .try_send(Err(AppServerError::RequestFailed { code }));
            }
        }
        IncomingMessage::Request { id, method, params } => {
            let event = AppServerEvent::Request {
                id: id.clone(),
                method,
                params,
            };
            if events
                .send_timeout(event, EVENT_BACKPRESSURE_TIMEOUT)
                .is_err()
            {
                let frame = encode_error_response(
                    &id,
                    -32_000,
                    "client event queue is busy",
                    max_frame_bytes,
                )?;
                write_router_frame(stdin, &frame)?;
            }
        }
        IncomingMessage::Notification { method, params } => {
            publish_drop_count(events, dropped_notifications);
            match events.try_send(AppServerEvent::Notification { method, params }) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => {
                    *dropped_notifications = dropped_notifications.saturating_add(1);
                }
                Err(TrySendError::Disconnected(_)) => {
                    return Err(AppServerError::TransportClosed);
                }
            }
        }
    }
    Ok(())
}

fn publish_drop_count(events: &CrossbeamSender<AppServerEvent>, dropped_notifications: &mut usize) {
    if *dropped_notifications == 0 {
        return;
    }
    if events
        .try_send(AppServerEvent::NotificationsDropped {
            count: *dropped_notifications,
        })
        .is_ok()
    {
        *dropped_notifications = 0;
    }
}

fn write_router_frame(stdin: &mut Option<ChildStdin>, frame: &[u8]) -> Result<(), AppServerError> {
    let stdin = stdin.as_mut().ok_or(AppServerError::TransportClosed)?;
    stdin.write_all(frame).map_err(AppServerError::Transport)?;
    stdin.flush().map_err(AppServerError::Transport)
}

fn expire_pending_requests(pending: &mut HashMap<u64, PendingRequest>) {
    let now = Instant::now();
    let expired = pending
        .iter()
        .filter_map(|(id, request)| (request.deadline <= now).then_some(*id))
        .collect::<Vec<_>>();
    for id in expired {
        if let Some(request) = pending.remove(&id) {
            let _ = request
                .reply
                .try_send(Err(AppServerError::RequestTimedOut(request.method)));
        }
    }
}

fn fail_pending_requests(pending: &mut HashMap<u64, PendingRequest>) {
    for (_, request) in pending.drain() {
        let _ = request.reply.try_send(Err(AppServerError::TransportClosed));
    }
}

fn routed_stdout_reader(
    stdout: ChildStdout,
    sender: CrossbeamSender<RoutedReaderEvent>,
    max_frame_bytes: NonZeroUsize,
) {
    let mut reader = BufReader::new(stdout);
    loop {
        match read_bounded_frame(&mut reader, max_frame_bytes) {
            Ok(Some(frame)) => {
                let message = decode_incoming(&frame);
                let terminal = message.is_err();
                if sender.send(RoutedReaderEvent::Message(message)).is_err() || terminal {
                    return;
                }
            }
            Ok(None) => {
                let _ = sender.send(RoutedReaderEvent::Eof);
                return;
            }
            Err(error) => {
                let _ = sender.send(RoutedReaderEvent::Message(Err(error)));
                return;
            }
        }
    }
}

fn add_interleaved(current: usize) -> Result<usize, AppServerError> {
    let next = current.saturating_add(1);
    if next > MAX_INTERLEAVED_MESSAGES_PER_REQUEST {
        Err(AppServerError::TooManyInterleavedMessages)
    } else {
        Ok(next)
    }
}

fn join_background_thread(thread: Option<JoinHandle<()>>) -> Result<(), AppServerError> {
    match thread {
        Some(thread) => thread
            .join()
            .map_err(|_| AppServerError::BackgroundThreadPanicked),
        None => Ok(()),
    }
}

enum ReaderEvent {
    Message(Result<IncomingMessage, ProtocolError>),
    Eof,
}

fn stdout_reader(
    stdout: ChildStdout,
    sender: SyncSender<ReaderEvent>,
    max_frame_bytes: NonZeroUsize,
) {
    let mut reader = BufReader::new(stdout);
    loop {
        match read_bounded_frame(&mut reader, max_frame_bytes) {
            Ok(Some(frame)) => {
                let message = decode_incoming(&frame);
                let terminal = message.is_err();
                if sender.send(ReaderEvent::Message(message)).is_err() || terminal {
                    return;
                }
            }
            Ok(None) => {
                let _ = sender.send(ReaderEvent::Eof);
                return;
            }
            Err(error) => {
                let _ = sender.send(ReaderEvent::Message(Err(error)));
                return;
            }
        }
    }
}

fn drain_stderr(mut stderr: ChildStderr) {
    let mut buffer = [0_u8; 4 * 1024];
    loop {
        match stderr.read(&mut buffer) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
    }
}

struct ManagedChild {
    child: Child,
    #[cfg(windows)]
    job: Option<Job>,
}

impl ManagedChild {
    fn spawn(command: &mut Command) -> Result<Self, AppServerError> {
        #[cfg(windows)]
        let job = {
            let mut limits = ExtendedLimitInfo::new();
            limits.limit_kill_on_job_close();
            Job::create_with_limit_info(&limits)?
        };

        let child = command.spawn().map_err(AppServerError::Spawn)?;
        #[cfg(windows)]
        let mut child = child;

        #[cfg(windows)]
        {
            if let Err(error) = job.assign_process(child.as_raw_handle() as isize) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.into());
            }
            Ok(Self {
                child,
                job: Some(job),
            })
        }

        #[cfg(not(windows))]
        {
            Ok(Self { child })
        }
    }

    fn take_stdin(&mut self) -> Option<ChildStdin> {
        self.child.stdin.take()
    }

    fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.stdout.take()
    }

    fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.child.stderr.take()
    }

    fn shutdown(&mut self, timeout: Duration) -> Result<(), AppServerError> {
        let deadline = Instant::now() + timeout;
        loop {
            if self
                .child
                .try_wait()
                .map_err(AppServerError::Transport)?
                .is_some()
            {
                #[cfg(windows)]
                drop(self.job.take());
                return Ok(());
            }

            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            thread::sleep(remaining.min(SHUTDOWN_POLL_INTERVAL));
        }

        self.force_terminate();
        Ok(())
    }

    fn force_terminate(&mut self) {
        #[cfg(windows)]
        {
            drop(self.job.take());
        }
        #[cfg(not(windows))]
        {
            let _ = self.child.kill();
        }
    }
}

impl Drop for ManagedChild {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            self.force_terminate();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::PathBuf;

    use super::{CodexHome, CodexHomeKind, MAX_THREAD_PAGE_LIMIT};

    #[test]
    fn configured_home_is_canonicalized_without_reading_its_contents() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("target")
            .join("test-fixtures")
            .join("codex-home-test");
        if let Err(error) = fs::create_dir_all(&fixture) {
            panic!("could not create test fixture: {error}");
        }

        let home = match CodexHome::resolve(Some(fixture.clone())) {
            Ok(home) => home,
            Err(error) => panic!("could not resolve test fixture: {error}"),
        };

        assert!(home.path().is_absolute());
        assert_eq!(home.kind(), CodexHomeKind::Configured);
        assert_eq!(MAX_THREAD_PAGE_LIMIT, 100);
    }
}
