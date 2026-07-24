<p align="center">
  <img src="docs/assets/codexrs-hero.png" alt="codexRS native workspace" width="100%">
</p>

<h1 align="center">codexRS</h1>

<p align="center">
  A native Rust workspace for the official Codex app-server.<br>
  Tasks, diffs, branches, worktrees, terminal, Computer Use, and plugins without a browser runtime.
</p>

<p align="center">
  <a href="README.md">English</a> ·
  <a href="README.ru.md">Русский</a> ·
  <a href="README.zh-CN.md">简体中文</a>
</p>

<p align="center">
  <a href="https://github.com/Kiwunaka/codexRS/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/Kiwunaka/codexRS/actions/workflows/ci.yml/badge.svg"></a>
  <a href="https://github.com/Kiwunaka/codexRS/releases"><img alt="Release" src="https://img.shields.io/github/v/release/Kiwunaka/codexRS?include_prereleases&sort=semver"></a>
  <a href="LICENSE"><img alt="Apache-2.0" src="https://img.shields.io/github/license/Kiwunaka/codexRS"></a>
  <a href="https://github.com/Kiwunaka/codexRS/stargazers"><img alt="Stars" src="https://img.shields.io/github/stars/Kiwunaka/codexRS?style=flat"></a>
  <a href="https://github.com/Kiwunaka/codexRS/graphs/contributors"><img alt="Contributors" src="https://img.shields.io/github/contributors/Kiwunaka/codexRS"></a>
  <img alt="Windows and Linux" src="https://img.shields.io/badge/platform-Windows%20%7C%20Linux-2f81f7">
</p>

> [!IMPORTANT]
> The current tree targets `v0.1.0-rc.1`. Windows has passed an end-to-end
> source-build smoke test against the exact stable reference. Linux is covered
> by native CI and still needs broader desktop-environment testing before a
> stable release.

## Why codexRS

Codex Desktop is useful, but a number of workflows need a smaller, inspectable
native client with predictable process and data boundaries. codexRS is that
client:

- native Rust UI built with GPUI;
- no Electron, Tauri, Wry, WebView, Node.js, or embedded browser runtime;
- the official `codex app-server` remains the source of truth;
- normal use points directly at `~/.codex`;
- live Codex auth, SQLite, JSONL, and logs are never opened by codexRS itself.

The behavioral oracle is Codex Desktop `26.721.3996.0`, which bundled Codex CLI
[`0.146.0-alpha.3.1`](https://github.com/openai/codex/releases/tag/rust-v0.146.0-alpha.3.1).
That upstream binary is a compatibility reference, not something this
repository redistributes.

## What works

| Area | Current behavior |
| --- | --- |
| Tasks | Bounded task pages, resume, fork, composer, streaming timeline, and approvals |
| Repository | Status, staged/unstaged files, large virtualized diffs, branch switching, and safe sibling worktrees |
| Terminal | Native PTY/ConPTY session with bounded VT output |
| Computer Use | Native window discovery, capture, pointer, scrolling, typing, and keys with task and session gates |
| Plugins | Marketplace listing, install, and uninstall through app-server protocol methods |
| Persistence | Single-writer codexRS SQLite for UI preferences and recent workspaces only |
| Platforms | Windows source build smoke-tested; Ubuntu build and tests run in CI |

Computer Use is opt-in for each task. Input also requires an explicit
per-session authorization and a selected window. Screenshots stay in memory and
are bounded before they enter the app-server protocol.

## Architecture

```mermaid
flowchart LR
    UI["Native GPUI shell"] --> Core["codex-core<br/>state + effects"]
    Core --> Platform["codex-platform"]
    Core --> Store["codex-storage<br/>owned UI state"]
    Platform --> AppServer["official codex app-server"]
    AppServer --> Home["~/.codex<br/>app-server owned"]
    Platform --> Git["Git"]
    Platform --> PTY["PTY / ConPTY"]
    Platform --> CU["native Computer Use"]
```

All protocol frames, queues, history pages, diffs, terminal output, screenshots,
and subprocess diagnostics have explicit limits. See
[Architecture](docs/architecture.md) and [Platform support](docs/platform-support.md)
for the full boundaries.

## Quick start

### 1. Install the official Codex CLI

Follow the current [Codex CLI setup guide](https://learn.chatgpt.com/docs/codex/cli),
then run `codex` once to sign in. codexRS supervises the CLI's native
`app-server`; it does not embed the CLI or require a Node runtime of its own.

Verify that the executable is available:

```text
codex --version
```

If it is not on `PATH`, set `CODEX_RS_CODEX_BIN` to the native `codex` or
`codex.exe` executable.

### 2. Build codexRS

Install Git, Rust through `rustup`, and the native packages listed under
[Platform support](docs/platform-support.md), then:

```text
git clone https://github.com/Kiwunaka/codexRS.git
cd codexRS
cargo build --release -p codex-app
```

Run on Windows:

```powershell
.\target\release\codexrs.exe
```

Run on Linux:

```bash
./target/release/codexrs
```

Linux build packages and desktop-session limitations are listed in
[Platform support](docs/platform-support.md).

### Development isolation

Normal use defaults to `~/.codex`. For protocol development and tests, point
`CODEX_HOME` at an isolated directory:

```powershell
$env:CODEX_HOME = 'E:\scratch\isolated-codex-home'
cargo run -p codex-app
```

codexRS-owned state can be redirected independently with
`CODEX_RS_DATA_DIR`.

## Contributing

Contributions are welcome. Start with
[Contributing](CONTRIBUTING.md), read [AGENTS.md](AGENTS.md), and keep changes
focused on an observable requirement or failure. Large features should begin
as an issue or discussion so the contract is clear before implementation.

- [Roadmap](ROADMAP.md)
- [Support](SUPPORT.md)
- [Security policy](SECURITY.md)
- [Code of Conduct](CODE_OF_CONDUCT.md)
- [Prior-art review](docs/prior-art.md)

## Contributors

<a href="https://github.com/Kiwunaka/codexRS/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=Kiwunaka/codexRS" alt="codexRS contributors">
</a>

## Star history

[![Star History Chart](https://api.star-history.com/svg?repos=Kiwunaka/codexRS&type=Date)](https://star-history.com/#Kiwunaka/codexRS&Date)

## License and upstream notice

codexRS is licensed under [Apache License 2.0](LICENSE).

This is an independent community project. It is not affiliated with or endorsed
by OpenAI. Codex and OpenAI names belong to their respective owners. The
official Codex CLI is installed and licensed separately.
