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

interface OllamaMessage {
  role: "system" | "user" | "assistant" | "tool";
  content: string;
  tool_calls?: OllamaToolCall[];
  tool_call_id?: string;
}

interface OllamaToolCall {
  id: string;
  type: "function";
  function: {
    name: string;
    arguments: Record<string, unknown>;
  };
}

interface OllamaStreamChunk {
  model: string;
  message?: {
    role: string;
    content: string;
    tool_calls?: OllamaToolCall[];
  };
  done: boolean;
  done_reason?: string;
}

export class OllamaProvider implements SidecarProvider {
  private send: SendFn = () => {};
  private callTool: ((name: string, args: Record<string, unknown>) => Promise<McpToolResult>) | null = null;
  private mcpTools: McpTool[] = [];
  private messages: OllamaMessage[] = [];
  private model = "";
  private baseUrl = "http://localhost:11434";
  private abortController: AbortController | null = null;
  private pendingPermissions = new Map<string, (optionId: string) => void>();
  private pendingQuestions = new Map<string, (answers: Record<string, string>) => void>();
  private pendingReplacements = new Map<string, (success: boolean) => void>();
  private idCounter = 0;

  private nextId(): string {
    return `ollama-${++this.idCounter}-${Date.now()}`;
  }

  async init(
    config: ProviderConfig,
    mcpTools: McpTool[],
    send: SendFn,
  ): Promise<void> {
    this.send = send;
    this.mcpTools = mcpTools;
    this.model = config.model || "llama3.2";

    // Support custom Ollama URL via apiKey field (reused as base URL)
    if (config.apiKey && config.apiKey.startsWith("http")) {
      this.baseUrl = config.apiKey.replace(/\/$/, "");
    }

    // Verify Ollama is reachable
    try {
      const resp = await fetch(`${this.baseUrl}/api/tags`);
      if (!resp.ok) {
        throw new Error(`HTTP ${resp.status}`);
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      send({
        type: "error",
        message: `Cannot connect to Ollama at ${this.baseUrl}: ${message}. Make sure Ollama is running (https://ollama.com).`,
      });
      throw err;
    }
  }

  async prompt(
    text: string,
    callTool: (
      name: string,
      args: Record<string, unknown>,
    ) => Promise<McpToolResult>,
  ): Promise<void> {
    this.callTool = callTool;
    this.abortController = new AbortController();

    // Add user message
    this.messages.push({ role: "user", content: text });

    try {
      // Agentic loop: keep calling until no more tool calls
      let maxIterations = 20;
      while (maxIterations-- > 0) {
        const toolCalls = await this.streamCompletion();

        if (this.abortController?.signal.aborted) {
          this.send({ type: "turn_complete", stop_reason: "Cancelled" });
          return;
        }

        if (!toolCalls || toolCalls.length === 0) {
          break;
        }

        // Process tool calls
        for (const tc of toolCalls) {
          const result = await this.executeTool(tc);

          // Add tool result to conversation
          this.messages.push({
            role: "tool",
            content: result,
            tool_call_id: tc.id,
          });
        }
      }

      this.send({ type: "turn_complete", stop_reason: "end_turn" });
    } catch (err) {
      if (this.abortController?.signal.aborted) {
        this.send({ type: "turn_complete", stop_reason: "Cancelled" });
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

  private buildTools(): object[] | undefined {
    const tools: object[] = [];

    // MCP tools
    for (const t of this.mcpTools) {
      tools.push({
        type: "function",
        function: {
          name: t.name,
          description: t.description,
          parameters: t.inputSchema,
        },
      });
    }

    // AskUserQuestion tool
    tools.push({
      type: "function",
      function: {
        name: "AskUserQuestion",
        description:
          "Ask the user one or more clarifying questions when you need more information to proceed. " +
          "Each question should have 2-4 predefined options for the user to choose from.",
        parameters: {
          type: "object",
          properties: {
            questions: {
              type: "array",
              items: {
                type: "object",
                properties: {
                  question: { type: "string" },
                  header: { type: "string" },
                  options: {
                    type: "array",
                    items: {
                      type: "object",
                      properties: {
                        label: { type: "string" },
                        description: { type: "string" },
                      },
                      required: ["label", "description"],
                    },
                  },
                  multiSelect: { type: "boolean" },
                },
                required: ["question", "header", "options", "multiSelect"],
              },
            },
          },
          required: ["questions"],
        },
      },
    });

    // replace_selection tool
    tools.push({
      type: "function",
      function: {
        name: "replace_selection",
        description:
          "Replace the user's currently selected text with new text. " +
          "Use when the user asks to rewrite, rephrase, translate, or transform their selected text.",
        parameters: {
          type: "object",
          properties: {
            text: { type: "string", description: "The replacement text" },
          },
          required: ["text"],
        },
      },
    });

    return tools.length > 0 ? tools : undefined;
  }

  private async streamCompletion(): Promise<OllamaToolCall[] | null> {
    const body: Record<string, unknown> = {
      model: this.model,
      messages: this.messages,
      stream: true,
    };

    const tools = this.buildTools();
    if (tools) {
      body.tools = tools;
    }

    const resp = await fetch(`${this.baseUrl}/api/chat`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
      signal: this.abortController?.signal,
    });

    if (!resp.ok) {
      const errText = await resp.text().catch(() => "");
      throw new Error(`Ollama API error ${resp.status}: ${errText}`);
    }

    const reader = resp.body?.getReader();
    if (!reader) throw new Error("No response body");

    const decoder = new TextDecoder();
    let buffer = "";
    let fullContent = "";
    let allToolCalls: OllamaToolCall[] = [];

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      buffer += decoder.decode(value, { stream: true });
      const lines = buffer.split("\n");
      buffer = lines.pop() || "";

      for (const line of lines) {
        if (!line.trim()) continue;

        let chunk: OllamaStreamChunk;
        try {
          chunk = JSON.parse(line);
        } catch {
          continue;
        }

        if (chunk.message?.content) {
          this.send({ type: "message_chunk", text: chunk.message.content });
          fullContent += chunk.message.content;
        }

        if (chunk.message?.tool_calls) {
          for (const tc of chunk.message.tool_calls) {
            // Ollama may not always provide an ID
            if (!tc.id) {
              tc.id = this.nextId();
            }
            allToolCalls.push(tc);
          }
        }
      }
    }

    // Add assistant message to conversation history
    const assistantMsg: OllamaMessage = { role: "assistant", content: fullContent };
    if (allToolCalls.length > 0) {
      assistantMsg.tool_calls = allToolCalls;
    }
    this.messages.push(assistantMsg);

    return allToolCalls.length > 0 ? allToolCalls : null;
  }

  private async executeTool(tc: OllamaToolCall): Promise<string> {
    const toolName = tc.function.name;
    const toolArgs = tc.function.arguments;
    const toolId = tc.id;

    // AskUserQuestion
    if (toolName === "AskUserQuestion") {
      this.send({
        type: "user_question",
        request_id: toolId,
        tool_use_id: toolId,
        questions: (toolArgs as any).questions,
      });

      const answers = await new Promise<Record<string, string>>((resolve) => {
        this.pendingQuestions.set(toolId, resolve);
      });

      if ("__cancelled__" in answers) {
        return "User cancelled the question";
      }
      return JSON.stringify({ questions: (toolArgs as any).questions, answers });
    }

    // replace_selection
    if (toolName === "replace_selection") {
      this.send({
        type: "tool_call",
        id: toolId,
        title: "replace_selection",
        kind: "custom",
        content: null,
      });

      this.send({
        type: "replace_selection_request",
        request_id: toolId,
        text: (toolArgs as any).text,
      });

      const success = await new Promise<boolean>((resolve) => {
        this.pendingReplacements.set(toolId, resolve);
      });

      this.send({
        type: "tool_call_update",
        id: toolId,
        title: null,
        status: success ? "Complete" : "Error",
      });

      return success ? "Selection replaced successfully" : "Failed to replace selection";
    }

    // MCP tools
    this.send({
      type: "tool_call",
      id: toolId,
      title: toolName,
      kind: "mcp",
      content: JSON.stringify(toolArgs),
    });

    // Permission check for write tools
    if (!BUDDIO_READ_TOOLS.has(toolName)) {
      this.send({
        type: "permission_request",
        request_id: toolId,
        session_id: "sidecar",
        tool_name: toolName,
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

    if (!this.callTool) return "Tool system not ready";

    const result = await this.callTool(toolName, toolArgs);

    this.send({
      type: "tool_call_update",
      id: toolId,
      title: null,
      status: result.isError ? "Error" : "Complete",
    });

    if (result.isError) {
      return `Error: ${result.content}`;
    }
    return result.content;
  }
}
