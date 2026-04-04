// === Rust → Node (stdin) messages ===

export interface InitMessage {
  type: "init";
  provider: "claude" | "copilot" | "codex" | "ollama";
  config: {
    api_key: string;
    model: string;
    auth_method: "oauth" | "api_key";
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

export interface ResolveQuestionMessage {
  type: "resolve_question";
  request_id: string;
  answers: Record<string, string>;
}

export interface ShutdownMessage {
  type: "shutdown";
}

export interface ResolveReplaceSelectionMessage {
  type: "resolve_replace_selection";
  request_id: string;
  success: boolean;
}

export type IncomingMessage =
  | InitMessage
  | PromptMessage
  | CancelMessage
  | ResolvePermissionMessage
  | ResolveQuestionMessage
  | ShutdownMessage
  | ResolveReplaceSelectionMessage;

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

export interface UserQuestionOut {
  type: "user_question";
  request_id: string;
  tool_use_id: string;
  questions: UserQuestionItem[];
}

export interface UserQuestionItem {
  question: string;
  header: string;
  options: { label: string; description: string }[];
  multiSelect: boolean;
}

export interface ErrorOut {
  type: "error";
  message: string;
}

export interface ReplaceSelectionRequestOut {
  type: "replace_selection_request";
  request_id: string;
  text: string;
}

export interface AuthStatusOut {
  type: "auth_status";
  is_authenticating: boolean;
  auth_url: string | null;
  error: string | null;
}

export type OutgoingMessage =
  | StatusChangeOut
  | MessageChunkOut
  | ThoughtChunkOut
  | ToolCallOut
  | ToolCallUpdateOut
  | PermissionRequestOut
  | UserQuestionOut
  | TurnCompleteOut
  | ErrorOut
  | ReplaceSelectionRequestOut
  | AuthStatusOut;
