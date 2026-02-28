import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import SearchBar from "../../../src/components/SearchBar";

describe("SearchBar", () => {
  const defaultProps = {
    query: "",
    onQueryChange: vi.fn(),
    loading: false,
    agentStatus: "disconnected" as const,
    onSettingsClick: vi.fn(),
  };

  it("renders the search input", () => {
    render(<SearchBar {...defaultProps} />);
    expect(screen.getByTestId("search-input")).toBeInTheDocument();
  });

  it("shows the search placeholder in search mode", () => {
    render(<SearchBar {...defaultProps} mode="search" />);
    expect(screen.getByTestId("search-input")).toHaveAttribute(
      "placeholder",
      "Ask anything...",
    );
  });

  it("shows the composer placeholder in composer mode", () => {
    render(<SearchBar {...defaultProps} mode="composer" />);
    expect(screen.getByTestId("search-input")).toHaveAttribute(
      "placeholder",
      "Message the agent...",
    );
  });

  it("calls onQueryChange when user types", async () => {
    const onQueryChange = vi.fn();
    render(<SearchBar {...defaultProps} onQueryChange={onQueryChange} />);
    const input = screen.getByTestId("search-input");
    await userEvent.type(input, "hello");
    expect(onQueryChange).toHaveBeenCalled();
  });

  it("shows clear button when query is non-empty", () => {
    render(<SearchBar {...defaultProps} query="test" />);
    expect(screen.getByTestId("clear-button")).toBeInTheDocument();
  });

  it("hides clear button when query is empty", () => {
    render(<SearchBar {...defaultProps} query="" />);
    expect(screen.queryByTestId("clear-button")).not.toBeInTheDocument();
  });

  it("calls onQueryChange with empty string when clear is clicked", async () => {
    const onQueryChange = vi.fn();
    render(
      <SearchBar {...defaultProps} query="test" onQueryChange={onQueryChange} />,
    );
    await userEvent.click(screen.getByTestId("clear-button"));
    expect(onQueryChange).toHaveBeenCalledWith("");
  });

  it("renders settings button", async () => {
    const onSettingsClick = vi.fn();
    render(
      <SearchBar {...defaultProps} onSettingsClick={onSettingsClick} />,
    );
    await userEvent.click(screen.getByTestId("settings-button"));
    expect(onSettingsClick).toHaveBeenCalledOnce();
  });

  it("shows back button in composer mode", () => {
    const onBackClick = vi.fn();
    render(
      <SearchBar
        {...defaultProps}
        mode="composer"
        onBackClick={onBackClick}
      />,
    );
    expect(screen.getByTestId("back-button")).toBeInTheDocument();
  });

  it("does not show back button in search mode", () => {
    render(<SearchBar {...defaultProps} mode="search" />);
    expect(screen.queryByTestId("back-button")).not.toBeInTheDocument();
  });
});
