# Prior art and reuse decisions

The first app-server slice was designed after checking current implementations
instead of reconstructing the protocol from the old desktop bundle.

| Project | Useful part | Decision |
| --- | --- | --- |
| [OpenAI Codex app-server](https://developers.openai.com/codex/app-server/) | Authoritative lifecycle, generated schema, and `CODEX_HOME` contract | Follow the generated protocol for the locally selected `codex` binary |
| [OpenAI app-server client](https://github.com/openai/codex/tree/main/codex-rs/app-server-client) | Event classification and shutdown patterns | Use as a reference; do not pull the tightly coupled Codex workspace into this client |
| [Beryl](https://github.com/berylorg/beryl) | Native Rust client, bounded stdio, Windows Job Object supervision | Reimplement the small applicable patterns behind project boundaries; do not vendor code |
| [CodexGui](https://github.com/wieslawsoltes/CodexGui) | Typed native desktop client over app-server | Use as a behavioral cross-check, not a runtime dependency |
| [T3 Code](https://github.com/pingdotgg/t3code) | Proven UX and default use of `~/.codex` | Keep as a product/protocol reference; its Electron/Node stack cannot be used here |
| [CodexMonitor](https://github.com/Dimillian/CodexMonitor) | Task history, Git/worktree, diff, terminal, and app-server UX | Use as a behavioral reference. Do not copy its Tauri/React runtime, unbounded channels, line reads, or pending-request map |
| [codex-desktop-linux](https://github.com/ilysenko/codex-desktop-linux) | Linux accessibility, portal screenshots, compositor adapters, and input backends | Reuse architectural ideas for native Linux Computer Use with attribution where code is actually adapted; do not use its Electron wrapper |
| [computer-use-linux](https://github.com/agent-sh/computer-use-linux) | MIT Rust library with AT-SPI trees, bounded screenshots, portal input, and GNOME/KWin/Hyprland/i3/COSMIC window adapters | Strongest ready Linux reference. Evaluate its library modules per Linux vertical slice; do not pull its full MCP/server dependency graph before a concrete backend requires it |
| [rs_peekaboo](https://github.com/undivisible/rs_peekaboo) | Broad cross-platform command vocabulary and snapshot/index UX | Vocabulary reference only. Its Windows backend shells out to PowerShell and does not provide the native AUMID/window identity boundary required here |
| [CodexPlusPlus](https://github.com/BigPizzaV3/CodexPlusPlus) | Provider-management and official-client launcher UX | Behavioral reference only. Tauri/CDP injection conflicts with the project boundary, and AGPL code is not copied |
| [oh-my-codex](https://github.com/Yeachan-Heo/oh-my-codex) | Plugin package layout and local marketplace metadata | Use the package shape as a UX reference. Do not depend on its Node workflow layer or copy code without a verifiable license |
| [AionUi](https://github.com/iOfficeAI/AionUi) | Multilingual open-source presentation, grouped history, tool disclosure, and marketplace UX | Product reference only; its Electron/Bun stack is outside the native client |
| [codex-manager](https://github.com/cnlimiter/codex-manager) | None for a local Codex Desktop replacement | Rejected: it automates account registration, token export, and payment-related flows outside this project's trust boundary |

Read-only inspection of the pinned stable bundle also identified the exact
Settings route registry and labels from `app.asar`, the Windows `@oai/sky`
0.5.2 surface, and its supervised
`codex-computer-use.exe` JSON-lines boundary. codexRS reproduces the behavior
with an Apache-2.0 Rust UI Automation wrapper inside its own helper process; it
does not copy, launch, or redistribute the official helper.

The official Rust app-server client currently depends on a substantial portion
of the upstream Codex workspace. The smaller maintainable choice is a typed,
bounded subset. It now covers initialization, paginated tasks and history,
turns, approvals, dynamic tools, apps, plugins, and marketplace operations;
schema additions remain tied to user-visible flows.

The checked wire shapes were generated from the selected local
`codex-cli 0.146.0-alpha.3.1`. Generated schemas stay under ignored `target/`
research output and are not shipped or treated as source code.
