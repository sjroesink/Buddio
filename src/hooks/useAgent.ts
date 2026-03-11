import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  AgentStatus,
  AgentUpdate,
  AgentConfig,
  PermissionRequest,
  AgentThreadMessage,
  ConversationWithPreview,
  SessionConfigOptionInfo,
  UserQuestionRequest,
} from "../types";

function makeMessageId(prefix: "user" | "assistant") {
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
}

function normalizeToolStatus(
  rawStatus: string | null | undefined,
): AgentThreadMessage["toolStatus"] | null {
  if (!rawStatus) return null;

  const normalized = rawStatus.toLowerCase().replace(/\s+/g, "");

  if (
    normalized.includes("error") ||
    normalized.includes("fail") ||
    normalized.includes("cancel") ||
    normalized.includes("deny") ||
    normalized.includes("reject")
  ) {
    return "error";
  }

  if (
    normalized.includes("complete") ||
    normalized.includes("done") ||
    normalized.includes("success")
  ) {
    return "completed";
  }

  if (normalized.includes("approve") || normalized.includes("allow")) {
    return "approved";
  }

  if (
    normalized.includes("pending") ||
    normalized.includes("await") ||
    normalized.includes("queue")
  ) {
    return "pending";
  }

  if (
    normalized.includes("running") ||
    normalized.includes("inprogress") ||
    normalized.includes("execut") ||
    normalized.includes("start")
  ) {
    return "running";
  }

  return null;
}

export function useAgent() {
  const [status, setStatus] = useState<AgentStatus>("disconnected");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [messages, setMessages] = useState("");
  const [thread, setThread] = useState<AgentThreadMessage[]>([]);
  const [thoughts, setThoughts] = useState("");
  const [isThinking, setIsThinking] = useState(false);
  const [turnActive, setTurnActive] = useState(false);
  const [permissionRequest, setPermissionRequest] =
    useState<PermissionRequest | null>(null);
  const [userQuestion, setUserQuestion] =
    useState<UserQuestionRequest | null>(null);
  const activeAssistantIdRef = useRef<string | null>(null);
  const startupConnectAttempted = useRef(false);

  // Conversation persistence state
  const [activeConversationId, setActiveConversationId] = useState<
    string | null
  >(null);
  const [conversations, setConversations] = useState<
    ConversationWithPreview[]
  >([]);

  // Session config options
  const [configOptions, setConfigOptions] = useState<
    SessionConfigOptionInfo[]
  >([]);

  // Refs for accessing current state in event listeners (avoids stale closures)
  const threadRef = useRef(thread);
  threadRef.current = thread;
  const activeConversationIdRef = useRef(activeConversationId);
  activeConversationIdRef.current = activeConversationId;

  useEffect(() => {
    const unlistenUpdate = listen<AgentUpdate>("agent-update", (event) => {
      const update = event.payload;

      switch (update.type) {
        case "message_chunk": {
          setMessages((prev) => prev + update.text);
          setIsThinking(false);

          const activeAssistantId = activeAssistantIdRef.current;
          if (activeAssistantId) {
            setThread((prev) =>
              prev.map((entry) =>
                entry.id === activeAssistantId
                  ? { ...entry, content: entry.content + update.text }
                  : entry,
              ),
            );
          }
          break;
        }
        case "thought_chunk":
          setThoughts((prev) => prev + update.text);
          setIsThinking(true);
          break;
        case "tool_call": {
          setIsThinking(true);
          // Close current assistant message so subsequent text goes to a new one
          activeAssistantIdRef.current = null;
          // Add tool call entry + new assistant message for chronological interleaving
          setThread((prev) => {
            const newAssistantId = makeMessageId("assistant");
            activeAssistantIdRef.current = newAssistantId;
            return [
              ...prev,
              {
                id: `tool-${update.id}`,
                role: "tool" as const,
                content: "",
                toolTitle: update.title ?? "Tool",
                toolStatus: "running" as const,
                toolKind: update.kind,
                toolContent: update.content ?? undefined,
              },
              { id: newAssistantId, role: "assistant" as const, content: "" },
            ];
          });
          break;
        }
        case "tool_call_update": {
          // Update existing tool call entry status
          const toolEntryId = `tool-${update.id}`;
          const normalizedStatus = normalizeToolStatus(update.status);
          setThread((prev) =>
            prev.map((entry) =>
              entry.id === toolEntryId
                ? {
                    ...entry,
                    toolTitle: update.title ?? entry.toolTitle,
                    toolStatus: normalizedStatus ?? entry.toolStatus,
                    toolStatusRaw: update.status ?? entry.toolStatusRaw,
                  }
                : entry,
            ),
          );
          break;
        }
        case "plan":
          break;
        case "turn_complete": {
          setTurnActive(false);
          setIsThinking(false);
          const turnCancelled = update.stop_reason.toLowerCase().includes("cancel");

          // Some ACP providers don't always emit a final tool_call_update.
          // Finalize still-running tool entries when the turn ends.
          setThread((prev) =>
            prev.map((entry) => {
              if (entry.role !== "tool") return entry;
              const currentStatus = entry.toolStatus ?? "running";
              if (currentStatus !== "running") return entry;
              return {
                ...entry,
                toolStatus: turnCancelled ? "error" : "completed",
                toolStatusRaw:
                  entry.toolStatusRaw ??
                  (turnCancelled
                    ? "Cancelled (inferred on turn_complete)"
                    : "Complete (inferred on turn_complete)"),
              };
            }),
          );

          // Persist combined assistant messages to the database
          const convId = activeConversationIdRef.current;
          const currentThread = threadRef.current;
          if (convId) {
            const combinedContent = currentThread
              .filter((m) => m.role === "assistant" && m.content.length > 0)
              .map((m) => m.content)
              .join("\n\n");
            if (combinedContent.length > 0) {
              invoke("add_conversation_message", {
                conversationId: convId,
                role: "assistant",
                content: combinedContent,
              }).catch(() => {});
            }
          }

          activeAssistantIdRef.current = null;
          break;
        }
        case "status_change":
          setStatus(update.status);
          if (update.status !== "error") setErrorMessage(null);
          break;
      }
    });

    const unlistenPermission = listen<PermissionRequest>(
      "agent-permission-request",
      (event) => {
        const req = event.payload;
        setPermissionRequest(req);

        // Update the matching tool call entry with the command preview and pending status
        const toolEntryId = `tool-${req.request_id}`;
        setThread((prev) =>
          prev.map((entry) =>
            entry.id === toolEntryId
              ? {
                  ...entry,
                  commandPreview: req.command_preview ?? undefined,
                  toolStatus: "pending" as const,
                }
              : entry,
          ),
        );
      },
    );

    const unlistenUserQuestion = listen<UserQuestionRequest>(
      "agent-user-question",
      (event) => {
        setUserQuestion(event.payload);
      },
    );

    const unlistenConfigOptions = listen<SessionConfigOptionInfo[]>(
      "agent-config-options",
      (event) => {
        setConfigOptions(event.payload);
      },
    );

    return () => {
      unlistenUpdate.then((f) => f());
      unlistenPermission.then((f) => f());
      unlistenUserQuestion.then((f) => f());
      unlistenConfigOptions.then((f) => f());
    };
  }, []);

  const connect = useCallback(async (config: AgentConfig) => {
    try {
      setStatus("connecting");
      setErrorMessage(null);
      await invoke("agent_connect", { config });
      // Fetch initial config options after connect
      const opts = await invoke<SessionConfigOptionInfo[]>(
        "agent_get_config_options",
      );
      setConfigOptions(opts);
    } catch (e) {
      console.error("Failed to connect agent:", e);
      setStatus("error");
      setErrorMessage(String(e));
    }
  }, []);

  useEffect(() => {
    if (startupConnectAttempted.current) return;
    startupConnectAttempted.current = true;

    async function connectOnStartup() {
      try {
        const currentStatus = await invoke<AgentStatus>("agent_get_status");
        setStatus(currentStatus);

        if (currentStatus === "connected" || currentStatus === "connecting") {
          if (currentStatus === "connected") {
            const opts = await invoke<SessionConfigOptionInfo[]>(
              "agent_get_config_options",
            );
            setConfigOptions(opts);
          }
          return;
        }

        const config = await invoke<AgentConfig>("get_agent_config");

        // Check provider-specific requirements before auto-connecting
        const provider = config.provider || "acp";
        if (provider === "acp" && !config.binary_path.trim()) {
          return;
        }
        if (provider === "claude" && config.auth_method !== "oauth" && !config.api_key?.trim()) {
          return;
        }
        if (provider === "copilot" && !config.api_key?.trim()) {
          return;
        }

        // Merge per-agent env vars into config so API keys are available
        if (config.agent_id) {
          try {
            const pairs = await invoke<[string, string][]>("get_agent_env", {
              agentId: config.agent_id,
            });
            if (pairs.length > 0) {
              const agentEnv = pairs
                .filter(([, v]) => v)
                .map(([k, v]) => `${k}=${v}`)
                .join(",");
              if (agentEnv) {
                config.env = config.env
                  ? `${config.env},${agentEnv}`
                  : agentEnv;
              }
            }
          } catch {
            // Per-agent env vars not available, continue with config.env
          }
        }

        setStatus("connecting");
        await connect(config);
      } catch (e) {
        console.error("Failed to restore agent connection on startup:", e);
        setStatus("error");
        setErrorMessage(String(e));
      }
    }

    connectOnStartup();
  }, [connect]);

  const disconnect = useCallback(async () => {
    try {
      await invoke("agent_disconnect");
    } catch (e) {
      console.error("Failed to disconnect agent:", e);
    }
    setStatus("disconnected");
    setErrorMessage(null);
    setThread([]);
    setMessages("");
    setThoughts("");
    setTurnActive(false);
    setIsThinking(false);
    setActiveConversationId(null);
    setConfigOptions([]);
    activeAssistantIdRef.current = null;
  }, []);

  const startTurn = useCallback(
    async (
      query: string,
      invokeCommand: (normalizedQuery: string) => Promise<void>,
    ) => {
      const normalizedQuery = query.trim();
      if (!normalizedQuery) return;

      const assistantId = makeMessageId("assistant");

      // Create conversation if none active
      let convId = activeConversationIdRef.current;
      if (!convId) {
        try {
          const title =
            normalizedQuery.length > 50
              ? normalizedQuery.slice(0, 50) + "..."
              : normalizedQuery;
          const conv = await invoke<{ id: string }>("create_conversation", {
            title,
          });
          convId = conv.id;
          setActiveConversationId(convId);
        } catch (e) {
          console.error("Failed to create conversation:", e);
        }
      }

      // Persist user message
      if (convId) {
        invoke("add_conversation_message", {
          conversationId: convId,
          role: "user",
          content: normalizedQuery,
        }).catch(() => {});
      }

      setThread((prev) => [
        ...prev,
        { id: makeMessageId("user"), role: "user", content: normalizedQuery },
        { id: assistantId, role: "assistant", content: "" },
      ]);

      activeAssistantIdRef.current = assistantId;
      setMessages("");
      setThoughts("");
      setTurnActive(true);
      setIsThinking(true);

      try {
        await invokeCommand(normalizedQuery);
      } catch (e) {
        console.error("Failed to prompt agent:", e);
        setTurnActive(false);
        setIsThinking(false);
        activeAssistantIdRef.current = null;
        setThread((prev) =>
          prev.map((entry) =>
            entry.id === assistantId && entry.content.length === 0
              ? {
                  ...entry,
                  content:
                    "I hit an error while sending that. Please try again.",
                }
              : entry,
          ),
        );
      }
    },
    [],
  );

  const prompt = useCallback(
    async (query: string) => {
      await startTurn(query, async (normalizedQuery) => {
        const items = await invoke("get_all_items");
        await invoke("agent_prompt", {
          query: normalizedQuery,
          contextItems: items,
        });
      });
    },
    [startTurn],
  );

  const promptSlashCommand = useCallback(
    async (query: string) => {
      await startTurn(query, async (normalizedQuery) => {
        await invoke("agent_prompt_slash_command", {
          query: normalizedQuery,
        });
      });
    },
    [startTurn],
  );

  const cancel = useCallback(async () => {
    try {
      await invoke("agent_cancel");
      setTurnActive(false);
      setIsThinking(false);
      activeAssistantIdRef.current = null;
    } catch (e) {
      console.error("Failed to cancel:", e);
    }
  }, []);

  const clearThread = useCallback(() => {
    setThread([]);
    setMessages("");
    setThoughts("");
    setTurnActive(false);
    setIsThinking(false);
    setActiveConversationId(null);
    activeAssistantIdRef.current = null;
  }, []);

  const resolvePermission = useCallback(
    async (requestId: string, optionId: string) => {
      try {
        await invoke("agent_resolve_permission", { requestId, optionId });
        setPermissionRequest(null);

        // Mark the tool call entry as approved (or denied based on optionId)
        const toolEntryId = `tool-${requestId}`;
        setThread((prev) =>
          prev.map((entry) =>
            entry.id === toolEntryId
              ? { ...entry, toolStatus: "approved" as const }
              : entry,
          ),
        );
      } catch (e) {
        console.error("Failed to resolve permission:", e);
      }
    },
    [],
  );

  const resolveQuestion = useCallback(
    async (requestId: string, answers: Record<string, string>) => {
      try {
        await invoke("agent_resolve_question", { requestId, answers });
        setUserQuestion(null);
      } catch (e) {
        console.error("Failed to resolve question:", e);
      }
    },
    [],
  );

  // --- Conversation management ---

  const loadConversations = useCallback(async () => {
    try {
      const list = await invoke<ConversationWithPreview[]>(
        "list_conversations",
        { limit: 50 },
      );
      setConversations(list);
    } catch (e) {
      console.error("Failed to load conversations:", e);
    }
  }, []);

  const loadConversation = useCallback(async (conversationId: string) => {
    try {
      const msgs = await invoke<
        { id: string; role: string; content: string }[]
      >("get_conversation_messages", { conversationId });

      const rebuilt: AgentThreadMessage[] = msgs.map((m) => ({
        id: m.id,
        role: m.role as "user" | "assistant",
        content: m.content,
      }));

      setThread(rebuilt);
      setActiveConversationId(conversationId);
      setMessages("");
      setThoughts("");
      setTurnActive(false);
      setIsThinking(false);
      activeAssistantIdRef.current = null;
    } catch (e) {
      console.error("Failed to load conversation:", e);
    }
  }, []);

  const newConversation = useCallback(() => {
    setThread([]);
    setMessages("");
    setThoughts("");
    setTurnActive(false);
    setIsThinking(false);
    setActiveConversationId(null);
    activeAssistantIdRef.current = null;
  }, []);

  const deleteConversation = useCallback(
    async (conversationId: string) => {
      try {
        await invoke("delete_conversation", { id: conversationId });
        setConversations((prev) =>
          prev.filter((c) => c.id !== conversationId),
        );
        if (activeConversationIdRef.current === conversationId) {
          newConversation();
        }
      } catch (e) {
        console.error("Failed to delete conversation:", e);
      }
    },
    [newConversation],
  );

  const searchConversations = useCallback(async (query: string) => {
    try {
      const list = await invoke<ConversationWithPreview[]>(
        "search_conversations",
        { query },
      );
      setConversations(list);
    } catch (e) {
      console.error("Failed to search conversations:", e);
    }
  }, []);

  // --- Config option management ---

  const setConfigOption = useCallback(
    async (configId: string, value: string) => {
      try {
        const updated = await invoke<SessionConfigOptionInfo[]>(
          "agent_set_config_option",
          { configId, value },
        );
        setConfigOptions(updated);
      } catch (e) {
        console.error("Failed to set config option:", e);
      }
    },
    [],
  );

  return {
    status,
    errorMessage,
    messages,
    thread,
    thoughts,
    isThinking,
    turnActive,
    permissionRequest,
    userQuestion,
    activeConversationId,
    conversations,
    configOptions,
    connect,
    disconnect,
    prompt,
    promptSlashCommand,
    cancel,
    clearThread,
    resolvePermission,
    resolveQuestion,
    loadConversations,
    loadConversation,
    newConversation,
    deleteConversation,
    searchConversations,
    setConfigOption,
  };
}
