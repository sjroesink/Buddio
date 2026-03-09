import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import type { McpTool, McpToolResult } from "./providers/base.js";

export class McpConnection {
  private client: Client;
  private transport: StdioClientTransport | null = null;
  private tools: McpTool[] = [];

  constructor() {
    this.client = new Client(
      { name: "buddio-sidecar", version: "0.1.0" },
      { capabilities: {} },
    );
  }

  async connect(mcpBinaryPath: string): Promise<McpTool[]> {
    this.transport = new StdioClientTransport({
      command: mcpBinaryPath,
      args: [],
    });

    await this.client.connect(this.transport);

    // List available tools
    const result = await this.client.listTools();
    this.tools = result.tools.map((t) => ({
      name: t.name,
      description: t.description ?? "",
      inputSchema: t.inputSchema as Record<string, unknown>,
    }));

    return this.tools;
  }

  getTools(): McpTool[] {
    return this.tools;
  }

  async callTool(
    name: string,
    args: Record<string, unknown>,
  ): Promise<McpToolResult> {
    try {
      const result = await this.client.callTool({ name, arguments: args });
      // MCP tool results have content array
      const contentParts = result.content as
        | { type: string; text?: string }[]
        | undefined;
      const text = contentParts
        ?.filter((c) => c.type === "text")
        .map((c) => c.text ?? "")
        .join("\n") ?? JSON.stringify(result);

      return { content: text, isError: result.isError === true };
    } catch (err) {
      return {
        content: `MCP tool error: ${err instanceof Error ? err.message : String(err)}`,
        isError: true,
      };
    }
  }

  async disconnect(): Promise<void> {
    try {
      await this.client.close();
    } catch {
      // Ignore close errors
    }
  }
}
