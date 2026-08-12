#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

PNG="assets/logo/printy-logo-1024.png"

if [ ! -f "$PNG" ]; then
  echo "missing $PNG — render assets/logo/printy-logo.svg at 1024x1024 and commit it" >&2
  exit 1
fi

# Emits src-tauri/icons/: icon.ico (16/24/32/48/64/256), icon.icns, the PNG
# ladder and the Windows Store Square*Logo set.
npm run tauri -- icon "$PNG"

echo "icons regenerated from $PNG"
