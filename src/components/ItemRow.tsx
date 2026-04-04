import { LaunchItem } from "../types";

interface ItemRowProps {
  item: LaunchItem;
  isSelected: boolean;
  onHover: () => void;
  onClick: () => void;
  onContextMenu: (e: React.MouseEvent) => void;
}

const ACTION_TYPE_ICONS: Record<string, string> = {
  url: "🌐",
  command: "⚡",
  script: "📜",
};

function ItemRow({ item, isSelected, onHover, onClick, onContextMenu }: ItemRowProps) {
  const icon = item.icon || ACTION_TYPE_ICONS[item.action_type] || "📦";

  return (
    <div
      data-testid={`item-row-${item.id}`}
      onMouseEnter={onHover}
      onClick={onClick}
      onContextMenu={onContextMenu}
      className={`window-no-drag flex items-center px-4 py-2.5 mx-1 rounded-lg cursor-pointer transition-all duration-100 ${
        isSelected
          ? "bg-launcher-selected/80 border border-launcher-border/40"
          : "border border-transparent hover:bg-launcher-hover/50"
      }`}
    >
      <div className="flex items-center justify-center w-9 h-9 rounded-lg bg-launcher-surface/80 text-lg mr-3 flex-shrink-0">
        {icon}
      </div>

      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <span
            className={`text-sm font-medium truncate ${
              isSelected ? "text-white" : "text-launcher-text"
            }`}
          >
            {item.title}
          </span>
          <span className="text-[10px] px-1.5 py-0.5 rounded bg-launcher-surface/60 text-launcher-muted flex-shrink-0">
            {item.category}
          </span>
        </div>
        {item.subtitle && (
          <p className="text-xs text-launcher-muted/70 truncate mt-0.5">
            {item.subtitle}
          </p>
        )}
      </div>

      <div className="flex items-center gap-2 ml-2 flex-shrink-0">
        <span className="text-[10px] text-launcher-muted/40">
          {item.action_type}
        </span>
        {isSelected && (
          <div className="flex items-center gap-1">
            <kbd className="text-[10px] px-1.5 py-0.5 rounded bg-launcher-surface text-launcher-muted border border-launcher-border/30">
              ↵
            </kbd>
          </div>
        )}
      </div>
    </div>
  );
}

export default ItemRow;

