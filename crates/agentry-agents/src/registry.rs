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