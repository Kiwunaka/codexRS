# codexRS

[![CI](https://github.com/Kiwunaka/codexRS/actions/workflows/ci.yml/badge.svg)](https://github.com/Kiwunaka/codexRS/actions/workflows/ci.yml)

Private native Rust replacement for Codex Desktop on Windows.

The project uses the installed stable build
`OpenAI.Codex_26.721.3996.0_x64` as a behavioral reference. It reproduces
observable flows and the app-server contract without embedding Electron, Owl,
Chromium, a WebView, Node.js, or extracted OpenAI assets.

## Status

Bootstrap. The current workspace establishes the boundaries and failure budgets
that later application code must preserve:

```text
codex-app
├── codex-core       domain types and state transitions
├── codex-protocol   bounded app-server framing and message schema
├── codex-storage    paginated, size-limited persistence
└── codex-platform   paths, Git, processes, and Windows integration
```

The next milestone is capturing and replaying the stable app-server handshake.
See [architecture.md](docs/architecture.md) for the planned sequence and
[known-failures.md](docs/known-failures.md) for the regressions the replacement
must not inherit.

## Development

Requirements:

- Windows;
- Git;
- Rust through `rustup` (the exact toolchain and components are pinned in
  `rust-toolchain.toml`).

Run the required checks from the repository root:

```powershell
cargo.exe fmt --all --check
cargo.exe clippy --workspace --all-targets -- -D warnings
cargo.exe test --workspace
```

Run the current bootstrap binary:

```powershell
cargo.exe run -p codex-app
```

## Working together

Before changing code, read [AGENTS.md](AGENTS.md) and
[CONTRIBUTING.md](CONTRIBUTING.md). Keep pull requests focused and do not commit
credentials, Codex history, extracted upstream bundles, executables, databases,
or other local runtime data.

This is a private project. Repository access does not grant permission to
redistribute OpenAI software, assets, user data, or credentials.
