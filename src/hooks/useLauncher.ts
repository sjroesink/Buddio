import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { LaunchItem, AgentStatus, CommandSuggestion, SlashCommand, SlashCommandParam } from "../types";

interface UseLauncherOptions {
  agentStatus: AgentStatus;
  agentAutoFallback: boolean;
  onAgentPrompt: (query: string) => void;
  onSlashCommandCreate: (query: string) => void;
  onAgentCancel: () => void;
  agentTurnActive: boolean;
  hasSelectedText?: boolean;
  onExecuteSuccess?: (output?: string | null) => void;
  onExecuteError?: (error: string) => void;
}

export function useLauncher(options: UseLauncherOptions) {
  const [query, setQueryState] = useState("");
  const [items, setItems] = useState<LaunchItem[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [categories, setCategories] = useState<string[]>([]);
  const [activeCategory, setActiveCategory] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [suggestions, setSuggestions] = useState<CommandSuggestion[]>([]);
  const [selectedSuggestionIndex, setSelectedSuggestionIndex] = useState(-1);
  const [focusInputSignal, setFocusInputSignal] = useState(0);
  const [slashCommands, setSlashCommands] = useState<SlashCommand[]>([]);
  const [slashCommandParamsByName, setSlashCommandParamsByName] = useState<
    Record<string, SlashCommandParam[]>
  >({});
  const [selectedSlashIndex, setSelectedSlashIndex] = useState(-1);
  const [paramEntryMode, setParamEntryMode] = useState(false);
  const [currentParamIndex, setCurrentParamIndex] = useState(0);
  const [activeCommandParams, setActiveCommandParams] = useState<SlashCommandParam[]>([]);
  const [activeCommandName, setActiveCommandName] = useState("");
  const debounceRef = useRef<ReturnType<typeof setTimeout>>();
  const queryBeforeSuggestionSelectRef = useRef<string | null>(null);
  const queryBeforeItemSelectRef = useRef<string | null>(null);

  const setQuery = useCallback((nextQuery: string) => {
    queryBeforeSuggestionSelectRef.current = null;
    queryBeforeItemSelectRef.current = null;
    setSelectedSuggestionIndex(-1);
    setQueryState(nextQuery);
  }, []);

  const fetchItems = useCallback(async (searchQuery: string) => {
    setLoading(true);
    try {
      const results = await invoke<LaunchItem[]>("search_items", {
        query: searchQuery,
      });
      setItems(results);
      setSelectedIndex(0);
    } catch (err) {
      console.error("Failed to fetch items:", err);
    } finally {
      setLoading(false);
    }
  }, []);

  const fetchCategories = useCallback(async () => {
    try {
      const cats = await invoke<string[]>("get_categories");
      setCategories(cats);
    } catch (err) {
      console.error("Failed to fetch categories:", err);
    }
  }, []);

  // Compute categories with "Selection" first if text is selected
  const displayCategories = options.hasSelectedText && categories.includes("Selection")
    ? ["Selection", ...categories.filter(c => c !== "Selection")]
    : categories;

  useEffect(() => {
    fetchItems("");
    fetchCategories();
  }, [fetchItems, fetchCategories]);

  // Auto-select "Selection" category when text is selected
  useEffect(() => {
    if (options.hasSelectedText && categories.includes("Selection")) {
      setActiveCategory("Selection");
    } else if (!options.hasSelectedText && activeCategory === "Selection") {
      setActiveCategory(null);
    }
  }, [options.hasSelectedText, categories]);

  useEffect(() => {
    // Don't re-fetch while previewing items with arrow keys
    if (queryBeforeItemSelectRef.current !== null) return;

    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => {
      fetchItems(query);
    }, 100);
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, [query, fetchItems]);

  // Slash mode: detect "/" prefix
  const isSlashMode = query.startsWith("/");

  // Fetch slash commands when in slash mode (but not in param entry mode)
  useEffect(() => {
    if (!isSlashMode) {
      setSlashCommands([]);
      setSlashCommandParamsByName({});
      setSelectedSlashIndex(-1);
      setParamEntryMode(false);
      setActiveCommandParams([]);
      setActiveCommandName("");
      setCurrentParamIndex(0);
      return;
    }

    // Don't fetch command list while entering params
    if (paramEntryMode) return;

    const afterSlash = query.slice(1);
    const nameFragment = afterSlash.split(/\s/)[0];

    if (nameFragment === "") {
      invoke<SlashCommand[]>("list_slash_commands")
        .then((cmds) => {
          setSlashCommands(cmds);
          setSelectedSlashIndex(-1);
        })
        .catch(() => setSlashCommands([]));
    } else {
      invoke<SlashCommand[]>("search_slash_commands", { query: nameFragment })
        .then((cmds) => {
          setSlashCommands(cmds);
          setSelectedSlashIndex(-1);
        })
        .catch(() => setSlashCommands([]));
    }
  }, [query, isSlashMode, paramEntryMode]);

  // Preload slash command params while browsing the command list to enable inline intellisense.
  useEffect(() => {
    if (!isSlashMode || paramEntryMode || slashCommands.length === 0) return;

    const missing = slashCommands.filter(
      (cmd) => !Object.prototype.hasOwnProperty.call(slashCommandParamsByName, cmd.name),
    );
    if (missing.length === 0) return;

    let cancelled = false;

    Promise.all(
      missing.map(async (cmd) => {
        try {
          const params = await invoke<SlashCommandParam[]>("get_slash_command_params", {
            name: cmd.name,
          });
          return [cmd.name, params] as const;
        } catch {
          return [cmd.name, [] as SlashCommandParam[]] as const;
        }
      }),
    ).then((entries) => {
      if (cancelled) return;
      setSlashCommandParamsByName((prev) => {
        const next = { ...prev };
        let changed = false;
        for (const [name, params] of entries) {
          if (!Object.prototype.hasOwnProperty.call(next, name)) {
            next[name] = params;
            changed = true;
          }
        }
        return changed ? next : prev;
      });
    });

    return () => {
      cancelled = true;
    };
  }, [isSlashMode, paramEntryMode, slashCommands, slashCommandParamsByName]);

  // Exit param entry mode if user backspaces past the space
  useEffect(() => {
    if (paramEntryMode && !query.includes(" ")) {
      setParamEntryMode(false);
      setActiveCommandParams([]);
      setActiveCommandName("");
      setCurrentParamIndex(0);
    }
  }, [query, paramEntryMode]);

  const parseSlashInput = useCallback(
    (input: string): { name: string; args: string } | null => {
      if (!input.startsWith("/")) return null;
      const withoutSlash = input.slice(1).trim();
      const spaceIndex = withoutSlash.indexOf(" ");
      if (spaceIndex === -1) {
        return { name: withoutSlash, args: "" };
      }
      return {
        name: withoutSlash.slice(0, spaceIndex),
        args: withoutSlash.slice(spaceIndex + 1).trim(),
      };
    },
    [],
  );

  // Track current param index based on comma count
  useEffect(() => {
    if (!paramEntryMode) {
      setCurrentParamIndex(0);
      return;
    }
    const parsed = parseSlashInput(query);
    if (!parsed) return;
    const commaCount = (parsed.args.match(/,/g) || []).length;
    setCurrentParamIndex(commaCount);
  }, [query, paramEntryMode, parseSlashInput]);

  const executeSlashCommand = useCallback(
    async (name: string, args: string) => {
      // Convert comma-separated args to space-separated for the backend
      const normalizedArgs = args.includes(",")
        ? args.split(",").map((s) => s.trim()).filter(Boolean).join(" ")
        : args;
      try {
        await invoke<string>("execute_slash_command", { name, args: normalizedArgs });
        setParamEntryMode(false);
        setActiveCommandParams([]);
        setActiveCommandName("");
        setCurrentParamIndex(0);
        await invoke("hide_window");
      } catch (err: unknown) {
        // Command not found -> send to slash command agent
        if (typeof err === "string" && err.includes("not found")) {
          options.onSlashCommandCreate(`/${name} ${args}`.trim());
          setQueryState("");
        } else {
          console.error("Slash command failed:", err);
        }
      }
    },
    [options],
  );

  // Filter items and sort so Selection items appear first when text is selected
  let filteredItems = activeCategory
    ? items.filter((item) => item.category === activeCategory)
    : items;

  // When text is selected, show Selection items first
  if (options.hasSelectedText && !activeCategory) {
    filteredItems = [
      ...filteredItems.filter((item) => item.category === "Selection"),
      ...filteredItems.filter((item) => item.category !== "Selection"),
    ];
  }

  // Agent mode conditions met (used internally — UI doesn't switch until Enter)
  const agentModeReady =
    filteredItems.length === 0 &&
    query.length > 2 &&
    options.agentStatus === "connected" &&
    options.agentAutoFallback;

  // Agent mode only activates after user confirms with Enter
  const [agentModeConfirmed, setAgentModeConfirmed] = useState(false);
  const agentMode = agentModeConfirmed;

  // Reset confirmed agent mode when query changes
  useEffect(() => {
    setAgentModeConfirmed(false);
  }, [query]);

  // Fetch suggestions when no results and agent mode not yet confirmed
  useEffect(() => {
    const shouldSuggest =
      filteredItems.length === 0 &&
      query.length > 2 &&
      !agentModeConfirmed;

    if (shouldSuggest) {
      invoke<CommandSuggestion[]>("get_command_suggestions", { query })
        .then(setSuggestions)
        .catch(() => setSuggestions([]));
    } else {
      setSuggestions([]);
    }
  }, [filteredItems.length, query, agentModeConfirmed]);

  useEffect(() => {
    if (suggestions.length === 0) {
      setSelectedSuggestionIndex(-1);
      queryBeforeSuggestionSelectRef.current = null;
      return;
    }

    if (selectedSuggestionIndex >= suggestions.length) {
      setSelectedSuggestionIndex(suggestions.length - 1);
    }
  }, [suggestions, selectedSuggestionIndex]);

  const previewSuggestion = useCallback(
    (nextIndex: number) => {
      if (suggestions.length === 0) return;

      const clampedIndex = Math.max(0, Math.min(nextIndex, suggestions.length - 1));

      setSelectedSuggestionIndex(clampedIndex);
      if (queryBeforeSuggestionSelectRef.current === null) {
        queryBeforeSuggestionSelectRef.current = query;
      }
      setQueryState(suggestions[clampedIndex].suggested_command);
    },
    [suggestions, query],
  );

  const restoreQueryFromSuggestionPreview = useCallback(() => {
    setSelectedSuggestionIndex(-1);
    if (queryBeforeSuggestionSelectRef.current !== null) {
      setQueryState(queryBeforeSuggestionSelectRef.current);
    }
    queryBeforeSuggestionSelectRef.current = null;
    setFocusInputSignal((prev) => prev + 1);
  }, []);

  const handleInputFocus = useCallback(() => {
    if (queryBeforeItemSelectRef.current !== null) {
      setQueryState(queryBeforeItemSelectRef.current);
      queryBeforeItemSelectRef.current = null;
    }
    if (selectedSuggestionIndex >= 0) {
      restoreQueryFromSuggestionPreview();
    }
  }, [selectedSuggestionIndex, restoreQueryFromSuggestionPreview]);

  const selectSuggestion = useCallback(
    (index: number) => {
      previewSuggestion(index);
    },
    [previewSuggestion],
  );

  const executeSelected = useCallback(async () => {
    const item = filteredItems[selectedIndex];
    if (!item) return;
    try {
      const output = await invoke<string | null>("execute_item", { id: item.id });
      options.onExecuteSuccess?.(output);
    } catch (err) {
      console.error("Failed to execute item:", err);
      options.onExecuteError?.(String(err));
    }
  }, [filteredItems, selectedIndex, options]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      // Slash command mode: Escape exits param entry mode
      if (isSlashMode && paramEntryMode && e.key === "Escape") {
        e.preventDefault();
        setParamEntryMode(false);
        setActiveCommandParams([]);
        setCurrentParamIndex(0);
        setQueryState(`/${activeCommandName}`);
        setActiveCommandName("");
        return;
      }

      // Slash command mode: Enter executes or enters param mode
      if (isSlashMode && e.key === "Enter") {
        e.preventDefault();
        const parsed = parseSlashInput(query);
        if (parsed && parsed.name) {
          if (paramEntryMode) {
            // In param entry mode — execute with comma-separated args
            executeSlashCommand(parsed.name, parsed.args);
          } else {
            const hasArgs = query.includes(" ");
            if (hasArgs) {
              // User typed "/kill 4924" — execute directly
              executeSlashCommand(parsed.name, parsed.args);
            } else {
              const selectedFromList =
                selectedSlashIndex >= 0 ? slashCommands[selectedSlashIndex] : undefined;
              const exactMatch = slashCommands.find((cmd) => cmd.name === parsed.name);
              const commandToEnter = selectedFromList ?? exactMatch;

              // Enter on an explicitly selected command (or exact name match) enters param mode.
              if (commandToEnter) {
                setQueryState(`/${commandToEnter.name} `);
                setActiveCommandName(commandToEnter.name);
                setParamEntryMode(true);
                setCurrentParamIndex(0);
                // Fetch params for this command (or reuse prefetched values).
                if (
                  Object.prototype.hasOwnProperty.call(
                    slashCommandParamsByName,
                    commandToEnter.name,
                  )
                ) {
                  setActiveCommandParams(slashCommandParamsByName[commandToEnter.name]);
                } else {
                  invoke<SlashCommandParam[]>("get_slash_command_params", {
                    name: commandToEnter.name,
                  })
                    .then((params) => {
                      setActiveCommandParams(params);
                      setSlashCommandParamsByName((prev) => ({
                        ...prev,
                        [commandToEnter.name]: params,
                      }));
                    })
                    .catch(() => setActiveCommandParams([]));
                }
              } else if (slashCommands.length === 0) {
                // No matching slash command — send to slash command agent for creation
                options.onSlashCommandCreate(query);
                setQueryState("");
              }
            }
          }
        }
        return;
      }

      // Slash command mode: Arrow keys navigate list (only when not in param entry mode)
      if (
        isSlashMode &&
        !paramEntryMode &&
        slashCommands.length > 0 &&
        (e.key === "ArrowDown" || e.key === "ArrowUp")
      ) {
        e.preventDefault();
        if (e.key === "ArrowDown") {
          setSelectedSlashIndex((prev) => {
            if (prev < 0) return 0;
            return Math.min(prev + 1, slashCommands.length - 1);
          });
        } else {
          setSelectedSlashIndex((prev) => {
            if (prev < 0) return slashCommands.length - 1;
            return Math.max(prev - 1, 0);
          });
        }
        return;
      }

      // In agent mode with active turn, Escape cancels
      if (options.agentTurnActive && e.key === "Escape") {
        e.preventDefault();
        options.onAgentCancel();
        return;
      }

      if (suggestions.length > 0 && (e.key === "ArrowDown" || e.key === "ArrowUp")) {
        e.preventDefault();

        if (selectedSuggestionIndex < 0) {
          if (e.key === "ArrowDown") {
            previewSuggestion(0);
          } else {
            previewSuggestion(suggestions.length - 1);
          }
          return;
        }

        if (e.key === "ArrowDown") {
          if (selectedSuggestionIndex >= suggestions.length - 1) {
            restoreQueryFromSuggestionPreview();
          } else {
            previewSuggestion(selectedSuggestionIndex + 1);
          }
        } else if (selectedSuggestionIndex <= 0) {
          restoreQueryFromSuggestionPreview();
        } else {
          previewSuggestion(selectedSuggestionIndex - 1);
        }

        return;
      }

      // Agent mode ready: Enter confirms and triggers agent prompt
      if (agentModeReady && e.key === "Enter") {
        e.preventDefault();
        setAgentModeConfirmed(true);
        options.onAgentPrompt(query);
        setQueryState("");
        return;
      }

      switch (e.key) {
        case "ArrowDown":
          e.preventDefault();
          if (filteredItems.length > 0) {
            const nextIdx =
              selectedIndex < filteredItems.length - 1 ? selectedIndex + 1 : 0;
            setSelectedIndex(nextIdx);
            if (queryBeforeItemSelectRef.current === null) {
              queryBeforeItemSelectRef.current = query;
            }
            setQueryState(filteredItems[nextIdx].title);
          }
          break;
        case "ArrowUp":
          e.preventDefault();
          if (filteredItems.length > 0) {
            const nextIdx =
              selectedIndex > 0 ? selectedIndex - 1 : filteredItems.length - 1;
            setSelectedIndex(nextIdx);
            if (queryBeforeItemSelectRef.current === null) {
              queryBeforeItemSelectRef.current = query;
            }
            setQueryState(filteredItems[nextIdx].title);
          }
          break;
        case "Enter":
          e.preventDefault();
          // Clear preview state if we were arrow-navigating
          if (queryBeforeItemSelectRef.current !== null) {
            queryBeforeItemSelectRef.current = null;
          }
          executeSelected();
          break;
        case "Escape":
          e.preventDefault();
          if (queryBeforeItemSelectRef.current !== null) {
            setQueryState(queryBeforeItemSelectRef.current);
            queryBeforeItemSelectRef.current = null;
            setFocusInputSignal((prev) => prev + 1);
            break;
          }
          if (selectedSuggestionIndex >= 0) {
            restoreQueryFromSuggestionPreview();
            break;
          }
          if (query) {
            setQuery("");
          } else {
            invoke("hide_window");
          }
          break;
        case "Tab":
          e.preventDefault();
          if (categories.length > 0) {
            const currentIdx = activeCategory
              ? categories.indexOf(activeCategory)
              : -1;
            if (e.shiftKey) {
              setActiveCategory(
                currentIdx <= 0 ? null : categories[currentIdx - 1],
              );
            } else {
              setActiveCategory(
                currentIdx >= categories.length - 1
                  ? null
                  : categories[currentIdx + 1],
              );
            }
            setSelectedIndex(0);
          }
          break;
      }
    },
    [
      filteredItems,
      executeSelected,
      query,
      categories,
      activeCategory,
      agentModeReady,
      options,
      suggestions,
      selectedSuggestionIndex,
      previewSuggestion,
      restoreQueryFromSuggestionPreview,
      setQuery,
      isSlashMode,
      slashCommands,
      slashCommandParamsByName,
      selectedSlashIndex,
      parseSlashInput,
      executeSlashCommand,
      paramEntryMode,
      activeCommandName,
    ],
  );

  const refresh = useCallback(() => {
    fetchItems(query);
    fetchCategories();
  }, [fetchItems, fetchCategories, query]);

  const reset = useCallback(() => {
    setQuery("");
    setSelectedIndex(0);
    setActiveCategory(null);
    setSuggestions([]);
    setSlashCommands([]);
    setSlashCommandParamsByName({});
    setSelectedSlashIndex(-1);
    setAgentModeConfirmed(false);
    setParamEntryMode(false);
    setCurrentParamIndex(0);
    setActiveCommandParams([]);
    setActiveCommandName("");
    queryBeforeItemSelectRef.current = null;
    fetchItems("");
    fetchCategories();
  }, [fetchItems, fetchCategories, setQuery]);

  return {
    query,
    setQuery,
    items: filteredItems,
    selectedIndex,
    setSelectedIndex,
    categories: displayCategories,
    activeCategory,
    setActiveCategory,
    loading,
    handleKeyDown,
    executeSelected,
    agentMode,
    suggestions,
    selectedSuggestionIndex,
    selectSuggestion,
    focusInputSignal,
    handleInputFocus,
    refresh,
    reset,
    isSlashMode,
    slashCommands,
    slashCommandParamsByName,
    selectedSlashIndex,
    setSelectedSlashIndex,
    executeSlashCommand,
    paramEntryMode,
    currentParamIndex,
    activeCommandParams,
    activeCommandName,
  };
}
