<p align="center">
  <img src="docs/assets/codexrs-hero.png" alt="codexRS 原生工作区" width="100%">
</p>

<h1 align="center">codexRS</h1>

<p align="center">
  以完整功能和 UX 对等为目标的 Codex Desktop 原生 Rust 替代方案。<br>
  无需 Electron、WebView 或浏览器运行时即可兼容官方 app-server。
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
  <img alt="Windows 与 Linux" src="https://img.shields.io/badge/platform-Windows%20%7C%20Linux-2f81f7">
</p>

> [!IMPORTANT]
> 当前代码树以 `v0.1.0-rc.2` 为目标。Windows 已通过针对精确 stable
> 参考版本的源码端到端冒烟测试。Linux 已纳入原生 CI，但在稳定版发布前仍需覆盖更多桌面环境。

## 为什么选择 codexRS

codexRS 提供一个体积更小、边界更清晰的 Codex 原生客户端：

- 使用 Rust 与 GPUI 构建原生界面；
- 不包含 Electron、Tauri、Wry、WebView、Node.js 或嵌入式浏览器；
- 官方 `codex app-server` 始终是数据源；
- 默认直接使用 `~/.codex`；
- codexRS 本身不会直接打开实时 Codex auth、SQLite、JSONL 或日志文件。

行为参考版本为 Codex Desktop `26.721.3996.0`，其中包含 Codex CLI
[`0.146.0-alpha.3.1`](https://github.com/openai/codex/releases/tag/rust-v0.146.0-alpha.3.1)。
该上游二进制文件仅用于兼容性验证，不会随本项目分发。

## 已实现功能

| 领域 | 当前能力 |
| --- | --- |
| 任务 | 有界分页、恢复、fork、输入框、流式时间线与审批 |
| 仓库 | 状态、暂存/未暂存文件、大型虚拟化差异、分支切换与安全的同级 worktree |
| 终端 | 原生 PTY/ConPTY 与有界 VT 输出 |
| Computer Use | 仅 Windows：原生窗口发现与控制，按应用审批、按任务授权，并由 app-server 管理始终允许列表 |
| 插件 | 原生目录标签、有界图片加载、已安装插件与来源管理、创建入口，以及通过 app-server 添加/移除/升级 marketplace、安装和卸载 |
| 持久化 | 独立单写者 SQLite，保存 codexRS 界面偏好以及带名称和置顶状态的有界本地项目注册表 |
| 平台 | Windows 已完成源码冒烟测试；Ubuntu CI 构建并测试 UI、app-server、Git 与 PTY；RC2 中 Linux Computer Use 不可用 |

RC2 中 Computer Use 仅在 Windows 可用，且必须按任务启用。每个读取或控制动作都携带由有界 discovery 返回的精确 `Window { app, id, title? }`；codexRS 会重新解析不透明窗口 ID，并在执行前验证当前所属应用。原生检查器中的窗口选择仅用于手动操作。每个真实应用首次被读取或控制前都会单独请求授权。在 Windows 上，packaged 应用保留大小写不变的 AUMID；普通 executable 优先采用与 stable 相同的 known-folder GUID 标识，否则采用大小写不变的绝对路径，旧的 `process:` 标识仍可用于匹配。已知共享宿主进程和过长标识会直接拒绝。`Allow once` 仅覆盖当前任务，`Always allow` 通过官方 app-server 持久化，但两者都不能绕过针对 Codex、终端、密码管理器、身份与安全界面的 product-policy 禁止规则。原生 Windows 应用目录会有界读取两套 Start Menu、execution aliases、AppsFolder 与已安装包 manifests，并通过 Shell AppUserModel ID、link target path 和有界 UserAssist process key 提供 stable 形状的应用与使用记录；模型直接启动应用时沿用同一套按应用审批。截图只保留在内存中，并在进入协议前受到尺寸限制；短期 screenshot ID 会把缩放图像坐标映射回真实窗口。Windows 的可访问性树、窗口激活与索引操作运行在受监管的原生 Rust helper 中：最多 512 个元素、128 KiB，单次请求超时为 10 秒；第三方 UI Automation provider 卡死时只终止 helper，不会冻结客户端。输入方法会自动激活精确目标窗口，`activate_window` 则保留为与 stable Window2 一致的显式恢复动作。受保护的输入开始前，必须先显示带有 stable 精确文案 `ChatGPT is using your computer` / `Esc to cancel` 的原生置顶系统指示器。它持续到本次 Computer Use 回合结束，不获取焦点、不拦截点击，也不会进入截图；若无法显示，输入动作会被拒绝。Windows 发行包会在 `codexrs.exe` 旁附带 `codex-computer-use-overlay.exe`；该窗口由此受监管的原生进程通过有界 IPC 驱动，并由 kill-on-close Job Object 随客户端一同终止。

## 架构

```mermaid
flowchart LR
    UI["原生 GPUI"] --> Core["codex-core<br/>状态 + effects"]
    Core --> Platform["codex-platform"]
    Core --> Store["codex-storage<br/>自有界面状态"]
    Platform --> AppServer["官方 codex app-server"]
    AppServer --> Home["~/.codex<br/>由 app-server 管理"]
    Platform --> Git["Git"]
    Platform --> PTY["PTY / ConPTY"]
    Platform --> CU["原生 Computer Use"]
```

协议帧、队列、历史分页、差异、终端输出、截图和进程诊断均具有明确上限。详见
[架构说明](docs/architecture.md)、[对等矩阵](docs/parity-matrix.md)与
[平台支持](docs/platform-support.md)。

## 快速开始

### 1. 安装官方 Codex CLI

按照最新的 [Codex CLI 指南](https://learn.chatgpt.com/docs/codex/cli)安装，
然后运行一次 `codex` 完成登录。codexRS 只负责监管 CLI 的原生
`app-server`，自身不依赖 Node.js。

```text
codex --version
```

如果 CLI 不在 `PATH` 中，请将 `CODEX_RS_CODEX_BIN` 设置为原生
`codex` 或 `codex.exe` 的路径。

### 2. 下载便携预览版

仅从 [GitHub Releases](https://github.com/Kiwunaka/codexRS/releases)
页面下载。RC2 文件为 `codexrs-v0.1.0-rc.2-windows-x86_64.zip` 和
`codexrs-v0.1.0-rc.2-linux-x86_64.tar.gz`，并附有 `SHA256SUMS.txt`。
解压前请校验 checksum：Linux 可运行
`grep ' \./codexrs-v0.1.0-rc.2-linux-x86_64.tar.gz$' SHA256SUMS.txt | sha256sum -c -`；Windows
可将 `(Get-FileHash .\codexrs-v0.1.0-rc.2-windows-x86_64.zip -Algorithm SHA256).Hash`
与对应条目比较。checksum 只能在从可信 release 页面取得后发现损坏，不能替代
独立的发布者签名。

这些是未签名的便携技术预览包，不会安装 Start Menu 项、URI handler、卸载程序或
自动更新程序。请解压到由你控制的目录。Windows 上必须将 `codexrs.exe` 与
`codex-computer-use-overlay.exe` 保留在同一目录。更新时先退出 codexRS，再替换
解压目录；删除时只删除该目录。两者都不会删除 `CODEX_HOME` 或 codexRS 自己的
状态数据。不要从来源或 checksum 不可信的归档中启用 Computer Use。

Linux 归档不是系统软件包：它不会安装 runtime 依赖或 desktop integration。
Ubuntu CI 会构建并测试二进制文件，但解压后的归档尚未完成 desktop smoke 测试。RC2
中 Linux Computer Use 不可用；X11/XWayland 与纯 Wayland 支持仍是未来工作。

### 3. 构建 codexRS

安装 Git、通过 `rustup` 安装 Rust，并准备[平台支持](docs/platform-support.md)
中列出的原生依赖：

```text
git clone https://github.com/Kiwunaka/codexRS.git
cd codexRS
cargo build --release -p codex-app
```

Windows：

```powershell
.\target\release\codexrs.exe
```

Linux：

```bash
./target/release/codexrs
```

Linux 构建依赖与桌面会话限制请参阅[平台支持](docs/platform-support.md)。

### 隔离开发环境

正常使用默认指向 `~/.codex`。协议开发和测试应将 `CODEX_HOME` 指向独立目录：

```powershell
$env:CODEX_HOME = 'E:\scratch\isolated-codex-home'
cargo build -p codex-app --bins
cargo run -p codex-app --bin codexrs
```

可使用 `CODEX_RS_DATA_DIR` 单独重定向 codexRS 自有数据。

## 参与贡献

欢迎贡献。请先阅读 [CONTRIBUTING.md](CONTRIBUTING.md) 与
[AGENTS.md](AGENTS.md)。大型功能应先通过 issue 或 Discussions 明确行为契约。

- [路线图](ROADMAP.md)
- [Codex Desktop 对等矩阵](docs/parity-matrix.md)
- [支持](SUPPORT.md)
- [安全策略](SECURITY.md)
- [行为准则](CODE_OF_CONDUCT.md)
- [同类项目调研](docs/prior-art.md)

## 贡献者

<a href="https://github.com/Kiwunaka/codexRS/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=Kiwunaka/codexRS" alt="codexRS 贡献者">
</a>

## Star 趋势

[![Star History Chart](https://api.star-history.com/svg?repos=Kiwunaka/codexRS&type=Date)](https://star-history.com/#Kiwunaka/codexRS&Date)

## 许可证与上游声明

codexRS 使用 [Apache License 2.0](LICENSE)。

这是一个独立社区项目，与 OpenAI 无隶属或背书关系。Codex 与 OpenAI 名称归其各自所有者。官方 Codex CLI 需单独安装并遵循其许可。
