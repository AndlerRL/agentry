use std::time::{Duration, SystemTime};

use crate::engine::CheckContext;
use crate::report::{AuditFinding, FindingCategory, Severity};

const STALE_AFTER: Duration = Duration::from_secs(90 * 24 * 60 * 60);

const CONFIG_FILES: &[(&str, &[&str])] = &[
    ("claude-code", &["settings.json", "CLAUDE.md"]),
    (
        "continue",
        &["config.yaml", "config.ts", ".continuerc.json"],
    ),
    ("gemini-cli", &["settings.json"]),
    ("codex", &["config.toml"]),
    (
        "opencode",
        &["../.config/opencode/opencode.json", "opencode.json"],
    ),
    ("amp", &["settings.json", "config.json"]),
    ("firebender", &["config.json", "config.toml"]),
    ("openclaw", &["config.yaml", "config.json"]),
    ("deepagents", &["config.json", "settings.json"]),
    ("antigravity", &["settings.json"]),
    ("warp", &["config.yaml", "settings.json"]),
];

pub fn run(ctx: &CheckContext) -> Vec<AuditFinding> {
    let mut findings = Vec::new();
    for agent in &ctx.agents {
        let Some(candidates) = config_files_for(&agent.spec.id) else {
            continue;
        };
        let Some(config_path) =
            first_existing(ctx.home_dir.join(&agent.spec.config_dir), candidates)
        else {
            continue;
        };
        findings.extend(unparseable(&agent.spec.id, &agent.spec.name, &config_path));
        findings.extend(stale(&agent.spec.id, &agent.spec.name, &config_path));
    }
    findings
}

fn config_files_for(agent_id: &str) -> Option<&'static [&'static str]> {
    CONFIG_FILES
        .iter()
        .find(|(id, _)| *id == agent_id)
        .map(|(_, files)| *files)
}

fn first_existing(
    config_dir: std::path::PathBuf,
    candidates: &[&str],
) -> Option<std::path::PathBuf> {
    candidates.iter().find_map(|file| {
        let path = config_dir.join(file);
        if path.is_file() {
            Some(path)
        } else {
            None
        }
    })
}

fn unparseable(agent_id: &str, agent_name: &str, path: &std::path::Path) -> Vec<AuditFinding> {
    let Some(content) = std::fs::read_to_string(path).ok() else {
        return Vec::new();
    };
    let parse_error = match path.extension().and_then(|e| e.to_str()) {
        Some("json") => serde_json::from_str::<serde_json::Value>(&content)
            .err()
            .map(|e| e.to_string()),
        Some("toml") => toml::from_str::<toml::Value>(&content)
            .err()
            .map(|e| e.to_string()),
        Some("yaml") | Some("yml") => serde_yaml::from_str::<serde_yaml::Value>(&content)
            .err()
            .map(|e| e.to_string()),
        _ => None,
    };
    let Some(error) = parse_error else {
        return Vec::new();
    };
    vec![AuditFinding {
        check_id: "config.unparseable".to_string(),
        severity: Severity::Warning,
        category: FindingCategory::Config,
        agent_id: Some(agent_id.to_string()),
        message: format!(
            "{} config file '{}' failed to parse",
            agent_name,
            path.display()
        ),
        remediation: format!("Fix the syntax error in '{}'", path.display()),
        auto_fixable: false,
        fix: None,
        evidence: Some(error),
    }]
}

fn stale(agent_id: &str, agent_name: &str, path: &std::path::Path) -> Vec<AuditFinding> {
    let Ok(metadata) = std::fs::metadata(path) else {
        return Vec::new();
    };
    let Ok(modified) = metadata.modified() else {
        return Vec::new();
    };
    let Ok(age) = SystemTime::now().duration_since(modified) else {
        return Vec::new();
    };
    if age <= STALE_AFTER {
        return Vec::new();
    }
    vec![AuditFinding {
        check_id: "config.stale".to_string(),
        severity: Severity::Info,
        category: FindingCategory::Config,
        agent_id: Some(agent_id.to_string()),
        message: format!(
            "{} config file '{}' has not been modified in {} days",
            agent_name,
            path.display(),
            age.as_secs() / (24 * 60 * 60)
        ),
        remediation: format!("Review '{}' for outdated settings", path.display()),
        auto_fixable: false,
        fix: None,
        evidence: Some(format!(
            "mtime={} age_days={}",
            chrono::DateTime::<chrono::Utc>::from(modified).to_rfc3339(),
            age.as_secs() / (24 * 60 * 60)
        )),
    }]
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

    fn spec(id: &str, config_dir: &str) -> AgentSpec {
        AgentSpec {
            id: id.to_string(),
            name: id.to_string(),
            cli_binary: id.to_string(),
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

    fn set_mtime_91_days_ago(path: &std::path::Path) {
        let old = SystemTime::now() - Duration::from_secs(91 * 24 * 60 * 60);
        let file = std::fs::File::options().write(true).open(path).unwrap();
        file.set_times(std::fs::FileTimes::new().set_modified(old))
            .unwrap();
    }

    #[test]
    fn unparseable_fires_for_invalid_json() {
        let tmp = TempDir::new("agentry_audit_config_invalid_json");
        let config_dir = tmp.path().join(".gemini");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("settings.json"), "{not valid json").unwrap();
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![agent(spec("gemini-cli", ".gemini"))],
        ));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].check_id, "config.unparseable");
        assert_eq!(findings[0].severity, Severity::Warning);
        assert!(!findings[0]
            .evidence
            .as_deref()
            .unwrap_or_default()
            .is_empty());
    }

    #[test]
    fn unparseable_skipped_for_valid_toml() {
        let tmp = TempDir::new("agentry_audit_config_valid_toml");
        let config_dir = tmp.path().join(".codex");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("config.toml"), "key = \"value\"\n").unwrap();
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![agent(spec("codex", ".codex"))],
        ));
        assert!(findings.is_empty());
    }

    #[test]
    fn unparseable_skipped_for_valid_yaml() {
        let tmp = TempDir::new("agentry_audit_config_valid_yaml");
        let config_dir = tmp.path().join(".continue");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("config.yaml"), "name: test\nversion: 1\n").unwrap();
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![agent(spec("continue", ".continue"))],
        ));
        assert!(findings.is_empty());
    }

    #[test]
    fn unparseable_skipped_when_no_config_file() {
        let tmp = TempDir::new("agentry_audit_config_absent");
        std::fs::create_dir_all(tmp.path().join(".gemini")).unwrap();
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![agent(spec("gemini-cli", ".gemini"))],
        ));
        assert!(findings.is_empty());
    }

    #[test]
    fn stale_fires_when_mtime_older_than_90_days() {
        let tmp = TempDir::new("agentry_audit_config_stale");
        let config_dir = tmp.path().join(".gemini");
        std::fs::create_dir_all(&config_dir).unwrap();
        let config_path = config_dir.join("settings.json");
        std::fs::write(&config_path, "{}").unwrap();
        set_mtime_91_days_ago(&config_path);
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![agent(spec("gemini-cli", ".gemini"))],
        ));
        let stale: Vec<_> = findings
            .iter()
            .filter(|f| f.check_id == "config.stale")
            .collect();
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].severity, Severity::Info);
    }

    #[test]
    fn stale_skipped_for_recent_mtime() {
        let tmp = TempDir::new("agentry_audit_config_fresh");
        let config_dir = tmp.path().join(".gemini");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("settings.json"), "{}").unwrap();
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![agent(spec("gemini-cli", ".gemini"))],
        ));
        assert!(findings.iter().all(|f| f.check_id != "config.stale"));
    }

    #[test]
    fn unknown_agent_id_is_skipped() {
        let tmp = TempDir::new("agentry_audit_config_unknown_agent");
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![agent(spec("mystery", ".mystery"))],
        ));
        assert!(findings.is_empty());
    }
}
