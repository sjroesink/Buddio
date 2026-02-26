import { useRef, useEffect } from "react";
import { LaunchItem } from "../types";
import ItemRow from "./ItemRow";

interface ItemListProps {
  items: LaunchItem[];
  selectedIndex: number;
  onSelect: (index: number) => void;
  onExecute: () => void;
}

function ItemList({ items, selectedIndex, onSelect, onExecute }: ItemListProps) {
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const selected = listRef.current?.children[selectedIndex] as HTMLElement;
    selected?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }, [selectedIndex]);

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
    <div data-testid="item-list" ref={listRef} className="flex-1 overflow-y-auto py-1">
      {items.map((item, index) => (
        <ItemRow
          key={item.id}
          item={item}
          isSelected={index === selectedIndex}
          onHover={() => onSelect(index)}
          onClick={onExecute}
        />
      ))}
    </div>
  );
}

export default ItemList;
