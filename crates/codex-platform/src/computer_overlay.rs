use std::io;

use serde::{Deserialize, Serialize};

const MAX_OVERLAY_ID_BYTES: usize = 1_024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputerUseOverlayTarget {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl ComputerUseOverlayTarget {
    #[must_use]
    pub fn from_window(window: &crate::ComputerWindow) -> Self {
        Self {
            x: window.x,
            y: window.y,
            width: window.width,
            height: window.height,
        }
    }

    #[cfg(any(windows, test))]
    #[must_use]
    fn center(self) -> (i32, i32) {
        let half_width = i32::try_from(self.width / 2).unwrap_or(i32::MAX);
        let half_height = i32::try_from(self.height / 2).unwrap_or(i32::MAX);
        (
            self.x.saturating_add(half_width),
            self.y.saturating_add(half_height),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct OverlayTurn {
    thread_id: String,
    turn_id: String,
    target: Option<ComputerUseOverlayTarget>,
}

pub struct ComputerUseSystemOverlay {
    active: Option<OverlayTurn>,
    #[cfg(windows)]
    renderer: windows::WindowsOverlayProcess,
}

impl ComputerUseSystemOverlay {
    pub fn new() -> io::Result<Self> {
        #[cfg(windows)]
        {
            Ok(Self {
                active: None,
                renderer: windows::WindowsOverlayProcess::new()?,
            })
        }

        #[cfg(not(windows))]
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Computer Use system overlay is not supported on this platform",
        ))
    }

    pub fn begin_turn(
        &mut self,
        thread_id: &str,
        turn_id: &str,
        target: Option<ComputerUseOverlayTarget>,
    ) -> io::Result<()> {
        validate_id(thread_id, "thread")?;
        validate_id(turn_id, "turn")?;
        let next = OverlayTurn {
            thread_id: thread_id.to_owned(),
            turn_id: turn_id.to_owned(),
            target,
        };
        if self.active.as_ref() == Some(&next) {
            return Ok(());
        }

        #[cfg(windows)]
        self.renderer.show(next.clone())?;

        self.active = Some(next);
        Ok(())
    }

    pub fn complete_turn(&mut self, thread_id: &str, turn_id: &str) -> io::Result<()> {
        if !self
            .active
            .as_ref()
            .is_some_and(|turn| turn.thread_id == thread_id && turn.turn_id == turn_id)
        {
            return Ok(());
        }
        self.hide()
    }

    pub fn hide(&mut self) -> io::Result<()> {
        if self.active.is_none() {
            return Ok(());
        }

        #[cfg(windows)]
        self.renderer.hide()?;

        self.active = None;
        Ok(())
    }

    #[must_use]
    pub fn active_turn(&self) -> Option<(&str, &str)> {
        self.active
            .as_ref()
            .map(|turn| (turn.thread_id.as_str(), turn.turn_id.as_str()))
    }
}

pub fn run_computer_use_overlay_helper() -> io::Result<()> {
    #[cfg(windows)]
    {
        windows::run_helper()
    }

    #[cfg(not(windows))]
    {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Computer Use system overlay is not supported on this platform",
        ))
    }
}

fn validate_id(value: &str, label: &str) -> io::Result<()> {
    if value.is_empty() || value.len() > MAX_OVERLAY_ID_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Computer Use overlay {label} id is invalid"),
        ));
    }
    Ok(())
}

#[cfg(windows)]
mod windows {
    use std::io::{self, BufRead, BufReader, Write};
    use std::os::windows::io::AsRawHandle;
    use std::os::windows::process::CommandExt;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::path::PathBuf;
    use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
    use std::sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{SyncSender, sync_channel},
    };
    use std::thread::{self, JoinHandle};
    use std::time::{Duration, Instant};

    use crossbeam_channel::{Receiver, Sender, bounded};
    use serde::{Deserialize, Serialize};
    use win32job::{ExtendedLimitInfo, Job};
    use winsafe::{
        self as w, gui,
        prelude::{
            GuiEvents, GuiEventsAll, GuiParent, GuiWindow, gdi_Hbrush, gdi_Hdc, gdi_Hfont,
            gdi_Hpen, user_Hdc, user_Hmonitor, user_Hwnd,
        },
    };

    use super::{ComputerUseOverlayTarget, OverlayTurn, validate_id};

    const READY_TIMEOUT: Duration = Duration::from_secs(2);
    const APPLY_TIMEOUT: Duration = Duration::from_secs(2);
    const HELPER_TIMEOUT: Duration = Duration::from_secs(4);
    const MAX_HELPER_FRAME_BYTES: usize = 8 * 1024;
    const HELPER_BINARY_NAME: &str = "codex-computer-use-overlay.exe";
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const TIMER_ID: usize = 1;
    const TIMER_INTERVAL_MS: u32 = 16;
    const INITIAL_TITLE: &str = "Codex Computer Use Display Overlay";
    const ACCESSIBLE_TITLE: &str = "ChatGPT is using your computer. Esc to cancel";
    const USING_COMPUTER: &str = "ChatGPT is using your computer";
    const ESC_TO_CANCEL: &str = "Esc to cancel";
    const DEFAULT_ACCENT: Rgb = Rgb::new(0x33, 0x9c, 0xff);

    #[derive(Debug, Serialize, Deserialize)]
    struct HelperRequest {
        id: u64,
        #[serde(flatten)]
        command: HelperCommand,
    }

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(tag = "method", rename_all = "snake_case")]
    enum HelperCommand {
        Ping,
        Show { turn: OverlayTurn },
        Hide,
        Shutdown,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct HelperResponse {
        id: u64,
        ok: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<HelperErrorCode>,
    }

    #[derive(Debug, Clone, Copy, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum HelperErrorCode {
        InvalidInput,
        Renderer,
    }

    pub(super) struct WindowsOverlayProcess {
        child: Child,
        stdin: ChildStdin,
        responses: Receiver<io::Result<Vec<u8>>>,
        reader: Option<JoinHandle<()>>,
        _job: Job,
        next_id: u64,
    }

    impl WindowsOverlayProcess {
        pub(super) fn new() -> io::Result<Self> {
            let executable = helper_executable_path()?;
            let mut command = Command::new(executable);
            command
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .creation_flags(CREATE_NO_WINDOW);

            let mut limits = ExtendedLimitInfo::new();
            limits.limit_kill_on_job_close();
            let job = Job::create_with_limit_info(&limits).map_err(|error| {
                io::Error::other(format!("overlay supervision failed: {error}"))
            })?;
            let mut child = command.spawn().map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!("could not start Computer Use overlay helper: {error}"),
                )
            })?;
            if let Err(error) = job.assign_process(child.as_raw_handle() as isize) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(io::Error::other(format!(
                    "could not supervise Computer Use overlay helper: {error}"
                )));
            }
            let stdin = child.stdin.take().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "Computer Use overlay helper stdin is unavailable",
                )
            })?;
            let stdout = child.stdout.take().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "Computer Use overlay helper stdout is unavailable",
                )
            })?;
            let (sender, responses) = bounded(2);
            let reader = thread::Builder::new()
                .name("codexrs-computer-use-overlay-helper".to_owned())
                .spawn(move || helper_response_reader(stdout, &sender))
                .inspect_err(|_| {
                    let _ = child.kill();
                    let _ = child.wait();
                })?;

            let mut process = Self {
                child,
                stdin,
                responses,
                reader: Some(reader),
                _job: job,
                next_id: 0,
            };
            process.exchange(HelperCommand::Ping)?;
            Ok(process)
        }

        pub(super) fn show(&mut self, turn: OverlayTurn) -> io::Result<()> {
            self.exchange(HelperCommand::Show { turn })
        }

        pub(super) fn hide(&mut self) -> io::Result<()> {
            self.exchange(HelperCommand::Hide)
        }

        fn exchange(&mut self, command: HelperCommand) -> io::Result<()> {
            self.next_id = self.next_id.wrapping_add(1).max(1);
            let request = HelperRequest {
                id: self.next_id,
                command,
            };
            let encoded = serde_json::to_vec(&request)
                .map_err(|_| io::Error::other("Computer Use overlay request is invalid"))?;
            if encoded.len() > MAX_HELPER_FRAME_BYTES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Computer Use overlay request exceeds its limit",
                ));
            }
            self.stdin
                .write_all(&encoded)
                .and_then(|()| self.stdin.write_all(b"\n"))
                .and_then(|()| self.stdin.flush())
                .map_err(|error| {
                    io::Error::new(
                        error.kind(),
                        format!("Computer Use overlay helper disconnected: {error}"),
                    )
                })?;
            let frame =
                self.responses
                    .recv_timeout(HELPER_TIMEOUT)
                    .map_err(|error| match error {
                        crossbeam_channel::RecvTimeoutError::Timeout => io::Error::new(
                            io::ErrorKind::TimedOut,
                            "Computer Use overlay helper timed out",
                        ),
                        crossbeam_channel::RecvTimeoutError::Disconnected => io::Error::new(
                            io::ErrorKind::BrokenPipe,
                            "Computer Use overlay helper disconnected",
                        ),
                    })??;
            let response = serde_json::from_slice::<HelperResponse>(&frame)
                .map_err(|_| io::Error::other("Computer Use overlay helper response is invalid"))?;
            if response.id != request.id {
                return Err(io::Error::other(
                    "Computer Use overlay helper response id does not match",
                ));
            }
            if response.ok {
                return Ok(());
            }
            Err(match response.error {
                Some(HelperErrorCode::InvalidInput) => io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Computer Use overlay helper rejected invalid input",
                ),
                Some(HelperErrorCode::Renderer) | None => {
                    io::Error::other("Computer Use overlay helper could not apply the indicator")
                }
            })
        }
    }

    impl Drop for WindowsOverlayProcess {
        fn drop(&mut self) {
            let _ = self.exchange(HelperCommand::Shutdown);
            let _ = self.child.kill();
            let _ = self.child.wait();
            if let Some(reader) = self.reader.take() {
                let _ = reader.join();
            }
        }
    }

    fn helper_executable_path() -> io::Result<PathBuf> {
        let current = std::env::current_exe()?;
        let parent = current.parent().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "Computer Use overlay helper directory is unavailable",
            )
        })?;
        let sibling = parent.join(HELPER_BINARY_NAME);
        if sibling.is_file() {
            return Ok(sibling);
        }
        if parent.file_name().is_some_and(|name| name == "deps")
            && let Some(debug_or_release) = parent.parent()
        {
            let cargo_sibling = debug_or_release.join(HELPER_BINARY_NAME);
            if cargo_sibling.is_file() {
                return Ok(cargo_sibling);
            }
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "codex-computer-use-overlay.exe is missing next to codexrs.exe",
        ))
    }

    fn helper_response_reader(stdout: ChildStdout, sender: &Sender<io::Result<Vec<u8>>>) {
        let mut reader = BufReader::new(stdout);
        loop {
            match read_bounded_line(&mut reader, MAX_HELPER_FRAME_BYTES) {
                Ok(Some(frame)) => {
                    if sender.send(Ok(frame)).is_err() {
                        return;
                    }
                }
                Ok(None) => {
                    let _ = sender.send(Err(io::Error::new(
                        io::ErrorKind::BrokenPipe,
                        "Computer Use overlay helper closed its output",
                    )));
                    return;
                }
                Err(error) => {
                    let _ = sender.send(Err(error));
                    return;
                }
            }
        }
    }

    #[derive(Debug, Clone)]
    struct OverlayFrame {
        turn: OverlayTurn,
        started_at: Instant,
    }

    #[derive(Default)]
    struct SharedState {
        desired: Option<OverlayFrame>,
        generation: u64,
        applied_generation: u64,
        error: Option<(u64, String)>,
        shutdown: bool,
    }

    #[derive(Default)]
    struct Shared {
        state: Mutex<SharedState>,
        applied: Condvar,
    }

    pub(super) struct WindowsOverlayRenderer {
        shared: Arc<Shared>,
        thread: Option<JoinHandle<()>>,
        closed: AtomicBool,
    }

    impl WindowsOverlayRenderer {
        pub(super) fn new() -> io::Result<Self> {
            let shared = Arc::new(Shared::default());
            let thread_shared = Arc::clone(&shared);
            let (ready_tx, ready_rx) = sync_channel(1);
            let ready_on_exit = ready_tx.clone();
            let thread = thread::Builder::new()
                .name("codexrs-computer-use-overlay".to_owned())
                .spawn(move || {
                    let result = catch_unwind(AssertUnwindSafe(|| {
                        run_overlay_window(thread_shared, ready_tx)
                    }));
                    let message = match result {
                        Ok(Ok(())) => return,
                        Ok(Err(error)) => error,
                        Err(payload) => panic_message(payload),
                    };
                    let _ = ready_on_exit.try_send(Err(message));
                })?;

            match ready_rx.recv_timeout(READY_TIMEOUT) {
                Ok(Ok(())) => Ok(Self {
                    shared,
                    thread: Some(thread),
                    closed: AtomicBool::new(false),
                }),
                Ok(Err(message)) => {
                    request_shutdown(&shared);
                    let _ = thread.join();
                    Err(io::Error::other(message))
                }
                Err(error) => {
                    request_shutdown(&shared);
                    if thread.is_finished() {
                        let _ = thread.join();
                    }
                    Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        format!("Computer Use overlay did not initialize: {error}"),
                    ))
                }
            }
        }

        pub(super) fn show(&self, turn: OverlayTurn) -> io::Result<()> {
            let started_at = self
                .shared
                .state
                .lock()
                .ok()
                .and_then(|state| {
                    state.desired.as_ref().and_then(|frame| {
                        (frame.turn.thread_id == turn.thread_id
                            && frame.turn.turn_id == turn.turn_id)
                            .then_some(frame.started_at)
                    })
                })
                .unwrap_or_else(Instant::now);
            self.request(Some(OverlayFrame { turn, started_at }))
        }

        pub(super) fn hide(&self) -> io::Result<()> {
            self.request(None)
        }

        fn request(&self, desired: Option<OverlayFrame>) -> io::Result<()> {
            let mut state = self
                .shared
                .state
                .lock()
                .map_err(|_| io::Error::other("Computer Use overlay state is unavailable"))?;
            if state.shutdown || self.closed.load(Ordering::Acquire) {
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "Computer Use overlay is closed",
                ));
            }
            state.generation = state.generation.wrapping_add(1).max(1);
            let generation = state.generation;
            state.desired = desired;
            state.error = None;

            let deadline = Instant::now() + APPLY_TIMEOUT;
            loop {
                if state.applied_generation >= generation {
                    if let Some((failed_generation, message)) = state.error.as_ref()
                        && *failed_generation == generation
                    {
                        return Err(io::Error::other(message.clone()));
                    }
                    return Ok(());
                }
                if state.shutdown {
                    return Err(io::Error::new(
                        io::ErrorKind::BrokenPipe,
                        "Computer Use overlay closed before applying the update",
                    ));
                }
                let now = Instant::now();
                if now >= deadline {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "Computer Use overlay did not apply the update in time",
                    ));
                }
                let wait = deadline.saturating_duration_since(now);
                let (next, timeout) =
                    self.shared.applied.wait_timeout(state, wait).map_err(|_| {
                        io::Error::other("Computer Use overlay state is unavailable")
                    })?;
                state = next;
                if timeout.timed_out() && state.applied_generation < generation {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "Computer Use overlay did not apply the update in time",
                    ));
                }
            }
        }
    }

    impl Drop for WindowsOverlayRenderer {
        fn drop(&mut self) {
            self.closed.store(true, Ordering::Release);
            request_shutdown(&self.shared);
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    pub(super) fn run_helper() -> io::Result<()> {
        let stdin = io::stdin();
        let stdout = io::stdout();
        let mut reader = stdin.lock();
        let mut writer = stdout.lock();
        let mut renderer = Some(WindowsOverlayRenderer::new()?);

        while let Some(frame) = read_bounded_line(&mut reader, MAX_HELPER_FRAME_BYTES)? {
            let request = serde_json::from_slice::<HelperRequest>(&frame).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Computer Use overlay helper request is invalid",
                )
            })?;
            let mut shutdown = false;
            let result = match request.command {
                HelperCommand::Ping => Ok(()),
                HelperCommand::Show { turn } => validate_id(&turn.thread_id, "thread")
                    .and_then(|()| validate_id(&turn.turn_id, "turn"))
                    .and_then(|()| {
                        renderer.as_ref().ok_or_else(|| {
                            io::Error::new(
                                io::ErrorKind::BrokenPipe,
                                "Computer Use overlay renderer is closed",
                            )
                        })
                    })
                    .and_then(|renderer| renderer.show(turn)),
                HelperCommand::Hide => renderer
                    .as_ref()
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::BrokenPipe,
                            "Computer Use overlay renderer is closed",
                        )
                    })
                    .and_then(WindowsOverlayRenderer::hide),
                HelperCommand::Shutdown => {
                    shutdown = true;
                    let result = renderer
                        .as_ref()
                        .map_or(Ok(()), WindowsOverlayRenderer::hide);
                    drop(renderer.take());
                    result
                }
            };
            let response = HelperResponse {
                id: request.id,
                ok: result.is_ok(),
                error: result.as_ref().err().map(|error| {
                    if error.kind() == io::ErrorKind::InvalidInput {
                        HelperErrorCode::InvalidInput
                    } else {
                        HelperErrorCode::Renderer
                    }
                }),
            };
            let encoded = serde_json::to_vec(&response)
                .map_err(|_| io::Error::other("Computer Use overlay response is invalid"))?;
            if encoded.len() > MAX_HELPER_FRAME_BYTES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Computer Use overlay response exceeds its limit",
                ));
            }
            writer.write_all(&encoded)?;
            writer.write_all(b"\n")?;
            writer.flush()?;
            if shutdown {
                return Ok(());
            }
        }
        Ok(())
    }

    fn read_bounded_line(reader: &mut impl BufRead, limit: usize) -> io::Result<Option<Vec<u8>>> {
        let mut line = Vec::with_capacity(limit.min(1024));
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
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "bounded Computer Use overlay frame exceeded",
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

    fn request_shutdown(shared: &Shared) {
        if let Ok(mut state) = shared.state.lock() {
            state.shutdown = true;
            state.generation = state.generation.wrapping_add(1).max(1);
            shared.applied.notify_all();
        }
    }

    fn run_overlay_window(
        shared: Arc<Shared>,
        ready: SyncSender<Result<(), String>>,
    ) -> Result<(), String> {
        let window = gui::WindowMain::new(gui::WindowMainOpts {
            class_name: "CodexComputerUseCursorOverlay".to_owned(),
            class_style: w::co::CS::DBLCLKS,
            class_icon: gui::Icon::None,
            class_cursor: gui::Cursor::None,
            class_bg_brush: gui::Brush::None,
            title: INITIAL_TITLE.to_owned(),
            size: (1, 1),
            style: w::co::WS::POPUP | w::co::WS::CLIPSIBLINGS,
            ex_style: w::co::WS_EX::NOACTIVATE
                | w::co::WS_EX::LAYERED
                | w::co::WS_EX::TOOLWINDOW
                | w::co::WS_EX::TRANSPARENT
                | w::co::WS_EX::TOPMOST,
            ..Default::default()
        });

        let create_window = window.clone();
        let create_ready = ready.clone();
        window.on().wm_create(move |_| {
            let result = configure_overlay_window(&create_window);
            let response = result.as_ref().map(|_| ()).map_err(ToString::to_string);
            let _ = create_ready.try_send(response);
            Ok(if result.is_ok() { 0 } else { -1 })
        });

        let timer_window = window.clone();
        let timer_shared = Arc::clone(&shared);
        window.on().wm_timer(TIMER_ID, move || {
            apply_requested_state(&timer_window, &timer_shared);
            Ok(())
        });

        let paint_window = window.clone();
        let paint_shared = Arc::clone(&shared);
        window.on().wm_paint(move || {
            paint_overlay(&paint_window, &paint_shared);
            Ok(())
        });

        window.on().wm_erase_bkgnd(|_| Ok(1));
        window.on().wm_nc_hit_test(|_| Ok(w::co::HT::TRANSPARENT));

        let display_window = window.clone();
        let display_shared = Arc::clone(&shared);
        window.on().wm_display_change(move |_| {
            if desired_frame(&display_shared).is_some() {
                let _ = position_overlay_window(&display_window, true);
                let _ = display_window.hwnd().InvalidateRect(None, true);
                let _ = display_window.hwnd().UpdateWindow();
            }
            Ok(())
        });

        window
            .run_main(Some(w::co::SW::HIDE))
            .map(|_| ())
            .map_err(|error| format!("Computer Use overlay window failed: {error}"))
    }

    fn configure_overlay_window(window: &gui::WindowMain) -> Result<(), w::co::ERROR> {
        window.hwnd().SetLayeredWindowAttributes(
            w::COLORREF::new(0, 0, 0),
            u8::MAX,
            w::co::LWA::COLORKEY,
        )?;
        window
            .hwnd()
            .SetWindowDisplayAffinity(w::co::WDA::EXCLUDEFROMCAPTURE)?;
        position_overlay_window(window, false)?;
        window.hwnd().SetTimer(TIMER_ID, TIMER_INTERVAL_MS, None)?;
        Ok(())
    }

    fn apply_requested_state(window: &gui::WindowMain, shared: &Shared) {
        let Some((generation, desired, shutdown)) = shared.state.lock().ok().and_then(|state| {
            (state.applied_generation < state.generation)
                .then(|| (state.generation, state.desired.clone(), state.shutdown))
        }) else {
            if desired_frame(shared).is_some() {
                let _ = window.hwnd().InvalidateRect(None, false);
            }
            return;
        };

        if shutdown {
            let _ = window.hwnd().DestroyWindow();
            return;
        }

        let result = if desired.is_some() {
            show_overlay_window(window)
        } else {
            hide_overlay_window(window)
        };
        if let Ok(mut state) = shared.state.lock() {
            state.applied_generation = generation;
            state.error = result
                .as_ref()
                .err()
                .map(|error| (generation, error.to_string()));
            shared.applied.notify_all();
        }
    }

    fn show_overlay_window(window: &gui::WindowMain) -> Result<(), w::co::ERROR> {
        window.hwnd().SetWindowText(ACCESSIBLE_TITLE)?;
        position_overlay_window(window, true)?;
        window.hwnd().InvalidateRect(None, true)?;
        window.hwnd().UpdateWindow()
    }

    fn hide_overlay_window(window: &gui::WindowMain) -> Result<(), w::co::ERROR> {
        window.hwnd().ShowWindow(w::co::SW::HIDE);
        window.hwnd().SetWindowText(INITIAL_TITLE)
    }

    fn position_overlay_window(window: &gui::WindowMain, show: bool) -> Result<(), w::co::ERROR> {
        let bounds = virtual_desktop_bounds();
        let mut flags = w::co::SWP::NOACTIVATE | w::co::SWP::NOOWNERZORDER;
        if show {
            flags |= w::co::SWP::SHOWWINDOW;
        }
        window.hwnd().SetWindowPos(
            w::HwndPlace::Place(w::co::HWND_PLACE::TOPMOST),
            w::POINT::new(bounds.left, bounds.top),
            w::SIZE::new(
                bounds.right.saturating_sub(bounds.left),
                bounds.bottom.saturating_sub(bounds.top),
            ),
            flags,
        )
    }

    fn virtual_desktop_bounds() -> w::RECT {
        let left = w::GetSystemMetrics(w::co::SM::XVIRTUALSCREEN);
        let top = w::GetSystemMetrics(w::co::SM::YVIRTUALSCREEN);
        let width = w::GetSystemMetrics(w::co::SM::CXVIRTUALSCREEN).max(1);
        let height = w::GetSystemMetrics(w::co::SM::CYVIRTUALSCREEN).max(1);
        w::RECT {
            left,
            top,
            right: left.saturating_add(width),
            bottom: top.saturating_add(height),
        }
    }

    fn desired_frame(shared: &Shared) -> Option<OverlayFrame> {
        shared
            .state
            .lock()
            .ok()
            .and_then(|state| state.desired.clone())
    }

    fn paint_overlay(window: &gui::WindowMain, shared: &Shared) {
        let Ok(hdc) = window.hwnd().BeginPaint() else {
            return;
        };
        let Ok(client) = window.hwnd().GetClientRect() else {
            return;
        };
        let Ok(clear) = w::HBRUSH::CreateSolidBrush(w::COLORREF::new(0, 0, 0)) else {
            return;
        };
        if hdc.FillRect(client, &clear).is_err() {
            return;
        }
        let Some(frame) = desired_frame(shared) else {
            return;
        };
        let _ = draw_overlay(&hdc, client, &frame);
    }

    fn draw_overlay(
        hdc: &w::HDC,
        client: w::RECT,
        frame: &OverlayFrame,
    ) -> Result<(), w::co::ERROR> {
        let monitor = target_monitor(frame.turn.target);
        let virtual_bounds = virtual_desktop_bounds();
        let monitor_local = w::RECT {
            left: monitor.left.saturating_sub(virtual_bounds.left),
            top: monitor.top.saturating_sub(virtual_bounds.top),
            right: monitor.right.saturating_sub(virtual_bounds.left),
            bottom: monitor.bottom.saturating_sub(virtual_bounds.top),
        };
        let scale = target_scale(frame.turn.target).clamp(0.5, 4.0);
        draw_edge_pulse(hdc, client, frame.started_at.elapsed())?;
        draw_status_pill(hdc, monitor_local, scale)
    }

    fn target_monitor(target: Option<ComputerUseOverlayTarget>) -> w::RECT {
        let point = target.map_or_else(
            || {
                w::POINT::new(
                    w::GetSystemMetrics(w::co::SM::CXSCREEN) / 2,
                    w::GetSystemMetrics(w::co::SM::CYSCREEN) / 2,
                )
            },
            |target| {
                let (x, y) = target.center();
                w::POINT::new(x, y)
            },
        );
        let monitor = w::HMONITOR::MonitorFromPoint(point, w::co::MONITOR::DEFAULTTOPRIMARY);
        let mut info = w::MONITORINFOEX::default();
        if monitor.GetMonitorInfo(&mut info).is_ok() {
            info.rcMonitor
        } else {
            w::RECT {
                left: 0,
                top: 0,
                right: w::GetSystemMetrics(w::co::SM::CXSCREEN).max(1),
                bottom: w::GetSystemMetrics(w::co::SM::CYSCREEN).max(1),
            }
        }
    }

    fn target_scale(target: Option<ComputerUseOverlayTarget>) -> f32 {
        let (x, y) = target.map_or_else(
            || {
                (
                    w::GetSystemMetrics(w::co::SM::CXSCREEN) / 2,
                    w::GetSystemMetrics(w::co::SM::CYSCREEN) / 2,
                )
            },
            ComputerUseOverlayTarget::center,
        );
        xcap::Monitor::from_point(x, y)
            .and_then(|monitor| monitor.scale_factor())
            .ok()
            .filter(|scale| scale.is_finite() && *scale > 0.0)
            .unwrap_or(1.0)
    }

    fn draw_edge_pulse(
        hdc: &w::HDC,
        client: w::RECT,
        elapsed: Duration,
    ) -> Result<(), w::co::ERROR> {
        let opacity = pulse_opacity(elapsed);
        let stops = [(1_i32, 0.58_f32), (2, 0.36), (3, 0.12)];
        for (inset, alpha) in stops {
            let color = DEFAULT_ACCENT.scale(opacity * alpha);
            let brush = w::HBRUSH::CreateSolidBrush(color.into())?;
            let left = client.left.saturating_add(inset - 1);
            let top = client.top.saturating_add(inset - 1);
            let right = client.right.saturating_sub(inset - 1);
            let bottom = client.bottom.saturating_sub(inset - 1);
            if right <= left || bottom <= top {
                continue;
            }
            hdc.FillRect(
                w::RECT {
                    left,
                    top,
                    right,
                    bottom: top.saturating_add(1),
                },
                &brush,
            )?;
            hdc.FillRect(
                w::RECT {
                    left,
                    top: bottom.saturating_sub(1),
                    right,
                    bottom,
                },
                &brush,
            )?;
            hdc.FillRect(
                w::RECT {
                    left,
                    top,
                    right: left.saturating_add(1),
                    bottom,
                },
                &brush,
            )?;
            hdc.FillRect(
                w::RECT {
                    left: right.saturating_sub(1),
                    top,
                    right,
                    bottom,
                },
                &brush,
            )?;
        }
        Ok(())
    }

    fn pulse_opacity(elapsed: Duration) -> f32 {
        let phase = (elapsed.as_secs_f32() % 3.0) / 3.0;
        if phase < 0.5 {
            0.4 + (phase / 0.5) * (0.88 - 0.4)
        } else {
            0.88 - ((phase - 0.5) / 0.5) * (0.88 - 0.76)
        }
    }

    fn draw_status_pill(hdc: &w::HDC, monitor: w::RECT, scale: f32) -> Result<(), w::co::ERROR> {
        let pill_height = scaled(32.0, scale).max(24);
        let top = monitor.top.saturating_add(scaled(56.0, scale));
        let horizontal_padding = scaled(12.0, scale);
        let item_gap = scaled(12.0, scale);
        let separator_width = scaled(1.0, scale).max(1);
        let separator_height = scaled(18.0, scale);
        let radius = scaled(8.0, scale);
        let main_font = w::HFONT::CreateFont(
            w::SIZE::new(0, -scaled(14.0, scale)),
            0,
            0,
            w::co::FW::SEMIBOLD,
            false,
            false,
            false,
            w::co::CHARSET::DEFAULT,
            w::co::OUT_PRECIS::DEFAULT,
            w::co::CLIP::DEFAULT_PRECIS,
            w::co::QUALITY::CLEARTYPE_NATURAL,
            w::co::PITCH::VARIABLE,
            "Segoe UI Variable",
        )?;
        let secondary_font = w::HFONT::CreateFont(
            w::SIZE::new(0, -scaled(12.0, scale)),
            0,
            0,
            w::co::FW::NORMAL,
            false,
            false,
            false,
            w::co::CHARSET::DEFAULT,
            w::co::OUT_PRECIS::DEFAULT,
            w::co::CLIP::DEFAULT_PRECIS,
            w::co::QUALITY::CLEARTYPE_NATURAL,
            w::co::PITCH::VARIABLE,
            "Segoe UI Variable",
        )?;
        let main_width = {
            let _selected = hdc.SelectObject(&*main_font)?;
            hdc.GetTextExtentPoint32(USING_COMPUTER)?.cx
        };
        let secondary_width = {
            let _selected = hdc.SelectObject(&*secondary_font)?;
            hdc.GetTextExtentPoint32(ESC_TO_CANCEL)?.cx
        };
        let content_width = main_width
            .saturating_add(item_gap)
            .saturating_add(separator_width)
            .saturating_add(item_gap)
            .saturating_add(secondary_width);
        let max_width = scaled(920.0, scale);
        let pill_width = content_width
            .saturating_add(horizontal_padding.saturating_mul(2))
            .min(max_width)
            .max(pill_height);
        let monitor_width = monitor.right.saturating_sub(monitor.left);
        let left = monitor
            .left
            .saturating_add((monitor_width.saturating_sub(pill_width)) / 2);
        let pill = w::RECT {
            left,
            top,
            right: left.saturating_add(pill_width),
            bottom: top.saturating_add(pill_height),
        };

        let accent_brush = w::HBRUSH::CreateSolidBrush(DEFAULT_ACCENT.into())?;
        let accent_pen = w::HPEN::CreatePen(w::co::PS::SOLID, 1, DEFAULT_ACCENT.into())?;
        let _brush = hdc.SelectObject(&*accent_brush)?;
        let _pen = hdc.SelectObject(&*accent_pen)?;
        hdc.RoundRect(
            pill,
            w::SIZE::new(radius.saturating_mul(2), radius.saturating_mul(2)),
        )?;

        let separator_left = left
            .saturating_add(horizontal_padding)
            .saturating_add(main_width)
            .saturating_add(item_gap);
        let separator_top = top.saturating_add((pill_height - separator_height) / 2);
        let separator = w::RECT {
            left: separator_left,
            top: separator_top,
            right: separator_left.saturating_add(separator_width),
            bottom: separator_top.saturating_add(separator_height),
        };
        let separator_color = DEFAULT_ACCENT.mix(Rgb::new(0xff, 0xff, 0xff), 0.36);
        let separator_brush = w::HBRUSH::CreateSolidBrush(separator_color.into())?;
        hdc.FillRect(separator, &separator_brush)?;

        let _ = hdc.SetBkMode(w::co::BKMODE::TRANSPARENT)?;
        let _ = hdc.SetTextColor(w::COLORREF::new(0xff, 0xff, 0xff))?;
        let text_flags = w::co::DT::SINGLELINE | w::co::DT::VCENTER | w::co::DT::NOPREFIX;
        {
            let _selected = hdc.SelectObject(&*main_font)?;
            hdc.DrawText(
                USING_COMPUTER,
                &w::RECT {
                    left: left.saturating_add(horizontal_padding),
                    top,
                    right: separator_left.saturating_sub(item_gap),
                    bottom: top.saturating_add(pill_height),
                },
                text_flags,
            )?;
        }
        {
            let _selected = hdc.SelectObject(&*secondary_font)?;
            hdc.DrawText(
                ESC_TO_CANCEL,
                &w::RECT {
                    left: separator.right.saturating_add(item_gap),
                    top,
                    right: pill.right.saturating_sub(horizontal_padding),
                    bottom: top.saturating_add(pill_height),
                },
                text_flags,
            )?;
        }
        Ok(())
    }

    fn scaled(value: f32, scale: f32) -> i32 {
        let value = (value * scale).round();
        if !value.is_finite() {
            return 0;
        }
        value.clamp(i32::MIN as f32, i32::MAX as f32) as i32
    }

    #[derive(Debug, Clone, Copy)]
    struct Rgb {
        red: u8,
        green: u8,
        blue: u8,
    }

    impl Rgb {
        const fn new(red: u8, green: u8, blue: u8) -> Self {
            Self { red, green, blue }
        }

        fn scale(self, alpha: f32) -> Self {
            let alpha = alpha.clamp(0.0, 1.0);
            Self::new(
                (f32::from(self.red) * alpha).round() as u8,
                (f32::from(self.green) * alpha).round() as u8,
                (f32::from(self.blue) * alpha).round() as u8,
            )
        }

        fn mix(self, other: Self, amount: f32) -> Self {
            let amount = amount.clamp(0.0, 1.0);
            let mix = |left: u8, right: u8| {
                (f32::from(left) + (f32::from(right) - f32::from(left)) * amount).round() as u8
            };
            Self::new(
                mix(self.red, other.red),
                mix(self.green, other.green),
                mix(self.blue, other.blue),
            )
        }
    }

    impl From<Rgb> for w::COLORREF {
        fn from(color: Rgb) -> Self {
            Self::new(color.red, color.green, color.blue)
        }
    }

    fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
        payload
            .downcast_ref::<&str>()
            .map(|message| (*message).to_owned())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "Computer Use overlay thread panicked".to_owned())
    }
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    use super::ComputerUseSystemOverlay;
    use super::{ComputerUseOverlayTarget, OverlayTurn, validate_id};

    #[test]
    fn overlay_target_uses_the_window_center_without_overflow() {
        let target = ComputerUseOverlayTarget {
            x: i32::MAX - 1,
            y: i32::MIN + 1,
            width: u32::MAX,
            height: u32::MAX,
        };
        assert_eq!(target.center(), (i32::MAX, 0));
    }

    #[test]
    fn overlay_turn_identity_ignores_neither_target_nor_turn() {
        let first = OverlayTurn {
            thread_id: "thread-1".to_owned(),
            turn_id: "turn-1".to_owned(),
            target: None,
        };
        let mut changed = first.clone();
        changed.target = Some(ComputerUseOverlayTarget {
            x: 0,
            y: 0,
            width: 800,
            height: 600,
        });
        assert_ne!(first, changed);
        changed.target = None;
        changed.turn_id = "turn-2".to_owned();
        assert_ne!(first, changed);
    }

    #[test]
    fn overlay_ids_are_bounded() {
        assert!(validate_id("turn-1", "turn").is_ok());
        assert!(validate_id("", "turn").is_err());
        assert!(validate_id(&"x".repeat(1_025), "turn").is_err());
    }

    #[cfg(not(windows))]
    #[test]
    fn non_windows_overlay_constructor_is_unsupported() -> std::io::Result<()> {
        let error = match super::ComputerUseSystemOverlay::new() {
            Err(error) => error,
            Ok(_) => {
                return Err(std::io::Error::other(
                    "non-Windows platforms must not report a system overlay as ready",
                ));
            }
        };
        assert_eq!(error.kind(), std::io::ErrorKind::Unsupported);
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "requires an interactive Windows desktop"]
    fn windows_overlay_show_hide_smoke() -> Result<(), Box<dyn std::error::Error>> {
        let _ = winsafe::SetProcessDPIAware();
        let mut overlay = ComputerUseSystemOverlay::new()?;
        overlay.begin_turn("thread-smoke", "turn-smoke", None)?;
        assert_eq!(overlay.active_turn(), Some(("thread-smoke", "turn-smoke")));
        overlay.complete_turn("thread-smoke", "other-turn")?;
        assert!(overlay.active_turn().is_some());
        overlay.complete_turn("thread-smoke", "turn-smoke")?;
        assert!(overlay.active_turn().is_none());
        Ok(())
    }
}
