# Stable failures used as acceptance tests

These are observed failure modes from the compatibility reference and public
reports. They are test inputs, not behavior to reproduce.

| Failure | Evidence | codexRS control | Status |
| --- | --- | --- | --- |
| Windows multi-root white screen | POSIX path handling reached Windows drive paths and a missing process cwd | Native `Path`/`PathBuf`; no browser path shim | Implemented |
| Unbounded JSONL line | One observed line reached 594,127,437 bytes | Live history is queried through bounded app-server pages; direct live JSONL reads are forbidden | Implemented |
| Startup history scan | History reached about 9 GB with files over 100 MB | `thread/list` is paginated and always sets `useStateDbOnly: true` | Implemented |
| Git process storm | Repeated `git.exe` spawning after filesystem notifications | 300 ms debounce, notification coalescing, one backend Git operation at a time | Implemented |
| Process cleanup storm | Repeated `taskkill.exe`, `conhost.exe`, and WMI activity | One supervised tree, graceful shutdown, one bounded fallback; Job Object on Windows | Implemented |
| Unbounded logging | Repeated writes and growth of upstream log storage | codexRS does not duplicate provider logs or raw payloads; its owned state is narrowly scoped | Implemented |

## Active release-candidate limitations

- Linux Computer Use window discovery currently requires an X11 or XWayland
  session. Pure Wayland input is not release-qualified.
- Dynamic Computer Use tools are attached at `thread/start`; an existing task
  cannot gain them after creation.
- Windows and Linux installers are not yet signed.
- Linux is validated by Ubuntu CI, but broader desktop-environment smoke
  coverage is still in progress.
- Accessibility semantics and full keyboard-only navigation need a dedicated
  pass before the stable release.

## Current budgets

- Protocol frame: 16 MiB.
- Decoded transport queue: 1 frame.
- Interleaved messages per request: 256.
- Thread metadata page: 20 by default, 100 maximum.
- Git metadata: 2 MiB per command.
- Unified diff: 4 MiB.
- Git files: 2,000; branches: 500; worktrees: 100; review commits: 30.
- Terminal input: 64 KiB; terminal events: 256.
- Computer capture: 1600×1200 and 3 MiB maximum.
- Computer text input: 16 KiB.
- codexRS preference value: 64 KiB.
- Owned-storage page: 500 rows maximum.
