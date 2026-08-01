# Changelog

All notable changes to codexRS are documented here. The project follows
[Semantic Versioning](https://semver.org/) once release tags are published.

## [Unreleased]

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
