# Contributing

This repository is private. Work only with collaborators who have been granted
access by the owner.

## Workflow

1. Start from an up-to-date `main` and create a short-lived branch such as
   `feat/app-server-handshake` or `fix/windows-paths`.
2. Make the smallest scoped change that satisfies the task and follows
   [AGENTS.md](AGENTS.md).
3. Run the required checks:

   ```powershell
   cargo.exe fmt --all --check
   cargo.exe clippy --workspace --all-targets -- -D warnings
   cargo.exe test --workspace
   ```

4. Open a pull request that explains the behavior changed, why it changed, and
   how it was verified.

Do not commit credentials, `.codex` history, databases, logs, extracted upstream
bundles, executables, or OpenAI assets. Test imports against bounded snapshots or
isolated fixtures; never share live write access with Codex Desktop.
