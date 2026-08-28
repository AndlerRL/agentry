use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// The format a prompt file uses on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptFormat {
    /// Plain markdown: CLAUDE.md, AGENTS.md, GEMINI.md, SOUL.md, TOOLS.md
    PlainMd,
    /// YAML frontmatter + markdown body: Continue prompts/rules, OpenCode agents
    FrontmatterMd,
    /// Firebender .mdc files
    Mdc,
    /// Markdown with XML wrapper tags: Continue <expertise>, <base_rules>
    XmlTagMd,
    /// OpenClaw .lobster YAML workflows
    LobsterYaml,
}

impl std::fmt::Display for PromptFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PromptFormat::PlainMd => write!(f, "Plain Markdown"),
            PromptFormat::FrontmatterMd => write!(f, "Frontmatter+MD"),
            PromptFormat::Mdc => write!(f, "MDC"),
            PromptFormat::XmlTagMd => write!(f, "XML Tag+MD"),
            PromptFormat::LobsterYaml => write!(f, "Lobster YAML"),
        }
    }
}

/// How an agent was or can be installed on the system.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InstallMethod {
    Brew {
        formula: String,
        cask: bool,
    },
    Npm {
        package: String,
    },
    Cargo {
        crate_name: String,
    },
    Pip {
        package: String,
    },
    VsCodeExtension {
        extension_id: String,
    },
    JetBrainsPlugin {
        plugin_id: String,
    },
    DirectDownload {
        url: String,
        binary_name: String,
    },
    AppBundle {
        app_name: String,
    },
    BuiltIn,
    Other {
        description: String,
        install_cmd: String,
    },
}

impl InstallMethod {
    /// Human-readable label for display.
    pub fn label(&self) -> &'static str {
        match self {
            InstallMethod::Brew { cask, .. } if *cask => "Homebrew Cask",
            InstallMethod::Brew { .. } => "Homebrew",
            InstallMethod::Npm { .. } => "npm",
            InstallMethod::Cargo { .. } => "Cargo",
            InstallMethod::Pip { .. } => "pip",
            InstallMethod::VsCodeExtension { .. } => "VS Code Ext",
            InstallMethod::JetBrainsPlugin { .. } => "JetBrains Plugin",
            InstallMethod::DirectDownload { .. } => "Direct Download",
            InstallMethod::AppBundle { .. } => "macOS App",
            InstallMethod::BuiltIn => "Built-in",
            InstallMethod::Other { .. } => "Other",
        }
    }

    /// Compact key for badge display (e.g. "brew", "npm").
    pub fn method_key(&self) -> &'static str {
        match self {
            InstallMethod::Brew { .. } => "brew",
            InstallMethod::Npm { .. } => "npm",
            InstallMethod::Cargo { .. } => "cargo",
            InstallMethod::Pip { .. } => "pip",
            InstallMethod::VsCodeExtension { .. } => "vscode",
            InstallMethod::JetBrainsPlugin { .. } => "jb",
            InstallMethod::DirectDownload { .. } => "dl",
            InstallMethod::AppBundle { .. } => "app",
            InstallMethod::BuiltIn => "builtin",
            InstallMethod::Other { .. } => "other",
        }
    }

    /// Whether this install method is available on the current OS.
    pub fn available_on_os(&self) -> bool {
        match self {
            InstallMethod::Brew { .. } => cfg!(any(target_os = "macos", target_os = "linux")),
            InstallMethod::AppBundle { .. } => cfg!(target_os = "macos"),
            InstallMethod::Npm { .. } => which_exists("npm"),
            InstallMethod::Cargo { .. } => which_exists("cargo"),
            InstallMethod::Pip { .. } => which_exists("pip3") || which_exists("pip"),
            InstallMethod::VsCodeExtension { .. } => std::env::var("HOME")
                .map(|h| {
                    std::path::PathBuf::from(h)
                        .join(".vscode")
                        .join("extensions")
                        .exists()
                })
                .unwrap_or(false),
            InstallMethod::JetBrainsPlugin { .. } => cfg!(any(
                target_os = "macos",
                target_os = "linux",
                target_os = "windows"
            )),
            InstallMethod::DirectDownload { .. }
            | InstallMethod::BuiltIn
            | InstallMethod::Other { .. } => true,
        }
    }

    /// Shell command to install this package (None = latest, Some = specific version).
    pub fn install_command(&self, version: Option<&str>) -> String {
        match self {
            InstallMethod::Brew { formula, cask } => {
                let flag = if *cask { " --cask" } else { "" };
                format!("brew install{flag} {formula}")
            }
            InstallMethod::Npm { package } => {
                if let Some(v) = version {
                    format!("npm install -g {package}@{v}")
                } else {
                    format!("npm install -g {package}")
                }
            }
            InstallMethod::Cargo { crate_name } => {
                if let Some(v) = version {
                    format!("cargo install {crate_name} --version {v}")
                } else {
                    format!("cargo install {crate_name}")
                }
            }
            InstallMethod::Pip { package } => {
                let pip = if which_exists("pip3") { "pip3" } else { "pip" };
                if let Some(v) = version {
                    format!("{pip} install {package}=={v}")
                } else {
                    format!("{pip} install {package}")
                }
            }
            InstallMethod::VsCodeExtension { extension_id } => {
                format!("code --install-extension {extension_id}")
            }
            InstallMethod::DirectDownload { url, .. } => {
                format!("curl -fsSL {url} | sh")
            }
            InstallMethod::Other { install_cmd, .. } => install_cmd.clone(),
            _ => "echo 'No automatic install available'".to_string(),
        }
    }

    /// Shell command to update this package.
    pub fn update_command(&self) -> String {
        match self {
            InstallMethod::Brew { formula, cask } => {
                let flag = if *cask { " --cask" } else { "" };
                format!("brew upgrade{flag} {formula}")
            }
            InstallMethod::Npm { package } => format!("npm update -g {package}"),
            InstallMethod::Cargo { crate_name } => format!("cargo install --force {crate_name}"),
            InstallMethod::Pip { package } => {
                let pip = if which_exists("pip3") { "pip3" } else { "pip" };
                format!("{pip} install --upgrade {package}")
            }
            InstallMethod::VsCodeExtension { extension_id } => {
                format!("code --install-extension {extension_id} --force")
            }
            _ => "echo 'No automatic update available'".to_string(),
        }
    }

    /// Shell command to remove/uninstall this package.
    pub fn remove_command(&self) -> String {
        match self {
            InstallMethod::Brew { formula, cask } => {
                let flag = if *cask { " --cask" } else { "" };
                format!("brew uninstall{flag} {formula}")
            }
            InstallMethod::Npm { package } => format!("npm uninstall -g {package}"),
            InstallMethod::Cargo { crate_name } => format!("brew uninstall {crate_name}"),
            InstallMethod::Pip { package } => {
                let pip = if which_exists("pip3") { "pip3" } else { "pip" };
                format!("{pip} uninstall -y {package}")
            }
            InstallMethod::VsCodeExtension { extension_id } => {
                format!("code --uninstall-extension {extension_id}")
            }
            _ => "echo 'No automatic remove available'".to_string(),
        }
    }

    /// Shell command to list available versions. Returns None if not supported.
    pub fn list_versions_command(&self) -> Option<String> {
        match self {
            InstallMethod::Brew { formula, cask } if !cask => {
                Some(format!("brew info --json=v2 {formula}"))
            }
            InstallMethod::Npm { package } => Some(format!("npm view {package} versions --json")),
            InstallMethod::Cargo { crate_name } => {
                Some(format!("cargo search {crate_name} --limit 1"))
            }
            InstallMethod::Pip { package } => {
                let pip = if which_exists("pip3") { "pip3" } else { "pip" };
                Some(format!("{pip} index versions {package} 2>/dev/null"))
            }
            _ => None,
        }
    }

    /// Identifier string (formula name, package name, etc.) for display.
    pub fn identifier(&self) -> &str {
        match self {
            InstallMethod::Brew { formula, .. } => formula.as_str(),
            InstallMethod::Npm { package } => package.as_str(),
            InstallMethod::Cargo { crate_name } => crate_name.as_str(),
            InstallMethod::Pip { package } => package.as_str(),
            InstallMethod::VsCodeExtension { extension_id } => extension_id.as_str(),
            InstallMethod::JetBrainsPlugin { plugin_id } => plugin_id.as_str(),
            InstallMethod::DirectDownload { binary_name, .. } => binary_name.as_str(),
            InstallMethod::AppBundle { app_name } => app_name.as_str(),
            InstallMethod::BuiltIn => "system",
            InstallMethod::Other { description, .. } => description.as_str(),
        }
    }
}

impl std::fmt::Display for InstallMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// Check if a binary exists on PATH.
fn which_exists(binary: &str) -> bool {
    std::env::var("PATH")
        .unwrap_or_default()
        .split(':')
        .any(|dir| {
            let path = std::path::PathBuf::from(dir).join(binary);
            path.exists()
        })
}

/// Scope of a prompt: global (user-wide) or project-specific.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptScope {
    Global,
    Project { root: PathBuf },
}

impl std::fmt::Display for PromptScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PromptScope::Global => write!(f, "Global"),
            PromptScope::Project { root } => write!(f, "Project({})", root.display()),
        }
    }
}

/// An XML tag wrapper used in Continue format (e.g. `<expertise>...</expertise>`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XmlTagWrap {
    pub tag: String,
    pub content: String,
}

/// Unified internal representation of a prompt from any agent format.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnifiedPrompt {
    pub id: String,
    pub name: String,
    pub description: String,
    /// YAML frontmatter fields (agent-specific)
    pub frontmatter: BTreeMap<String, serde_yaml::Value>,
    /// Markdown body content
    pub body: String,
    /// XML tag wrappers (Continue format)
    pub xml_tags: Vec<XmlTagWrap>,
    pub scope: PromptScope,
    pub source_format: PromptFormat,
    /// Original file path on disk
    pub source_path: Option<PathBuf>,
}

impl UnifiedPrompt {
    /// Generate the file name for this prompt in the canonical store.
    pub fn canonical_filename(&self) -> String {
        format!("{}.md", self.name)
    }
}

/// An agent that has been detected on the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedAgent {
    pub spec: AgentSpec,
    pub installed: bool,
    pub version: Option<String>,
    pub config_dir_exists: bool,
    pub prompt_file_exists: bool,
    pub skills_dir: Option<PathBuf>,
    pub skills_symlink_pattern: Option<String>,
    pub installed_skills: Vec<String>,
    /// Which install methods were detected on this system.
    #[serde(default)]
    pub detected_methods: Vec<InstallMethod>,
}

impl DetectedAgent {
    pub fn status_label(&self) -> &'static str {
        if self.installed {
            "ON"
        } else {
            "OFF"
        }
    }

    /// Returns a comma-separated list of detected install method keys.
    pub fn detected_method_keys(&self) -> String {
        if self.detected_methods.is_empty() {
            String::new()
        } else {
            self.detected_methods
                .iter()
                .map(|m| m.method_key().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        }
    }
}

/// Static specification of a known agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSpec {
    pub id: String,
    pub name: String,
    pub cli_binary: String,
    pub config_dir: String,
    pub prompt_filename: String,
    pub prompt_format: PromptFormat,
    pub skills_dir_name: Option<String>,
    /// Max prompt file size in bytes (None = no limit)
    pub max_size: Option<usize>,
    /// Known install methods for this agent (ordered by preference).
    #[serde(default)]
    pub install_methods: Vec<InstallMethod>,
}

/// A skill entry from the skill hub.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillEntry {
    pub name: String,
    pub description: String,
    pub source_repo: String,
    pub installed: bool,
    pub version_hash: Option<String>,
    pub install_path: Option<PathBuf>,
    pub skill_md_path: Option<PathBuf>,
}

/// Status of a sync target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncStatus {
    UpToDate,
    Missing,
    Outdated,
    Conflict,
}

impl std::fmt::Display for SyncStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncStatus::UpToDate => write!(f, "Up to date"),
            SyncStatus::Missing => write!(f, "Missing"),
            SyncStatus::Outdated => write!(f, "Outdated"),
            SyncStatus::Conflict => write!(f, "Conflict"),
        }
    }
}

/// Action to take when syncing a prompt to an agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncAction {
    /// Copy with format conversion
    Copy,
    /// Create a relative symlink
    Symlink,
    /// This is the source — skip
    Source,
    /// Skip this agent
    Skip,
}

/// A single mapping in a sync plan: one prompt → one agent destination.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncMapping {
    pub prompt_id: String,
    pub agent_id: String,
    pub destination: PathBuf,
    pub target_format: PromptFormat,
    pub action: SyncAction,
    pub status: SyncStatus,
}

/// A complete sync plan for one prompt across all agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncPlan {
    pub prompt_id: String,
    pub mappings: Vec<SyncMapping>,
}

/// App configuration stored at ~/.agents/agentry.toml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_project_dirs")]
    pub project_dirs: Vec<PathBuf>,
    #[serde(default)]
    pub sync_defaults: SyncDefaults,
    #[serde(default)]
    pub extra_skill_sources: Vec<String>,
}

fn default_project_dirs() -> Vec<PathBuf> {
    vec![PathBuf::from("~/Development")]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncDefaults {
    #[serde(default = "default_true")]
    pub dry_run: bool,
    #[serde(default)]
    pub conflict_strategy: ConflictStrategy,
}

impl Default for SyncDefaults {
    fn default() -> Self {
        Self {
            dry_run: true,
            conflict_strategy: ConflictStrategy::Overwrite,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConflictStrategy {
    #[default]
    Overwrite,
    Keep,
    Merge,
    Diff,
}

fn default_true() -> bool {
    true
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            project_dirs: default_project_dirs(),
            sync_defaults: SyncDefaults::default(),
            extra_skill_sources: Vec::new(),
        }
    }
}
