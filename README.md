# Buddio

A fast, keyboard-driven launcher inspired by Raycast, built with [Tauri](https://tauri.app/) and React. Items are managed via a CLI, making it ideal for automation by AI agents.

## Architecture

```
Buddio/
├── golaunch-core/      # Shared library (SQLite database + models)
├── golaunch-cli/       # CLI crate (builds `buddio-cli`)
├── src-tauri/          # Tauri desktop application
├── src/                # React frontend (Raycast-like UI)
└── .github/workflows/  # Release automation
```

## Features

- **Raycast-like UI** — Dark, borderless, always-on-top launcher window
- **Keyboard-first** — Navigate with arrow keys, Enter to execute, Esc to dismiss, Tab for categories
- **Global shortcut** — `Ctrl+Space` to toggle the launcher
- **CLI management** — Add, remove, update, search, import/export items via `buddio-cli`
- **SQLite database** — Lightweight, file-based storage shared between app and CLI
- **AI-agent friendly** — JSON output, scriptable CLI, import/export for batch operations
- **Cross-platform** — Linux, macOS, and Windows via Tauri

## CLI Usage

```bash
# Add items
buddio-cli add --title "Google" --action-type url --action-value "https://google.com" --icon "🔍" --category "Web"
buddio-cli add --title "Terminal" --action-type command --action-value "gnome-terminal" --icon "⚡" --category "Apps"
buddio-cli add --title "Deploy" --action-type script --action-value "./deploy.sh" --category "DevOps"

# List all items
buddio-cli list
buddio-cli list --json
buddio-cli list --category Web

# Search
buddio-cli search "google" --json

# Update an item
buddio-cli update <id> --title "New Title" --icon "🚀"

# Remove an item
buddio-cli remove <id>

# Import from JSON
buddio-cli import items.json

# Export all items
buddio-cli export --output backup.json

# Execute an item by ID
buddio-cli run <id>

# Show database location
buddio-cli db-path
```

### Import JSON format

```json
[
  {
    "title": "Google",
    "action_type": "url",
    "action_value": "https://google.com",
    "subtitle": "Search engine",
    "icon": "🔍",
    "category": "Web",
    "tags": "search,web"
  }
]
```

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Ctrl+Space` | Toggle launcher window |
| `↑` / `↓` | Navigate items |
| `Enter` | Execute selected item |
| `Escape` | Clear search / hide window |
| `Tab` / `Shift+Tab` | Cycle through categories |

## Development

### Prerequisites

- [Node.js](https://nodejs.org/) >= 18
- [Rust](https://rustup.rs/) >= 1.70
- System dependencies (Linux): `libwebkit2gtk-4.1-dev libgtk-3-dev libappindicator3-dev librsvg2-dev`

### Setup

```bash
npm install
cargo build --workspace
```

### Run in development

```bash
npx tauri dev
```

### Build for production

```bash
npx tauri build
```

### Build CLI only

```bash
cargo build --release --package buddio-cli
```

## Releases

Releases are automated via GitHub Actions. To create a release:

```bash
git tag v0.1.0
git push origin v0.1.0
```

This triggers the release workflow which:
1. Creates a draft GitHub Release
2. Builds the Tauri app for Linux, macOS (arm64 + x64), and Windows
3. Builds the CLI for all platforms
4. Attaches all artifacts to the release
5. Publishes the release

## Database

Buddio uses SQLite, stored at:
- **Linux**: `~/.local/share/buddio/buddio.db`
- **macOS**: `~/Library/Application Support/buddio/buddio.db`
- **Windows**: `C:\Users\<user>\AppData\Local\buddio\buddio.db`

Legacy GoLaunch databases are still detected automatically at the old path.

Both the Tauri app and CLI share the same database file.

## License

MIT
