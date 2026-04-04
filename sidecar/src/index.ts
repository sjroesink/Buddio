import * as readline from "node:readline";
import type { IncomingMessage, OutgoingMessage } from "./protocol.js";
import type { SidecarProvider } from "./providers/base.js";
import { ClaudeProvider } from "./providers/claude.js";
import { CopilotProvider } from "./providers/copilot.js";
import { CodexProvider } from "./providers/codex.js";
import { OllamaProvider } from "./providers/ollama.js";
import { McpConnection } from "./mcp.js";

// --- JSON lines I/O ---

function send(msg: OutgoingMessage): void {
  process.stdout.write(JSON.stringify(msg) + "\n");
}

function log(message: string): void {
  process.stderr.write(`[sidecar] ${message}\n`);
}

// --- Main ---

let provider: SidecarProvider | null = null;
let mcpConnection: McpConnection | null = null;

const rl = readline.createInterface({
  input: process.stdin,
  crlfDelay: Infinity,
});

rl.on("line", async (line: string) => {
  let msg: IncomingMessage;
  try {
    msg = JSON.parse(line) as IncomingMessage;
  } catch {
    log(`Invalid JSON: ${line}`);
    return;
  }

  try {
    await handleMessage(msg);
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    log(`Error handling message: ${message}`);
    send({ type: "error", message });
  }
});

rl.on("close", () => {
  cleanup().then(() => process.exit(0));
});

async function handleMessage(msg: IncomingMessage): Promise<void> {
  switch (msg.type) {
    case "init": {
      // Create provider
      if (msg.provider === "claude") {
        provider = new ClaudeProvider();
      } else if (msg.provider === "copilot") {
        provider = new CopilotProvider();
      } else if (msg.provider === "codex") {
        provider = new CodexProvider();
      } else if (msg.provider === "ollama") {
        provider = new OllamaProvider();
      } else {
        send({ type: "error", message: `Unknown provider: ${msg.provider}` });
        return;
      }

      if (msg.provider === "claude") {
        // Claude provider uses the Agent SDK which manages MCP connections internally
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
      } else {
        // Other providers use a shared MCP connection
        mcpConnection = new McpConnection();
        let tools;
        try {
          tools = await mcpConnection.connect(msg.mcp_binary);
          log(`Connected to MCP, ${tools.length} tools available`);
        } catch (err) {
          const message = err instanceof Error ? err.message : String(err);
          send({ type: "error", message: `Failed to connect to MCP: ${message}` });
          send({ type: "status_change", status: "error" });
          return;
        }

        await provider.init(
          { apiKey: msg.config.api_key, model: msg.config.model },
          tools,
          send,
        );
      }
      send({ type: "status_change", status: "connected" });
      break;
    }

    case "prompt": {
      if (!provider) {
        send({ type: "error", message: "Not initialized" });
        return;
      }

      if (mcpConnection) {
        const mcp = mcpConnection;
        await provider.prompt(msg.text, (name, args) => mcp.callTool(name, args));
      } else {
        // Claude provider manages its own MCP connections via the Agent SDK
        await provider.prompt(msg.text, async () => ({ content: "", isError: true }));
      }
      break;
    }

    case "cancel": {
      provider?.cancel();
      break;
    }

    case "resolve_permission": {
      provider?.resolvePermission?.(msg.request_id, msg.option_id);
      break;
    }

    case "resolve_question": {
      provider?.resolveQuestion?.(msg.request_id, msg.answers);
      break;
    }

    case "resolve_replace_selection": {
      provider?.resolveReplacement?.(msg.request_id, msg.success);
      break;
    }

    case "shutdown": {
      await cleanup();
      process.exit(0);
    }
  }
}

async function cleanup(): Promise<void> {
  try {
    await mcpConnection?.disconnect();
  } catch {
    // Ignore cleanup errors
  }
}
