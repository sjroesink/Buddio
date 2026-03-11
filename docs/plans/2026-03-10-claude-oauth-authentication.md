# Claude OAuth Authentication Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Allow users to authenticate Claude via OAuth (Claude Pro/Max subscription) instead of API keys.

**Architecture:** Add `auth_method` field (`"oauth"` | `"api_key"`) to the agent config. When OAuth is selected, the sidecar starts the Agent SDK without an API key — the SDK initiates its own OAuth flow, opening the browser automatically. Auth status events flow from the SDK → sidecar → Rust → frontend via the existing event system. The SDK persists tokens in `~/.claude/`.

**Tech Stack:** Tauri (Rust), React (TypeScript), Node.js sidecar, Claude Agent SDK

---

### Task 1: Add `auth_method` to config types

**Files:**
- Modify: `src-tauri/src/agent/types.rs:114-128` (AgentConfig struct)
- Modify: `src/types.ts:109-119` (AgentConfig interface)

**Step 1: Add `auth_method` to Rust AgentConfig**

In `src-tauri/src/agent/types.rs`, add field to `AgentConfig`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentConfig {
    #[serde(default)]
    pub provider: String,
    pub source: String,
    pub agent_id: String,
    pub binary_path: String,
    pub args: String,
    pub env: String,
    pub auto_fallback: bool,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub model: String,
    #[serde(default = "default_auth_method")]
    pub auth_method: String,
}

fn default_auth_method() -> String {
    "oauth".to_string()
}
```

**Step 2: Add `auth_method` to TypeScript AgentConfig**

In `src/types.ts`, add field:

```typescript
export interface AgentConfig {
  provider: ProviderKind;
  source: string;
  agent_id: string;
  binary_path: string;
  args: string;
  env: string;
  auto_fallback: boolean;
  api_key: string;
  model: string;
  auth_method: "oauth" | "api_key";
}
```

**Step 3: Commit**

```bash
git add src-tauri/src/agent/types.rs src/types.ts
git commit -m "feat: add auth_method field to AgentConfig"
```

---

### Task 2: Persist and load `auth_method` in Rust commands

**Files:**
- Modify: `src-tauri/src/commands.rs:220-263` (save_agent_config, load_agent_config)

**Step 1: Save `auth_method` in `save_agent_config`**

Add after line 238 (`agent.claude.model`):

```rust
db.set_setting("agent.claude.auth_method", &config.auth_method)?;
```

**Step 2: Load `auth_method` in `load_agent_config`**

Add to the `AgentConfig` struct literal, after the `model` field:

```rust
auth_method: db
    .get_setting("agent.claude.auth_method")?
    .unwrap_or_else(|| "oauth".to_string()),
```

**Step 3: Commit**

```bash
git add src-tauri/src/commands.rs
git commit -m "feat: persist auth_method setting in database"
```

---

### Task 3: Add `auth_status` to sidecar protocol

**Files:**
- Modify: `sidecar/src/protocol.ts` (add AuthStatusOut, add auth_method to InitMessage)

**Step 1: Add `auth_method` to InitMessage config**

In `sidecar/src/protocol.ts`, update the `InitMessage` config:

```typescript
export interface InitMessage {
  type: "init";
  provider: "claude" | "copilot" | "codex";
  config: {
    api_key: string;
    model: string;
    auth_method: "oauth" | "api_key";
  };
  mcp_binary: string;
}
```

**Step 2: Add `AuthStatusOut` to outgoing messages**

Add after `ReplaceSelectionRequestOut`:

```typescript
export interface AuthStatusOut {
  type: "auth_status";
  is_authenticating: boolean;
  auth_url: string | null;
  error: string | null;
}
```

Add `AuthStatusOut` to the `OutgoingMessage` union.

**Step 3: Commit**

```bash
git add sidecar/src/protocol.ts
git commit -m "feat: add auth_status and auth_method to sidecar protocol"
```

---

### Task 4: Add `authMethod` to sidecar provider config

**Files:**
- Modify: `sidecar/src/providers/base.ts:14-18` (ProviderConfig)

**Step 1: Add `authMethod` field**

```typescript
export interface ProviderConfig {
  apiKey: string;
  model: string;
  mcpBinaryPath?: string;
  authMethod?: "oauth" | "api_key";
}
```

**Step 2: Commit**

```bash
git add sidecar/src/providers/base.ts
git commit -m "feat: add authMethod to ProviderConfig"
```

---

### Task 5: Update Claude provider for OAuth

**Files:**
- Modify: `sidecar/src/providers/claude.ts`

**Step 1: Store and use authMethod**

In `init()`, store the auth method:

```typescript
this.authMethod = config.authMethod ?? "oauth";
```

Add field to the class:

```typescript
private authMethod: "oauth" | "api_key" = "oauth";
```

**Step 2: Conditionally set API key and forceLoginMethod**

In `prompt()`, update the `query()` options. Change the `env` block:

```typescript
env: {
  ...process.env,
  ...(this.authMethod === "api_key" && this.apiKey
    ? { ANTHROPIC_API_KEY: this.apiKey }
    : {}),
},
```

Add `settings` option to force claudeai login method for OAuth:

```typescript
settings: this.authMethod === "oauth"
  ? { forceLoginMethod: "claudeai" as const }
  : undefined,
```

**Step 3: Forward auth_status messages**

In `processMessage()`, add a case for `auth_status`:

```typescript
case "auth_status": {
  const msg = message as { type: "auth_status"; isAuthenticating: boolean; output: string[]; error?: string };
  this.send({
    type: "auth_status",
    is_authenticating: msg.isAuthenticating,
    auth_url: msg.output.find((line: string) => line.startsWith("http")) ?? null,
    error: msg.error ?? null,
  });
  break;
}
```

**Step 4: Commit**

```bash
git add sidecar/src/providers/claude.ts
git commit -m "feat: Claude provider OAuth support via Agent SDK"
```

---

### Task 6: Pass auth_method from Rust sidecar to Node

**Files:**
- Modify: `src-tauri/src/agent/sidecar.rs`

**Step 1: Add `auth_method` to `SidecarConfig`**

```rust
#[derive(Debug, Serialize)]
struct SidecarConfig {
    api_key: String,
    model: String,
    auth_method: String,
}
```

**Step 2: Pass auth_method in connect()**

Update the `SidecarCommand::Init` creation (around line 241):

```rust
let _ = stdin_tx_for_init.send(SidecarCommand::Init {
    provider: provider_name,
    config: SidecarConfig {
        api_key,
        model,
        auth_method: config.auth_method.clone(),
    },
    mcp_binary,
});
```

**Step 3: Add `AuthStatus` to `SidecarEvent` enum**

```rust
AuthStatus {
    is_authenticating: bool,
    auth_url: Option<String>,
    error: Option<String>,
},
```

**Step 4: Handle auth_status in the event match**

Add before the `Error` match arm. Open the browser when an auth URL arrives:

```rust
SidecarEvent::AuthStatus {
    is_authenticating,
    auth_url,
    error,
} => {
    if let Some(url) = &auth_url {
        let _ = tauri::async_runtime::spawn(
            tauri::api::shell::open(&app_clone.shell(), url, None)
        );
    }
    if let Some(err) = &error {
        eprintln!("Auth error: {err}");
    }
    // Auth completing is signaled by status_change → connected
    let _ = is_authenticating; // suppress unused warning
}
```

**Step 5: Commit**

```bash
git add src-tauri/src/agent/sidecar.rs
git commit -m "feat: pass auth_method to sidecar, handle auth_status events"
```

---

### Task 7: Pass auth_method through sidecar index.ts

**Files:**
- Modify: `sidecar/src/index.ts`

**Step 1: Pass authMethod from init message to provider**

Update the Claude init block to include `authMethod`:

```typescript
if (msg.provider === "claude") {
  provider = new ClaudeProvider();
  await provider.init(
    {
      apiKey: msg.config.api_key,
      model: msg.config.model,
      mcpBinaryPath: msg.mcp_binary,
      authMethod: msg.config.auth_method,
    },
    [],
    send,
  );
}
```

**Step 2: Commit**

```bash
git add sidecar/src/index.ts
git commit -m "feat: pass auth_method from init message to Claude provider"
```

---

### Task 8: Update Settings UI with OAuth/API key toggle

**Files:**
- Modify: `src/components/AgentSettings.tsx`

**Step 1: Add state for auth method**

Add to the state declarations (around line 113):

```typescript
const [claudeAuthMethod, setClaudeAuthMethod] = useState<"oauth" | "api_key">("oauth");
```

**Step 2: Load saved auth method**

In the `useEffect` init function, after loading config (around line 155), add:

```typescript
if (config.auth_method === "api_key") {
  setClaudeAuthMethod("api_key");
}
```

**Step 3: Update renderClaudeSettings()**

Replace the current `renderClaudeSettings` function:

```tsx
function renderClaudeSettings() {
  return (
    <div className="provider-settings">
      <div className="agent-env-row">
        <label className="agent-env-label">Auth:</label>
        <select
          className="config-option-select"
          value={claudeAuthMethod}
          onChange={(e) => setClaudeAuthMethod(e.target.value as "oauth" | "api_key")}
        >
          <option value="oauth">Claude Account (OAuth)</option>
          <option value="api_key">API Key</option>
        </select>
      </div>
      {claudeAuthMethod === "api_key" && (
        <div className="agent-env-row">
          <label className="agent-env-label">API Key:</label>
          <input
            type="password"
            className="agent-env-input"
            placeholder="sk-ant-..."
            value={claudeApiKey}
            onChange={(e) => setClaudeApiKey(e.target.value)}
          />
        </div>
      )}
      <div className="agent-env-row">
        <label className="agent-env-label">Model:</label>
        <select
          className="config-option-select"
          value={claudeModel}
          onChange={(e) => setClaudeModel(e.target.value)}
        >
          <option value="claude-sonnet-4-20250514">Claude Sonnet 4</option>
          <option value="claude-opus-4-20250514">Claude Opus 4</option>
          <option value="claude-haiku-4-20250506">Claude Haiku 4</option>
        </select>
      </div>
      {claudeAuthMethod === "oauth" && (
        <div className="text-[11px] text-white/40 px-1 mt-1">
          Signs in with your Claude Pro/Max subscription. A browser window will open on first connect.
        </div>
      )}
    </div>
  );
}
```

**Step 4: Update handleConnect() for OAuth**

In the Claude branch of `handleConnect()` (around line 359), replace:

```typescript
} else if (selectedProvider === "claude") {
  if (claudeAuthMethod === "api_key" && !claudeApiKey) return;
  config = {
    provider: "claude",
    source: "sdk",
    agent_id: "claude",
    binary_path: "",
    args: "",
    env: "",
    auto_fallback: autoFallback,
    api_key: claudeAuthMethod === "api_key" ? claudeApiKey : "",
    model: claudeModel,
    auth_method: claudeAuthMethod,
  };
```

**Step 5: Commit**

```bash
git add src/components/AgentSettings.tsx
git commit -m "feat: OAuth/API key toggle in Claude settings UI"
```

---

### Task 9: Allow auto-connect without API key for OAuth

**Files:**
- Modify: `src/hooks/useAgent.ts`

**Step 1: Update the auto-connect guard**

Around line 309, replace:

```typescript
if ((provider === "claude" || provider === "copilot") && !config.api_key?.trim()) {
  return;
}
```

With:

```typescript
if (provider === "claude" && config.auth_method !== "oauth" && !config.api_key?.trim()) {
  return;
}
if (provider === "copilot" && !config.api_key?.trim()) {
  return;
}
```

**Step 2: Commit**

```bash
git add src/hooks/useAgent.ts
git commit -m "feat: allow auto-connect without API key when OAuth is configured"
```

---

### Task 10: Build and verify

**Step 1: Build sidecar**

```bash
cd sidecar && npm run build
```

Expected: Build succeeds without errors.

**Step 2: Type check**

```bash
cd sidecar && npx tsc --noEmit
```

Expected: No TypeScript errors.

**Step 3: Build Rust**

```bash
cargo build
```

Expected: Compiles without errors.

**Step 4: Manual test**

1. Open app → Settings → Claude tab
2. Verify "Auth" dropdown defaults to "Claude Account (OAuth)"
3. API key field is hidden when OAuth selected
4. Switch to "API Key" → API key field appears
5. With OAuth selected, click Connect → browser should open for Claude login
6. After login, status should show "connected"

**Step 5: Commit any fixes, then final commit**

```bash
git add -A
git commit -m "feat: Claude OAuth authentication complete"
```
