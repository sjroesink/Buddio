import { vi } from "vitest";
import type { LaunchItem } from "../../../src/types";

// ── Default mock data ──────────────────────────────────────────────

export const MOCK_ITEMS: LaunchItem[] = [
  {
    id: "1",
    title: "Google",
    subtitle: "https://google.com",
    icon: null,
    action_type: "url",
    action_value: "https://google.com",
    category: "Web",
    tags: "search,browser",
    frequency: 10,
    enabled: true,
    created_at: "2024-01-01T00:00:00Z",
    updated_at: "2024-01-01T00:00:00Z",
  },
  {
    id: "2",
    title: "VS Code",
    subtitle: "Open Visual Studio Code",
    icon: null,
    action_type: "command",
    action_value: "code",
    category: "Dev",
    tags: "editor,ide",
    frequency: 8,
    enabled: true,
    created_at: "2024-01-01T00:00:00Z",
    updated_at: "2024-01-01T00:00:00Z",
  },
  {
    id: "3",
    title: "Terminal",
    subtitle: "Open terminal",
    icon: null,
    action_type: "command",
    action_value: "wt",
    category: "Dev",
    tags: "shell,terminal",
    frequency: 5,
    enabled: true,
    created_at: "2024-01-01T00:00:00Z",
    updated_at: "2024-01-01T00:00:00Z",
  },
];

export const MOCK_CATEGORIES = ["Web", "Dev"];

// ── Invoke mock ────────────────────────────────────────────────────

type InvokeHandler = (cmd: string, args?: Record<string, unknown>) => unknown;

let invokeHandler: InvokeHandler = defaultInvokeHandler;

function defaultInvokeHandler(
  cmd: string,
  args?: Record<string, unknown>,
): unknown {
  switch (cmd) {
    case "search_items": {
      const query = ((args?.query as string) ?? "").toLowerCase();
      if (!query) return MOCK_ITEMS;
      return MOCK_ITEMS.filter(
        (item) =>
          item.title.toLowerCase().includes(query) ||
          item.tags.toLowerCase().includes(query),
      );
    }
    case "get_categories":
      return MOCK_CATEGORIES;
    case "get_command_suggestions":
      return [];
    case "execute_item":
      return undefined;
    case "hide_window":
      return undefined;
    case "set_window_compact":
      return undefined;
    case "get_setting":
      return null;
    case "set_setting":
      return undefined;
    case "get_agent_config":
      return {
        source: "",
        agent_id: "",
        binary_path: "",
        args: "",
        env: "",
        auto_fallback: false,
      };
    case "acp_get_status":
      return "disconnected";
    case "get_launch_context":
      return {
        clipboard_text: null,
        selected_text: null,
        source_window_title: null,
        source_process_name: null,
      };
    case "list_conversations":
      return [];
    case "get_rewrite_suggestions":
      return [];
    default:
      return undefined;
  }
}

export function setInvokeHandler(handler: InvokeHandler) {
  invokeHandler = handler;
}

export function resetInvokeHandler() {
  invokeHandler = defaultInvokeHandler;
}

// ── Event listener mock ────────────────────────────────────────────

type EventCallback = (event: { payload: unknown }) => void;
const eventListeners = new Map<string, Set<EventCallback>>();

export function emitMockEvent(event: string, payload: unknown) {
  const callbacks = eventListeners.get(event);
  if (callbacks) {
    callbacks.forEach((cb) => cb({ payload }));
  }
}

export function clearMockListeners() {
  eventListeners.clear();
}

// ── Module mocks ───────────────────────────────────────────────────

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string, args?: Record<string, unknown>) =>
    invokeHandler(cmd, args),
  ),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (event: string, callback: EventCallback) => {
    if (!eventListeners.has(event)) {
      eventListeners.set(event, new Set());
    }
    eventListeners.get(event)!.add(callback);
    // Return unlisten function
    return () => {
      eventListeners.get(event)?.delete(callback);
    };
  }),
  emit: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-global-shortcut", () => ({
  register: vi.fn(),
  unregister: vi.fn(),
}));
