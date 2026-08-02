#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 <codexrs-binary>" >&2
  exit 2
fi

binary=$1
if [[ ! -x "$binary" ]]; then
  echo "Linux desktop smoke binary is not executable: $binary" >&2
  exit 2
fi

smoke_root=$(mktemp -d)
cleanup() {
  rm -rf -- "$smoke_root"
}
trap cleanup EXIT

install -d -m 0700 \
  "$smoke_root/home" \
  "$smoke_root/data" \
  "$smoke_root/runtime" \
  "$smoke_root/codex"
log_file="$smoke_root/codexrs.log"

set +e
HOME="$smoke_root/home" \
XDG_DATA_HOME="$smoke_root/data" \
XDG_RUNTIME_DIR="$smoke_root/runtime" \
CODEX_HOME="$smoke_root/codex" \
CODEX_RS_DATA_DIR="$smoke_root/data/codexrs" \
CODEX_RS_CODEX_BIN=/bin/false \
LIBGL_ALWAYS_SOFTWARE=1 \
xvfb-run --auto-servernum --server-args="-screen 0 1280x800x24 -nolisten tcp" \
  timeout --signal=TERM --kill-after=5s 15s "$binary" \
  >"$log_file" 2>&1
status=$?
set -e

if [[ $status -ne 124 ]]; then
  echo "Linux desktop smoke exited before the expected timeout (status $status)" >&2
  tail -c 16384 "$log_file" >&2
  exit 1
fi
