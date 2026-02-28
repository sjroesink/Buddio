import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, waitFor, screen, act } from "@testing-library/react";
import "./helpers/mockTauri";
import { resetInvokeHandler } from "./helpers/mockTauri";
import { invoke } from "@tauri-apps/api/core";

const mockPrompt = vi.fn();
let capturedOnAgentPrompt: ((query: string) => void) | null = null;
let mockedLauncherAgentMode = true;

vi.mock("../../src/hooks/useAcpAgent", () => ({
  useAcpAgent: () => ({
    status: "connected",
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
    prompt: mockPrompt,
    promptSlashCommand: vi.fn(),
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
    },
    clearContext: vi.fn(),
    clearSelection: vi.fn(),
    clearClipboard: vi.fn(),
    typeText: vi.fn(),
    replaceSelection: vi.fn(),
    recordRewrite: vi.fn(),
    rewriteSuggestions: [],
    hasSelection: false,
    hasClipboard: false,
  }),
}));

vi.mock("../../src/hooks/useLauncher", () => ({
  useLauncher: (options: { onAgentPrompt: (query: string) => void }) => {
    capturedOnAgentPrompt = options.onAgentPrompt;
    return {
      query: "",
      setQuery: vi.fn(),
      items: [],
      selectedIndex: 0,
      setSelectedIndex: vi.fn(),
      categories: [],
      activeCategory: null,
      setActiveCategory: vi.fn(),
      loading: false,
      handleKeyDown: vi.fn(),
      executeSelected: vi.fn(),
      agentMode: mockedLauncherAgentMode,
      suggestions: [],
      selectedSuggestionIndex: -1,
      selectSuggestion: vi.fn(),
      focusInputSignal: 0,
      handleInputFocus: vi.fn(),
      savingCommand: false,
      saveCommandFromSuggestion: vi.fn(),
      refresh: vi.fn(),
      reset: vi.fn(),
      isSlashMode: false,
      slashCommands: [],
      selectedSlashIndex: 0,
      setSelectedSlashIndex: vi.fn(),
      executeSlashCommand: vi.fn(),
      paramEntryMode: false,
      currentParamIndex: 0,
      activeCommandParams: [],
      activeCommandName: "",
    };
  },
}));

import App from "../../src/App";

describe("App agent mode window sizing", () => {
  beforeEach(() => {
    resetInvokeHandler();
    vi.clearAllMocks();
    capturedOnAgentPrompt = null;
    mockedLauncherAgentMode = true;
  });

  it("disables compact window mode when agent mode is active", async () => {
    render(<App />);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "set_window_compact",
        expect.objectContaining({ compact: false, anchor: "bottom" }),
      );
    });
  });

  it("forces composer mode when fallback prompt is triggered", async () => {
    mockedLauncherAgentMode = false;

    render(<App />);

    expect(screen.queryByTestId("back-button")).not.toBeInTheDocument();

    act(() => {
      capturedOnAgentPrompt?.("fix this query");
    });

    await waitFor(() => {
      expect(screen.getByTestId("back-button")).toBeInTheDocument();
    });

    expect(mockPrompt).toHaveBeenCalledWith("fix this query");
  });
});
