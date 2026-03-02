import { useEffect, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { AgentThreadMessage, PermissionRequest } from "../types";
import { PermissionDialog } from "./PermissionDialog";

function friendlyToolTitle(raw: string): string {
  const l = raw.toLowerCase();
  if (l.includes("slash_commands_add")) return "Create slash command";
  if (l.includes("slash_commands_run")) return "Run slash command";
  if (l.includes("slash_commands_remove")) return "Remove slash command";
  if (l.includes("slash_commands_get_params")) return "Get command parameters";
  if (l.includes("slash_commands_get")) return "Get slash command";
  if (l.includes("slash_commands_list")) return "List slash commands";
  if (l.includes("slash_commands_search")) return "Search slash commands";
  if (l.includes("items_search")) return "Search items";
  if (l.includes("items_list")) return "List items";
  if (l.includes("items_get")) return "Get item";
  if (l.includes("memory_")) return "Access memory";
  if (l.includes("history_")) return "Check history";
  if (l.includes("settings_")) return "Access settings";
  if (l.includes("conversations_")) return "Manage conversations";
  // Fallback: strip prefix and humanize
  return raw.replace(/^mcp_(buddio|golaunch)_/i, "").replace(/_/g, " ");
}

/** Extract the command name from a slash_commands_add tool call's content (JSON). */
function extractCommandName(toolContent?: string): string | null {
  if (!toolContent) return null;
  try {
    const parsed = JSON.parse(toolContent);
    if (parsed.name) return parsed.name;
  } catch {
    // Content might not be JSON — try to find a "name" field pattern
    const match = toolContent.match(/"name"\s*:\s*"([^"]+)"/);
    if (match) return match[1];
  }
  return null;
}

function ReplaceSelectionAction({
  text,
  onReplace,
}: {
  text: string;
  onReplace: (text: string) => void;
}) {
  const [state, setState] = useState<"idle" | "done" | "error">("idle");

  return (
    <div className="mb-3 ml-1 space-y-1.5">
      {/* Preview of the replacement text — always visible */}
      <div
        className={`max-w-[85%] rounded-md px-3 py-2 text-xs border ${
          state === "done"
            ? "bg-green-500/10 border-green-500/20"
            : "bg-purple-500/10 border-purple-500/20"
        } text-launcher-text/80`}
      >
        <div
          className={`text-[10px] font-medium mb-1 uppercase tracking-wider ${
            state === "done"
              ? "text-green-400/70"
              : "text-purple-400/70"
          }`}
        >
          {state === "done" ? "Replaced" : "Preview"}
        </div>
        <div className="whitespace-pre-wrap break-words leading-relaxed">
          {text}
        </div>
      </div>

      {/* Action button / status */}
      {state === "done" ? (
        <div className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium bg-green-500/20 text-green-300 border border-green-500/30">
          <svg className="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
          </svg>
          Replaced in source app
        </div>
      ) : state === "error" ? (
        <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium bg-red-500/20 text-red-300 border border-red-500/30">
          <svg className="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
          </svg>
          Failed to replace — try copying manually
        </span>
      ) : (
        <button
          onClick={async () => {
            try {
              onReplace(text);
              setState("done");
            } catch {
              setState("error");
            }
          }}
          className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium bg-purple-500/20 text-purple-300 border border-purple-500/30 hover:bg-purple-500/30 hover:text-purple-200 transition-colors"
          title="Replace selection in source app with this response"
        >
          <svg className="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
          </svg>
          Replace selection
        </button>
      )}
    </div>
  );
}

function ExecuteSlashCommandAction({
  commandName,
  onExecute,
  onError,
}: {
  commandName: string;
  onExecute: (name: string) => Promise<void>;
  onError: (error: string) => void;
}) {
  const [state, setState] = useState<"idle" | "running" | "done" | "error">("idle");

  return (
    <div className="mb-3 ml-1 space-y-1.5">
      <div className="flex items-center gap-2">
        {state === "done" ? (
          <div className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium bg-green-500/20 text-green-300 border border-green-500/30">
            <svg className="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
            </svg>
            Executed
          </div>
        ) : state === "error" ? (
          <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium bg-red-500/20 text-red-300 border border-red-500/30">
            <svg className="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
            </svg>
            Failed
          </span>
        ) : state === "running" ? (
          <div className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium bg-blue-500/20 text-blue-300 border border-blue-500/30">
            <svg className="w-3 h-3 animate-spin" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
            </svg>
            Running...
          </div>
        ) : (
          <button
            onClick={async () => {
              setState("running");
              try {
                await onExecute(commandName);
                setState("done");
              } catch (e) {
                setState("error");
                onError(String(e));
              }
            }}
            className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium bg-green-500/20 text-green-300 border border-green-500/30 hover:bg-green-500/30 hover:text-green-200 transition-colors"
            title={`Execute /${commandName}`}
          >
            <svg className="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z" />
              <path strokeLinecap="round" strokeLinejoin="round" d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
            Execute /{commandName}
          </button>
        )}
      </div>
    </div>
  );
}

function DogAvatar() {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 250 250"
      className="robot-container"
      aria-hidden="true"
    >
      <g className="tail">
        <path
          d="M 160 140 Q 195 125 185 85"
          fill="none"
          stroke="#64748b"
          strokeWidth="6"
          strokeLinecap="round"
        />
        <circle cx="185" cy="85" r="8" fill="#fcd34d" stroke="#64748b" strokeWidth="4" />
      </g>

      <rect
        x="42"
        y="65"
        width="26"
        height="60"
        rx="13"
        transform="rotate(25 55 95)"
        fill="#94a3b8"
        stroke="#475569"
        strokeWidth="4"
      />
      <rect
        x="182"
        y="65"
        width="26"
        height="60"
        rx="13"
        transform="rotate(-25 195 95)"
        fill="#94a3b8"
        stroke="#475569"
        strokeWidth="4"
      />

      <circle cx="75" cy="165" r="22" fill="#94a3b8" stroke="#475569" strokeWidth="4" />
      <rect x="45" y="175" width="36" height="16" rx="8" fill="#64748b" stroke="#475569" strokeWidth="4" />
      <circle cx="175" cy="165" r="22" fill="#94a3b8" stroke="#475569" strokeWidth="4" />
      <rect x="169" y="175" width="36" height="16" rx="8" fill="#64748b" stroke="#475569" strokeWidth="4" />

      <rect x="85" y="110" width="80" height="70" rx="30" fill="#e2e8f0" stroke="#475569" strokeWidth="4" />

      <rect x="105" y="145" width="40" height="16" rx="6" fill="#1e293b" />
      <rect x="110" y="149" width="8" height="8" rx="2" fill="#22c55e" className="battery-light" />
      <rect x="121" y="149" width="8" height="8" rx="2" fill="#22c55e" className="battery-light" />
      <rect x="132" y="149" width="8" height="8" rx="2" fill="#22c55e" className="battery-light" />

      <rect x="95" y="125" width="22" height="65" rx="11" fill="#cbd5e1" stroke="#475569" strokeWidth="4" />
      <rect x="133" y="125" width="22" height="65" rx="11" fill="#cbd5e1" stroke="#475569" strokeWidth="4" />

      <line x1="99" y1="180" x2="113" y2="180" stroke="#475569" strokeWidth="3" strokeLinecap="round" />
      <line x1="137" y1="180" x2="151" y2="180" stroke="#475569" strokeWidth="3" strokeLinecap="round" />

      <rect x="90" y="112" width="70" height="14" rx="7" fill="#ef4444" stroke="#475569" strokeWidth="4" />
      <circle cx="125" cy="133" r="10" fill="#fbbf24" stroke="#475569" strokeWidth="4" />
      <circle cx="125" cy="133" r="3" fill="#f59e0b" />

      <line x1="125" y1="45" x2="125" y2="15" stroke="#475569" strokeWidth="4" strokeLinecap="round" />
      <circle cx="125" cy="15" r="7" fill="#38bdf8" stroke="#475569" strokeWidth="4" />
      <circle cx="125" cy="15" r="12" fill="#38bdf8" opacity="0.3" className="battery-light" />

      <rect x="65" y="45" width="120" height="85" rx="35" fill="#cbd5e1" stroke="#475569" strokeWidth="4" />

      <rect x="80" y="55" width="90" height="58" rx="20" fill="#0f172a" stroke="#1e293b" strokeWidth="2" />

      <g className="eyes">
        <ellipse cx="103" cy="78" rx="8" ry="13" fill="#38bdf8" />
        <circle cx="101" cy="73" r="3" fill="#ffffff" />
        <ellipse cx="147" cy="78" rx="8" ry="13" fill="#38bdf8" />
        <circle cx="145" cy="73" r="3" fill="#ffffff" />
      </g>

      <circle cx="92" cy="95" r="6" fill="#f43f5e" opacity="0.7" />
      <circle cx="158" cy="95" r="6" fill="#f43f5e" opacity="0.7" />

      <path d="M 116 92 Q 125 102 134 92" fill="none" stroke="#38bdf8" strokeWidth="4" strokeLinecap="round" />

      <path
        d="M 85 55 Q 125 50 165 55"
        fill="none"
        stroke="#ffffff"
        strokeWidth="4"
        strokeLinecap="round"
        opacity="0.5"
      />
    </svg>
  );
}

interface AgentResponseProps {
  thread: AgentThreadMessage[];
  thoughts: string;
  isThinking: boolean;
  turnActive: boolean;
  permissionRequest: PermissionRequest | null;
  onResolvePermission: (requestId: string, optionId: string) => void;
  onNewConversation?: () => void;
  onShowHistory?: () => void;
  hasSelection?: boolean;
  onExecuteSlashCommand?: (name: string) => Promise<void>;
  onExecuteSlashCommandError?: (error: string) => void;
  onReplaceSelection?: (text: string) => void;
}

export function AgentResponse({
  thread,
  thoughts,
  isThinking,
  turnActive,
  permissionRequest,
  onResolvePermission,
  onNewConversation,
  onShowHistory,
  hasSelection,
  onExecuteSlashCommand,
  onExecuteSlashCommandError,
  onReplaceSelection,
}: AgentResponseProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const [expandedTools, setExpandedTools] = useState<Set<string>>(new Set());

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [thread, thoughts, isThinking, turnActive, permissionRequest]);

  const toggleExpanded = (id: string) => {
    setExpandedTools((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  // Detect if a slash command was created during this thread
  const createdCommandEntry = !turnActive
    ? thread.find(
        (e) =>
          e.role === "tool" &&
          e.toolTitle?.toLowerCase().includes("slash_commands_add") &&
          (e.toolStatus === "completed" || e.toolStatus === "approved"),
      )
    : null;
  const createdCommandName = createdCommandEntry
    ? extractCommandName(createdCommandEntry.toolContent ?? createdCommandEntry.commandPreview)
    : null;
  const hasVisibleEntries = thread.some(
    (entry) => entry.role === "tool" || entry.content.length > 0,
  );

  return (
    <div data-testid="agent-response" className="flex flex-col flex-1 min-h-0">
      {(onNewConversation || onShowHistory) && (
        <div className="flex items-center justify-end px-3 py-1 border-b border-launcher-border/20 flex-shrink-0">
          {onShowHistory && (
            <button
              onClick={onShowHistory}
              className="text-xs px-2 py-0.5 rounded text-launcher-muted hover:text-launcher-text hover:bg-launcher-hover transition-colors"
              title="Conversation history"
            >
              <svg
                className="w-3.5 h-3.5 inline mr-1"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
                strokeWidth={2}
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"
                />
              </svg>
              History
            </button>
          )}
          {onNewConversation && (
            <button
              onClick={onNewConversation}
              className="text-xs px-2 py-0.5 rounded text-launcher-muted hover:text-launcher-text hover:bg-launcher-hover transition-colors ml-1"
              title="New conversation"
            >
              <svg
                className="w-3.5 h-3.5 inline mr-1"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
                strokeWidth={2}
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  d="M12 4v16m8-8H4"
                />
              </svg>
              New
            </button>
          )}
        </div>
      )}

      <div data-testid="agent-thread" className="agent-response" ref={scrollRef}>
        {!hasVisibleEntries && !isThinking && !turnActive && !permissionRequest && (
          <div
            data-testid="agent-empty-state"
            className="flex h-full min-h-[160px] flex-col items-center justify-center px-4 text-center text-launcher-muted/60"
          >
            <span className="text-sm text-launcher-text/75">New session</span>
            <span className="mt-1 text-xs">
              Start typing below, or open History to continue a previous chat.
            </span>
          </div>
        )}
        {thread.map((entry, index) => {
          // Skip empty assistant messages
          if (entry.role === "assistant" && entry.content.length === 0) {
            return null;
          }

          // Tool call entries render as expandable status lines
          if (entry.role === "tool") {
            const isPending = entry.toolStatus === "pending";
            const statusClass = entry.toolStatus
              ? `tool-call-status-${entry.toolStatus}`
              : "tool-call-status-running";
            const statusLabel =
              entry.toolStatus === "completed"
                ? "DONE"
                : (entry.toolStatus ?? "running").toUpperCase();
            const isExpanded = expandedTools.has(entry.id);
            const detailsId = `tool-details-${entry.id.replace(/[^a-zA-Z0-9_-]/g, "-")}`;
            const toolPayload = entry.toolContent ?? entry.commandPreview;
            return (
              <div key={entry.id}>
                <button
                  type="button"
                  className={`tool-call-entry${isExpanded ? " tool-call-entry-expanded" : ""}`}
                  onClick={() => toggleExpanded(entry.id)}
                  aria-expanded={isExpanded}
                  aria-controls={detailsId}
                >
                  <span className="tool-call-chevron">
                    {isExpanded ? "\u25BC" : "\u25B6"}
                  </span>
                  <span className="tool-call-title">
                    {friendlyToolTitle(entry.toolTitle ?? "Tool")}
                  </span>
                  <span className={`tool-call-status ${statusClass}`}>
                    {statusLabel}
                  </span>
                </button>
                {isExpanded && (
                  <div id={detailsId} className="tool-call-details">
                    <div className="tool-call-details-meta">
                      <div className="tool-call-details-row">
                        <span className="tool-call-details-label">Tool</span>
                        <span className="tool-call-details-value">
                          {entry.toolTitle ?? "Tool"}
                        </span>
                      </div>
                      <div className="tool-call-details-row">
                        <span className="tool-call-details-label">Call ID</span>
                        <span className="tool-call-details-value">
                          {entry.id.replace(/^tool-/, "")}
                        </span>
                      </div>
                      {entry.toolKind && (
                        <div className="tool-call-details-row">
                          <span className="tool-call-details-label">Kind</span>
                          <span className="tool-call-details-value">
                            {entry.toolKind}
                          </span>
                        </div>
                      )}
                      {entry.toolStatusRaw && (
                        <div className="tool-call-details-row">
                          <span className="tool-call-details-label">
                            Raw status
                          </span>
                          <span className="tool-call-details-value">
                            {entry.toolStatusRaw}
                          </span>
                        </div>
                      )}
                    </div>
                    {toolPayload ? (
                      <pre>{toolPayload}</pre>
                    ) : (
                      <div className="tool-call-details-empty">
                        No payload captured for this tool call.
                      </div>
                    )}
                  </div>
                )}
                {isPending && permissionRequest && (
                  <PermissionDialog
                    request={permissionRequest}
                    onResolve={onResolvePermission}
                  />
                )}
              </div>
            );
          }

          // Find the last assistant message that actually has content
          const isLastNonEmptyAssistant =
            entry.role === "assistant" &&
            entry.content.length > 0 &&
            !thread.some(
              (e, i) =>
                i > index && e.role === "assistant" && e.content.length > 0,
            );
          const showExecute =
            isLastNonEmptyAssistant &&
            !turnActive &&
            createdCommandName &&
            onExecuteSlashCommand;
          const showReplace =
            isLastNonEmptyAssistant &&
            !turnActive &&
            !createdCommandName &&
            hasSelection &&
            onReplaceSelection;
          const bubbleClassName = `rounded-lg px-3 py-2 text-sm leading-relaxed ${
            entry.role === "user"
              ? "max-w-[85%] bg-launcher-accent/25 border border-launcher-accent/35 text-launcher-text"
              : "max-w-[82%] bg-launcher-surface/55 border border-launcher-border/40 text-launcher-text/90"
          }`;

          return (
            <div key={entry.id}>
              <div
                className={`mb-1 flex ${
                  entry.role === "user" ? "justify-end" : "justify-start"
                }`}
              >
                {entry.role === "assistant" ? (
                  <div className="agent-assistant-row">
                    <span className="agent-dog-avatar">
                      <DogAvatar />
                    </span>
                    <div className={bubbleClassName}>
                      <div className="agent-markdown">
                        <ReactMarkdown remarkPlugins={[remarkGfm]}>
                          {entry.content}
                        </ReactMarkdown>
                      </div>
                    </div>
                  </div>
                ) : (
                  <div className={bubbleClassName}>
                    <div className="whitespace-pre-wrap">{entry.content}</div>
                  </div>
                )}
              </div>
              {showExecute && (
                <ExecuteSlashCommandAction
                  commandName={createdCommandName!}
                  onExecute={onExecuteSlashCommand!}
                  onError={onExecuteSlashCommandError ?? (() => {})}
                />
              )}
              {showReplace && (
                <ReplaceSelectionAction
                  text={entry.content}
                  onReplace={onReplaceSelection!}
                />
              )}
            </div>
          );
        })}

        {/* Fallback: show permission dialog at the bottom if there's no matching tool_call entry */}
        {permissionRequest &&
          !thread.some(
            (e) => e.role === "tool" && e.toolStatus === "pending",
          ) && (
            <div className="mb-3 flex justify-start">
              <div className="max-w-[85%] rounded-lg bg-launcher-surface/55 border border-launcher-border/40">
                <PermissionDialog
                  request={permissionRequest}
                  onResolve={onResolvePermission}
                />
              </div>
            </div>
          )}

        {isThinking && (
          <div className="agent-thinking mb-2">
            <span className="thinking-dots">
              <span>.</span>
              <span>.</span>
              <span>.</span>
            </span>
            {thoughts && <div className="agent-thought">{thoughts}</div>}
          </div>
        )}

        {turnActive && <div className="agent-streaming-indicator" />}
      </div>
    </div>
  );
}
