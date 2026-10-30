#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="${ROOT_DIR}/dist"

mkdir -p "${DIST_DIR}/doc/images"
cp -f "${ROOT_DIR}/static/index.html" "${DIST_DIR}/index.html"
cp -f "${ROOT_DIR}/bazel-bin/app_wasm/app_wasm.js" "${DIST_DIR}/app_wasm.js"

if [ -f "${ROOT_DIR}/bazel-bin/app_wasm/app_wasm_bg_opt.wasm" ]; then
  cp -f "${ROOT_DIR}/bazel-bin/app_wasm/app_wasm_bg_opt.wasm" "${DIST_DIR}/app_wasm_bg.wasm"
else
  cp -f "${ROOT_DIR}/bazel-bin/app_wasm/app_wasm_bg.wasm" "${DIST_DIR}/app_wasm_bg.wasm"
fi

if [ -f "${ROOT_DIR}/bazel-bin/app_wasm/app_wasm_bg_opt.wasm.br" ]; then
  cp -f "${ROOT_DIR}/bazel-bin/app_wasm/app_wasm_bg_opt.wasm.br" "${DIST_DIR}/app_wasm_bg.wasm.br"
fi

if compgen -G "${ROOT_DIR}/doc/images/*" > /dev/null; then
  cp -rf "${ROOT_DIR}/doc/images/"* "${DIST_DIR}/doc/images/"
fi
chmod -R a+rX "${DIST_DIR}"

# GitHub Pages: bypass Jekyll processing
: > "${DIST_DIR}/.nojekyll"

# Cloudflare Pages: explicit MIME type & caching headers for WASM
cat > "${DIST_DIR}/_headers" << 'EOF'
/*.wasm
  Content-Type: application/wasm
  Cache-Control: public, max-age=3600, must-revalidate
EOF

echo "Staged Yew WASM frontend into ${DIST_DIR} (Tauri v2 / GitHub Pages / Cloudflare Pages)."
