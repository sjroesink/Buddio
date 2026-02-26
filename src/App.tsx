import { useEffect, useRef, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useLauncher } from "./hooks/useLauncher";
import { useAcpAgent } from "./hooks/useAcpAgent";
import { useLaunchContext } from "./hooks/useLaunchContext";
import SearchBar, { type PastedImage } from "./components/SearchBar";
import CategoryBar from "./components/CategoryBar";
import ItemList from "./components/ItemList";
import StatusBar from "./components/StatusBar";
import { AgentResponse } from "./components/AgentResponse";
import { AgentSettings } from "./components/AgentSettings";
import CommandSuggestionPanel from "./components/CommandSuggestionPanel";
import SlashCommandList from "./components/SlashCommandList";
import ParameterHint from "./components/ParameterHint";
import ConversationHistory from "./components/ConversationHistory";
import { RewriteQuickActions } from "./components/RewriteQuickActions";
import { CommandOutput } from "./components/CommandOutput";
import { ContextPanel } from "./components/ContextPanel";
import { Toast, type ToastData } from "./components/Toast";
import type { AgentConfig } from "./types";

function App() {
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [autoFallback, setAutoFallback] = useState(false);
  const [showHistory, setShowHistory] = useState(false);
  const [forceAgentMode, setForceAgentMode] = useState(false);
  const [historySelectedIndex, setHistorySelectedIndex] = useState(0);
  const [rewriteSelectedIndex, setRewriteSelectedIndex] = useState(0);
  const [toast, setToast] = useState<ToastData | null>(null);
  const [commandOutput, setCommandOutput] = useState<string | null>(null);
  const [pastedImages, setPastedImages] = useState<PastedImage[]>([]);
  const [contextPanelOpen, setContextPanelOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  const agent = useAcpAgent();
  const launchCtx = useLaunchContext();

  const handleExecuteSuccess = useCallback(async (output?: string | null) => {
    if (output) {
      setCommandOutput(output);
    } else {
      setToast({ message: "Launched", type: "success" });
      await invoke("hide_window");
    }
  }, []);

  const handleExecuteError = useCallback((error: string) => {
    setToast({ message: error, type: "error" });
  }, []);

  const launcher = useLauncher({
    agentStatus: agent.status,
    agentAutoFallback: autoFallback,
    onAgentPrompt: agent.prompt,
    onSlashCommandCreate: agent.promptSlashCommand,
    onAgentCancel: agent.cancel,
    agentTurnActive: agent.turnActive,
    hasSelectedText: launchCtx.hasSelection,
    onExecuteSuccess: handleExecuteSuccess,
    onExecuteError: handleExecuteError,
  });

  // Load auto_fallback setting on mount
  useEffect(() => {
    invoke<string | null>("get_setting", { key: "acp.auto_fallback" })
      .then((val) => setAutoFallback(val === "true"))
      .catch(() => {});
  }, []);

  useEffect(() => {
    containerRef.current?.focus();
  }, []);

  // Ctrl+, for settings
  useEffect(() => {
    function handleGlobalKey(e: KeyboardEvent) {
      if (e.key === "," && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        setSettingsOpen((prev) => !prev);
      }
    }
    window.addEventListener("keydown", handleGlobalKey);
    return () => window.removeEventListener("keydown", handleGlobalKey);
  }, []);

  // Reset all state when the window is closed/hidden
  useEffect(() => {
    const unlisten = listen("launcher-reset", () => {
      launcher.reset();
      setSettingsOpen(false);
      setShowHistory(false);
      setForceAgentMode(false);
      setHistorySelectedIndex(0);
      setRewriteSelectedIndex(0);
      setToast(null);
      setCommandOutput(null);
      setPastedImages([]);
      setContextPanelOpen(false);
      if (agent.turnActive) {
        agent.cancel();
      }
      if (agent.thread.length > 0) {
        agent.clearThread();
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [launcher, agent]);

  // Detect clipboard images when the window gains focus
  useEffect(() => {
    const clipboardImageChecked = { current: false };
    const unlisten = listen("tauri://focus", async () => {
      if (clipboardImageChecked.current) return;
      clipboardImageChecked.current = true;
      try {
        const items = await navigator.clipboard.read();
        for (const item of items) {
          for (const type of item.types) {
            if (type.startsWith("image/")) {
              const blob = await item.getType(type);
              const reader = new FileReader();
              reader.onload = () => {
                const dataUrl = reader.result as string;
                setPastedImages((prev) => {
                  if (prev.some((img) => img.dataUrl === dataUrl)) return prev;
                  return [
                    ...prev,
                    {
                      id: crypto.randomUUID(),
                      dataUrl,
                      name: "Clipboard image",
                    },
                  ];
                });
              };
              reader.readAsDataURL(blob);
            }
          }
        }
      } catch {
        // Clipboard API not available or permission denied
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const handleAddImage = useCallback((image: PastedImage) => {
    setPastedImages((prev) => [...prev, image]);
  }, []);

  const handleRemoveImage = useCallback((id: string) => {
    setPastedImages((prev) => prev.filter((img) => img.id !== id));
  }, []);

  const toggleContextPanel = useCallback(() => {
    setContextPanelOpen((prev) => !prev);
  }, []);

  const hasSourceApp = !!launchCtx.context.source_window_title;
  const contextCount =
    (hasSourceApp ? 1 : 0) +
    (launchCtx.hasSelection ? 1 : 0) +
    (launchCtx.hasClipboard ? 1 : 0) +
    pastedImages.length;

  const handleConnect = useCallback(
    async (config: AgentConfig) => {
      setAutoFallback(config.auto_fallback);
      await agent.connect(config);
    },
    [agent],
  );

  const handleShowHistory = useCallback(() => {
    setShowHistory(true);
    setHistorySelectedIndex(0);
    agent.loadConversations();
  }, [agent]);

  const handleHideHistory = useCallback(() => {
    setShowHistory(false);
  }, []);

  const handleLoadConversation = useCallback(
    (conversationId: string) => {
      agent.loadConversation(conversationId);
      setShowHistory(false);
    },
    [agent],
  );

  const handleNewConversation = useCallback(() => {
    agent.newConversation();
    setShowHistory(false);
  }, [agent]);

  const enterAgentMode = useCallback(() => {
    setForceAgentMode(true);
  }, []);

  const exitAgentMode = useCallback(() => {
    if (agent.turnActive) {
      agent.cancel();
    }

    if (launcher.query.length > 0) {
      launcher.setQuery("");
    }

    if (agent.thread.length > 0) {
      agent.clearThread();
    }

    setShowHistory(false);
    setForceAgentMode(false);
    setContextPanelOpen(false);

    // Refresh search results — the agent may have added/removed items
    launcher.refresh();
  }, [agent, launcher]);

  const handleExecuteSlashCommand = useCallback(
    async (name: string) => {
      await invoke("execute_slash_command", { name, args: "" });
      setToast({ message: "Executed", type: "success" });
      await invoke("hide_window");
    },
    [],
  );

  const handleExecuteSlashCommandError = useCallback(
    (error: string) => {
      setToast({ message: error, type: "error" });
      // Feed the error back to the agent session
      agent.prompt(`The command failed with error: ${error}`);
    },
    [agent],
  );

  const handleRewriteQuickAction = useCallback(
    (prompt: string) => {
      agent.prompt(prompt);
      launcher.setQuery("");
    },
    [agent, launcher],
  );

  const showAgentThread = agent.turnActive || agent.thread.length > 0;
  const isAgentInputMode = launcher.agentMode || showAgentThread || showHistory || forceAgentMode;

  // Determine what to show in agent mode
  const showRewriteActions =
    isAgentInputMode &&
    !showAgentThread &&
    !showHistory &&
    launchCtx.hasSelection &&
    launchCtx.rewriteSuggestions.length > 0 &&
    !launcher.query.trim();

  const showConversationHistory =
    isAgentInputMode &&
    (!showAgentThread || showHistory) &&
    !showRewriteActions &&
    !contextPanelOpen &&
    (showHistory || agent.conversations.length > 0);

  const hasQuery = launcher.query.trim().length > 0;
  const showResults = hasQuery && launcher.items.length > 0;
  const showOnlySearch =
    (!hasQuery || launcher.items.length === 0) &&
    !launcher.isSlashMode &&
    !showAgentThread &&
    !showHistory &&
    !showRewriteActions &&
    !commandOutput &&
    !contextPanelOpen;
  const windowAnchor = isAgentInputMode ? "bottom" : "top";

  useEffect(() => {
    invoke("set_window_compact", {
      compact: showOnlySearch && !settingsOpen,
      anchor: windowAnchor,
    }).catch(() => {});
  }, [showOnlySearch, settingsOpen, windowAnchor]);

  // Refresh search results when agent turn ends (agent may have added/removed items)
  const prevTurnActiveRef = useRef(false);
  useEffect(() => {
    if (!agent.turnActive && prevTurnActiveRef.current) {
      launcher.refresh();
    }
    prevTurnActiveRef.current = agent.turnActive;
  }, [agent.turnActive, launcher]);

  // Load conversations when entering agent mode with no thread
  useEffect(() => {
    if (isAgentInputMode && !showAgentThread && !showHistory) {
      agent.loadConversations();
    }
  }, [isAgentInputMode, showAgentThread, showHistory]);

  // Filter conversations when typing in history mode
  useEffect(() => {
    if (showHistory && launcher.query.trim()) {
      agent.searchConversations(launcher.query.trim());
    } else if (showHistory) {
      agent.loadConversations();
    }
  }, [showHistory, launcher.query]);

  const handleContainerKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      // Escape dismisses command output
      if (commandOutput && e.key === "Escape") {
        e.preventDefault();
        setCommandOutput(null);
        invoke("hide_window");
        return;
      }

      if (isAgentInputMode && e.key === "Enter") {
        if (showHistory) {
          // Enter in history mode loads the selected conversation
          e.preventDefault();
          const conv = agent.conversations[historySelectedIndex];
          if (conv) handleLoadConversation(conv.id);
          return;
        }
        // Enter on rewrite quick action triggers agent with that prompt
        if (showRewriteActions && !launcher.query.trim()) {
          e.preventDefault();
          const suggestion =
            launchCtx.rewriteSuggestions[rewriteSelectedIndex];
          if (suggestion) handleRewriteQuickAction(suggestion.suggested_command);
          return;
        }
        e.preventDefault();
        if (!agent.turnActive) {
          const message = launcher.query.trim();
          if (message.length > 0) {
            agent.prompt(message);
            launcher.setQuery("");
          }
        }
        return;
      }

      if (isAgentInputMode && e.key === "Escape") {
        e.preventDefault();
        if (contextPanelOpen) {
          setContextPanelOpen(false);
        } else if (showHistory) {
          handleHideHistory();
        } else {
          exitAgentMode();
        }
        return;
      }

      // Arrow keys in rewrite quick actions mode
      if (
        showRewriteActions &&
        (e.key === "ArrowDown" || e.key === "ArrowUp")
      ) {
        e.preventDefault();
        const max = launchCtx.rewriteSuggestions.length - 1;
        if (e.key === "ArrowDown") {
          setRewriteSelectedIndex((prev) => Math.min(prev + 1, max));
        } else {
          setRewriteSelectedIndex((prev) => Math.max(prev - 1, 0));
        }
        return;
      }

      // Arrow keys in history mode
      if (showHistory && (e.key === "ArrowDown" || e.key === "ArrowUp")) {
        e.preventDefault();
        if (e.key === "ArrowDown") {
          setHistorySelectedIndex((prev) =>
            Math.min(prev + 1, agent.conversations.length - 1),
          );
        } else {
          setHistorySelectedIndex((prev) => Math.max(prev - 1, 0));
        }
        return;
      }

      launcher.handleKeyDown(e);
    },
    [
      commandOutput,
      isAgentInputMode,
      showHistory,
      showRewriteActions,
      contextPanelOpen,
      agent,
      launcher,
      launchCtx,
      exitAgentMode,
      handleHideHistory,
      handleLoadConversation,
      handleRewriteQuickAction,
      historySelectedIndex,
      rewriteSelectedIndex,
    ],
  );

  return (
    <div
      data-testid="app-root"
      ref={containerRef}
      className="h-full flex flex-col bg-launcher-bg/95 backdrop-blur-xl rounded-xl border border-launcher-border/50 shadow-2xl overflow-hidden"
      onKeyDown={handleContainerKeyDown}
      tabIndex={0}
    >
      {isAgentInputMode ? (
        <>
          {showConversationHistory ? (
            <ConversationHistory
              conversations={agent.conversations}
              selectedIndex={historySelectedIndex}
              onSelect={setHistorySelectedIndex}
              onLoad={handleLoadConversation}
              onDelete={agent.deleteConversation}
              onNewConversation={handleNewConversation}
            />
          ) : showAgentThread ? (
            <AgentResponse
              thread={agent.thread}
              thoughts={agent.thoughts}
              isThinking={agent.isThinking}
              turnActive={agent.turnActive}
              permissionRequest={agent.permissionRequest}
              onResolvePermission={agent.resolvePermission}
              onNewConversation={handleNewConversation}
              onShowHistory={handleShowHistory}
              hasSelection={launchCtx.hasSelection}
              onExecuteSlashCommand={handleExecuteSlashCommand}
              onExecuteSlashCommandError={handleExecuteSlashCommandError}
              onReplaceSelection={async (text) => {
                try {
                  await launchCtx.replaceSelection(text);
                  const lastUserMsg = [...agent.thread]
                    .reverse()
                    .find((m) => m.role === "user");
                  if (lastUserMsg) {
                    launchCtx
                      .recordRewrite(lastUserMsg.content)
                      .catch(() => {});
                  }
                  await invoke("hide_window");
                } catch (e) {
                  console.error("Failed to replace selection:", e);
                }
              }}
            />
          ) : showRewriteActions ? (
            <RewriteQuickActions
              suggestions={launchCtx.rewriteSuggestions}
              onSelect={handleRewriteQuickAction}
              selectedIndex={rewriteSelectedIndex}
              onHover={setRewriteSelectedIndex}
            />
          ) : contextPanelOpen && contextCount > 0 ? (
            <ContextPanel
              contextInfo={{
                hasSelection: launchCtx.hasSelection,
                hasClipboard: launchCtx.hasClipboard,
                sourceApp: launchCtx.context.source_window_title,
                sourceProcessName: launchCtx.context.source_process_name,
                selectedText: launchCtx.context.selected_text,
                clipboardText: launchCtx.context.clipboard_text,
              }}
              images={pastedImages}
              onRemoveSelection={launchCtx.clearSelection}
              onRemoveClipboard={launchCtx.clearClipboard}
              onRemoveImage={handleRemoveImage}
            />
          ) : (
            <div className="flex-1" />
          )}
          <SearchBar
            query={launcher.query}
            onQueryChange={launcher.setQuery}
            loading={false}
            agentStatus={agent.status}
            onSettingsClick={() => setSettingsOpen(true)}
            onBackClick={exitAgentMode}
            mode="composer"
            position="bottom"
            contextInfo={{
              hasSelection: launchCtx.hasSelection,
              hasClipboard: launchCtx.hasClipboard,
              sourceApp: launchCtx.context.source_window_title,
              selectedText: launchCtx.context.selected_text,
              clipboardText: launchCtx.context.clipboard_text,
            }}
            contextCount={contextCount}
            contextPanelOpen={contextPanelOpen}
            onContextClick={toggleContextPanel}
            onAddImage={handleAddImage}
          />
        </>
      ) : (
        <>
          <SearchBar
            query={launcher.query}
            onQueryChange={launcher.setQuery}
            onInputFocus={launcher.handleInputFocus}
            focusSignal={launcher.focusInputSignal}
            loading={launcher.loading}
            agentStatus={agent.status}
            onSettingsClick={() => setSettingsOpen(true)}
            onAgentClick={enterAgentMode}
            mode="search"
            position="top"
          />

          {commandOutput ? (
            <CommandOutput
              output={commandOutput}
              onDismiss={async () => {
                setCommandOutput(null);
                await invoke("hide_window");
              }}
            />
          ) : (
            <>
              {showResults && launcher.categories.length > 0 && (
                <CategoryBar
                  categories={launcher.categories}
                  activeCategory={launcher.activeCategory}
                  onCategoryChange={launcher.setActiveCategory}
                />
              )}

              {launcher.paramEntryMode && launcher.activeCommandParams.length > 0 && (
                <ParameterHint
                  commandName={launcher.activeCommandName}
                  params={launcher.activeCommandParams}
                  currentIndex={launcher.currentParamIndex}
                />
              )}

              {hasQuery &&
                (launcher.isSlashMode && !launcher.paramEntryMode ? (
                  <SlashCommandList
                    commands={launcher.slashCommands}
                    query={launcher.query}
                    selectedIndex={launcher.selectedSlashIndex}
                    onSelect={launcher.setSelectedSlashIndex}
                    onExecute={(cmd, args) =>
                      launcher.executeSlashCommand(cmd.name, args)
                    }
                  />
                ) : launcher.suggestions.length > 0 ? (
                  <CommandSuggestionPanel
                    suggestions={launcher.suggestions}
                    query={launcher.query}
                    selectedIndex={launcher.selectedSuggestionIndex}
                    onSelect={launcher.selectSuggestion}
                    onSave={launcher.saveCommandFromSuggestion}
                    saving={launcher.savingCommand}
                  />
                ) : showResults ? (
                  <ItemList
                    items={launcher.items}
                    selectedIndex={launcher.selectedIndex}
                    onSelect={launcher.setSelectedIndex}
                    onExecute={launcher.executeSelected}
                  />
                ) : null)}

              {(showResults || (hasQuery && launcher.suggestions.length > 0)) && (
                <StatusBar
                  itemCount={launcher.items.length}
                  agentMode={launcher.agentMode}
                  agentTurnActive={agent.turnActive}
                  hasSuggestions={launcher.suggestions.length > 0}
                />
              )}
            </>
          )}
        </>
      )}

      {settingsOpen && (
        <AgentSettings
          status={agent.status}
          configOptions={agent.configOptions}
          onConnect={handleConnect}
          onDisconnect={agent.disconnect}
          onClose={() => setSettingsOpen(false)}
          onSetConfigOption={agent.setConfigOption}
        />
      )}

      {toast && (
        <Toast toast={toast} onDismiss={() => setToast(null)} />
      )}
    </div>
  );
}

export default App;
