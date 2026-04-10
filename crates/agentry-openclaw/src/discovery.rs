use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Parsed OpenClaw configuration (from ~/.openclaw/openclaw.json).
/// Supports JSON5 format (comments, trailing commas).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenClawConfig {
    #[serde(default)]
    pub agents: AgentsConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentsConfig {
    #[serde(default)]
    pub defaults: AgentDefaults,
    #[serde(default)]
    pub list: Vec<AgentEntry>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentDefaults {
    #[serde(default)]
    pub workspace: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEntry {
    pub id: String,
    #[serde(default)]
    pub default: Option<bool>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub workspace: Option<String>,
    #[serde(default)]
    #[serde(rename = "agentDir")]
    pub agent_dir: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub identity: Option<serde_json::Value>,
    #[serde(default)]
    pub group_chat: Option<serde_json::Value>,
    #[serde(default)]
    pub sandbox: Option<serde_json::Value>,
    #[serde(default)]
    pub tools: Option<serde_json::Value>,
}

/// A discovered OpenClaw workspace with its docs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenClawWorkspace {
    pub id: String,
    pub name: String,
    pub workspace_path: PathBuf,
    pub model: Option<String>,
    pub is_default: bool,
    pub docs: Vec<WorkspaceDoc>,
    pub lobster_workflows: Vec<LobsterWorkflow>,
    pub has_agents_md: bool,
    pub has_soul_md: bool,
    pub has_tools_md: bool,
    pub has_identity_md: bool,
    pub has_memory_md: bool,
    pub has_user_md: bool,
}

/// A document file in a workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceDoc {
    pub name: String,
    pub path: PathBuf,
    pub doc_type: DocType,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocType {
    Agents,
    Soul,
    Tools,
    Identity,
    Memory,
    User,
    Heartbeat,
    Boot,
    Bootstrap,
    Other,
}

impl std::fmt::Display for DocType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DocType::Agents => write!(f, "AGENTS.md"),
            DocType::Soul => write!(f, "SOUL.md"),
            DocType::Tools => write!(f, "TOOLS.md"),
            DocType::Identity => write!(f, "IDENTITY.md"),
            DocType::Memory => write!(f, "MEMORY.md"),
            DocType::User => write!(f, "USER.md"),
            DocType::Heartbeat => write!(f, "HEARTBEAT.md"),
            DocType::Boot => write!(f, "BOOT.md"),
            DocType::Bootstrap => write!(f, "BOOTSTRAP.md"),
            DocType::Other => write!(f, "Other"),
        }
    }
}

/// A .lobster workflow file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LobsterWorkflow {
    pub name: String,
    pub path: PathBuf,
}

/// Discover all OpenClaw workspaces from the config.
pub fn discover_workspaces(home_dir: &Path) -> Result<Vec<OpenClawWorkspace>> {
    let config_path = home_dir.join(".openclaw").join("openclaw.json");

    if !config_path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read {}", config_path.display()))?;

    // Try to parse as JSON (strip comments for JSON5 support)
    let config: OpenClawConfig = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse {}", config_path.display()))?;

    let mut workspaces = Vec::new();
    let default_workspace = config.agents.defaults.workspace.as_deref();

    for entry in &config.agents.list {
        let workspace_path = entry
            .workspace
            .as_deref()
            .or(default_workspace)
            .unwrap_or("~/.openclaw/workspace");

        // Expand ~ to home dir
        let workspace_path = expand_tilde(workspace_path, home_dir);
        let workspace_name = entry.name.as_deref().unwrap_or(&entry.id);

        let mut ws = OpenClawWorkspace {
            id: entry.id.clone(),
            name: workspace_name.to_string(),
            workspace_path: workspace_path.clone(),
            model: entry.model.clone().or(config.agents.defaults.model.clone()),
            is_default: entry.default.unwrap_or(false),
            docs: Vec::new(),
            lobster_workflows: Vec::new(),
            has_agents_md: false,
            has_soul_md: false,
            has_tools_md: false,
            has_identity_md: false,
            has_memory_md: false,
            has_user_md: false,
        };

        // Scan workspace for docs and workflows
        if workspace_path.is_dir() {
            scan_workspace(&mut ws);
        }

        workspaces.push(ws);
    }

    // If no agents list but default workspace exists
    if config.agents.list.is_empty() {
        let default_ws = default_workspace.unwrap_or("~/.openclaw/workspace");
        let ws_path = expand_tilde(default_ws, home_dir);
        if ws_path.is_dir() {
            let mut ws = OpenClawWorkspace {
                id: "default".to_string(),
                name: "Default".to_string(),
                workspace_path: ws_path.clone(),
                model: config.agents.defaults.model.clone(),
                is_default: true,
                docs: Vec::new(),
                lobster_workflows: Vec::new(),
                has_agents_md: false,
                has_soul_md: false,
                has_tools_md: false,
                has_identity_md: false,
                has_memory_md: false,
                has_user_md: false,
            };
            scan_workspace(&mut ws);
            workspaces.push(ws);
        }
    }

    Ok(workspaces)
}

/// Check if OpenClaw CLI is installed.
pub fn is_openclaw_installed() -> bool {
    std::process::Command::new("openclaw")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Expand ~ in paths.
fn expand_tilde(path: &str, home_dir: &Path) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        home_dir.join(rest)
    } else if let Some(rest) = path.strip_prefix('~') {
        home_dir.join(rest)
    } else {
        PathBuf::from(path)
    }
}

/// Scan a workspace directory for docs and .lobster workflows.
fn scan_workspace(ws: &mut OpenClawWorkspace) {
    let known_docs = [
        ("AGENTS.md", DocType::Agents),
        ("SOUL.md", DocType::Soul),
        ("TOOLS.md", DocType::Tools),
        ("IDENTITY.md", DocType::Identity),
        ("MEMORY.md", DocType::Memory),
        ("USER.md", DocType::User),
        ("HEARTBEAT.md", DocType::Heartbeat),
        ("BOOT.md", DocType::Boot),
        ("BOOTSTRAP.md", DocType::Bootstrap),
    ];

    for (filename, doc_type) in &known_docs {
        let path = ws.workspace_path.join(filename);
        if path.exists() {
            let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            ws.docs.push(WorkspaceDoc {
                name: filename.to_string(),
                path: path.clone(),
                doc_type: *doc_type,
                size_bytes: size,
            });
            match doc_type {
                DocType::Agents => ws.has_agents_md = true,
                DocType::Soul => ws.has_soul_md = true,
                DocType::Tools => ws.has_tools_md = true,
                DocType::Identity => ws.has_identity_md = true,
                DocType::Memory => ws.has_memory_md = true,
                DocType::User => ws.has_user_md = true,
                _ => {}
            }
        }
    }

    // Also scan for .lobster files
    if let Ok(entries) = std::fs::read_dir(&ws.workspace_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == "lobster" {
                    let name = path
                        .file_stem()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    ws.lobster_workflows.push(LobsterWorkflow { name, path });
                }
            }
        }
    }

    // Also check memory/ directory
    let memory_dir = ws.workspace_path.join("memory");
    if memory_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&memory_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("md") {
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();
                    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                    ws.docs.push(WorkspaceDoc {
                        name: format!("memory/{}", name),
                        path,
                        doc_type: DocType::Other,
                        size_bytes: size,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_tilde() {
        let home = PathBuf::from("/home/user");
        assert_eq!(
            expand_tilde("~/.openclaw/workspace", &home),
            PathBuf::from("/home/user/.openclaw/workspace")
        );
        assert_eq!(
            expand_tilde("/absolute/path", &home),
            PathBuf::from("/absolute/path")
        );
    }

    #[test]
    fn test_discover_workspaces_no_config() {
        let tmp = std::env::temp_dir().join("agentry_test_oc_noconfig");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let result = discover_workspaces(&tmp).unwrap();
        assert!(result.is_empty());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_doc_type_display() {
        assert_eq!(format!("{}", DocType::Agents), "AGENTS.md");
        assert_eq!(format!("{}", DocType::Soul), "SOUL.md");
        assert_eq!(format!("{}", DocType::Tools), "TOOLS.md");
    }
}
