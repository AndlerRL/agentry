use crate::engine::CheckContext;
use crate::report::AuditFinding;

pub mod acp;
pub mod auth;
pub mod config;
pub mod drift;
pub mod install;
pub mod openclaw;
pub mod orphan;
pub mod prompt;
pub mod skills;
pub mod sync;
pub mod version;

pub fn run_all(ctx: &CheckContext) -> Vec<AuditFinding> {
    let mut findings = install::run(ctx);
    findings.extend(version::run(ctx));
    findings.extend(config::run(ctx));
    findings.extend(prompt::run(ctx));
    findings.extend(sync::run(ctx));
    findings.extend(drift::run(ctx));
    findings.extend(skills::run(ctx));
    findings.extend(auth::run(ctx));
    findings.extend(orphan::run(ctx));
    findings.extend(openclaw::run(ctx));
    findings.extend(acp::run(ctx));
    findings
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

    fn agent_spec(config_dir: &str, prompt_filename: &str) -> AgentSpec {
        AgentSpec {
            id: "gemini-cli".to_string(),
            name: "Gemini CLI".to_string(),
            cli_binary: "gemini".to_string(),
            config_dir: config_dir.to_string(),
            prompt_filename: prompt_filename.to_string(),
            prompt_format: PromptFormat::PlainMd,
            skills_dir_name: None,
            max_size: None,
            install_methods: vec![InstallMethod::Npm {
                package: "@google/gemini-cli".to_string(),
            }],
        }
    }

    fn detected_agent(spec: AgentSpec) -> DetectedAgent {
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
    fn run_all_concatenates_all_checks_in_catalog_order() {
        let tmp = TempDir::new("agentry_audit_run_all_empty");
        let _minimal_findings = run_all(&ctx(tmp.path().clone(), Vec::new()));

        let gemini_dir = tmp.path().join(".gemini");
        std::fs::create_dir_all(&gemini_dir).unwrap();
        std::fs::write(gemini_dir.join("GEMINI.md"), "# GEMINI\n\nOrphan").unwrap();
        let agent = detected_agent(agent_spec(".gemini", "GEMINI.md"));
        let findings = run_all(&ctx(tmp.path().clone(), vec![agent]));

        assert!(findings
            .iter()
            .any(|f| f.check_id == "files.orphaned_prompt"));
        assert!(findings
            .iter()
            .any(|f| f.check_id == "install.binary_missing"));
        assert!(findings
            .iter()
            .enumerate()
            .any(|(i, f)| f.check_id == "files.orphaned_prompt"
                && findings[..i].iter().all(
                    |p| !p.check_id.starts_with("openclaw.") && !p.check_id.starts_with("acp.")
                )));
    }
}
