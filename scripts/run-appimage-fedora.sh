#!/usr/bin/env bash
set -euo pipefail

# The AppImage's older libwayland-client can conflict with Fedora's Mesa EGL.
# Load the host library before AppRun adds the bundled libraries to its path.
if [[ $# -lt 1 ]]; then
  echo "Usage: $0 /path/to/Time-Ledger.AppImage [arguments...]" >&2
  exit 2
fi

image=$1
shift
if [[ ! -f "$image" || ! -x "$image" ]]; then
  echo "AppImage is missing or not executable: $image" >&2
  exit 1
fi

wayland=/usr/lib64/libwayland-client.so.0
if [[ -r "$wayland" ]]; then
  export LD_PRELOAD="$wayland${LD_PRELOAD:+:$LD_PRELOAD}"
fi
exec "$image" "$@"
