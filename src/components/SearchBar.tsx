import { useRef, useEffect, useState } from "react";
import { AgentStatusIndicator } from "./AgentStatusIndicator";
import type { AgentStatus } from "../types";

interface ContextInfo {
  hasSelection: boolean;
  hasClipboard: boolean;
  sourceApp: string | null;
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
}

function ContextDropdown({ contextInfo }: { contextInfo: ContextInfo }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  // Close on click outside
  useEffect(() => {
    if (!open) return;
    function handleClick(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [open]);

  const truncate = (text: string, max: number) =>
    text.length > max ? text.slice(0, max) + "…" : text;

  const badgeColor = contextInfo.hasSelection
    ? "bg-purple-500/20 text-purple-300 border-purple-500/30"
    : "bg-blue-500/20 text-blue-300 border-blue-500/30";

  return (
    <div className="relative mr-2" ref={ref}>
      <button
        onClick={() => setOpen((p) => !p)}
        className={`inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium border transition-colors ${badgeColor} hover:brightness-125`}
        title="Show captured context"
      >
        {contextInfo.hasSelection ? (
          <>
            <svg className="w-2.5 h-2.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M4 6h16M4 12h8" />
            </svg>
            SEL
          </>
        ) : (
          <>
            <svg className="w-2.5 h-2.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2" />
            </svg>
            CLIP
          </>
        )}
        <svg
          className={`w-2.5 h-2.5 transition-transform ${open ? "rotate-180" : ""}`}
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
          strokeWidth={2}
        >
          <path strokeLinecap="round" strokeLinejoin="round" d="M19 9l-7 7-7-7" />
        </svg>
      </button>

      {open && (
        <div className="absolute bottom-full left-0 mb-1 w-72 rounded-lg bg-launcher-bg border border-launcher-border/50 shadow-xl z-50 overflow-hidden">
          {contextInfo.sourceApp && (
            <div className="px-3 py-1.5 border-b border-launcher-border/30">
              <div className="text-[10px] text-launcher-muted uppercase tracking-wider">Source</div>
              <div className="text-xs text-launcher-text truncate">{contextInfo.sourceApp}</div>
            </div>
          )}
          {contextInfo.hasSelection && contextInfo.selectedText && (
            <div className="px-3 py-1.5 border-b border-launcher-border/30">
              <div className="text-[10px] text-purple-400/70 uppercase tracking-wider">Selected text</div>
              <div className="text-xs text-launcher-text/80 whitespace-pre-wrap break-words max-h-32 overflow-y-auto leading-relaxed">
                {truncate(contextInfo.selectedText, 500)}
              </div>
            </div>
          )}
          {contextInfo.hasClipboard && contextInfo.clipboardText && (
            <div className="px-3 py-1.5">
              <div className="text-[10px] text-blue-400/70 uppercase tracking-wider">Clipboard</div>
              <div className="text-xs text-launcher-text/80 whitespace-pre-wrap break-words max-h-32 overflow-y-auto leading-relaxed">
                {truncate(contextInfo.clipboardText, 500)}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
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
      <div className="flex items-center justify-center w-8 h-8 mr-3 window-drag-region">
        {loading && !isComposer ? (
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
        ) : isComposer ? (
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
              d="M8 10h8M8 14h5m-1 8h8a2 2 0 002-2V8a2 2 0 00-2-2h-8l-4 4v10a2 2 0 002 2h2z"
            />
          </svg>
        ) : (
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
        )}
      </div>
      {contextInfo && (contextInfo.hasSelection || contextInfo.hasClipboard) && (
        <ContextDropdown contextInfo={contextInfo} />
      )}
      <input
        data-testid="search-input"
        ref={inputRef}
        type="text"
        value={query}
        onChange={(e) => onQueryChange(e.target.value)}
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
