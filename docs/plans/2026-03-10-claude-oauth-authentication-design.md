# Claude OAuth Authentication

## Problem
Users must manually paste API keys to use Claude. We want OAuth login so users can authenticate with their Claude Pro/Max subscription directly.

## Design

### Auth method selection
- Claude settings tab: toggle between "Claude Account (OAuth)" (default) and "API Key"
- Stored in `agent.claude.auth_method` setting: `"oauth"` | `"api_key"`

### OAuth flow
- Agent SDK handles the full OAuth flow internally when no `ANTHROPIC_API_KEY` is set
- On connect: sidecar starts `query()` without API key env var
- SDK detects no key, initiates OAuth, emits `SDKAuthStatusMessage` events
- Sidecar forwards auth status to Rust via new `auth_status` protocol message
- Rust opens browser via `tauri::shell::open` when SDK provides auth URL
- Existing status indicator shows "connecting" -> "connected"
- SDK persists OAuth tokens in `~/.claude/` — no token management needed on our side
- `forceLoginMethod: 'claudeai'` in SDK settings ensures subscription billing

### Auto-connect at app start
- `connectOnStartup` adjusted: when `auth_method === "oauth"`, connect without requiring API key
- Sidecar always starts when a provider is configured

### Protocol changes
- New outgoing message: `auth_status { is_authenticating: bool, auth_url?: string, error?: string }`
- `InitMessage.config.api_key` becomes optional (empty string for OAuth)
- New field: `InitMessage.config.auth_method: "oauth" | "api_key"`

### Files to change

| File | Change |
|------|--------|
| `sidecar/src/providers/base.ts` | Add `authMethod` to `ProviderConfig` |
| `sidecar/src/providers/claude.ts` | Omit API key env for OAuth, forward `auth_status` messages, set `forceLoginMethod` |
| `sidecar/src/protocol.ts` | Add `AuthStatusOut` type, add `auth_method` to `InitMessage.config` |
| `sidecar/src/index.ts` | Forward auth status messages |
| `src-tauri/src/agent/sidecar.rs` | Handle `auth_status` messages, open browser for auth URL |
| `src-tauri/src/agent/types.rs` | Add `auth_method` to config types |
| `src-tauri/src/commands.rs` | Save/load `auth_method` setting |
| `src/components/AgentSettings.tsx` | OAuth/API key toggle, hide API key field when OAuth selected |
| `src/hooks/useAgent.ts` | Allow auto-connect without API key when OAuth |
| `src/types.ts` | Add `auth_method` to TypeScript config types |
