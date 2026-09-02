use crate::engine::CheckContext;
use crate::report::{AuditFinding, FindingCategory, FixAction, Severity};

pub fn run(ctx: &CheckContext) -> Vec<AuditFinding> {
    let mut findings = Vec::new();
    for agent in &ctx.agents {
        findings.extend(binary_missing(ctx, agent));
        findings.extend(config_dir_missing(ctx, agent));
        findings.extend(method_conflict(agent));
    }
    findings
}

fn binary_missing(
    ctx: &CheckContext,
    agent: &agentry_core::models::DetectedAgent,
) -> Vec<AuditFinding> {
    if !agent.config_dir_exists && !agent.prompt_file_exists {
        return Vec::new();
    }
    if ctx
        .binary_on_path
        .iter()
        .any(|b| b == &agent.spec.cli_binary)
    {
        return Vec::new();
    }
    let install_command = agent
        .spec
        .install_methods
        .first()
        .map(|m| m.install_command(None));
    let fix = install_command.map(|command| FixAction::ShellCommand {
        description: format!("Install {} via preferred method", agent.spec.name),
        command,
    });
    vec![AuditFinding {
        check_id: "install.binary_missing".to_string(),
        severity: Severity::Warning,
        category: FindingCategory::Installation,
        agent_id: Some(agent.spec.id.clone()),
        message: format!(
            "{} has configuration on disk but binary '{}' is not on PATH",
            agent.spec.name, agent.spec.cli_binary
        ),
        remediation: format!(
            "Install {} via its preferred method ({})",
            agent.spec.name,
            agent
                .spec
                .install_methods
                .first()
                .map(|m| m.label())
                .unwrap_or("unknown")
        ),
        auto_fixable: fix.is_some(),
        fix,
        suggested_fix: None,
        evidence: Some(format!(
            "config_dir_exists={} prompt_file_exists={} binary_on_path={}",
            agent.config_dir_exists,
            agent.prompt_file_exists,
            ctx.binary_on_path.join(", ")
        )),
    }]
}

fn config_dir_missing(
    ctx: &CheckContext,
    agent: &agentry_core::models::DetectedAgent,
) -> Vec<AuditFinding> {
    if agent.config_dir_exists {
        return Vec::new();
    }
    if !ctx
        .binary_on_path
        .iter()
        .any(|b| b == &agent.spec.cli_binary)
    {
        return Vec::new();
    }
    vec![AuditFinding {
        check_id: "install.config_dir_missing".to_string(),
        severity: Severity::Info,
        category: FindingCategory::Installation,
        agent_id: Some(agent.spec.id.clone()),
        message: format!(
            "{} binary is installed but config directory '{}' does not exist",
            agent.spec.name, agent.spec.config_dir
        ),
        remediation: format!(
            "Run '{}' once to generate its configuration directory",
            agent.spec.cli_binary
        ),
        auto_fixable: false,
        fix: None,
        suggested_fix: None,
        evidence: Some(agent.spec.config_dir.clone()),
    }]
}

fn method_conflict(agent: &agentry_core::models::DetectedAgent) -> Vec<AuditFinding> {
    if agent.detected_methods.len() <= 1 {
        return Vec::new();
    }
    let methods = agent
        .detected_methods
        .iter()
        .map(|m| m.label())
        .collect::<Vec<_>>()
        .join(", ");
    vec![AuditFinding {
        check_id: "install.method_conflict".to_string(),
        severity: Severity::Info,
        category: FindingCategory::Installation,
        agent_id: Some(agent.spec.id.clone()),
        message: format!(
            "{} was installed via multiple methods: {}",
            agent.spec.name, methods
        ),
        remediation: format!(
            "Remove the redundant install method; keep '{}' and uninstall the rest",
            agent
                .detected_methods
                .first()
                .map(|m| m.label())
                .unwrap_or("preferred")
        ),
        auto_fixable: false,
        fix: None,
        suggested_fix: None,
        evidence: Some(
            agent
                .detected_methods
                .iter()
                .map(|m| m.method_key())
                .collect::<Vec<_>>()
                .join(", "),
        ),
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
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

    fn spec(id: &str, binary: &str, config_dir: &str) -> AgentSpec {
        AgentSpec {
            id: id.to_string(),
            name: id.to_string(),
            cli_binary: binary.to_string(),
            config_dir: config_dir.to_string(),
            prompt_filename: "AGENTS.md".to_string(),
            prompt_format: PromptFormat::PlainMd,
            skills_dir_name: None,
            max_size: None,
            install_methods: vec![InstallMethod::Npm {
                package: id.to_string(),
            }],
        }
    }

    fn agent(spec: AgentSpec, config_dir_exists: bool) -> DetectedAgent {
        DetectedAgent {
            spec,
            installed: config_dir_exists,
            version: None,
            config_dir_exists,
            prompt_file_exists: false,
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

    #[test]
    fn binary_missing_fires_when_config_exists_but_binary_absent() {
        let tmp = TempDir::new("agentry_audit_install_binary_missing");
        let agent = agent(spec("codex", "codex", ".codex"), true);
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![agent],
            vec!["other-tool".to_string()],
        ));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].check_id, "install.binary_missing");
        assert_eq!(findings[0].severity, Severity::Warning);
        assert!(findings[0].auto_fixable);
        match &findings[0].fix {
            Some(FixAction::ShellCommand { command, .. }) => {
                assert!(command.contains("npm install"));
            }
            other => panic!("expected ShellCommand fix, got {:?}", other),
        }
    }

    #[test]
    fn binary_missing_skipped_when_binary_on_path() {
        let tmp = TempDir::new("agentry_audit_install_binary_present");
        let agent = agent(spec("codex", "codex", ".codex"), true);
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![agent],
            vec!["codex".to_string()],
        ));
        assert!(findings.is_empty());
    }

    #[test]
    fn binary_missing_skipped_when_no_config_and_no_prompt() {
        let tmp = TempDir::new("agentry_audit_install_no_config");
        let agent = agent(spec("codex", "codex", ".codex"), false);
        let findings = run(&ctx(tmp.path().clone(), vec![agent], vec![]));
        assert!(findings.is_empty());
    }

    #[test]
    fn config_dir_missing_fires_when_installed_but_no_config() {
        let tmp = TempDir::new("agentry_audit_install_config_missing");
        let mut agent = agent(spec("codex", "codex", ".codex"), false);
        agent.installed = true;
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![agent],
            vec!["codex".to_string()],
        ));
        let config_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.check_id == "install.config_dir_missing")
            .collect();
        assert_eq!(config_findings.len(), 1);
        assert_eq!(config_findings[0].severity, Severity::Info);
        assert!(!config_findings[0].auto_fixable);
    }

    #[test]
    fn config_dir_missing_skipped_when_config_exists() {
        let tmp = TempDir::new("agentry_audit_install_config_exists");
        let agent = agent(spec("codex", "codex", ".codex"), true);
        let findings = run(&ctx(tmp.path().clone(), vec![agent], vec![]));
        assert!(!findings
            .iter()
            .any(|f| f.check_id == "install.config_dir_missing"));
    }

    #[test]
    fn method_conflict_fires_with_two_methods() {
        let tmp = TempDir::new("agentry_audit_install_method_conflict");
        let mut agent = agent(spec("codex", "codex", ".codex"), true);
        agent.detected_methods = vec![
            InstallMethod::Npm {
                package: "@openai/codex".to_string(),
            },
            InstallMethod::Brew {
                formula: "codex".to_string(),
                cask: true,
            },
        ];
        let findings = run(&ctx(tmp.path().clone(), vec![agent], vec![]));
        let conflicts: Vec<_> = findings
            .iter()
            .filter(|f| f.check_id == "install.method_conflict")
            .collect();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].severity, Severity::Info);
        assert!(conflicts[0]
            .evidence
            .as_deref()
            .unwrap_or_default()
            .contains("npm"));
    }

    #[test]
    fn method_conflict_skipped_with_single_method() {
        let tmp = TempDir::new("agentry_audit_install_single_method");
        let mut agent = agent(spec("codex", "codex", ".codex"), true);
        agent.detected_methods = vec![InstallMethod::Npm {
            package: "@openai/codex".to_string(),
        }];
        let findings = run(&ctx(tmp.path().clone(), vec![agent], vec![]));
        assert!(!findings
            .iter()
            .any(|f| f.check_id == "install.method_conflict"));
    }
}
