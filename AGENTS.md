# Project rules

- Build a native Rust replacement for Codex Desktop. Do not add Electron, Tauri,
  Wry, WebView, Node.js, or browser-runtime dependencies.
- Treat stable `26.721.3996.0` as a behavioral reference, not a runtime
  dependency.
- Never modify the installed MSIX or the live `C:\Users\kiwun\.codex` data
  without an explicit request.
- Bound every external frame, event, log, queue, and history query. No
  unbounded `read_to_string`, JSONL line reads, or startup scans.
- Keep platform process management behind `codex-platform`. On Windows, prefer
  Job Objects and graceful cancellation; never implement polling `taskkill`
  loops.
- Keep storage single-writer and paginated. Import existing Codex data from a
  snapshot, never by sharing live write access.
- Run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets`, and
  `cargo test --workspace` before handing off changes.

## Proportional Engineering

- Apply KISS, YAGNI, and the Pareto principle. Make the smallest maintainable
  change that satisfies the explicit acceptance criteria and current evidence;
  prefer existing patterns and code paths.
- Do not add speculative abstractions, dependencies, compatibility layers,
  fallbacks, configuration, cleanup, documentation, or future-proofing outside
  the assigned scope.
- Keep verification proportional. Add or update only the smallest focused tests
  needed to prove changed behavior or prevent a concrete observed regression.
  Do not add redundant unit/integration/E2E coverage, exhaustive edge-case
  matrices, broad regression suites, or unrelated test refactors unless the
  task, affected shared contract, or observed failure requires them.
- Keep security work proportional to the actual trust boundary and concrete
  threat model. Preserve mandatory safeguards and fix vulnerabilities
  introduced or exposed by the task, but do not add speculative hardening, new
  security frameworks, or unrelated defenses without evidence or an explicit
  requirement.
- Before expanding scope, identify the concrete acceptance criterion, failure,
  or risk that requires it. If none exists, omit the extra work. If expansion
  would materially change the solution, request owner direction first.
- These proportionality rules do not authorize skipping checks explicitly
  required by the selected router row, current repository contracts, or release
  gates applicable to the changed behavior.

## Universal Safety

- Integrity and safety checks are authorized only for the local repositories,
  owned runtime surfaces, and isolated fixtures named by the task. They are
  defensive validation; third-party systems, accounts, credentials, and data
  are out of scope.
- Never print, commit, move, copy into artifacts, or expose secrets, tokens,
  credentials, private keys, raw connection material, customer data, or
  unredacted provider payloads.
- Do not perform broad deletion, destructive Git operations, database resets,
  account deletion, production mutation, deploy, payment action, or external
  communication unless the task explicitly authorizes it and the required guard
  is satisfied.
- Preserve evidence, audit artifacts, archive history, rollback material, and
  generated provenance. Relabel unclear history; do not erase it during routine
  cleanup.
- Treat dirty files and other worktrees as concurrent work. Do not overwrite,
  revert, stage, or reformat changes outside the assigned scope.
- Keep cleanup targeted, reviewable, and reversible. Never use an automated
  cleanup `--apply` mode for repository-context work.
