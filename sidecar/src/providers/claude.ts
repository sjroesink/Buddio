import {
  query,
  tool,
  createSdkMcpServer,
  type SDKMessage,
  type SDKPartialAssistantMessage,
  type PermissionResult,
  type CanUseTool,
  type HookCallback,
} from "@anthropic-ai/claude-agent-sdk";
import { z } from "zod";
import type {
  SidecarProvider,
  ProviderConfig,
  McpTool,
  McpToolResult,
  SendFn,
} from "./base.js";

// Read-only Buddio MCP tools that can be auto-allowed
const BUDDIO_READ_TOOLS = new Set([
  "items_get", "items_search", "items_list", "items_get_categories", "items_export",
  "memory_get", "memory_get_by_key", "memory_search", "memory_list", "memory_touch",
  "memory_get_relevant", "history_search", "history_recent", "history_suggest",
  "history_recent_rewrites", "conversations_get", "conversations_list",
  "conversations_search", "conversations_get_messages", "conversations_search_messages",
  "conversations_recent_context", "slash_commands_get", "slash_commands_list",
  "slash_commands_search", "slash_commands_get_params", "settings_get", "settings_list", "db_path",
]);

export class ClaudeProvider implements SidecarProvider {
  private send: SendFn = () => {};
  private model = "claude-sonnet-4-6";
  private apiKey = "";
  private mcpBinaryPath = "";
  private authMethod: "oauth" | "api_key" = "oauth";
  private abortController: AbortController | null = null;
  private pendingPermissions = new Map<string, (optionId: string) => void>();
  private pendingQuestions = new Map<string, (answers: Record<string, string>) => void>();
  private pendingReplacements = new Map<string, (success: boolean) => void>();

  async init(
    config: ProviderConfig,
    _mcpTools: McpTool[],
    send: SendFn,
  ): Promise<void> {
    this.send = send;
    this.model = config.model || "claude-sonnet-4-6";
    this.apiKey = config.apiKey;
    this.mcpBinaryPath = config.mcpBinaryPath ?? "";
    this.authMethod = config.authMethod ?? "oauth";
  }

  async prompt(
    text: string,
    _callTool: (
      name: string,
      args: Record<string, unknown>,
    ) => Promise<McpToolResult>,
  ): Promise<void> {
    this.abortController = new AbortController();

    const customServer = this.createCustomMcpServer();
    let receivedResult = false;

    try {
      const q = query({
        prompt: text,
        options: {
          abortController: this.abortController,
          model: this.model,
          tools: [],
          mcpServers: {
            ...(this.mcpBinaryPath
              ? { buddio: { command: this.mcpBinaryPath, args: [] as string[] } }
              : {}),
            custom: customServer,
          },
          includePartialMessages: true,
          persistSession: false,
          canUseTool: ((toolName, _input, options) =>
            this.handlePermission(toolName, options.toolUseID)) as CanUseTool,
          env: (() => {
            const env = { ...process.env };
            // Remove CLAUDECODE to allow nested Claude Code subprocess
            delete env.CLAUDECODE;
            if (this.authMethod === "api_key" && this.apiKey) {
              env.ANTHROPIC_API_KEY = this.apiKey;
            }
            return env;
          })(),
          settings: this.authMethod === "oauth"
            ? { forceLoginMethod: "claudeai" as const }
            : undefined,
          hooks: {
            PreToolUse: [{
              hooks: [this.createPreToolUseHook()],
            }],
            PostToolUse: [{
              hooks: [this.createPostToolUseHook("Complete")],
            }],
            PostToolUseFailure: [{
              hooks: [this.createPostToolUseHook("Error")],
            }],
          },
        },
      });

      for await (const message of q) {
        if (message.type === "result") receivedResult = true;
        this.processMessage(message);
      }

      this.send({ type: "turn_complete", stop_reason: "end_turn" });
    } catch (err) {
      if (this.abortController?.signal.aborted) {
        this.send({ type: "turn_complete", stop_reason: "Cancelled" });
      } else if (receivedResult) {
        // SDK may throw after yielding a result (e.g. process exit code 1 during cleanup).
        // The result message already handled any errors, so just complete the turn.
        this.send({ type: "turn_complete", stop_reason: "end_turn" });
      } else {
        const message = err instanceof Error ? err.message : String(err);
        this.send({ type: "error", message });
        this.send({ type: "turn_complete", stop_reason: `Error: ${message}` });
      }
    } finally {
      this.abortController = null;
    }
  }

  cancel(): void {
    this.abortController?.abort();
    for (const [, resolver] of this.pendingPermissions) {
      resolver("__cancelled__");
    }
    this.pendingPermissions.clear();
    for (const [, resolver] of this.pendingQuestions) {
      resolver({ __cancelled__: "true" });
    }
    this.pendingQuestions.clear();
    for (const [, resolver] of this.pendingReplacements) {
      resolver(false);
    }
    this.pendingReplacements.clear();
  }

  resolvePermission(requestId: string, optionId: string): void {
    const resolver = this.pendingPermissions.get(requestId);
    if (resolver) {
      this.pendingPermissions.delete(requestId);
      resolver(optionId);
    }
  }

  resolveQuestion(requestId: string, answers: Record<string, string>): void {
    const resolver = this.pendingQuestions.get(requestId);
    if (resolver) {
      this.pendingQuestions.delete(requestId);
      resolver(answers);
    }
  }

  resolveReplacement(requestId: string, success: boolean): void {
    const resolver = this.pendingReplacements.get(requestId);
    if (resolver) {
      this.pendingReplacements.delete(requestId);
      resolver(success);
    }
  }

  private processMessage(message: SDKMessage): void {
    switch (message.type) {
      case "stream_event": {
        const { event } = message as SDKPartialAssistantMessage;
        if (event.type === "content_block_delta") {
          const delta = event.delta as unknown as Record<string, unknown>;
          if (delta.type === "text_delta" && typeof delta.text === "string") {
            this.send({ type: "message_chunk", text: delta.text });
          } else if (delta.type === "thinking_delta" && typeof delta.thinking === "string") {
            this.send({ type: "thought_chunk", text: delta.thinking });
          }
        }
        break;
      }
      case "result": {
        if ("subtype" in message && message.subtype !== "success") {
          const errors = "errors" in message ? (message as { errors: string[] }).errors : [];
          if (errors.length > 0) {
            this.send({ type: "error", message: errors.join("; ") });
          }
        }
        break;
      }
      case "auth_status": {
        const authMsg = message as unknown as {
          type: "auth_status";
          isAuthenticating: boolean;
          output: string[];
          error?: string;
        };
        this.send({
          type: "auth_status",
          is_authenticating: authMsg.isAuthenticating,
          auth_url: authMsg.output?.find((line: string) => line.startsWith("http")) ?? null,
          error: authMsg.error ?? null,
        });
        break;
      }
    }
  }

  private createCustomMcpServer() {
    const self = this;

    const askUserQuestionTool = tool(
      "AskUserQuestion",
      "Ask the user one or more clarifying questions when you need more information to proceed. " +
        "Each question should have 2-4 predefined options for the user to choose from. " +
        "Use this when the task has multiple valid approaches and you need user input to decide.",
      {
        questions: z.array(z.object({
          question: z.string().describe("The question text to display"),
          header: z.string().describe("Short label (max 12 chars)"),
          options: z.array(z.object({
            label: z.string().describe("Option label"),
            description: z.string().describe("Option description"),
          })).min(2).max(4),
          multiSelect: z.boolean().describe("Allow multiple selections"),
        })).min(1).max(4),
      },
      async (args) => {
        const requestId = crypto.randomUUID();
        self.send({
          type: "user_question",
          request_id: requestId,
          tool_use_id: requestId,
          questions: args.questions,
        });

        const answers = await new Promise<Record<string, string>>((resolve) => {
          self.pendingQuestions.set(requestId, resolve);
        });

        if ("__cancelled__" in answers) {
          return { content: [{ type: "text" as const, text: "User cancelled the question" }], isError: true };
        }

        return {
          content: [{ type: "text" as const, text: JSON.stringify({ questions: args.questions, answers }) }],
        };
      },
    );

    const replaceSelectionTool = tool(
      "replace_selection",
      "Replace the user's currently selected text with new text. " +
        "Use this when the user asks to rewrite, rephrase, translate, summarize, or transform their selected text. " +
        "Only use this tool when there is selected text in the context.",
      {
        text: z.string().describe("The replacement text to insert in place of the current selection"),
      },
      async (args) => {
        const requestId = crypto.randomUUID();

        self.send({
          type: "replace_selection_request",
          request_id: requestId,
          text: args.text,
        });

        const success = await new Promise<boolean>((resolve) => {
          self.pendingReplacements.set(requestId, resolve);
        });

        return {
          content: [{ type: "text" as const, text: success ? "Selection replaced successfully" : "Failed to replace selection" }],
          isError: !success,
        };
      },
    );

    return createSdkMcpServer({
      name: "buddio-custom",
      tools: [askUserQuestionTool, replaceSelectionTool],
    });
  }

  private async handlePermission(toolName: string, toolUseId: string): Promise<PermissionResult> {
    // Always allow custom tools
    if (toolName === "AskUserQuestion" || toolName === "replace_selection") {
      return { behavior: "allow" };
    }

    // Auto-allow read-only tools
    if (BUDDIO_READ_TOOLS.has(toolName)) {
      return { behavior: "allow" };
    }

    // Ask user for permission on write tools
    this.send({
      type: "permission_request",
      request_id: toolUseId,
      session_id: "sidecar",
      tool_name: toolName,
      options: [
        { option_id: "allow", name: "Allow", kind: "Allow" },
        { option_id: "deny", name: "Deny", kind: "Deny" },
      ],
    });

    const optionId = await new Promise<string>((resolve) => {
      this.pendingPermissions.set(toolUseId, resolve);
    });

    if (optionId === "__cancelled__" || optionId === "deny") {
      return { behavior: "deny", message: "Permission denied by user" };
    }

    return { behavior: "allow" };
  }

  private createPreToolUseHook(): HookCallback {
    const self = this;
    return async (input) => {
      const { tool_name, tool_input, tool_use_id } = input as {
        tool_name: string;
        tool_input: unknown;
        tool_use_id: string;
      };

      if (tool_name !== "AskUserQuestion") {
        const kind = tool_name === "replace_selection" ? "custom" : "mcp";
        self.send({
          type: "tool_call",
          id: tool_use_id,
          title: tool_name,
          kind,
          content: kind === "mcp" ? JSON.stringify(tool_input) : null,
        });
      }

      return {};
    };
  }

  private createPostToolUseHook(status: string): HookCallback {
    const self = this;
    return async (input) => {
      const { tool_name, tool_use_id } = input as {
        tool_name: string;
        tool_use_id: string;
      };

      if (tool_name !== "AskUserQuestion") {
        self.send({
          type: "tool_call_update",
          id: tool_use_id,
          title: null,
          status,
        });
      }

      return {};
    };
  }
}
