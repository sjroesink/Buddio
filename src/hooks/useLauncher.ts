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
  onExecuteSuccess?: () => void;
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
  const [savingCommand, setSavingCommand] = useState(false);
  const [slashCommands, setSlashCommands] = useState<SlashCommand[]>([]);
  const [selectedSlashIndex, setSelectedSlashIndex] = useState(0);
  const [paramEntryMode, setParamEntryMode] = useState(false);
  const [currentParamIndex, setCurrentParamIndex] = useState(0);
  const [activeCommandParams, setActiveCommandParams] = useState<SlashCommandParam[]>([]);
  const [activeCommandName, setActiveCommandName] = useState("");
  const debounceRef = useRef<ReturnType<typeof setTimeout>>();
  const queryBeforeSuggestionSelectRef = useRef<string | null>(null);

  const setQuery = useCallback((nextQuery: string) => {
    queryBeforeSuggestionSelectRef.current = null;
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
      setSelectedSlashIndex(0);
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
          setSelectedSlashIndex(0);
        })
        .catch(() => setSlashCommands([]));
    } else {
      invoke<SlashCommand[]>("search_slash_commands", { query: nameFragment })
        .then((cmds) => {
          setSlashCommands(cmds);
          setSelectedSlashIndex(0);
        })
        .catch(() => setSlashCommands([]));
    }
  }, [query, isSlashMode, paramEntryMode]);

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

  const saveCommandFromSuggestion = useCallback(
    async (suggestion: CommandSuggestion) => {
      setSavingCommand(true);
      try {
        await invoke("add_item_from_suggestion", {
          title: suggestion.suggested_command,
          actionValue: suggestion.suggested_command,
          actionType: "command",
          category: null,
        });
        await fetchItems(query);
        fetchCategories();
        setSuggestions([]);
      } catch (err) {
        console.error("Failed to save command:", err);
      } finally {
        setSavingCommand(false);
      }
    },
    [query, fetchItems, fetchCategories],
  );

  const executeSelected = useCallback(async () => {
    const item = filteredItems[selectedIndex];
    if (!item) return;
    try {
      await invoke("execute_item", { id: item.id });
      options.onExecuteSuccess?.();
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
            } else if (slashCommands.length > 0) {
              // User pressed Enter on a slash command from the list — enter param entry mode
              const selected = slashCommands[selectedSlashIndex];
              if (selected) {
                setQueryState(`/${selected.name} `);
                setActiveCommandName(selected.name);
                setParamEntryMode(true);
                setCurrentParamIndex(0);
                // Fetch params for this command
                invoke<SlashCommandParam[]>("get_slash_command_params", { name: selected.name })
                  .then((params) => setActiveCommandParams(params))
                  .catch(() => setActiveCommandParams([]));
              }
            } else {
              // No matching slash command — send to slash command agent for creation
              options.onSlashCommandCreate(query);
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
          setSelectedSlashIndex((prev) =>
            Math.min(prev + 1, slashCommands.length - 1),
          );
        } else {
          setSelectedSlashIndex((prev) => Math.max(prev - 1, 0));
        }
        return;
      }

      // In agent mode with active turn, Escape cancels
      if (options.agentTurnActive && e.key === "Escape") {
        e.preventDefault();
        options.onAgentCancel();
        return;
      }

      // Ctrl+S saves the top suggestion as a command
      if (
        suggestions.length > 0 &&
        e.key === "s" &&
        (e.ctrlKey || e.metaKey)
      ) {
        e.preventDefault();
        saveCommandFromSuggestion(suggestions[0]);
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
        return;
      }

      switch (e.key) {
        case "ArrowDown":
          e.preventDefault();
          setSelectedIndex((prev) =>
            prev < filteredItems.length - 1 ? prev + 1 : 0,
          );
          break;
        case "ArrowUp":
          e.preventDefault();
          setSelectedIndex((prev) =>
            prev > 0 ? prev - 1 : filteredItems.length - 1,
          );
          break;
        case "Enter":
          e.preventDefault();
          executeSelected();
          break;
        case "Escape":
          e.preventDefault();
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
      saveCommandFromSuggestion,
      selectedSuggestionIndex,
      previewSuggestion,
      restoreQueryFromSuggestionPreview,
      setQuery,
      isSlashMode,
      slashCommands,
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
    setSelectedSlashIndex(0);
    setAgentModeConfirmed(false);
    setParamEntryMode(false);
    setCurrentParamIndex(0);
    setActiveCommandParams([]);
    setActiveCommandName("");
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
    savingCommand,
    saveCommandFromSuggestion,
    refresh,
    reset,
    isSlashMode,
    slashCommands,
    selectedSlashIndex,
    setSelectedSlashIndex,
    executeSlashCommand,
    paramEntryMode,
    currentParamIndex,
    activeCommandParams,
    activeCommandName,
  };
}
