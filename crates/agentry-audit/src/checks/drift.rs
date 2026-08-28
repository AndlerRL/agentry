use agentry_core::format::{convert_to, converter_for};
use agentry_core::models::{PromptFormat, PromptScope, SyncAction, SyncMapping, UnifiedPrompt};
use agentry_sync::planner::plan_sync;

use crate::engine::CheckContext;
use crate::report::{AuditFinding, FindingCategory, Severity};

pub fn run(ctx: &CheckContext) -> Vec<AuditFinding> {
    ctx.prompts
        .iter()
        .filter(|prompt| matches!(prompt.scope, PromptScope::Global))
        .filter_map(|prompt| cross_agent_finding(ctx, prompt))
        .collect()
}

struct AgentSnapshot {
    agent_id: String,
    content: String,
}

fn cross_agent_finding(ctx: &CheckContext, prompt: &UnifiedPrompt) -> Option<AuditFinding> {
    let snapshots = agent_snapshots(ctx, prompt);
    if snapshots.len() < 2 {
        return None;
    }
    if snapshots
        .iter()
        .all(|snapshot| snapshot.content == snapshots[0].content)
    {
        return None;
    }
    Some(finding(prompt, &snapshots))
}

fn agent_snapshots(ctx: &CheckContext, prompt: &UnifiedPrompt) -> Vec<AgentSnapshot> {
    let plan = plan_sync(prompt, &ctx.agents, &ctx.home_dir);
    let mut snapshots = Vec::new();
    for mapping in &plan.mappings {
        if let Some(snapshot) = agent_snapshot(ctx, mapping) {
            snapshots.push(snapshot);
        }
    }
    snapshots.sort_by(|a, b| a.agent_id.cmp(&b.agent_id));
    snapshots
}

fn agent_snapshot(ctx: &CheckContext, mapping: &SyncMapping) -> Option<AgentSnapshot> {
    if mapping.action == SyncAction::Skip {
        return None;
    }
    let agent = ctx
        .agents
        .iter()
        .find(|agent| agent.spec.id == mapping.agent_id)?;
    if is_directory_prompt(&agent.spec.prompt_filename) {
        return None;
    }
    if !mapping.destination.is_file() {
        return None;
    }
    let raw = std::fs::read_to_string(&mapping.destination).ok()?;
    let parsed = converter_for(agent.spec.prompt_format)
        .parse(
            mapping.prompt_id.as_str(),
            &raw,
            Some(mapping.destination.clone()),
        )
        .ok()?;
    let normalized = convert_to(&parsed, PromptFormat::PlainMd).ok()?;
    Some(AgentSnapshot {
        agent_id: agent.spec.id.clone(),
        content: normalized.trim().to_string(),
    })
}

fn is_directory_prompt(prompt_filename: &str) -> bool {
    prompt_filename.ends_with('/') || matches!(prompt_filename, "prompts" | "rules")
}

fn finding(prompt: &UnifiedPrompt, snapshots: &[AgentSnapshot]) -> AuditFinding {
    AuditFinding {
        check_id: "drift.cross_agent".to_string(),
        severity: Severity::Info,
        category: FindingCategory::CrossAgentDrift,
        agent_id: None,
        message: format!(
            "Prompt '{}' content has diverged across agents: {}",
            prompt.name,
            agent_list(snapshots, ", ")
        ),
        remediation: format!(
            "Pick the canonical content for '{}' and re-sync every agent with 'agentry sync --prompt {}'",
            prompt.name, prompt.id
        ),
        auto_fixable: false,
        fix: None,
        evidence: Some(format!(
            "agents={} diff={}",
            agent_list(snapshots, ","),
            diff_excerpt(snapshots)
        )),
    }
}

fn agent_list(snapshots: &[AgentSnapshot], separator: &str) -> String {
    snapshots
        .iter()
        .map(|snapshot| snapshot.agent_id.as_str())
        .collect::<Vec<_>>()
        .join(separator)
}

fn diff_excerpt(snapshots: &[AgentSnapshot]) -> String {
    let first = &snapshots[0];
    for other in &snapshots[1..] {
        if other.content == first.content {
            continue;
        }
        let (left, right) = first_divergent_lines(&first.content, &other.content);
        let (left, right) = if left == right {
            (excerpt(&first.content), excerpt(&other.content))
        } else {
            (left, right)
        };
        return format!(
            "{} {} vs {} {}",
            first.agent_id, left, other.agent_id, right
        );
    }
    String::new()
}

fn first_divergent_lines(a: &str, b: &str) -> (String, String) {
    let mut lines_a = a.lines();
    let mut lines_b = b.lines();
    loop {
        match (lines_a.next(), lines_b.next()) {
            (None, None) => return (String::new(), String::new()),
            (left, right) => {
                let left = left.unwrap_or("").trim();
                let right = right.unwrap_or("").trim();
                if left != right {
                    return (excerpt(left), excerpt(right));
                }
            }
        }
    }
}

fn excerpt(content: &str) -> String {
    const LIMIT: usize = 60;
    if content.chars().count() <= LIMIT {
        format!("'{}'", content)
    } else {
        let cut: String = content.chars().take(LIMIT).collect();
        format!("'{}...'", cut)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentry_core::models::{AgentSpec, DetectedAgent};
    use std::collections::BTreeMap;
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

    fn prompt(name: &str, body: &str) -> UnifiedPrompt {
        UnifiedPrompt {
            id: name.to_string(),
            name: name.to_string(),
            description: String::new(),
            frontmatter: BTreeMap::new(),
            body: body.to_string(),
            xml_tags: Vec::new(),
            scope: PromptScope::Global,
            source_format: PromptFormat::PlainMd,
            source_path: None,
        }
    }

    fn agent(id: &str, config_dir: &str, prompt_filename: &str) -> DetectedAgent {
        DetectedAgent {
            spec: AgentSpec {
                id: id.to_string(),
                name: id.to_string(),
                cli_binary: id.to_string(),
                config_dir: config_dir.to_string(),
                prompt_filename: prompt_filename.to_string(),
                prompt_format: PromptFormat::PlainMd,
                skills_dir_name: None,
                max_size: None,
                install_methods: Vec::new(),
            },
            installed: true,
            version: None,
            config_dir_exists: true,
            prompt_file_exists: true,
            skills_dir: None,
            skills_symlink_pattern: None,
            installed_skills: Vec::new(),
            detected_methods: Vec::new(),
        }
    }

    fn ctx(home: PathBuf, agents: Vec<DetectedAgent>, prompts: Vec<UnifiedPrompt>) -> CheckContext {
        CheckContext {
            home_dir: home,
            agents,
            prompts,
            version_lookup: None,
            binary_on_path: Vec::new(),
        }
    }

    #[test]
    fn identical_content_across_agents_does_not_fire() {
        let tmp = TempDir::new("agentry_audit_cross_drift_identical");
        let claude = tmp.path().join(".claude");
        let codex = tmp.path().join(".codex");
        std::fs::create_dir_all(&claude).unwrap();
        std::fs::create_dir_all(&codex).unwrap();
        std::fs::write(claude.join("CLAUDE.md"), "# Architect rules\n").unwrap();
        std::fs::write(codex.join("AGENTS.md"), "# Architect rules\n").unwrap();
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![
                agent("claude-code", ".claude", "CLAUDE.md"),
                agent("codex", ".codex", "AGENTS.md"),
            ],
            vec![prompt("architect", "# Architect rules")],
        ));
        assert!(findings.is_empty());
    }

    #[test]
    fn differing_content_across_agents_fires_with_both_ids() {
        let tmp = TempDir::new("agentry_audit_cross_drift_two");
        let claude = tmp.path().join(".claude");
        let codex = tmp.path().join(".codex");
        std::fs::create_dir_all(&claude).unwrap();
        std::fs::create_dir_all(&codex).unwrap();
        std::fs::write(claude.join("CLAUDE.md"), "# Architect rules\n").unwrap();
        std::fs::write(codex.join("AGENTS.md"), "# Codex rules\n").unwrap();
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![
                agent("claude-code", ".claude", "CLAUDE.md"),
                agent("codex", ".codex", "AGENTS.md"),
            ],
            vec![prompt("architect", "# Architect rules")],
        ));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].check_id, "drift.cross_agent");
        assert_eq!(findings[0].severity, Severity::Info);
        assert_eq!(findings[0].category, FindingCategory::CrossAgentDrift);
        assert_eq!(findings[0].agent_id, None);
        assert!(!findings[0].auto_fixable);
        assert!(findings[0].fix.is_none());
        assert!(!findings[0].message.is_empty());
        assert!(!findings[0].remediation.is_empty());
        let evidence = findings[0].evidence.as_deref().unwrap_or_default();
        assert!(evidence.contains("claude-code"));
        assert!(evidence.contains("codex"));
        assert!(evidence.contains("Architect"));
        assert!(evidence.contains("Codex"));
    }

    #[test]
    fn fires_when_one_of_three_agents_diverges() {
        let tmp = TempDir::new("agentry_audit_cross_drift_three");
        for dir in [".claude", ".codex", ".gemini"] {
            std::fs::create_dir_all(tmp.path().join(dir)).unwrap();
        }
        std::fs::write(
            tmp.path().join(".claude").join("CLAUDE.md"),
            "# Architect rules\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join(".codex").join("AGENTS.md"),
            "# Architect rules\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join(".gemini").join("GEMINI.md"),
            "# Gemini rules\n",
        )
        .unwrap();
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![
                agent("claude-code", ".claude", "CLAUDE.md"),
                agent("codex", ".codex", "AGENTS.md"),
                agent("gemini-cli", ".gemini", "GEMINI.md"),
            ],
            vec![prompt("architect", "# Architect rules")],
        ));
        assert_eq!(findings.len(), 1);
        let evidence = findings[0].evidence.as_deref().unwrap_or_default();
        assert!(evidence.contains("claude-code"));
        assert!(evidence.contains("codex"));
        assert!(evidence.contains("gemini-cli"));
        assert!(findings[0].message.contains("architect"));
    }

    #[test]
    fn single_agent_does_not_fire() {
        let tmp = TempDir::new("agentry_audit_cross_drift_single");
        let dir = tmp.path().join(".claude");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("CLAUDE.md"), "# Architect rules\n").unwrap();
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![agent("claude-code", ".claude", "CLAUDE.md")],
            vec![prompt("architect", "# Architect rules")],
        ));
        assert!(findings.is_empty());
    }
}
