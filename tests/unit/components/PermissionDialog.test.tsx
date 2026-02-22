import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { PermissionDialog } from "../../../src/components/PermissionDialog";
import type { PermissionRequest } from "../../../src/types";

const mockRequest: PermissionRequest = {
  request_id: "req-1",
  session_id: "sess-1",
  tool_name: "file_write",
  tool_description: "Write to a file",
  command_preview: null,
  options: [
    { option_id: "opt-allow", name: "Allow", kind: "allow" },
    { option_id: "opt-deny", name: "Deny", kind: "deny" },
  ],
};

describe("PermissionDialog", () => {
  it("renders the dialog with tool name", () => {
    render(<PermissionDialog request={mockRequest} onResolve={vi.fn()} />);
    expect(screen.getByTestId("permission-dialog")).toBeInTheDocument();
    expect(screen.getByText("file_write")).toBeInTheDocument();
  });

  it("renders approve and deny buttons", () => {
    render(<PermissionDialog request={mockRequest} onResolve={vi.fn()} />);
    expect(screen.getByTestId("permission-approve")).toBeInTheDocument();
    expect(screen.getByTestId("permission-deny")).toBeInTheDocument();
  });

  it("calls onResolve with allow option on approve click", async () => {
    const onResolve = vi.fn();
    render(<PermissionDialog request={mockRequest} onResolve={onResolve} />);
    await userEvent.click(screen.getByTestId("permission-approve"));
    expect(onResolve).toHaveBeenCalledWith("req-1", "opt-allow");
  });

  it("calls onResolve with deny option on deny click", async () => {
    const onResolve = vi.fn();
    render(<PermissionDialog request={mockRequest} onResolve={onResolve} />);
    await userEvent.click(screen.getByTestId("permission-deny"));
    expect(onResolve).toHaveBeenCalledWith("req-1", "opt-deny");
  });

  it("resolves with allow option on Enter key", () => {
    const onResolve = vi.fn();
    render(<PermissionDialog request={mockRequest} onResolve={onResolve} />);
    fireEvent.keyDown(window, { key: "Enter" });
    expect(onResolve).toHaveBeenCalledWith("req-1", "opt-allow");
  });

  it("resolves with deny option on Escape key", () => {
    const onResolve = vi.fn();
    render(<PermissionDialog request={mockRequest} onResolve={onResolve} />);
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onResolve).toHaveBeenCalledWith("req-1", "opt-deny");
  });
});
