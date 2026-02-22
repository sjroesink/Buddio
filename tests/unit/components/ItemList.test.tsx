import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import ItemList from "../../../src/components/ItemList";
import type { LaunchItem } from "../../../src/types";

const mockItems: LaunchItem[] = [
  {
    id: "1",
    title: "Google",
    subtitle: "https://google.com",
    icon: null,
    action_type: "url",
    action_value: "https://google.com",
    category: "Web",
    tags: "",
    frequency: 0,
    enabled: true,
    created_at: "",
    updated_at: "",
  },
  {
    id: "2",
    title: "VS Code",
    subtitle: "Open editor",
    icon: null,
    action_type: "command",
    action_value: "code",
    category: "Dev",
    tags: "",
    frequency: 0,
    enabled: true,
    created_at: "",
    updated_at: "",
  },
];

describe("ItemList", () => {
  const defaultProps = {
    items: mockItems,
    selectedIndex: 0,
    onSelect: vi.fn(),
    onExecute: vi.fn(),
  };

  it("renders all items", () => {
    render(<ItemList {...defaultProps} />);
    expect(screen.getByTestId("item-list")).toBeInTheDocument();
    expect(screen.getByTestId("item-row-1")).toBeInTheDocument();
    expect(screen.getByTestId("item-row-2")).toBeInTheDocument();
  });

  it("shows item titles", () => {
    render(<ItemList {...defaultProps} />);
    expect(screen.getByText("Google")).toBeInTheDocument();
    expect(screen.getByText("VS Code")).toBeInTheDocument();
  });

  it("shows empty state when no items", () => {
    render(<ItemList {...defaultProps} items={[]} />);
    expect(screen.getByTestId("item-list-empty")).toBeInTheDocument();
    expect(screen.getByText("No items found")).toBeInTheDocument();
  });

  it("calls onSelect on hover", async () => {
    const onSelect = vi.fn();
    render(<ItemList {...defaultProps} onSelect={onSelect} />);
    await userEvent.hover(screen.getByTestId("item-row-2"));
    expect(onSelect).toHaveBeenCalledWith(1);
  });

  it("calls onExecute on click", async () => {
    const onExecute = vi.fn();
    render(<ItemList {...defaultProps} onExecute={onExecute} />);
    await userEvent.click(screen.getByTestId("item-row-1"));
    expect(onExecute).toHaveBeenCalledOnce();
  });
});
