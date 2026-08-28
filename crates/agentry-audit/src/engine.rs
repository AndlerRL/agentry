use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use agentry_agents::all_agent_specs;
use agentry_core::models::{AgentSpec, DetectedAgent, UnifiedPrompt};

use crate::checks;
use crate::report::{AgentAudit, AuditFinding, AuditReport, AuditSummary, HealthGrade, Severity};

pub type VersionLookup = dyn Fn(&str, &str) -> Option<Vec<String>>;

pub struct CheckContext {
    pub home_dir: PathBuf,
    pub agents: Vec<DetectedAgent>,
    pub prompts: Vec<UnifiedPrompt>,
    pub version_lookup: Option<Box<VersionLookup>>,
    pub binary_on_path: Vec<String>,
}

pub fn build_context(home_dir: &Path, prompts: Vec<UnifiedPrompt>) -> CheckContext {
    let specs = all_agent_specs();
    let binary_on_path = specs
        .iter()
        .filter(|spec| which_finds(&spec.cli_binary))
        .map(|spec| spec.cli_binary.clone())
        .collect();
    let agents = specs
        .iter()
        .map(|spec| detected_from_spec(home_dir, spec))
        .collect();
    CheckContext {
        home_dir: home_dir.to_path_buf(),
        agents,
        prompts,
        version_lookup: None,
        binary_on_path,
    }
}

fn detected_from_spec(home_dir: &Path, spec: &AgentSpec) -> DetectedAgent {
    let config_dir = home_dir.join(&spec.config_dir);
    let config_dir_exists = config_dir.exists();
    let prompt_file_exists = config_dir.join(&spec.prompt_filename).exists();
    let skills_dir = spec
        .skills_dir_name
        .as_ref()
        .map(|name| config_dir.join(name))
        .filter(|path| path.exists());
    let installed_skills = skills_dir
        .as_deref()
        .map(read_dir_names)
        .unwrap_or_default();
    DetectedAgent {
        spec: spec.clone(),
        installed: config_dir_exists || prompt_file_exists,
        version: None,
        config_dir_exists,
        prompt_file_exists,
        skills_dir,
        skills_symlink_pattern: None,
        installed_skills,
        detected_methods: Vec::new(),
    }
}

fn read_dir_names(dir: &Path) -> Vec<String> {
    match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .flatten()
            .filter_map(|entry| entry.file_name().into_string().ok())
            .collect(),
        Err(_) => Vec::new(),
    }
}

fn which_finds(binary: &str) -> bool {
    std::process::Command::new("which")
        .arg(binary)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

pub fn health_score(findings: &[AuditFinding]) -> u8 {
    let deductions: usize = findings
        .iter()
        .map(|finding| match finding.severity {
            Severity::Critical => 25,
            Severity::Warning => 10,
            Severity::Info => 3,
            Severity::Suggestion => 1,
        })
        .sum();
    100usize.saturating_sub(deductions) as u8
}

pub fn grade_for(score: u8) -> HealthGrade {
    if score >= 90 {
        HealthGrade::Healthy
    } else if score >= 70 {
        HealthGrade::Degraded
    } else if score >= 40 {
        HealthGrade::Unhealthy
    } else {
        HealthGrade::Critical
    }
}

pub fn run_audit(ctx: &CheckContext) -> AuditReport {
    let findings = checks::run_all(ctx);
    let global_findings: Vec<AuditFinding> = findings
        .iter()
        .filter(|finding| finding.agent_id.is_none())
        .cloned()
        .collect();
    let agents: Vec<AgentAudit> = ctx
        .agents
        .iter()
        .filter(|detected| is_auditable(detected))
        .map(|detected| agent_audit(&findings, detected))
        .collect();
    let auditable_ids: Vec<&str> = agents.iter().map(|agent| agent.agent_id.as_str()).collect();
    let attached_findings: Vec<AuditFinding> = findings
        .iter()
        .filter(|finding| {
            finding
                .agent_id
                .as_deref()
                .is_none_or(|id| auditable_ids.contains(&id))
        })
        .cloned()
        .collect();
    let summary = build_summary(&attached_findings, &agents);
    AuditReport {
        generated_at: chrono::Utc::now(),
        machine_id: machine_id(),
        agents,
        global_findings,
        summary,
        schema_version: 1,
    }
}

fn is_auditable(detected: &DetectedAgent) -> bool {
    detected.installed || detected.config_dir_exists || detected.prompt_file_exists
}

fn agent_audit(findings: &[AuditFinding], detected: &DetectedAgent) -> AgentAudit {
    let findings: Vec<AuditFinding> = findings
        .iter()
        .filter(|finding| finding.agent_id.as_deref() == Some(detected.spec.id.as_str()))
        .cloned()
        .collect();
    let health_score = health_score(&findings);
    AgentAudit {
        agent_id: detected.spec.id.clone(),
        health_score,
        grade: grade_for(health_score),
        detected: detected.clone(),
        findings,
    }
}

fn build_summary(findings: &[AuditFinding], agents: &[AgentAudit]) -> AuditSummary {
    let mut by_severity = BTreeMap::new();
    let mut by_category = BTreeMap::new();
    for finding in findings {
        *by_severity.entry(finding.severity).or_insert(0) += 1;
        *by_category.entry(finding.category).or_insert(0) += 1;
    }
    AuditSummary {
        total_findings: findings.len(),
        by_severity,
        by_category,
        auto_fixable_count: findings.iter().filter(|f| f.auto_fixable).count(),
        healthy_agents: agents
            .iter()
            .filter(|agent| agent.grade == HealthGrade::Healthy)
            .count(),
        degraded_agents: agents
            .iter()
            .filter(|agent| agent.grade == HealthGrade::Degraded)
            .count(),
    }
}

pub fn machine_id() -> String {
    let hostname = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("HOST"))
        .unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    if !hostname.is_empty() {
        hostname.hash(&mut hasher);
    } else {
        dirs_home().hash(&mut hasher);
    }
    format!("{:016x}", hasher.finish())
}

fn dirs_home() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::FindingCategory;
    use agentry_core::models::{InstallMethod, PromptFormat};

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(prefix: &str) -> Self {
            let path = std::env::temp_dir().join(format!("{}_{}", prefix, std::process::id()));
            std::fs::create_dir_all(&path).expect("failed to create temp dir");
            Self { path }
        }

        fn path(&self) -> &PathBuf {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn spec(id: &str, config_dir: &str, prompt_filename: &str) -> AgentSpec {
        AgentSpec {
            id: id.to_string(),
            name: id.to_string(),
            cli_binary: id.to_string(),
            config_dir: config_dir.to_string(),
            prompt_filename: prompt_filename.to_string(),
            prompt_format: PromptFormat::PlainMd,
            skills_dir_name: None,
            max_size: None,
            install_methods: vec![InstallMethod::Npm {
                package: id.to_string(),
            }],
        }
    }

    fn agent(spec: AgentSpec) -> DetectedAgent {
        DetectedAgent {
            spec,
            installed: true,
            version: None,
            config_dir_exists: true,
            prompt_file_exists: true,
            skills_dir: None,
            skills_symlink_pattern: None,
            installed_skills: Vec::new(),
            detected_methods: Vec::new(),
        }
    }

    fn ctx(home: PathBuf, agents: Vec<DetectedAgent>, binary_on_path: Vec<String>) -> CheckContext {
        CheckContext {
            home_dir: home,
            agents,
            prompts: Vec::new(),
            version_lookup: None,
            binary_on_path,
        }
    }

    fn finding(severity: Severity) -> AuditFinding {
        AuditFinding {
            check_id: "test.check".to_string(),
            severity,
            category: FindingCategory::Installation,
            agent_id: Some("codex".to_string()),
            message: "test finding".to_string(),
            remediation: "fix it".to_string(),
            auto_fixable: false,
            fix: None,
            evidence: None,
        }
    }

    #[test]
    fn health_score_empty_findings_is_100() {
        assert_eq!(health_score(&[]), 100);
    }

    #[test]
    fn health_score_single_critical_deducts_25() {
        let findings = vec![finding(Severity::Critical)];
        assert_eq!(health_score(&findings), 75);
    }

    #[test]
    fn health_score_four_warnings_deduct_40() {
        let findings = vec![finding(Severity::Warning); 4];
        assert_eq!(health_score(&findings), 60);
    }

    #[test]
    fn health_score_caps_at_zero() {
        let findings = vec![finding(Severity::Critical); 5];
        assert_eq!(health_score(&findings), 0);
    }

    #[test]
    fn grade_for_matches_thresholds() {
        assert_eq!(grade_for(100), HealthGrade::Healthy);
        assert_eq!(grade_for(90), HealthGrade::Healthy);
        assert_eq!(grade_for(89), HealthGrade::Degraded);
        assert_eq!(grade_for(70), HealthGrade::Degraded);
        assert_eq!(grade_for(69), HealthGrade::Unhealthy);
        assert_eq!(grade_for(40), HealthGrade::Unhealthy);
        assert_eq!(grade_for(39), HealthGrade::Critical);
        assert_eq!(grade_for(0), HealthGrade::Critical);
    }

    #[test]
    fn run_audit_scores_two_agents_and_summarizes() {
        let tmp = TempDir::new("agentry_audit_engine_two_agents");
        let codex_dir = tmp.path().join(".codex");
        let gemini_dir = tmp.path().join(".gemini");
        std::fs::create_dir_all(&codex_dir).unwrap();
        std::fs::create_dir_all(&gemini_dir).unwrap();
        std::fs::write(codex_dir.join("AGENTS.md"), "a".repeat(32769)).unwrap();
        std::fs::write(gemini_dir.join("GEMINI.md"), "# GEMINI rules\n").unwrap();
        std::fs::write(gemini_dir.join("oauth_creds.json"), "{}").unwrap();
        let canonical_dir = tmp.path().join(".agents").join("prompts");
        std::fs::create_dir_all(&canonical_dir).unwrap();
        std::fs::write(canonical_dir.join("AGENTS.md"), "a".repeat(32769)).unwrap();
        std::fs::write(canonical_dir.join("GEMINI.md"), "# GEMINI rules\n").unwrap();

        let mut codex_spec = spec("codex", ".codex", "AGENTS.md");
        codex_spec.max_size = Some(32768);
        let agents = vec![
            agent(codex_spec),
            agent(spec("gemini-cli", ".gemini", "GEMINI.md")),
        ];
        let report = run_audit(&ctx(
            tmp.path().clone(),
            agents,
            vec!["codex".to_string(), "gemini-cli".to_string()],
        ));

        assert_eq!(report.agents.len(), 2);
        let codex_audit = report
            .agents
            .iter()
            .find(|a| a.agent_id == "codex")
            .expect("codex audit should exist");
        let gemini_audit = report
            .agents
            .iter()
            .find(|a| a.agent_id == "gemini-cli")
            .expect("gemini audit should exist");

        assert_eq!(codex_audit.findings.len(), 2);
        assert!(codex_audit
            .findings
            .iter()
            .any(|f| f.check_id == "prompt.oversized"));
        assert_eq!(codex_audit.health_score, 87);
        assert_eq!(codex_audit.grade, HealthGrade::Degraded);

        assert_eq!(gemini_audit.findings.len(), 0);
        assert_eq!(gemini_audit.health_score, 100);
        assert_eq!(gemini_audit.grade, HealthGrade::Healthy);

        assert_eq!(report.summary.total_findings, 2);
        assert_eq!(report.summary.by_severity[&Severity::Warning], 1);
        assert_eq!(report.summary.by_severity[&Severity::Info], 1);
        assert_eq!(report.summary.auto_fixable_count, 0);
        assert_eq!(report.summary.healthy_agents, 1);
        assert_eq!(report.summary.degraded_agents, 1);
        assert!(report.global_findings.is_empty());
    }

    #[test]
    fn run_audit_skips_fully_absent_agents() {
        let mut absent = agent(spec("mystery", ".mystery", "AGENTS.md"));
        absent.installed = false;
        absent.config_dir_exists = false;
        absent.prompt_file_exists = false;
        let tmp = TempDir::new("agentry_audit_engine_skip_absent");
        let report = run_audit(&ctx(tmp.path().clone(), vec![absent], Vec::new()));
        assert!(report.agents.is_empty());
    }

    #[test]
    fn run_audit_empty_machine_reports_zero_findings() {
        let tmp = TempDir::new("agentry_audit_engine_empty");
        let report = run_audit(&ctx(tmp.path().clone(), Vec::new(), Vec::new()));
        assert!(report.agents.is_empty());
        assert!(report.global_findings.is_empty());
        assert_eq!(report.summary.total_findings, 0);
        assert_eq!(report.summary.auto_fixable_count, 0);
        assert_eq!(report.summary.healthy_agents, 0);
        assert_eq!(report.schema_version, 1);
    }

    #[test]
    fn machine_id_is_stable_across_calls() {
        assert!(!machine_id().is_empty());
        assert_eq!(machine_id(), machine_id());
    }
}
