// === Rust → Node (stdin) messages ===

export interface InitMessage {
  type: "init";
  provider: "claude" | "copilot";
  config: {
    api_key: string;
    model: string;
  };
  mcp_binary: string;
}

export interface PromptMessage {
  type: "prompt";
  text: string;
}

export interface CancelMessage {
  type: "cancel";
}

export interface ResolvePermissionMessage {
  type: "resolve_permission";
  request_id: string;
  option_id: string;
}

export interface ShutdownMessage {
  type: "shutdown";
}

export type IncomingMessage =
  | InitMessage
  | PromptMessage
  | CancelMessage
  | ResolvePermissionMessage
  | ShutdownMessage;

// === Node → Rust (stdout) messages ===

export interface StatusChangeOut {
  type: "status_change";
  status: "connected" | "disconnected" | "error";
}

export interface MessageChunkOut {
  type: "message_chunk";
  text: string;
}

export interface ThoughtChunkOut {
  type: "thought_chunk";
  text: string;
}

export interface ToolCallOut {
  type: "tool_call";
  id: string;
  title: string;
  kind: string;
  content: string | null;
}

export interface ToolCallUpdateOut {
  type: "tool_call_update";
  id: string;
  title: string | null;
  status: string | null;
}

export interface PermissionRequestOut {
  type: "permission_request";
  request_id: string;
  session_id: string;
  tool_name: string;
  options: { option_id: string; name: string; kind: string }[];
}

export interface TurnCompleteOut {
  type: "turn_complete";
  stop_reason: string;
}

export interface ErrorOut {
  type: "error";
  message: string;
}

export type OutgoingMessage =
  | StatusChangeOut
  | MessageChunkOut
  | ThoughtChunkOut
  | ToolCallOut
  | ToolCallUpdateOut
  | PermissionRequestOut
  | TurnCompleteOut
  | ErrorOut;
