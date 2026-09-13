#!/usr/bin/env bash
set -euo pipefail
APP_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
TOOL_ENV="${TIME_LEDGER_TOOL_ENV:-$APP_DIR/.local/toolchains-env.sh}"
if [ ! -f "$TOOL_ENV" ]; then TOOL_ENV="$APP_DIR/../../work/toolchains/env.sh"; fi
if [ -f "$TOOL_ENV" ]; then source "$TOOL_ENV"; fi
cd "$APP_DIR"
exec "$@"
