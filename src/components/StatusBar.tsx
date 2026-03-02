interface StatusBarProps {
  itemCount: number;
  agentMode: boolean;
  agentTurnActive: boolean;
  hasSuggestions?: boolean;
}

function StatusBar({
  itemCount,
  agentMode,
  agentTurnActive,
  hasSuggestions,
}: StatusBarProps) {
  return (
    <div data-testid="status-bar" className="flex items-center justify-between px-4 py-2 border-t border-launcher-border/30 text-[11px] text-launcher-muted/50">
      <span data-testid="status-text">
        {hasSuggestions
          ? "Suggestions"
          : agentMode
            ? "Agent mode"
            : `${itemCount} ${itemCount === 1 ? "item" : "items"}`}
      </span>
      <div className="flex items-center gap-3">
        {hasSuggestions ? (
          <>
            <span>
              <kbd className="px-1 py-0.5 rounded bg-launcher-surface/50 border border-launcher-border/20">
                ↑↓
              </kbd>{" "}
              suggest
            </span>
            <span>
              <kbd className="px-1 py-0.5 rounded bg-launcher-surface/50 border border-launcher-border/20">
                esc
              </kbd>{" "}
              clear
            </span>
          </>
        ) : agentMode || agentTurnActive ? (
          <>
            <span>
              <kbd className="px-1 py-0.5 rounded bg-launcher-surface/50 border border-launcher-border/20">
                ↵
              </kbd>{" "}
              ask agent
            </span>
            <span>
              <kbd className="px-1 py-0.5 rounded bg-launcher-surface/50 border border-launcher-border/20">
                esc
              </kbd>{" "}
              {agentTurnActive ? "cancel" : "clear"}
            </span>
          </>
        ) : (
          <>
            <span>
              <kbd className="px-1 py-0.5 rounded bg-launcher-surface/50 border border-launcher-border/20">
                ↑↓
              </kbd>{" "}
              navigate
            </span>
            <span>
              <kbd className="px-1 py-0.5 rounded bg-launcher-surface/50 border border-launcher-border/20">
                ↵
              </kbd>{" "}
              open
            </span>
            <span>
              <kbd className="px-1 py-0.5 rounded bg-launcher-surface/50 border border-launcher-border/20">
                tab
              </kbd>{" "}
              category
            </span>
            <span>
              <kbd className="px-1 py-0.5 rounded bg-launcher-surface/50 border border-launcher-border/20">
                esc
              </kbd>{" "}
              close
            </span>
          </>
        )}
      </div>
    </div>
  );
}

export default StatusBar;
