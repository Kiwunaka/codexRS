# Changelog

All notable changes to codexRS are documented here. The project follows
[Semantic Versioning](https://semver.org/) once release tags are published.

## [Unreleased]

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
