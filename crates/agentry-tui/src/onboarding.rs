use std::path::Path;

use agentry_core::models::{DetectedAgent, InstallMethod};
use agentry_harness::context::HarnessConfig;

pub struct InstallOffer {
    pub agent_id: String,
    pub agent_name: String,
    pub method: InstallMethod,
    pub command: String,
}

pub struct AuditorSetupReport {
    pub config_written: bool,
    pub prompt_written: bool,
    pub collection_adopted: bool,
}

pub fn select_install_offers(agents: &[DetectedAgent]) -> Vec<InstallOffer> {
    agents
        .iter()
        .filter(|agent| !agent.installed)
        .filter_map(|agent| {
            let method = agent.spec.install_methods.iter().find(|method| {
                method.available_on_os()
                    && !matches!(method, InstallMethod::BuiltIn)
                    && method.install_command(None) != "echo 'No automatic install available'"
            })?;
            Some(InstallOffer {
                agent_id: agent.spec.id.clone(),
                agent_name: agent.spec.name.clone(),
                method: method.clone(),
                command: method.install_command(None),
            })
        })
        .collect()
}

pub fn default_harness_config(agents: &[DetectedAgent]) -> HarnessConfig {
    let mut config = HarnessConfig::default();
    config.harness.enabled_agents = agents
        .iter()
        .filter(|agent| agent.installed)
        .map(|agent| agent.spec.id.clone())
        .collect();
    config.local.runtime = Some("ollama".to_string());
    config.local.model = Some("qwen2.5-coder:7b".to_string());
    config.onboarding.setup_completed_at = Some(chrono::Utc::now().to_rfc3339());
    config
}

pub fn run_auditor_setup(home_dir: &Path) -> Result<AuditorSetupReport, String> {
    let config = agentry_auditor::config::load_config(home_dir);
    let mut config_written = false;
    if config.host_cli.is_none() {
        agentry_auditor::config::write_config(home_dir, &config)?;
        config_written = true;
    }
    let prompt_written = agentry_auditor::config::write_canonical_prompt_if_absent(home_dir)?;
    let collection_adopted = agentry_auditor::config::adopt_orphaned_collection(home_dir)?;
    Ok(AuditorSetupReport {
        config_written,
        prompt_written,
        collection_adopted,
    })
}

pub fn confirm(prompt: &str) -> bool {
    use std::io::Write;
    print!("{prompt} ");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    match std::io::stdin().read_line(&mut line) {
        Ok(_) => matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes"),
        Err(_) => false,
    }
}

pub fn run_install_command(command: &str) -> Result<bool, String> {
    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .status()
        .map_err(|err| format!("failed to run command: {err}"))?;
    Ok(status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_home(prefix: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("{}_{}", prefix, std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn detected_agent(
        id: &str,
        name: &str,
        installed: bool,
        methods: Vec<InstallMethod>,
    ) -> DetectedAgent {
        DetectedAgent {
            spec: agentry_core::models::AgentSpec {
                id: id.to_string(),
                name: name.to_string(),
                cli_binary: id.to_string(),
                config_dir: format!(".{id}"),
                prompt_filename: "AGENTS.md".to_string(),
                prompt_format: agentry_core::models::PromptFormat::PlainMd,
                skills_dir_name: None,
                max_size: None,
                install_methods: methods,
            },
            installed,
            version: None,
            config_dir_exists: false,
            prompt_file_exists: false,
            skills_dir: None,
            skills_symlink_pattern: None,
            installed_skills: Vec::new(),
            detected_methods: Vec::new(),
        }
    }

    fn other_method(cmd: &str) -> InstallMethod {
        InstallMethod::Other {
            description: "test".to_string(),
            install_cmd: cmd.to_string(),
        }
    }

    #[test]
    fn select_install_offers_skips_installed_agents() {
        let agents = vec![
            detected_agent(
                "installed-a",
                "Installed A",
                true,
                vec![other_method("true")],
            ),
            detected_agent("missing-b", "Missing B", false, vec![other_method("true")]),
        ];
        let offers = select_install_offers(&agents);
        assert_eq!(offers.len(), 1);
        assert_eq!(offers[0].agent_id, "missing-b");
        assert_eq!(offers[0].command, "true");
    }

    #[test]
    fn select_install_offers_picks_first_available_method() {
        let agents = vec![detected_agent(
            "missing-a",
            "Missing A",
            false,
            vec![other_method("first"), other_method("second")],
        )];
        let offers = select_install_offers(&agents);
        assert_eq!(offers.len(), 1);
        assert_eq!(offers[0].command, "first");
    }

    #[test]
    fn select_install_offers_skips_agents_without_methods() {
        let agents = vec![detected_agent("missing-a", "Missing A", false, Vec::new())];
        assert!(select_install_offers(&agents).is_empty());
    }

    #[test]
    fn select_install_offers_skips_builtin_methods() {
        let agents = vec![detected_agent(
            "missing-a",
            "Missing A",
            false,
            vec![InstallMethod::BuiltIn],
        )];
        assert!(select_install_offers(&agents).is_empty());
    }

    #[test]
    fn select_install_offers_skips_noop_commands() {
        let agents = vec![detected_agent(
            "missing-a",
            "Missing A",
            false,
            vec![InstallMethod::JetBrainsPlugin {
                plugin_id: "com.example".to_string(),
            }],
        )];
        assert!(select_install_offers(&agents).is_empty());
    }

    #[test]
    fn default_harness_config_enables_installed_agents() {
        let agents = vec![
            detected_agent("installed-a", "Installed A", true, Vec::new()),
            detected_agent("missing-b", "Missing B", false, Vec::new()),
        ];
        let config = default_harness_config(&agents);
        assert_eq!(config.harness.enabled_agents, vec!["installed-a"]);
        assert_eq!(config.local.runtime.as_deref(), Some("ollama"));
        assert_eq!(config.local.model.as_deref(), Some("qwen2.5-coder:7b"));
        assert!(config.onboarding.setup_completed_at.is_some());
    }

    #[test]
    fn run_auditor_setup_is_idempotent() {
        let home = temp_home("agentry_test_onb_auditor");
        let first = run_auditor_setup(&home).unwrap();
        assert!(first.config_written);
        assert!(first.prompt_written);
        assert!(!first.collection_adopted);
        let path = agentry_harness::context::config_path(&home);
        let content_before = std::fs::read_to_string(&path).unwrap();
        let second = run_auditor_setup(&home).unwrap();
        assert!(!second.prompt_written);
        assert!(!second.collection_adopted);
        let content_after = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content_before, content_after);
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn run_auditor_setup_preserves_existing_auditor_config() {
        let home = temp_home("agentry_test_onb_audcfg");
        let config = agentry_auditor::config::AuditorConfig {
            host_cli: Some("codex".to_string()),
            ..Default::default()
        };
        agentry_auditor::config::write_config(&home, &config).unwrap();
        let report = run_auditor_setup(&home).unwrap();
        assert!(!report.config_written);
        assert!(report.prompt_written);
        std::fs::remove_dir_all(&home).unwrap();
    }
}
