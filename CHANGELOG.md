# Changelog

All notable changes to codexRS are documented here. The project follows
[Semantic Versioning](https://semver.org/) once release tags are published.

## [Unreleased]

### Added

- Native per-occurrence chat Find navigation with Unicode-safe matching,
  stable-style result ordinals, inline highlights for plain-text timeline
  content, and active-message reveal for rendered Markdown.

## [0.1.0-rc.1] - 2026-07-24

### Added

- Native GPUI task, repository, marketplace, settings, diff, terminal, and
  Computer Use surfaces.
- Bounded typed protocol for the official Codex app-server.
- Task paging, timeline streaming, forks, approvals, and plugin operations.
- Stable-shaped generated-image placeholders and bounded, clickable timeline
  previews backed by the existing native Outputs viewer.
- Generated-image downloads through the native Save As dialog with the stable
  date-based filename and workspace-confined streaming copy.
- Native Git status, staging, branch switching, worktree creation, and
  virtualized unified diffs.
- Native PTY/ConPTY terminal and supervised process trees.
- Opt-in Computer Use with bounded in-memory screenshots and explicit input
  authorization.
- Native singleton About window with the package version, fixed reference
  geometry, and OK/Escape close behavior.
- Single-writer codexRS SQLite state separate from live Codex data.
- Windows and Ubuntu CI, dependency policy, and release packaging workflows.

### Fixed

- Unexpected app-server exits now recover through one bounded exponential
  reconnect timer (`1/2/4/8/16/20` seconds), reset after a successful
  initialization, with native attempt and failure diagnostics.

### Security

- Live `~/.codex` data remains app-server-owned and is never opened directly.
- Frames, channels, pages, diffs, terminal data, screenshots, and diagnostics
  are bounded.
- Git refreshes are debounced, coalesced, and serialized.
