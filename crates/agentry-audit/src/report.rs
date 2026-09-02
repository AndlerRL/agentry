use std::collections::BTreeMap;
use std::path::PathBuf;

use agentry_core::models::DetectedAgent;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Critical,
    Warning,
    Info,
    Suggestion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingCategory {
    Installation,
    Version,
    Config,
    PromptFile,
    SyncDrift,
    CrossAgentDrift,
    Skills,
    Auth,
    OrphanedFiles,
    OpenClaw,
    Acp,
    Audited,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditFinding {
    pub check_id: String,
    pub severity: Severity,
    pub category: FindingCategory,
    pub agent_id: Option<String>,
    pub message: String,
    pub remediation: String,
    pub auto_fixable: bool,
    pub fix: Option<FixAction>,
    #[serde(default)]
    pub suggested_fix: Option<FixAction>,
    pub evidence: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FixAction {
    ShellCommand {
        description: String,
        command: String,
    },
    FileWrite {
        path: PathBuf,
        content: String,
    },
    FileRemove {
        path: PathBuf,
    },
    SymlinkRecreate {
        path: PathBuf,
        target: String,
    },
    SyncPrompt {
        prompt_id: String,
        agent_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentAudit {
    pub agent_id: String,
    pub health_score: u8,
    pub grade: HealthGrade,
    pub detected: DetectedAgent,
    pub findings: Vec<AuditFinding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthGrade {
    Healthy,
    Degraded,
    Unhealthy,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditSummary {
    pub total_findings: usize,
    pub by_severity: BTreeMap<Severity, usize>,
    pub by_category: BTreeMap<FindingCategory, usize>,
    pub auto_fixable_count: usize,
    pub healthy_agents: usize,
    pub degraded_agents: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditReport {
    pub generated_at: DateTime<Utc>,
    pub machine_id: String,
    pub agents: Vec<AgentAudit>,
    pub global_findings: Vec<AuditFinding>,
    pub summary: AuditSummary,
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
}

fn default_schema_version() -> u32 {
    2
}
