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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XmlTagWrap {
    pub tag: String,
    pub content: String,
}

/// Unified internal representation of a prompt from any agent format.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

impl DetectedAgent {
    pub fn status_label(&self) -> &'static str {
        if self.installed {
            "ON"
        } else {
            "OFF"
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