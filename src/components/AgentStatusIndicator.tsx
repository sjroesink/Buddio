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
          <rect
            x="14"
            y="32"
            width="14"
            height="30"
            rx="7"
            transform="rotate(24 21 47)"
            fill="#94a3b8"
            stroke="#475569"
            strokeWidth="3"
          />
          <rect
            x="100"
            y="32"
            width="14"
            height="30"
            rx="7"
            transform="rotate(-24 107 47)"
            fill="#94a3b8"
            stroke="#475569"
            strokeWidth="3"
          />

          <line
            x1="64"
            y1="18"
            x2="64"
            y2="8"
            stroke="#475569"
            strokeWidth="3"
            strokeLinecap="round"
          />
          <circle cx="64" cy="7" r="4.5" fill="#38bdf8" stroke="#475569" strokeWidth="2.5" />

          <rect
            x="22"
            y="20"
            width="84"
            height="60"
            rx="24"
            fill="#cbd5e1"
            stroke="#475569"
            strokeWidth="3"
          />
          <rect
            x="33"
            y="30"
            width="62"
            height="34"
            rx="12"
            fill="#0f172a"
            stroke="#1e293b"
            strokeWidth="2"
          />

          <ellipse cx="50" cy="46" rx="6" ry="9" fill="#38bdf8" />
          <ellipse cx="78" cy="46" rx="6" ry="9" fill="#38bdf8" />
          <circle cx="49" cy="43" r="2.2" fill="#ffffff" />
          <circle cx="77" cy="43" r="2.2" fill="#ffffff" />
          <path
            d="M57 58 Q64 64 71 58"
            fill="none"
            stroke="#38bdf8"
            strokeWidth="3"
            strokeLinecap="round"
          />
          <circle cx="41" cy="57" r="3" fill="#f43f5e" opacity="0.65" />
          <circle cx="87" cy="57" r="3" fill="#f43f5e" opacity="0.65" />

          <rect
            x="34"
            y="74"
            width="60"
            height="38"
            rx="16"
            fill="#e2e8f0"
            stroke="#475569"
            strokeWidth="3"
          />
          <rect x="48" y="87" width="32" height="11" rx="4" fill="#1e293b" />
          <rect x="51" y="90" width="5" height="5" rx="1.5" fill="#22c55e" />
          <rect x="59" y="90" width="5" height="5" rx="1.5" fill="#22c55e" />
          <rect x="67" y="90" width="5" height="5" rx="1.5" fill="#22c55e" />
        </svg>
        <span
          className="absolute -right-0.5 -top-0.5 h-2.5 w-2.5 rounded-full border border-launcher-bg"
          style={{ backgroundColor: STATUS_COLORS[status] }}
        />
      </span>
    </button>
  );
}
