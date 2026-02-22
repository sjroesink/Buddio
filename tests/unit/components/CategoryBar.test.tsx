import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import CategoryBar from "../../../src/components/CategoryBar";

describe("CategoryBar", () => {
  const defaultProps = {
    categories: ["Web", "Dev", "Tools"],
    activeCategory: null as string | null,
    onCategoryChange: vi.fn(),
  };

  it("renders all category buttons plus All", () => {
    render(<CategoryBar {...defaultProps} />);
    expect(screen.getByTestId("category-bar")).toBeInTheDocument();
    expect(screen.getByTestId("category-all")).toBeInTheDocument();
    expect(screen.getByTestId("category-Web")).toBeInTheDocument();
    expect(screen.getByTestId("category-Dev")).toBeInTheDocument();
    expect(screen.getByTestId("category-Tools")).toBeInTheDocument();
  });

  it("clicking All sets category to null", async () => {
    const onCategoryChange = vi.fn();
    render(
      <CategoryBar {...defaultProps} onCategoryChange={onCategoryChange} />,
    );
    await userEvent.click(screen.getByTestId("category-all"));
    expect(onCategoryChange).toHaveBeenCalledWith(null);
  });

  it("clicking a category selects it", async () => {
    const onCategoryChange = vi.fn();
    render(
      <CategoryBar {...defaultProps} onCategoryChange={onCategoryChange} />,
    );
    await userEvent.click(screen.getByTestId("category-Web"));
    expect(onCategoryChange).toHaveBeenCalledWith("Web");
  });

  it("clicking the active category deselects it", async () => {
    const onCategoryChange = vi.fn();
    render(
      <CategoryBar
        {...defaultProps}
        activeCategory="Web"
        onCategoryChange={onCategoryChange}
      />,
    );
    await userEvent.click(screen.getByTestId("category-Web"));
    expect(onCategoryChange).toHaveBeenCalledWith(null);
  });
});
