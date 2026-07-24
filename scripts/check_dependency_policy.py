#!/usr/bin/env python3
"""Reject browser-runtime dependencies from the native desktop graph."""

from __future__ import annotations

import shutil
import subprocess
import sys

MAX_LINES = 100_000
MAX_LINE_BYTES = 4_096
FORBIDDEN_NAMES = {
    "electron",
    "node",
    "nodejs",
    "tauri",
    "web-view",
    "webview2",
    "webview2-com",
    "wry",
}
FORBIDDEN_PREFIXES = ("electron-", "tauri-", "webview2-", "wry-")


def main() -> int:
    cargo = shutil.which("cargo")
    if cargo is None:
        print("cargo is unavailable", file=sys.stderr)
        return 2

    process = subprocess.Popen(
        [
            cargo,
            "tree",
            "--workspace",
            "--edges",
            "normal",
            "--prefix",
            "none",
            "--format",
            "{p}",
        ],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    if process.stdout is None:
        process.kill()
        process.wait()
        print("cargo tree output pipe is unavailable", file=sys.stderr)
        return 2
    discovered: set[str] = set()
    diagnostic_tail = ""
    for index, line in enumerate(process.stdout):
        if index >= MAX_LINES:
            process.kill()
            process.wait()
            print("dependency graph exceeds the 100,000-line audit limit", file=sys.stderr)
            return 2
        if len(line.encode("utf-8")) > MAX_LINE_BYTES:
            process.kill()
            process.wait()
            print("dependency graph contains an oversized line", file=sys.stderr)
            return 2
        diagnostic_tail = (diagnostic_tail + line)[-(64 * 1024) :]
        package = line.strip().split(maxsplit=2)
        if len(package) >= 2 and package[1].startswith("v"):
            discovered.add(package[0].lower())

    return_code = process.wait()
    if return_code != 0:
        print(diagnostic_tail.rstrip(), file=sys.stderr)
        return return_code

    forbidden = sorted(
        name
        for name in discovered
        if name in FORBIDDEN_NAMES or name.startswith(FORBIDDEN_PREFIXES)
    )
    if forbidden:
        print(
            "browser-runtime dependency policy violation: " + ", ".join(forbidden),
            file=sys.stderr,
        )
        return 1

    print(f"dependency policy passed for {len(discovered)} packages")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
