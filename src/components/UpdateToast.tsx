import type { UpdateState } from "../hooks/useUpdateChecker";

interface UpdateToastProps {
  state: UpdateState;
  onInstall: () => void;
  onRestart: () => void;
  onDismiss: () => void;
}

export function UpdateToast({ state, onInstall, onRestart, onDismiss }: UpdateToastProps) {
  if (state.status === "idle") return null;

  const progressPercent =
    state.status === "downloading" && state.total > 0
      ? Math.round((state.progress / state.total) * 100)
      : 0;

  return (
    <div className="w-full px-3 py-2 bg-blue-500/15 border-b border-blue-500/30 text-blue-300 text-xs">
      {state.status === "available" && (
        <div className="flex items-center justify-between gap-2">
          <span>Update v{state.version} beschikbaar</span>
          <div className="flex items-center gap-1.5">
            <button
              onClick={onInstall}
              className="px-2 py-0.5 rounded bg-blue-500/30 hover:bg-blue-500/50 transition-colors font-medium"
            >
              Installeren
            </button>
            <button
              onClick={onDismiss}
              className="px-2 py-0.5 rounded bg-white/5 hover:bg-white/10 transition-colors"
            >
              Later
            </button>
          </div>
        </div>
      )}

      {state.status === "downloading" && (
        <div className="flex flex-col gap-1">
          <div className="flex items-center justify-between">
            <span>Downloaden... {progressPercent}%</span>
          </div>
          <div className="w-full h-1 bg-blue-500/20 rounded-full overflow-hidden">
            <div
              className="h-full bg-blue-400 rounded-full transition-all duration-300"
              style={{ width: `${progressPercent}%` }}
            />
          </div>
        </div>
      )}

      {state.status === "ready" && (
        <div className="flex items-center justify-between gap-2">
          <span>Update klaar om te installeren</span>
          <button
            onClick={onRestart}
            className="px-2 py-0.5 rounded bg-green-500/30 hover:bg-green-500/50 text-green-300 transition-colors font-medium"
          >
            Herstarten
          </button>
        </div>
      )}
    </div>
  );
}
