# Roadmap

The roadmap is ordered by release risk rather than feature count. Items move
only when an observable requirement or failure justifies the work.

## v0.1.0-rc.1

- [x] Bounded official app-server supervisor and typed protocol.
- [x] Native GPUI task browser, composer, streaming timeline, forks, and approvals.
- [x] Repository view with staged/unstaged files and virtualized unified diffs.
- [x] Safe branch switching and sibling worktree creation.
- [x] Native PTY/ConPTY terminal.
- [x] Opt-in native Computer Use with task and session authorization.
- [x] App-server plugin marketplace.
- [x] Separate single-writer codexRS SQLite state.
- [x] Windows smoke path and Windows/Linux CI matrix.
- [ ] Publish unsigned Windows and Linux release archives with checksums.
- [ ] Complete the first public Ubuntu CI run.

## Release-candidate hardening

- [ ] Smoke-test GNOME, KDE, X11, and XWayland sessions.
- [ ] Add portal-backed pure Wayland Computer Use.
- [ ] Complete keyboard-only navigation and accessibility semantics.
- [ ] Soak-test long streaming tasks, reconnects, large diffs, and PTY output.
- [ ] Add branch comparison and clearer multi-worktree navigation.
- [ ] Measure startup, idle memory, diff rendering, and event latency.

## Stable release

- [ ] Publish signed Windows and Linux packages.
- [ ] Document the compatibility window for newer official Codex CLI versions.
- [ ] Promote only after Windows and Linux release gates remain green.

Feature proposals belong in GitHub Issues or Discussions. A proposal should
name the user-visible behavior, acceptance criteria, and the evidence that
requires expanding the current scope.
