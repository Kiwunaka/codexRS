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
| [CodexPlusPlus](https://github.com/BigPizzaV3/CodexPlusPlus) | Provider-management and official-client launcher UX | Behavioral reference only. Tauri/CDP injection conflicts with the project boundary, and AGPL code is not copied |
| [oh-my-codex](https://github.com/Yeachan-Heo/oh-my-codex) | Plugin package layout and local marketplace metadata | Use the package shape as a UX reference. Do not depend on its Node workflow layer or copy code without a verifiable license |
| [AionUi](https://github.com/iOfficeAI/AionUi) | Multilingual open-source presentation, grouped history, tool disclosure, and marketplace UX | Product reference only; its Electron/Bun stack is outside the native client |
| [codex-manager](https://github.com/cnlimiter/codex-manager) | None for a local Codex Desktop replacement | Rejected: it automates account registration, token export, and payment-related flows outside this project's trust boundary |

The official Rust app-server client currently depends on a substantial portion
of the upstream Codex workspace. The smaller maintainable choice is a typed,
bounded subset. It now covers initialization, paginated tasks and history,
turns, approvals, dynamic tools, apps, plugins, and marketplace operations;
schema additions remain tied to user-visible flows.

The checked wire shapes were generated from the selected local
`codex-cli 0.146.0-alpha.3.1`. Generated schemas stay under ignored `target/`
research output and are not shipped or treated as source code.
