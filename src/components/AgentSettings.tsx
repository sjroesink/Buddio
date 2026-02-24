import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import type {
  AgentConfig,
  AgentStatus,
  RegistryAgent,
  SessionConfigOptionInfo,
} from "../types";

type SettingsTab = "general" | "agent";

const SHORTCUT_OPTIONS = [
  { value: "double-caps-lock", label: "Double Caps Lock", description: "Tap Caps Lock twice to toggle" },
  { value: "ctrl+space", label: "Ctrl + Space", description: "Hold Ctrl and press Space" },
  { value: "alt+space", label: "Alt + Space", description: "Hold Alt and press Space" },
] as const;

const NAV_ITEMS: { id: SettingsTab; label: string; icon: string }[] = [
  { id: "general", label: "General", icon: "M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.066 2.573c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.573 1.066c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.066-2.573c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065zM15 12a3 3 0 11-6 0 3 3 0 016 0z" },
  { id: "agent", label: "Agent", icon: "M9.75 3.104v5.714a2.25 2.25 0 01-.659 1.591L5 14.5M9.75 3.104c-.251.023-.501.05-.75.082m.75-.082a24.301 24.301 0 014.5 0m0 0v5.714a2.25 2.25 0 00.659 1.591L19 14.5M14.25 3.104c.251.023.501.05.75.082M19 14.5l-2.47 2.47a2.25 2.25 0 01-1.59.659H9.06a2.25 2.25 0 01-1.591-.659L5 14.5m14 0V5.846a2.25 2.25 0 00-1.36-2.066A48.07 48.07 0 0012 3c-1.77 0-3.513.12-5.218.345A2.25 2.25 0 005.64 5.394V14.5" },
];

interface AgentSettingsProps {
  status: AgentStatus;
  configOptions: SessionConfigOptionInfo[];
  onConnect: (config: AgentConfig) => void;
  onDisconnect: () => void;
  onClose: () => void;
  onSetConfigOption: (configId: string, value: string) => void;
}

export function AgentSettings({
  status,
  configOptions,
  onConnect,
  onDisconnect,
  onClose,
  onSetConfigOption,
}: AgentSettingsProps) {
  const [activeTab, setActiveTab] = useState<SettingsTab>("general");
  const [agents, setAgents] = useState<RegistryAgent[]>([]);
  const [installStatus, setInstallStatus] = useState<Record<string, boolean>>(
    {},
  );
  const [selectedAgentId, setSelectedAgentId] = useState("");
  const [autoFallback, setAutoFallback] = useState(false);
  const [agentEnvValues, setAgentEnvValues] = useState<
    Record<string, Record<string, string>>
  >({});
  const [loading, setLoading] = useState(true);
  const [installing, setInstalling] = useState<string | null>(null);
  const [shortcutMode, setShortcutMode] = useState("double-caps-lock");

  // Load saved config, fetch registry, check installs
  useEffect(() => {
    async function init() {
      try {
        const mode = await invoke<string>("get_shortcut_mode");
        setShortcutMode(mode);

        const config = await invoke<AgentConfig>("get_agent_config");
        setSelectedAgentId(config.agent_id);
        setAutoFallback(config.auto_fallback);

        const registryAgents =
          await invoke<RegistryAgent[]>("acp_fetch_registry");
        setAgents(registryAgents);

        const installed = await invoke<Record<string, boolean>>(
          "acp_check_agents_installed",
          { agents: registryAgents },
        );
        setInstallStatus(installed);

        const envMap: Record<string, Record<string, string>> = {};
        for (const agent of registryAgents) {
          if (agent.required_env.length > 0) {
            try {
              const pairs = await invoke<[string, string][]>("get_agent_env", {
                agentId: agent.id,
              });
              const values: Record<string, string> = {};
              for (const [name, value] of pairs) {
                values[name] = value;
              }
              envMap[agent.id] = values;
            } catch {
              envMap[agent.id] = {};
            }
          }
        }
        setAgentEnvValues(envMap);
      } catch (e) {
        console.error("Failed to initialize agent settings:", e);
      }
      setLoading(false);
    }
    init();
  }, []);

  // Escape to close
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  async function handleShortcutChange(mode: string) {
    setShortcutMode(mode);
    try {
      await invoke("set_shortcut_mode", { mode });
    } catch (e) {
      console.error("Failed to set shortcut mode:", e);
    }
  }

  function updateEnvValue(agentId: string, envName: string, value: string) {
    setAgentEnvValues((prev) => ({
      ...prev,
      [agentId]: { ...prev[agentId], [envName]: value },
    }));
  }

  async function saveEnvValue(
    agentId: string,
    envName: string,
    value: string,
  ) {
    try {
      await invoke("set_agent_env", { agentId, envName, value });
    } catch (e) {
      console.error("Failed to save env var:", e);
    }
  }

  async function handleInstall(agent: RegistryAgent) {
    setInstalling(agent.id);
    try {
      await invoke("acp_install_agent", { agent });
      const installed = await invoke<Record<string, boolean>>(
        "acp_check_agents_installed",
        { agents },
      );
      setInstallStatus(installed);
    } catch (e) {
      console.error("Failed to install agent:", e);
    }
    setInstalling(null);
  }

  async function handleConnect() {
    const agent = agents.find((a) => a.id === selectedAgentId);
    if (!agent) return;

    const envEntries = agentEnvValues[agent.id] || {};
    const envString = Object.entries(envEntries)
      .filter(([, v]) => v)
      .map(([k, v]) => `${k}=${v}`)
      .join(",");

    const isNpx = agent.distribution_type === "npx";
    const distArgs = agent.distribution_args || [];

    let binaryPath: string;
    let args: string;

    if (isNpx) {
      binaryPath = "npx";
      args = [agent.distribution_detail, ...distArgs].join(" ");
    } else {
      const rawCmd = agent.distribution_detail.replace(/^\.\//, "").replace(/^\.\\/, "");
      binaryPath = rawCmd;
      args = distArgs.join(" ");
    }

    const config: AgentConfig = {
      source: "registry",
      agent_id: agent.id,
      binary_path: binaryPath,
      args,
      env: envString,
      auto_fallback: autoFallback,
    };

    try {
      await invoke("save_agent_config", { config });
      for (const [name, value] of Object.entries(envEntries)) {
        await invoke("set_agent_env", {
          agentId: agent.id,
          envName: name,
          value,
        });
      }
    } catch (e) {
      console.error("Failed to save config:", e);
    }

    onConnect(config);
  }

  const selectedAgent = agents.find((a) => a.id === selectedAgentId);
  const canConnect =
    selectedAgent && installStatus[selectedAgentId] !== false;

  // ── Render sections ──

  function renderGeneralSection() {
    return (
      <div className="settings-section-content">
        <div className="settings-section-title">Activation Shortcut</div>
        <div className="shortcut-options">
          {SHORTCUT_OPTIONS.map((opt) => (
            <label
              key={opt.value}
              className={`shortcut-option ${shortcutMode === opt.value ? "shortcut-option-active" : ""}`}
            >
              <input
                type="radio"
                name="shortcut-mode"
                value={opt.value}
                checked={shortcutMode === opt.value}
                onChange={() => handleShortcutChange(opt.value)}
              />
              <div className="shortcut-option-content">
                <span className="shortcut-option-label">{opt.label}</span>
                <span className="shortcut-option-desc">{opt.description}</span>
              </div>
            </label>
          ))}
        </div>
      </div>
    );
  }

  function renderAgentSection() {
    return (
      <div className="settings-section-content">
        <div className="settings-section-title">Agent</div>

        {loading ? (
          <div className="agent-list-empty">Loading agents...</div>
        ) : agents.length === 0 ? (
          <div className="agent-list-empty">
            No agents available. Check your network connection.
          </div>
        ) : (
          <div className="agent-list">
            {agents.map((agent) => {
              const isSelected = selectedAgentId === agent.id;
              const isInstalled = installStatus[agent.id] === true;
              const canInstall =
                (agent.distribution_type === "npx" && agent.distribution_detail) ||
                (agent.distribution_type === "binary" && agent.archive_url);
              const isInstallingThis = installing === agent.id;

              return (
                <div
                  key={agent.id}
                  className={`agent-item ${isSelected ? "agent-item-selected" : ""}`}
                  onClick={() => setSelectedAgentId(agent.id)}
                >
                  <div className="agent-item-header">
                    <input
                      type="radio"
                      className="agent-item-radio"
                      name="agent-select"
                      checked={isSelected}
                      onChange={() => setSelectedAgentId(agent.id)}
                    />
                    <span className="agent-item-name">{agent.name}</span>
                    <span className="agent-item-version">
                      v{agent.version}
                    </span>
                    {isInstalled ? (
                      <span className="agent-badge agent-badge-installed">
                        Installed
                      </span>
                    ) : canInstall ? (
                      <button
                        className="agent-badge agent-badge-install"
                        onClick={(e) => {
                          e.stopPropagation();
                          handleInstall(agent);
                        }}
                        disabled={isInstallingThis}
                      >
                        {isInstallingThis ? (
                          <>
                            <svg
                              className="w-3 h-3 animate-spin inline mr-1"
                              viewBox="0 0 24 24"
                              fill="none"
                            >
                              <circle
                                cx="12"
                                cy="12"
                                r="10"
                                stroke="currentColor"
                                strokeWidth="3"
                                className="opacity-25"
                              />
                              <path
                                d="M12 2a10 10 0 0 1 10 10"
                                stroke="currentColor"
                                strokeWidth="3"
                                strokeLinecap="round"
                              />
                            </svg>
                            Installing...
                          </>
                        ) : (
                          "Install"
                        )}
                      </button>
                    ) : (
                      <span className="agent-badge agent-badge-missing">
                        Not installed
                      </span>
                    )}
                  </div>

                  <div className="agent-item-description">
                    {agent.description}
                  </div>

                  {isSelected && agent.required_env.length > 0 && (
                    <div className="agent-env-section">
                      {agent.required_env.map((envVar) => (
                        <div key={envVar.name} className="agent-env-row">
                          <label className="agent-env-label">
                            {envVar.name}:
                          </label>
                          <input
                            type={envVar.is_secret ? "password" : "text"}
                            className="agent-env-input"
                            placeholder={envVar.description}
                            value={
                              agentEnvValues[agent.id]?.[envVar.name] || ""
                            }
                            onChange={(e) =>
                              updateEnvValue(
                                agent.id,
                                envVar.name,
                                e.target.value,
                              )
                            }
                            onBlur={(e) =>
                              saveEnvValue(
                                agent.id,
                                envVar.name,
                                e.target.value,
                              )
                            }
                            onClick={(e) => e.stopPropagation()}
                          />
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}

        {/* Session Config Options - shown when connected */}
        {status === "connected" && configOptions.length > 0 && (
          <div className="config-options-section">
            <div className="config-options-header">Session Config</div>
            {configOptions.map((opt) => {
              const allOptions =
                opt.select_options.type === "ungrouped"
                  ? opt.select_options.options
                  : opt.select_options.groups.flatMap((g) => g.options);

              return (
                <div key={opt.id} className="config-option-row">
                  <label className="config-option-label" title={opt.description || ""}>
                    {opt.name}
                  </label>
                  <select
                    className="config-option-select"
                    value={opt.current_value}
                    onChange={(e) =>
                      onSetConfigOption(opt.id, e.target.value)
                    }
                  >
                    {opt.select_options.type === "ungrouped"
                      ? allOptions.map((o) => (
                          <option key={o.value} value={o.value}>
                            {o.name}
                          </option>
                        ))
                      : opt.select_options.groups.map((g) => (
                          <optgroup key={g.group} label={g.name}>
                            {g.options.map((o) => (
                              <option key={o.value} value={o.value}>
                                {o.name}
                              </option>
                            ))}
                          </optgroup>
                        ))}
                  </select>
                </div>
              );
            })}
          </div>
        )}

        <div className="settings-footer">
          <label className="settings-label">
            <input
              type="checkbox"
              checked={autoFallback}
              onChange={(e) => setAutoFallback(e.target.checked)}
            />
            Auto-fallback to agent on zero results
          </label>

          <div className="settings-actions">
            {status === "connected" ? (
              <button
                className="settings-btn settings-btn-danger"
                onClick={onDisconnect}
              >
                Disconnect
              </button>
            ) : (
              <button
                className="settings-btn settings-btn-primary"
                onClick={handleConnect}
                disabled={status === "connecting" || !canConnect}
              >
                {status === "connecting" ? "Connecting..." : "Connect"}
              </button>
            )}
            <span className={`settings-status settings-status-${status}`}>
              {status}
            </span>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="agent-settings-overlay">
      <div className="agent-settings-panel">
        <div className="settings-header">
          <h3>Settings</h3>
          <button className="settings-close-btn" onClick={onClose}>
            &#x2715;
          </button>
        </div>

        <div className="settings-layout">
          {/* Sidebar navigation */}
          <nav className="settings-nav">
            {NAV_ITEMS.map((item) => (
              <button
                key={item.id}
                className={`settings-nav-item ${activeTab === item.id ? "settings-nav-item-active" : ""}`}
                onClick={() => setActiveTab(item.id)}
              >
                <svg
                  className="settings-nav-icon"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="1.5"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                >
                  <path d={item.icon} />
                </svg>
                <span>{item.label}</span>
              </button>
            ))}
          </nav>

          {/* Content area */}
          <div className="settings-content">
            {activeTab === "general" && renderGeneralSection()}
            {activeTab === "agent" && renderAgentSection()}
          </div>
        </div>
      </div>
    </div>
  );
}
