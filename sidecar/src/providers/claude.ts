import Anthropic from "@anthropic-ai/sdk";
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
  "slash_commands_search", "settings_get", "settings_list", "db_path",
]);

// AskUserQuestion tool definition — allows Claude to ask the user clarifying questions
const ASK_USER_QUESTION_TOOL: Anthropic.Tool = {
  name: "AskUserQuestion",
  description:
    "Ask the user one or more clarifying questions when you need more information to proceed. " +
    "Each question should have 2-4 predefined options for the user to choose from. " +
    "Use this when the task has multiple valid approaches and you need user input to decide.",
  input_schema: {
    type: "object" as const,
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
  },
};

export class ClaudeProvider implements SidecarProvider {
  private client: Anthropic | null = null;
  private model = "claude-sonnet-4-20250514";
  private tools: Anthropic.Tool[] = [];
  private send: SendFn = () => {};
  private abortController: AbortController | null = null;
  private pendingPermissions = new Map<string, (optionId: string) => void>();
  private pendingQuestions = new Map<string, (answers: Record<string, string>) => void>();

  async init(
    config: ProviderConfig,
    mcpTools: McpTool[],
    send: SendFn,
  ): Promise<void> {
    this.send = send;
    this.model = config.model || "claude-sonnet-4-20250514";
    this.client = new Anthropic({ apiKey: config.apiKey });

    // Convert MCP tools to Anthropic tool format + add AskUserQuestion
    this.tools = [
      ...mcpTools.map((t) => ({
        name: t.name,
        description: t.description,
        input_schema: t.inputSchema as Anthropic.Tool.InputSchema,
      })),
      ASK_USER_QUESTION_TOOL,
    ];
  }

  async prompt(
    text: string,
    callTool: (
      name: string,
      args: Record<string, unknown>,
    ) => Promise<McpToolResult>,
  ): Promise<void> {
    if (!this.client) {
      this.send({ type: "error", message: "Claude client not initialized" });
      return;
    }

    this.abortController = new AbortController();

    const messages: Anthropic.MessageParam[] = [
      { role: "user", content: text },
    ];

    try {
      await this.agentLoop(messages, callTool);
      this.send({ type: "turn_complete", stop_reason: "end_turn" });
    } catch (err) {
      if (this.abortController?.signal.aborted) {
        this.send({ type: "turn_complete", stop_reason: "Cancelled" });
      } else {
        const message =
          err instanceof Error ? err.message : String(err);
        this.send({ type: "error", message });
        this.send({
          type: "turn_complete",
          stop_reason: `Error: ${message}`,
        });
      }
    } finally {
      this.abortController = null;
    }
  }

  cancel(): void {
    this.abortController?.abort();
    // Cancel pending permissions
    for (const [, resolver] of this.pendingPermissions) {
      resolver("__cancelled__");
    }
    this.pendingPermissions.clear();
    // Cancel pending questions
    for (const [, resolver] of this.pendingQuestions) {
      resolver({ __cancelled__: "true" });
    }
    this.pendingQuestions.clear();
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

  private async agentLoop(
    messages: Anthropic.MessageParam[],
    callTool: (
      name: string,
      args: Record<string, unknown>,
    ) => Promise<McpToolResult>,
  ): Promise<void> {
    const MAX_ITERATIONS = 25;

    for (let i = 0; i < MAX_ITERATIONS; i++) {
      if (this.abortController?.signal.aborted) return;

      const stream = this.client!.messages.stream(
        {
          model: this.model,
          max_tokens: 8192,
          messages,
          tools: this.tools.length > 0 ? this.tools : undefined,
        },
        { signal: this.abortController?.signal },
      );

      // Process streaming events
      stream.on("text", (text) => {
        this.send({ type: "message_chunk", text });
      });

      const response = await stream.finalMessage();

      // Add assistant response to messages for multi-turn
      messages.push({ role: "assistant", content: response.content });

      // Check for tool use
      const toolUseBlocks = response.content.filter(
        (block): block is Anthropic.ContentBlock & { type: "tool_use" } =>
          block.type === "tool_use",
      );

      if (toolUseBlocks.length === 0 || response.stop_reason !== "tool_use") {
        // No tool calls — done
        return;
      }

      // Process tool calls
      const toolResults: Anthropic.ToolResultBlockParam[] = [];

      for (const toolUse of toolUseBlocks) {
        if (this.abortController?.signal.aborted) return;

        const toolId = toolUse.id;
        const toolName = toolUse.name;
        const toolInput = toolUse.input as Record<string, unknown>;

        // Handle AskUserQuestion specially — don't go through MCP
        if (toolName === "AskUserQuestion") {
          const questions = (toolInput.questions ?? []) as Array<{
            question: string;
            header: string;
            options: { label: string; description: string }[];
            multiSelect: boolean;
          }>;

          const requestId = toolId;
          this.send({
            type: "user_question",
            request_id: requestId,
            tool_use_id: toolId,
            questions,
          });

          // Wait for user to answer
          const answers = await new Promise<Record<string, string>>((resolve) => {
            this.pendingQuestions.set(requestId, resolve);
          });

          if ("__cancelled__" in answers) {
            toolResults.push({
              type: "tool_result",
              tool_use_id: toolId,
              content: "User cancelled the question",
              is_error: true,
            });
            continue;
          }

          // Return answers to Claude in the expected format
          toolResults.push({
            type: "tool_result",
            tool_use_id: toolId,
            content: JSON.stringify({ questions, answers }),
          });
          continue;
        }

        // Emit tool call event
        this.send({
          type: "tool_call",
          id: toolId,
          title: toolName,
          kind: "mcp",
          content: JSON.stringify(toolInput),
        });

        // Check if this is a read-only tool (auto-allow)
        const isReadOnly = BUDDIO_READ_TOOLS.has(toolName);

        if (!isReadOnly) {
          // Request permission for write tools
          const requestId = toolId;
          this.send({
            type: "permission_request",
            request_id: requestId,
            session_id: "sidecar",
            tool_name: toolName,
            options: [
              { option_id: "allow", name: "Allow", kind: "Allow" },
              { option_id: "deny", name: "Deny", kind: "Deny" },
            ],
          });

          // Wait for permission resolution
          const optionId = await new Promise<string>((resolve) => {
            this.pendingPermissions.set(requestId, resolve);
          });

          if (
            optionId === "__cancelled__" ||
            optionId === "deny"
          ) {
            this.send({
              type: "tool_call_update",
              id: toolId,
              title: null,
              status: optionId === "__cancelled__" ? "Cancelled" : "Denied",
            });
            toolResults.push({
              type: "tool_result",
              tool_use_id: toolId,
              content: "Permission denied by user",
              is_error: true,
            });
            continue;
          }

          this.send({
            type: "tool_call_update",
            id: toolId,
            title: null,
            status: "Approved",
          });
        }

        // Execute the tool
        const result = await callTool(toolName, toolInput);

        this.send({
          type: "tool_call_update",
          id: toolId,
          title: null,
          status: result.isError ? "Error" : "Complete",
        });

        toolResults.push({
          type: "tool_result",
          tool_use_id: toolId,
          content: result.content,
          is_error: result.isError,
        });
      }

      // Add tool results to messages for next iteration
      messages.push({ role: "user", content: toolResults });
    }

    // Max iterations reached
    this.send({
      type: "error",
      message: "Max tool iterations reached",
    });
  }
}
