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

export function AgentStatusIndicator({ status, onClick }: AgentStatusIndicatorProps) {
  if (status === "disconnected") return null;

  const clickable = onClick && status === "connected";

  return (
    <button
      className={`ml-2 p-1 rounded transition-colors ${
        clickable
          ? "hover:bg-launcher-hover cursor-pointer"
          : "cursor-default"
      }`}
      title={status === "connected" ? "Enter agent mode" : `Agent: ${status}`}
      onClick={clickable ? onClick : undefined}
      tabIndex={clickable ? 0 : -1}
      type="button"
    >
      <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="16" height="16">
        <defs>
          <linearGradient id="screenGrad" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stopColor="#0f172a"/>
            <stop offset="100%" stopColor="#1e293b"/>
          </linearGradient>
        </defs>

        <line x1="50" y1="20" x2="50" y2="8" stroke="#64748b" strokeWidth="4" strokeLinecap="round"/>
        <circle cx="50" cy="6" r="4" fill={STATUS_COLORS[status]} />

        <rect x="15" y="22" width="70" height="56" rx="10" fill="#cbd5e1" stroke="#475569" strokeWidth="3" />

        <rect x="22" y="29" width="56" height="40" rx="4" fill="url(#screenGrad)" />

        <text x="50" y="56" fontFamily="monospace" fontSize="22" fill="#38bdf8" fontWeight="bold" textAnchor="middle">&lt;/&gt;</text>

        <path d="M40 78 L60 78 L65 90 L35 90 Z" fill="#64748b" />
        <line x1="30" y1="92" x2="70" y2="92" stroke="#475569" strokeWidth="4" strokeLinecap="round"/>
      </svg>
    </button>
  );
}
