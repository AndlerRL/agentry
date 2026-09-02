use std::collections::{HashMap, HashSet};
use std::fs::OpenOptions;
use std::io;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::engine::build_summary;
use crate::report::{AuditFinding, AuditReport, FindingCategory, Severity};

const RECURRING_CONSECUTIVE_RUNS: usize = 3;
const DORMANT_WINDOW_RUNS: usize = 10;
const NEW_CHECK_CANDIDATE_CHECK_ID: &str = "audit.new_check_candidate";

const DEFAULT_CHECK_IDS: [&str; 24] = [
    "install.binary_missing",
    "install.config_dir_missing",
    "install.method_conflict",
    "version.unparseable",
    "version.outdated",
    "version.latest_unknown",
    "config.unparseable",
    "config.stale",
    "prompt.missing",
    "prompt.empty",
    "prompt.oversized",
    "prompt.frontmatter_invalid",
    "prompt.format_mismatch",
    "sync.drift",
    "sync.missing",
    "drift.cross_agent",
    "skills.symlink_broken",
    "skills.orphaned",
    "skills.hash_mismatch",
    "auth.not_logged_in",
    "files.orphaned_prompt",
    "openclaw.lobster_invalid",
    "openclaw.workspace_incomplete",
    "acp.capability_mismatch",
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub run_id: String,
    pub generated_at: DateTime<Utc>,
    pub machine_id: String,
    pub check_id: String,
    pub agent_id: Option<String>,
    pub severity: Severity,
    pub category: FindingCategory,
    pub fixed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckRegistryEntry {
    pub check_id: String,
    pub enabled: bool,
    pub severity_weight: Option<u8>,
    pub threshold: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckRegistry {
    pub checks: Vec<CheckRegistryEntry>,
}

impl Default for CheckRegistry {
    fn default() -> Self {
        Self {
            checks: DEFAULT_CHECK_IDS
                .iter()
                .map(|check_id| CheckRegistryEntry {
                    check_id: (*check_id).to_string(),
                    enabled: true,
                    severity_weight: None,
                    threshold: None,
                })
                .collect(),
        }
    }
}

pub fn audit_dir(home_dir: &Path) -> PathBuf {
    home_dir.join(".agents").join("audit")
}

pub fn history_path(home_dir: &Path) -> PathBuf {
    audit_dir(home_dir).join("history.jsonl")
}

pub fn registry_path(home_dir: &Path) -> PathBuf {
    audit_dir(home_dir).join("checks.json")
}

pub fn append_history(
    home_dir: &Path,
    report: &AuditReport,
    fixed_keys: &[(String, Option<String>)],
) -> io::Result<()> {
    std::fs::create_dir_all(audit_dir(home_dir))?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(history_path(home_dir))?;
    let run_id = report.generated_at.to_rfc3339();
    for finding in &report.global_findings {
        write_finding(&mut file, report, finding, fixed_keys, &run_id)?;
    }
    for agent in &report.agents {
        for finding in &agent.findings {
            write_finding(&mut file, report, finding, fixed_keys, &run_id)?;
        }
    }
    Ok(())
}

fn write_finding<W: io::Write>(
    writer: &mut W,
    report: &AuditReport,
    finding: &AuditFinding,
    fixed_keys: &[(String, Option<String>)],
    run_id: &str,
) -> io::Result<()> {
    let entry = HistoryEntry {
        run_id: run_id.to_string(),
        generated_at: report.generated_at,
        machine_id: report.machine_id.clone(),
        check_id: finding.check_id.clone(),
        agent_id: finding.agent_id.clone(),
        severity: finding.severity,
        category: finding.category,
        fixed: fixed_keys.contains(&(finding.check_id.clone(), finding.agent_id.clone())),
    };
    serde_json::to_writer(&mut *writer, &entry)?;
    writer.write_all(b"\n")
}

pub fn load_history(home_dir: &Path) -> io::Result<Vec<HistoryEntry>> {
    let content = std::fs::read_to_string(history_path(home_dir))?;
    let mut entries = Vec::new();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str(line) {
            entries.push(entry);
        }
    }
    Ok(entries)
}

pub fn load_registry(home_dir: &Path) -> CheckRegistry {
    std::fs::read_to_string(registry_path(home_dir))
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

pub fn save_registry(home_dir: &Path, registry: &CheckRegistry) -> io::Result<()> {
    std::fs::create_dir_all(audit_dir(home_dir))?;
    let content = serde_json::to_string_pretty(registry)?;
    std::fs::write(registry_path(home_dir), content)
}

fn ordered_run_ids(history: &[HistoryEntry]) -> Vec<String> {
    let mut runs: Vec<(DateTime<Utc>, String)> = history
        .iter()
        .map(|entry| (entry.generated_at, entry.run_id.clone()))
        .collect();
    runs.sort();
    runs.dedup();
    runs.into_iter().map(|(_, run_id)| run_id).collect()
}

pub fn recurring_check_ids(history: &[HistoryEntry]) -> Vec<String> {
    let run_ids = ordered_run_ids(history);
    let all_combos: HashSet<(String, Option<String>)> = history
        .iter()
        .map(|entry| (entry.check_id.clone(), entry.agent_id.clone()))
        .collect();
    let mut streaks: HashMap<(String, Option<String>), usize> = HashMap::new();
    let mut recurring: HashSet<String> = HashSet::new();
    for run_id in &run_ids {
        let fired: HashSet<(String, Option<String>)> = history
            .iter()
            .filter(|entry| &entry.run_id == run_id)
            .map(|entry| (entry.check_id.clone(), entry.agent_id.clone()))
            .collect();
        for combo in &fired {
            let streak = streaks.entry(combo.clone()).or_insert(0);
            *streak += 1;
            if *streak >= RECURRING_CONSECUTIVE_RUNS {
                recurring.insert(combo.0.clone());
            }
        }
        for combo in &all_combos {
            if !fired.contains(combo) {
                streaks.insert(combo.clone(), 0);
            }
        }
    }
    let mut ids: Vec<String> = recurring.into_iter().collect();
    ids.sort();
    ids
}

pub fn promote_severity(severity: Severity, recurring: bool) -> Severity {
    if !recurring {
        return severity;
    }
    match severity {
        Severity::Critical => Severity::Critical,
        Severity::Warning => Severity::Critical,
        Severity::Info => Severity::Warning,
        Severity::Suggestion => Severity::Info,
    }
}

pub fn dormant_check_ids(history: &[HistoryEntry], total_runs: usize) -> Vec<String> {
    if total_runs < DORMANT_WINDOW_RUNS {
        return Vec::new();
    }
    let run_ids = ordered_run_ids(history);
    let recent: HashSet<String> = run_ids
        .iter()
        .rev()
        .take(DORMANT_WINDOW_RUNS)
        .cloned()
        .collect();
    let recent_checks: HashSet<String> = history
        .iter()
        .filter(|entry| recent.contains(&entry.run_id))
        .map(|entry| entry.check_id.clone())
        .collect();
    let mut dormant: Vec<String> = history
        .iter()
        .map(|entry| entry.check_id.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .filter(|check_id| !recent_checks.contains(check_id))
        .collect();
    dormant.sort();
    dormant
}

pub fn demote_severity(severity: Severity, dormant: bool) -> Severity {
    if dormant {
        Severity::Suggestion
    } else {
        severity
    }
}

pub fn new_check_candidates(history: &[HistoryEntry]) -> Vec<String> {
    let all_runs: HashSet<(String, String)> = history
        .iter()
        .map(|entry| (entry.machine_id.clone(), entry.run_id.clone()))
        .collect();
    if all_runs.is_empty() {
        return Vec::new();
    }
    let total_runs = all_runs.len();
    let mut fired_by_check: HashMap<String, HashSet<(String, String)>> = HashMap::new();
    for entry in history {
        fired_by_check
            .entry(entry.check_id.clone())
            .or_default()
            .insert((entry.machine_id.clone(), entry.run_id.clone()));
    }
    let mut candidates: Vec<String> = fired_by_check
        .into_iter()
        .filter(|(check_id, fired)| {
            check_id != NEW_CHECK_CANDIDATE_CHECK_ID && fired.len() * 5 >= total_runs * 4
        })
        .map(|(check_id, _)| check_id)
        .collect();
    candidates.sort();
    candidates
}

pub fn apply_feedback(report: &mut AuditReport, history: &[HistoryEntry]) {
    let recurring = recurring_check_ids(history);
    let total_runs = history
        .iter()
        .map(|entry| (entry.machine_id.clone(), entry.run_id.clone()))
        .collect::<HashSet<_>>()
        .len();
    let dormant = dormant_check_ids(history, total_runs);
    for finding in report.global_findings.iter_mut().chain(
        report
            .agents
            .iter_mut()
            .flat_map(|agent| agent.findings.iter_mut()),
    ) {
        if finding.category == FindingCategory::Audited {
            continue;
        }
        finding.severity =
            promote_severity(finding.severity, recurring.contains(&finding.check_id));
        finding.severity = demote_severity(finding.severity, dormant.contains(&finding.check_id));
    }
    let candidates = new_check_candidates(history);
    if !candidates.is_empty() {
        report
            .global_findings
            .push(new_check_candidate_finding(&candidates));
    }
    report.summary = build_summary(
        report
            .global_findings
            .iter()
            .chain(report.agents.iter().flat_map(|agent| agent.findings.iter())),
        &report.agents,
    );
}

fn new_check_candidate_finding(candidates: &[String]) -> AuditFinding {
    AuditFinding {
        check_id: NEW_CHECK_CANDIDATE_CHECK_ID.to_string(),
        severity: Severity::Suggestion,
        category: FindingCategory::Config,
        agent_id: None,
        message: format!(
            "checks firing on >=80% of runs are candidates for the default catalog: {}",
            candidates.join(", ")
        ),
        remediation:
            "promote the candidate checks into the check catalog or disable them via checks.json"
                .to_string(),
        auto_fixable: false,
        fix: None,
        suggested_fix: None,
        evidence: Some(candidates.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use agentry_core::models::{AgentSpec, DetectedAgent, InstallMethod, PromptFormat};
    use chrono::{Duration, TimeZone};

    use super::*;
    use crate::report::{AgentAudit, AuditSummary, HealthGrade};

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

    fn base_time() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()
    }

    fn entry(check_id: &str, run_index: usize, agent_id: Option<&str>) -> HistoryEntry {
        HistoryEntry {
            run_id: format!("run-{run_index:04}"),
            generated_at: base_time() + Duration::seconds(run_index as i64),
            machine_id: "machine-a".to_string(),
            check_id: check_id.to_string(),
            agent_id: agent_id.map(str::to_string),
            severity: Severity::Warning,
            category: FindingCategory::Installation,
            fixed: false,
        }
    }

    fn finding(check_id: &str, severity: Severity) -> AuditFinding {
        AuditFinding {
            check_id: check_id.to_string(),
            severity,
            category: FindingCategory::Installation,
            agent_id: None,
            message: "test finding".to_string(),
            remediation: "fix it".to_string(),
            auto_fixable: false,
            fix: None,
            suggested_fix: None,
            evidence: None,
        }
    }

    fn agent_finding(check_id: &str, severity: Severity) -> AuditFinding {
        let mut finding = finding(check_id, severity);
        finding.agent_id = Some("codex".to_string());
        finding
    }

    fn detected_codex() -> DetectedAgent {
        DetectedAgent {
            spec: AgentSpec {
                id: "codex".to_string(),
                name: "codex".to_string(),
                cli_binary: "codex".to_string(),
                config_dir: ".codex".to_string(),
                prompt_filename: "AGENTS.md".to_string(),
                prompt_format: PromptFormat::PlainMd,
                skills_dir_name: None,
                max_size: None,
                install_methods: vec![InstallMethod::Npm {
                    package: "codex".to_string(),
                }],
            },
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

    fn fixture_report(
        agent_findings: Vec<AuditFinding>,
        global_findings: Vec<AuditFinding>,
    ) -> AuditReport {
        let mut all: Vec<AuditFinding> = agent_findings.to_vec();
        all.extend(global_findings.iter().cloned());
        let mut by_severity = BTreeMap::new();
        let mut by_category = BTreeMap::new();
        for finding in &all {
            *by_severity.entry(finding.severity).or_insert(0) += 1;
            *by_category.entry(finding.category).or_insert(0) += 1;
        }
        AuditReport {
            generated_at: base_time(),
            machine_id: "machine-a".to_string(),
            agents: vec![AgentAudit {
                agent_id: "codex".to_string(),
                health_score: 100,
                grade: HealthGrade::Healthy,
                detected: detected_codex(),
                findings: agent_findings,
            }],
            global_findings,
            summary: AuditSummary {
                total_findings: all.len(),
                by_severity,
                by_category,
                auto_fixable_count: 0,
                healthy_agents: 1,
                degraded_agents: 0,
            },
            schema_version: 2,
        }
    }

    #[test]
    fn append_history_then_load_roundtrips() {
        let tmp = TempDir::new("agentry_history_roundtrip");
        let report = fixture_report(
            vec![agent_finding("sync.drift", Severity::Warning)],
            vec![finding("install.binary_missing", Severity::Critical)],
        );
        append_history(
            tmp.path(),
            &report,
            &[("sync.drift".to_string(), Some("codex".to_string()))],
        )
        .unwrap();

        let history = load_history(tmp.path()).unwrap();
        assert_eq!(history.len(), 2);

        let drift = history
            .iter()
            .find(|e| e.check_id == "sync.drift")
            .expect("sync.drift entry should exist");
        assert!(drift.fixed);
        assert_eq!(drift.agent_id.as_deref(), Some("codex"));
        assert_eq!(drift.severity, Severity::Warning);
        assert_eq!(drift.category, FindingCategory::Installation);
        assert_eq!(drift.run_id, report.generated_at.to_rfc3339());
        assert_eq!(drift.machine_id, report.machine_id);
        assert_eq!(drift.generated_at, report.generated_at);

        let binary = history
            .iter()
            .find(|e| e.check_id == "install.binary_missing")
            .expect("install.binary_missing entry should exist");
        assert!(!binary.fixed);
        assert!(binary.agent_id.is_none());
    }

    #[test]
    fn append_history_appends_one_line_per_finding_per_run() {
        let tmp = TempDir::new("agentry_history_append");
        let mut report =
            fixture_report(vec![agent_finding("sync.drift", Severity::Warning)], vec![]);
        report.generated_at = base_time() + Duration::seconds(1);
        append_history(tmp.path(), &report, &[]).unwrap();
        report.generated_at = base_time() + Duration::seconds(2);
        append_history(tmp.path(), &report, &[]).unwrap();

        let history = load_history(tmp.path()).unwrap();
        assert_eq!(history.len(), 2);
        let run_ids: HashSet<String> = history.iter().map(|e| e.run_id.clone()).collect();
        assert_eq!(run_ids.len(), 2);
    }

    #[test]
    fn append_history_marks_fixed_only_for_matching_agent() {
        let tmp = TempDir::new("agentry_history_fixed_per_agent");
        let mut report =
            fixture_report(vec![agent_finding("sync.drift", Severity::Warning)], vec![]);
        let mut gemini_finding = agent_finding("sync.drift", Severity::Warning);
        gemini_finding.agent_id = Some("gemini-cli".to_string());
        report.agents[0].findings.push(gemini_finding);

        append_history(
            tmp.path(),
            &report,
            &[("sync.drift".to_string(), Some("codex".to_string()))],
        )
        .unwrap();

        let history = load_history(tmp.path()).unwrap();
        assert_eq!(history.len(), 2);
        let codex = history
            .iter()
            .find(|e| e.agent_id.as_deref() == Some("codex"))
            .expect("codex entry should exist");
        assert!(codex.fixed);
        let gemini = history
            .iter()
            .find(|e| e.agent_id.as_deref() == Some("gemini-cli"))
            .expect("gemini entry should exist");
        assert!(!gemini.fixed);
    }

    #[test]
    fn load_history_skips_malformed_lines() {
        let tmp = TempDir::new("agentry_history_malformed");
        let dir = audit_dir(tmp.path());
        std::fs::create_dir_all(&dir).unwrap();
        let first = entry("sync.drift", 0, Some("codex"));
        let second = entry("config.stale", 0, Some("codex"));
        let content = format!(
            "{}\nnot json at all\n{{\"broken\": true}}\n{}\n\n",
            serde_json::to_string(&first).unwrap(),
            serde_json::to_string(&second).unwrap()
        );
        std::fs::write(history_path(tmp.path()), content).unwrap();

        let history = load_history(tmp.path()).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].check_id, "sync.drift");
        assert_eq!(history[1].check_id, "config.stale");
    }

    #[test]
    fn load_registry_defaults_when_missing() {
        let tmp = TempDir::new("agentry_history_registry_default");
        let registry = load_registry(tmp.path());
        assert!(!registry.checks.is_empty());
        assert!(registry.checks.iter().all(|check| check.enabled));
        assert!(registry
            .checks
            .iter()
            .all(|check| check.severity_weight.is_none() && check.threshold.is_none()));
        assert!(registry
            .checks
            .iter()
            .any(|check| check.check_id == "install.binary_missing"));
    }

    #[test]
    fn save_registry_then_load_roundtrips() {
        let tmp = TempDir::new("agentry_history_registry_roundtrip");
        let mut registry = load_registry(tmp.path());
        registry.checks[0].enabled = false;
        registry.checks[0].severity_weight = Some(20);
        registry.checks[0].threshold = Some(90);
        save_registry(tmp.path(), &registry).unwrap();

        let loaded = load_registry(tmp.path());
        assert_eq!(loaded, registry);
    }

    #[test]
    fn recurring_check_ids_require_three_consecutive_runs() {
        let history = vec![
            entry("sync.drift", 0, Some("codex")),
            entry("config.stale", 0, Some("codex")),
            entry("sync.drift", 1, Some("codex")),
            entry("config.stale", 1, Some("codex")),
            entry("sync.drift", 2, Some("codex")),
            entry("config.stale", 9, Some("codex")),
        ];
        assert_eq!(
            recurring_check_ids(&history),
            vec!["sync.drift".to_string()]
        );
    }

    #[test]
    fn recurring_check_ids_track_check_and_agent_combos() {
        let mut history = vec![
            entry("sync.drift", 0, Some("codex")),
            entry("sync.drift", 1, Some("gemini-cli")),
            entry("sync.drift", 2, Some("codex")),
        ];
        assert!(recurring_check_ids(&history).is_empty());
        history.push(entry("sync.drift", 3, Some("codex")));
        history.push(entry("sync.drift", 4, Some("codex")));
        assert_eq!(
            recurring_check_ids(&history),
            vec!["sync.drift".to_string()]
        );
    }

    #[test]
    fn dormant_check_ids_absent_from_last_ten_runs() {
        let mut history = vec![entry("config.stale", 0, Some("codex"))];
        for run in 1..=10 {
            history.push(entry("sync.drift", run, Some("codex")));
        }
        assert_eq!(
            dormant_check_ids(&history, 11),
            vec!["config.stale".to_string()]
        );
    }

    #[test]
    fn dormant_check_ids_wait_until_ten_runs_elapsed() {
        let history = vec![
            entry("config.stale", 0, Some("codex")),
            entry("config.stale", 1, Some("codex")),
        ];
        assert!(dormant_check_ids(&history, 9).is_empty());
        assert!(dormant_check_ids(&history, 10).is_empty());
    }

    #[test]
    fn promote_severity_moves_one_level() {
        assert_eq!(
            promote_severity(Severity::Warning, true),
            Severity::Critical
        );
        assert_eq!(promote_severity(Severity::Info, true), Severity::Warning);
        assert_eq!(promote_severity(Severity::Suggestion, true), Severity::Info);
        assert_eq!(
            promote_severity(Severity::Critical, true),
            Severity::Critical
        );
        assert_eq!(
            promote_severity(Severity::Warning, false),
            Severity::Warning
        );
    }

    #[test]
    fn demote_severity_falls_back_to_suggestion() {
        assert_eq!(
            demote_severity(Severity::Critical, true),
            Severity::Suggestion
        );
        assert_eq!(
            demote_severity(Severity::Warning, true),
            Severity::Suggestion
        );
        assert_eq!(demote_severity(Severity::Info, true), Severity::Suggestion);
        assert_eq!(
            demote_severity(Severity::Suggestion, true),
            Severity::Suggestion
        );
        assert_eq!(demote_severity(Severity::Warning, false), Severity::Warning);
    }

    #[test]
    fn new_check_candidates_requires_eighty_percent_fire_rate() {
        let mut history = Vec::new();
        for run in 0..10 {
            if run < 8 {
                history.push(entry("sync.drift", run, Some("codex")));
            }
            if run < 7 {
                history.push(entry("config.stale", run, Some("codex")));
            }
            history.push(entry("skills.orphaned", run, Some("codex")));
        }
        assert_eq!(
            new_check_candidates(&history),
            vec!["skills.orphaned".to_string(), "sync.drift".to_string()]
        );
    }

    #[test]
    fn apply_feedback_skips_audited_category() {
        let mut history = Vec::new();
        for run in 0..12 {
            history.push(entry("auditor.suggestion", run, Some("codex")));
        }
        let mut audited = finding("auditor.suggestion", Severity::Suggestion);
        audited.category = FindingCategory::Audited;
        audited.agent_id = Some("codex".to_string());
        let mut report = fixture_report(vec![audited], vec![]);
        apply_feedback(&mut report, &history);
        assert_eq!(
            report.agents[0].findings[0].severity,
            Severity::Suggestion,
            "Audited findings must never be promoted by recurrence"
        );
    }

    #[test]
    fn append_history_records_audited_findings() {
        let tmp = TempDir::new("agentry_history_audited");
        let mut audited = finding("auditor.suggestion", Severity::Suggestion);
        audited.category = FindingCategory::Audited;
        let report = fixture_report(vec![], vec![audited]);
        append_history(tmp.path(), &report, &[]).unwrap();
        let history = load_history(tmp.path()).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].category, FindingCategory::Audited);
        assert_eq!(history[0].check_id, "auditor.suggestion");
    }

    #[test]
    fn apply_feedback_promotes_demotes_and_appends_candidates() {
        let mut history = Vec::new();
        for run in 0..12 {
            if run <= 2 || run >= 9 {
                history.push(entry("sync.drift", run, Some("codex")));
            }
            if run <= 1 {
                history.push(entry("config.stale", run, Some("codex")));
            }
            if run < 10 {
                history.push(entry("install.binary_missing", run, Some("codex")));
            }
            if run == 5 {
                history.push(entry("install.config_dir_missing", run, Some("codex")));
            }
        }
        let mut report = fixture_report(
            vec![agent_finding("sync.drift", Severity::Warning)],
            vec![
                finding("config.stale", Severity::Warning),
                finding("install.config_dir_missing", Severity::Info),
            ],
        );
        apply_feedback(&mut report, &history);

        assert_eq!(report.agents[0].findings[0].check_id, "sync.drift");
        assert_eq!(report.agents[0].findings[0].severity, Severity::Critical);
        let global: Vec<(String, Severity)> = report
            .global_findings
            .iter()
            .map(|f| (f.check_id.clone(), f.severity))
            .collect();
        assert!(global.contains(&("config.stale".to_string(), Severity::Suggestion)));
        assert!(global.contains(&("install.config_dir_missing".to_string(), Severity::Info)));
        assert_eq!(report.global_findings.len(), 3);
        let candidate = report.global_findings.last().unwrap();
        assert_eq!(candidate.check_id, "audit.new_check_candidate");
        assert_eq!(candidate.severity, Severity::Suggestion);
        assert_eq!(candidate.agent_id, None);
        assert!(candidate
            .evidence
            .as_deref()
            .expect("candidate evidence should exist")
            .contains("install.binary_missing"));
    }
}
