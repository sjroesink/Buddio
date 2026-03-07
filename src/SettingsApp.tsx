import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { AgentSettings } from "./components/AgentSettings";
import type {
  AgentConfig,
  AgentStatus,
  AgentUpdate,
  SessionConfigOptionInfo,
} from "./types";

function SettingsApp() {
  const [status, setStatus] = useState<AgentStatus>("disconnected");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [configOptions, setConfigOptions] = useState<
    SessionConfigOptionInfo[]
  >([]);

  // Sync agent status from backend on mount + listen for changes
  useEffect(() => {
    invoke<AgentStatus>("acp_get_status").then(setStatus).catch(() => {});

    invoke<SessionConfigOptionInfo[]>("acp_get_config_options")
      .then(setConfigOptions)
      .catch(() => {});

    const unlistenUpdate = listen<AgentUpdate>("acp-update", (event) => {
      if (event.payload.type === "status_change") {
        setStatus(event.payload.status);
        if (event.payload.status !== "error") setErrorMessage(null);
      }
    });

    const unlistenConfig = listen<SessionConfigOptionInfo[]>(
      "acp-config-options",
      (event) => {
        setConfigOptions(event.payload);
      },
    );

    return () => {
      unlistenUpdate.then((f) => f());
      unlistenConfig.then((f) => f());
    };
  }, []);

  const handleConnect = useCallback(async (config: AgentConfig) => {
    try {
      setStatus("connecting");
      setErrorMessage(null);
      await invoke("acp_connect", { config });
      const opts = await invoke<SessionConfigOptionInfo[]>(
        "acp_get_config_options",
      );
      setConfigOptions(opts);
    } catch (e) {
      console.error("Failed to connect agent:", e);
      setStatus("error");
      setErrorMessage(String(e));
    }
  }, []);

  const handleDisconnect = useCallback(async () => {
    try {
      await invoke("acp_disconnect");
      setStatus("disconnected");
      setErrorMessage(null);
      setConfigOptions([]);
    } catch (e) {
      console.error("Failed to disconnect agent:", e);
    }
  }, []);

  const handleClose = useCallback(() => {
    getCurrentWindow().close();
  }, []);

  const handleSetConfigOption = useCallback(
    async (configId: string, value: string) => {
      try {
        const updated = await invoke<SessionConfigOptionInfo[]>(
          "acp_set_config_option",
          { configId, value },
        );
        setConfigOptions(updated);
      } catch (e) {
        console.error("Failed to set config option:", e);
      }
    },
    [],
  );

  return (
    <div className="settings-window">
      <AgentSettings
        status={status}
        errorMessage={errorMessage}
        configOptions={configOptions}
        onConnect={handleConnect}
        onDisconnect={handleDisconnect}
        onClose={handleClose}
        onSetConfigOption={handleSetConfigOption}
      />
    </div>
  );
}

export default SettingsApp;
