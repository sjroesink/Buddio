import { useId } from "react";
import type { AgentStatus } from "../types";

interface AgentStatusIndicatorProps {
  status: AgentStatus;
  onClick?: () => void;
}

const STATUS_COLORS: Record<AgentStatus, string> = {
  connected: "#4ade80",
  connecting: "#facc15",
  disconnected: "#9ca3af",
  error: "#f87171",
};

export function AgentStatusIndicator({
  status,
  onClick,
}: AgentStatusIndicatorProps) {
  const clickable = !!onClick;
  const gradientId = `dog-grad-${useId().replace(/:/g, "")}`;

  return (
    <button
      className={`ml-2 rounded p-1 transition-colors ${
        clickable ? "cursor-pointer hover:bg-launcher-hover" : "cursor-default"
      }`}
      title={clickable ? "Enter agent mode" : `Agent: ${status}`}
      onClick={clickable ? onClick : undefined}
      tabIndex={clickable ? 0 : -1}
      type="button"
    >
      <span className="relative block h-5 w-5">
        <svg
          xmlns="http://www.w3.org/2000/svg"
          viewBox="0 0 128 128"
          className="h-5 w-5"
          aria-hidden="true"
        >
          <defs>
            <linearGradient id={gradientId} x1="0%" y1="0%" x2="100%" y2="100%">
              <stop offset="0%" stopColor="#F59E0B" />
              <stop offset="100%" stopColor="#EF4444" />
            </linearGradient>
          </defs>
          <path d="M 30 55 L 12 18 L 52 38 Z" fill={`url(#${gradientId})`} />
          <path d="M 98 55 L 116 18 L 76 38 Z" fill={`url(#${gradientId})`} />
          <path
            d="M 30 55 C 30 20 98 20 98 55 L 110 85 C 110 115 18 115 18 85 Z"
            fill={`url(#${gradientId})`}
          />
          <circle cx="48" cy="66" r="9" fill="#FFFFFF" />
          <circle cx="80" cy="66" r="9" fill="#FFFFFF" />
          <circle cx="64" cy="88" r="10" fill="#1E293B" />
          <path
            d="M 48 104 Q 64 112 80 104"
            stroke="#FFFFFF"
            strokeWidth="6"
            strokeLinecap="round"
            fill="none"
          />
        </svg>
        <span
          className="absolute -right-0.5 -top-0.5 h-2.5 w-2.5 rounded-full border border-launcher-bg"
          style={{ backgroundColor: STATUS_COLORS[status] }}
        />
      </span>
    </button>
  );
}
