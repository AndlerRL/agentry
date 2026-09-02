use std::path::PathBuf;

use agentry_core::models::{
    AgentSpec, DetectedAgent, PromptFormat, PromptScope, SyncAction, SyncMapping, SyncPlan,
    UnifiedPrompt,
};

/// Default sync strategy: determine how a prompt should be synced to each agent.
pub fn plan_sync(
    prompt: &UnifiedPrompt,
    agents: &[DetectedAgent],
    home_dir: &std::path::Path,
) -> SyncPlan {
    let mut mappings = Vec::new();

    if prompt.frontmatter.contains_key("agentry-role") {
        return SyncPlan {
            prompt_id: prompt.id.clone(),
            mappings,
        };
    }

    for agent in agents {
        if !agent.installed {
            continue;
        }

        let (action, dest_path, target_format) = default_sync_action(prompt, &agent.spec, home_dir);

        mappings.push(SyncMapping {
            prompt_id: prompt.id.clone(),
            agent_id: agent.spec.id.clone(),
            destination: dest_path,
            target_format,
            action,
            status: agentry_core::models::SyncStatus::Missing, // Will be computed by executor
        });
    }

    // Add project-level syncs if the prompt is global
    if matches!(prompt.scope, PromptScope::Global) {
        // Project-level syncs are added separately by project_sync_plans
    }

    SyncPlan {
        prompt_id: prompt.id.clone(),
        mappings,
    }
}

/// Determine the default sync action for a prompt → agent pair.
fn default_sync_action(
    prompt: &UnifiedPrompt,
    spec: &AgentSpec,
    home_dir: &std::path::Path,
) -> (SyncAction, PathBuf, PromptFormat) {
    let config_dir = home_dir.join(&spec.config_dir);

    match spec.id.as_str() {
        // Continue is often the source — sync to prompts directory as XmlTagMd
        "continue" => {
            let dest = config_dir.join("prompts").join(prompt.canonical_filename());
            (SyncAction::Copy, dest, PromptFormat::XmlTagMd)
        }
        // Claude Code — copy as PlainMd to ~/.claude/CLAUDE.md or project-level
        "claude-code" => {
            let dest = config_dir.join("CLAUDE.md");
            (SyncAction::Copy, dest, PromptFormat::PlainMd)
        }
        // Gemini CLI — copy as PlainMd
        "gemini-cli" => {
            let dest = config_dir.join("GEMINI.md");
            (SyncAction::Copy, dest, PromptFormat::PlainMd)
        }
        // Codex — copy as PlainMd (32KiB limit)
        "codex" => {
            let dest = config_dir.join("AGENTS.md");
            (SyncAction::Copy, dest, PromptFormat::PlainMd)
        }
        // OpenCode — copy as FrontmatterMd
        "opencode" => {
            let dest = config_dir.join("AGENTS.md");
            (SyncAction::Copy, dest, PromptFormat::FrontmatterMd)
        }
        // Amp — copy as PlainMd
        "amp" => {
            let dest = config_dir.join("AGENTS.md");
            (SyncAction::Copy, dest, PromptFormat::PlainMd)
        }
        // Firebender — copy as MDC
        "firebender" => {
            let dest = config_dir
                .join("rules")
                .join(format!("{}.mdc", prompt.name));
            (SyncAction::Copy, dest, PromptFormat::Mdc)
        }
        // OpenClaw — copy as PlainMd
        "openclaw" => {
            let dest = config_dir.join("AGENTS.md");
            (SyncAction::Copy, dest, PromptFormat::PlainMd)
        }
        // DeepAgents — copy as PlainMd
        "deepagents" => {
            let dest = config_dir.join("AGENTS.md");
            (SyncAction::Copy, dest, PromptFormat::PlainMd)
        }
        // Antigravity — copy as FrontmatterMd
        "antigravity" => {
            let dest = config_dir.join("SKILL.md");
            (SyncAction::Copy, dest, PromptFormat::FrontmatterMd)
        }
        // Warp — copy as FrontmatterMd
        "warp" => {
            let dest = config_dir.join("AGENTS.md");
            (SyncAction::Copy, dest, PromptFormat::FrontmatterMd)
        }
        _ => (SyncAction::Skip, PathBuf::new(), PromptFormat::PlainMd),
    }
}

/// Generate project-level sync plans.
/// For each global prompt, create a copy in each project's CLAUDE.md (or equivalent).
pub fn project_sync_plans(
    prompt: &UnifiedPrompt,
    project_dirs: &[PathBuf],
    _home_dir: &std::path::Path,
) -> Vec<SyncMapping> {
    let mut mappings = Vec::new();

    if prompt.frontmatter.contains_key("agentry-role") {
        return mappings;
    }

    if !matches!(prompt.scope, PromptScope::Global) {
        return mappings;
    }

    for project_dir in project_dirs {
        if let Ok(entries) = std::fs::read_dir(project_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && path.join(".git").exists() {
                    // This is a git project — sync prompt as CLAUDE.md
                    let dest = path.join("CLAUDE.md");
                    mappings.push(SyncMapping {
                        prompt_id: prompt.id.clone(),
                        agent_id: format!(
                            "project:{}",
                            path.file_name().unwrap_or_default().to_string_lossy()
                        ),
                        destination: dest,
                        target_format: PromptFormat::PlainMd,
                        action: SyncAction::Copy,
                        status: agentry_core::models::SyncStatus::Missing,
                    });
                }
            }
        }
    }

    mappings
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn make_prompt(name: &str, format: PromptFormat) -> UnifiedPrompt {
        UnifiedPrompt {
            id: name.to_string(),
            name: name.to_string(),
            description: String::new(),
            frontmatter: BTreeMap::new(),
            body: "Test prompt content".to_string(),
            xml_tags: vec![],
            scope: PromptScope::Global,
            source_format: format,
            source_path: None,
        }
    }

    fn make_agent(id: &str, name: &str, installed: bool) -> DetectedAgent {
        let config_dir = match id {
            "claude-code" => ".claude".to_string(),
            other => format!(".{}", other),
        };
        DetectedAgent {
            spec: AgentSpec {
                id: id.to_string(),
                name: name.to_string(),
                cli_binary: id.to_string(),
                config_dir,
                prompt_filename: "AGENTS.md".to_string(),
                prompt_format: PromptFormat::PlainMd,
                skills_dir_name: None,
                max_size: None,
                install_methods: vec![],
            },
            installed,
            version: None,
            config_dir_exists: installed,
            prompt_file_exists: false,
            skills_dir: None,
            skills_symlink_pattern: None,
            installed_skills: vec![],
            detected_methods: vec![],
        }
    }

    #[test]
    fn test_plan_sync_includes_installed_agents() {
        let prompt = make_prompt("test", PromptFormat::PlainMd);
        let home = PathBuf::from("/tmp/testhome");
        let agents = vec![
            make_agent("claude-code", "Claude Code", true),
            make_agent("codex", "Codex", false), // not installed
        ];
        let plan = plan_sync(&prompt, &agents, &home);
        assert_eq!(plan.mappings.len(), 1); // only claude-code
        assert_eq!(plan.mappings[0].agent_id, "claude-code");
    }

    #[test]
    fn test_plan_sync_claude_code_destination() {
        let prompt = make_prompt("architect", PromptFormat::PlainMd);
        let home = PathBuf::from("/home/user");
        let agents = vec![make_agent("claude-code", "Claude Code", true)];
        let plan = plan_sync(&prompt, &agents, &home);
        assert_eq!(
            plan.mappings[0].destination,
            PathBuf::from("/home/user/.claude/CLAUDE.md")
        );
        assert_eq!(plan.mappings[0].target_format, PromptFormat::PlainMd);
    }

    #[test]
    fn test_plan_sync_excludes_role_marked_prompts() {
        let mut prompt = make_prompt("auditor", PromptFormat::PlainMd);
        prompt.frontmatter.insert(
            "agentry-role".to_string(),
            serde_yaml::Value::String("auditor".to_string()),
        );
        let home = PathBuf::from("/home/user");
        let agents = vec![make_agent("claude-code", "Claude Code", true)];
        let plan = plan_sync(&prompt, &agents, &home);
        assert!(plan.mappings.is_empty());
    }

    #[test]
    fn test_project_sync_plans_excludes_role_marked_prompts() {
        let mut prompt = make_prompt("auditor", PromptFormat::PlainMd);
        prompt.frontmatter.insert(
            "agentry-role".to_string(),
            serde_yaml::Value::String("auditor".to_string()),
        );
        let project = PathBuf::from("/tmp/proj");
        std::fs::create_dir_all(project.join(".git")).unwrap();
        let mappings = project_sync_plans(
            &prompt,
            std::slice::from_ref(&project),
            &PathBuf::from("/home"),
        );
        assert!(mappings.is_empty());
        std::fs::remove_dir_all(&project).unwrap();
    }
}
