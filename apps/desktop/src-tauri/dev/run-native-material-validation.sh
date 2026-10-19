#!/bin/zsh
# Development-only launcher for validating the local material sidecar from the
# native command path. It deliberately execs the binary instead of using
# LaunchServices, whose environment is not a reliable source for dev variables.
set -euo pipefail

script_dir="${0:A:h}"
repo_root="${FOCAUS_REPO_ROOT:-${script_dir:h:h:h:h}}"
binary="${FOCAUS_DESKTOP_BINARY:-$repo_root/target/debug/focaus}"
engine="${FOCAUS_MATERIAL_ENGINE:-$repo_root/engines/material}"
python="${FOCAUS_MATERIAL_ENGINE_PYTHON:-}"

if [[ -z "$python" ]]; then
  python="$(command -v python3 || true)"
fi

if [[ ! -x "$binary" ]]; then
  print -u2 "native material validation binary is missing: $binary"
  print -u2 "build apps/desktop/src-tauri with the native-shell feature first"
  exit 2
fi
if [[ ! -d "$engine" ]]; then
  print -u2 "material engine directory is missing: $engine"
  print -u2 "set FOCAUS_MATERIAL_ENGINE to an explicit local checkout"
  exit 2
fi
if [[ -z "$python" || ! -x "$python" ]]; then
  print -u2 "a usable Python executable is required"
  print -u2 "set FOCAUS_MATERIAL_ENGINE_PYTHON to an explicit local interpreter"
  exit 2
fi

export FOCAUS_MATERIAL_ENGINE="$engine"
export FOCAUS_MATERIAL_ENGINE_PYTHON="$python"
if [[ "${1:-}" == "--check" ]]; then
  print "FOCAUS_MATERIAL_ENGINE=$FOCAUS_MATERIAL_ENGINE"
  print "FOCAUS_MATERIAL_ENGINE_PYTHON=$FOCAUS_MATERIAL_ENGINE_PYTHON"
  print "FOCAUS_DESKTOP_BINARY=$binary"
  exit 0
fi
exec "$binary" "$@"
