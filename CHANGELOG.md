# Changelog

All notable changes to codexRS are documented here. The project follows
[Semantic Versioning](https://semver.org/) once release tags are published.

## [Unreleased]

## [0.1.0-rc.10] - 2026-08-03

### Fixed

- Failed submissions cannot restore an older prompt over a newer draft in the
  same chat.
- Marketplace catalogs and plugin details refresh when the selected workspace
  changes; MCP runtime status stays scoped to the selected task on every page.
- Timeline and archived-chat pagination stop on repeated cursors, visible-item
  limits, or bounded page budgets.
- Browser Agent tab focus and cached expressions cannot cross task contexts.
- Successful Unix parent exits terminate supervised descendants before output
  readers join.

### Security

- Oversized persisted preferences are rejected before SQLite materializes the
  value in memory.

## [0.1.0-rc.9] - 2026-08-03

### Fixed

- Git diffs, commits, and pull requests ignore late results from a previous
  repository or operation.
- Failed model-catalog loads can be retried without discarding cached models.
- Task-only panel commands and Find shortcuts are unavailable on routes where
  they cannot run.
- App-server disconnects release Marketplace mutation locks while preserving
  catalogs, local install confirmation, and OAuth correlation.
- Runtime snapshots for the same active turn preserve safety retry and Stop
  state; reconnect releases in-flight controls whose terminal reply was lost.
- Unix Browser descendants and platform children are terminated through their
  owned process group when the root process or output-reader startup fails.

### Security

- Computer Use revalidates the live application identity immediately before
  accessibility capture or a screenshot.

## [0.1.0-rc.8] - 2026-08-03

### Added

- MCP runtime-status catalogs load bounded additional pages instead of
  stopping at the first page.

### Fixed

- Browser screencast delivery coalesces superseded frames and respects the UI
  byte budget without displacing control or disconnect events.
- Hidden right-side panels restore only at wide layout widths.
- Failed or late Goal, shell, login, approval, MCP, and pull-request operations
  preserve retry or resync state.
- New-chat creation no longer overwrites a newer draft on late failure or
  success.
- Task, diff, review-panel, usage, pull-request, MCP OAuth, and compact-layout
  state remains scoped to the active workspace.
- Linux launchers preserve the configured Codex CLI path, validate executable
  Browser paths, and bound opener and terminal process lifecycles.
- Account refresh no longer interrupts device or browser login cancellation.
- Import completion resync and task-only Find command behavior are corrected.

### Security

- Browser runtime queues and shutdown are bounded, debugger targets stay
  isolated to their owning tab, and raw Target-domain escape paths are
  rejected.
- Computer Use revalidates the approved application identity before raw input
  and accessibility actions.
- Failed approval and MCP responses restore the exact pending request instead
  of silently dropping it.

## [0.1.0-rc.7] - 2026-08-03

### Added

- Linux sends native notifications when background chats complete.

### Fixed

- Reconnect recovery reloads selected timelines and goals, preserves canceled
  login state, restores failed composer submissions in their owning chat, and
  surfaces selected-file diff failures.
- Stale plugin catalog results are ignored; transient Git refresh failures
  preserve the current snapshot and branch-operation state.
- Computer Use requires fresh state after a target switch and keeps concurrent
  turn interruption monitors isolated.
- MCP catalog results are scoped to the selected task and workspace generation,
  preventing stale results after either changes.

### Security

- App-server ingress and derived UI events are byte-bounded through reduction;
  semantic overload reconnects for authoritative recovery rather than silently
  dropping state.

## [0.1.0-rc.6] - 2026-08-02

### Added

- API-key and experimental Amazon Bedrock sign-in, plus bounded daily
  token-usage visibility.
- Plugin and App management now reflects installed runtime availability, exposes
  admin-disabled plugin policy, supports keyword search, and exposes validated
  public share links.
- Eligible models show a bounded availability introduction, remembered after
  dismissal.
- The window title and Windows notification-area tooltip show the bounded count
  of non-selected chats that are running or awaiting approval; Windows also
  sends a quiet-hours-respecting completion notification.
- Linux release packaging smokes the extracted archive in Xvfb, and the
  per-user desktop entry and native windows share the `com.codexrs.CodexRS`
  application ID.
- Reduced-motion settings are applied by native components.

### Fixed

- Modal dialogs confine keyboard focus and global shortcuts do not escape
  active modals; competing right panels are hidden when the layout becomes
  compact.
- Live command and file-output deltas reconnect on backpressure rather than
  being silently dropped, and Computer Use preserves its active interruption
  target during discovery.
- Logout remains pending across stale account refreshes; backend shutdown
  takes priority over queued commands; stale workspace-specific plugin catalogs
  are ignored.
- Switching tasks clears stale composer text, attachments, errors, and popovers
  so prompts and `/shell` commands cannot be delivered to another task.

## [0.1.0-rc.5] - 2026-08-02

### Added

- First-run setup supports browser and device-code ChatGPT sign-in plus native
  project-folder selection.
- Goal files attach through the public turn-input path.
- Linux gains bounded screenshot-only X11/XWayland Computer Use and an
  explicit per-user desktop-entry installer.
- The model picker shows descriptions, availability, and upgrade notices.
- Usage & billing shows a bounded ChatGPT token-activity summary and the
  available usage-limit-reset count.
- Marketplace sources can be upgraded individually, and plugin installation
  requires the stable confirmation interstitial.
- Background completion banners provide native Open and Dismiss actions.

### Fixed

- Startup recovery preserves distinct backend and app-server failure actions.
- App catalogs stay scoped to the selected chat and ignore stale paginated
  results after chat switches.
- Partial marketplace failures remain visible without discarding valid apps.
- Compact Changes layouts remain usable on narrow windows, and unsafe
  assistant Markdown links are neutralized.
- Managed worktrees accept locked entries while rejecting relative paths before
  backend dispatch.
- The logout confirmation keeps keyboard focus inside its native modal.

## [0.1.0-rc.4] - 2026-08-01

### Added

- Marketplace entries that require authentication expose the existing guarded
  sign-in handoff.
- Project selection is keyboard accessible, and successful background chats
  announce completion without interrupting the active task.
- The Changes view groups bounded multi-file diffs with native Unified/Split
  controls and per-file folding.
- Managed worktree handoffs use a bounded three-item FIFO with correlated
  cancellation, retry, and failure recovery.

### Fixed

- Linux downloads honor the configured XDG Downloads directory.
- Pull requests restore Cargo build caches without publishing branch-specific
  cache entries.

## [0.1.0-rc.3] - 2026-08-01

### Added

- Native Code review with saved Inline/Detached delivery preference.
- Composer `Continue in` Work picker for local work or a new worktree.
- Permission picker descriptions for the available profiles.

### Fixed

- Release archives include the README-linked documentation and assets.
- Windows rejects case-aliased nested worktree paths.
- Stale repository snapshots and cross-repository branch results no longer
  update the active repository; duplicate worktree handoffs are blocked.

## [0.1.0-rc.2] - 2026-07-31

### Added

- Native per-occurrence chat Find navigation with Unicode-safe matching,
  stable-style result ordinals, and source-range inline highlights across
  plain-text and rendered GFM Markdown without losing links or nested styling.
- Bounded continuation of chat Find through unloaded official history pages.
- Native generated-image timeline previews and Save As downloads.
- Complete review-source and searchable base selection for working-tree,
  branch, commit, and pull-request diffs.
- Guarded pull-request lifecycle actions for create, merge, and GitHub handoff.
- A persistent bounded local-project registry with native sidebar actions.
- Explicit full-system-access `/shell` commands through the official
  app-server.
- Native personalization and per-chat memory controls.
- Typed external-agent detection and import for Claude Code, Claude Cowork,
  and Cursor through official app-server methods.
- A singleton native About window.
- Native Remote Connections settings and typed remote-control state backed by
  official app-server RPCs.

### Fixed

- Unexpected app-server exits now recover through one bounded exponential
  reconnect timer (`1/2/4/8/16/20` seconds), reset after successful
  initialization, with native attempt and failure diagnostics.
- Safety-buffered turns can retry on the advertised faster model without
  applying rollback twice or losing accepted steer messages.
- Native switches are keyboard operable.

### Security

- Windows Computer Use actions require the capture-excluded native system
  indicator and preserve the per-app approval and product-policy guards.
- The release archive includes the separately supervised native Computer Use
  overlay helper.

## [0.1.0-rc.1] - 2026-07-24

### Added

- Native GPUI task, repository, marketplace, settings, diff, terminal, and
  Computer Use surfaces.
- Bounded typed protocol for the official Codex app-server.
- Task paging, timeline streaming, forks, approvals, and plugin operations.
- Native Git status, staging, branch switching, worktree creation, and
  virtualized unified diffs.
- Native PTY/ConPTY terminal and supervised process trees.
- Opt-in Computer Use with bounded in-memory screenshots and explicit input
  authorization.
- Single-writer codexRS SQLite state separate from live Codex data.
- Windows and Ubuntu CI, dependency policy, and release packaging workflows.

### Security

- Live `~/.codex` data remains app-server-owned and is never opened directly.
- Frames, channels, pages, diffs, terminal data, screenshots, and diagnostics
  are bounded.
- Git refreshes are debounced, coalesced, and serialized.
