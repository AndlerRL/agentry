use std::path::Path;

use serde::{Deserialize, Serialize};

pub mod invoke;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostKind {
    AgentCli,
    LocalRuntime,
    ApiCli,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    Stdin,
    Argv,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostProfile {
    pub id: String,
    pub display_name: String,
    pub kind: HostKind,
    pub detect_binary: String,
    pub headless_command: Option<String>,
    pub model_argument: Option<String>,
    pub transport: Transport,
}

pub fn builtin_hosts() -> Vec<HostProfile> {
    vec![
        HostProfile {
            id: "claude-code".to_string(),
            display_name: "Claude Code".to_string(),
            kind: HostKind::AgentCli,
            detect_binary: "claude".to_string(),
            headless_command: Some("claude -p --output-format text".to_string()),
            model_argument: None,
            transport: Transport::Stdin,
        },
        HostProfile {
            id: "codex".to_string(),
            display_name: "Codex".to_string(),
            kind: HostKind::AgentCli,
            detect_binary: "codex".to_string(),
            headless_command: Some("codex exec -".to_string()),
            model_argument: None,
            transport: Transport::Stdin,
        },
        HostProfile {
            id: "gemini-cli".to_string(),
            display_name: "Gemini CLI".to_string(),
            kind: HostKind::AgentCli,
            detect_binary: "gemini".to_string(),
            headless_command: Some("gemini -p".to_string()),
            model_argument: None,
            transport: Transport::Stdin,
        },
        HostProfile {
            id: "zai".to_string(),
            display_name: "Z.ai GLM".to_string(),
            kind: HostKind::AgentCli,
            detect_binary: "zai".to_string(),
            headless_command: Some("zai -p".to_string()),
            model_argument: None,
            transport: Transport::Stdin,
        },
        HostProfile {
            id: "fal".to_string(),
            display_name: "fal.ai".to_string(),
            kind: HostKind::ApiCli,
            detect_binary: "fal".to_string(),
            headless_command: None,
            model_argument: None,
            transport: Transport::Stdin,
        },
        HostProfile {
            id: "ollama".to_string(),
            display_name: "Ollama".to_string(),
            kind: HostKind::LocalRuntime,
            detect_binary: "ollama".to_string(),
            headless_command: Some("ollama run {model}".to_string()),
            model_argument: Some("{model}".to_string()),
            transport: Transport::Stdin,
        },
    ]
}

pub fn default_priority() -> Vec<String> {
    vec![
        "claude-code".to_string(),
        "codex".to_string(),
        "gemini-cli".to_string(),
        "zai".to_string(),
        "ollama".to_string(),
    ]
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostsSection {
    #[serde(default)]
    pub priority: Vec<String>,
    #[serde(default, flatten)]
    pub overrides: std::collections::BTreeMap<String, HostOverride>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostOverride {
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub kind: Option<HostKind>,
    #[serde(default)]
    pub detect_binary: Option<String>,
    #[serde(default)]
    pub headless_command: Option<String>,
    #[serde(default)]
    pub model_argument: Option<String>,
    #[serde(default)]
    pub transport: Option<Transport>,
}

pub fn resolve_hosts(config: &HostsSection) -> Vec<HostProfile> {
    let mut by_id: std::collections::BTreeMap<String, HostProfile> = builtin_hosts()
        .into_iter()
        .map(|host| (host.id.clone(), host))
        .collect();
    for (id, override_) in &config.overrides {
        let entry = by_id.entry(id.clone()).or_insert_with(|| HostProfile {
            id: id.clone(),
            display_name: id.clone(),
            kind: HostKind::AgentCli,
            detect_binary: id.clone(),
            headless_command: None,
            model_argument: None,
            transport: Transport::Stdin,
        });
        if let Some(display_name) = &override_.display_name {
            entry.display_name = display_name.clone();
        }
        if let Some(kind) = override_.kind {
            entry.kind = kind;
        }
        if let Some(detect_binary) = &override_.detect_binary {
            entry.detect_binary = detect_binary.clone();
        }
        if let Some(headless_command) = &override_.headless_command {
            entry.headless_command = Some(headless_command.clone());
        }
        if let Some(model_argument) = &override_.model_argument {
            entry.model_argument = Some(model_argument.clone());
        }
        if let Some(transport) = override_.transport {
            entry.transport = transport;
        }
    }
    let mut ordered: Vec<HostProfile> = Vec::new();
    for id in &config.priority {
        if let Some(host) = by_id.remove(id) {
            ordered.push(host);
        }
    }
    ordered.extend(by_id.into_values());
    ordered
}

pub fn is_installed(host: &HostProfile) -> bool {
    binary_on_path(&host.detect_binary)
}

pub fn binary_on_path(binary: &str) -> bool {
    std::process::Command::new("which")
        .arg(binary)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

pub fn first_installed(hosts: &[HostProfile]) -> Option<&HostProfile> {
    hosts.iter().find(|host| is_installed(host))
}

pub fn host_by_id<'a>(hosts: &'a [HostProfile], id: &str) -> Option<&'a HostProfile> {
    hosts.iter().find(|host| host.id == id)
}

pub fn config_hosts(home_dir: &Path) -> HostsSection {
    let path = home_dir.join(".agents").join("agentry.toml");
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(_) => return HostsSection::default(),
    };
    toml::from_str(&content)
        .map(|config: crate::context::HarnessConfig| config.hosts)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_home(prefix: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("{}_{}", prefix, std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn builtins_cover_registry_ids() {
        let hosts = builtin_hosts();
        let ids: Vec<&str> = hosts.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(
            ids,
            ["claude-code", "codex", "gemini-cli", "zai", "fal", "ollama"]
        );
        let fal = hosts.iter().find(|h| h.id == "fal").unwrap();
        assert_eq!(fal.kind, HostKind::ApiCli);
        assert!(fal.headless_command.is_none());
        let ollama = hosts.iter().find(|h| h.id == "ollama").unwrap();
        assert_eq!(ollama.kind, HostKind::LocalRuntime);
        assert_eq!(
            ollama.headless_command.as_deref(),
            Some("ollama run {model}")
        );
    }

    #[test]
    fn default_priority_excludes_fal() {
        let priority = default_priority();
        assert!(!priority.contains(&"fal".to_string()));
        assert_eq!(priority[0], "claude-code");
        assert_eq!(priority[4], "ollama");
    }

    #[test]
    fn resolve_hosts_orders_by_priority() {
        let config = HostsSection {
            priority: vec!["ollama".to_string(), "claude-code".to_string()],
            overrides: std::collections::BTreeMap::new(),
        };
        let hosts = resolve_hosts(&config);
        assert_eq!(hosts[0].id, "ollama");
        assert_eq!(hosts[1].id, "claude-code");
        assert!(hosts.iter().any(|h| h.id == "fal"));
    }

    #[test]
    fn resolve_hosts_applies_overrides() {
        let mut overrides = std::collections::BTreeMap::new();
        overrides.insert(
            "zai".to_string(),
            HostOverride {
                headless_command: Some("zai -p --json".to_string()),
                ..Default::default()
            },
        );
        let config = HostsSection {
            priority: vec!["zai".to_string()],
            overrides,
        };
        let hosts = resolve_hosts(&config);
        assert_eq!(hosts[0].headless_command.as_deref(), Some("zai -p --json"));
    }

    #[test]
    fn resolve_hosts_adds_unknown_config_host() {
        let mut overrides = std::collections::BTreeMap::new();
        overrides.insert(
            "custom-cli".to_string(),
            HostOverride {
                headless_command: Some("custom-cli -p".to_string()),
                detect_binary: Some("custom-cli".to_string()),
                ..Default::default()
            },
        );
        let config = HostsSection {
            priority: vec!["custom-cli".to_string()],
            overrides,
        };
        let hosts = resolve_hosts(&config);
        assert_eq!(hosts[0].id, "custom-cli");
        assert_eq!(hosts[0].detect_binary, "custom-cli");
    }

    #[test]
    fn config_hosts_reads_priority_from_toml() {
        let home = temp_home("agentry_test_hosts_config");
        let path = home.join(".agents").join("agentry.toml");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "[hosts]\npriority = [\"codex\", \"ollama\"]\n").unwrap();
        let section = config_hosts(&home);
        assert_eq!(section.priority, vec!["codex", "ollama"]);
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn config_hosts_defaults_when_missing() {
        let home = temp_home("agentry_test_hosts_missing");
        let section = config_hosts(&home);
        assert!(section.priority.is_empty());
        std::fs::remove_dir_all(&home).unwrap();
    }
}
