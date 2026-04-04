import { useState, useEffect, useRef, useCallback } from "react";

interface ContextMenuProps {
  x: number;
  y: number;
  onClose: () => void;
  items: ContextMenuItem[];
}

export interface ContextMenuItem {
  label: string;
  icon?: string;
  danger?: boolean;
  onClick: () => void;
}

function ContextMenu({ x, y, onClose, items }: ContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState({ x, y });

  useEffect(() => {
    // Adjust position if menu would overflow the window
    const menu = menuRef.current;
    if (!menu) return;
    const rect = menu.getBoundingClientRect();
    const adjustedX = x + rect.width > window.innerWidth ? x - rect.width : x;
    const adjustedY = y + rect.height > window.innerHeight ? y - rect.height : y;
    setPosition({ x: Math.max(0, adjustedX), y: Math.max(0, adjustedY) });
  }, [x, y]);

  useEffect(() => {
    const handleClick = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("mousedown", handleClick);
    document.addEventListener("keydown", handleKey);
    return () => {
      document.removeEventListener("mousedown", handleClick);
      document.removeEventListener("keydown", handleKey);
    };
  }, [onClose]);

  return (
    <div
      ref={menuRef}
      className="fixed z-50 min-w-[160px] py-1 rounded-lg bg-launcher-surface border border-launcher-border/60 shadow-xl animate-fade-in"
      style={{ left: position.x, top: position.y }}
    >
      {items.map((item, index) => (
        <button
          key={index}
          onClick={() => {
            item.onClick();
            onClose();
          }}
          className={`w-full text-left px-3 py-1.5 text-sm flex items-center gap-2 transition-colors ${
            item.danger
              ? "text-red-400 hover:bg-red-500/20"
              : "text-launcher-text hover:bg-launcher-hover/60"
          }`}
        >
          {item.icon && <span className="text-xs">{item.icon}</span>}
          {item.label}
        </button>
      ))}
    </div>
  );
}

export default ContextMenu;

export function useContextMenu() {
  const [state, setState] = useState<{ x: number; y: number; data: unknown } | null>(null);

  const open = useCallback((e: React.MouseEvent, data?: unknown) => {
    e.preventDefault();
    e.stopPropagation();
    setState({ x: e.clientX, y: e.clientY, data });
  }, []);

  const close = useCallback(() => setState(null), []);

  return { state, open, close };
}
