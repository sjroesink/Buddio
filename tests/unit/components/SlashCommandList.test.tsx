import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import SlashCommandList from "../../../src/components/SlashCommandList";
import type { SlashCommand, SlashCommandParam } from "../../../src/types";

describe("SlashCommandList", () => {
  it("does not render a selected row when selectedIndex is -1", () => {
    const commands: SlashCommand[] = [
      {
        id: "cmd-1",
        name: "killport",
        description: "Kill process on port",
        script_path: "C:\\scripts\\killport.ps1",
        usage_count: 2,
        created_at: "2024-01-01T00:00:00Z",
        updated_at: "2024-01-01T00:00:00Z",
      },
    ];

    render(
      <SlashCommandList
        commands={commands}
        commandParamsByName={{}}
        query="/ki"
        selectedIndex={-1}
        onSelect={vi.fn()}
        onExecute={vi.fn()}
      />,
    );

    const row = screen.getByRole("button", { name: /killport/i });
    expect(row.className).not.toContain("bg-launcher-selected/80");
  });

  it("shows parameter signature when params are available", () => {
    const commands: SlashCommand[] = [
      {
        id: "cmd-1",
        name: "killport",
        description: "Kill process on port",
        script_path: "C:\\scripts\\killport.ps1",
        usage_count: 2,
        created_at: "2024-01-01T00:00:00Z",
        updated_at: "2024-01-01T00:00:00Z",
      },
    ];

    const paramsByName: Record<string, SlashCommandParam[]> = {
      killport: [
        {
          id: "param-1",
          command_id: "cmd-1",
          name: "poort",
          description: "Port number",
          position: 0,
          required: true,
        },
      ],
    };

    render(
      <SlashCommandList
        commands={commands}
        commandParamsByName={paramsByName}
        query="/killport"
        selectedIndex={0}
        onSelect={vi.fn()}
        onExecute={vi.fn()}
      />,
    );

    expect(screen.getByText("/killport <poort>")).toBeInTheDocument();
  });
});
