use crate::engine::CheckContext;
use crate::report::{AuditFinding, FindingCategory, Severity};

const AUTH_PROBES: &[(&str, &str)] = &[
    ("claude-code", ".claude/.credentials.json"),
    ("codex", ".codex/auth.json"),
    ("gemini-cli", ".gemini/oauth_creds.json"),
    ("opencode", ".local/share/opencode/auth.json"),
    ("continue", ".continue/auth.json"),
];

pub fn run(ctx: &CheckContext) -> Vec<AuditFinding> {
    let mut findings = Vec::new();
    for agent in &ctx.agents {
        let Some(probe_relative) = AUTH_PROBES
            .iter()
            .find(|(id, _)| *id == agent.spec.id)
            .map(|(_, path)| *path)
        else {
            continue;
        };
        let probe_path = ctx.home_dir.join(probe_relative);
        if probe_path.exists() {
            continue;
        }
        findings.push(AuditFinding {
            check_id: "auth.not_logged_in".to_string(),
            severity: Severity::Info,
            category: FindingCategory::Auth,
            agent_id: Some(agent.spec.id.clone()),
            message: format!(
                "{} has no credential file at '{}' — likely not logged in",
                agent.spec.name,
                probe_path.display()
            ),
            remediation: format!(
                "Run '{} login' to authenticate {}",
                agent.spec.cli_binary, agent.spec.name
            ),
            auto_fixable: false,
            fix: None,
            evidence: Some(probe_path.display().to_string()),
        });
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::CheckContext;
    use agentry_core::models::{AgentSpec, DetectedAgent, InstallMethod, PromptFormat};
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

    fn spec(id: &str) -> AgentSpec {
        AgentSpec {
            id: id.to_string(),
            name: id.to_string(),
            cli_binary: id.to_string(),
            config_dir: format!(".{id}"),
            prompt_filename: "AGENTS.md".to_string(),
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
            prompt_file_exists: false,
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
    fn not_logged_in_fires_when_probe_file_absent() {
        let tmp = TempDir::new("agentry_audit_auth_absent");
        std::fs::create_dir_all(tmp.path().join(".codex")).unwrap();
        let findings = run(&ctx(tmp.path().clone(), vec![agent(spec("codex"))]));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].check_id, "auth.not_logged_in");
        assert_eq!(findings[0].severity, Severity::Info);
        assert_eq!(findings[0].agent_id.as_deref(), Some("codex"));
    }

    #[test]
    fn not_logged_in_skipped_when_probe_file_present() {
        let tmp = TempDir::new("agentry_audit_auth_present");
        let codex_dir = tmp.path().join(".codex");
        std::fs::create_dir_all(&codex_dir).unwrap();
        std::fs::write(codex_dir.join("auth.json"), "{}").unwrap();
        let findings = run(&ctx(tmp.path().clone(), vec![agent(spec("codex"))]));
        assert!(findings.is_empty());
    }

    #[test]
    fn agents_without_probe_are_skipped() {
        let tmp = TempDir::new("agentry_audit_auth_no_probe");
        let findings = run(&ctx(tmp.path().clone(), vec![agent(spec("warp"))]));
        assert!(findings.is_empty());
    }
}
