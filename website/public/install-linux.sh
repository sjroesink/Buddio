#!/usr/bin/env bash
set -euo pipefail

api_url="https://api.github.com/repos/sjroesink/Buddio/releases/latest"

appimage_url="$(curl -fsSL "$api_url" | grep -Eo 'https://[^"]+\.AppImage' | head -n 1 || true)"
if [[ -n "$appimage_url" ]]; then
  mkdir -p "${HOME}/.local/bin"
  target="${HOME}/.local/bin/buddio.AppImage"

  echo "Downloading Buddio AppImage..."
  curl -fL "$appimage_url" -o "$target"
  chmod +x "$target"

  echo "Launching Buddio..."
  "$target" >/dev/null 2>&1 &
  echo "Installed to $target"
  exit 0
fi

deb_url="$(curl -fsSL "$api_url" | grep -Eo 'https://[^"]+\.deb' | head -n 1 || true)"
if [[ -z "$deb_url" ]]; then
  echo "No Linux AppImage or .deb asset found in the latest release." >&2
  exit 1
fi

target="/tmp/buddio.deb"
echo "Downloading Buddio .deb..."
curl -fL "$deb_url" -o "$target"

if command -v apt-get >/dev/null 2>&1; then
  echo "Installing Buddio via apt..."
  sudo apt-get install -y "$target"
  echo "Install completed."
else
  echo "Downloaded $target. Install manually with your package manager."
fi
