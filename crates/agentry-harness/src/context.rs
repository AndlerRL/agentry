use std::path::PathBuf;

use agentry_core::models::DetectedAgent;
use serde::{Deserialize, Serialize};

use agentry_audit::report::AuditReport;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HarnessConfig {
    #[serde(default)]
    pub harness: HarnessSection,
    #[serde(default)]
    pub local: LocalSection,
    #[serde(default)]
    pub onboarding: OnboardingSection,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HarnessSection {
    #[serde(default)]
    pub enabled_agents: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LocalSection {
    #[serde(default)]
    pub runtime: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OnboardingSection {
    #[serde(default)]
    pub setup_completed_at: Option<String>,
}

pub fn config_path(home_dir: &std::path::Path) -> PathBuf {
    home_dir.join(".agents").join("agentry.toml")
}

pub fn load_config(home_dir: &std::path::Path) -> HarnessConfig {
    let path = config_path(home_dir);
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(_) => return HarnessConfig::default(),
    };
    toml::from_str(&content).unwrap_or_else(|err| {
        eprintln!(
            "warning: failed to parse {}: {err}; using defaults",
            path.display()
        );
        HarnessConfig::default()
    })
}

pub struct HarnessContext {
    pub home_dir: PathBuf,
    pub detected_agents: Vec<DetectedAgent>,
    pub prompts: Vec<agentry_core::models::UnifiedPrompt>,
    pub config: HarnessConfig,
    pub report: Option<AuditReport>,
}

impl HarnessContext {
    pub fn new(
        home_dir: PathBuf,
        detected_agents: Vec<DetectedAgent>,
        prompts: Vec<agentry_core::models::UnifiedPrompt>,
    ) -> Self {
        let config = load_config(&home_dir);
        Self {
            home_dir,
            detected_agents,
            prompts,
            config,
            report: None,
        }
    }

    pub fn with_report(mut self, report: Option<AuditReport>) -> Self {
        self.report = report;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_home(prefix: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("{}_{}", prefix, std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn load_config_returns_defaults_when_file_missing() {
        let home = temp_home("agentry_test_ctx_missing");
        let config = load_config(&home);
        assert!(config.harness.enabled_agents.is_empty());
        assert!(config.local.runtime.is_none());
        assert!(config.onboarding.setup_completed_at.is_none());
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn load_config_parses_partial_file_with_defaults() {
        let home = temp_home("agentry_test_ctx_partial");
        let path = config_path(&home);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "[harness]\nenabled_agents = [\"claude-code\"]\n").unwrap();
        let config = load_config(&home);
        assert_eq!(config.harness.enabled_agents, vec!["claude-code"]);
        assert!(config.local.runtime.is_none());
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn load_config_parses_all_sections() {
        let home = temp_home("agentry_test_ctx_full");
        let path = config_path(&home);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            "[onboarding]\nsetup_completed_at = \"2026-09-01T12:00:00Z\"\n\n[harness]\nenabled_agents = [\"codex\"]\n\n[local]\nruntime = \"ollama\"\nmodel = \"qwen2.5-coder:7b\"\n",
        )
        .unwrap();
        let config = load_config(&home);
        assert_eq!(
            config.onboarding.setup_completed_at.as_deref(),
            Some("2026-09-01T12:00:00Z")
        );
        assert_eq!(config.harness.enabled_agents, vec!["codex"]);
        assert_eq!(config.local.runtime.as_deref(), Some("ollama"));
        assert_eq!(config.local.model.as_deref(), Some("qwen2.5-coder:7b"));
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn load_config_falls_back_to_defaults_on_malformed_file() {
        let home = temp_home("agentry_test_ctx_malformed");
        let path = config_path(&home);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "not [ valid toml").unwrap();
        let config = load_config(&home);
        assert!(config.harness.enabled_agents.is_empty());
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn context_new_loads_config_from_home() {
        let home = temp_home("agentry_test_ctx_new");
        let path = config_path(&home);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "[harness]\nenabled_agents = [\"zai\"]\n").unwrap();
        let ctx = HarnessContext::new(home.clone(), Vec::new(), Vec::new());
        assert_eq!(ctx.config.harness.enabled_agents, vec!["zai"]);
        assert!(ctx.report.is_none());
        std::fs::remove_dir_all(&home).unwrap();
    }
}
