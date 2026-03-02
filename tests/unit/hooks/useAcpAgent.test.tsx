import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";
import "../helpers/mockTauri";
import {
  clearMockListeners,
  emitMockEvent,
  resetInvokeHandler,
} from "../helpers/mockTauri";
import { useAcpAgent } from "../../../src/hooks/useAcpAgent";

describe("useAcpAgent tool call status handling", () => {
  beforeEach(() => {
    resetInvokeHandler();
    clearMockListeners();
  });

  afterEach(() => {
    clearMockListeners();
  });

  it("maps tool_call_update status strings like Complete to completed", async () => {
    const { result } = renderHook(() => useAcpAgent());

    await act(async () => {
      await Promise.resolve();
    });

    act(() => {
      emitMockEvent("acp-update", {
        type: "tool_call",
        id: "call-1",
        title: "ToolSearch",
        kind: "Mcp",
        content: null,
      });
    });

    await waitFor(() => {
      const toolEntry = result.current.thread.find((e) => e.role === "tool");
      expect(toolEntry?.toolStatus).toBe("running");
    });

    act(() => {
      emitMockEvent("acp-update", {
        type: "tool_call_update",
        id: "call-1",
        title: null,
        status: "Complete",
      });
    });

    await waitFor(() => {
      const toolEntry = result.current.thread.find((e) => e.role === "tool");
      expect(toolEntry?.toolStatus).toBe("completed");
      expect(toolEntry?.toolStatusRaw).toBe("Complete");
    });
  });

  it("maps wrapped status strings like Some(Error) to error", async () => {
    const { result } = renderHook(() => useAcpAgent());

    await act(async () => {
      await Promise.resolve();
    });

    act(() => {
      emitMockEvent("acp-update", {
        type: "tool_call",
        id: "call-2",
        title: "mcp_buddio_items_remove",
        kind: "Mcp",
        content: null,
      });
    });

    act(() => {
      emitMockEvent("acp-update", {
        type: "tool_call_update",
        id: "call-2",
        title: null,
        status: "Some(Error)",
      });
    });

    await waitFor(() => {
      const toolEntry = result.current.thread.find((e) => e.role === "tool");
      expect(toolEntry?.toolStatus).toBe("error");
      expect(toolEntry?.toolStatusRaw).toBe("Some(Error)");
    });
  });

  it("finalizes still-running tool calls on turn_complete", async () => {
    const { result } = renderHook(() => useAcpAgent());

    await act(async () => {
      await Promise.resolve();
    });

    act(() => {
      emitMockEvent("acp-update", {
        type: "tool_call",
        id: "call-3",
        title: "ToolSearch",
        kind: "Mcp",
        content: null,
      });
    });

    act(() => {
      emitMockEvent("acp-update", {
        type: "turn_complete",
        stop_reason: "completed",
      });
    });

    await waitFor(() => {
      const toolEntry = result.current.thread.find((e) => e.role === "tool");
      expect(toolEntry?.toolStatus).toBe("completed");
      expect(toolEntry?.toolStatusRaw).toContain("inferred on turn_complete");
    });
  });
});
