import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { AgentResponse } from "../../../src/components/AgentResponse";

describe("AgentResponse", () => {
  const baseProps = {
    thread: [],
    thoughts: "",
    isThinking: false,
    turnActive: false,
    permissionRequest: null,
    onResolvePermission: vi.fn(),
  };

  it("shows an empty session state with history action", async () => {
    const onShowHistory = vi.fn();
    const user = userEvent.setup();

    render(
      <AgentResponse
        {...baseProps}
        onShowHistory={onShowHistory}
        onNewConversation={vi.fn()}
      />,
    );

    expect(screen.getByTestId("agent-empty-state")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "History" }));
    expect(onShowHistory).toHaveBeenCalledOnce();
  });

  it("expands tool call details when clicking an entry without payload", async () => {
    const user = userEvent.setup();

    render(
      <AgentResponse
        {...baseProps}
        thread={[
          {
            id: "tool-abc123",
            role: "tool",
            content: "",
            toolTitle: "ToolSearch",
            toolStatus: "completed",
            toolStatusRaw: "Complete",
            toolKind: "Mcp",
          },
        ]}
      />,
    );

    expect(
      screen.queryByText("No payload captured for this tool call."),
    ).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /toolsearch/i }));

    expect(
      screen.getByText("No payload captured for this tool call."),
    ).toBeInTheDocument();
    expect(screen.getByText("abc123")).toBeInTheDocument();
    expect(screen.getByText("Complete")).toBeInTheDocument();
  });

  it("shows tool payload details when available", async () => {
    const user = userEvent.setup();

    render(
      <AgentResponse
        {...baseProps}
        thread={[
          {
            id: "tool-xyz789",
            role: "tool",
            content: "",
            toolTitle: "mcp_buddio_items_remove",
            toolStatus: "running",
            commandPreview: "buddio-cli items remove --id 1",
          },
        ]}
      />,
    );

    await user.click(screen.getByRole("button", { name: /items remove/i }));

    expect(
      screen.getByText("buddio-cli items remove --id 1"),
    ).toBeInTheDocument();
    expect(screen.getByText("xyz789")).toBeInTheDocument();
  });
});
