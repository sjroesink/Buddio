import { CopilotClient, defineTool } from "@github/copilot-sdk";
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

export class CopilotProvider implements SidecarProvider {
  private client: CopilotClient | null = null;
  private session: ReturnType<CopilotClient["createSession"]> extends Promise<infer T> ? T : never = null as any;
  private send: SendFn = () => {};
  private callTool: ((name: string, args: Record<string, unknown>) => Promise<McpToolResult>) | null = null;
  private pendingPermissions = new Map<string, (optionId: string) => void>();
  private pendingQuestions = new Map<string, (answers: Record<string, string>) => void>();
  private pendingReplacements = new Map<string, (success: boolean) => void>();
  private unsubscribers: (() => void)[] = [];
  private idCounter = 0;

  private nextId(): string {
    return `copilot-${++this.idCounter}-${Date.now()}`;
  }

  async init(
    config: ProviderConfig,
    mcpTools: McpTool[],
    send: SendFn,
  ): Promise<void> {
    this.send = send;

    try {
      this.client = new CopilotClient();
      await this.client.start();
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      send({
        type: "error",
        message: `Failed to start Copilot client: ${message}. Make sure GitHub Copilot CLI is installed and authenticated (https://docs.github.com/en/copilot/how-tos/set-up/install-copilot-cli).`,
      });
      throw err;
    }

    // Convert MCP tools to Copilot custom tools
    const tools = [
      ...mcpTools.map((t) => this.createMcpTool(t)),
      this.createAskUserQuestionTool(),
      this.createReplaceSelectionTool(),
    ];

    this.session = await this.client.createSession({
      model: config.model || "gpt-4.1",
      streaming: true,
      tools,
      onPermissionRequest: async () => ({ kind: "approved" as const }),
    });

    // Subscribe to streaming events
    this.setupEventListeners();
  }

  async prompt(
    text: string,
    callTool: (
      name: string,
      args: Record<string, unknown>,
    ) => Promise<McpToolResult>,
  ): Promise<void> {
    if (!this.session) {
      this.send({ type: "error", message: "Copilot session not initialized" });
      return;
    }

    this.callTool = callTool;

    try {
      await this.session.sendAndWait({ prompt: text });
      this.send({ type: "turn_complete", stop_reason: "end_turn" });
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      this.send({ type: "error", message });
      this.send({ type: "turn_complete", stop_reason: `Error: ${message}` });
    }
  }

  cancel(): void {
    this.session?.abort().catch(() => {});

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

  private setupEventListeners(): void {
    // Stream text chunks
    const unsub1 = this.session.on("assistant.message_delta", (event: any) => {
      this.send({ type: "message_chunk", text: event.data.deltaContent });
    });
    this.unsubscribers.push(unsub1);

    // Stream reasoning/thinking chunks
    const unsub2 = this.session.on("assistant.reasoning_delta", (event: any) => {
      this.send({ type: "thought_chunk", text: event.data.deltaContent });
    });
    this.unsubscribers.push(unsub2);
  }

  private createMcpTool(t: McpTool) {
    return defineTool(t.name, {
      description: t.description,
      parameters: t.inputSchema as any,
      handler: async (args: Record<string, unknown>) => {
        const toolId = this.nextId();

        // Emit tool call event for UI
        this.send({
          type: "tool_call",
          id: toolId,
          title: t.name,
          kind: "mcp",
          content: JSON.stringify(args),
        });

        // Permission check for write tools
        if (!BUDDIO_READ_TOOLS.has(t.name)) {
          this.send({
            type: "permission_request",
            request_id: toolId,
            session_id: "sidecar",
            tool_name: t.name,
            options: [
              { option_id: "allow", name: "Allow", kind: "Allow" },
              { option_id: "deny", name: "Deny", kind: "Deny" },
            ],
          });

          const optionId = await new Promise<string>((resolve) => {
            this.pendingPermissions.set(toolId, resolve);
          });

          if (optionId === "__cancelled__" || optionId === "deny") {
            this.send({
              type: "tool_call_update",
              id: toolId,
              title: null,
              status: optionId === "__cancelled__" ? "Cancelled" : "Denied",
            });
            return "Permission denied by user";
          }

          this.send({
            type: "tool_call_update",
            id: toolId,
            title: null,
            status: "Approved",
          });
        }

        // Execute the tool via MCP
        if (!this.callTool) return "Tool system not ready";

        const result = await this.callTool(t.name, args);

        this.send({
          type: "tool_call_update",
          id: toolId,
          title: null,
          status: result.isError ? "Error" : "Complete",
        });

        if (result.isError) {
          throw new Error(result.content);
        }
        return result.content;
      },
    });
  }

  private createAskUserQuestionTool() {
    return defineTool("AskUserQuestion", {
      description:
        "Ask the user one or more clarifying questions when you need more information to proceed. " +
        "Each question should have 2-4 predefined options for the user to choose from. " +
        "Use this when the task has multiple valid approaches and you need user input to decide.",
      parameters: {
        type: "object",
        properties: {
          questions: {
            type: "array",
            items: {
              type: "object",
              properties: {
                question: { type: "string", description: "The question text to display" },
                header: { type: "string", description: "Short label (max 12 chars)" },
                options: {
                  type: "array",
                  items: {
                    type: "object",
                    properties: {
                      label: { type: "string", description: "Option label" },
                      description: { type: "string", description: "Option description" },
                    },
                    required: ["label", "description"],
                  },
                  minItems: 2,
                  maxItems: 4,
                },
                multiSelect: { type: "boolean", description: "Allow multiple selections" },
              },
              required: ["question", "header", "options", "multiSelect"],
            },
            minItems: 1,
            maxItems: 4,
          },
        },
        required: ["questions"],
      } as any,
      handler: async (args: {
        questions: Array<{
          question: string;
          header: string;
          options: { label: string; description: string }[];
          multiSelect: boolean;
        }>;
      }) => {
        const requestId = this.nextId();

        this.send({
          type: "user_question",
          request_id: requestId,
          tool_use_id: requestId,
          questions: args.questions,
        });

        const answers = await new Promise<Record<string, string>>((resolve) => {
          this.pendingQuestions.set(requestId, resolve);
        });

        if ("__cancelled__" in answers) {
          return "User cancelled the question";
        }
        return JSON.stringify({ questions: args.questions, answers });
      },
    });
  }

  private createReplaceSelectionTool() {
    return defineTool("replace_selection", {
      description:
        "Replace the user's currently selected text with new text. " +
        "Use this when the user asks to rewrite, rephrase, translate, summarize, or transform their selected text. " +
        "Only use this tool when there is selected text in the context.",
      parameters: {
        type: "object",
        properties: {
          text: {
            type: "string",
            description: "The replacement text to insert in place of the current selection",
          },
        },
        required: ["text"],
      } as any,
      handler: async (args: { text: string }) => {
        const requestId = this.nextId();

        this.send({
          type: "tool_call",
          id: requestId,
          title: "replace_selection",
          kind: "custom",
          content: null,
        });

        this.send({
          type: "replace_selection_request",
          request_id: requestId,
          text: args.text,
        });

        const success = await new Promise<boolean>((resolve) => {
          this.pendingReplacements.set(requestId, resolve);
        });

        this.send({
          type: "tool_call_update",
          id: requestId,
          title: null,
          status: success ? "Complete" : "Error",
        });

        return success ? "Selection replaced successfully" : "Failed to replace selection";
      },
    });
  }
}
