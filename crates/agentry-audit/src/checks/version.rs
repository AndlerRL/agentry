use semver::Version;

use crate::engine::CheckContext;
use crate::report::{AuditFinding, FindingCategory, FixAction, Severity};

pub fn run(ctx: &CheckContext) -> Vec<AuditFinding> {
    let mut findings = Vec::new();
    for agent in &ctx.agents {
        findings.extend(unparseable(agent));
        findings.extend(outdated(ctx, agent));
        findings.extend(latest_unknown(ctx, agent));
    }
    findings
}

fn parse_lenient(raw: &str) -> Option<Version> {
    let trimmed = raw.trim().trim_start_matches('v');
    let core = trimmed.split(['+', '-']).next()?;
    let parts: Vec<&str> = core.split('.').collect();
    if parts.is_empty() || parts.len() > 3 {
        return None;
    }
    let mut numeric: Vec<&str> = Vec::new();
    for part in &parts {
        if part.is_empty() || !part.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        numeric.push(part);
    }
    while numeric.len() < 3 {
        numeric.push("0");
    }
    Version::parse(&numeric.join(".")).ok()
}

fn unparseable(agent: &agentry_core::models::DetectedAgent) -> Vec<AuditFinding> {
    let Some(version) = agent.version.as_deref() else {
        return Vec::new();
    };
    if parse_lenient(version).is_some() {
        return Vec::new();
    }
    vec![AuditFinding {
        check_id: "version.unparseable".to_string(),
        severity: Severity::Info,
        category: FindingCategory::Version,
        agent_id: Some(agent.spec.id.clone()),
        message: format!(
            "{} reports version '{}' which has no semver-like token",
            agent.spec.name, version
        ),
        remediation: "Check the installed version manually with the CLI's version command"
            .to_string(),
        auto_fixable: false,
        fix: None,
        suggested_fix: None,
        evidence: Some(version.to_string()),
    }]
}

fn outdated(ctx: &CheckContext, agent: &agentry_core::models::DetectedAgent) -> Vec<AuditFinding> {
    let Some(lookup) = ctx.version_lookup.as_deref() else {
        return Vec::new();
    };
    let Some(installed_raw) = agent.version.as_deref() else {
        return Vec::new();
    };
    let Some(installed) = parse_lenient(installed_raw) else {
        return Vec::new();
    };
    let method = agent
        .spec
        .install_methods
        .first()
        .filter(|m| m.list_versions_command().is_some());
    let Some(method) = method else {
        return Vec::new();
    };
    let Some(versions) = lookup(&agent.spec.id, method.method_key()) else {
        return Vec::new();
    };
    let Some(latest) = versions.iter().filter_map(|v| parse_lenient(v)).max() else {
        return Vec::new();
    };
    if installed >= latest {
        return Vec::new();
    }
    let fix = FixAction::ShellCommand {
        description: format!("Update {} to {}", agent.spec.name, latest),
        command: method.update_command(),
    };
    vec![AuditFinding {
        check_id: "version.outdated".to_string(),
        severity: Severity::Warning,
        category: FindingCategory::Version,
        agent_id: Some(agent.spec.id.clone()),
        message: format!(
            "{} is outdated: installed {}, latest {}",
            agent.spec.name, installed_raw, latest
        ),
        remediation: format!(
            "Update {} via {} ({})",
            agent.spec.name,
            method.label(),
            method.update_command()
        ),
        auto_fixable: true,
        fix: Some(fix),
        suggested_fix: None,
        evidence: Some(format!(
            "installed={} latest={} method={}",
            installed_raw,
            latest,
            method.method_key()
        )),
    }]
}

fn latest_unknown(
    ctx: &CheckContext,
    agent: &agentry_core::models::DetectedAgent,
) -> Vec<AuditFinding> {
    if ctx.version_lookup.is_none() {
        return Vec::new();
    }
    let Some(method) = agent.spec.install_methods.first() else {
        return Vec::new();
    };
    if method.list_versions_command().is_some() {
        return Vec::new();
    }
    vec![AuditFinding {
        check_id: "version.latest_unknown".to_string(),
        severity: Severity::Suggestion,
        category: FindingCategory::Version,
        agent_id: Some(agent.spec.id.clone()),
        message: format!(
            "{} was installed via {} which cannot list available versions",
            agent.spec.name,
            method.label()
        ),
        remediation: "Check the vendor's release channel manually for newer versions".to_string(),
        auto_fixable: false,
        fix: None,
        suggested_fix: None,
        evidence: Some(method.method_key().to_string()),
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{CheckContext, VersionLookup};
    use agentry_core::models::{AgentSpec, DetectedAgent, InstallMethod, PromptFormat};
    use std::path::PathBuf;

    fn spec(id: &str, methods: Vec<InstallMethod>) -> AgentSpec {
        AgentSpec {
            id: id.to_string(),
            name: id.to_string(),
            cli_binary: id.to_string(),
            config_dir: format!(".{id}"),
            prompt_filename: "AGENTS.md".to_string(),
            prompt_format: PromptFormat::PlainMd,
            skills_dir_name: None,
            max_size: None,
            install_methods: methods,
        }
    }

    fn agent(spec: AgentSpec, version: Option<&str>) -> DetectedAgent {
        DetectedAgent {
            spec,
            installed: true,
            version: version.map(|v| v.to_string()),
            config_dir_exists: true,
            prompt_file_exists: false,
            skills_dir: None,
            skills_symlink_pattern: None,
            installed_skills: Vec::new(),
            detected_methods: Vec::new(),
        }
    }

    fn ctx(agents: Vec<DetectedAgent>, lookup: Option<Box<VersionLookup>>) -> CheckContext {
        CheckContext {
            home_dir: PathBuf::from("/tmp"),
            agents,
            prompts: Vec::new(),
            version_lookup: lookup,
            binary_on_path: Vec::new(),
        }
    }

    #[test]
    fn parse_lenient_handles_common_shapes() {
        assert!(parse_lenient("v1.2.3").is_some());
        assert!(parse_lenient("1.0").is_some());
        assert!(parse_lenient("1.2.3-beta.1").is_some());
        assert!(parse_lenient("1.2.3+build.7").is_some());
        assert!(parse_lenient("dev").is_none());
        assert!(parse_lenient("latest").is_none());
        assert!(parse_lenient("").is_none());
    }

    #[test]
    fn unparseable_fires_for_dev_version() {
        let agent = agent(
            spec(
                "codex",
                vec![InstallMethod::Npm {
                    package: "codex".into(),
                }],
            ),
            Some("dev"),
        );
        let findings = run(&ctx(vec![agent], None));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].check_id, "version.unparseable");
        assert_eq!(findings[0].severity, Severity::Info);
    }

    #[test]
    fn unparseable_skipped_for_semver_version() {
        let agent = agent(
            spec(
                "codex",
                vec![InstallMethod::Npm {
                    package: "codex".into(),
                }],
            ),
            Some("v1.2.3"),
        );
        let findings = run(&ctx(vec![agent], None));
        assert!(findings.is_empty());
    }

    #[test]
    fn unparseable_skipped_when_no_version() {
        let agent = agent(
            spec(
                "codex",
                vec![InstallMethod::Npm {
                    package: "codex".into(),
                }],
            ),
            None,
        );
        let findings = run(&ctx(vec![agent], None));
        assert!(findings.is_empty());
    }

    #[test]
    fn outdated_fires_when_lookup_has_newer_version() {
        let agent = agent(
            spec(
                "codex",
                vec![InstallMethod::Npm {
                    package: "@openai/codex".into(),
                }],
            ),
            Some("1.0.0"),
        );
        let lookup: Box<VersionLookup> =
            Box::new(|_agent_id: &str, _method: &str| Some(vec!["2.0.0".to_string()]));
        let findings = run(&ctx(vec![agent], Some(lookup)));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].check_id, "version.outdated");
        assert_eq!(findings[0].severity, Severity::Warning);
        assert!(findings[0].auto_fixable);
        match &findings[0].fix {
            Some(FixAction::ShellCommand { command, .. }) => {
                assert!(command.contains("npm update"));
            }
            other => panic!("expected ShellCommand fix, got {:?}", other),
        }
    }

    #[test]
    fn outdated_skipped_when_lookup_returns_none() {
        let agent = agent(
            spec(
                "codex",
                vec![InstallMethod::Npm {
                    package: "@openai/codex".into(),
                }],
            ),
            Some("1.0.0"),
        );
        let lookup: Box<VersionLookup> = Box::new(|_a: &str, _m: &str| None);
        let findings = run(&ctx(vec![agent], Some(lookup)));
        assert!(findings.is_empty());
    }

    #[test]
    fn outdated_skipped_when_up_to_date() {
        let agent = agent(
            spec(
                "codex",
                vec![InstallMethod::Npm {
                    package: "@openai/codex".into(),
                }],
            ),
            Some("2.0.0"),
        );
        let lookup: Box<VersionLookup> =
            Box::new(|_a: &str, _m: &str| Some(vec!["2.0.0".to_string()]));
        let findings = run(&ctx(vec![agent], Some(lookup)));
        assert!(findings.is_empty());
    }

    #[test]
    fn latest_unknown_fires_for_direct_download_method() {
        let agent = agent(
            spec(
                "claude-code",
                vec![InstallMethod::DirectDownload {
                    url: "https://claude.ai/install.sh".into(),
                    binary_name: "claude".into(),
                }],
            ),
            Some("1.0.0"),
        );
        let lookup: Box<VersionLookup> =
            Box::new(|_a: &str, _m: &str| Some(vec!["1.0.0".to_string()]));
        let findings = run(&ctx(vec![agent], Some(lookup)));
        let unknowns: Vec<_> = findings
            .iter()
            .filter(|f| f.check_id == "version.latest_unknown")
            .collect();
        assert_eq!(unknowns.len(), 1);
        assert_eq!(unknowns[0].severity, Severity::Suggestion);
    }

    #[test]
    fn latest_unknown_skipped_without_lookup() {
        let agent = agent(
            spec(
                "claude-code",
                vec![InstallMethod::DirectDownload {
                    url: "https://claude.ai/install.sh".into(),
                    binary_name: "claude".into(),
                }],
            ),
            Some("1.0.0"),
        );
        let findings = run(&ctx(vec![agent], None));
        assert!(findings.is_empty());
    }
}
