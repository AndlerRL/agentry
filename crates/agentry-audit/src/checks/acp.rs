use crate::engine::CheckContext;
use crate::report::{AuditFinding, FindingCategory, Severity};

const EXPLICIT_CAPABILITY_AGENTS: [&str; 10] = [
    "claude-code",
    "continue",
    "gemini-cli",
    "codex",
    "opencode",
    "amp",
    "firebender",
    "deepagents",
    "antigravity",
    "warp",
];

pub fn run(ctx: &CheckContext) -> Vec<AuditFinding> {
    ctx.agents
        .iter()
        .filter(|agent| agent.installed)
        .filter(|agent| !EXPLICIT_CAPABILITY_AGENTS.contains(&agent.spec.id.as_str()))
        .map(capability_finding)
        .collect()
}

fn capability_finding(agent: &agentry_core::models::DetectedAgent) -> AuditFinding {
    AuditFinding {
        check_id: "acp.capability_mismatch".to_string(),
        severity: Severity::Info,
        category: FindingCategory::Acp,
        agent_id: Some(agent.spec.id.clone()),
        message: format!(
            "Installed agent '{}' has no explicit capability arm in build_capability_matrix and would resolve to the generic 'general' capability only",
            agent.spec.id
        ),
        remediation: format!(
            "Add an explicit capability arm for '{}' to build_capability_matrix in agentry-acp/src/router.rs",
            agent.spec.id
        ),
        auto_fixable: false,
        fix: None,
        evidence: Some(format!(
            "agent_id={} installed={} explicit_capability_arm=false fallback_capability=general",
            agent.spec.id, agent.installed
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentry_core::models::{AgentSpec, DetectedAgent, PromptFormat};
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

    fn agent(id: &str, config_dir: &str, installed: bool) -> DetectedAgent {
        DetectedAgent {
            spec: AgentSpec {
                id: id.to_string(),
                name: id.to_string(),
                cli_binary: id.to_string(),
                config_dir: config_dir.to_string(),
                prompt_filename: "AGENTS.md".to_string(),
                prompt_format: PromptFormat::PlainMd,
                skills_dir_name: None,
                max_size: None,
                install_methods: Vec::new(),
            },
            installed,
            version: None,
            config_dir_exists: true,
            prompt_file_exists: true,
            skills_dir: None,
            skills_symlink_pattern: None,
            installed_skills: Vec::new(),
            detected_methods: Vec::new(),
        }
    }

    fn ctx(home: PathBuf, agents: Vec<DetectedAgent>) -> CheckContext {
        CheckContext {
            home_dir: home,
            agents,
            prompts: Vec::new(),
            version_lookup: None,
            binary_on_path: Vec::new(),
        }
    }

    #[test]
    fn skips_agent_with_explicit_capability_arm() {
        let tmp = TempDir::new("agentry_audit_acp_explicit_no_fires");
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![agent("claude-code", ".claude", true)],
        ));
        assert!(findings.is_empty());
    }

    #[test]
    fn skips_agent_without_explicit_arm_when_not_installed() {
        let tmp = TempDir::new("agentry_audit_acp_uninstalled_no_fires");
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![agent("openclaw", ".openclaw", false)],
        ));
        assert!(findings.is_empty());
    }

    #[test]
    fn fires_for_installed_agent_without_explicit_arm() {
        let tmp = TempDir::new("agentry_audit_acp_no_arm_fires");
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![agent("openclaw", ".openclaw", true)],
        ));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].check_id, "acp.capability_mismatch");
        assert_eq!(findings[0].severity, Severity::Info);
        assert_eq!(findings[0].category, FindingCategory::Acp);
        assert_eq!(findings[0].agent_id.as_deref(), Some("openclaw"));
        assert!(!findings[0].auto_fixable);
        assert!(findings[0].fix.is_none());
        assert!(!findings[0].message.is_empty());
        assert!(!findings[0].remediation.is_empty());
        let evidence = findings[0].evidence.as_deref().unwrap_or_default();
        assert!(evidence.contains("agent_id=openclaw"));
        assert!(evidence.contains("explicit_capability_arm=false"));
        assert!(evidence.contains("fallback_capability=general"));
    }

    #[test]
    fn covers_all_explicit_arm_ids_from_router() {
        let tmp = TempDir::new("agentry_audit_acp_all_arms");
        let known = [
            "claude-code",
            "continue",
            "gemini-cli",
            "codex",
            "opencode",
            "amp",
            "firebender",
            "deepagents",
            "antigravity",
            "warp",
        ];
        let agents: Vec<DetectedAgent> = known
            .iter()
            .enumerate()
            .map(|(i, id)| agent(id, &format!(".dir{}", i), true))
            .collect();
        let findings = run(&ctx(tmp.path().clone(), agents));
        assert!(findings.is_empty());
    }
}
