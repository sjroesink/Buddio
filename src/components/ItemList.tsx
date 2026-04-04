import { useRef, useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { LaunchItem } from "../types";
import ItemRow from "./ItemRow";
import ContextMenu, { useContextMenu } from "./ContextMenu";
import ConfirmDialog from "./ConfirmDialog";

interface ItemListProps {
  items: LaunchItem[];
  selectedIndex: number;
  onSelect: (index: number) => void;
  onExecute: () => void;
  onItemDeleted?: () => void;
}

function ItemList({ items, selectedIndex, onSelect, onExecute, onItemDeleted }: ItemListProps) {
  const listRef = useRef<HTMLDivElement>(null);
  const contextMenu = useContextMenu();
  const [confirmDelete, setConfirmDelete] = useState<LaunchItem | null>(null);

  useEffect(() => {
    const selected = listRef.current?.children[selectedIndex] as HTMLElement;
    selected?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }, [selectedIndex]);

  const handleDelete = useCallback(async (item: LaunchItem) => {
    try {
      await invoke<boolean>("delete_item", { id: item.id });
      setConfirmDelete(null);
      onItemDeleted?.();
    } catch (err) {
      console.error("Failed to delete item:", err);
    }
  }, [onItemDeleted]);

  if (items.length === 0) {
    return (
      <div data-testid="item-list-empty" className="flex-1 flex items-center justify-center text-launcher-muted/50">
        <div className="text-center">
          <svg
            className="w-12 h-12 mx-auto mb-3 opacity-30"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            strokeWidth={1}
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M20 13V6a2 2 0 00-2-2H6a2 2 0 00-2 2v7m16 0v5a2 2 0 01-2 2H6a2 2 0 01-2-2v-5m16 0h-2.586a1 1 0 00-.707.293l-2.414 2.414a1 1 0 01-.707.293h-3.172a1 1 0 01-.707-.293l-2.414-2.414A1 1 0 006.586 13H4"
            />
          </svg>
          <p className="text-sm">No items found</p>
          <p className="text-xs mt-1 opacity-60">
            Use buddio-cli to add items
          </p>
        </div>
      </div>
    );
  }

  return (
    <>
      <div data-testid="item-list" ref={listRef} className="flex-1 overflow-y-auto py-1">
        {items.map((item, index) => (
          <ItemRow
            key={item.id}
            item={item}
            isSelected={index === selectedIndex}
            onHover={() => onSelect(index)}
            onClick={onExecute}
            onContextMenu={(e) => contextMenu.open(e, item)}
          />
        ))}
      </div>

      {contextMenu.state && (
        <ContextMenu
          x={contextMenu.state.x}
          y={contextMenu.state.y}
          onClose={contextMenu.close}
          items={[
            {
              label: "Delete",
              icon: "🗑",
              danger: true,
              onClick: () => setConfirmDelete(contextMenu.state!.data as LaunchItem),
            },
          ]}
        />
      )}

      {confirmDelete && (
        <ConfirmDialog
          title="Delete item"
          message={`Are you sure you want to delete "${confirmDelete.title}"? This cannot be undone.`}
          onConfirm={() => handleDelete(confirmDelete)}
          onCancel={() => setConfirmDelete(null)}
        />
      )}
    </>
  );
}

export default ItemList;
