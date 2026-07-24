use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;

pub const MAX_COMPOSER_BYTES: usize = 256 * 1024;
pub const MAX_VISIBLE_THREADS: usize = 500;
pub const MAX_TIMELINE_ITEMS: usize = 2_000;
pub const MAX_PENDING_APPROVALS: usize = 64;
pub const MAX_MARKETPLACE_ITEMS: usize = 500;
pub const MAX_COMPUTER_WINDOWS: usize = 100;
pub const MAX_GIT_BRANCH_BYTES: usize = 1_024;

/// Identifies the installed build used as the behavioral oracle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildReference {
    pub package_name: &'static str,
    pub package_version: &'static str,
    pub cli_version: &'static str,
    pub architecture: &'static str,
    pub runtime: &'static str,
}

pub const STABLE_REFERENCE: BuildReference = BuildReference {
    package_name: "OpenAI.Codex",
    package_version: "26.721.3996.0",
    cli_version: "0.146.0-alpha.3.1",
    architecture: "x64",
    runtime: "Owl/Chromium 150.0.7871.128",
};

#[must_use]
pub const fn stable_reference() -> &'static BuildReference {
    &STABLE_REFERENCE
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionStatus {
    Offline,
    Connecting,
    Online,
    Recovering,
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainRoute {
    Tasks,
    Repository,
    Marketplace,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorPane {
    Changes,
    Terminal,
    ComputerUse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadStatus {
    Idle,
    Loading,
    Ready,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskRunStatus {
    Idle,
    Running,
    WaitingForApproval,
    Completed,
    Interrupted,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSummary {
    pub id: String,
    pub title: String,
    pub preview: String,
    pub cwd: PathBuf,
    pub updated_at: i64,
    pub parent_task_id: Option<String>,
    pub forked_from_id: Option<String>,
    pub status: TaskRunStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelineKind {
    User,
    Agent,
    Reasoning,
    Plan,
    Command,
    FileChange,
    Tool,
    Notice,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineItem {
    pub id: String,
    pub turn_id: String,
    pub kind: TimelineKind,
    pub text: String,
    pub completed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineState {
    pub status: LoadStatus,
    pub generation: u64,
    pub items: Vec<TimelineItem>,
    pub next_cursor: Option<String>,
    pub active_turn_id: Option<String>,
}

impl Default for TimelineState {
    fn default() -> Self {
        Self {
            status: LoadStatus::Idle,
            generation: 0,
            items: Vec::new(),
            next_cursor: None,
            active_turn_id: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalKind {
    Command,
    FileChange,
    Permissions,
    UserInput,
    DynamicTool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalDecision {
    Accept,
    Decline,
    AcceptForSession,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRequest {
    pub request_id: String,
    pub task_id: String,
    pub turn_id: Option<String>,
    pub kind: ApprovalKind,
    pub title: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ComputerUseState {
    pub available_for_task: bool,
    pub enabled_for_task: bool,
    pub selected_window_id: Option<String>,
    pub selected_window_title: Option<String>,
    pub input_authorized_for_session: bool,
    pub last_capture_label: Option<String>,
    pub windows: Vec<ComputerWindowState>,
    pub windows_loading: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComputerWindowState {
    pub id: String,
    pub application: String,
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub minimized: bool,
    pub focused: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginCard {
    pub id: String,
    pub marketplace: String,
    pub name: String,
    pub description: String,
    pub category: Option<String>,
    pub installed: bool,
    pub enabled: bool,
    pub featured: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MarketplaceState {
    pub status: Option<LoadStatus>,
    pub query: String,
    pub plugins: Vec<PluginCard>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GitState {
    pub repository_root: Option<PathBuf>,
    pub branch: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub changed_files: usize,
    pub staged_files: usize,
    pub files: Vec<GitFileState>,
    pub branches: Vec<GitBranchState>,
    pub worktrees: Vec<GitWorktreeState>,
    pub diff_generation: u64,
    pub selected_path: Option<PathBuf>,
    pub unified_diff: String,
    pub truncated: bool,
}

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
pub struct GitFileState {
    pub path: PathBuf,
    pub old_path: Option<PathBuf>,
    pub kind: GitFileKind,
    pub staged: bool,
    pub unstaged: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitBranchState {
    pub name: String,
    pub commit: String,
    pub current: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitWorktreeState {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub detached: bool,
    pub locked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TerminalState {
    pub process_id: Option<String>,
    pub title: String,
    pub output: String,
    pub running: bool,
    pub truncated: bool,
    pub exit_code: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StorageState {
    pub path: Option<PathBuf>,
    pub ready: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeState {
    pub codex_binary: Option<PathBuf>,
    pub codex_home: Option<PathBuf>,
    pub codex_home_default: bool,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub connection: ConnectionStatus,
    pub route: MainRoute,
    pub inspector: InspectorPane,
    pub task_status: LoadStatus,
    pub task_generation: u64,
    pub tasks: Vec<TaskSummary>,
    pub next_task_cursor: Option<String>,
    pub selected_task_id: Option<String>,
    pub timelines: HashMap<String, TimelineState>,
    pub composer: String,
    pub composer_error: Option<String>,
    pub approvals: VecDeque<ApprovalRequest>,
    pub computer_use: HashMap<String, ComputerUseState>,
    pub marketplace: MarketplaceState,
    pub git: GitState,
    pub terminal: TerminalState,
    pub storage: StorageState,
    pub runtime: RuntimeState,
    pub status_message: Option<String>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            connection: ConnectionStatus::Offline,
            route: MainRoute::Tasks,
            inspector: InspectorPane::Changes,
            task_status: LoadStatus::Idle,
            task_generation: 0,
            tasks: Vec::new(),
            next_task_cursor: None,
            selected_task_id: None,
            timelines: HashMap::new(),
            composer: String::new(),
            composer_error: None,
            approvals: VecDeque::new(),
            computer_use: HashMap::new(),
            marketplace: MarketplaceState::default(),
            git: GitState::default(),
            terminal: TerminalState::default(),
            storage: StorageState::default(),
            runtime: RuntimeState::default(),
            status_message: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Action {
    Connect,
    Connected,
    ConnectionLost,
    ConnectionFailed(String),
    RetryConnection,
    StorageOpened {
        path: PathBuf,
        route: Option<MainRoute>,
        inspector: Option<InspectorPane>,
    },
    StorageFailed(String),
    RuntimeResolved {
        codex_binary: PathBuf,
        codex_home: PathBuf,
        codex_home_default: bool,
    },
    Navigate(MainRoute),
    ShowInspector(InspectorPane),
    RefreshTasks,
    LoadMoreTasks,
    NewTask,
    ForkSelectedTask,
    TaskCreated(TaskSummary),
    TasksLoaded {
        generation: u64,
        tasks: Vec<TaskSummary>,
        next_cursor: Option<String>,
        append: bool,
    },
    TasksFailed {
        generation: u64,
        message: String,
    },
    SelectTask(String),
    TimelineLoaded {
        task_id: String,
        generation: u64,
        items: Vec<TimelineItem>,
        next_cursor: Option<String>,
        append: bool,
    },
    TimelineFailed {
        task_id: String,
        generation: u64,
    },
    ComposerChanged(String),
    SubmitComposer,
    TurnStarted {
        task_id: String,
        turn_id: String,
    },
    TurnCompleted {
        task_id: String,
        turn_id: String,
        failed: bool,
    },
    TurnInterrupted {
        task_id: String,
        turn_id: String,
    },
    TimelineDelta {
        task_id: String,
        turn_id: String,
        item_id: String,
        kind: TimelineKind,
        delta: String,
    },
    UpsertTimelineItem {
        task_id: String,
        item: TimelineItem,
    },
    TimelineItemCompleted {
        task_id: String,
        item_id: String,
    },
    ApprovalRequested(ApprovalRequest),
    ResolveApproval {
        request_id: String,
        decision: ApprovalDecision,
    },
    ComputerUseAvailable {
        task_id: String,
    },
    ToggleComputerUse {
        task_id: String,
        enabled: bool,
    },
    SelectComputerUseWindow {
        task_id: String,
        window_id: String,
        title: String,
    },
    AuthorizeComputerInputForSession {
        task_id: String,
        authorized: bool,
    },
    RefreshComputerWindows {
        task_id: String,
    },
    ComputerWindowsLoaded {
        task_id: String,
        windows: Vec<ComputerWindowState>,
    },
    ComputerWindowsFailed {
        task_id: String,
        message: String,
    },
    CaptureComputerWindow {
        task_id: String,
    },
    ComputerCaptureReady {
        task_id: String,
        label: String,
    },
    RefreshMarketplace,
    MarketplaceLoaded(Vec<PluginCard>),
    MarketplaceFailed(String),
    MarketplaceQueryChanged(String),
    InstallPlugin {
        plugin_id: String,
        marketplace: String,
    },
    UninstallPlugin {
        plugin_id: String,
    },
    PluginMutationFinished {
        plugin_id: String,
        installed: bool,
    },
    RefreshGit,
    GitSnapshotLoaded(GitState),
    SelectDiffPath(PathBuf),
    StagePath(PathBuf),
    UnstagePath(PathBuf),
    SwitchGitBranch(String),
    CreateGitWorktree {
        path: PathBuf,
        branch: String,
        create_branch: bool,
    },
    DiffLoaded {
        generation: u64,
        text: String,
        truncated: bool,
    },
    SpawnTerminal,
    TerminalStarted {
        process_id: String,
        title: String,
    },
    TerminalScreen(String),
    TerminalOutputTruncated,
    SubmitTerminalInput(String),
    StopTerminal,
    TerminalExited {
        code: u32,
    },
    SetStatus(String),
    ClearStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    ConnectAppServer,
    LoadTasks {
        generation: u64,
        cursor: Option<String>,
    },
    CreateTask,
    ForkTask {
        task_id: String,
    },
    LoadTimeline {
        task_id: String,
        generation: u64,
        cursor: Option<String>,
    },
    StartTurn {
        task_id: String,
        text: String,
    },
    RespondApproval {
        request_id: String,
        decision: ApprovalDecision,
    },
    RefreshMarketplace,
    InstallPlugin {
        plugin_id: String,
        marketplace: String,
    },
    UninstallPlugin {
        plugin_id: String,
    },
    ConfigureComputerUse {
        task_id: String,
        enabled: bool,
        selected_window_id: Option<String>,
        input_authorized: bool,
    },
    LoadComputerWindows {
        task_id: String,
    },
    CaptureComputerWindow {
        task_id: String,
        window_id: String,
    },
    RefreshGit {
        cwd: PathBuf,
    },
    LoadDiff {
        generation: u64,
        root: PathBuf,
        path: PathBuf,
    },
    StagePath {
        root: PathBuf,
        path: PathBuf,
    },
    UnstagePath {
        root: PathBuf,
        path: PathBuf,
    },
    SwitchGitBranch {
        root: PathBuf,
        branch: String,
    },
    CreateGitWorktree {
        root: PathBuf,
        path: PathBuf,
        branch: String,
        create_branch: bool,
    },
    SpawnTerminal {
        cwd: PathBuf,
    },
    WriteTerminal {
        input: String,
    },
    StopTerminal,
    PersistUiState {
        route: MainRoute,
        inspector: InspectorPane,
    },
    RememberWorkspace {
        path: PathBuf,
    },
}

pub fn reduce(state: &mut AppState, action: Action) -> Vec<Effect> {
    match action {
        Action::Connect | Action::RetryConnection => {
            state.connection = ConnectionStatus::Connecting;
            vec![Effect::ConnectAppServer]
        }
        Action::Connected => {
            state.connection = ConnectionStatus::Online;
            state.task_generation = state.task_generation.saturating_add(1);
            state.task_status = LoadStatus::Loading;
            vec![Effect::LoadTasks {
                generation: state.task_generation,
                cursor: None,
            }]
        }
        Action::ConnectionLost => {
            state.connection = ConnectionStatus::Recovering;
            state.status_message = Some("Connection lost. Reconnecting…".to_owned());
            vec![Effect::ConnectAppServer]
        }
        Action::ConnectionFailed(message) => {
            state.connection = ConnectionStatus::Failed(message);
            Vec::new()
        }
        Action::StorageOpened {
            path,
            route,
            inspector,
        } => {
            state.storage = StorageState {
                path: Some(path),
                ready: true,
                error: None,
            };
            if let Some(route) = route {
                state.route = if route == MainRoute::Repository && state.selected_task_id.is_none()
                {
                    MainRoute::Tasks
                } else {
                    route
                };
            }
            if let Some(inspector) = inspector {
                state.inspector = inspector;
            }
            Vec::new()
        }
        Action::StorageFailed(message) => {
            state.storage.ready = false;
            state.storage.error = Some(message.clone());
            state.status_message = Some(format!("local UI state is unavailable: {message}"));
            Vec::new()
        }
        Action::RuntimeResolved {
            codex_binary,
            codex_home,
            codex_home_default,
        } => {
            state.runtime.codex_binary = Some(codex_binary);
            state.runtime.codex_home = Some(codex_home);
            state.runtime.codex_home_default = codex_home_default;
            Vec::new()
        }
        Action::Navigate(route) => {
            state.route = route;
            let mut effects = vec![Effect::PersistUiState {
                route: state.route,
                inspector: state.inspector,
            }];
            if route == MainRoute::Marketplace
                && state.marketplace.status != Some(LoadStatus::Ready)
            {
                state.marketplace.status = Some(LoadStatus::Loading);
                effects.push(Effect::RefreshMarketplace);
            }
            effects
        }
        Action::ShowInspector(pane) => {
            state.inspector = pane;
            vec![Effect::PersistUiState {
                route: state.route,
                inspector: state.inspector,
            }]
        }
        Action::RefreshTasks => {
            state.task_generation = state.task_generation.saturating_add(1);
            state.task_status = LoadStatus::Loading;
            vec![Effect::LoadTasks {
                generation: state.task_generation,
                cursor: None,
            }]
        }
        Action::NewTask => vec![Effect::CreateTask],
        Action::ForkSelectedTask => state
            .selected_task_id
            .clone()
            .map(|task_id| vec![Effect::ForkTask { task_id }])
            .unwrap_or_default(),
        Action::TaskCreated(task) => {
            let task_id = task.id.clone();
            let cwd = task.cwd.clone();
            state.tasks.retain(|existing| existing.id != task_id);
            state.tasks.insert(0, task);
            state.tasks.truncate(MAX_VISIBLE_THREADS);
            state.selected_task_id = Some(task_id.clone());
            state.timelines.insert(
                task_id.clone(),
                TimelineState {
                    status: LoadStatus::Ready,
                    ..TimelineState::default()
                },
            );
            vec![
                Effect::RememberWorkspace { path: cwd.clone() },
                Effect::RefreshGit { cwd },
            ]
        }
        Action::LoadMoreTasks => {
            if state.task_status == LoadStatus::Loading || state.next_task_cursor.is_none() {
                Vec::new()
            } else {
                state.task_status = LoadStatus::Loading;
                vec![Effect::LoadTasks {
                    generation: state.task_generation,
                    cursor: state.next_task_cursor.clone(),
                }]
            }
        }
        Action::TasksLoaded {
            generation,
            mut tasks,
            next_cursor,
            append,
        } => {
            if generation != state.task_generation {
                return Vec::new();
            }
            if append {
                state.tasks.append(&mut tasks);
            } else {
                if let Some(selected) = state.selected_task_id.as_deref().and_then(|selected_id| {
                    (!tasks.iter().any(|task| task.id == selected_id))
                        .then(|| {
                            state
                                .tasks
                                .iter()
                                .find(|task| task.id == selected_id)
                                .cloned()
                        })
                        .flatten()
                }) {
                    tasks.insert(0, selected);
                }
                state.tasks = tasks;
            }
            state.tasks.truncate(MAX_VISIBLE_THREADS);
            state.next_task_cursor = next_cursor;
            state.task_status = LoadStatus::Ready;
            Vec::new()
        }
        Action::TasksFailed {
            generation,
            message,
        } => {
            if generation == state.task_generation {
                state.task_status = LoadStatus::Failed;
                state.status_message = Some(message);
            }
            Vec::new()
        }
        Action::SelectTask(task_id) => {
            state.selected_task_id = Some(task_id.clone());
            let mut effects = state
                .tasks
                .iter()
                .find(|task| task.id == task_id)
                .map(|task| {
                    vec![
                        Effect::RememberWorkspace {
                            path: task.cwd.clone(),
                        },
                        Effect::RefreshGit {
                            cwd: task.cwd.clone(),
                        },
                    ]
                })
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
            let timeline = state.timelines.entry(task_id.clone()).or_default();
            if matches!(timeline.status, LoadStatus::Idle | LoadStatus::Failed) {
                timeline.generation = timeline.generation.saturating_add(1);
                timeline.status = LoadStatus::Loading;
                effects.insert(
                    0,
                    Effect::LoadTimeline {
                        task_id,
                        generation: timeline.generation,
                        cursor: None,
                    },
                );
            }
            effects
        }
        Action::TimelineLoaded {
            task_id,
            generation,
            mut items,
            next_cursor,
            append,
        } => {
            let timeline = state.timelines.entry(task_id).or_default();
            if generation != timeline.generation {
                return Vec::new();
            }
            if append {
                timeline.items.append(&mut items);
            } else {
                timeline.items = items;
            }
            trim_front(&mut timeline.items, MAX_TIMELINE_ITEMS);
            timeline.next_cursor = next_cursor;
            timeline.status = LoadStatus::Ready;
            Vec::new()
        }
        Action::TimelineFailed {
            task_id,
            generation,
        } => {
            let timeline = state.timelines.entry(task_id).or_default();
            if generation == timeline.generation {
                timeline.status = LoadStatus::Failed;
            }
            Vec::new()
        }
        Action::ComposerChanged(text) => {
            if text.len() <= MAX_COMPOSER_BYTES {
                state.composer = text;
                state.composer_error = None;
            } else {
                state.composer_error = Some("Message exceeds the 256 KiB limit.".to_owned());
            }
            Vec::new()
        }
        Action::SubmitComposer => {
            let Some(task_id) = state.selected_task_id.clone() else {
                state.composer_error = Some("Select a task first.".to_owned());
                return Vec::new();
            };
            let text = state.composer.trim().to_owned();
            if text.is_empty() {
                return Vec::new();
            }
            state.composer.clear();
            state.composer_error = None;
            vec![Effect::StartTurn { task_id, text }]
        }
        Action::TurnStarted { task_id, turn_id } => {
            let timeline = state.timelines.entry(task_id.clone()).or_default();
            timeline.active_turn_id = Some(turn_id);
            if let Some(task) = state.tasks.iter_mut().find(|task| task.id == task_id) {
                task.status = TaskRunStatus::Running;
            }
            Vec::new()
        }
        Action::TurnCompleted {
            task_id,
            turn_id,
            failed,
        } => {
            let timeline = state.timelines.entry(task_id.clone()).or_default();
            if timeline.active_turn_id.as_deref() == Some(turn_id.as_str()) {
                timeline.active_turn_id = None;
            }
            if let Some(task) = state.tasks.iter_mut().find(|task| task.id == task_id) {
                task.status = if failed {
                    TaskRunStatus::Failed
                } else {
                    TaskRunStatus::Completed
                };
            }
            Vec::new()
        }
        Action::TurnInterrupted { task_id, turn_id } => {
            let timeline = state.timelines.entry(task_id.clone()).or_default();
            if timeline.active_turn_id.as_deref() == Some(turn_id.as_str()) {
                timeline.active_turn_id = None;
            }
            if let Some(task) = state.tasks.iter_mut().find(|task| task.id == task_id) {
                task.status = TaskRunStatus::Interrupted;
            }
            Vec::new()
        }
        Action::TimelineDelta {
            task_id,
            turn_id,
            item_id,
            kind,
            delta,
        } => {
            let timeline = state.timelines.entry(task_id).or_default();
            if let Some(item) = timeline.items.iter_mut().find(|item| item.id == item_id) {
                append_bounded(&mut item.text, &delta, MAX_COMPOSER_BYTES);
            } else {
                timeline.items.push(TimelineItem {
                    id: item_id,
                    turn_id,
                    kind,
                    text: bounded_string(delta, MAX_COMPOSER_BYTES),
                    completed: false,
                });
                trim_front(&mut timeline.items, MAX_TIMELINE_ITEMS);
            }
            Vec::new()
        }
        Action::UpsertTimelineItem { task_id, item } => {
            let timeline = state.timelines.entry(task_id).or_default();
            if let Some(existing) = timeline
                .items
                .iter_mut()
                .find(|existing| existing.id == item.id)
            {
                *existing = item;
            } else {
                timeline.items.push(item);
                trim_front(&mut timeline.items, MAX_TIMELINE_ITEMS);
            }
            Vec::new()
        }
        Action::TimelineItemCompleted { task_id, item_id } => {
            if let Some(item) = state
                .timelines
                .entry(task_id)
                .or_default()
                .items
                .iter_mut()
                .find(|item| item.id == item_id)
            {
                item.completed = true;
            }
            Vec::new()
        }
        Action::ApprovalRequested(request) => {
            if state.approvals.len() == MAX_PENDING_APPROVALS {
                state.approvals.pop_front();
                state.status_message =
                    Some("Oldest pending approval was dropped at the queue limit.".to_owned());
            }
            if let Some(task) = state
                .tasks
                .iter_mut()
                .find(|task| task.id == request.task_id)
            {
                task.status = TaskRunStatus::WaitingForApproval;
            }
            state.approvals.push_back(request);
            Vec::new()
        }
        Action::ResolveApproval {
            request_id,
            decision,
        } => {
            let position = state
                .approvals
                .iter()
                .position(|request| request.request_id == request_id);
            if let Some(position) = position {
                state.approvals.remove(position);
                vec![Effect::RespondApproval {
                    request_id,
                    decision,
                }]
            } else {
                Vec::new()
            }
        }
        Action::ComputerUseAvailable { task_id } => {
            state
                .computer_use
                .entry(task_id)
                .or_default()
                .available_for_task = true;
            Vec::new()
        }
        Action::ToggleComputerUse { task_id, enabled } => {
            let computer_use = state.computer_use.entry(task_id.clone()).or_default();
            if !computer_use.available_for_task {
                state.status_message =
                    Some("Create a new task before enabling Computer Use.".to_owned());
                return Vec::new();
            }
            computer_use.enabled_for_task = enabled;
            if !enabled {
                computer_use.input_authorized_for_session = false;
                computer_use.selected_window_id = None;
                computer_use.selected_window_title = None;
                computer_use.windows.clear();
                computer_use.windows_loading = false;
                computer_use.error = None;
            } else {
                computer_use.windows_loading = true;
                computer_use.error = None;
            }
            let mut effects = vec![Effect::ConfigureComputerUse {
                task_id: task_id.clone(),
                enabled,
                selected_window_id: computer_use.selected_window_id.clone(),
                input_authorized: computer_use.input_authorized_for_session,
            }];
            if enabled {
                effects.push(Effect::LoadComputerWindows { task_id });
            }
            effects
        }
        Action::SelectComputerUseWindow {
            task_id,
            window_id,
            title,
        } => {
            let computer_use = state.computer_use.entry(task_id.clone()).or_default();
            if computer_use.available_for_task && computer_use.enabled_for_task {
                computer_use.selected_window_id = Some(window_id);
                computer_use.selected_window_title = Some(title);
                computer_use.input_authorized_for_session = false;
                return vec![Effect::ConfigureComputerUse {
                    task_id,
                    enabled: true,
                    selected_window_id: computer_use.selected_window_id.clone(),
                    input_authorized: false,
                }];
            }
            Vec::new()
        }
        Action::AuthorizeComputerInputForSession {
            task_id,
            authorized,
        } => {
            let computer_use = state.computer_use.entry(task_id.clone()).or_default();
            computer_use.input_authorized_for_session = authorized
                && computer_use.available_for_task
                && computer_use.enabled_for_task
                && computer_use.selected_window_id.is_some();
            vec![Effect::ConfigureComputerUse {
                task_id,
                enabled: computer_use.enabled_for_task,
                selected_window_id: computer_use.selected_window_id.clone(),
                input_authorized: computer_use.input_authorized_for_session,
            }]
        }
        Action::RefreshComputerWindows { task_id } => {
            let computer_use = state.computer_use.entry(task_id.clone()).or_default();
            if computer_use.available_for_task && computer_use.enabled_for_task {
                computer_use.windows_loading = true;
                computer_use.error = None;
                vec![Effect::LoadComputerWindows { task_id }]
            } else {
                Vec::new()
            }
        }
        Action::ComputerWindowsLoaded {
            task_id,
            mut windows,
        } => {
            windows.truncate(MAX_COMPUTER_WINDOWS);
            let computer_use = state.computer_use.entry(task_id).or_default();
            computer_use.windows = windows;
            computer_use.windows_loading = false;
            computer_use.error = None;
            Vec::new()
        }
        Action::ComputerWindowsFailed { task_id, message } => {
            let computer_use = state.computer_use.entry(task_id).or_default();
            computer_use.windows_loading = false;
            computer_use.error = Some(message);
            Vec::new()
        }
        Action::CaptureComputerWindow { task_id } => {
            let computer_use = state.computer_use.entry(task_id.clone()).or_default();
            if computer_use.available_for_task && computer_use.enabled_for_task {
                computer_use
                    .selected_window_id
                    .clone()
                    .map(|window_id| vec![Effect::CaptureComputerWindow { task_id, window_id }])
                    .unwrap_or_default()
            } else {
                Vec::new()
            }
        }
        Action::ComputerCaptureReady { task_id, label } => {
            let computer_use = state.computer_use.entry(task_id).or_default();
            if computer_use.enabled_for_task {
                computer_use.last_capture_label = Some(label);
            }
            Vec::new()
        }
        Action::RefreshMarketplace => {
            state.marketplace.status = Some(LoadStatus::Loading);
            vec![Effect::RefreshMarketplace]
        }
        Action::MarketplaceLoaded(mut plugins) => {
            plugins.truncate(MAX_MARKETPLACE_ITEMS);
            state.marketplace.plugins = plugins;
            state.marketplace.status = Some(LoadStatus::Ready);
            state.marketplace.errors.clear();
            Vec::new()
        }
        Action::MarketplaceFailed(message) => {
            state.marketplace.status = Some(LoadStatus::Failed);
            state.marketplace.errors = vec![message];
            Vec::new()
        }
        Action::MarketplaceQueryChanged(query) => {
            state.marketplace.query = bounded_string(query, 4 * 1024);
            Vec::new()
        }
        Action::InstallPlugin {
            plugin_id,
            marketplace,
        } => vec![Effect::InstallPlugin {
            plugin_id,
            marketplace,
        }],
        Action::UninstallPlugin { plugin_id } => vec![Effect::UninstallPlugin { plugin_id }],
        Action::PluginMutationFinished {
            plugin_id,
            installed,
        } => {
            if let Some(plugin) = state
                .marketplace
                .plugins
                .iter_mut()
                .find(|plugin| plugin.id == plugin_id)
            {
                plugin.installed = installed;
                plugin.enabled = installed;
            }
            Vec::new()
        }
        Action::RefreshGit => state
            .selected_task_id
            .as_deref()
            .and_then(|task_id| state.tasks.iter().find(|task| task.id == task_id))
            .map(|task| {
                vec![Effect::RefreshGit {
                    cwd: task.cwd.clone(),
                }]
            })
            .unwrap_or_else(|| {
                state.status_message = Some("Select a task before refreshing Git.".to_owned());
                Vec::new()
            }),
        Action::GitSnapshotLoaded(git) => {
            state.git = git;
            Vec::new()
        }
        Action::SelectDiffPath(path) => {
            let Some(root) = state.git.repository_root.clone() else {
                state.status_message = Some("No Git repository is selected.".to_owned());
                return Vec::new();
            };
            state.git.selected_path = Some(path.clone());
            state.git.diff_generation = state.git.diff_generation.saturating_add(1);
            vec![Effect::LoadDiff {
                generation: state.git.diff_generation,
                root,
                path,
            }]
        }
        Action::StagePath(path) => state
            .git
            .repository_root
            .clone()
            .map(|root| vec![Effect::StagePath { root, path }])
            .unwrap_or_default(),
        Action::UnstagePath(path) => state
            .git
            .repository_root
            .clone()
            .map(|root| vec![Effect::UnstagePath { root, path }])
            .unwrap_or_default(),
        Action::SwitchGitBranch(branch) => {
            let branch = branch.trim().to_owned();
            if branch.is_empty() || branch.len() > MAX_GIT_BRANCH_BYTES {
                state.status_message = Some("Enter a valid Git branch name.".to_owned());
                Vec::new()
            } else {
                state
                    .git
                    .repository_root
                    .clone()
                    .map(|root| vec![Effect::SwitchGitBranch { root, branch }])
                    .unwrap_or_default()
            }
        }
        Action::CreateGitWorktree {
            path,
            branch,
            create_branch,
        } => {
            let branch = branch.trim().to_owned();
            if branch.is_empty()
                || branch.len() > MAX_GIT_BRANCH_BYTES
                || path.as_os_str().is_empty()
            {
                state.status_message =
                    Some("Enter a branch and an absolute worktree path.".to_owned());
                Vec::new()
            } else {
                state
                    .git
                    .repository_root
                    .clone()
                    .map(|root| {
                        vec![Effect::CreateGitWorktree {
                            root,
                            path,
                            branch,
                            create_branch,
                        }]
                    })
                    .unwrap_or_default()
            }
        }
        Action::DiffLoaded {
            generation,
            text,
            truncated,
        } => {
            if generation == state.git.diff_generation {
                state.git.unified_diff = text;
                state.git.truncated |= truncated;
            }
            Vec::new()
        }
        Action::SpawnTerminal => {
            if state.terminal.running {
                Vec::new()
            } else {
                state
                    .selected_task_id
                    .as_deref()
                    .and_then(|task_id| state.tasks.iter().find(|task| task.id == task_id))
                    .map(|task| {
                        vec![Effect::SpawnTerminal {
                            cwd: task.cwd.clone(),
                        }]
                    })
                    .unwrap_or_else(|| {
                        state.status_message =
                            Some("Select a task before opening a terminal.".to_owned());
                        Vec::new()
                    })
            }
        }
        Action::TerminalStarted { process_id, title } => {
            state.terminal.process_id = Some(process_id);
            state.terminal.title = title;
            state.terminal.running = true;
            state.terminal.output.clear();
            state.terminal.truncated = false;
            state.terminal.exit_code = None;
            Vec::new()
        }
        Action::TerminalScreen(screen) => {
            const MAX_TERMINAL_BYTES: usize = 4 * 1024 * 1024;
            state.terminal.output = bounded_string(screen, MAX_TERMINAL_BYTES);
            state.terminal.truncated |= state.terminal.output.len() == MAX_TERMINAL_BYTES;
            Vec::new()
        }
        Action::TerminalOutputTruncated => {
            state.terminal.truncated = true;
            Vec::new()
        }
        Action::SubmitTerminalInput(input) => {
            const MAX_TERMINAL_INPUT_BYTES: usize = 64 * 1024;
            if state.terminal.running && input.len() <= MAX_TERMINAL_INPUT_BYTES {
                vec![Effect::WriteTerminal { input }]
            } else {
                if input.len() > MAX_TERMINAL_INPUT_BYTES {
                    state.status_message = Some("Terminal input exceeds 64 KiB.".to_owned());
                }
                Vec::new()
            }
        }
        Action::StopTerminal => {
            if state.terminal.running {
                vec![Effect::StopTerminal]
            } else {
                Vec::new()
            }
        }
        Action::TerminalExited { code } => {
            state.terminal.running = false;
            state.terminal.process_id = None;
            state.terminal.exit_code = Some(code);
            Vec::new()
        }
        Action::SetStatus(message) => {
            state.status_message = Some(bounded_string(message, 16 * 1024));
            Vec::new()
        }
        Action::ClearStatus => {
            state.status_message = None;
            Vec::new()
        }
    }
}

fn trim_front<T>(items: &mut Vec<T>, limit: usize) {
    let overflow = items.len().saturating_sub(limit);
    if overflow > 0 {
        items.drain(..overflow);
    }
}

fn append_bounded(target: &mut String, delta: &str, limit: usize) {
    if target.len() >= limit {
        return;
    }
    let remaining = limit - target.len();
    let boundary = delta
        .char_indices()
        .map(|(index, _)| index)
        .take_while(|index| *index <= remaining)
        .last()
        .unwrap_or(0);
    let end = if delta.len() <= remaining {
        delta.len()
    } else {
        boundary
    };
    target.push_str(&delta[..end]);
}

fn bounded_string(mut value: String, limit: usize) -> String {
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        Action, AppState, ComputerUseState, Effect, LoadStatus, MainRoute, TaskRunStatus,
        TaskSummary, reduce, stable_reference,
    };

    fn task(id: &str) -> TaskSummary {
        TaskSummary {
            id: id.to_owned(),
            title: "Task".to_owned(),
            preview: String::new(),
            cwd: PathBuf::from("C:\\repo"),
            updated_at: 0,
            parent_task_id: None,
            forked_from_id: None,
            status: TaskRunStatus::Idle,
        }
    }

    #[test]
    fn stable_reference_is_pinned() {
        assert_eq!(stable_reference().package_version, "26.721.3996.0");
        assert_eq!(stable_reference().cli_version, "0.146.0-alpha.3.1");
    }

    #[test]
    fn resolved_runtime_paths_are_available_to_settings_without_file_access() {
        let mut state = AppState::default();

        assert!(
            reduce(
                &mut state,
                Action::RuntimeResolved {
                    codex_binary: PathBuf::from("C:\\tools\\codex.exe"),
                    codex_home: PathBuf::from("C:\\isolated-codex-home"),
                    codex_home_default: false,
                }
            )
            .is_empty()
        );
        assert_eq!(
            state.runtime.codex_binary,
            Some(PathBuf::from("C:\\tools\\codex.exe"))
        );
        assert_eq!(
            state.runtime.codex_home,
            Some(PathBuf::from("C:\\isolated-codex-home"))
        );
        assert!(!state.runtime.codex_home_default);
    }

    #[test]
    fn selecting_an_unloaded_task_requests_one_timeline_page() {
        let mut state = AppState::default();
        state.tasks.push(task("t1"));

        let effects = reduce(&mut state, Action::SelectTask("t1".to_owned()));

        assert_eq!(
            effects,
            [
                Effect::LoadTimeline {
                    task_id: "t1".to_owned(),
                    generation: 1,
                    cursor: None,
                },
                Effect::RememberWorkspace {
                    path: PathBuf::from("C:\\repo"),
                },
                Effect::RefreshGit {
                    cwd: PathBuf::from("C:\\repo"),
                },
            ]
        );
        assert_eq!(
            state.timelines.get("t1").map(|timeline| timeline.status),
            Some(LoadStatus::Loading)
        );
    }

    #[test]
    fn navigation_persists_only_codexrs_owned_ui_state() {
        let mut state = AppState::default();

        let effects = reduce(&mut state, Action::Navigate(MainRoute::Repository));

        assert_eq!(
            effects,
            [Effect::PersistUiState {
                route: MainRoute::Repository,
                inspector: state.inspector,
            }]
        );
    }

    #[test]
    fn repository_route_is_not_restored_without_a_selected_task() {
        let mut state = AppState::default();

        reduce(
            &mut state,
            Action::StorageOpened {
                path: PathBuf::from("C:\\codexrs\\state.sqlite"),
                route: Some(MainRoute::Repository),
                inspector: None,
            },
        );

        assert_eq!(state.route, MainRoute::Tasks);
    }

    #[test]
    fn branch_mutations_are_scoped_to_the_detected_repository() {
        let mut state = AppState::default();
        state.git.repository_root = Some(PathBuf::from("C:\\repo"));

        assert_eq!(
            reduce(
                &mut state,
                Action::SwitchGitBranch("feature/native-ui".to_owned())
            ),
            [Effect::SwitchGitBranch {
                root: PathBuf::from("C:\\repo"),
                branch: "feature/native-ui".to_owned(),
            }]
        );
        assert_eq!(
            reduce(
                &mut state,
                Action::CreateGitWorktree {
                    path: PathBuf::from("C:\\repo-native-ui"),
                    branch: "feature/native-ui".to_owned(),
                    create_branch: true,
                }
            ),
            [Effect::CreateGitWorktree {
                root: PathBuf::from("C:\\repo"),
                path: PathBuf::from("C:\\repo-native-ui"),
                branch: "feature/native-ui".to_owned(),
                create_branch: true,
            }]
        );
    }

    #[test]
    fn stale_task_page_cannot_replace_a_newer_refresh() {
        let mut state = AppState::default();
        reduce(&mut state, Action::RefreshTasks);
        reduce(&mut state, Action::RefreshTasks);

        reduce(
            &mut state,
            Action::TasksLoaded {
                generation: 1,
                tasks: vec![task("stale")],
                next_cursor: None,
                append: false,
            },
        );

        assert!(state.tasks.is_empty());
        assert_eq!(state.task_generation, 2);
    }

    #[test]
    fn computer_input_requires_a_capable_task_and_selected_window() {
        let mut state = AppState::default();

        reduce(
            &mut state,
            Action::AuthorizeComputerInputForSession {
                task_id: "t1".to_owned(),
                authorized: true,
            },
        );
        assert_eq!(
            state.computer_use.get("t1"),
            Some(&ComputerUseState::default())
        );

        reduce(&mut state, Action::TaskCreated(task("t1")));
        reduce(
            &mut state,
            Action::ComputerUseAvailable {
                task_id: "t1".to_owned(),
            },
        );
        reduce(
            &mut state,
            Action::ToggleComputerUse {
                task_id: "t1".to_owned(),
                enabled: true,
            },
        );
        reduce(
            &mut state,
            Action::SelectComputerUseWindow {
                task_id: "t1".to_owned(),
                window_id: "42".to_owned(),
                title: "Editor".to_owned(),
            },
        );
        reduce(
            &mut state,
            Action::AuthorizeComputerInputForSession {
                task_id: "t1".to_owned(),
                authorized: true,
            },
        );
        assert!(
            state
                .computer_use
                .get("t1")
                .is_some_and(|state| state.input_authorized_for_session)
        );
    }

    #[test]
    fn refresh_preserves_a_selected_new_task_until_app_server_persists_it() {
        let mut state = AppState::default();
        reduce(&mut state, Action::TaskCreated(task("new")));
        reduce(&mut state, Action::RefreshTasks);

        reduce(
            &mut state,
            Action::TasksLoaded {
                generation: 1,
                tasks: Vec::new(),
                next_cursor: None,
                append: false,
            },
        );

        assert_eq!(
            state.tasks.first().map(|task| task.id.as_str()),
            Some("new")
        );
        assert_eq!(state.selected_task_id.as_deref(), Some("new"));
    }
}
