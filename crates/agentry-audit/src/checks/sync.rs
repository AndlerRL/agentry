use agentry_core::models::{DetectedAgent, SyncAction, SyncMapping, SyncStatus};
use agentry_sync::executor::check_sync_status;
use agentry_sync::planner::plan_sync;

use crate::engine::CheckContext;
use crate::report::{AuditFinding, FindingCategory, FixAction, Severity};

pub fn run(ctx: &CheckContext) -> Vec<AuditFinding> {
    let mut findings = Vec::new();
    for prompt in &ctx.prompts {
        let plan = plan_sync(prompt, &ctx.agents, &ctx.home_dir);
        let statuses = check_sync_status(prompt, &planned_mappings(ctx, &plan.mappings));
        findings.extend(statuses.iter().filter_map(drift_finding));
        findings.extend(statuses.iter().filter_map(missing_finding));
    }
    findings
}

fn planned_mappings(ctx: &CheckContext, mappings: &[SyncMapping]) -> Vec<SyncMapping> {
    mappings
        .iter()
        .filter(|mapping| mapping.action != SyncAction::Skip)
        .filter(|mapping| !agent_prompt_file_absent(ctx, mapping))
        .cloned()
        .collect()
}

fn agent_prompt_file_absent(ctx: &CheckContext, mapping: &SyncMapping) -> bool {
    ctx.agents
        .iter()
        .find(|agent| agent.spec.id == mapping.agent_id)
        .is_some_and(|agent| !agent_prompt_file_exists(ctx, agent))
}

fn agent_prompt_file_exists(ctx: &CheckContext, agent: &DetectedAgent) -> bool {
    let path = ctx
        .home_dir
        .join(&agent.spec.config_dir)
        .join(&agent.spec.prompt_filename);
    if is_directory_prompt(&agent.spec.prompt_filename) {
        path.is_dir()
    } else {
        path.exists()
    }
}

fn is_directory_prompt(prompt_filename: &str) -> bool {
    prompt_filename.ends_with('/') || matches!(prompt_filename, "prompts" | "rules")
}

fn drift_finding(mapping: &SyncMapping) -> Option<AuditFinding> {
    if !matches!(mapping.status, SyncStatus::Outdated | SyncStatus::Conflict) {
        return None;
    }
    Some(AuditFinding {
        check_id: "sync.drift".to_string(),
        severity: Severity::Warning,
        category: FindingCategory::SyncDrift,
        agent_id: Some(mapping.agent_id.clone()),
        message: format!(
            "Prompt '{}' is {} at '{}'",
            mapping.prompt_id,
            mapping.status,
            mapping.destination.display()
        ),
        remediation: format!(
            "Run 'agentry sync --prompt {}' to restore '{}'",
            mapping.prompt_id,
            mapping.destination.display()
        ),
        auto_fixable: true,
        fix: Some(FixAction::SyncPrompt {
            prompt_id: mapping.prompt_id.clone(),
            agent_id: mapping.agent_id.clone(),
        }),
        suggested_fix: None,
        evidence: Some(format!(
            "status={} destination={}",
            mapping.status,
            mapping.destination.display()
        )),
    })
}

fn missing_finding(mapping: &SyncMapping) -> Option<AuditFinding> {
    if mapping.status != SyncStatus::Missing {
        return None;
    }
    Some(AuditFinding {
        check_id: "sync.missing".to_string(),
        severity: Severity::Info,
        category: FindingCategory::SyncDrift,
        agent_id: Some(mapping.agent_id.clone()),
        message: format!(
            "Prompt '{}' has not been synced to '{}'",
            mapping.prompt_id,
            mapping.destination.display()
        ),
        remediation: format!(
            "Run 'agentry sync --prompt {}' to create '{}'",
            mapping.prompt_id,
            mapping.destination.display()
        ),
        auto_fixable: true,
        fix: Some(FixAction::SyncPrompt {
            prompt_id: mapping.prompt_id.clone(),
            agent_id: mapping.agent_id.clone(),
        }),
        suggested_fix: None,
        evidence: Some(format!(
            "status=Missing destination={}",
            mapping.destination.display()
        )),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentry_core::models::{AgentSpec, PromptFormat, PromptScope, UnifiedPrompt};
    use std::collections::BTreeMap;
    use std::path::PathBuf;

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

    fn prompt(name: &str, body: &str) -> UnifiedPrompt {
        UnifiedPrompt {
            id: name.to_string(),
            name: name.to_string(),
            description: String::new(),
            frontmatter: BTreeMap::new(),
            body: body.to_string(),
            xml_tags: Vec::new(),
            scope: PromptScope::Global,
            source_format: PromptFormat::PlainMd,
            source_path: None,
        }
    }

    fn agent(id: &str, config_dir: &str, prompt_filename: &str) -> DetectedAgent {
        DetectedAgent {
            spec: AgentSpec {
                id: id.to_string(),
                name: id.to_string(),
                cli_binary: id.to_string(),
                config_dir: config_dir.to_string(),
                prompt_filename: prompt_filename.to_string(),
                prompt_format: PromptFormat::PlainMd,
                skills_dir_name: None,
                max_size: None,
                install_methods: Vec::new(),
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

    fn ctx(home: PathBuf, agents: Vec<DetectedAgent>, prompts: Vec<UnifiedPrompt>) -> CheckContext {
        CheckContext {
            home_dir: home,
            agents,
            prompts,
            version_lookup: None,
            binary_on_path: Vec::new(),
        }
    }

    #[test]
    fn drift_fires_when_agent_file_differs_from_canonical() {
        let tmp = TempDir::new("agentry_audit_sync_drift_fires");
        let dir = tmp.path().join(".claude");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("CLAUDE.md"), "# Drifted rules").unwrap();
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![agent("claude-code", ".claude", "CLAUDE.md")],
            vec![prompt("architect", "# Architect rules")],
        ));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].check_id, "sync.drift");
        assert_eq!(findings[0].severity, Severity::Warning);
        assert_eq!(findings[0].category, FindingCategory::SyncDrift);
        assert_eq!(findings[0].agent_id.as_deref(), Some("claude-code"));
        assert!(findings[0].auto_fixable);
        assert!(!findings[0].message.is_empty());
        assert!(!findings[0].remediation.is_empty());
        match &findings[0].fix {
            Some(FixAction::SyncPrompt {
                prompt_id,
                agent_id,
            }) => {
                assert_eq!(prompt_id, "architect");
                assert_eq!(agent_id, "claude-code");
            }
            other => panic!("expected SyncPrompt fix, got {:?}", other),
        }
    }

    #[test]
    fn drift_skips_when_agent_file_matches_canonical() {
        let tmp = TempDir::new("agentry_audit_sync_drift_match");
        let dir = tmp.path().join(".claude");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("CLAUDE.md"), "# Architect rules").unwrap();
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![agent("claude-code", ".claude", "CLAUDE.md")],
            vec![prompt("architect", "# Architect rules")],
        ));
        assert!(findings.is_empty());
    }

    #[test]
    fn drift_fires_on_unreadable_file() {
        let tmp = TempDir::new("agentry_audit_sync_drift_conflict");
        let dir = tmp.path().join(".claude");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("CLAUDE.md"), b"\xff\xfe\xfc").unwrap();
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![agent("claude-code", ".claude", "CLAUDE.md")],
            vec![prompt("architect", "# Architect rules")],
        ));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].check_id, "sync.drift");
        let evidence = findings[0].evidence.as_deref().unwrap_or_default();
        assert!(evidence.contains("status=Conflict"));
    }

    #[test]
    fn missing_fires_when_destination_file_absent() {
        let tmp = TempDir::new("agentry_audit_sync_missing_fires");
        std::fs::create_dir_all(tmp.path().join(".continue").join("prompts")).unwrap();
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![agent("continue", ".continue", "prompts")],
            vec![prompt("architect", "# Architect rules")],
        ));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].check_id, "sync.missing");
        assert_eq!(findings[0].severity, Severity::Info);
        assert_eq!(findings[0].category, FindingCategory::SyncDrift);
        assert_eq!(findings[0].agent_id.as_deref(), Some("continue"));
        assert!(findings[0].auto_fixable);
        assert!(!findings[0].message.is_empty());
        assert!(!findings[0].remediation.is_empty());
        match &findings[0].fix {
            Some(FixAction::SyncPrompt {
                prompt_id,
                agent_id,
            }) => {
                assert_eq!(prompt_id, "architect");
                assert_eq!(agent_id, "continue");
            }
            other => panic!("expected SyncPrompt fix, got {:?}", other),
        }
    }

    #[test]
    fn skips_when_agent_prompt_file_absent() {
        let tmp = TempDir::new("agentry_audit_sync_skip_prompt_missing");
        std::fs::create_dir_all(tmp.path().join(".continue")).unwrap();
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![agent("continue", ".continue", "prompts")],
            vec![prompt("architect", "# Architect rules")],
        ));
        assert!(findings.is_empty());
    }

    #[test]
    fn skips_unknown_agent_with_empty_destination() {
        let tmp = TempDir::new("agentry_audit_sync_skip_unknown_agent");
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![agent("custom-agent", ".custom-agent", "AGENTS.md")],
            vec![prompt("architect", "# Architect rules")],
        ));
        assert!(findings.is_empty());
    }
}
