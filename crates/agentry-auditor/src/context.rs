use std::path::{Path, PathBuf};

use agentry_audit::report::{AuditFinding, AuditReport, Severity};

pub const CONTEXT_BUDGET_BYTES: usize = 32 * 1024;
pub const EXCERPT_MAX_BYTES: usize = 4 * 1024;
pub const EXCERPT_MAX_FILES: usize = 8;

#[derive(Debug, Clone)]
pub struct FileExcerpt {
    pub path: PathBuf,
    pub withheld: bool,
    pub content: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AuditorContext {
    pub report: AuditReport,
    pub focus: Option<AuditFinding>,
    pub excerpts: Vec<FileExcerpt>,
    pub skills_inventory: Vec<String>,
}

fn credential_shaped(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    name == "auth.json"
        || name == ".env"
        || name.ends_with(".token")
        || name.ends_with("_creds.json")
        || name.ends_with("_credentials.json")
        || name.contains("oauth")
        || name.contains("secret")
        || name.contains("credential")
}

fn excerpt_file(path: &Path) -> FileExcerpt {
    if credential_shaped(path) {
        return FileExcerpt {
            path: path.to_path_buf(),
            withheld: true,
            content: None,
        };
    }
    let content = std::fs::read_to_string(path).ok();
    let content = content.map(|text| {
        let mut truncated = text;
        truncated.truncate(EXCERPT_MAX_BYTES);
        truncated
    });
    FileExcerpt {
        path: path.to_path_buf(),
        withheld: content.is_none(),
        content,
    }
}

fn severity_rank(severity: Severity) -> u8 {
    match severity {
        Severity::Critical => 0,
        Severity::Warning => 1,
        Severity::Info => 2,
        Severity::Suggestion => 3,
    }
}

pub fn package(
    report: AuditReport,
    focus: Option<AuditFinding>,
    excerpt_paths: &[PathBuf],
    skills_inventory: Vec<String>,
) -> AuditorContext {
    let mut excerpts: Vec<FileExcerpt> = excerpt_paths
        .iter()
        .take(EXCERPT_MAX_FILES)
        .map(|path| excerpt_file(path))
        .collect();
    let mut budget_used: usize = excerpts
        .iter()
        .map(|excerpt| {
            excerpt
                .content
                .as_ref()
                .map(|content| content.len())
                .unwrap_or(0)
        })
        .sum();
    while budget_used > CONTEXT_BUDGET_BYTES {
        let Some(largest) = excerpts
            .iter_mut()
            .filter(|excerpt| excerpt.content.is_some())
            .max_by_key(|excerpt| excerpt.content.as_ref().map(|c| c.len()).unwrap_or(0))
        else {
            break;
        };
        let Some(content) = largest.content.as_mut() else {
            break;
        };
        let excess = budget_used.saturating_sub(CONTEXT_BUDGET_BYTES);
        let cut = content.len().saturating_sub(excess).max(1);
        content.truncate(cut);
        budget_used = budget_used.saturating_sub(excess);
    }
    AuditorContext {
        report,
        focus,
        excerpts,
        skills_inventory,
    }
}

pub fn prioritized_findings(report: &AuditReport) -> Vec<AuditFinding> {
    let mut findings: Vec<AuditFinding> = report
        .agents
        .iter()
        .flat_map(|agent| agent.findings.iter().cloned())
        .chain(report.global_findings.iter().cloned())
        .collect();
    findings.sort_by_key(|finding| severity_rank(finding.severity));
    findings
}

pub fn auth_findings_status_only(report: &AuditReport) -> Vec<&AuditFinding> {
    report
        .agents
        .iter()
        .flat_map(|agent| agent.findings.iter())
        .chain(report.global_findings.iter())
        .filter(|finding| finding.category == agentry_audit::report::FindingCategory::Auth)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentry_audit::report::{FindingCategory, HealthGrade};

    fn temp_home(prefix: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("{}_{}", prefix, std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn finding(check_id: &str, severity: Severity) -> AuditFinding {
        AuditFinding {
            check_id: check_id.to_string(),
            severity,
            category: FindingCategory::Config,
            agent_id: None,
            message: "m".to_string(),
            remediation: "r".to_string(),
            auto_fixable: false,
            fix: None,
            suggested_fix: None,
            evidence: None,
        }
    }

    fn report_with(findings: Vec<AuditFinding>) -> AuditReport {
        use agentry_audit::report::{AgentAudit, AuditSummary};
        AuditReport {
            generated_at: chrono::Utc::now(),
            machine_id: "m".to_string(),
            agents: vec![AgentAudit {
                agent_id: "codex".to_string(),
                health_score: 100,
                grade: HealthGrade::Healthy,
                detected: agentry_core::models::DetectedAgent {
                    spec: agentry_core::models::AgentSpec {
                        id: "codex".to_string(),
                        name: "codex".to_string(),
                        cli_binary: "codex".to_string(),
                        config_dir: ".codex".to_string(),
                        prompt_filename: "AGENTS.md".to_string(),
                        prompt_format: agentry_core::models::PromptFormat::PlainMd,
                        skills_dir_name: None,
                        max_size: None,
                        install_methods: vec![],
                    },
                    installed: true,
                    version: None,
                    config_dir_exists: true,
                    prompt_file_exists: true,
                    skills_dir: None,
                    skills_symlink_pattern: None,
                    installed_skills: vec![],
                    detected_methods: vec![],
                },
                findings,
            }],
            global_findings: vec![],
            summary: AuditSummary {
                total_findings: 0,
                by_severity: std::collections::BTreeMap::new(),
                by_category: std::collections::BTreeMap::new(),
                auto_fixable_count: 0,
                healthy_agents: 1,
                degraded_agents: 0,
            },
            schema_version: 2,
        }
    }

    #[test]
    fn credential_shaped_paths_are_withheld() {
        let home = temp_home("agentry_test_ctx_creds");
        let auth = home.join("auth.json");
        let env = home.join(".env");
        let token = home.join("gh.token");
        std::fs::write(&auth, "{\"token\":\"secret\"}").unwrap();
        std::fs::write(&env, "API_KEY=secret").unwrap();
        std::fs::write(&token, "secret").unwrap();
        let ctx = package(report_with(vec![]), None, &[auth, env, token], vec![]);
        assert!(ctx.excerpts.iter().all(|e| e.withheld));
        assert!(ctx.excerpts.iter().all(|e| e.content.is_none()));
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn excerpts_are_truncated_to_budget() {
        let home = temp_home("agentry_test_ctx_budget");
        let big = home.join("big.md");
        std::fs::write(&big, "x".repeat(20 * 1024)).unwrap();
        let ctx = package(report_with(vec![]), None, &[big], vec![]);
        let total: usize = ctx
            .excerpts
            .iter()
            .map(|e| e.content.as_ref().map(|c| c.len()).unwrap_or(0))
            .sum();
        assert!(total <= CONTEXT_BUDGET_BYTES);
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn excerpt_count_capped_at_eight() {
        let home = temp_home("agentry_test_ctx_cap");
        let paths: Vec<PathBuf> = (0..12)
            .map(|i| {
                let path = home.join(format!("f{i}.md"));
                std::fs::write(&path, "x").unwrap();
                path
            })
            .collect();
        let ctx = package(report_with(vec![]), None, &paths, vec![]);
        assert_eq!(ctx.excerpts.len(), EXCERPT_MAX_FILES);
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn prioritized_findings_orders_critical_first() {
        let report = report_with(vec![
            finding("info", Severity::Info),
            finding("critical", Severity::Critical),
            finding("warning", Severity::Warning),
        ]);
        let ordered = prioritized_findings(&report);
        assert_eq!(ordered[0].check_id, "critical");
        assert_eq!(ordered[1].check_id, "warning");
        assert_eq!(ordered[2].check_id, "info");
    }
}
