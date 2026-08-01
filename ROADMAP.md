# Roadmap

The roadmap is ordered by release risk rather than feature count. Items move
only when an observable requirement or failure justifies the work.

The product goal is a native Rust replacement with full Codex Desktop
behavioral and UX parity. The detailed contract lives in the
[parity matrix](docs/parity-matrix.md).

## v0.1.0-rc.2

- [x] Bounded official app-server supervisor and typed protocol.
- [x] Native GPUI task browser, composer, streaming timeline, forks, and approvals.
- [x] Repository view with staged/unstaged files and virtualized unified diffs.
- [x] Safe branch switching and sibling worktree creation.
- [x] Native Worktrees settings for the managed root, repository inventory, and
  linked-chat navigation.
- [x] Native Personalization settings backed by official app-server config,
  including explicit personality propagation to normal threads and turns.
- [x] Native Configuration settings backed by layered `config/read`,
  `configRequirements/read`, and versioned `config/batchWrite`.
- [x] Native PTY/ConPTY terminal.
- [x] Opt-in native Computer Use with selected-app approval, task access, and an
  app-server-owned always-allowed list.
- [x] App-server plugin marketplace.
- [x] Separate single-writer codexRS SQLite state.
- [x] Windows smoke path and Windows/Linux CI matrix.
- [x] Publish unsigned Windows and Linux release archives with checksums.
- [x] Complete the first public Ubuntu CI run.

## Release-candidate hardening

- [ ] Smoke-test GNOME, KDE, X11, and XWayland sessions.
- [ ] Add portal-backed pure Wayland Computer Use.
- [ ] Complete keyboard-only navigation and accessibility semantics.
- [ ] Soak-test long streaming tasks, reconnects, large diffs, and PTY output.
- [ ] Add branch comparison and clearer multi-worktree navigation.
- [ ] Measure startup, idle memory, diff rendering, and event latency.

## Desktop parity release candidate

- [x] Pin the installed Desktop, bundled CLI hash, official UI evidence, renderer
  route inventory, settings inventory, and generated app-server schemas.
- [x] Load the live model, reasoning-effort, and permission-profile catalogs
  into the composer and forward selections to new threads and turns.
- [x] Match native file/folder/image attachments, Plan collaboration mode, and
  the app-server-owned Goal lifecycle with live status, progress, guarded
  continuation, and pause-before-stop behavior.
- [x] Match active-turn Send, Steer, and Stop behavior through typed stable
  app-server methods.
- [x] Add bounded Unified/Split diff review with real old/new hunk line numbers.
- [ ] Match the project/chat shell, thread lifecycle, sidebar, header, composer,
  search, commands, attachments, goals, plans, and execution targets.
- [ ] Match activity, diff review, Git, worktrees, terminal, outputs, and pull
  request workflows.
- [ ] Match Computer Use, browser, skills, plugins, MCP apps, scheduled tasks,
  cloud environments, and notifications.
- [ ] Complete remote control beyond the current native public-RPC UI; keep-awake,
  SSH profiles, remote chats, and handoff require a public app-server contract.
- [ ] Match artifacts, files, Sites, visualizations, appshots, image/audio/voice,
  previews, and the output library.
- [ ] Match all settings, onboarding, account, usage, update, accessibility,
  Windows, and Linux contracts listed in the parity matrix.
- [ ] Pass same-state screenshot comparison and native interaction smoke tests
  for every release-critical flow.

## Stable release

- [ ] Publish signed Windows and Linux packages.
- [ ] Document the compatibility window for newer official Codex CLI versions.
- [ ] Promote only after Windows and Linux release gates remain green.

Feature proposals belong in GitHub Issues or Discussions. A proposal should
name the user-visible behavior, acceptance criteria, and the evidence that
requires expanding the current scope.
