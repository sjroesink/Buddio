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
});
