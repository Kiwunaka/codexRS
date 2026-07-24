# Platform support

## Support matrix

| Platform | Build | UI and app-server | Git and terminal | Computer Use |
| --- | --- | --- | --- | --- |
| Windows 10/11 x64 | Supported | Locally smoke-tested | Native Git and ConPTY; child trees use Job Objects | Supported and smoke-tested |
| Ubuntu 24.04 x64 | CI | Native build and tests in CI | Native Git and PTY | X11/XWayland only for the RC |
| Other Linux distributions | Source | Expected with equivalent native packages | Expected | Not release-qualified |
| macOS | Not targeted | Not supported in this release | Not supported | Not supported |

The stable compatibility oracle is Windows Codex Desktop `26.721.3996.0` with
Codex CLI `0.146.0-alpha.3.1`. The runtime executable is still the user's
separately installed official Codex CLI.

## Windows

Requirements:

- Windows 10 or 11 on x64;
- Git;
- Rust `1.97.1` through `rustup`;
- Visual Studio 2022 Build Tools with Desktop development with C++ and a
  Windows 10/11 SDK;
- the official Codex CLI, signed in once.

Optimized GPUI builds compile their Windows shaders with the SDK's x64
`fxc.exe`. If the compiler is installed but is not on `PATH`, point GPUI at it:

```powershell
$fxc = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin" `
  -Filter fxc.exe -Recurse |
  Where-Object FullName -Match '\\x64\\fxc\.exe$' |
  Sort-Object FullName -Descending |
  Select-Object -First 1
$env:GPUI_FXC_PATH = $fxc.FullName
```

codexRS resolves the CLI in this order:

1. an explicit probe argument;
2. `CODEX_RS_CODEX_BIN`;
3. the native executable inside the official global npm package, when present;
4. `codex.exe` on `PATH`.

The npm location is only a binary discovery path. Node.js is not started or
linked by codexRS.

## Linux

The Ubuntu CI installs these native build dependencies:

```bash
sudo apt-get install --yes --no-install-recommends \
  clang \
  libclang-dev \
  libdrm-dev \
  libegl1-mesa-dev \
  libfontconfig1-dev \
  libfreetype6-dev \
  libgbm-dev \
  libgl1-mesa-dev \
  libpipewire-0.3-dev \
  libwayland-dev \
  libx11-dev \
  libx11-xcb-dev \
  libxcb-render0-dev \
  libxcb-shape0-dev \
  libxcb-xfixes0-dev \
  libxcb1-dev \
  libxkbcommon-dev \
  libxkbcommon-x11-dev \
  pkg-config
```

Package names vary across distributions.

GPUI can run on native Linux desktops, but the current Computer Use window
enumeration path is XCB-based. Use an X11 session or XWayland for RC testing.
Pure Wayland support requires a portal-backed selection and input path and is
tracked for a later candidate.

## Runtime directories

The two data boundaries are independent:

| Variable | Purpose | Default |
| --- | --- | --- |
| `CODEX_HOME` | Official app-server-owned Codex state | `~/.codex` |
| `CODEX_RS_DATA_DIR` | codexRS-owned UI preferences and recent workspaces | OS data directory |
| `CODEX_RS_CODEX_BIN` | Native official Codex executable | `codex` / `codex.exe` resolution |

Never point development tests at a live `CODEX_HOME`. Create an isolated
directory and keep fixtures free of credentials and user history.
