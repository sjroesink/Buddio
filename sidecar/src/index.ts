import * as readline from "node:readline";
import type { IncomingMessage, OutgoingMessage } from "./protocol.js";
import type { SidecarProvider } from "./providers/base.js";
import { ClaudeProvider } from "./providers/claude.js";
import { CopilotProvider } from "./providers/copilot.js";
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
      } else {
        send({ type: "error", message: `Unknown provider: ${msg.provider}` });
        return;
      }

      // Connect to MCP server
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

      // Initialize provider with tools
      await provider.init(
        { apiKey: msg.config.api_key, model: msg.config.model },
        tools,
        send,
      );
      send({ type: "status_change", status: "connected" });
      break;
    }

    case "prompt": {
      if (!provider || !mcpConnection) {
        send({ type: "error", message: "Not initialized" });
        return;
      }

      const mcp = mcpConnection;
      await provider.prompt(msg.text, (name, args) => mcp.callTool(name, args));
      break;
    }

    case "cancel": {
      provider?.cancel();
      break;
    }

    case "resolve_permission": {
      if (provider && "resolvePermission" in provider) {
        (provider as ClaudeProvider).resolvePermission(
          msg.request_id,
          msg.option_id,
        );
      }
      break;
    }

    case "resolve_question": {
      if (provider && "resolveQuestion" in provider) {
        (provider as ClaudeProvider).resolveQuestion(
          msg.request_id,
          msg.answers,
        );
      }
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
