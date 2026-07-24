# Contributing to codexRS

Thanks for helping build codexRS. Small fixes, platform reports, documentation,
tests, and focused features are all welcome.

## Before writing code

1. Read [AGENTS.md](AGENTS.md) and the
   [architecture contract](docs/architecture.md).
2. Search existing issues and pull requests.
3. Open an issue or discussion before a large feature, protocol expansion,
   dependency, storage migration, or trust-boundary change.
4. State the observable behavior and acceptance criteria. If there is no
   concrete requirement, failure, or risk, keep the scope out of the change.

Security reports do not belong in public issues. Follow
[SECURITY.md](SECURITY.md).

## Development workflow

1. Fork the repository and branch from an up-to-date `main`.
2. Keep the change focused and follow existing code paths.
3. Add the smallest test that proves changed behavior or prevents an observed
   regression.
4. Run:

   ```text
   python scripts/check_dependency_policy.py
   cargo fmt --all --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   ```

5. For runtime, platform, or packaging changes, also run:

   ```text
   cargo build --release -p codex-app
   ```

6. Open a pull request describing what changed, why it was required, and how it
   was verified.

## Data and safety boundary

- Never commit credentials, tokens, private keys, customer data, `.codex`
  history, databases, logs, screenshots, or unredacted provider payloads.
- Never add extracted OpenAI bundles, binaries, DLLs, assets, or proprietary
  runtime files.
- Do not open a live `CODEX_HOME` directly. The official app-server owns it.
- Use an isolated `CODEX_HOME` for development and tests.
- Treat dirty files and other worktrees as concurrent work.
- Do not use destructive Git recovery, broad deletion, production mutation, or
  automated cleanup `--apply` modes.
- New external frames, queues, pages, logs, captures, and subprocess output must
  have explicit bounds.

## Native-only dependency policy

codexRS is a native Rust application. Pull requests must not introduce
Electron, Tauri, Wry, WebView, Node.js, or browser-runtime dependencies.

If a dependency is needed, explain the concrete requirement, license, platform
impact, trust boundary, and why the existing workspace cannot satisfy it.

## Pull-request expectations

A reviewable pull request:

- has one clear purpose;
- avoids unrelated cleanup and formatting churn;
- calls out Windows and Linux impact;
- includes screenshots for visible UI changes when practical;
- documents new environment variables or user-facing behavior;
- updates the changelog only when the change is release-notable;
- leaves the required checks green.

By submitting a contribution, you agree that it is licensed under the
[Apache License 2.0](LICENSE).
