import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import "./helpers/mockTauri";
import { resetInvokeHandler } from "./helpers/mockTauri";

// Mock the hooks that do heavy Tauri IPC
vi.mock("../../src/hooks/useAcpAgent", () => ({
  useAcpAgent: () => ({
    status: "disconnected",
    messages: "",
    thread: [],
    thoughts: "",
    isThinking: false,
    turnActive: false,
    permissionRequest: null,
    activeConversationId: null,
    conversations: [],
    configOptions: [],
    connect: vi.fn(),
    disconnect: vi.fn(),
    prompt: vi.fn(),
    cancel: vi.fn(),
    clearThread: vi.fn(),
    resolvePermission: vi.fn(),
    loadConversations: vi.fn(),
    loadConversation: vi.fn(),
    newConversation: vi.fn(),
    deleteConversation: vi.fn(),
    searchConversations: vi.fn(),
    setConfigOption: vi.fn(),
  }),
}));

vi.mock("../../src/hooks/useLaunchContext", () => ({
  useLaunchContext: () => ({
    context: {
      clipboard_text: null,
      selected_text: null,
      source_window_title: null,
      source_process_name: null,
      source_process_path: null,
    },
    clearContext: vi.fn(),
    typeText: vi.fn(),
    replaceSelection: vi.fn(),
    recordRewrite: vi.fn(),
    rewriteSuggestions: [],
    hasSelection: false,
    hasClipboard: false,
  }),
}));

import App from "../../src/App";

describe("App", () => {
  beforeEach(() => {
    resetInvokeHandler();
  });

  it("renders the app root container", async () => {
    render(<App />);
    expect(screen.getByTestId("app-root")).toBeInTheDocument();
  });

  it("renders the search bar in search mode", async () => {
    render(<App />);
    expect(screen.getByTestId("search-bar")).toBeInTheDocument();
    expect(screen.getByTestId("search-input")).toBeInTheDocument();
  });

  it("renders items after loading", async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("item-list")).toBeInTheDocument();
    });

    expect(screen.getByText("Google")).toBeInTheDocument();
    expect(screen.getByText("VS Code")).toBeInTheDocument();
    expect(screen.getByText("Terminal")).toBeInTheDocument();
  });

  it("renders category bar when items exist", async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("category-bar")).toBeInTheDocument();
    });

    expect(screen.getByTestId("category-all")).toBeInTheDocument();
  });

  it("renders status bar with item count", async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("status-bar")).toBeInTheDocument();
    });

    expect(screen.getByTestId("status-text")).toHaveTextContent("3 items");
  });

  it("filters items when typing in search", async () => {
    const user = userEvent.setup();
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("item-list")).toBeInTheDocument();
    });

    const input = screen.getByTestId("search-input");
    await user.type(input, "google");

    await waitFor(() => {
      expect(screen.getByText("Google")).toBeInTheDocument();
      expect(screen.queryByText("VS Code")).not.toBeInTheDocument();
    });
  });
});
