use std::path::Path;

use anyhow::Result;

use agentry_agents::detector;
use agentry_agents::spec;
use agentry_skills::hub::SkillHub;

use crate::protocol::{AgentCapability, SkillLookupPayload, SkillLookupResultPayload};

/// Build a capability matrix from detected agents and installed skills.
pub fn build_capability_matrix(home_dir: &Path) -> Result<Vec<AgentCapability>> {
    // Use sync detection for each spec
    let specs = spec::all_agent_specs();
    let agents: Vec<_> = specs.iter().map(detector::detect_agent).collect();
    let hub = SkillHub::load(home_dir, &[])?;

    let mut capabilities = Vec::new();

    for agent in &agents {
        if !agent.installed {
            continue;
        }

        let mut caps = Vec::new();

        // Add capabilities based on agent type
        match agent.spec.id.as_str() {
            "claude-code" => {
                caps.extend_from_slice(&[
                    "code_generation".to_string(),
                    "code_review".to_string(),
                    "debugging".to_string(),
                    "refactoring".to_string(),
                    "testing".to_string(),
                    "documentation".to_string(),
                ]);
            }
            "continue" => {
                caps.extend_from_slice(&[
                    "code_completion".to_string(),
                    "code_editing".to_string(),
                    "chat".to_string(),
                ]);
            }
            "gemini-cli" => {
                caps.extend_from_slice(&[
                    "code_generation".to_string(),
                    "multi_modal".to_string(),
                    "research".to_string(),
                    "analysis".to_string(),
                ]);
            }
            "codex" => {
                caps.extend_from_slice(&[
                    "code_generation".to_string(),
                    "autonomous_coding".to_string(),
                    "task_execution".to_string(),
                ]);
            }
            "opencode" => {
                caps.extend_from_slice(&[
                    "code_editing".to_string(),
                    "terminal".to_string(),
                ]);
            }
            "amp" => {
                caps.extend_from_slice(&[
                    "autonomous_coding".to_string(),
                    "task_execution".to_string(),
                ]);
            }
            "firebender" => {
                caps.extend_from_slice(&[
                    "code_generation".to_string(),
                    "web_development".to_string(),
                ]);
            }
            "deepagents" => {
                caps.extend_from_slice(&[
                    "multi_agent".to_string(),
                    "orchestration".to_string(),
                    "task_decomposition".to_string(),
                ]);
            }
            "antigravity" => {
                caps.extend_from_slice(&[
                    "code_generation".to_string(),
                    "autonomous".to_string(),
                ]);
            }
            "warp" => {
                caps.extend_from_slice(&[
                    "terminal".to_string(),
                    "shell_commands".to_string(),
                    "devops".to_string(),
                ]);
            }
            _ => {
                caps.push("general".to_string());
            }
        }

        // Add capabilities from installed skills
        let mut skills = Vec::new();
        if let Some(skill_names) = agent.spec.skills_dir_name.as_ref() {
            // The agent has a skills directory
            let _ = skill_names; // Just marking it
        }

        // Check the hub for skills associated with this agent
        for skill in hub.skills.values() {
            if skill.installed {
                // If the skill is installed, all agents with skills dirs can use it
                skills.push(skill.name.clone());
            }
        }

        capabilities.push(AgentCapability {
            agent_id: agent.spec.id.clone(),
            agent_name: agent.spec.name.clone(),
            capabilities: caps,
            skills,
            model: agent.version.clone(),
        });
    }

    Ok(capabilities)
}

/// Route a prompt to the best-fit agent based on capabilities.
pub fn route_prompt(
    capabilities: &[AgentCapability],
    task_type: &str,
    _prompt: &str,
) -> Option<AgentCapability> {
    // Simple routing: find the agent whose capabilities best match the task type
    let mut best_match: Option<&AgentCapability> = None;
    let mut best_score = 0;

    for cap in capabilities {
        let mut score = 0;
        for c in &cap.capabilities {
            if c.contains(task_type) || task_type.contains(c) {
                score += 2;
            }
        }
        // Bonus for having skills
        score += cap.skills.len().min(3) as i32;

        if score > best_score {
            best_score = score;
            best_match = Some(cap);
        }
    }

    // If no capability match, use first available agent as fallback
    if best_match.is_none() && !capabilities.is_empty() {
        best_match = Some(&capabilities[0]);
    }

    best_match.cloned()
}

/// Handle a SkillLookup message by searching capabilities.
pub fn handle_skill_lookup(
    capabilities: &[AgentCapability],
    lookup: &SkillLookupPayload,
) -> SkillLookupResultPayload {
    let mut matched = Vec::new();

    for cap in capabilities {
        let mut is_match = false;
        for req_cap in &lookup.required_capabilities {
            for agent_cap in &cap.capabilities {
                if agent_cap.contains(req_cap) || req_cap.contains(agent_cap) {
                    is_match = true;
                    break;
                }
            }
            if is_match {
                break;
            }
        }

        // Also check task description
        if !is_match {
            for agent_cap in &cap.capabilities {
                if lookup.task_description.contains(agent_cap) {
                    is_match = true;
                    break;
                }
            }
        }

        if is_match {
            matched.push(cap.clone());
        }
    }

    SkillLookupResultPayload {
        lookup_id: lookup.id.clone(),
        from_agent: lookup.from_agent.clone(),
        matched_agents: matched,
        timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::AgentCapability;

    #[test]
    fn test_route_prompt() {
        let caps = vec![
            AgentCapability {
                agent_id: "claude-code".to_string(),
                agent_name: "Claude Code".to_string(),
                capabilities: vec![
                    "code_generation".to_string(),
                    "code_review".to_string(),
                ],
                skills: vec![],
                model: Some("2.1.50".to_string()),
            },
            AgentCapability {
                agent_id: "gemini-cli".to_string(),
                agent_name: "Gemini CLI".to_string(),
                capabilities: vec![
                    "research".to_string(),
                    "multi_modal".to_string(),
                ],
                skills: vec![],
                model: None,
            },
        ];

        let result = route_prompt(&caps, "code_review", "review this code");
        assert!(result.is_some());
        assert_eq!(result.unwrap().agent_id, "claude-code");
    }

    #[test]
    fn test_handle_skill_lookup() {
        let caps = vec![AgentCapability {
            agent_id: "claude-code".to_string(),
            agent_name: "Claude Code".to_string(),
            capabilities: vec!["code_generation".to_string()],
            skills: vec!["deploy-to-vercel".to_string()],
            model: None,
        }];

        let lookup = SkillLookupPayload {
            id: "lookup-1".to_string(),
            from_agent: "agentry".to_string(),
            task_description: "deploy this code".to_string(),
            required_capabilities: vec!["code_generation".to_string()],
            timestamp: "2026-04-09T12:00:00Z".to_string(),
        };

        let result = handle_skill_lookup(&caps, &lookup);
        assert_eq!(result.matched_agents.len(), 1);
        assert_eq!(result.matched_agents[0].agent_id, "claude-code");
    }

    #[test]
    fn test_route_prompt_fallback() {
        let caps = vec![AgentCapability {
            agent_id: "warp".to_string(),
            agent_name: "Warp".to_string(),
            capabilities: vec!["terminal".to_string()],
            skills: vec![],
            model: None,
        }];

        let result = route_prompt(&caps, "design", "design a system");
        // Should fall back to first available agent
        assert!(result.is_some());
    }
}