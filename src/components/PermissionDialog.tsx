import { useEffect, useState } from "react";
import type { PermissionRequest } from "../types";

interface PermissionDialogProps {
  request: PermissionRequest;
  onResolve: (requestId: string, optionId: string) => void;
}

export function PermissionDialog({ request, onResolve }: PermissionDialogProps) {
  const yesOption =
    request.options.find((option) => {
      const value = `${option.kind} ${option.name}`.toLowerCase();
      return value.includes("allow") || value.includes("approve");
    }) ?? request.options[0];

  const noOption =
    request.options.find((option) => {
      const value = `${option.kind} ${option.name}`.toLowerCase();
      return (
        value.includes("deny") ||
        value.includes("reject") ||
        value.includes("cancel")
      );
    }) ?? request.options[request.options.length - 1] ?? yesOption;

  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Enter" && yesOption) {
        e.preventDefault();
        onResolve(request.request_id, yesOption.option_id);
      } else if (e.key === "Escape" && noOption) {
        e.preventDefault();
        onResolve(request.request_id, noOption.option_id);
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [request, onResolve, yesOption, noOption]);

  const hasDescription = !!request.command_preview;
  const [commandExpanded, setCommandExpanded] = useState(false);

  return (
    <div data-testid="permission-dialog" className="permission-dialog">
      {hasDescription ? (
        <>
          <div className="permission-header">
            <span className="permission-title">{request.command_preview}</span>
            <span className="permission-status-badge permission-status-pending">PENDING</span>
          </div>
          <div
            className="permission-command-expandable"
            onClick={() => setCommandExpanded(!commandExpanded)}
          >
            <span className={`permission-expand-arrow ${commandExpanded ? "expanded" : ""}`}>&#9654;</span>
            <span className="permission-command-label">{request.tool_name}</span>
          </div>
          {commandExpanded && (
            <div className="permission-command-preview">
              {request.tool_name}
            </div>
          )}
        </>
      ) : (
        <div className="permission-header">
          <span className="permission-title">{request.tool_name}</span>
          <span className="permission-status-badge permission-status-pending">PENDING</span>
        </div>
      )}
      <div className="permission-actions">
        <button
          data-testid="permission-approve"
          className="permission-btn permission-btn-approve"
          onClick={() =>
            yesOption && onResolve(request.request_id, yesOption.option_id)
          }
          disabled={!yesOption}
        >
          Yes
        </button>
        <button
          data-testid="permission-deny"
          className="permission-btn permission-btn-deny"
          onClick={() => noOption && onResolve(request.request_id, noOption.option_id)}
          disabled={!noOption}
        >
          No
        </button>
      </div>
    </div>
  );
}
