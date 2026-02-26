import { useRef, useEffect, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import { AgentStatusIndicator } from "./AgentStatusIndicator";
import type { AgentStatus } from "../types";

export interface PastedImage {
  id: string;
  dataUrl: string;
  name: string;
}

interface ContextInfo {
  hasSelection: boolean;
  hasClipboard: boolean;
  sourceApp: string | null;
  sourceProcessName: string | null;
  selectedText?: string | null;
  clipboardText?: string | null;
}

interface SearchBarProps {
  query: string;
  onQueryChange: (query: string) => void;
  loading: boolean;
  agentStatus: AgentStatus;
  onSettingsClick: () => void;
  onBackClick?: () => void;
  onAgentClick?: () => void;
  mode?: "search" | "composer";
  position?: "top" | "bottom";
  focusSignal?: number;
  onInputFocus?: () => void;
  contextInfo?: ContextInfo;
  contextCount?: number;
  contextPanelOpen?: boolean;
  onContextClick?: () => void;
  onAddImage?: (image: PastedImage) => void;
}

function SearchBar({
  query,
  onQueryChange,
  loading,
  agentStatus,
  onSettingsClick,
  onBackClick,
  onAgentClick,
  mode = "search",
  position = "top",
  focusSignal,
  onInputFocus,
  contextInfo,
  contextCount = 0,
  contextPanelOpen = false,
  onContextClick,
  onAddImage,
}: SearchBarProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const isComposer = mode === "composer";

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    inputRef.current?.focus();
  }, [mode]);

  useEffect(() => {
    if (focusSignal !== undefined) {
      inputRef.current?.focus();
    }
  }, [focusSignal]);

  // Re-focus input whenever the window gains focus
  useEffect(() => {
    const unlisten = listen("tauri://focus", () => {
      inputRef.current?.focus();
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Handle paste events for image detection
  const handlePaste = useCallback(
    (e: React.ClipboardEvent) => {
      if (!onAddImage) return;
      const items = e.clipboardData?.items;
      if (!items) return;

      for (let i = 0; i < items.length; i++) {
        const item = items[i];
        if (item.type.startsWith("image/")) {
          e.preventDefault();
          const blob = item.getAsFile();
          if (!blob) continue;

          const reader = new FileReader();
          reader.onload = () => {
            const dataUrl = reader.result as string;
            onAddImage({
              id: crypto.randomUUID(),
              dataUrl,
              name: `Image ${new Date().toLocaleTimeString()}`,
            });
          };
          reader.readAsDataURL(blob);
          return;
        }
      }
    },
    [onAddImage],
  );

  return (
    <div
      data-testid="search-bar"
      className={`flex items-center px-4 py-3 border-launcher-border/30 ${
        position === "bottom" ? "border-t" : "border-b"
      }`}
    >
      {isComposer && onBackClick && (
        <button
          data-testid="back-button"
          onClick={onBackClick}
          className="mr-2 p-1 rounded text-launcher-muted hover:text-launcher-text hover:bg-launcher-hover transition-colors"
          title="Back to search (Esc)"
          aria-label="Back to search"
        >
          <svg
            className="w-4 h-4"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            strokeWidth={2}
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M15 19l-7-7 7-7"
            />
          </svg>
        </button>
      )}
      <div className="flex items-center justify-center mr-3">
        {loading && !isComposer ? (
          <div className="flex items-center justify-center w-8 h-8 window-drag-region">
            <svg
              className="w-5 h-5 text-launcher-accent animate-spin"
              viewBox="0 0 24 24"
              fill="none"
            >
              <circle
                cx="12"
                cy="12"
                r="10"
                stroke="currentColor"
                strokeWidth="3"
                strokeLinecap="round"
                className="opacity-25"
              />
              <path
                d="M12 2a10 10 0 0 1 10 10"
                stroke="currentColor"
                strokeWidth="3"
                strokeLinecap="round"
              />
            </svg>
          </div>
        ) : isComposer ? (
          <button
            onClick={onContextClick}
            className={`relative flex items-center justify-center w-8 h-8 rounded transition-colors ${
              contextCount > 0
                ? "text-launcher-text cursor-pointer hover:bg-launcher-hover"
                : "text-launcher-muted cursor-default"
            } ${contextPanelOpen ? "bg-launcher-hover" : ""}`}
            title={
              contextCount > 0
                ? `${contextCount} context item${contextCount > 1 ? "s" : ""} (click to view)`
                : "Composer"
            }
          >
            {/* Compose icon */}
            <svg
              className="w-5 h-5"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth={2}
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M8 10h8M8 14h5m-1 8h8a2 2 0 002-2V8a2 2 0 00-2-2h-8l-4 4v10a2 2 0 002 2h2z"
              />
            </svg>
            {/* Count badge */}
            {contextCount > 0 && (
              <span className="absolute -top-0.5 -right-0.5 flex items-center justify-center w-3.5 h-3.5 rounded-full bg-purple-500 text-white text-[8px] font-bold leading-none">
                {contextCount}
              </span>
            )}
          </button>
        ) : (
          <div className="flex items-center justify-center w-8 h-8 window-drag-region">
            <svg
              className="w-5 h-5 text-launcher-muted"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth={2}
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
              />
            </svg>
          </div>
        )}
      </div>
      <input
        data-testid="search-input"
        ref={inputRef}
        type="text"
        value={query}
        onChange={(e) => onQueryChange(e.target.value)}
        onPaste={handlePaste}
        onFocus={onInputFocus}
        placeholder={
          isComposer
            ? contextInfo?.hasSelection
              ? "Ask about selection... (e.g. rewrite this)"
              : "Message the agent..."
            : "Search commands, apps, URLs..."
        }
        className="flex-1 bg-transparent text-launcher-text text-lg placeholder-launcher-muted/60 outline-none"
        spellCheck={false}
        autoComplete="off"
      />
      <AgentStatusIndicator status={agentStatus} onClick={onAgentClick} />
      {query && (
        <button
          data-testid="clear-button"
          onClick={() => onQueryChange("")}
          className="ml-2 p-1 rounded text-launcher-muted hover:text-launcher-text hover:bg-launcher-hover transition-colors"
        >
          <svg
            className="w-4 h-4"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            strokeWidth={2}
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M6 18L18 6M6 6l12 12"
            />
          </svg>
        </button>
      )}
      <button
        data-testid="settings-button"
        onClick={onSettingsClick}
        className="ml-2 p-1 rounded text-launcher-muted hover:text-launcher-text hover:bg-launcher-hover transition-colors"
        title="Agent Settings (Ctrl+,)"
      >
        <svg
          className="w-4 h-4"
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
          strokeWidth={2}
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.066 2.573c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.573 1.066c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.066-2.573c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"
          />
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
          />
        </svg>
      </button>
    </div>
  );
}

export default SearchBar;
