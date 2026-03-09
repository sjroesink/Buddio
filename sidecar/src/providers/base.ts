import type { OutgoingMessage } from "../protocol.js";

export interface McpTool {
  name: string;
  description: string;
  inputSchema: Record<string, unknown>;
}

export interface McpToolResult {
  content: string;
  isError?: boolean;
}

export interface ProviderConfig {
  apiKey: string;
  model: string;
}

export type SendFn = (msg: OutgoingMessage) => void;

export interface SidecarProvider {
  init(config: ProviderConfig, tools: McpTool[], send: SendFn): Promise<void>;
  prompt(text: string, callTool: (name: string, args: Record<string, unknown>) => Promise<McpToolResult>): Promise<void>;
  cancel(): void;
}
