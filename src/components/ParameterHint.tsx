import type { SlashCommandParam } from "../types";

interface ParameterHintProps {
  commandName: string;
  params: SlashCommandParam[];
  currentIndex: number;
}

function ParameterHint({ commandName, params, currentIndex }: ParameterHintProps) {
  if (params.length === 0) return null;

  const clampedIndex = Math.min(currentIndex, params.length - 1);
  const currentParam = params[clampedIndex];

  return (
    <div className="px-4 py-1.5 border-b border-launcher-border/30 bg-launcher-surface/60">
      <div className="flex items-center gap-0 text-xs font-mono">
        <span className="text-launcher-muted">/{commandName}(</span>
        {params.map((param, i) => (
          <span key={param.id}>
            {i > 0 && <span className="text-launcher-muted">, </span>}
            <span
              className={
                i === clampedIndex
                  ? "text-launcher-accent font-bold"
                  : "text-launcher-muted"
              }
            >
              {param.name}
              {!param.required && "?"}
            </span>
          </span>
        ))}
        <span className="text-launcher-muted">)</span>
      </div>
      {currentParam && (
        <div className="text-[11px] text-launcher-muted/80 mt-0.5 ml-1">
          <span className="text-launcher-text/70 font-medium">{currentParam.name}</span>
          {currentParam.description && (
            <span> — {currentParam.description}</span>
          )}
        </div>
      )}
    </div>
  );
}

export default ParameterHint;
