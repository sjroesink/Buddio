export interface LaunchItem {
  id: string;
  title: string;
  subtitle: string | null;
  icon: string | null;
  action_type: string;
  action_value: string;
  category: string;
  tags: string;
  frequency: number;
  enabled: boolean;
  created_at: string;
  updated_at: string;
}

// ACP Agent types

export type AgentStatus = "disconnected" | "connecting" | "connected" | "error";

export type AgentUpdate =
  | { type: "message_chunk"; text: string }
  | { type: "thought_chunk"; text: string }
  | { type: "tool_call"; id: string; title: string; kind: string }
  | {
      type: "tool_call_update";
      id: string;
      title: string | null;
      status: string | null;
    }
  | { type: "plan"; entries: PlanEntry[] }
  | { type: "turn_complete"; stop_reason: string }
  | { type: "status_change"; status: AgentStatus };

export interface AgentThreadMessage {
  id: string;
  role: "user" | "assistant" | "tool";
  content: string;
  // Tool call fields (only when role === "tool")
  toolTitle?: string;
  toolStatus?: "pending" | "approved" | "running" | "completed" | "error";
  commandPreview?: string;
}

export interface PlanEntry {
  content: string;
  priority: string;
  status: string;
}

export interface PermissionRequest {
  request_id: string;
  session_id: string;
  tool_name: string;
  tool_description: string | null;
  command_preview: string | null;
  options: PermissionOption[];
}

export interface PermissionOption {
  option_id: string;
  name: string;
  kind: string;
}

export interface RequiredEnvVar {
  name: string;
  description: string;
  is_secret: boolean;
}

export interface RegistryAgent {
  id: string;
  name: string;
  version: string;
  description: string;
  icon: string | null;
  distribution_type: string;
  distribution_detail: string;
  distribution_args: string[];
  archive_url: string;
  required_env: RequiredEnvVar[];
}

export interface AgentConfig {
  source: string;
  agent_id: string;
  binary_path: string;
  args: string;
  env: string;
  auto_fallback: boolean;
}

// --- Launch context types ---

export interface LaunchContext {
  clipboard_text: string | null;
  selected_text: string | null;
  source_window_title: string | null;
  source_process_name: string | null;
}

export interface CommandSuggestion {
  suggested_command: string;
  reason: string;
  confidence: number;
  source_item_id: string | null;
}

export interface Memory {
  id: string;
  key: string;
  value: string;
  context: string | null;
  memory_type: string;
  confidence: number;
  created_at: string;
  updated_at: string;
  last_accessed: string;
}

// --- Slash Command types ---

export interface SlashCommand {
  id: string;
  name: string;
  description: string;
  script_path: string;
  usage_count: number;
  created_at: string;
  updated_at: string;
}

// --- Conversation types ---

export interface Conversation {
  id: string;
  title: string;
  created_at: string;
  updated_at: string;
}

export interface ConversationWithPreview {
  id: string;
  title: string;
  created_at: string;
  updated_at: string;
  message_count: number;
  last_message_preview: string | null;
}

export interface ConversationMessage {
  id: string;
  conversation_id: string;
  role: string;
  content: string;
  created_at: string;
}

// --- Session Config Option types ---

export interface SessionConfigSelectOption {
  value: string;
  name: string;
  description: string | null;
}

export interface SessionConfigSelectGroup {
  group: string;
  name: string;
  options: SessionConfigSelectOption[];
}

export type SessionConfigSelectOptions =
  | { type: "ungrouped"; options: SessionConfigSelectOption[] }
  | { type: "grouped"; groups: SessionConfigSelectGroup[] };

export interface SessionConfigOptionInfo {
  id: string;
  name: string;
  description: string | null;
  category: string | null;
  current_value: string;
  select_options: SessionConfigSelectOptions;
}
