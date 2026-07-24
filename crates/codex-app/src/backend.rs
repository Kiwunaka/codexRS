use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use codex_core::{
    Action, ApprovalDecision, ApprovalKind, ApprovalRequest, ComputerWindowState, Effect,
    GitBranchState, GitFileKind as CoreGitFileKind, GitFileState, GitState, GitWorktreeState,
    InspectorPane, MainRoute, PluginCard, TaskRunStatus, TaskSummary, TimelineItem, TimelineKind,
};
use codex_platform::{
    AppServerConfig, AppServerConnection, AppServerEvent, CodexHome, CodexHomeKind, ComputerButton,
    ComputerCapture, ComputerKey, ComputerWindow, DEFAULT_THREAD_PAGE_LIMIT, GitError,
    GitFileKind as PlatformGitFileKind, GitSnapshot, RuntimePolicy, TerminalConfig, TerminalEvent,
    TerminalSession, capture_computer_window, click_computer_window, codexrs_data_dir,
    create_worktree as git_create_worktree, git_diff, git_snapshot, git_stage, git_unstage,
    inspect_computer_window, list_computer_windows, move_over_computer_window, press_computer_key,
    resolve_codex_binary, scroll_computer_window, switch_branch as git_switch_branch,
    type_into_computer_window,
};
use codex_protocol::{
    ClientInfo, DynamicToolCallOutputContentItem, DynamicToolCallParams, DynamicToolCallResponse,
    DynamicToolFunction, DynamicToolNamespaceTool, DynamicToolSpec, HistorySortDirection,
    InitializeCapabilities, PluginInstallParams, PluginListParams, PluginUninstallParams,
    ThreadForkParams, ThreadItemsListParams, ThreadListParams, ThreadResumeInitialTurnsPageParams,
    ThreadResumeParams, ThreadStartParams, TurnStartParams, UserInput,
};
use codex_storage::Store;
use crossbeam_channel::{Receiver, Sender, TryRecvError};
use serde_json::{Value, json};

const BACKEND_COMMAND_CAPACITY: usize = 64;
const BACKEND_EVENT_CAPACITY: usize = 1_024;
const BACKEND_TICK: Duration = Duration::from_millis(25);
const UI_EVENT_TIMEOUT: Duration = Duration::from_millis(100);
const HISTORY_PAGE_LIMIT: u32 = 100;
const MAX_ITEM_TEXT_BYTES: usize = 256 * 1024;
const MAX_STATUS_BYTES: usize = 16 * 1024;

enum BackendCommand {
    Run(Effect),
    Shutdown,
}

struct PendingApproval {
    id: Value,
    method: String,
    params: Value,
}

#[derive(Debug, Default)]
struct GitRefreshDebouncer {
    pending: Option<(PathBuf, Instant)>,
}

impl GitRefreshDebouncer {
    fn schedule(&mut self, cwd: PathBuf, now: Instant, delay: Duration) {
        self.pending = Some((cwd, now + delay));
    }

    fn take_due(&mut self, now: Instant) -> Option<PathBuf> {
        if self
            .pending
            .as_ref()
            .is_some_and(|(_, deadline)| now >= *deadline)
        {
            return self.pending.take().map(|(cwd, _)| cwd);
        }
        None
    }
}

#[derive(Debug, Clone, Default)]
struct ComputerUsePermission {
    enabled: bool,
    selected_window_id: Option<String>,
    input_authorized: bool,
}

pub struct Backend {
    commands: Sender<BackendCommand>,
    events: Receiver<Action>,
    thread: Option<JoinHandle<()>>,
}

impl Backend {
    pub fn spawn() -> Result<Self, String> {
        let (command_sender, command_receiver) =
            crossbeam_channel::bounded(BACKEND_COMMAND_CAPACITY);
        let (event_sender, event_receiver) = crossbeam_channel::bounded(BACKEND_EVENT_CAPACITY);
        let thread = thread::Builder::new()
            .name("codex-rs-backend".to_owned())
            .spawn(move || run_backend(command_receiver, event_sender))
            .map_err(|error| format!("failed to start backend: {error}"))?;

        Ok(Self {
            commands: command_sender,
            events: event_receiver,
            thread: Some(thread),
        })
    }

    pub fn send(&self, effect: Effect) -> Result<(), &'static str> {
        self.commands
            .try_send(BackendCommand::Run(effect))
            .map_err(|error| match error {
                crossbeam_channel::TrySendError::Full(_) => "backend command queue is full",
                crossbeam_channel::TrySendError::Disconnected(_) => "backend is disconnected",
            })
    }

    pub fn try_recv(&self) -> Result<Option<Action>, &'static str> {
        match self.events.try_recv() {
            Ok(action) => Ok(Some(action)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err("backend is disconnected"),
        }
    }
}

impl Drop for Backend {
    fn drop(&mut self) {
        let _ = self.commands.try_send(BackendCommand::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn open_storage(events: &Sender<Action>) -> Option<Store> {
    let path = match codexrs_data_dir() {
        Ok(directory) => directory.join("state.sqlite3"),
        Err(error) => {
            emit(events, Action::StorageFailed(error.to_string()));
            return None;
        }
    };
    match Store::open(&path).and_then(|store| {
        let route = store.preference("route")?;
        let inspector = store.preference("inspector")?;
        Ok((store, route, inspector))
    }) {
        Ok((store, route, inspector)) => {
            emit(
                events,
                Action::StorageOpened {
                    path,
                    route: route.as_deref().and_then(parse_route),
                    inspector: inspector.as_deref().and_then(parse_inspector),
                },
            );
            Some(store)
        }
        Err(error) => {
            emit(events, Action::StorageFailed(error.to_string()));
            None
        }
    }
}

const fn route_key(route: MainRoute) -> &'static str {
    match route {
        MainRoute::Tasks => "tasks",
        MainRoute::Repository => "repository",
        MainRoute::Marketplace => "marketplace",
        MainRoute::Settings => "settings",
    }
}

fn parse_route(value: &str) -> Option<MainRoute> {
    match value {
        "tasks" => Some(MainRoute::Tasks),
        "repository" => Some(MainRoute::Repository),
        "marketplace" => Some(MainRoute::Marketplace),
        "settings" => Some(MainRoute::Settings),
        _ => None,
    }
}

const fn inspector_key(inspector: InspectorPane) -> &'static str {
    match inspector {
        InspectorPane::Changes => "changes",
        InspectorPane::Terminal => "terminal",
        InspectorPane::ComputerUse => "computer-use",
    }
}

fn parse_inspector(value: &str) -> Option<InspectorPane> {
    match value {
        "changes" => Some(InspectorPane::Changes),
        "terminal" => Some(InspectorPane::Terminal),
        "computer-use" => Some(InspectorPane::ComputerUse),
        _ => None,
    }
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or_default()
}

fn run_backend(commands: Receiver<BackendCommand>, events: Sender<Action>) {
    let runtime_policy = RuntimePolicy::default();
    let mut storage = open_storage(&events);
    let mut connection: Option<AppServerConnection> = None;
    let mut pending_approvals = HashMap::new();
    let mut marketplaces = HashMap::new();
    let mut computer_permissions = HashMap::new();
    let mut computer_capable_threads = HashSet::new();
    let mut terminal = None;
    let mut terminal_parser = None;
    let mut terminal_truncation_reported = false;
    let mut git_refresh = GitRefreshDebouncer::default();

    loop {
        let mut disconnected = false;
        let mut filesystem_changed = false;
        if let Some(app_server) = connection.as_ref() {
            for _ in 0..64 {
                match app_server.try_recv_event() {
                    Ok(Some(AppServerEvent::Disconnected)) => {
                        emit(&events, Action::ConnectionLost);
                        disconnected = true;
                        break;
                    }
                    Ok(Some(event)) => {
                        filesystem_changed |= handle_app_server_event(
                            app_server,
                            event,
                            &events,
                            &mut pending_approvals,
                            &computer_permissions,
                        )
                    }
                    Ok(None) => break,
                    Err(error) => {
                        emit(
                            &events,
                            Action::SetStatus(format!("app-server event error: {error}")),
                        );
                        break;
                    }
                }
            }
        }
        if filesystem_changed {
            emit(&events, Action::RefreshGit);
        }
        if disconnected {
            connection.take();
            pending_approvals.clear();
        }
        drain_terminal(
            &mut terminal,
            &mut terminal_parser,
            &mut terminal_truncation_reported,
            &events,
        );

        match commands.recv_timeout(BACKEND_TICK) {
            Ok(BackendCommand::Run(Effect::RefreshGit { cwd })) => {
                git_refresh.schedule(cwd, Instant::now(), runtime_policy.git_debounce);
            }
            Ok(BackendCommand::Run(effect)) => {
                run_effect(
                    effect,
                    &events,
                    &mut connection,
                    &mut pending_approvals,
                    &mut marketplaces,
                    &mut computer_permissions,
                    &mut computer_capable_threads,
                    &mut storage,
                    &mut terminal,
                    &mut terminal_parser,
                    &mut terminal_truncation_reported,
                );
            }
            Ok(BackendCommand::Shutdown)
            | Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                break;
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
        }

        if let Some(cwd) = git_refresh.take_due(Instant::now()) {
            refresh_git(&cwd, &events);
        }
    }

    if let Some(mut app_server) = connection {
        let _ = app_server.shutdown();
    }
    if let Some(mut terminal) = terminal {
        terminal.shutdown();
    }
}

#[allow(clippy::too_many_arguments)]
fn run_effect(
    effect: Effect,
    events: &Sender<Action>,
    connection: &mut Option<AppServerConnection>,
    pending_approvals: &mut HashMap<String, PendingApproval>,
    marketplaces: &mut HashMap<String, Option<PathBuf>>,
    computer_permissions: &mut HashMap<String, ComputerUsePermission>,
    computer_capable_threads: &mut HashSet<String>,
    storage: &mut Option<Store>,
    terminal: &mut Option<TerminalSession>,
    terminal_parser: &mut Option<vt100::Parser>,
    terminal_truncation_reported: &mut bool,
) {
    if effect == Effect::ConnectAppServer {
        connect(events, connection);
        return;
    }

    match &effect {
        Effect::PersistUiState { route, inspector } => {
            let result = storage.as_mut().map_or(Ok(()), |store| {
                let now = unix_timestamp();
                store.set_preference("route", route_key(*route), now)?;
                store.set_preference("inspector", inspector_key(*inspector), now)
            });
            if let Err(error) = result {
                storage.take();
                emit(events, Action::StorageFailed(error.to_string()));
            }
            return;
        }
        Effect::RememberWorkspace { path } => {
            let result = storage.as_mut().map_or(Ok(()), |store| {
                store.remember_workspace(path, unix_timestamp())
            });
            if let Err(error) = result {
                storage.take();
                emit(events, Action::StorageFailed(error.to_string()));
            }
            return;
        }
        Effect::RefreshGit { cwd } => {
            refresh_git(cwd, events);
            return;
        }
        Effect::LoadDiff {
            generation,
            root,
            path,
        } => {
            match git_diff(root, path) {
                Ok(diff) => emit(
                    events,
                    Action::DiffLoaded {
                        generation: *generation,
                        text: diff.text,
                        truncated: diff.truncated,
                    },
                ),
                Err(error) => {
                    emit(
                        events,
                        Action::DiffLoaded {
                            generation: *generation,
                            text: String::new(),
                            truncated: false,
                        },
                    );
                    emit(
                        events,
                        Action::SetStatus(format!("failed to load diff: {error}")),
                    );
                }
            }
            return;
        }
        Effect::StagePath { root, path } => {
            match git_stage(root, path) {
                Ok(()) => emit(events, Action::RefreshGit),
                Err(error) => emit(
                    events,
                    Action::SetStatus(format!("failed to stage path: {error}")),
                ),
            }
            return;
        }
        Effect::UnstagePath { root, path } => {
            match git_unstage(root, path) {
                Ok(()) => emit(events, Action::RefreshGit),
                Err(error) => emit(
                    events,
                    Action::SetStatus(format!("failed to unstage path: {error}")),
                ),
            }
            return;
        }
        Effect::SwitchGitBranch { root, branch } => {
            match git_switch_branch(root, branch) {
                Ok(()) => {
                    emit(
                        events,
                        Action::SetStatus(format!("switched to branch {branch}")),
                    );
                    emit(events, Action::RefreshGit);
                }
                Err(error) => emit(
                    events,
                    Action::SetStatus(format!("failed to switch branch: {error}")),
                ),
            }
            return;
        }
        Effect::CreateGitWorktree {
            root,
            path,
            branch,
            create_branch,
        } => {
            match git_create_worktree(root, path, branch, *create_branch) {
                Ok(()) => {
                    emit(
                        events,
                        Action::SetStatus(format!("worktree created for {branch}")),
                    );
                    emit(events, Action::RefreshGit);
                }
                Err(error) => emit(
                    events,
                    Action::SetStatus(format!("failed to create worktree: {error}")),
                ),
            }
            return;
        }
        Effect::SpawnTerminal { cwd } => {
            match TerminalSession::spawn(TerminalConfig::new(cwd.clone())) {
                Ok(session) => {
                    let process_id = session
                        .process_id()
                        .map_or_else(|| "terminal".to_owned(), |id| id.to_string());
                    *terminal_parser = Some(vt100::Parser::new(24, 80, 2_000));
                    *terminal_truncation_reported = false;
                    *terminal = Some(session);
                    emit(
                        events,
                        Action::TerminalStarted {
                            process_id,
                            title: cwd.display().to_string(),
                        },
                    );
                }
                Err(error) => emit(
                    events,
                    Action::SetStatus(format!("failed to open terminal: {error}")),
                ),
            }
            return;
        }
        Effect::WriteTerminal { input } => {
            if let Some(session) = terminal.as_ref() {
                let mut bytes = input.as_bytes().to_vec();
                bytes.push(b'\r');
                if let Err(error) = session.write(&bytes) {
                    emit(
                        events,
                        Action::SetStatus(format!("terminal input failed: {error}")),
                    );
                }
            }
            return;
        }
        Effect::StopTerminal => {
            if let Some(mut session) = terminal.take() {
                session.shutdown();
            }
            terminal_parser.take();
            emit(events, Action::TerminalExited { code: 0 });
            return;
        }
        Effect::ConfigureComputerUse {
            task_id,
            enabled,
            selected_window_id,
            input_authorized,
        } => {
            if computer_capable_threads.contains(task_id) {
                computer_permissions.insert(
                    task_id.clone(),
                    ComputerUsePermission {
                        enabled: *enabled,
                        selected_window_id: selected_window_id.clone(),
                        input_authorized: *input_authorized,
                    },
                );
            }
            return;
        }
        Effect::LoadComputerWindows { task_id } => {
            match list_computer_windows() {
                Ok(windows) => emit(
                    events,
                    Action::ComputerWindowsLoaded {
                        task_id: task_id.clone(),
                        windows: windows.into_iter().map(map_computer_window).collect(),
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::ComputerWindowsFailed {
                        task_id: task_id.clone(),
                        message: error.to_string(),
                    },
                ),
            }
            return;
        }
        Effect::CaptureComputerWindow { task_id, window_id } => {
            match capture_computer_window(window_id) {
                Ok(capture) => emit(
                    events,
                    Action::ComputerCaptureReady {
                        task_id: task_id.clone(),
                        label: capture_label(&capture),
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::ComputerWindowsFailed {
                        task_id: task_id.clone(),
                        message: error.to_string(),
                    },
                ),
            }
            return;
        }
        _ => {}
    }

    let Some(app_server) = connection.as_ref() else {
        emit(events, Action::ConnectionLost);
        return;
    };

    match effect {
        Effect::ConnectAppServer => {}
        Effect::LoadTasks { generation, cursor } => {
            let append = cursor.is_some();
            let params =
                ThreadListParams::state_db_page(DEFAULT_THREAD_PAGE_LIMIT).with_cursor(cursor);
            match app_server.list_threads(params) {
                Ok(page) => emit(
                    events,
                    Action::TasksLoaded {
                        generation,
                        tasks: page.data.into_iter().map(map_task).collect(),
                        next_cursor: page.next_cursor,
                        append,
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::TasksFailed {
                        generation,
                        message: format!("failed to load tasks: {error}"),
                    },
                ),
            }
        }
        Effect::CreateTask => match app_server.start_thread(ThreadStartParams {
            dynamic_tools: Some(computer_use_dynamic_tools()),
            ..ThreadStartParams::default()
        }) {
            Ok(response) => {
                let task = map_task(response.thread);
                let task_id = task.id.clone();
                computer_capable_threads.insert(task_id.clone());
                emit(events, Action::TaskCreated(task));
                emit(events, Action::ComputerUseAvailable { task_id });
            }
            Err(error) => emit(
                events,
                Action::SetStatus(format!("failed to create task: {error}")),
            ),
        },
        Effect::ForkTask { task_id } => {
            match app_server.fork_thread(ThreadForkParams {
                thread_id: task_id.clone(),
                last_turn_id: None,
                before_turn_id: None,
                exclude_turns: true,
                defer_goal_continuation: false,
            }) {
                Ok(response) => {
                    let task = map_task(response.thread);
                    let fork_id = task.id.clone();
                    let inherits_computer_use = computer_capable_threads.contains(&task_id);
                    if inherits_computer_use {
                        computer_capable_threads.insert(fork_id.clone());
                    }
                    emit(events, Action::TaskCreated(task));
                    if inherits_computer_use {
                        emit(events, Action::ComputerUseAvailable { task_id: fork_id });
                    }
                }
                Err(error) => emit(
                    events,
                    Action::SetStatus(format!("failed to fork task: {error}")),
                ),
            }
        }
        Effect::LoadTimeline {
            task_id,
            generation,
            cursor,
        } => {
            let append = cursor.is_some();
            let params = ThreadItemsListParams {
                thread_id: task_id.clone(),
                limit: HISTORY_PAGE_LIMIT,
                sort_direction: HistorySortDirection::Asc,
                turn_id: None,
                cursor,
            };
            match app_server.list_thread_items(params) {
                Ok(page) => emit(
                    events,
                    Action::TimelineLoaded {
                        task_id,
                        generation,
                        items: page
                            .data
                            .into_iter()
                            .map(|entry| map_timeline_item(entry.turn_id, entry.item, true))
                            .collect(),
                        next_cursor: page.next_cursor,
                        append,
                    },
                ),
                Err(_) => emit(
                    events,
                    Action::TimelineFailed {
                        task_id,
                        generation,
                    },
                ),
            }
        }
        Effect::StartTurn { task_id, text } => {
            let _ = app_server.resume_thread(ThreadResumeParams {
                thread_id: task_id.clone(),
                exclude_turns: Some(true),
                initial_turns_page: Some(ThreadResumeInitialTurnsPageParams {
                    cursor: None,
                    limit: 1,
                    sort_direction: HistorySortDirection::Desc,
                    items_view: Some("summary".to_owned()),
                }),
            });
            match app_server.start_turn(TurnStartParams {
                thread_id: task_id.clone(),
                input: vec![UserInput::text(text)],
                cwd: None,
                runtime_workspace_roots: None,
                approval_policy: None,
                permissions: None,
                model: None,
                effort: None,
            }) {
                Ok(response) => {
                    let turn_id = string_field(&response.turn, "id").unwrap_or_default();
                    if !turn_id.is_empty() {
                        emit(events, Action::TurnStarted { task_id, turn_id });
                    }
                }
                Err(error) => emit(
                    events,
                    Action::SetStatus(format!("failed to start turn: {error}")),
                ),
            }
        }
        Effect::RespondApproval {
            request_id,
            decision,
        } => {
            respond_to_approval(app_server, request_id, decision, events, pending_approvals);
        }
        Effect::RefreshMarketplace => {
            match app_server.list_plugins(PluginListParams {
                cwds: None,
                marketplace_kinds: None,
                force_refetch: false,
            }) {
                Ok(response) => {
                    marketplaces.clear();
                    let featured = response.featured_plugin_ids;
                    let mut cards = Vec::new();
                    for marketplace in response.marketplaces {
                        marketplaces.insert(marketplace.name.clone(), marketplace.path.clone());
                        for plugin in marketplace.plugins {
                            let display_name = plugin
                                .presentation
                                .as_ref()
                                .and_then(|presentation| presentation.display_name.clone())
                                .unwrap_or(plugin.name);
                            let description = plugin
                                .presentation
                                .as_ref()
                                .and_then(|presentation| {
                                    presentation
                                        .short_description
                                        .clone()
                                        .or_else(|| presentation.long_description.clone())
                                })
                                .unwrap_or_default();
                            let category = plugin
                                .presentation
                                .as_ref()
                                .and_then(|presentation| presentation.category.clone());
                            cards.push(PluginCard {
                                id: plugin.id.clone(),
                                marketplace: marketplace.name.clone(),
                                name: display_name,
                                description,
                                category,
                                installed: plugin.installed,
                                enabled: plugin.enabled,
                                featured: featured.contains(&plugin.id),
                            });
                        }
                    }
                    emit(events, Action::MarketplaceLoaded(cards));
                }
                Err(error) => emit(
                    events,
                    Action::MarketplaceFailed(format!("failed to load marketplace: {error}")),
                ),
            }
        }
        Effect::InstallPlugin {
            plugin_id,
            marketplace,
        } => {
            let path = marketplaces.get(&marketplace).cloned().flatten();
            let result = app_server.install_plugin(PluginInstallParams {
                marketplace_path: path,
                remote_marketplace_name: marketplaces
                    .get(&marketplace)
                    .is_none_or(Option::is_none)
                    .then_some(marketplace),
                plugin_name: plugin_id.clone(),
            });
            match result {
                Ok(_) => emit(
                    events,
                    Action::PluginMutationFinished {
                        plugin_id,
                        installed: true,
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::SetStatus(format!("plugin installation failed: {error}")),
                ),
            }
        }
        Effect::UninstallPlugin { plugin_id } => {
            match app_server.uninstall_plugin(PluginUninstallParams {
                plugin_id: plugin_id.clone(),
            }) {
                Ok(_) => emit(
                    events,
                    Action::PluginMutationFinished {
                        plugin_id,
                        installed: false,
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::SetStatus(format!("plugin removal failed: {error}")),
                ),
            }
        }
        Effect::RefreshGit { .. }
        | Effect::LoadDiff { .. }
        | Effect::StagePath { .. }
        | Effect::UnstagePath { .. }
        | Effect::SwitchGitBranch { .. }
        | Effect::CreateGitWorktree { .. }
        | Effect::PersistUiState { .. }
        | Effect::RememberWorkspace { .. }
        | Effect::ConfigureComputerUse { .. }
        | Effect::LoadComputerWindows { .. }
        | Effect::CaptureComputerWindow { .. }
        | Effect::SpawnTerminal { .. }
        | Effect::WriteTerminal { .. }
        | Effect::StopTerminal => unreachable!("local effects return before app-server routing"),
    }
}

fn drain_terminal(
    terminal: &mut Option<TerminalSession>,
    parser: &mut Option<vt100::Parser>,
    truncation_reported: &mut bool,
    events: &Sender<Action>,
) {
    let Some(session) = terminal.as_ref() else {
        return;
    };
    let mut screen_changed = false;
    let mut exit_code = None;
    for _ in 0..64 {
        match session.try_recv_event() {
            Ok(Some(TerminalEvent::Output(bytes))) => {
                if let Some(parser) = parser.as_mut() {
                    parser.process(&bytes);
                    screen_changed = true;
                }
            }
            Ok(Some(TerminalEvent::Exited { code })) => {
                exit_code = Some(code);
                break;
            }
            Ok(Some(TerminalEvent::Failed(message))) => {
                emit(events, Action::SetStatus(message.to_owned()));
            }
            Ok(None) => break,
            Err(_) => {
                exit_code = Some(127);
                break;
            }
        }
    }
    if screen_changed && let Some(parser) = parser.as_ref() {
        emit(events, Action::TerminalScreen(parser.screen().contents()));
    }
    if session.output_was_truncated() && !*truncation_reported {
        *truncation_reported = true;
        emit(events, Action::TerminalOutputTruncated);
    }
    if let Some(code) = exit_code {
        terminal.take();
        parser.take();
        emit(events, Action::TerminalExited { code });
    }
}

fn map_git_snapshot(snapshot: GitSnapshot) -> GitState {
    let changed_files = snapshot.files.len();
    let staged_files = snapshot.files.iter().filter(|file| file.staged).count();
    GitState {
        repository_root: Some(snapshot.repository_root),
        branch: snapshot.branch,
        ahead: snapshot.ahead,
        behind: snapshot.behind,
        changed_files,
        staged_files,
        files: snapshot
            .files
            .into_iter()
            .map(|file| GitFileState {
                path: file.path,
                old_path: file.old_path,
                kind: match file.kind {
                    PlatformGitFileKind::Added => CoreGitFileKind::Added,
                    PlatformGitFileKind::Modified => CoreGitFileKind::Modified,
                    PlatformGitFileKind::Deleted => CoreGitFileKind::Deleted,
                    PlatformGitFileKind::Renamed => CoreGitFileKind::Renamed,
                    PlatformGitFileKind::Copied => CoreGitFileKind::Copied,
                    PlatformGitFileKind::Untracked => CoreGitFileKind::Untracked,
                    PlatformGitFileKind::Conflicted => CoreGitFileKind::Conflicted,
                    PlatformGitFileKind::TypeChanged => CoreGitFileKind::TypeChanged,
                },
                staged: file.staged,
                unstaged: file.unstaged,
            })
            .collect(),
        branches: snapshot
            .branches
            .into_iter()
            .map(|branch| GitBranchState {
                name: branch.name,
                commit: branch.commit,
                current: branch.current,
            })
            .collect(),
        worktrees: snapshot
            .worktrees
            .into_iter()
            .map(|worktree| GitWorktreeState {
                path: worktree.path,
                branch: worktree.branch,
                detached: worktree.detached,
                locked: worktree.locked,
            })
            .collect(),
        diff_generation: 0,
        selected_path: None,
        unified_diff: String::new(),
        truncated: snapshot.truncated,
    }
}

fn connect(events: &Sender<Action>, connection: &mut Option<AppServerConnection>) {
    if connection.is_some() {
        emit(events, Action::Connected);
        return;
    }

    let binary = resolve_codex_binary(None);
    let home = match CodexHome::resolve(None) {
        Ok(home) => home,
        Err(error) => {
            emit(events, Action::ConnectionFailed(error.to_string()));
            return;
        }
    };
    let runtime_binary = binary.clone();
    let runtime_home = home.path().to_path_buf();
    let runtime_home_default = home.kind() == CodexHomeKind::Default;
    let app_server = match AppServerConnection::spawn(AppServerConfig::new(binary, home)) {
        Ok(app_server) => app_server,
        Err(error) => {
            emit(events, Action::ConnectionFailed(error.to_string()));
            return;
        }
    };
    match app_server.initialize_with_capabilities(
        ClientInfo {
            name: "codex-rs".to_owned(),
            title: Some("codexRS".to_owned()),
            version: env!("CARGO_PKG_VERSION").to_owned(),
        },
        Some(InitializeCapabilities {
            experimental_api: true,
            ..InitializeCapabilities::default()
        }),
    ) {
        Ok(_) => {
            *connection = Some(app_server);
            emit(
                events,
                Action::RuntimeResolved {
                    codex_binary: runtime_binary,
                    codex_home: runtime_home,
                    codex_home_default: runtime_home_default,
                },
            );
            emit(events, Action::Connected);
        }
        Err(error) => emit(events, Action::ConnectionFailed(error.to_string())),
    }
}

fn computer_use_dynamic_tools() -> Vec<DynamicToolSpec> {
    vec![DynamicToolSpec::Namespace {
        name: "computer_use".to_owned(),
        description:
            "Inspect and control the single desktop window explicitly selected by the user."
                .to_owned(),
        tools: vec![
            dynamic_tool(
                "inspect",
                "Read bounded metadata for the selected window.",
                json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }),
            ),
            dynamic_tool(
                "screenshot",
                "Capture the selected window. Coordinates in later calls are relative to this image.",
                json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }),
            ),
            dynamic_tool(
                "move",
                "Move the pointer to a pixel coordinate relative to the selected window.",
                json!({
                    "type": "object",
                    "required": ["x", "y"],
                    "properties": {
                        "x": {"type": "integer", "minimum": 0},
                        "y": {"type": "integer", "minimum": 0}
                    },
                    "additionalProperties": false
                }),
            ),
            dynamic_tool(
                "click",
                "Click a pixel coordinate relative to the selected window.",
                json!({
                    "type": "object",
                    "required": ["x", "y"],
                    "properties": {
                        "x": {"type": "integer", "minimum": 0},
                        "y": {"type": "integer", "minimum": 0},
                        "button": {"type": "string", "enum": ["left", "right", "middle"]},
                        "clicks": {"type": "integer", "minimum": 1, "maximum": 2}
                    },
                    "additionalProperties": false
                }),
            ),
            dynamic_tool(
                "scroll",
                "Scroll at a pixel coordinate relative to the selected window. Positive y scrolls down and positive x scrolls right.",
                json!({
                    "type": "object",
                    "required": ["x", "y"],
                    "properties": {
                        "x": {"type": "integer", "minimum": 0},
                        "y": {"type": "integer", "minimum": 0},
                        "deltaX": {"type": "integer", "minimum": -100, "maximum": 100},
                        "deltaY": {"type": "integer", "minimum": -100, "maximum": 100}
                    },
                    "additionalProperties": false
                }),
            ),
            dynamic_tool(
                "type",
                "Type bounded Unicode text into the selected, focused window.",
                json!({
                    "type": "object",
                    "required": ["text"],
                    "properties": {
                        "text": {"type": "string", "minLength": 1, "maxLength": 16384}
                    },
                    "additionalProperties": false
                }),
            ),
            dynamic_tool(
                "key",
                "Press one key or shortcut in the selected, focused window.",
                json!({
                    "type": "object",
                    "required": ["key"],
                    "properties": {
                        "key": {"type": "string", "minLength": 1, "maxLength": 32},
                        "modifiers": {
                            "type": "array",
                            "maxItems": 4,
                            "uniqueItems": true,
                            "items": {
                                "type": "string",
                                "enum": ["alt", "control", "meta", "shift"]
                            }
                        }
                    },
                    "additionalProperties": false
                }),
            ),
        ],
    }]
}

fn dynamic_tool(name: &str, description: &str, input_schema: Value) -> DynamicToolNamespaceTool {
    DynamicToolNamespaceTool::new(DynamicToolFunction {
        name: name.to_owned(),
        description: description.to_owned(),
        input_schema,
        defer_loading: None,
    })
}

fn handle_dynamic_tool_call(
    app_server: &AppServerConnection,
    id: &Value,
    params: Value,
    events: &Sender<Action>,
    permissions: &HashMap<String, ComputerUsePermission>,
) {
    let params = match serde_json::from_value::<DynamicToolCallParams>(params) {
        Ok(params) => params,
        Err(_) => {
            let _ = app_server.respond_error(id, -32602, "invalid dynamic tool arguments");
            return;
        }
    };
    if params.namespace.as_deref() != Some("computer_use") {
        respond_dynamic_tool_failure(app_server, id, "unsupported dynamic tool namespace");
        return;
    }
    let Some(permission) = permissions.get(&params.thread_id) else {
        respond_dynamic_tool_failure(app_server, id, "Computer Use is disabled for this task");
        return;
    };
    if !permission.enabled {
        respond_dynamic_tool_failure(app_server, id, "Computer Use is disabled for this task");
        return;
    }
    let Some(window_id) = permission.selected_window_id.as_deref() else {
        respond_dynamic_tool_failure(app_server, id, "no desktop window is selected");
        return;
    };
    if computer_tool_needs_input(&params.tool) && !permission.input_authorized {
        respond_dynamic_tool_failure(
            app_server,
            id,
            "desktop input is not authorized for this session",
        );
        return;
    }

    match run_computer_tool(
        &params.tool,
        &params.arguments,
        &params.thread_id,
        window_id,
        events,
    ) {
        Ok(content_items) => {
            let _ = app_server.respond_success(
                id,
                &DynamicToolCallResponse {
                    content_items,
                    success: true,
                },
            );
        }
        Err(message) => respond_dynamic_tool_failure(app_server, id, &message),
    }
}

fn computer_tool_needs_input(tool: &str) -> bool {
    matches!(tool, "move" | "click" | "scroll" | "type" | "key")
}

fn run_computer_tool(
    tool: &str,
    arguments: &Value,
    task_id: &str,
    window_id: &str,
    events: &Sender<Action>,
) -> Result<Vec<DynamicToolCallOutputContentItem>, String> {
    match tool {
        "inspect" => {
            let window = inspect_computer_window(window_id).map_err(|error| error.to_string())?;
            Ok(vec![text_content(computer_window_description(&window))])
        }
        "screenshot" => {
            let capture = capture_computer_window(window_id).map_err(|error| error.to_string())?;
            emit(
                events,
                Action::ComputerCaptureReady {
                    task_id: task_id.to_owned(),
                    label: capture_label(&capture),
                },
            );
            Ok(vec![
                text_content(computer_capture_description(&capture)),
                DynamicToolCallOutputContentItem::InputImage {
                    image_url: capture.image_url,
                },
            ])
        }
        "move" => {
            let (x, y) = computer_coordinates(arguments)?;
            move_over_computer_window(window_id, x, y).map_err(|error| error.to_string())?;
            Ok(vec![text_content(format!("Pointer moved to ({x}, {y})."))])
        }
        "click" => {
            let (x, y) = computer_coordinates(arguments)?;
            let button = match optional_string_argument(arguments, "button")?.as_deref() {
                None | Some("left") => ComputerButton::Left,
                Some("right") => ComputerButton::Right,
                Some("middle") => ComputerButton::Middle,
                Some(_) => return Err("button must be left, right, or middle".to_owned()),
            };
            let clicks = optional_i32_argument(arguments, "clicks")?.unwrap_or(1);
            let clicks = u8::try_from(clicks).map_err(|_| "clicks must be 1 or 2".to_owned())?;
            click_computer_window(window_id, x, y, button, clicks)
                .map_err(|error| error.to_string())?;
            Ok(vec![text_content(format!("Clicked ({x}, {y})."))])
        }
        "scroll" => {
            let (x, y) = computer_coordinates(arguments)?;
            let delta_x = optional_i32_argument(arguments, "deltaX")?.unwrap_or(0);
            let delta_y = optional_i32_argument(arguments, "deltaY")?.unwrap_or(0);
            scroll_computer_window(window_id, x, y, delta_x, delta_y)
                .map_err(|error| error.to_string())?;
            Ok(vec![text_content(format!(
                "Scrolled at ({x}, {y}) by ({delta_x}, {delta_y})."
            ))])
        }
        "type" => {
            let text = string_argument(arguments, "text")?;
            type_into_computer_window(window_id, text).map_err(|error| error.to_string())?;
            Ok(vec![text_content(format!("Typed {} bytes.", text.len()))])
        }
        "key" => {
            let key = parse_computer_key(string_argument(arguments, "key")?)?;
            let modifiers = parse_computer_modifiers(arguments)?;
            press_computer_key(window_id, key, &modifiers).map_err(|error| error.to_string())?;
            Ok(vec![text_content("Key pressed.".to_owned())])
        }
        _ => Err("unsupported Computer Use tool".to_owned()),
    }
}

fn respond_dynamic_tool_failure(app_server: &AppServerConnection, id: &Value, message: &str) {
    let _ = app_server.respond_success(
        id,
        &DynamicToolCallResponse {
            content_items: vec![text_content(message.to_owned())],
            success: false,
        },
    );
}

fn text_content(text: String) -> DynamicToolCallOutputContentItem {
    DynamicToolCallOutputContentItem::InputText { text }
}

fn computer_coordinates(arguments: &Value) -> Result<(i32, i32), String> {
    Ok((i32_argument(arguments, "x")?, i32_argument(arguments, "y")?))
}

fn i32_argument(arguments: &Value, field: &str) -> Result<i32, String> {
    let value = arguments
        .get(field)
        .and_then(Value::as_i64)
        .ok_or_else(|| format!("{field} must be an integer"))?;
    i32::try_from(value).map_err(|_| format!("{field} is outside the supported range"))
}

fn optional_i32_argument(arguments: &Value, field: &str) -> Result<Option<i32>, String> {
    arguments
        .get(field)
        .map(|_| i32_argument(arguments, field))
        .transpose()
}

fn string_argument<'a>(arguments: &'a Value, field: &str) -> Result<&'a str, String> {
    arguments
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{field} must be a string"))
}

fn optional_string_argument(arguments: &Value, field: &str) -> Result<Option<String>, String> {
    arguments
        .get(field)
        .map(|_| string_argument(arguments, field).map(str::to_owned))
        .transpose()
}

fn parse_computer_modifiers(arguments: &Value) -> Result<Vec<ComputerKey>, String> {
    let Some(value) = arguments.get("modifiers") else {
        return Ok(Vec::new());
    };
    let values = value
        .as_array()
        .ok_or_else(|| "modifiers must be an array".to_owned())?;
    if values.len() > 4 {
        return Err("at most four modifiers are supported".to_owned());
    }
    let mut modifiers = Vec::with_capacity(values.len());
    for value in values {
        let value = value
            .as_str()
            .ok_or_else(|| "each modifier must be a string".to_owned())?;
        let modifier = match value {
            "alt" => ComputerKey::Alt,
            "control" => ComputerKey::Control,
            "meta" => ComputerKey::Meta,
            "shift" => ComputerKey::Shift,
            _ => return Err("unsupported modifier".to_owned()),
        };
        if modifiers.contains(&modifier) {
            return Err("duplicate modifier".to_owned());
        }
        modifiers.push(modifier);
    }
    Ok(modifiers)
}

fn parse_computer_key(value: &str) -> Result<ComputerKey, String> {
    let key = match value.to_ascii_lowercase().as_str() {
        "alt" => ComputerKey::Alt,
        "backspace" => ComputerKey::Backspace,
        "control" | "ctrl" => ComputerKey::Control,
        "delete" => ComputerKey::Delete,
        "down" | "arrowdown" => ComputerKey::Down,
        "end" => ComputerKey::End,
        "enter" | "return" => ComputerKey::Enter,
        "escape" | "esc" => ComputerKey::Escape,
        "home" => ComputerKey::Home,
        "left" | "arrowleft" => ComputerKey::Left,
        "meta" | "super" | "win" => ComputerKey::Meta,
        "pagedown" => ComputerKey::PageDown,
        "pageup" => ComputerKey::PageUp,
        "right" | "arrowright" => ComputerKey::Right,
        "shift" => ComputerKey::Shift,
        "space" => ComputerKey::Space,
        "tab" => ComputerKey::Tab,
        "up" | "arrowup" => ComputerKey::Up,
        _ => {
            let mut characters = value.chars();
            let Some(character) = characters.next() else {
                return Err("key must not be empty".to_owned());
            };
            if characters.next().is_some() {
                return Err("unsupported key name".to_owned());
            }
            ComputerKey::Character(character)
        }
    };
    Ok(key)
}

fn map_computer_window(window: ComputerWindow) -> ComputerWindowState {
    ComputerWindowState {
        id: window.id,
        application: window.application,
        title: window.title,
        width: window.width,
        height: window.height,
        minimized: window.minimized,
        focused: window.focused,
    }
}

fn capture_label(capture: &ComputerCapture) -> String {
    format!(
        "Captured {}×{} ({} KiB)",
        capture.width,
        capture.height,
        capture.jpeg_bytes.div_ceil(1024)
    )
}

fn computer_window_description(window: &ComputerWindow) -> String {
    json!({
        "application": window.application,
        "title": window.title,
        "width": window.width,
        "height": window.height,
        "minimized": window.minimized,
        "focused": window.focused
    })
    .to_string()
}

fn computer_capture_description(capture: &ComputerCapture) -> String {
    json!({
        "application": capture.window.application,
        "title": capture.window.title,
        "width": capture.width,
        "height": capture.height,
        "coordinateOrigin": "top-left",
        "coordinateSpace": "captured-image-pixels"
    })
    .to_string()
}

fn handle_app_server_event(
    app_server: &AppServerConnection,
    event: AppServerEvent,
    events: &Sender<Action>,
    pending_approvals: &mut HashMap<String, PendingApproval>,
    computer_permissions: &HashMap<String, ComputerUsePermission>,
) -> bool {
    match event {
        AppServerEvent::Notification { method, params } => {
            handle_notification(&method, params, events)
        }
        AppServerEvent::Request { id, method, params } => {
            match method.as_str() {
                "item/commandExecution/requestApproval"
                | "item/fileChange/requestApproval"
                | "item/permissions/requestApproval" => {
                    let request_id = request_key(&id);
                    let request = ApprovalRequest {
                        request_id: request_id.clone(),
                        task_id: string_field(&params, "threadId").unwrap_or_default(),
                        turn_id: string_field(&params, "turnId"),
                        kind: match method.as_str() {
                            "item/commandExecution/requestApproval" => ApprovalKind::Command,
                            "item/fileChange/requestApproval" => ApprovalKind::FileChange,
                            _ => ApprovalKind::Permissions,
                        },
                        title: approval_title(&method),
                        detail: approval_detail(&method, &params),
                    };
                    pending_approvals.insert(request_id, PendingApproval { id, method, params });
                    emit(events, Action::ApprovalRequested(request));
                }
                "currentTime/read" => {
                    let current_time_at = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map_or(0, |duration| duration.as_secs());
                    let _ = app_server
                        .respond_success(&id, &json!({ "currentTimeAt": current_time_at }));
                }
                "item/tool/call" => {
                    handle_dynamic_tool_call(app_server, &id, params, events, computer_permissions);
                }
                _ => {
                    let _ = app_server.respond_error(&id, -32601, "unsupported client request");
                }
            }
            false
        }
        AppServerEvent::NotificationsDropped { count } => {
            emit(
                events,
                Action::SetStatus(format!("{count} app-server notifications were coalesced")),
            );
            false
        }
        AppServerEvent::Disconnected => {
            emit(events, Action::ConnectionLost);
            false
        }
    }
}

fn handle_notification(method: &str, params: Value, events: &Sender<Action>) -> bool {
    if method == "fs/changed" {
        return true;
    }
    match method {
        "turn/started" => {
            if let (Some(task_id), Some(turn_id)) = (
                string_field(&params, "threadId"),
                params.get("turn").and_then(|turn| string_field(turn, "id")),
            ) {
                emit(events, Action::TurnStarted { task_id, turn_id });
            }
        }
        "turn/completed" => {
            if let (Some(task_id), Some(turn)) =
                (string_field(&params, "threadId"), params.get("turn"))
                && let Some(turn_id) = string_field(turn, "id")
            {
                let failed = string_field(turn, "status").as_deref() == Some("failed");
                emit(
                    events,
                    Action::TurnCompleted {
                        task_id,
                        turn_id,
                        failed,
                    },
                );
            }
        }
        "item/started" | "item/completed" => {
            if let (Some(task_id), Some(turn_id), Some(item)) = (
                string_field(&params, "threadId"),
                string_field(&params, "turnId"),
                params.get("item").cloned(),
            ) {
                emit(
                    events,
                    Action::UpsertTimelineItem {
                        task_id,
                        item: map_timeline_item(turn_id, item, method == "item/completed"),
                    },
                );
            }
        }
        "item/agentMessage/delta"
        | "item/plan/delta"
        | "item/reasoning/summaryTextDelta"
        | "item/reasoning/textDelta"
        | "item/commandExecution/outputDelta"
        | "item/fileChange/outputDelta" => {
            if let (Some(task_id), Some(turn_id), Some(item_id), Some(delta)) = (
                string_field(&params, "threadId"),
                string_field(&params, "turnId"),
                string_field(&params, "itemId"),
                string_field(&params, "delta"),
            ) {
                emit(
                    events,
                    Action::TimelineDelta {
                        task_id,
                        turn_id,
                        item_id,
                        kind: notification_kind(method),
                        delta,
                    },
                );
            }
        }
        "thread/started"
        | "thread/status/changed"
        | "thread/name/updated"
        | "thread/archived"
        | "thread/unarchived"
        | "thread/deleted" => emit(events, Action::RefreshTasks),
        "warning" | "guardianWarning" | "deprecationNotice" | "configWarning" => {
            if let Some(message) = string_field(&params, "message") {
                emit(
                    events,
                    Action::SetStatus(bounded(message, MAX_STATUS_BYTES)),
                );
            }
        }
        "error" => {
            let message = params
                .get("error")
                .and_then(|error| {
                    string_field(error, "message")
                        .or_else(|| string_field(error, "additionalDetails"))
                })
                .unwrap_or_else(|| "Codex reported an error.".to_owned());
            emit(
                events,
                Action::SetStatus(bounded(message, MAX_STATUS_BYTES)),
            );
        }
        _ => {}
    }
    false
}

fn refresh_git(cwd: &std::path::Path, events: &Sender<Action>) {
    match git_snapshot(cwd) {
        Ok(snapshot) => emit(
            events,
            Action::GitSnapshotLoaded(map_git_snapshot(snapshot)),
        ),
        Err(GitError::InvalidRepository) => {
            emit(events, Action::GitSnapshotLoaded(GitState::default()));
        }
        Err(error) => {
            emit(events, Action::GitSnapshotLoaded(GitState::default()));
            emit(
                events,
                Action::SetStatus(format!("failed to inspect Git repository: {error}")),
            );
        }
    }
}

fn respond_to_approval(
    app_server: &AppServerConnection,
    request_id: String,
    decision: ApprovalDecision,
    events: &Sender<Action>,
    pending_approvals: &mut HashMap<String, PendingApproval>,
) {
    let Some(pending) = pending_approvals.remove(&request_id) else {
        return;
    };
    let response = match pending.method.as_str() {
        "item/commandExecution/requestApproval" | "item/fileChange/requestApproval" => {
            let decision = match decision {
                ApprovalDecision::Accept => "accept",
                ApprovalDecision::Decline => "decline",
                ApprovalDecision::AcceptForSession => "acceptForSession",
            };
            app_server.respond_success(&pending.id, &json!({ "decision": decision }))
        }
        "item/permissions/requestApproval" => {
            if decision == ApprovalDecision::Decline {
                app_server.respond_error(&pending.id, -32001, "user declined permissions")
            } else {
                let requested = pending
                    .params
                    .get("permissions")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                let permissions = json!({
                    "network": requested.get("network").cloned().unwrap_or(Value::Null),
                    "fileSystem": requested.get("fileSystem").cloned().unwrap_or(Value::Null)
                });
                let scope = if decision == ApprovalDecision::AcceptForSession {
                    "session"
                } else {
                    "turn"
                };
                app_server.respond_success(
                    &pending.id,
                    &json!({ "permissions": permissions, "scope": scope }),
                )
            }
        }
        _ => app_server.respond_error(&pending.id, -32601, "unsupported approval"),
    };
    if let Err(error) = response {
        emit(
            events,
            Action::SetStatus(format!("failed to answer approval: {error}")),
        );
    }
}

fn map_task(thread: codex_protocol::ThreadSummary) -> TaskSummary {
    let title = thread
        .name
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| {
            thread
                .preview
                .lines()
                .find(|line| !line.trim().is_empty())
                .unwrap_or("Untitled task")
                .trim()
                .to_owned()
        });
    let status_type = thread
        .status
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("idle");
    let waiting = thread
        .status
        .get("activeFlags")
        .and_then(Value::as_array)
        .is_some_and(|flags| {
            flags
                .iter()
                .any(|flag| flag.as_str() == Some("waitingOnApproval"))
        });
    TaskSummary {
        id: thread.id,
        title: bounded(title, 512),
        preview: bounded(thread.preview, 4 * 1024),
        cwd: thread.cwd,
        updated_at: thread.recency_at.unwrap_or(thread.updated_at),
        parent_task_id: thread.parent_thread_id,
        forked_from_id: thread.forked_from_id,
        status: if waiting {
            TaskRunStatus::WaitingForApproval
        } else {
            match status_type {
                "active" => TaskRunStatus::Running,
                "systemError" => TaskRunStatus::Failed,
                _ => TaskRunStatus::Idle,
            }
        },
    }
}

fn map_timeline_item(turn_id: String, item: Value, completed: bool) -> TimelineItem {
    let item_type = string_field(&item, "type").unwrap_or_else(|| "notice".to_owned());
    let id = string_field(&item, "id").unwrap_or_else(|| format!("{turn_id}:{item_type}"));
    let (kind, text) = match item_type.as_str() {
        "userMessage" => (
            TimelineKind::User,
            item.get("content")
                .and_then(Value::as_array)
                .map(|content| {
                    content
                        .iter()
                        .filter_map(|entry| {
                            string_field(entry, "text")
                                .or_else(|| string_field(entry, "path"))
                                .or_else(|| string_field(entry, "name"))
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default(),
        ),
        "agentMessage" => (
            TimelineKind::Agent,
            string_field(&item, "text").unwrap_or_default(),
        ),
        "plan" => (
            TimelineKind::Plan,
            string_field(&item, "text").unwrap_or_default(),
        ),
        "reasoning" => {
            let text = string_array(&item, "summary");
            let text = if text.is_empty() {
                string_array(&item, "content")
            } else {
                text
            };
            (TimelineKind::Reasoning, text)
        }
        "commandExecution" => {
            let command = string_field(&item, "command").unwrap_or_default();
            let output = string_field(&item, "aggregatedOutput").unwrap_or_default();
            let text = if output.is_empty() {
                command
            } else {
                format!("$ {command}\n{output}")
            };
            (TimelineKind::Command, text)
        }
        "fileChange" => {
            let paths = item
                .get("changes")
                .and_then(Value::as_array)
                .map(|changes| {
                    changes
                        .iter()
                        .filter_map(|change| {
                            string_field(change, "path")
                                .or_else(|| string_field(change, "filePath"))
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default();
            (TimelineKind::FileChange, paths)
        }
        "mcpToolCall" => {
            let server = string_field(&item, "server").unwrap_or_default();
            let tool = string_field(&item, "tool").unwrap_or_default();
            (TimelineKind::Tool, format!("{server} / {tool}"))
        }
        "dynamicToolCall" => {
            let namespace = string_field(&item, "namespace").unwrap_or_default();
            let tool = string_field(&item, "tool").unwrap_or_default();
            (TimelineKind::Tool, format!("{namespace} / {tool}"))
        }
        "collabAgentToolCall" | "subAgentActivity" => (
            TimelineKind::Tool,
            string_field(&item, "tool")
                .or_else(|| string_field(&item, "kind"))
                .unwrap_or(item_type.clone()),
        ),
        "enteredReviewMode" | "exitedReviewMode" => (
            TimelineKind::Notice,
            string_field(&item, "review").unwrap_or(item_type),
        ),
        _ => (TimelineKind::Notice, item_type),
    };
    TimelineItem {
        id,
        turn_id,
        kind,
        text: bounded(text, MAX_ITEM_TEXT_BYTES),
        completed,
    }
}

fn notification_kind(method: &str) -> TimelineKind {
    match method {
        "item/agentMessage/delta" => TimelineKind::Agent,
        "item/plan/delta" => TimelineKind::Plan,
        "item/reasoning/summaryTextDelta" | "item/reasoning/textDelta" => TimelineKind::Reasoning,
        "item/commandExecution/outputDelta" => TimelineKind::Command,
        "item/fileChange/outputDelta" => TimelineKind::FileChange,
        _ => TimelineKind::Notice,
    }
}

fn approval_title(method: &str) -> String {
    match method {
        "item/commandExecution/requestApproval" => "Command approval",
        "item/fileChange/requestApproval" => "File change approval",
        "item/permissions/requestApproval" => "Permission request",
        _ => "Approval",
    }
    .to_owned()
}

fn approval_detail(method: &str, params: &Value) -> String {
    let detail = match method {
        "item/commandExecution/requestApproval" => string_field(params, "command")
            .or_else(|| string_field(params, "reason"))
            .unwrap_or_else(|| "Codex wants to run a command.".to_owned()),
        "item/fileChange/requestApproval" => string_field(params, "reason")
            .or_else(|| string_field(params, "grantRoot"))
            .unwrap_or_else(|| "Codex wants to change files.".to_owned()),
        "item/permissions/requestApproval" => string_field(params, "reason")
            .or_else(|| string_field(params, "cwd"))
            .unwrap_or_else(|| "Codex requests additional permissions.".to_owned()),
        _ => "Approval requested.".to_owned(),
    };
    bounded(detail, 8 * 1024)
}

fn request_key(id: &Value) -> String {
    bounded(id.to_string(), 512)
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn string_array(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

fn bounded(mut value: String, limit: usize) -> String {
    if value.len() <= limit {
        return value;
    }
    let boundary = value
        .char_indices()
        .map(|(index, _)| index)
        .take_while(|index| *index <= limit)
        .last()
        .unwrap_or(0);
    value.truncate(boundary);
    value
}

fn emit(events: &Sender<Action>, action: Action) {
    let _ = events.send_timeout(action, UI_EVENT_TIMEOUT);
}

#[cfg(test)]
mod tests {
    use super::GitRefreshDebouncer;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    #[test]
    fn git_refreshes_are_coalesced_until_the_debounce_expires() {
        let mut debouncer = GitRefreshDebouncer::default();
        let start = Instant::now();
        let delay = Duration::from_millis(300);

        debouncer.schedule(PathBuf::from("first"), start, delay);
        debouncer.schedule(
            PathBuf::from("latest"),
            start + Duration::from_millis(100),
            delay,
        );

        assert_eq!(debouncer.take_due(start + Duration::from_millis(399)), None);
        assert_eq!(
            debouncer.take_due(start + Duration::from_millis(400)),
            Some(PathBuf::from("latest"))
        );
        assert_eq!(debouncer.take_due(start + Duration::from_millis(500)), None);
    }
}
