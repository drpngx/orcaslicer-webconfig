#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="${ROOT_DIR}/dist"

mkdir -p "${DIST_DIR}"
cp -f "${ROOT_DIR}/static/index.html" "${DIST_DIR}/index.html"
cp -f "${ROOT_DIR}/bazel-bin/app_wasm/app_wasm.js" "${DIST_DIR}/app_wasm.js"
cp -f "${ROOT_DIR}/bazel-bin/app_wasm/app_wasm_bg.wasm" "${DIST_DIR}/app_wasm_bg.wasm"

echo "Staged Yew WASM frontend into ${DIST_DIR} for Tauri v2 packaging."
