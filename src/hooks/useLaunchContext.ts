import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { LaunchContext, CommandSuggestion } from "../types";

const EMPTY_CONTEXT: LaunchContext = {
  clipboard_text: null,
  selected_text: null,
  source_window_title: null,
  source_process_name: null,
  source_process_path: null,
};

export function useLaunchContext() {
  const [context, setContext] = useState<LaunchContext>(EMPTY_CONTEXT);
  const [rewriteSuggestions, setRewriteSuggestions] = useState<
    CommandSuggestion[]
  >([]);

  // Listen for context events emitted by the global shortcut handler
  useEffect(() => {
    const unlisten = listen<LaunchContext>("launch-context", (event) => {
      setContext(event.payload);
    });

    // Also fetch the current context on mount (in case we missed the event)
    invoke<LaunchContext>("get_launch_context")
      .then(setContext)
      .catch(() => {});

    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  // Load rewrite suggestions whenever we have a selection
  useEffect(() => {
    if (context.selected_text) {
      invoke<CommandSuggestion[]>("get_rewrite_suggestions")
        .then(setRewriteSuggestions)
        .catch(() => setRewriteSuggestions([]));
    } else {
      setRewriteSuggestions([]);
    }
  }, [context.selected_text]);

  // Clear context (e.g. when launcher is reset)
  const clearContext = useCallback(() => {
    setContext(EMPTY_CONTEXT);
    setRewriteSuggestions([]);
  }, []);

  // Clear individual context parts
  const clearSelection = useCallback(() => {
    setContext((prev) => ({ ...prev, selected_text: null }));
  }, []);

  const clearClipboard = useCallback(() => {
    setContext((prev) => ({ ...prev, clipboard_text: null }));
  }, []);

  // Type text into the source app (hides launcher, focuses source, types)
  const typeText = useCallback(async (text: string) => {
    await invoke("type_text_to_app", { text });
  }, []);

  // Replace the selection in the source app with new text
  const replaceSelection = useCallback(async (text: string) => {
    await invoke("replace_selection_text", { text });
  }, []);

  // Record a rewrite prompt for future suggestions
  const recordRewrite = useCallback(async (prompt: string) => {
    await invoke("record_rewrite", { prompt });
    // Refresh suggestions
    invoke<CommandSuggestion[]>("get_rewrite_suggestions")
      .then(setRewriteSuggestions)
      .catch(() => {});
  }, []);

  return {
    context,
    clearContext,
    clearSelection,
    clearClipboard,
    typeText,
    replaceSelection,
    recordRewrite,
    rewriteSuggestions,
    hasSelection: !!context.selected_text,
    hasClipboard: !!context.clipboard_text,
  };
}
