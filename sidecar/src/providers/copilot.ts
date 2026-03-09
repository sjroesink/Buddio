import type {
  SidecarProvider,
  ProviderConfig,
  McpTool,
  McpToolResult,
  SendFn,
} from "./base.js";

export class CopilotProvider implements SidecarProvider {
  private send: SendFn = () => {};

  async init(
    _config: ProviderConfig,
    _tools: McpTool[],
    send: SendFn,
  ): Promise<void> {
    this.send = send;
  }

  async prompt(
    _text: string,
    _callTool: (
      name: string,
      args: Record<string, unknown>,
    ) => Promise<McpToolResult>,
  ): Promise<void> {
    this.send({
      type: "error",
      message: "Copilot provider is not yet implemented. Coming soon!",
    });
    this.send({ type: "turn_complete", stop_reason: "error" });
  }

  cancel(): void {
    // No-op
  }
}
