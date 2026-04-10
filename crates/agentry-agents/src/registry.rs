use agentry_core::models::DetectedAgent;

use crate::detector::detect_all_agents;

/// Registry of all detected agents on the system.
pub struct AgentRegistry {
    agents: Vec<DetectedAgent>,
}

impl AgentRegistry {
    /// Create a new registry by detecting all agents.
    pub async fn detect() -> Self {
        let agents = detect_all_agents().await;
        Self { agents }
    }

    /// Create a registry from a pre-built list.
    pub fn from_list(agents: Vec<DetectedAgent>) -> Self {
        Self { agents }
    }

    /// Get all detected agents.
    pub fn agents(&self) -> &[DetectedAgent] {
        &self.agents
    }

    /// Get only installed agents.
    pub fn installed(&self) -> Vec<&DetectedAgent> {
        self.agents.iter().filter(|a| a.installed).collect()
    }

    /// Get an agent by ID.
    pub fn get_by_id(&self, id: &str) -> Option<&DetectedAgent> {
        self.agents.iter().find(|a| a.spec.id == id)
    }

    /// Count of installed agents.
    pub fn installed_count(&self) -> usize {
        self.agents.iter().filter(|a| a.installed).count()
    }

    /// Total count of known agents.
    pub fn total_count(&self) -> usize {
        self.agents.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentry_core::models::{AgentSpec, PromptFormat};

    /// Helper to create a minimal DetectedAgent for testing.
    fn make_agent(id: &str, installed: bool) -> DetectedAgent {
        DetectedAgent {
            spec: AgentSpec {
                id: id.into(),
                name: format!("{} Agent", id),
                cli_binary: id.into(),
                config_dir: format!(".{}", id),
                prompt_filename: "AGENTS.md".into(),
                prompt_format: PromptFormat::PlainMd,
                skills_dir_name: None,
                max_size: None,
            },
            installed,
            version: if installed {
                Some("1.0.0".into())
            } else {
                None
            },
            config_dir_exists: installed,
            prompt_file_exists: false,
            skills_dir: None,
            skills_symlink_pattern: None,
            installed_skills: Vec::new(),
        }
    }

    #[test]
    fn from_list_creates_registry() {
        let agents = vec![
            make_agent("alpha", true),
            make_agent("beta", false),
            make_agent("gamma", true),
        ];
        let registry = AgentRegistry::from_list(agents);
        assert_eq!(registry.total_count(), 3);
    }

    #[test]
    fn agents_returns_all() {
        let agents = vec![make_agent("alpha", true), make_agent("beta", false)];
        let registry = AgentRegistry::from_list(agents);
        assert_eq!(registry.agents().len(), 2);
        assert_eq!(registry.agents()[0].spec.id, "alpha");
        assert_eq!(registry.agents()[1].spec.id, "beta");
    }

    #[test]
    fn installed_filters_correctly() {
        let agents = vec![
            make_agent("alpha", true),
            make_agent("beta", false),
            make_agent("gamma", true),
        ];
        let registry = AgentRegistry::from_list(agents);
        let installed = registry.installed();
        assert_eq!(installed.len(), 2);
        let installed_ids: Vec<&str> = installed.iter().map(|a| a.spec.id.as_str()).collect();
        assert!(installed_ids.contains(&"alpha"));
        assert!(installed_ids.contains(&"gamma"));
        assert!(!installed_ids.contains(&"beta"));
    }

    #[test]
    fn installed_returns_empty_when_none() {
        let agents = vec![make_agent("alpha", false), make_agent("beta", false)];
        let registry = AgentRegistry::from_list(agents);
        assert!(registry.installed().is_empty());
    }

    #[test]
    fn get_by_id_finds_existing() {
        let agents = vec![make_agent("alpha", true), make_agent("beta", false)];
        let registry = AgentRegistry::from_list(agents);
        let found = registry.get_by_id("beta");
        assert!(found.is_some());
        assert_eq!(found.unwrap().spec.id, "beta");
    }

    #[test]
    fn get_by_id_returns_none_for_missing() {
        let agents = vec![make_agent("alpha", true)];
        let registry = AgentRegistry::from_list(agents);
        assert!(registry.get_by_id("nonexistent").is_none());
    }

    #[test]
    fn installed_count_matches() {
        let agents = vec![
            make_agent("alpha", true),
            make_agent("beta", false),
            make_agent("gamma", true),
            make_agent("delta", true),
        ];
        let registry = AgentRegistry::from_list(agents);
        assert_eq!(registry.installed_count(), 3);
    }

    #[test]
    fn installed_count_zero_when_none() {
        let agents = vec![make_agent("alpha", false), make_agent("beta", false)];
        let registry = AgentRegistry::from_list(agents);
        assert_eq!(registry.installed_count(), 0);
    }

    #[test]
    fn total_count_matches() {
        let agents = vec![
            make_agent("alpha", true),
            make_agent("beta", false),
            make_agent("gamma", true),
        ];
        let registry = AgentRegistry::from_list(agents);
        assert_eq!(registry.total_count(), 3);
    }

    #[test]
    fn total_count_empty_registry() {
        let registry = AgentRegistry::from_list(Vec::new());
        assert_eq!(registry.total_count(), 0);
        assert_eq!(registry.installed_count(), 0);
        assert!(registry.agents().is_empty());
    }

    #[test]
    fn installed_count_equals_installed_len() {
        let agents = vec![
            make_agent("alpha", true),
            make_agent("beta", false),
            make_agent("gamma", true),
        ];
        let registry = AgentRegistry::from_list(agents);
        assert_eq!(
            registry.installed_count(),
            registry.installed().len(),
            "installed_count should equal installed().len()"
        );
    }

    #[test]
    fn get_by_id_among_multiple() {
        let agents = vec![
            make_agent("aaa", false),
            make_agent("bbb", true),
            make_agent("ccc", false),
        ];
        let registry = AgentRegistry::from_list(agents);
        assert_eq!(registry.get_by_id("aaa").unwrap().installed, false);
        assert_eq!(registry.get_by_id("bbb").unwrap().installed, true);
        assert_eq!(registry.get_by_id("ccc").unwrap().installed, false);
    }

    #[test]
    fn registry_preserves_order() {
        let agents = vec![
            make_agent("first", true),
            make_agent("second", false),
            make_agent("third", true),
        ];
        let registry = AgentRegistry::from_list(agents);
        let ids: Vec<&str> = registry
            .agents()
            .iter()
            .map(|a| a.spec.id.as_str())
            .collect();
        assert_eq!(ids, vec!["first", "second", "third"]);
    }
}
