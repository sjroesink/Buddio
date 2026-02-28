#!/usr/bin/env bash
set -euo pipefail

api_url="https://api.github.com/repos/sjroesink/Buddio/releases/latest"
dmg_url="$(curl -fsSL "$api_url" | grep -Eo 'https://[^"]+\.dmg' | head -n 1 || true)"

if [[ -z "$dmg_url" ]]; then
  echo "No macOS .dmg asset found in the latest release." >&2
  exit 1
fi

target="/tmp/Buddio.dmg"

echo "Downloading Buddio DMG..."
curl -fL "$dmg_url" -o "$target"

echo "Opening installer..."
open "$target"
echo "Mounted $target. Drag Buddio into Applications to finish install."
