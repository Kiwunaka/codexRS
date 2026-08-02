use std::collections::VecDeque;
use std::error::Error;
#[cfg(windows)]
use std::ffi::OsString;
use std::fmt;
use std::io::{self, Read, Write};
#[cfg(windows)]
use std::path::Path;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use codex_core::IntegratedTerminalShell;
use crossbeam_channel::{Receiver, Sender, TryRecvError, TrySendError};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};

#[cfg(windows)]
use win32job::{ExtendedLimitInfo, Job};

pub const MAX_TERMINAL_INPUT_BYTES: usize = 64 * 1024;
pub const TERMINAL_EVENT_CAPACITY: usize = 256;

const TERMINAL_COMMAND_CAPACITY: usize = 64;
const TERMINAL_READ_CHUNK_BYTES: usize = 8 * 1024;
const TERMINAL_TICK: Duration = Duration::from_millis(25);
const TERMINAL_GRACEFUL_EXIT: Duration = Duration::from_millis(750);

#[derive(Debug, Clone)]
pub struct TerminalConfig {
    pub cwd: PathBuf,
    pub rows: u16,
    pub cols: u16,
    pub shell: Option<IntegratedTerminalShell>,
}

impl TerminalConfig {
    #[must_use]
    pub fn new(cwd: PathBuf) -> Self {
        Self {
            cwd,
            rows: 24,
            cols: 80,
            shell: None,
        }
    }

    #[must_use]
    pub fn with_shell(mut self, shell: Option<IntegratedTerminalShell>) -> Self {
        self.shell = shell;
        self
    }
}

#[must_use]
pub fn available_terminal_shells() -> Vec<IntegratedTerminalShell> {
    #[cfg(windows)]
    {
        let mut shells = vec![
            IntegratedTerminalShell::PowerShell,
            IntegratedTerminalShell::CommandPrompt,
        ];
        if windows_git_bash().is_some() {
            shells.push(IntegratedTerminalShell::GitBash);
        }
        if find_windows_executable("wsl.exe").is_some() {
            shells.push(IntegratedTerminalShell::Wsl);
        }
        shells
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalEvent {
    Output(Vec<u8>),
    Exited { code: u32 },
    Failed(&'static str),
}

#[derive(Debug)]
pub enum TerminalError {
    Open,
    Spawn,
    Stream(&'static str),
    Thread(io::Error),
    #[cfg(windows)]
    Job(win32job::JobError),
    #[cfg(windows)]
    MissingProcessHandle,
}

impl fmt::Display for TerminalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open => formatter.write_str("could not open a native pseudo-terminal"),
            Self::Spawn => formatter.write_str("could not start the default shell"),
            Self::Stream(name) => write!(formatter, "could not open terminal {name}"),
            Self::Thread(_) => formatter.write_str("could not start the terminal supervisor"),
            #[cfg(windows)]
            Self::Job(_) => formatter.write_str("could not supervise the terminal process tree"),
            #[cfg(windows)]
            Self::MissingProcessHandle => {
                formatter.write_str("terminal child did not expose a process handle")
            }
        }
    }
}

impl Error for TerminalError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Thread(error) => Some(error),
            #[cfg(windows)]
            Self::Job(error) => Some(error),
            Self::Open | Self::Spawn | Self::Stream(_) => None,
            #[cfg(windows)]
            Self::MissingProcessHandle => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalCommandError {
    InputTooLarge,
    QueueFull,
    Disconnected,
}

impl fmt::Display for TerminalCommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputTooLarge => formatter.write_str("terminal input exceeds 64 KiB"),
            Self::QueueFull => formatter.write_str("terminal command queue is full"),
            Self::Disconnected => formatter.write_str("terminal is disconnected"),
        }
    }
}

impl Error for TerminalCommandError {}

enum TerminalCommand {
    Write(Vec<u8>),
    DeviceStatusQuery,
    Resize(PtySize),
    Shutdown,
}

pub struct TerminalSession {
    commands: Sender<TerminalCommand>,
    events: Receiver<TerminalEvent>,
    output_truncated: Arc<AtomicBool>,
    shutdown_requested: Arc<AtomicBool>,
    process_id: Option<u32>,
    thread: Option<JoinHandle<()>>,
}

impl TerminalSession {
    pub fn spawn(config: TerminalConfig) -> Result<Self, TerminalError> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: config.rows.max(1),
                cols: config.cols.max(1),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|_| TerminalError::Open)?;

        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|_| TerminalError::Stream("reader"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|_| TerminalError::Stream("writer"))?;
        let mut command = terminal_command(config.shell);
        command.cwd(&config.cwd);
        command.env("TERM", "xterm-256color");
        let mut child = pair
            .slave
            .spawn_command(command)
            .map_err(|_| TerminalError::Spawn)?;
        let process_id = child.process_id();
        let job = create_terminal_job(child.as_ref())?;
        drop(pair.slave);

        let (command_sender, command_receiver) =
            crossbeam_channel::bounded(TERMINAL_COMMAND_CAPACITY);
        let (event_sender, event_receiver) = crossbeam_channel::bounded(TERMINAL_EVENT_CAPACITY);
        let output_truncated = Arc::new(AtomicBool::new(false));
        let reader_truncated = Arc::clone(&output_truncated);
        let shutdown_requested = Arc::new(AtomicBool::new(false));
        let terminal_shutdown_requested = Arc::clone(&shutdown_requested);
        let response_commands = command_sender.clone();
        let thread = thread::Builder::new()
            .name("codex-terminal-supervisor".to_owned())
            .spawn(move || {
                run_terminal(
                    pair.master,
                    writer,
                    reader,
                    &mut child,
                    job,
                    command_receiver,
                    response_commands,
                    event_sender,
                    reader_truncated,
                    terminal_shutdown_requested,
                );
            })
            .map_err(TerminalError::Thread)?;

        Ok(Self {
            commands: command_sender,
            events: event_receiver,
            output_truncated,
            shutdown_requested,
            process_id,
            thread: Some(thread),
        })
    }

    #[must_use]
    pub fn process_id(&self) -> Option<u32> {
        self.process_id
    }

    pub fn write(&self, bytes: &[u8]) -> Result<(), TerminalCommandError> {
        if bytes.len() > MAX_TERMINAL_INPUT_BYTES {
            return Err(TerminalCommandError::InputTooLarge);
        }
        send_command(&self.commands, TerminalCommand::Write(bytes.to_vec()))
    }

    pub fn resize(&self, rows: u16, cols: u16) -> Result<(), TerminalCommandError> {
        send_command(
            &self.commands,
            TerminalCommand::Resize(PtySize {
                rows: rows.max(1),
                cols: cols.max(1),
                pixel_width: 0,
                pixel_height: 0,
            }),
        )
    }

    pub fn try_recv_event(&self) -> Result<Option<TerminalEvent>, TerminalCommandError> {
        match self.events.try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(TerminalCommandError::Disconnected),
        }
    }

    #[must_use]
    pub fn output_was_truncated(&self) -> bool {
        self.output_truncated.load(Ordering::Relaxed)
    }

    pub fn shutdown(&mut self) {
        self.shutdown_requested.store(true, Ordering::Release);
        let _ = self.commands.try_send(TerminalCommand::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(not(windows))]
fn terminal_command(_shell: Option<IntegratedTerminalShell>) -> CommandBuilder {
    CommandBuilder::new_default_prog()
}

#[cfg(windows)]
fn terminal_command(shell: Option<IntegratedTerminalShell>) -> CommandBuilder {
    CommandBuilder::from_argv(windows_shell_argv(shell))
}

#[cfg(windows)]
fn windows_shell_argv(shell: Option<IntegratedTerminalShell>) -> Vec<OsString> {
    let fallback = || {
        windows_powershell()
            .map(|program| vec![program.into_os_string()])
            .unwrap_or_else(|| vec![windows_command_prompt().into_os_string()])
    };
    match shell {
        Some(IntegratedTerminalShell::PowerShell) => fallback(),
        Some(IntegratedTerminalShell::CommandPrompt) => {
            vec![windows_command_prompt().into_os_string()]
        }
        Some(IntegratedTerminalShell::GitBash) => {
            windows_git_bash().map_or_else(fallback, |program| {
                vec![
                    program.into_os_string(),
                    OsString::from("--login"),
                    OsString::from("-i"),
                ]
            })
        }
        Some(IntegratedTerminalShell::Wsl) => find_windows_executable("wsl.exe")
            .map(|program| vec![program.into_os_string()])
            .unwrap_or_else(fallback),
        None => fallback(),
    }
}

#[cfg(windows)]
fn windows_powershell() -> Option<PathBuf> {
    ["pwsh.exe", "powershell.exe"]
        .into_iter()
        .find_map(find_windows_executable)
}

#[cfg(windows)]
fn windows_command_prompt() -> PathBuf {
    std::env::var_os("COMSPEC")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("cmd.exe"))
}

#[cfg(windows)]
fn windows_git_bash() -> Option<PathBuf> {
    if let Some(program) = find_windows_executable("git-bash.exe") {
        return Some(program);
    }
    if let Some(git) = find_windows_executable("git.exe") {
        let directory = git.parent()?;
        let mut candidates = vec![directory.join("bash.exe")];
        if let Some(parent) = directory.parent() {
            candidates.push(parent.join("bin").join("bash.exe"));
        }
        for candidate in candidates {
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    windows_install_candidate(Path::new("Git").join("bin").join("bash.exe"))
}

#[cfg(windows)]
fn find_windows_executable(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
}

#[cfg(windows)]
fn windows_install_candidate(relative: PathBuf) -> Option<PathBuf> {
    let local_programs = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|path| path.join("Programs"));
    [
        local_programs,
        std::env::var_os("ProgramFiles").map(PathBuf::from),
        std::env::var_os("ProgramFiles(x86)").map(PathBuf::from),
    ]
    .into_iter()
    .flatten()
    .map(|root| root.join(&relative))
    .find(|candidate| candidate.is_file())
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn send_command(
    sender: &Sender<TerminalCommand>,
    command: TerminalCommand,
) -> Result<(), TerminalCommandError> {
    sender.try_send(command).map_err(|error| match error {
        TrySendError::Full(_) => TerminalCommandError::QueueFull,
        TrySendError::Disconnected(_) => TerminalCommandError::Disconnected,
    })
}

#[cfg(windows)]
type TerminalJob = Job;

#[cfg(not(windows))]
struct TerminalJob;

#[cfg(windows)]
fn create_terminal_job(child: &(dyn Child + Send + Sync)) -> Result<TerminalJob, TerminalError> {
    let mut limits = ExtendedLimitInfo::new();
    limits.limit_kill_on_job_close();
    let job = Job::create_with_limit_info(&limits).map_err(TerminalError::Job)?;
    let handle = child
        .as_raw_handle()
        .ok_or(TerminalError::MissingProcessHandle)?;
    if let Err(error) = job.assign_process(handle as isize) {
        return Err(TerminalError::Job(error));
    }
    Ok(job)
}

#[cfg(not(windows))]
fn create_terminal_job(_child: &(dyn Child + Send + Sync)) -> Result<TerminalJob, TerminalError> {
    Ok(TerminalJob)
}

#[allow(clippy::too_many_arguments)]
fn run_terminal(
    master: Box<dyn MasterPty + Send>,
    mut writer: Box<dyn Write + Send>,
    reader: Box<dyn Read + Send>,
    child: &mut Box<dyn Child + Send + Sync>,
    _job: TerminalJob,
    commands: Receiver<TerminalCommand>,
    response_commands: Sender<TerminalCommand>,
    events: Sender<TerminalEvent>,
    output_truncated: Arc<AtomicBool>,
    shutdown_requested: Arc<AtomicBool>,
) {
    let reader_events = events.clone();
    let reader_thread = thread::Builder::new()
        .name("codex-terminal-reader".to_owned())
        .spawn(move || {
            read_terminal(reader, reader_events, response_commands, output_truncated);
        })
        .ok();

    let mut exit_code = None;
    let mut terminal_ready = !cfg!(windows);
    let mut pending_input = VecDeque::with_capacity(TERMINAL_COMMAND_CAPACITY);
    'supervisor: loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                exit_code = Some(status.exit_code());
                break;
            }
            Ok(None) => {}
            Err(_) => {
                let _ = events.try_send(TerminalEvent::Failed("terminal wait failed"));
                break;
            }
        }

        let command = next_terminal_command(&shutdown_requested, &commands);
        if shutdown_requested.load(Ordering::Acquire) {
            graceful_terminal_exit(child.as_mut(), writer.as_mut());
            exit_code = child
                .try_wait()
                .ok()
                .flatten()
                .map(|status| status.exit_code());
            break;
        }

        match command {
            Ok(TerminalCommand::Write(bytes)) => {
                if !terminal_ready {
                    pending_input.push_back(bytes);
                    continue;
                }
                if writer
                    .write_all(&bytes)
                    .and_then(|()| writer.flush())
                    .is_err()
                {
                    let _ = events.try_send(TerminalEvent::Failed("terminal input failed"));
                    break;
                }
            }
            Ok(TerminalCommand::DeviceStatusQuery) => {
                if writer
                    .write_all(b"\x1b[1;1R")
                    .and_then(|()| writer.flush())
                    .is_err()
                {
                    let _ = events.try_send(TerminalEvent::Failed("terminal response failed"));
                    break;
                }
                terminal_ready = true;
                while let Some(bytes) = pending_input.pop_front() {
                    if writer
                        .write_all(&bytes)
                        .and_then(|()| writer.flush())
                        .is_err()
                    {
                        let _ = events.try_send(TerminalEvent::Failed("terminal input failed"));
                        break 'supervisor;
                    }
                }
            }
            Ok(TerminalCommand::Resize(size)) => {
                if master.resize(size).is_err() {
                    let _ = events.try_send(TerminalEvent::Failed("terminal resize failed"));
                }
            }
            Ok(TerminalCommand::Shutdown)
            | Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                graceful_terminal_exit(child.as_mut(), writer.as_mut());
                exit_code = child
                    .try_wait()
                    .ok()
                    .flatten()
                    .map(|status| status.exit_code());
                break;
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
        }
    }

    drop(writer);
    drop(master);
    if let Some(reader_thread) = reader_thread {
        let _ = reader_thread.join();
    }
    let _ = events.try_send(TerminalEvent::Exited {
        code: exit_code.unwrap_or(127),
    });
}

fn next_terminal_command(
    shutdown_requested: &AtomicBool,
    commands: &Receiver<TerminalCommand>,
) -> Result<TerminalCommand, crossbeam_channel::RecvTimeoutError> {
    if shutdown_requested.load(Ordering::Acquire) {
        Ok(TerminalCommand::Shutdown)
    } else {
        commands.recv_timeout(TERMINAL_TICK)
    }
}

fn graceful_terminal_exit(child: &mut (dyn Child + Send + Sync), writer: &mut (dyn Write + Send)) {
    let _ = writer.write_all(b"exit\r");
    let _ = writer.flush();
    let deadline = Instant::now() + TERMINAL_GRACEFUL_EXIT;
    while Instant::now() < deadline {
        if child.try_wait().ok().flatten().is_some() {
            return;
        }
        thread::sleep(TERMINAL_TICK);
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn read_terminal(
    mut reader: Box<dyn Read + Send>,
    events: Sender<TerminalEvent>,
    response_commands: Sender<TerminalCommand>,
    output_truncated: Arc<AtomicBool>,
) {
    let mut buffer = [0_u8; TERMINAL_READ_CHUNK_BYTES];
    let mut carry = Vec::with_capacity(3);
    loop {
        let read = match reader.read(&mut buffer) {
            Ok(0) | Err(_) => return,
            Ok(read) => read,
        };
        let mut query_window = Vec::with_capacity(carry.len() + read);
        query_window.extend_from_slice(&carry);
        query_window.extend_from_slice(&buffer[..read]);
        if query_window.windows(4).any(|window| window == b"\x1b[6n") {
            let _ = response_commands.try_send(TerminalCommand::DeviceStatusQuery);
        }
        carry.clear();
        let carry_start = query_window.len().saturating_sub(3);
        carry.extend_from_slice(&query_window[carry_start..]);
        match events.try_send(TerminalEvent::Output(buffer[..read].to_vec())) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                output_truncated.store(true, Ordering::Relaxed);
            }
            Err(TrySendError::Disconnected(_)) => return,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io::Cursor;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use std::time::{Duration, Instant};

    #[cfg(windows)]
    use super::available_terminal_shells;
    #[cfg(windows)]
    use codex_core::IntegratedTerminalShell;

    use super::{
        MAX_TERMINAL_INPUT_BYTES, TerminalCommand, TerminalConfig, TerminalEvent, TerminalSession,
        next_terminal_command, read_terminal,
    };

    #[test]
    fn shutdown_request_outranks_a_full_terminal_command_queue() {
        let (sender, receiver) = crossbeam_channel::bounded(1);
        assert!(sender.try_send(TerminalCommand::Write(vec![b'x'])).is_ok());
        let shutdown_requested = AtomicBool::new(false);
        shutdown_requested.store(true, Ordering::Release);

        assert!(matches!(
            next_terminal_command(&shutdown_requested, &receiver),
            Ok(TerminalCommand::Shutdown)
        ));
        assert!(matches!(
            receiver.try_recv(),
            Ok(TerminalCommand::Write(bytes)) if bytes == b"x"
        ));
    }

    #[test]
    fn reader_marks_output_truncated_when_the_bounded_queue_fills() {
        let (sender, receiver) = crossbeam_channel::bounded(1);
        let (command_sender, _command_receiver) = crossbeam_channel::bounded(1);
        let truncated = Arc::new(AtomicBool::new(false));
        read_terminal(
            Box::new(Cursor::new(vec![b'x'; 3 * 8 * 1024])),
            sender,
            command_sender,
            Arc::clone(&truncated),
        );

        assert!(matches!(receiver.try_recv(), Ok(TerminalEvent::Output(_))));
        assert!(truncated.load(Ordering::Relaxed));
    }

    #[test]
    fn terminal_input_budget_is_fixed() {
        assert_eq!(MAX_TERMINAL_INPUT_BYTES, 64 * 1024);
    }

    #[cfg(windows)]
    #[test]
    fn windows_shell_catalog_keeps_the_stable_default_order() {
        assert!(available_terminal_shells().starts_with(&[
            IntegratedTerminalShell::PowerShell,
            IntegratedTerminalShell::CommandPrompt,
        ]));
    }

    #[test]
    fn native_terminal_runs_a_bounded_shell_round_trip() -> Result<(), Box<dyn Error>> {
        let config = TerminalConfig::new(std::env::current_dir()?);
        #[cfg(windows)]
        let config = config.with_shell(Some(IntegratedTerminalShell::CommandPrompt));
        let mut session = TerminalSession::spawn(config)?;
        session.write(b"echo codexrs-terminal-smoke\rexit\r")?;

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut output = String::new();
        let mut exit_code = None;
        while Instant::now() < deadline {
            match session.try_recv_event() {
                Ok(Some(TerminalEvent::Output(bytes))) => {
                    if output.len() < 64 * 1024 {
                        output.push_str(&String::from_utf8_lossy(&bytes));
                    }
                }
                Ok(Some(TerminalEvent::Exited { code })) => {
                    exit_code = Some(code);
                    break;
                }
                Ok(Some(TerminalEvent::Failed(message))) => {
                    return Err(std::io::Error::other(message).into());
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                Err(error) => return Err(error.into()),
            }
        }
        session.shutdown();

        assert!(
            output.contains("codexrs-terminal-smoke"),
            "terminal produced {:?} and exit code {exit_code:?}",
            output.as_bytes()
        );
        Ok(())
    }
}
