#!/usr/bin/env bash
set -euo pipefail

# Serve OrcaSlicer WebConfig on port 8003 (default) via Bazel.
PORT="${PORT:-8003}"
export HTTP_LISTEN_ADDR="${HTTP_LISTEN_ADDR:-0.0.0.0:${PORT}}"

exec bazel run "$@" //server:server
