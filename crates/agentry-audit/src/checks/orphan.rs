use std::collections::HashSet;

use crate::engine::CheckContext;
use crate::report::{AuditFinding, FindingCategory, Severity};

pub fn run(ctx: &CheckContext) -> Vec<AuditFinding> {
    let mut findings = Vec::new();
    let canonical = canonical_filenames(ctx);
    for agent in &ctx.agents {
        if agent.spec.prompt_filename.ends_with('/')
            || is_directory_prompt(&agent.spec.prompt_filename)
        {
            continue;
        }
        let prompt_path = ctx
            .home_dir
            .join(&agent.spec.config_dir)
            .join(&agent.spec.prompt_filename);
        if !prompt_path.is_file() {
            continue;
        }
        let Some(filename) = prompt_path.file_name().and_then(|f| f.to_str()) else {
            continue;
        };
        if canonical.contains(filename) {
            continue;
        }
        findings.push(AuditFinding {
            check_id: "files.orphaned_prompt".to_string(),
            severity: Severity::Info,
            category: FindingCategory::OrphanedFiles,
            agent_id: Some(agent.spec.id.clone()),
            message: format!(
                "{} prompt file '{}' is not present in the canonical store",
                agent.spec.name,
                prompt_path.display()
            ),
            remediation: format!(
                "Import '{}' into '~/.agents/prompts/' or delete it",
                prompt_path.display()
            ),
            auto_fixable: false,
            fix: None,
            suggested_fix: None,
            evidence: Some(prompt_path.display().to_string()),
        });
    }
    findings
}

fn is_directory_prompt(prompt_filename: &str) -> bool {
    matches!(prompt_filename, "prompts" | "rules")
}

fn canonical_filenames(ctx: &CheckContext) -> HashSet<String> {
    let canonical_dir = ctx.home_dir.join(".agents").join("prompts");
    let mut names = HashSet::new();
    let Ok(entries) = std::fs::read_dir(&canonical_dir) else {
        return names;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(name) = path.file_name().and_then(|f| f.to_str()) {
                names.insert(name.to_string());
            }
        }
    }
    names
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

    fn spec(id: &str, config_dir: &str, prompt_filename: &str) -> AgentSpec {
        AgentSpec {
            id: id.to_string(),
            name: id.to_string(),
            cli_binary: id.to_string(),
            config_dir: config_dir.to_string(),
            prompt_filename: prompt_filename.to_string(),
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
            prompt_file_exists: true,
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

    #[test]
    fn orphaned_prompt_fires_when_not_in_canonical_store() {
        let tmp = TempDir::new("agentry_audit_orphan_absent");
        let gemini_dir = tmp.path().join(".gemini");
        std::fs::create_dir_all(&gemini_dir).unwrap();
        std::fs::write(gemini_dir.join("GEMINI.md"), "# GEMINI\n\nOrphan").unwrap();
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![agent(spec("gemini-cli", ".gemini", "GEMINI.md"))],
        ));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].check_id, "files.orphaned_prompt");
        assert_eq!(findings[0].severity, Severity::Info);
        assert_eq!(findings[0].category, FindingCategory::OrphanedFiles);
    }

    #[test]
    fn orphaned_prompt_skipped_when_in_canonical_store() {
        let tmp = TempDir::new("agentry_audit_orphan_canonical");
        let canonical_dir = tmp.path().join(".agents").join("prompts");
        let gemini_dir = tmp.path().join(".gemini");
        std::fs::create_dir_all(&canonical_dir).unwrap();
        std::fs::create_dir_all(&gemini_dir).unwrap();
        std::fs::write(canonical_dir.join("GEMINI.md"), "# GEMINI").unwrap();
        std::fs::write(gemini_dir.join("GEMINI.md"), "# GEMINI").unwrap();
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![agent(spec("gemini-cli", ".gemini", "GEMINI.md"))],
        ));
        assert!(findings.is_empty());
    }

    #[test]
    fn directory_prompts_are_skipped() {
        let tmp = TempDir::new("agentry_audit_orphan_directory");
        let continue_dir = tmp.path().join(".continue");
        std::fs::create_dir_all(continue_dir.join("prompts")).unwrap();
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![agent(spec("continue", ".continue", "prompts"))],
        ));
        assert!(findings.is_empty());
    }

    #[test]
    fn missing_prompt_file_is_skipped() {
        let tmp = TempDir::new("agentry_audit_orphan_no_file");
        std::fs::create_dir_all(tmp.path().join(".gemini")).unwrap();
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![agent(spec("gemini-cli", ".gemini", "GEMINI.md"))],
        ));
        assert!(findings.is_empty());
    }
}
