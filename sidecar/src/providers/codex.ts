import { Codex } from "@openai/codex-sdk";
import type { Thread } from "@openai/codex-sdk";
import type {
  SidecarProvider,
  ProviderConfig,
  McpTool,
  McpToolResult,
  SendFn,
} from "./base.js";

export class CodexProvider implements SidecarProvider {
  private codex: Codex | null = null;
  private thread: Thread | null = null;
  private send: SendFn = () => {};
  private abortController: AbortController | null = null;

  async init(
    config: ProviderConfig,
    _mcpTools: McpTool[],
    send: SendFn,
  ): Promise<void> {
    this.send = send;

    try {
      this.codex = new Codex({
        apiKey: config.apiKey || undefined,
      });
      this.thread = this.codex.startThread({
        model: config.model || undefined,
        approvalPolicy: "never",
      });
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      send({
        type: "error",
        message: `Failed to start Codex client: ${message}. Make sure @openai/codex is installed with optional dependencies.`,
      });
      throw err;
    }
  }

  async prompt(
    text: string,
    _callTool: (
      name: string,
      args: Record<string, unknown>,
    ) => Promise<McpToolResult>,
  ): Promise<void> {
    if (!this.thread) {
      this.send({ type: "error", message: "Codex thread not initialized" });
      return;
    }

    this.abortController = new AbortController();

    try {
      const { events } = await this.thread.runStreamed(text, {
        signal: this.abortController.signal,
      });

      // Track the latest text for agent_message items to compute deltas
      const messageTexts = new Map<string, string>();

      for await (const event of events) {
        if (this.abortController?.signal.aborted) break;

        switch (event.type) {
          case "item.started":
          case "item.updated": {
            const item = event.item;

            if (item.type === "agent_message") {
              const prev = messageTexts.get(item.id) ?? "";
              const delta = item.text.slice(prev.length);
              if (delta) {
                this.send({ type: "message_chunk", text: delta });
              }
              messageTexts.set(item.id, item.text);
            }

            if (item.type === "reasoning") {
              const prev = messageTexts.get(item.id) ?? "";
              const delta = item.text.slice(prev.length);
              if (delta) {
                this.send({ type: "thought_chunk", text: delta });
              }
              messageTexts.set(item.id, item.text);
            }

            if (item.type === "command_execution") {
              if (event.type === "item.started") {
                this.send({
                  type: "tool_call",
                  id: item.id,
                  title: "Shell",
                  kind: "command",
                  content: item.command,
                });
              } else {
                this.send({
                  type: "tool_call_update",
                  id: item.id,
                  title: null,
                  status: item.status === "completed"
                    ? "Complete"
                    : item.status === "failed"
                      ? "Error"
                      : null,
                });
              }
            }

            if (item.type === "file_change") {
              if (event.type === "item.started") {
                const summary = item.changes
                  .map((c) => `${c.kind} ${c.path}`)
                  .join(", ");
                this.send({
                  type: "tool_call",
                  id: item.id,
                  title: "File Changes",
                  kind: "file_change",
                  content: summary,
                });
              }
            }

            if (item.type === "mcp_tool_call") {
              if (event.type === "item.started") {
                this.send({
                  type: "tool_call",
                  id: item.id,
                  title: `${item.server}/${item.tool}`,
                  kind: "mcp",
                  content: JSON.stringify(item.arguments ?? {}),
                });
              } else {
                this.send({
                  type: "tool_call_update",
                  id: item.id,
                  title: null,
                  status: item.status === "completed"
                    ? "Complete"
                    : item.status === "failed"
                      ? "Error"
                      : null,
                });
              }
            }

            if (item.type === "web_search" && event.type === "item.started") {
              this.send({
                type: "tool_call",
                id: item.id,
                title: "Web Search",
                kind: "web_search",
                content: item.query,
              });
            }

            break;
          }

          case "item.completed": {
            const item = event.item;

            if (item.type === "agent_message") {
              const prev = messageTexts.get(item.id) ?? "";
              const delta = item.text.slice(prev.length);
              if (delta) {
                this.send({ type: "message_chunk", text: delta });
              }
              messageTexts.delete(item.id);
            }

            if (item.type === "reasoning") {
              const prev = messageTexts.get(item.id) ?? "";
              const delta = item.text.slice(prev.length);
              if (delta) {
                this.send({ type: "thought_chunk", text: delta });
              }
              messageTexts.delete(item.id);
            }

            if (
              item.type === "command_execution" ||
              item.type === "mcp_tool_call"
            ) {
              this.send({
                type: "tool_call_update",
                id: item.id,
                title: null,
                status: item.status === "completed" ? "Complete" : "Error",
              });
            }

            if (item.type === "file_change") {
              this.send({
                type: "tool_call_update",
                id: item.id,
                title: null,
                status: item.status === "completed" ? "Complete" : "Error",
              });
            }

            if (item.type === "web_search") {
              this.send({
                type: "tool_call_update",
                id: item.id,
                title: null,
                status: "Complete",
              });
            }

            if (item.type === "error") {
              this.send({ type: "error", message: item.message });
            }

            break;
          }

          case "turn.completed": {
            // turn.completed is handled after the loop
            break;
          }

          case "turn.failed": {
            this.send({ type: "error", message: event.error.message });
            break;
          }

          case "error": {
            this.send({ type: "error", message: event.message });
            break;
          }
        }
      }

      if (this.abortController?.signal.aborted) {
        this.send({ type: "turn_complete", stop_reason: "Cancelled" });
      } else {
        this.send({ type: "turn_complete", stop_reason: "end_turn" });
      }
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
  }
}
