import { useEffect, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { AgentThreadMessage, LaunchItem, PermissionRequest } from "../types";
import { PermissionDialog } from "./PermissionDialog";

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

function ExecuteItemAction({
  items,
  onExecute,
}: {
  items: LaunchItem[];
  onExecute: (itemId: string) => void;
}) {
  const [executed, setExecuted] = useState<Set<string>>(new Set());

  return (
    <div className="mb-3 ml-1 space-y-1.5">
      {items.map((item) => {
        const done = executed.has(item.id);
        return (
          <div key={item.id} className="flex items-center gap-2">
            {done ? (
              <div className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium bg-green-500/20 text-green-300 border border-green-500/30">
                <svg className="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                  <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
                </svg>
                Launched
              </div>
            ) : (
              <button
                onClick={() => {
                  onExecute(item.id);
                  setExecuted((prev) => new Set(prev).add(item.id));
                }}
                className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium bg-launcher-accent/20 text-launcher-accent border border-launcher-accent/30 hover:bg-launcher-accent/30 hover:text-launcher-accent transition-colors"
                title={`Run: ${item.title}`}
              >
                <svg className="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                  <path strokeLinecap="round" strokeLinejoin="round" d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z" />
                  <path strokeLinecap="round" strokeLinejoin="round" d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                </svg>
                Run "{item.title}"
              </button>
            )}
          </div>
        );
      })}
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
  newlyCreatedItems?: LaunchItem[];
  onExecuteItem?: (itemId: string) => void;
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
  newlyCreatedItems,
  onExecuteItem,
  onReplaceSelection,
}: AgentResponseProps) {
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [thread, thoughts, isThinking, turnActive, permissionRequest]);

  return (
    <div className="flex flex-col flex-1 min-h-0">
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

      <div className="agent-response" ref={scrollRef}>
        {thread.map((entry, index) => {
          if (entry.role === "assistant" && entry.content.length === 0) {
            return null;
          }

          // Tool call entries render as compact status lines
          if (entry.role === "tool") {
            const statusClass = entry.toolStatus
              ? `tool-call-status-${entry.toolStatus}`
              : "tool-call-status-running";
            const statusLabel =
              entry.toolStatus === "completed"
                ? "DONE"
                : (entry.toolStatus ?? "running").toUpperCase();
            return (
              <div key={entry.id} className="tool-call-entry">
                <span className="tool-call-title">
                  {entry.toolTitle ?? "Tool"}
                </span>
                {entry.commandPreview && (
                  <span
                    className="tool-call-command"
                    title={entry.commandPreview}
                  >
                    {entry.commandPreview}
                  </span>
                )}
                <span className={`tool-call-status ${statusClass}`}>
                  {statusLabel}
                </span>
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
          const hasNewItems = newlyCreatedItems && newlyCreatedItems.length > 0;
          const showExecute =
            isLastNonEmptyAssistant &&
            !turnActive &&
            hasNewItems &&
            onExecuteItem;
          const showReplace =
            isLastNonEmptyAssistant &&
            !turnActive &&
            !hasNewItems &&
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
                <ExecuteItemAction
                  items={newlyCreatedItems!}
                  onExecute={onExecuteItem!}
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

        {permissionRequest && (
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
