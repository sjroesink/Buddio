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
            const hasDetails = !!(entry.toolContent || entry.commandPreview);
            const isExpanded = expandedTools.has(entry.id);
            return (
              <div key={entry.id}>
                <div
                  className={`tool-call-entry${hasDetails ? "" : " tool-call-no-expand"}`}
                  onClick={hasDetails ? () => toggleExpanded(entry.id) : undefined}
                >
                  {hasDetails && (
                    <span className="tool-call-chevron">
                      {isExpanded ? "\u25BC" : "\u25B6"}
                    </span>
                  )}
                  <span className="tool-call-title">
                    {friendlyToolTitle(entry.toolTitle ?? "Tool")}
                  </span>
                  <span className={`tool-call-status ${statusClass}`}>
                    {statusLabel}
                  </span>
                </div>
                {isExpanded && hasDetails && (
                  <div className="tool-call-details">
                    <pre>{entry.toolContent || entry.commandPreview}</pre>
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

          return (
            <div key={entry.id}>
              <div
                className={`mb-1 flex ${
                  entry.role === "user" ? "justify-end" : "justify-start"
                }`}
              >
                <div
                  className={`max-w-[85%] rounded-lg px-3 py-2 text-sm leading-relaxed ${
                    entry.role === "user"
                      ? "bg-launcher-accent/25 border border-launcher-accent/35 text-launcher-text"
                      : "bg-launcher-surface/55 border border-launcher-border/40 text-launcher-text/90"
                  }`}
                >
                  {entry.role === "assistant" ? (
                    <div className="agent-markdown">
                      <ReactMarkdown remarkPlugins={[remarkGfm]}>
                        {entry.content}
                      </ReactMarkdown>
                    </div>
                  ) : (
                    <div className="whitespace-pre-wrap">{entry.content}</div>
                  )}
                </div>
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
