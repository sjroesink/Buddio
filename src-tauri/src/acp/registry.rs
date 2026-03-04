use super::types::{RegistryAgent, RequiredEnvVar};
use serde_json::Value;
use std::collections::HashMap;

pub async fn fetch_registry() -> Result<Vec<RegistryAgent>, String> {
    let url = "https://cdn.agentclientprotocol.com/registry/v1/latest/registry.json";

    let response = reqwest::get(url)
        .await
        .map_err(|e| format!("Failed to fetch registry: {e}"))?;

    let payload: Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse registry JSON: {e}"))?;

    let entries: Vec<&Value> = match &payload {
        // Current registry shape: { "version": "...", "agents": [...] }
        Value::Object(map) => map
            .get("agents")
            .and_then(Value::as_array)
            .map(|agents| agents.iter().collect())
            .ok_or("Failed to parse registry JSON: missing 'agents' array".to_string())?,
        // Backward compatibility for old top-level array shape.
        Value::Array(items) => items.iter().collect(),
        _ => {
            return Err(
                "Failed to parse registry JSON: expected object or array at root".to_string(),
            )
        }
    };

    let platform_keys = detect_platform_keys();

    let agents: Vec<RegistryAgent> = entries
        .into_iter()
        .filter_map(|entry| {
            let id = entry.get("id")?.as_str()?.to_string();
            let name = entry
                .get("name")
                .and_then(Value::as_str)
                .map(ToString::to_string)
                .unwrap_or_else(|| id.clone());
            let version = entry
                .get("version")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let description = entry
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let icon = entry
                .get("icon")
                .and_then(Value::as_str)
                .map(ToString::to_string);

            let dist_info = extract_distribution(entry, &platform_keys);

            let required_env = resolve_env_vars(&id, entry);

            Some(RegistryAgent {
                id,
                name,
                version,
                description,
                icon,
                distribution_type: dist_info.dist_type,
                distribution_detail: dist_info.detail,
                distribution_args: dist_info.args,
                archive_url: dist_info.archive_url,
                required_env,
            })
        })
        .collect();

    Ok(agents)
}

/// Check whether an npm package is globally installed.
fn check_npm_package_installed(package: &str) -> bool {
    // Derive the unversioned package name to search for in `npm list -g` output.
    // Handles scoped packages like `@scope/name@version` and plain `name@version`.
    let search_name = if package.starts_with('@') {
        // Scoped: "@scope/pkg-name@version" → "@scope/pkg-name"
        package
            .split('@') // ["", "scope/pkg-name", "version"]
            .nth(1) // "scope/pkg-name"
            .map(|s| {
                // Re-attach the leading '@' that splitn stripped
                // We'll just search for the unscoped part after the '/'
                s.find('/').map(|i| &s[i + 1..]).unwrap_or(s)
            })
            .unwrap_or(package)
    } else {
        // Plain: "pkg-name@version" → "pkg-name"
        package.split('@').next().unwrap_or(package)
    };

    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", "npm list -g --depth=0"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output();

    #[cfg(not(target_os = "windows"))]
    let result = std::process::Command::new("npm")
        .args(["list", "-g", "--depth=0"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output();

    result
        .map(|output| {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout.contains(search_name)
        })
        .unwrap_or(false)
}

/// Check whether a binary agent has been installed to our local agents directory.
fn check_agent_installed_locally(agent_id: &str, binary_name: &str) -> bool {
    if let Some(data_dir) = dirs::data_local_dir() {
        let buddio_candidate = data_dir
            .join("Buddio")
            .join("agents")
            .join(agent_id)
            .join(binary_name);
        if buddio_candidate.exists() {
            return true;
        }

        let legacy_candidate = data_dir
            .join("GoLaunch")
            .join("agents")
            .join(agent_id)
            .join(binary_name);
        legacy_candidate.exists()
    } else {
        false
    }
}

/// Check whether a command is available on the system PATH.
pub fn check_command_available(cmd: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("where")
            .arg(cmd)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::process::Command::new("which")
            .arg(cmd)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

/// Check installation status for a list of agents by probing their CLIs.
pub fn check_agents_installed(agents: &[RegistryAgent]) -> HashMap<String, bool> {
    agents
        .iter()
        .map(|agent| {
            let available = match agent.distribution_type.as_str() {
                "npx" => {
                    // For npx agents, check if the package is globally installed.
                    // Just having `npx` on PATH doesn't mean the package is installed.
                    if agent.distribution_detail.is_empty() {
                        false
                    } else {
                        check_npm_package_installed(&agent.distribution_detail)
                    }
                }
                "binary" => {
                    if !agent.distribution_detail.is_empty() {
                        // Extract the binary name, stripping ./ prefix
                        let bin = agent
                            .distribution_detail
                            .split_whitespace()
                            .next()
                            .unwrap_or(&agent.distribution_detail);
                        let bin = bin.trim_start_matches("./").trim_start_matches(".\\");
                        // On Windows, also try without .exe (npm-installed binaries are .cmd)
                        let bin_no_ext = bin.strip_suffix(".exe").unwrap_or(bin);
                        check_command_available(bin)
                            || check_command_available(bin_no_ext)
                            || check_agent_installed_locally(&agent.id, bin)
                    } else {
                        false
                    }
                }
                _ => false,
            };
            (agent.id.clone(), available)
        })
        .collect()
}

/// Resolve required env vars for an agent by merging registry-provided
/// env metadata with a hardcoded lookup table for well-known agents.
fn resolve_env_vars(agent_id: &str, entry: &Value) -> Vec<RequiredEnvVar> {
    // Parse env vars from registry JSON if present
    let mut env_vars: Vec<RequiredEnvVar> = entry
        .get("env")
        .and_then(Value::as_array)
        .map(|vars| {
            vars.iter()
                .filter_map(|v| {
                    let name = v.get("name")?.as_str()?.to_string();
                    let description = v
                        .get("description")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let is_secret = v.get("secret").and_then(Value::as_bool).unwrap_or(false);
                    Some(RequiredEnvVar {
                        name,
                        description,
                        is_secret,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    // Merge with hardcoded known env vars (only add if not already present)
    let known = get_known_env_vars(agent_id);
    for known_var in known {
        if !env_vars.iter().any(|v| v.name == known_var.name) {
            env_vars.push(known_var);
        }
    }

    env_vars
}

/// Hardcoded lookup table for well-known agents and their required env vars.
fn get_known_env_vars(agent_id: &str) -> Vec<RequiredEnvVar> {
    let id = agent_id.to_lowercase();

    if id.contains("claude") || id.contains("anthropic") {
        vec![RequiredEnvVar {
            name: "ANTHROPIC_API_KEY".to_string(),
            description: "Anthropic API key".to_string(),
            is_secret: true,
        }]
    } else if id.contains("codex") || id.contains("openai") {
        vec![RequiredEnvVar {
            name: "OPENAI_API_KEY".to_string(),
            description: "OpenAI API key".to_string(),
            is_secret: true,
        }]
    } else if id.contains("gemini") || id.contains("google") {
        vec![RequiredEnvVar {
            name: "GOOGLE_API_KEY".to_string(),
            description: "Google API key".to_string(),
            is_secret: true,
        }]
    } else {
        vec![]
    }
}

fn detect_platform_keys() -> Vec<&'static str> {
    if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            vec!["darwin-aarch64", "macos-arm64", "darwin-arm64"]
        } else {
            vec!["darwin-x86_64", "macos-x64", "darwin-x64"]
        }
    } else if cfg!(target_os = "windows") {
        if cfg!(target_arch = "aarch64") {
            vec!["windows-aarch64"]
        } else {
            vec!["windows-x86_64", "windows-x64"]
        }
    } else if cfg!(target_arch = "aarch64") {
        vec!["linux-aarch64", "linux-arm64"]
    } else {
        vec!["linux-x86_64", "linux-x64"]
    }
}

struct DistributionInfo {
    dist_type: String,
    detail: String,
    args: Vec<String>,
    archive_url: String,
}

fn parse_string_array(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn extract_distribution(entry: &Value, platform_keys: &[&str]) -> DistributionInfo {
    let distribution = match entry.get("distribution").and_then(Value::as_object) {
        Some(dist) => dist,
        None => {
            return DistributionInfo {
                dist_type: "unknown".to_string(),
                detail: String::new(),
                args: vec![],
                archive_url: String::new(),
            }
        }
    };

    // Prefer binary distribution if available for this platform (more reliable than npx)
    if let Some(binary) = distribution.get("binary").and_then(Value::as_object) {
        if let Some(info) = extract_binary_platform(binary, platform_keys) {
            return info;
        }
    }

    // Fall back to npx
    if let Some(npx) = distribution.get("npx").and_then(Value::as_object) {
        let package = npx
            .get("package")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if !package.is_empty() {
            let args = npx.get("args").map(parse_string_array).unwrap_or_default();
            return DistributionInfo {
                dist_type: "npx".to_string(),
                detail: package,
                args,
                archive_url: String::new(),
            };
        }
    }

    DistributionInfo {
        dist_type: "unknown".to_string(),
        detail: String::new(),
        args: vec![],
        archive_url: String::new(),
    }
}

fn extract_binary_platform(
    binary: &serde_json::Map<String, Value>,
    platform_keys: &[&str],
) -> Option<DistributionInfo> {
    // Try matching platform keys first
    for key in platform_keys {
        if let Some(info) = parse_binary_entry(binary.get(*key)?) {
            return Some(info);
        }
    }

    // Fallback to first entry
    if let Some((_, fallback)) = binary.iter().next() {
        return parse_binary_entry(fallback);
    }

    None
}

fn parse_binary_entry(entry: &Value) -> Option<DistributionInfo> {
    let platform_entry = entry.as_object()?;
    let cmd = platform_entry
        .get("cmd")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let archive = platform_entry
        .get("archive")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let args = platform_entry
        .get("args")
        .map(parse_string_array)
        .unwrap_or_default();

    let detail = if !cmd.is_empty() {
        cmd
    } else {
        archive.clone()
    };

    Some(DistributionInfo {
        dist_type: "binary".to_string(),
        detail,
        args,
        archive_url: archive,
    })
}
