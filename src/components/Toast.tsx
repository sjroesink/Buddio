import { useEffect } from "react";

export interface ToastData {
  message: string;
  type: "success" | "error" | "info";
  action?: {
    label: string;
    onClick: () => void;
  };
}

interface ToastProps {
  toast: ToastData;
  onDismiss: () => void;
  /** Auto-dismiss delay in ms. Defaults to 2000 for success, no auto-dismiss for error. */
  duration?: number;
}

export function Toast({ toast, onDismiss, duration }: ToastProps) {
  const autoDismiss = duration ?? (toast.type === "success" ? 2000 : 0);

  useEffect(() => {
    if (autoDismiss > 0) {
      const timer = setTimeout(onDismiss, autoDismiss);
      return () => clearTimeout(timer);
    }
  }, [autoDismiss, onDismiss]);

  const isError = toast.type === "error";
  const isInfo = toast.type === "info";

  const colorClasses = isError
    ? "bg-red-500/20 border-red-500/40 text-red-300"
    : isInfo
      ? "bg-blue-500/20 border-blue-500/40 text-blue-300"
      : "bg-green-500/20 border-green-500/40 text-green-300";

  return (
    <div
      className={`fixed bottom-3 left-1/2 -translate-x-1/2 z-50 px-4 py-2 rounded-lg text-xs font-medium shadow-lg border backdrop-blur-sm animate-toast-in ${colorClasses}`}
      onClick={isError ? onDismiss : undefined}
      role={isError ? "alert" : "status"}
    >
      <div className="flex items-center gap-2">
        {isError ? (
          <svg className="w-3.5 h-3.5 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4.5c-.77-.833-2.694-.833-3.464 0L3.34 16.5c-.77.833.192 2.5 1.732 2.5z" />
          </svg>
        ) : isInfo ? (
          <svg className="w-3.5 h-3.5 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
        ) : (
          <svg className="w-3.5 h-3.5 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
          </svg>
        )}
        <span>{toast.message}</span>
        {toast.action && (
          <button
            onClick={(e) => {
              e.stopPropagation();
              toast.action!.onClick();
              onDismiss();
            }}
            className="ml-1 px-2 py-0.5 rounded bg-white/10 hover:bg-white/20 transition-colors"
          >
            {toast.action.label}
          </button>
        )}
        {(isError || isInfo) && (
          <button
            onClick={onDismiss}
            className="ml-1 opacity-60 hover:opacity-100 transition-colors"
          >
            <svg className="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        )}
      </div>
    </div>
  );
}
