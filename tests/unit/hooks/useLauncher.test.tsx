import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";
import "../helpers/mockTauri";
import {
  resetInvokeHandler,
  MOCK_ITEMS,
  MOCK_CATEGORIES,
} from "../helpers/mockTauri";
import { useLauncher } from "../../../src/hooks/useLauncher";
import React from "react";

const defaultOptions = {
  agentStatus: "disconnected" as const,
  agentAutoFallback: false,
  onAgentPrompt: vi.fn(),
  onAgentCancel: vi.fn(),
  agentTurnActive: false,
};

describe("useLauncher", () => {
  beforeEach(() => {
    resetInvokeHandler();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("loads items and categories on mount", async () => {
    vi.useRealTimers();
    const { result } = renderHook(() => useLauncher(defaultOptions));

    await waitFor(() => {
      expect(result.current.items.length).toBe(MOCK_ITEMS.length);
    });
    expect(result.current.categories).toEqual(MOCK_CATEGORIES);
  });

  it("filters items by query after debounce", async () => {
    vi.useRealTimers();
    const { result } = renderHook(() => useLauncher(defaultOptions));

    await waitFor(() => {
      expect(result.current.items.length).toBe(MOCK_ITEMS.length);
    });

    act(() => {
      result.current.setQuery("google");
    });

    await waitFor(() => {
      expect(result.current.items.length).toBe(1);
      expect(result.current.items[0].title).toBe("Google");
    });
  });

  it("navigates items with arrow key handler", async () => {
    vi.useRealTimers();
    const { result } = renderHook(() => useLauncher(defaultOptions));

    await waitFor(() => {
      expect(result.current.items.length).toBe(MOCK_ITEMS.length);
    });

    expect(result.current.selectedIndex).toBe(0);

    act(() => {
      result.current.handleKeyDown({
        key: "ArrowDown",
        preventDefault: vi.fn(),
      } as unknown as React.KeyboardEvent);
    });

    expect(result.current.selectedIndex).toBe(1);

    act(() => {
      result.current.handleKeyDown({
        key: "ArrowUp",
        preventDefault: vi.fn(),
      } as unknown as React.KeyboardEvent);
    });

    expect(result.current.selectedIndex).toBe(0);
  });

  it("wraps around on arrow navigation", async () => {
    vi.useRealTimers();
    const { result } = renderHook(() => useLauncher(defaultOptions));

    await waitFor(() => {
      expect(result.current.items.length).toBe(MOCK_ITEMS.length);
    });

    // Go up from 0 should wrap to last
    act(() => {
      result.current.handleKeyDown({
        key: "ArrowUp",
        preventDefault: vi.fn(),
      } as unknown as React.KeyboardEvent);
    });

    expect(result.current.selectedIndex).toBe(MOCK_ITEMS.length - 1);
  });

  it("cycles categories with Tab", async () => {
    vi.useRealTimers();
    const { result } = renderHook(() => useLauncher(defaultOptions));

    await waitFor(() => {
      expect(result.current.categories.length).toBe(MOCK_CATEGORIES.length);
    });

    expect(result.current.activeCategory).toBeNull();

    act(() => {
      result.current.handleKeyDown({
        key: "Tab",
        shiftKey: false,
        preventDefault: vi.fn(),
      } as unknown as React.KeyboardEvent);
    });

    expect(result.current.activeCategory).toBe(MOCK_CATEGORIES[0]);

    act(() => {
      result.current.handleKeyDown({
        key: "Tab",
        shiftKey: false,
        preventDefault: vi.fn(),
      } as unknown as React.KeyboardEvent);
    });

    expect(result.current.activeCategory).toBe(MOCK_CATEGORIES[1]);

    // Tab past last goes back to null (All)
    act(() => {
      result.current.handleKeyDown({
        key: "Tab",
        shiftKey: false,
        preventDefault: vi.fn(),
      } as unknown as React.KeyboardEvent);
    });

    expect(result.current.activeCategory).toBeNull();
  });

  it("clears query on Escape", async () => {
    vi.useRealTimers();
    const { result } = renderHook(() => useLauncher(defaultOptions));

    await waitFor(() => {
      expect(result.current.items.length).toBe(MOCK_ITEMS.length);
    });

    act(() => {
      result.current.setQuery("test");
    });

    act(() => {
      result.current.handleKeyDown({
        key: "Escape",
        preventDefault: vi.fn(),
      } as unknown as React.KeyboardEvent);
    });

    expect(result.current.query).toBe("");
  });

  it("enters agent mode when conditions are met", async () => {
    vi.useRealTimers();
    const { result } = renderHook(() =>
      useLauncher({
        ...defaultOptions,
        agentStatus: "connected",
        agentAutoFallback: true,
      }),
    );

    await waitFor(() => {
      expect(result.current.items.length).toBe(MOCK_ITEMS.length);
    });

    // Type a query that doesn't match anything
    act(() => {
      result.current.setQuery("xyznoitemmatches");
    });

    await waitFor(() => {
      expect(result.current.agentMode).toBe(true);
    });
  });

  it("triggers agent prompt on Enter in agent mode", async () => {
    vi.useRealTimers();
    const onAgentPrompt = vi.fn();
    const { result } = renderHook(() =>
      useLauncher({
        ...defaultOptions,
        agentStatus: "connected",
        agentAutoFallback: true,
        onAgentPrompt,
      }),
    );

    await waitFor(() => {
      expect(result.current.items.length).toBe(MOCK_ITEMS.length);
    });

    act(() => {
      result.current.setQuery("xyznoitemmatches");
    });

    await waitFor(() => {
      expect(result.current.agentMode).toBe(true);
    });

    act(() => {
      result.current.handleKeyDown({
        key: "Enter",
        preventDefault: vi.fn(),
      } as unknown as React.KeyboardEvent);
    });

    expect(onAgentPrompt).toHaveBeenCalledWith("xyznoitemmatches");
  });

  it("resets all state", async () => {
    vi.useRealTimers();
    const { result } = renderHook(() => useLauncher(defaultOptions));

    await waitFor(() => {
      expect(result.current.items.length).toBe(MOCK_ITEMS.length);
    });

    act(() => {
      result.current.setQuery("google");
      result.current.setActiveCategory("Web");
    });

    act(() => {
      result.current.reset();
    });

    await waitFor(() => {
      expect(result.current.query).toBe("");
      expect(result.current.activeCategory).toBeNull();
      expect(result.current.selectedIndex).toBe(0);
    });
  });
});
