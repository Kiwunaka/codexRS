use std::ops::Range;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use codex_core::{
    Action, AppState, ApprovalDecision, ConnectionStatus, GitFileKind, InspectorPane, MainRoute,
    TaskRunStatus, TimelineKind, reduce,
};
use gpui::{
    AnyElement, App, Application, Bounds, Context, Entity, IntoElement, Render, SharedString,
    Subscription, Task, Timer, Window, WindowBounds, WindowOptions, div, prelude::*, px, size,
    uniform_list,
};
use gpui_component::{
    ActiveTheme, Disableable, Root, Selectable, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    scroll::ScrollableElement,
    v_flex,
};

use crate::backend::Backend;

const WINDOW_WIDTH: f32 = 1_440.0;
const WINDOW_HEIGHT: f32 = 920.0;
const SIDEBAR_WIDTH: f32 = 300.0;
const INSPECTOR_WIDTH: f32 = 360.0;
const POLL_INTERVAL: Duration = Duration::from_millis(16);
const MAX_EVENTS_PER_TICK: usize = 128;
const MAX_RENDERED_DIFF_LINES: usize = 20_000;

pub fn run() {
    Application::new().run(|cx: &mut App| {
        gpui_component::init(cx);
        let bounds = Bounds::centered(None, size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)), cx);
        let result = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(960.0), px(640.0))),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("codexRS".into()),
                    ..Default::default()
                }),
                app_id: Some("dev.codexrs.desktop".to_owned()),
                ..Default::default()
            },
            |window, cx| {
                let workspace = cx.new(|cx| WorkspaceView::new(window, cx));
                cx.new(|cx| Root::new(workspace, window, cx))
            },
        );
        if let Err(error) = result {
            eprintln!("codex-rs: failed to open window: {error}");
            cx.quit();
            return;
        }
        cx.activate(true);
    });
}

struct WorkspaceView {
    state: AppState,
    backend: Option<Backend>,
    composer: Entity<InputState>,
    terminal_input: Entity<InputState>,
    task_search: Entity<InputState>,
    plugin_search: Entity<InputState>,
    worktree_branch: Entity<InputState>,
    worktree_path: Entity<InputState>,
    task_query: String,
    _subscriptions: Vec<Subscription>,
    _poll_task: Task<()>,
}

impl WorkspaceView {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let composer = cx.new(|cx| {
            InputState::new(window, cx)
                .auto_grow(3, 10)
                .placeholder("Ask Codex to change, inspect, or explain…")
        });
        let task_search = cx.new(|cx| InputState::new(window, cx).placeholder("Search tasks…"));
        let plugin_search = cx.new(|cx| InputState::new(window, cx).placeholder("Search plugins…"));
        let terminal_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Enter a shell command…"));
        let worktree_branch = cx.new(|cx| InputState::new(window, cx).placeholder("Branch name"));
        let worktree_path = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Worktree path (blank = sibling folder)")
        });

        let subscriptions = vec![
            cx.subscribe_in(
                &composer,
                window,
                |this, input, event: &InputEvent, window, cx| match event {
                    InputEvent::Change => {
                        let value = input.read(cx).value().to_string();
                        this.dispatch(Action::ComposerChanged(value), cx);
                    }
                    InputEvent::PressEnter { secondary: true } => {
                        this.submit(window, cx);
                    }
                    _ => {}
                },
            ),
            cx.subscribe(&task_search, |this, input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.task_query = input.read(cx).value().to_string();
                    cx.notify();
                }
            }),
            cx.subscribe(&plugin_search, |this, input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.dispatch(
                        Action::MarketplaceQueryChanged(input.read(cx).value().to_string()),
                        cx,
                    );
                }
            }),
            cx.subscribe_in(
                &terminal_input,
                window,
                |this, input, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::PressEnter { .. }) {
                        let value = input.read(cx).value().trim().to_owned();
                        if !value.is_empty() {
                            this.dispatch(Action::SubmitTerminalInput(value), cx);
                            input.update(cx, |input, cx| {
                                input.set_value("", window, cx);
                            });
                        }
                    }
                },
            ),
        ];

        let backend = match Backend::spawn() {
            Ok(backend) => Some(backend),
            Err(error) => {
                let mut state = AppState::default();
                let _ = reduce(&mut state, Action::ConnectionFailed(error));
                None
            }
        };
        let mut state = AppState::default();
        if backend.is_some() {
            let effects = reduce(&mut state, Action::Connect);
            if let Some(backend) = backend.as_ref() {
                for effect in effects {
                    let _ = backend.send(effect);
                }
            }
        }

        let poll_task = cx.spawn(async move |view, cx| {
            loop {
                Timer::after(POLL_INTERVAL).await;
                let keep_running = cx
                    .update(|cx| {
                        let Some(view) = view.upgrade() else {
                            return false;
                        };
                        view.update(cx, |view, cx| view.drain_backend(cx));
                        true
                    })
                    .unwrap_or(false);
                if !keep_running {
                    break;
                }
            }
        });

        Self {
            state,
            backend,
            composer,
            terminal_input,
            task_search,
            plugin_search,
            worktree_branch,
            worktree_path,
            task_query: String::new(),
            _subscriptions: subscriptions,
            _poll_task: poll_task,
        }
    }

    fn dispatch(&mut self, action: Action, cx: &mut Context<Self>) {
        let effects = reduce(&mut self.state, action);
        for effect in effects {
            let result = self
                .backend
                .as_ref()
                .ok_or("backend is unavailable")
                .and_then(|backend| backend.send(effect));
            if let Err(error) = result {
                let _ = reduce(&mut self.state, Action::SetStatus(error.to_owned()));
            }
        }
        cx.notify();
    }

    fn drain_backend(&mut self, cx: &mut Context<Self>) {
        let Some(backend) = self.backend.as_ref() else {
            return;
        };
        let mut actions = Vec::new();
        for _ in 0..MAX_EVENTS_PER_TICK {
            match backend.try_recv() {
                Ok(Some(action)) => actions.push(action),
                Ok(None) => break,
                Err(error) => {
                    actions.push(Action::ConnectionFailed(error.to_owned()));
                    break;
                }
            }
        }
        for action in actions {
            self.dispatch(action, cx);
        }
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.composer.read(cx).value().to_string();
        self.dispatch(Action::ComposerChanged(value), cx);
        self.dispatch(Action::SubmitComposer, cx);
        if self.state.composer.is_empty() {
            self.composer.update(cx, |input, cx| {
                input.set_value("", window, cx);
            });
        }
    }

    fn navigate(&mut self, route: MainRoute, cx: &mut Context<Self>) {
        self.dispatch(Action::Navigate(route), cx);
    }

    fn render_sidebar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let route = self.state.route;
        let task_indices = Rc::new(self.filtered_task_indices());
        let task_count = task_indices.len();

        v_flex()
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .flex_shrink_0()
            .bg(cx.theme().sidebar)
            .border_r_1()
            .border_color(cx.theme().sidebar_border)
            .child(
                h_flex()
                    .h(px(62.0))
                    .px_4()
                    .gap_3()
                    .items_center()
                    .border_b_1()
                    .border_color(cx.theme().sidebar_border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(34.0))
                            .rounded_lg()
                            .bg(cx.theme().primary)
                            .text_color(cx.theme().primary_foreground)
                            .font_weight(gpui::FontWeight::BOLD)
                            .child("RS"),
                    )
                    .child(
                        v_flex()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child("codexRS"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Native agent workspace"),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .p_2()
                    .gap_1()
                    .border_b_1()
                    .border_color(cx.theme().sidebar_border)
                    .child(
                        Button::new("nav-tasks")
                            .label("Tasks")
                            .ghost()
                            .small()
                            .selected(route == MainRoute::Tasks)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.navigate(MainRoute::Tasks, cx);
                            })),
                    )
                    .child(
                        Button::new("nav-repository")
                            .label("Repo")
                            .ghost()
                            .small()
                            .selected(route == MainRoute::Repository)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.navigate(MainRoute::Repository, cx);
                            })),
                    )
                    .child(
                        Button::new("nav-marketplace")
                            .label("Market")
                            .ghost()
                            .small()
                            .selected(route == MainRoute::Marketplace)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.navigate(MainRoute::Marketplace, cx);
                            })),
                    )
                    .child(
                        Button::new("nav-settings")
                            .label("Settings")
                            .ghost()
                            .small()
                            .selected(route == MainRoute::Settings)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.navigate(MainRoute::Settings, cx);
                            })),
                    ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .p_2()
                    .gap_2()
                    .when(route == MainRoute::Tasks, |sidebar| {
                        sidebar.child(Input::new(&self.task_search).small().cleanable(true))
                    })
                    .when(route == MainRoute::Marketplace, |sidebar| {
                        sidebar.child(Input::new(&self.plugin_search).small().cleanable(true))
                    })
                    .when(route == MainRoute::Tasks, |sidebar| {
                        sidebar
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .px_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(format!("{} TASKS", task_count)),
                                    )
                                    .child(
                                        Button::new("refresh-tasks")
                                            .label("Refresh")
                                            .xsmall()
                                            .ghost()
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.dispatch(Action::RefreshTasks, cx);
                                            })),
                                    ),
                            )
                            .child(
                                uniform_list(
                                    "task-list",
                                    task_count,
                                    cx.processor(move |this, range: Range<usize>, _, cx| {
                                        range
                                            .filter_map(|row| task_indices.get(row).copied())
                                            .map(|index| this.render_task(index, cx))
                                            .collect()
                                    }),
                                )
                                .flex_1()
                                .min_h_0(),
                            )
                            .child(
                                Button::new("new-task")
                                    .label("+ New task")
                                    .primary()
                                    .w_full()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.dispatch(Action::NewTask, cx);
                                    })),
                            )
                    })
                    .when(route != MainRoute::Tasks, |sidebar| {
                        sidebar.child(
                            div()
                                .p_3()
                                .rounded_lg()
                                .bg(cx.theme().muted)
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(match route {
                                    MainRoute::Repository => {
                                        "Branches, worktrees, changed files, and bounded diffs."
                                    }
                                    MainRoute::Marketplace => {
                                        "Install plugins through the official app-server contract."
                                    }
                                    MainRoute::Settings => {
                                        "Runtime paths, compatibility, safety, and platform details."
                                    }
                                    MainRoute::Tasks => "",
                                }),
                        )
                    }),
            )
            .child(self.render_connection_footer(cx))
            .into_any_element()
    }

    fn render_task(&mut self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(task) = self.state.tasks.get(index) else {
            return div().into_any_element();
        };
        let selected = self.state.selected_task_id.as_deref() == Some(task.id.as_str());
        let task_id = task.id.clone();
        let title = task.title.clone();
        let preview = task.preview.clone();
        let cwd = task
            .cwd
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_owned();
        let (status, status_color) = task_status(task.status, cx);

        v_flex()
            .id(SharedString::from(format!("task-{}", task.id)))
            .h(px(82.0))
            .p_2()
            .gap_1()
            .rounded_lg()
            .cursor_pointer()
            .when(selected, |item| {
                item.bg(cx.theme().sidebar_accent)
                    .border_1()
                    .border_color(cx.theme().list_active_border)
            })
            .when(!selected, |item| {
                item.hover(|style| style.bg(cx.theme().list_hover))
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.dispatch(Action::SelectTask(task_id.clone()), cx);
            }))
            .child(
                h_flex()
                    .gap_2()
                    .justify_between()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .truncate()
                            .child(title),
                    )
                    .child(div().text_xs().text_color(status_color).child(status)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .truncate()
                    .child(if preview.trim().is_empty() {
                        "No messages yet".to_owned()
                    } else {
                        preview.replace('\n', " ")
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .truncate()
                    .child(cwd),
            )
            .into_any_element()
    }

    fn render_connection_footer(&self, cx: &mut Context<Self>) -> AnyElement {
        let (label, color) = match &self.state.connection {
            ConnectionStatus::Offline => ("Offline", cx.theme().muted_foreground),
            ConnectionStatus::Connecting => ("Connecting…", cx.theme().info),
            ConnectionStatus::Online => ("App-server online", cx.theme().success),
            ConnectionStatus::Recovering => ("Reconnecting…", cx.theme().warning),
            ConnectionStatus::Failed(_) => ("Connection failed", cx.theme().danger),
        };
        h_flex()
            .h(px(42.0))
            .px_4()
            .gap_2()
            .items_center()
            .border_t_1()
            .border_color(cx.theme().sidebar_border)
            .child(div().size(px(7.0)).rounded_full().bg(color))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(label),
            )
            .into_any_element()
    }

    fn render_main(&mut self, cx: &mut Context<Self>) -> AnyElement {
        match self.state.route {
            MainRoute::Tasks => self.render_task_workspace(cx),
            MainRoute::Repository => self.render_repository(cx),
            MainRoute::Marketplace => self.render_marketplace(cx),
            MainRoute::Settings => self.render_settings(cx),
        }
    }

    fn render_repository(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let repository_root = self.state.git.repository_root.clone();
        let file_count = self.state.git.files.len();
        let branch_count = self.state.git.branches.len();
        let worktree_count = self.state.git.worktrees.len();
        let branch_list_height = (branch_count.clamp(1, 4) as f32) * 42.0;
        let worktree_list_height = (worktree_count.clamp(1, 3) as f32) * 54.0;
        let selected_file = self
            .state
            .git
            .selected_path
            .as_ref()
            .and_then(|selected| {
                self.state
                    .git
                    .files
                    .iter()
                    .find(|file| file.path == *selected)
            })
            .cloned();
        let stage_path = selected_file
            .as_ref()
            .filter(|file| file.unstaged)
            .map(|file| file.path.clone());
        let unstage_path = selected_file
            .as_ref()
            .filter(|file| file.staged)
            .map(|file| file.path.clone());
        let diff_lines = Rc::new(
            self.state
                .git
                .unified_diff
                .lines()
                .take(MAX_RENDERED_DIFF_LINES)
                .map(str::to_owned)
                .collect::<Vec<_>>(),
        );
        let diff_line_count = diff_lines.len();
        let diff_lines_for_list = Rc::clone(&diff_lines);

        v_flex()
            .flex_1()
            .h_full()
            .min_w_0()
            .child(
                h_flex()
                    .h(px(62.0))
                    .px_6()
                    .gap_4()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        v_flex()
                            .min_w_0()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .child("Repository"),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py_0p5()
                                            .rounded_md()
                                            .bg(cx.theme().muted)
                                            .font_family(cx.theme().mono_font_family.clone())
                                            .text_xs()
                                            .child(
                                                self.state
                                                    .git
                                                    .branch
                                                    .clone()
                                                    .unwrap_or_else(|| "no branch".to_owned()),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(
                                        repository_root
                                            .as_ref()
                                            .map_or_else(
                                                || "Select a task backed by a Git repository.".to_owned(),
                                                |root| display_path(root),
                                            ),
                                    ),
                            ),
                    )
                    .child(
                        Button::new("repository-refresh")
                            .label("Refresh")
                            .small()
                            .disabled(repository_root.is_none())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.dispatch(Action::RefreshGit, cx);
                            })),
                    ),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .p_5()
                    .gap_4()
                    .child(
                        v_flex()
                            .w(px(400.0))
                            .h_full()
                            .flex_shrink_0()
                            .min_h_0()
                            .gap_3()
                            .overflow_y_scrollbar()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(metric_badge(
                                        "Changed",
                                        self.state.git.changed_files,
                                        cx,
                                    ))
                                    .child(metric_badge(
                                        "Staged",
                                        self.state.git.staged_files,
                                        cx,
                                    ))
                                    .child(metric_badge("Branches", branch_count, cx))
                                    .child(metric_badge("Worktrees", worktree_count, cx)),
                            )
                            .child(section_label("CHANGED FILES", cx))
                            .child(if file_count == 0 {
                                div()
                                    .h(px(72.0))
                                    .p_3()
                                    .rounded_lg()
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .bg(cx.theme().background)
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(if repository_root.is_some() {
                                        "Working tree is clean."
                                    } else {
                                        "No Git repository detected."
                                    })
                                    .into_any_element()
                            } else {
                                uniform_list(
                                    "repository-files",
                                    file_count,
                                    cx.processor(|this, range: Range<usize>, _, cx| {
                                        range
                                            .map(|index| this.render_git_file(index, cx))
                                            .collect()
                                    }),
                                )
                                .h(px(220.0))
                                .rounded_lg()
                                .border_1()
                                .border_color(cx.theme().border)
                                .bg(cx.theme().background)
                                .into_any_element()
                            })
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        Button::new("repository-stage")
                                            .label("Stage")
                                            .small()
                                            .disabled(stage_path.is_none())
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                if let Some(path) = stage_path.clone() {
                                                    this.dispatch(Action::StagePath(path), cx);
                                                }
                                            })),
                                    )
                                    .child(
                                        Button::new("repository-unstage")
                                            .label("Unstage")
                                            .small()
                                            .ghost()
                                            .disabled(unstage_path.is_none())
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                if let Some(path) = unstage_path.clone() {
                                                    this.dispatch(Action::UnstagePath(path), cx);
                                                }
                                            })),
                                    ),
                            )
                            .child(section_label("BRANCHES", cx))
                            .child(
                                uniform_list(
                                    "repository-branches",
                                    branch_count,
                                    cx.processor(|this, range: Range<usize>, _, cx| {
                                        range
                                            .map(|index| this.render_repository_branch(index, cx))
                                            .collect()
                                    }),
                                )
                                .h(px(branch_list_height))
                                .rounded_lg()
                                .border_1()
                                .border_color(cx.theme().border)
                                .bg(cx.theme().background),
                            )
                            .child(section_label("WORKTREES", cx))
                            .child(
                                uniform_list(
                                    "repository-worktrees",
                                    worktree_count,
                                    cx.processor(|this, range: Range<usize>, _, cx| {
                                        range
                                            .map(|index| {
                                                this.render_repository_worktree(index, cx)
                                            })
                                            .collect()
                                    }),
                                )
                                .h(px(worktree_list_height))
                                .rounded_lg()
                                .border_1()
                                .border_color(cx.theme().border)
                                .bg(cx.theme().background),
                            )
                            .child(section_label("NEW WORKTREE", cx))
                            .child(Input::new(&self.worktree_branch).small().cleanable(true))
                            .child(Input::new(&self.worktree_path).small().cleanable(true))
                            .child(
                                Button::new("repository-create-worktree")
                                    .label("Create worktree")
                                    .primary()
                                    .small()
                                    .disabled(repository_root.is_none())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.create_worktree(cx);
                                    })),
                            )
                            .child(
                                div()
                                    .pb_3()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(
                                        "An unknown branch name creates a new branch. Existing branches are attached without force.",
                                    ),
                            ),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .h_full()
                            .min_w_0()
                            .min_h_0()
                            .gap_2()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .child(section_label("BOUNDED UNIFIED DIFF", cx))
                                    .when(self.state.git.truncated, |header| {
                                        header.child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().warning)
                                                .child("Output truncated at the safety limit"),
                                        )
                                    }),
                            )
                            .child(if diff_line_count == 0 {
                                div()
                                    .flex_1()
                                    .min_h_0()
                                    .p_5()
                                    .rounded_lg()
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .bg(cx.theme().background)
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(
                                        "Select a changed file. Staged and working-tree hunks are shown separately.",
                                    )
                                    .into_any_element()
                            } else {
                                uniform_list(
                                    "repository-diff",
                                    diff_line_count,
                                    cx.processor(
                                        move |this, range: Range<usize>, _, cx| {
                                            range
                                                .filter_map(|index| {
                                                    diff_lines_for_list
                                                        .get(index)
                                                        .cloned()
                                                        .map(|line| (index, line))
                                                })
                                                .map(|(index, line)| {
                                                    this.render_diff_line(index, line, cx)
                                                })
                                                .collect()
                                        },
                                    ),
                                )
                                .flex_1()
                                .min_h_0()
                                .rounded_lg()
                                .border_1()
                                .border_color(cx.theme().border)
                                .bg(cx.theme().background)
                                .into_any_element()
                            }),
                    ),
            )
            .into_any_element()
    }

    fn render_repository_branch(&mut self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(branch) = self.state.git.branches.get(index).cloned() else {
            return div().into_any_element();
        };
        let branch_name = branch.name.clone();
        h_flex()
            .h(px(42.0))
            .px_3()
            .gap_2()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(cx.theme().border.opacity(0.65))
            .when(branch.current, |row| row.bg(cx.theme().sidebar_accent))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .truncate()
                            .font_family(cx.theme().mono_font_family.clone())
                            .text_xs()
                            .child(branch.name),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(branch.commit),
                    ),
            )
            .child(
                div().flex_shrink_0().child(
                    Button::new(SharedString::from(format!("switch-branch-{index}")))
                        .label(if branch.current { "Current" } else { "Switch" })
                        .xsmall()
                        .ghost()
                        .disabled(branch.current)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.dispatch(Action::SwitchGitBranch(branch_name.clone()), cx);
                        })),
                ),
            )
            .into_any_element()
    }

    fn render_repository_worktree(&mut self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(worktree) = self.state.git.worktrees.get(index) else {
            return div().into_any_element();
        };
        let status = if worktree.locked {
            "locked".to_owned()
        } else if worktree.detached {
            "detached".to_owned()
        } else {
            worktree
                .branch
                .clone()
                .unwrap_or_else(|| "no branch".to_owned())
        };
        v_flex()
            .h(px(54.0))
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(cx.theme().border.opacity(0.65))
            .child(
                div()
                    .truncate()
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_xs()
                    .child(display_path(&worktree.path)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(status),
            )
            .into_any_element()
    }

    fn render_diff_line(
        &mut self,
        index: usize,
        line: String,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let added = line.starts_with('+') && !line.starts_with("+++");
        let removed = line.starts_with('-') && !line.starts_with("---");
        let header = line.starts_with("@@")
            || line.starts_with("diff --git")
            || line.starts_with("index ")
            || line.starts_with("---")
            || line.starts_with("+++");
        h_flex()
            .h(px(22.0))
            .min_w_full()
            .when(added, |row| row.bg(cx.theme().success.opacity(0.09)))
            .when(removed, |row| row.bg(cx.theme().danger.opacity(0.09)))
            .when(header, |row| row.bg(cx.theme().info.opacity(0.08)))
            .child(
                div()
                    .w(px(58.0))
                    .h_full()
                    .pr_3()
                    .flex()
                    .items_center()
                    .justify_end()
                    .border_r_1()
                    .border_color(cx.theme().border.opacity(0.6))
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child((index + 1).to_string()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .px_2()
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_xs()
                    .text_color(if added {
                        cx.theme().success
                    } else if removed {
                        cx.theme().danger
                    } else if header {
                        cx.theme().info
                    } else {
                        cx.theme().foreground
                    })
                    .child(if line.is_empty() {
                        " ".to_owned()
                    } else {
                        line
                    }),
            )
            .into_any_element()
    }

    fn create_worktree(&mut self, cx: &mut Context<Self>) {
        let branch = self.worktree_branch.read(cx).value().trim().to_owned();
        let Some(root) = self.state.git.repository_root.as_deref() else {
            self.dispatch(
                Action::SetStatus("Select a Git repository first.".to_owned()),
                cx,
            );
            return;
        };
        let entered_path = self.worktree_path.read(cx).value().trim().to_owned();
        let path = if entered_path.is_empty() {
            default_worktree_path(root, &branch)
        } else {
            PathBuf::from(entered_path)
        };
        let create_branch = !self
            .state
            .git
            .branches
            .iter()
            .any(|candidate| candidate.name == branch);
        self.dispatch(
            Action::CreateGitWorktree {
                path,
                branch,
                create_branch,
            },
            cx,
        );
    }

    fn render_task_workspace(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(task_id) = self.state.selected_task_id.clone() else {
            return v_flex()
                .flex_1()
                .h_full()
                .items_center()
                .justify_center()
                .gap_3()
                .child(
                    div()
                        .text_2xl()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child("Start with a task"),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child("Select history on the left or create a new task."),
                )
                .child(
                    Button::new("empty-new-task")
                        .label("New task")
                        .primary()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.dispatch(Action::NewTask, cx);
                        })),
                )
                .into_any_element();
        };

        let task = self
            .state
            .tasks
            .iter()
            .find(|task| task.id == task_id)
            .cloned();
        let title = task
            .as_ref()
            .map_or_else(|| "Task".to_owned(), |task| task.title.clone());
        let cwd = task
            .as_ref()
            .map(|task| task.cwd.display().to_string())
            .unwrap_or_default();
        let timeline_count = self
            .state
            .timelines
            .get(&task_id)
            .map_or(0, |timeline| timeline.items.len());
        let task_id_for_list = task_id.clone();

        h_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(
                        h_flex()
                            .h(px(62.0))
                            .px_5()
                            .gap_3()
                            .items_center()
                            .justify_between()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .child(
                                v_flex()
                                    .min_w_0()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .truncate()
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .child(title),
                                    )
                                    .child(
                                        div()
                                            .truncate()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(cwd),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap_1()
                                    .child(Button::new("fork-task").label("Fork").small().on_click(
                                        cx.listener(|this, _, _, cx| {
                                            this.dispatch(Action::ForkSelectedTask, cx);
                                        }),
                                    ))
                                    .child(
                                        Button::new("refresh-git")
                                            .label("Refresh Git")
                                            .small()
                                            .ghost()
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.dispatch(Action::RefreshGit, cx);
                                            })),
                                    ),
                            ),
                    )
                    .child(
                        uniform_list(
                            "timeline",
                            timeline_count,
                            cx.processor(move |this, range: Range<usize>, _, cx| {
                                range
                                    .map(|index| {
                                        this.render_timeline_item(&task_id_for_list, index, cx)
                                    })
                                    .collect()
                            }),
                        )
                        .flex_1()
                        .min_h_0()
                        .bg(cx.theme().background),
                    )
                    .child(self.render_approval(cx))
                    .child(self.render_composer(cx)),
            )
            .child(self.render_inspector(cx))
            .into_any_element()
    }

    fn render_timeline_item(
        &mut self,
        task_id: &str,
        index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(item) = self
            .state
            .timelines
            .get(task_id)
            .and_then(|timeline| timeline.items.get(index))
        else {
            return div().into_any_element();
        };
        let (label, accent, background) = timeline_style(item.kind, cx);
        let text = if item.text.trim().is_empty() {
            format!("{label} activity")
        } else {
            item.text.clone()
        };

        h_flex()
            .h(px(148.0))
            .px_6()
            .py_3()
            .items_start()
            .child(div().w(px(3.0)).h_full().rounded_full().bg(accent))
            .child(
                v_flex()
                    .ml_3()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .p_3()
                    .gap_2()
                    .rounded_lg()
                    .bg(background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(
                        h_flex()
                            .justify_between()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(accent)
                                    .child(label),
                            )
                            .when(!item.completed, |header| {
                                header.child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child("running"),
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .text_sm()
                            .line_height(px(19.0))
                            .line_clamp(5)
                            .when(
                                matches!(
                                    item.kind,
                                    TimelineKind::Command | TimelineKind::FileChange
                                ),
                                |text| {
                                    text.font_family(cx.theme().mono_font_family.clone())
                                        .text_xs()
                                },
                            )
                            .child(text),
                    ),
            )
            .into_any_element()
    }

    fn render_approval(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(approval) = self.state.approvals.front().cloned() else {
            return div().h_0().into_any_element();
        };
        let accept_id = approval.request_id.clone();
        let session_id = approval.request_id.clone();
        let decline_id = approval.request_id.clone();

        v_flex()
            .mx_5()
            .mb_2()
            .p_3()
            .gap_2()
            .rounded_lg()
            .border_1()
            .border_color(cx.theme().warning)
            .bg(cx.theme().warning.opacity(0.10))
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(approval.title),
            )
            .child(
                div()
                    .text_xs()
                    .line_clamp(4)
                    .text_color(cx.theme().muted_foreground)
                    .child(approval.detail),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("approval-accept")
                            .label("Allow once")
                            .primary()
                            .small()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.dispatch(
                                    Action::ResolveApproval {
                                        request_id: accept_id.clone(),
                                        decision: ApprovalDecision::Accept,
                                    },
                                    cx,
                                );
                            })),
                    )
                    .child(
                        Button::new("approval-session")
                            .label("Allow session")
                            .small()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.dispatch(
                                    Action::ResolveApproval {
                                        request_id: session_id.clone(),
                                        decision: ApprovalDecision::AcceptForSession,
                                    },
                                    cx,
                                );
                            })),
                    )
                    .child(
                        Button::new("approval-decline")
                            .label("Decline")
                            .danger()
                            .small()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.dispatch(
                                    Action::ResolveApproval {
                                        request_id: decline_id.clone(),
                                        decision: ApprovalDecision::Decline,
                                    },
                                    cx,
                                );
                            })),
                    ),
            )
            .into_any_element()
    }

    fn render_composer(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let can_send = self.state.selected_task_id.is_some()
            && !self.state.composer.trim().is_empty()
            && self.state.composer_error.is_none();
        v_flex()
            .px_5()
            .pb_4()
            .pt_2()
            .gap_2()
            .border_t_1()
            .border_color(cx.theme().border)
            .child(
                h_flex()
                    .items_end()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Input::new(&self.composer).h(px(92.0))),
                    )
                    .child(
                        Button::new("send")
                            .label("Send")
                            .primary()
                            .disabled(!can_send)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.submit(window, cx);
                            })),
                    ),
            )
            .child(
                h_flex()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .text_color(
                                self.state
                                    .composer_error
                                    .as_ref()
                                    .map_or(cx.theme().muted_foreground, |_| cx.theme().danger),
                            )
                            .child(self.state.composer_error.clone().unwrap_or_else(|| {
                                "Ctrl/Cmd+Enter sends · Enter adds a line".to_owned()
                            })),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!(
                                "{} / {} KiB",
                                self.state.composer.len() / 1024,
                                codex_core::MAX_COMPOSER_BYTES / 1024
                            )),
                    ),
            )
            .into_any_element()
    }

    fn render_inspector(&mut self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .w(px(INSPECTOR_WIDTH))
            .h_full()
            .flex_shrink_0()
            .border_l_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().sidebar)
            .child(
                h_flex()
                    .h(px(62.0))
                    .px_3()
                    .gap_1()
                    .items_center()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(self.inspector_button(
                        "inspector-changes",
                        "Changes",
                        InspectorPane::Changes,
                        cx,
                    ))
                    .child(self.inspector_button(
                        "inspector-terminal",
                        "Terminal",
                        InspectorPane::Terminal,
                        cx,
                    ))
                    .child(self.inspector_button(
                        "inspector-computer",
                        "Computer",
                        InspectorPane::ComputerUse,
                        cx,
                    )),
            )
            .child(match self.state.inspector {
                InspectorPane::Changes => self.render_changes(cx),
                InspectorPane::Terminal => self.render_terminal(cx),
                InspectorPane::ComputerUse => self.render_computer_use(cx),
            })
            .into_any_element()
    }

    fn inspector_button(
        &self,
        id: &'static str,
        label: &'static str,
        pane: InspectorPane,
        cx: &mut Context<Self>,
    ) -> Button {
        Button::new(id)
            .label(label)
            .small()
            .ghost()
            .selected(self.state.inspector == pane)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.dispatch(Action::ShowInspector(pane), cx);
            }))
    }

    fn render_changes(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let file_count = self.state.git.files.len();
        let selected_file = self
            .state
            .git
            .selected_path
            .as_ref()
            .and_then(|selected| {
                self.state
                    .git
                    .files
                    .iter()
                    .find(|file| file.path == *selected)
            })
            .cloned();
        let stage_path = selected_file
            .as_ref()
            .filter(|file| file.unstaged)
            .map(|file| file.path.clone());
        let unstage_path = selected_file
            .as_ref()
            .filter(|file| file.staged)
            .map(|file| file.path.clone());

        v_flex()
            .flex_1()
            .min_h_0()
            .p_4()
            .gap_3()
            .child(section_label("GIT", cx))
            .child(
                v_flex()
                    .p_3()
                    .gap_2()
                    .rounded_lg()
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(detail_row(
                        "Branch",
                        self.state.git.branch.as_deref().unwrap_or("Not detected"),
                        cx,
                    ))
                    .child(detail_row(
                        "Changed",
                        &self.state.git.changed_files.to_string(),
                        cx,
                    ))
                    .child(detail_row(
                        "Staged",
                        &self.state.git.staged_files.to_string(),
                        cx,
                    ))
                    .child(detail_row(
                        "Ahead / behind",
                        &format!("{} / {}", self.state.git.ahead, self.state.git.behind),
                        cx,
                    ))
                    .child(detail_row(
                        "Branches / worktrees",
                        &format!(
                            "{} / {}",
                            self.state.git.branches.len(),
                            self.state.git.worktrees.len()
                        ),
                        cx,
                    )),
            )
            .child(section_label("CHANGED FILES", cx))
            .child(if file_count == 0 {
                div()
                    .h(px(52.0))
                    .px_3()
                    .items_center()
                    .rounded_lg()
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(if self.state.git.repository_root.is_some() {
                        "Working tree is clean."
                    } else {
                        "No Git repository detected."
                    })
                    .into_any_element()
            } else {
                uniform_list(
                    "git-files",
                    file_count,
                    cx.processor(|this, range: Range<usize>, _, cx| {
                        range.map(|index| this.render_git_file(index, cx)).collect()
                    }),
                )
                .h(px(176.0))
                .rounded_lg()
                .bg(cx.theme().background)
                .border_1()
                .border_color(cx.theme().border)
                .into_any_element()
            })
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("stage-selected")
                            .label("Stage")
                            .small()
                            .disabled(stage_path.is_none())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(path) = stage_path.clone() {
                                    this.dispatch(Action::StagePath(path), cx);
                                }
                            })),
                    )
                    .child(
                        Button::new("unstage-selected")
                            .label("Unstage")
                            .small()
                            .ghost()
                            .disabled(unstage_path.is_none())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(path) = unstage_path.clone() {
                                    this.dispatch(Action::UnstagePath(path), cx);
                                }
                            })),
                    ),
            )
            .child(section_label("DIFF", cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .p_3()
                    .overflow_y_scrollbar()
                    .rounded_lg()
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_xs()
                    .child(if self.state.git.unified_diff.is_empty() {
                        "Select a changed file to inspect its bounded diff.".to_owned()
                    } else {
                        self.state.git.unified_diff.clone()
                    }),
            )
            .into_any_element()
    }

    fn render_git_file(&mut self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(file) = self.state.git.files.get(index).cloned() else {
            return div().into_any_element();
        };
        let selected = self.state.git.selected_path.as_ref() == Some(&file.path);
        let path = file.path.clone();
        let label = file.path.display().to_string();
        let status = git_file_status(file.kind);
        let flags = match (file.staged, file.unstaged) {
            (true, true) => "index + tree",
            (true, false) => "staged",
            (false, true) => "working tree",
            (false, false) => "",
        };

        h_flex()
            .id(SharedString::from(format!("git-file-{index}")))
            .h(px(44.0))
            .px_2()
            .gap_2()
            .items_center()
            .cursor_pointer()
            .border_b_1()
            .border_color(cx.theme().border.opacity(0.65))
            .when(selected, |row| row.bg(cx.theme().sidebar_accent))
            .when(!selected, |row| {
                row.hover(|style| style.bg(cx.theme().list_hover))
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.dispatch(Action::SelectDiffPath(path.clone()), cx);
            }))
            .child(
                div()
                    .w(px(18.0))
                    .text_xs()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(match file.kind {
                        GitFileKind::Deleted | GitFileKind::Conflicted => cx.theme().danger,
                        GitFileKind::Added | GitFileKind::Untracked => cx.theme().success,
                        _ => cx.theme().warning,
                    })
                    .child(status),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .truncate()
                            .font_family(cx.theme().mono_font_family.clone())
                            .text_xs()
                            .child(label),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(flags),
                    ),
            )
            .into_any_element()
    }

    fn render_terminal(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let running = self.state.terminal.running;
        let action = if running {
            Action::StopTerminal
        } else {
            Action::SpawnTerminal
        };
        v_flex()
            .flex_1()
            .min_h_0()
            .p_4()
            .gap_3()
            .child(
                h_flex()
                    .justify_between()
                    .child(section_label("INTEGRATED TERMINAL", cx))
                    .child(
                        Button::new("terminal-toggle")
                            .label(if running { "Stop" } else { "Start" })
                            .small()
                            .when(!running, ButtonVariants::primary)
                            .when(running, ButtonVariants::danger)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.dispatch(action.clone(), cx);
                            })),
                    ),
            )
            .when(!self.state.terminal.title.is_empty(), |view| {
                view.child(
                    div()
                        .truncate()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(self.state.terminal.title.clone()),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .p_3()
                    .overflow_y_scrollbar()
                    .rounded_lg()
                    .bg(gpui::rgb(0x111418))
                    .text_color(gpui::rgb(0xd7dee7))
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_xs()
                    .child(if self.state.terminal.output.is_empty() {
                        if running {
                            "Shell is starting…".to_owned()
                        } else {
                            "Start a terminal for the selected task workspace.".to_owned()
                        }
                    } else {
                        self.state.terminal.output.clone()
                    }),
            )
            .when(running, |view| view.child(Input::new(&self.terminal_input)))
            .when(self.state.terminal.truncated, |view| {
                view.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().warning)
                        .child("Output was truncated at the bounded queue limit."),
                )
            })
            .when(
                !running && self.state.terminal.exit_code.is_some(),
                |view| {
                    view.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!(
                                "Exited with code {}",
                                self.state.terminal.exit_code.unwrap_or_default()
                            )),
                    )
                },
            )
            .into_any_element()
    }

    fn render_computer_use(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(task_id) = self.state.selected_task_id.clone() else {
            return v_flex()
                .flex_1()
                .p_4()
                .child("Select a task to configure Computer Use.")
                .into_any_element();
        };
        let state = self
            .state
            .computer_use
            .get(&task_id)
            .cloned()
            .unwrap_or_default();
        if !state.available_for_task {
            return v_flex()
                .flex_1()
                .min_h_0()
                .p_4()
                .gap_4()
                .child(section_label("COMPUTER USE", cx))
                .child(
                    div()
                        .p_3()
                        .rounded_lg()
                        .bg(cx.theme().warning.opacity(0.08))
                        .border_1()
                        .border_color(cx.theme().warning.opacity(0.45))
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(
                            "The official app-server accepts dynamic tools only when a task starts. Create a new task in codexRS to use Computer Use.",
                        ),
                )
                .child(
                    Button::new("new-computer-task")
                        .label("Create Computer Use task")
                        .primary()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.dispatch(Action::NewTask, cx);
                        })),
                )
                .into_any_element();
        }

        let enabled = state.enabled_for_task;
        let task_for_toggle = task_id.clone();
        let task_for_refresh = task_id.clone();
        let task_for_capture = task_id.clone();
        let task_for_auth = task_id.clone();
        let selected_window_id = state.selected_window_id.clone();
        let window_rows = state
            .windows
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, window)| {
                let selected = selected_window_id.as_deref() == Some(window.id.as_str());
                let task_for_select = task_id.clone();
                let window_id = window.id.clone();
                let window_title = window.title.clone();
                let subtitle = format!(
                    "{} · {}×{}{}{}",
                    window.application,
                    window.width,
                    window.height,
                    if window.focused { " · focused" } else { "" },
                    if window.minimized {
                        " · minimized"
                    } else {
                        ""
                    }
                );
                v_flex()
                    .p_2()
                    .gap_1()
                    .rounded_md()
                    .border_1()
                    .border_color(if selected {
                        cx.theme().primary
                    } else {
                        cx.theme().border
                    })
                    .child(
                        div()
                            .truncate()
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .child(window.title),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(subtitle),
                    )
                    .child(
                        Button::new(("select-computer-window", index))
                            .label(if selected { "Selected" } else { "Select" })
                            .small()
                            .disabled(selected || window.minimized)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.dispatch(
                                    Action::SelectComputerUseWindow {
                                        task_id: task_for_select.clone(),
                                        window_id: window_id.clone(),
                                        title: window_title.clone(),
                                    },
                                    cx,
                                );
                            })),
                    )
                    .into_any_element()
            })
            .collect::<Vec<_>>();

        v_flex()
            .flex_1()
            .min_h_0()
            .p_4()
            .gap_4()
            .child(section_label("COMPUTER USE", cx))
            .child(
                div()
                    .p_3()
                    .rounded_lg()
                    .bg(cx.theme().warning.opacity(0.08))
                    .border_1()
                    .border_color(cx.theme().warning.opacity(0.45))
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        "Disabled by default. Capture and accessibility reads are scoped to the selected target; input needs explicit session approval.",
                    ),
            )
            .child(
                Button::new("toggle-computer-use")
                    .label(if enabled {
                        "Disable for task"
                    } else {
                        "Enable for task"
                    })
                    .when(enabled, ButtonVariants::danger)
                    .when(!enabled, ButtonVariants::primary)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.dispatch(
                            Action::ToggleComputerUse {
                                task_id: task_for_toggle.clone(),
                                enabled: !enabled,
                            },
                            cx,
                        );
                    })),
            )
            .when(enabled, |view| {
                view.child(
                    Button::new("refresh-computer-windows")
                        .label(if state.windows_loading {
                            "Scanning windows…"
                        } else {
                            "Refresh windows"
                        })
                        .small()
                        .disabled(state.windows_loading)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.dispatch(
                                Action::RefreshComputerWindows {
                                    task_id: task_for_refresh.clone(),
                                },
                                cx,
                            );
                        })),
                )
            })
            .when(enabled && state.windows_loading && state.windows.is_empty(), |view| {
                view.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("Reading the bounded desktop window list…"),
                )
            })
            .when(enabled && !window_rows.is_empty(), |view| {
                view.child(
                    v_flex()
                        .max_h(px(270.0))
                        .overflow_y_scrollbar()
                        .gap_2()
                        .children(window_rows),
                )
            })
            .when_some(state.error.clone(), |view, error| {
                view.child(
                    div()
                        .p_2()
                        .rounded_md()
                        .bg(cx.theme().danger.opacity(0.08))
                        .text_xs()
                        .text_color(cx.theme().danger)
                        .child(error),
                )
            })
            .child(detail_row(
                "Target",
                state
                    .selected_window_title
                    .as_deref()
                    .unwrap_or("No window selected"),
                cx,
            ))
            .when_some(state.last_capture_label.clone(), |view, label| {
                view.child(detail_row("Last capture", &label, cx))
            })
            .child(
                Button::new("test-computer-capture")
                    .label("Test selected-window capture")
                    .small()
                    .disabled(!enabled || state.selected_window_id.is_none())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.dispatch(
                            Action::CaptureComputerWindow {
                                task_id: task_for_capture.clone(),
                            },
                            cx,
                        );
                    })),
            )
            .child(
                Button::new("authorize-computer-input")
                    .label(if state.input_authorized_for_session {
                        "Revoke input access"
                    } else {
                        "Allow input this session"
                    })
                    .disabled(!enabled || state.selected_window_id.is_none())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.dispatch(
                            Action::AuthorizeComputerInputForSession {
                                task_id: task_for_auth.clone(),
                                authorized: !state.input_authorized_for_session,
                            },
                            cx,
                        );
                    })),
            )
            .into_any_element()
    }

    fn render_marketplace(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let indices = Rc::new(self.filtered_plugin_indices());
        let count = indices.len();
        v_flex()
            .flex_1()
            .h_full()
            .min_w_0()
            .child(
                h_flex()
                    .h(px(62.0))
                    .px_6()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        v_flex()
                            .child(
                                div()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child("Plugin marketplace"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Official app-server catalogs and installed plugins"),
                            ),
                    )
                    .child(
                        Button::new("refresh-marketplace")
                            .label("Refresh")
                            .small()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.dispatch(Action::RefreshMarketplace, cx);
                            })),
                    ),
            )
            .child(
                uniform_list(
                    "marketplace-list",
                    count,
                    cx.processor(move |this, range: Range<usize>, _, cx| {
                        range
                            .filter_map(|row| indices.get(row).copied())
                            .map(|index| this.render_plugin(index, cx))
                            .collect()
                    }),
                )
                .flex_1()
                .min_h_0()
                .p_4(),
            )
            .into_any_element()
    }

    fn render_plugin(&mut self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(plugin) = self.state.marketplace.plugins.get(index).cloned() else {
            return div().into_any_element();
        };
        let plugin_id = plugin.id.clone();
        let marketplace = plugin.marketplace.clone();
        let installed = plugin.installed;
        h_flex()
            .w_full()
            .h(px(124.0))
            .px_6()
            .py_3()
            .gap_4()
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(46.0))
                    .rounded_lg()
                    .bg(cx.theme().accent)
                    .font_weight(gpui::FontWeight::BOLD)
                    .child(
                        plugin
                            .name
                            .chars()
                            .next()
                            .unwrap_or('P')
                            .to_uppercase()
                            .to_string(),
                    ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child(plugin.name),
                            )
                            .when(plugin.featured, |row| {
                                row.child(
                                    div()
                                        .px_1p5()
                                        .py_0p5()
                                        .rounded_md()
                                        .bg(cx.theme().primary.opacity(0.12))
                                        .text_xs()
                                        .text_color(cx.theme().primary)
                                        .child("Featured"),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_sm()
                            .line_clamp(2)
                            .text_color(cx.theme().muted_foreground)
                            .child(if plugin.description.is_empty() {
                                "No description provided.".to_owned()
                            } else {
                                plugin.description
                            }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(plugin.marketplace),
                    ),
            )
            .child(
                Button::new(SharedString::from(format!("plugin-action-{plugin_id}")))
                    .label(if installed { "Uninstall" } else { "Install" })
                    .when(!installed, ButtonVariants::primary)
                    .when(installed, ButtonVariants::danger)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let action = if installed {
                            Action::UninstallPlugin {
                                plugin_id: plugin_id.clone(),
                            }
                        } else {
                            Action::InstallPlugin {
                                plugin_id: plugin_id.clone(),
                                marketplace: marketplace.clone(),
                            }
                        };
                        this.dispatch(action, cx);
                    })),
            )
            .into_any_element()
    }

    fn render_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let reference = codex_core::stable_reference();
        let codex_binary = self
            .state
            .runtime
            .codex_binary
            .as_ref()
            .map_or_else(|| "Resolving…".to_owned(), |path| display_path(path));
        let codex_home = self
            .state
            .runtime
            .codex_home
            .as_ref()
            .map_or_else(|| "Resolving…".to_owned(), |path| display_path(path));
        let codex_home_kind = if self.state.runtime.codex_home_default {
            "default"
        } else {
            "configured"
        };
        let storage = self.state.storage.path.as_ref().map_or_else(
            || {
                self.state.storage.error.as_ref().map_or_else(
                    || "Opening…".to_owned(),
                    |error| format!("Unavailable: {error}"),
                )
            },
            |path| display_path(path),
        );
        v_flex()
            .flex_1()
            .h_full()
            .min_w_0()
            .child(
                h_flex()
                    .h(px(62.0))
                    .px_6()
                    .items_center()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Settings"),
                    ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .p_6()
                    .gap_4()
                    .child(settings_card(
                        "Build and compatibility",
                        format!(
                            "codexRS {}\nBehavioral reference: Codex Desktop {} / CLI {}",
                            env!("CARGO_PKG_VERSION"),
                            reference.package_version,
                            reference.cli_version
                        ),
                        cx,
                    ))
                    .child(settings_card(
                        "Official Codex runtime",
                        format!(
                            "Executable: {codex_binary}\nCODEX_HOME ({codex_home_kind}): {codex_home}\nLive runtime data is accessed only through the supervised official app-server."
                        ),
                        cx,
                    ))
                    .child(settings_card(
                        "codexRS state",
                        format!(
                            "{storage}\nContains UI preferences and recent workspaces only; it never mirrors live Codex history."
                        ),
                        cx,
                    ))
                    .child(settings_card(
                        "Native runtime and safety",
                        "GPUI renderer, Rust process supervision, bounded frames and queues, and explicit approvals. No Electron, Tauri, Wry, WebView, Node.js, or embedded browser.",
                        cx,
                    ))
                    .child(settings_card(
                        "Platforms",
                        format!(
                            "Current build: {} / {}. Windows is smoke-tested; Linux is validated in native CI and requires X11/XWayland for RC Computer Use.",
                            std::env::consts::OS,
                            std::env::consts::ARCH
                        ),
                        cx,
                    )),
            )
            .into_any_element()
    }

    fn filtered_task_indices(&self) -> Vec<usize> {
        let query = self.task_query.trim().to_lowercase();
        self.state
            .tasks
            .iter()
            .enumerate()
            .filter(|(_, task)| {
                query.is_empty()
                    || task.title.to_lowercase().contains(&query)
                    || task.preview.to_lowercase().contains(&query)
                    || task.cwd.to_string_lossy().to_lowercase().contains(&query)
            })
            .map(|(index, _)| index)
            .collect()
    }

    fn filtered_plugin_indices(&self) -> Vec<usize> {
        let query = self.state.marketplace.query.trim().to_lowercase();
        self.state
            .marketplace
            .plugins
            .iter()
            .enumerate()
            .filter(|(_, plugin)| {
                query.is_empty()
                    || plugin.name.to_lowercase().contains(&query)
                    || plugin.description.to_lowercase().contains(&query)
                    || plugin
                        .category
                        .as_deref()
                        .is_some_and(|category| category.to_lowercase().contains(&query))
            })
            .map(|(index, _)| index)
            .collect()
    }
}

impl Render for WorkspaceView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .size_full()
            .min_w_0()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.render_sidebar(cx))
            .child(self.render_main(cx))
            .when_some(self.state.status_message.clone(), |root, message| {
                root.child(
                    h_flex()
                        .absolute()
                        .left(px(SIDEBAR_WIDTH + 24.0))
                        .right(px(24.0))
                        .bottom(px(16.0))
                        .p_3()
                        .gap_3()
                        .justify_between()
                        .rounded_lg()
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().popover)
                        .shadow_lg()
                        .child(
                            div()
                                .text_sm()
                                .line_clamp(2)
                                .text_color(cx.theme().popover_foreground)
                                .child(message),
                        )
                        .child(
                            Button::new("dismiss-status")
                                .label("Dismiss")
                                .xsmall()
                                .ghost()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.dispatch(Action::ClearStatus, cx);
                                })),
                        ),
                )
            })
    }
}

fn task_status(status: TaskRunStatus, cx: &App) -> (&'static str, gpui::Hsla) {
    match status {
        TaskRunStatus::Idle => ("idle", cx.theme().muted_foreground),
        TaskRunStatus::Running => ("running", cx.theme().info),
        TaskRunStatus::WaitingForApproval => ("approval", cx.theme().warning),
        TaskRunStatus::Completed => ("done", cx.theme().success),
        TaskRunStatus::Interrupted => ("stopped", cx.theme().warning),
        TaskRunStatus::Failed => ("failed", cx.theme().danger),
    }
}

fn git_file_status(kind: GitFileKind) -> &'static str {
    match kind {
        GitFileKind::Added => "A",
        GitFileKind::Modified => "M",
        GitFileKind::Deleted => "D",
        GitFileKind::Renamed => "R",
        GitFileKind::Copied => "C",
        GitFileKind::Untracked => "?",
        GitFileKind::Conflicted => "!",
        GitFileKind::TypeChanged => "T",
    }
}

fn timeline_style(kind: TimelineKind, cx: &App) -> (&'static str, gpui::Hsla, gpui::Hsla) {
    match kind {
        TimelineKind::User => ("YOU", cx.theme().info, cx.theme().info.opacity(0.055)),
        TimelineKind::Agent => (
            "CODEX",
            cx.theme().primary,
            cx.theme().primary.opacity(0.055),
        ),
        TimelineKind::Reasoning => (
            "REASONING",
            cx.theme().muted_foreground,
            cx.theme().muted.opacity(0.45),
        ),
        TimelineKind::Plan => ("PLAN", cx.theme().warning, cx.theme().warning.opacity(0.05)),
        TimelineKind::Command => (
            "COMMAND",
            cx.theme().success,
            cx.theme().success.opacity(0.045),
        ),
        TimelineKind::FileChange => (
            "FILE CHANGE",
            cx.theme().warning,
            cx.theme().warning.opacity(0.045),
        ),
        TimelineKind::Tool => (
            "TOOL",
            cx.theme().chart_4,
            cx.theme().chart_4.opacity(0.045),
        ),
        TimelineKind::Notice => (
            "SYSTEM",
            cx.theme().muted_foreground,
            cx.theme().muted.opacity(0.35),
        ),
    }
}

fn section_label(label: &'static str, cx: &App) -> AnyElement {
    div()
        .text_xs()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(cx.theme().muted_foreground)
        .child(label)
        .into_any_element()
}

fn detail_row(label: &str, value: &str, cx: &App) -> AnyElement {
    h_flex()
        .gap_3()
        .justify_between()
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(label.to_owned()),
        )
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_xs()
                .font_weight(gpui::FontWeight::MEDIUM)
                .child(value.to_owned()),
        )
        .into_any_element()
}

fn metric_badge(label: &str, value: usize, cx: &App) -> AnyElement {
    v_flex()
        .flex_1()
        .min_w_0()
        .p_2()
        .rounded_md()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().background)
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .truncate()
                .child(label.to_owned()),
        )
        .child(
            div()
                .text_sm()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(value.to_string()),
        )
        .into_any_element()
}

fn default_worktree_path(root: &Path, branch: &str) -> PathBuf {
    let repository = root
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("repo");
    let mut slug = branch
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .take(80)
        .collect::<String>();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    let slug = slug.trim_matches('-');
    let slug = if slug.is_empty() { "worktree" } else { slug };
    root.parent()
        .unwrap_or_else(|| Path::new(""))
        .join(format!("{repository}-{slug}"))
}

fn display_path(path: &Path) -> String {
    let rendered = path.display().to_string();
    #[cfg(windows)]
    {
        if let Some(path) = rendered.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{path}");
        }
        if let Some(path) = rendered.strip_prefix(r"\\?\") {
            return path.to_owned();
        }
    }
    rendered
}

fn settings_card(
    title: impl Into<SharedString>,
    body: impl Into<SharedString>,
    cx: &App,
) -> AnyElement {
    v_flex()
        .max_w(px(760.0))
        .p_4()
        .gap_2()
        .rounded_lg()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().sidebar)
        .child(
            div()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(title.into()),
        )
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(body.into()),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::display_path;
    use std::path::Path;

    #[cfg(windows)]
    #[test]
    fn extended_windows_paths_are_displayed_normally() {
        assert_eq!(
            display_path(Path::new(r"\\?\C:\codexrs\state.sqlite")),
            r"C:\codexrs\state.sqlite"
        );
        assert_eq!(
            display_path(Path::new(r"\\?\UNC\server\share\state.sqlite")),
            r"\\server\share\state.sqlite"
        );
    }
}
