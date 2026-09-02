use std::io::{self, Write};
use std::path::Path;

use agentry_agents::{all_agent_specs, detect_agent};
use agentry_core::discovery::discover_prompts;
use agentry_core::models::{DetectedAgent, SyncAction, DEFAULT_PROJECT_DIR};
use agentry_sync::executor::execute_sync;
use agentry_sync::planner::plan_sync;
use serde::{Deserialize, Serialize};

use crate::report::{AuditFinding, AuditReport, FixAction};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixOutcome {
    pub check_id: String,
    pub agent_id: Option<String>,
    pub success: bool,
    pub message: String,
}

fn is_safe_shell_arg(arg: &str) -> bool {
    arg.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/' | '@' | '='))
}

pub fn is_safe_shell_command(command: &str) -> bool {
    if command.trim().is_empty() || command.contains(|c: char| c.is_whitespace() && c != ' ') {
        return false;
    }
    command.split(' ').all(is_safe_shell_arg)
}

pub fn apply_fix(finding: &AuditFinding, home_dir: &Path) -> FixOutcome {
    let (success, message) = match &finding.fix {
        Some(action) => {
            if let Err(reason) = validate(action, home_dir) {
                (false, reason)
            } else {
                execute_fix_action(action, &finding.check_id, home_dir)
            }
        }
        None => (
            false,
            format!("finding {} has no fix action", finding.check_id),
        ),
    };
    FixOutcome {
        check_id: finding.check_id.clone(),
        agent_id: finding.agent_id.clone(),
        success,
        message,
    }
}

pub fn validate(fix: &FixAction, home_dir: &Path) -> Result<(), String> {
    match fix {
        FixAction::ShellCommand { command, .. } => {
            if is_safe_shell_command(command) {
                Ok(())
            } else {
                Err(format!("refused unsafe shell command: {command}"))
            }
        }
        FixAction::FileWrite { .. } => Ok(()),
        FixAction::FileRemove { .. } => Ok(()),
        FixAction::SymlinkRecreate { path, target } => {
            if !path.starts_with(home_dir) {
                return Err(format!(
                    "refused symlink outside {}: {}",
                    home_dir.display(),
                    path.display()
                ));
            }
            if Path::new(target).is_absolute() {
                return Err(format!("refused absolute symlink target: {target}"));
            }
            if target.split('/').any(|component| component.is_empty()) {
                return Err(format!(
                    "refused symlink target with empty components: {target}"
                ));
            }
            Ok(())
        }
        FixAction::SyncPrompt { .. } => Ok(()),
    }
}

fn execute_fix_action(fix: &FixAction, check_id: &str, home_dir: &Path) -> (bool, String) {
    match fix {
        FixAction::ShellCommand { command, .. } => run_shell_command(command),
        FixAction::FileWrite { path, content } => write_file(path, content),
        FixAction::FileRemove { path } => remove_file(path),
        FixAction::SymlinkRecreate { path, target } => recreate_symlink(path, target, home_dir),
        FixAction::SyncPrompt {
            prompt_id,
            agent_id,
        } => {
            let outcome = apply_sync_prompt(prompt_id, agent_id, check_id, home_dir);
            (outcome.success, outcome.message)
        }
    }
}

pub fn apply_sync_prompt(
    prompt_id: &str,
    agent_id: &str,
    check_id: &str,
    home_dir: &Path,
) -> FixOutcome {
    let agents = all_agent_specs()
        .iter()
        .map(detect_agent)
        .collect::<Vec<_>>();
    apply_sync_prompt_with_agents(prompt_id, agent_id, check_id, home_dir, &agents)
}

pub fn apply_sync_prompt_with_agents(
    prompt_id: &str,
    agent_id: &str,
    check_id: &str,
    home_dir: &Path,
    agents: &[DetectedAgent],
) -> FixOutcome {
    let fail = |message: String| FixOutcome {
        check_id: check_id.to_string(),
        agent_id: Some(agent_id.to_string()),
        success: false,
        message,
    };
    let prompts = discover_prompts(home_dir, &[home_dir.join(DEFAULT_PROJECT_DIR)]);
    let Some(prompt) = prompts
        .iter()
        .find(|p| p.id == prompt_id || p.name == prompt_id)
    else {
        return fail(format!("prompt '{prompt_id}' not found"));
    };
    let plan = plan_sync(prompt, agents, home_dir);
    let Some(mapping) = plan
        .mappings
        .iter()
        .find(|m| m.agent_id == agent_id && m.action != SyncAction::Skip)
    else {
        return fail(format!("no sync mapping for agent '{agent_id}'"));
    };
    let shared = prompts
        .iter()
        .filter(|other| other.id != prompt.id)
        .filter(|other| {
            plan_sync(other, agents, home_dir).mappings.iter().any(|m| {
                m.agent_id == agent_id
                    && m.action != SyncAction::Skip
                    && m.destination == mapping.destination
            })
        })
        .count();
    if shared > 0 {
        return fail(format!(
            "destination {} is shared by {} prompts; run agentry sync --all to rebuild all prompts for this agent",
            mapping.destination.display(),
            shared + 1
        ));
    }
    let results = execute_sync(prompt, std::slice::from_ref(mapping), false);
    let result = &results[0];
    FixOutcome {
        check_id: check_id.to_string(),
        agent_id: Some(agent_id.to_string()),
        success: result.success,
        message: if result.success {
            format!(
                "synced {} to {}",
                prompt.name,
                mapping.destination.display()
            )
        } else {
            result.message.clone()
        },
    }
}

fn run_shell_command(command: &str) -> (bool, String) {
    if !is_safe_shell_command(command) {
        return (false, format!("refused unsafe shell command: {command}"));
    }
    match std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .output()
    {
        Ok(output) => {
            let status = output.status;
            (
                status.success(),
                format!("command exited {status}: {command}"),
            )
        }
        Err(err) => (false, format!("command {command} failed: {err}")),
    }
}

fn write_file(path: &Path, content: &str) -> (bool, String) {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => {
            if let Err(err) = std::fs::create_dir_all(parent) {
                return (
                    false,
                    format!("failed to create {}: {err}", parent.display()),
                );
            }
        }
        Some(_) => {}
        None => return (false, format!("path {} has no parent", path.display())),
    }
    match std::fs::write(path, content) {
        Ok(()) => (true, format!("wrote {}", path.display())),
        Err(err) => (false, format!("failed to write {}: {err}", path.display())),
    }
}

fn remove_file(path: &Path) -> (bool, String) {
    match std::fs::remove_file(path) {
        Ok(()) => (true, format!("removed {}", path.display())),
        Err(err) => (false, format!("failed to remove {}: {err}", path.display())),
    }
}

fn recreate_symlink(path: &Path, target: &str, home_dir: &Path) -> (bool, String) {
    if !path.starts_with(home_dir) {
        return (
            false,
            format!(
                "refused symlink outside {}: {}",
                home_dir.display(),
                path.display()
            ),
        );
    }
    if Path::new(target).is_absolute() {
        return (false, format!("refused absolute symlink target: {target}"));
    }
    if target.split('/').any(|component| component.is_empty()) {
        return (
            false,
            format!("refused symlink target with empty components: {target}"),
        );
    }
    if path.symlink_metadata().is_ok() {
        if let Err(err) = std::fs::remove_file(path) {
            if err.kind() != std::io::ErrorKind::NotFound {
                return (
                    false,
                    format!("failed to remove existing {}: {err}", path.display()),
                );
            }
        }
    }
    match std::os::unix::fs::symlink(target, path) {
        Ok(()) => match path.canonicalize() {
            Ok(_) => (true, format!("symlinked {} -> {}", path.display(), target)),
            Err(err) => (
                false,
                format!(
                    "target does not exist after recreation: {} -> {}: {err}",
                    path.display(),
                    target
                ),
            ),
        },
        Err(err) => (
            false,
            format!("failed to symlink {}: {err}", path.display()),
        ),
    }
}

pub fn fixable_findings(report: &AuditReport) -> Vec<&AuditFinding> {
    let mut fixable: Vec<&AuditFinding> = report
        .agents
        .iter()
        .flat_map(|agent| agent.findings.iter())
        .chain(report.global_findings.iter())
        .filter(|finding| finding.auto_fixable && finding.fix.is_some())
        .collect();
    fixable.sort_by_key(|finding| finding.severity);
    fixable
}

pub fn apply_fixes(findings: &[&AuditFinding], home_dir: &Path, yes: bool) -> Vec<FixOutcome> {
    let mut read_input = || {
        let mut answer = String::new();
        let _ = io::stdin().read_line(&mut answer);
        answer
    };
    apply_fixes_with_input(findings, home_dir, yes, &mut read_input)
}

fn apply_fixes_with_input(
    findings: &[&AuditFinding],
    home_dir: &Path,
    yes: bool,
    read_input: &mut dyn FnMut() -> String,
) -> Vec<FixOutcome> {
    let mut outcomes = Vec::new();
    for finding in findings {
        let Some(fix) = finding.fix.as_ref() else {
            continue;
        };
        if !yes {
            println!("[{}] {}", finding.check_id, finding.message);
            println!("  fix: {}", fix_description(fix));
            if !confirm(read_input) {
                outcomes.push(FixOutcome {
                    check_id: finding.check_id.clone(),
                    agent_id: finding.agent_id.clone(),
                    success: false,
                    message: "skipped by user".to_string(),
                });
                continue;
            }
        }
        outcomes.push(apply_fix(finding, home_dir));
    }
    outcomes
}

fn confirm(read_input: &mut dyn FnMut() -> String) -> bool {
    print!("  apply? [y/N] ");
    let _ = io::stdout().flush();
    let answer = read_input().trim().to_ascii_lowercase();
    answer == "y" || answer == "yes"
}

fn fix_description(fix: &FixAction) -> String {
    match fix {
        FixAction::ShellCommand {
            description,
            command,
        } => format!("{description}: {command}"),
        FixAction::FileWrite { path, .. } => format!("write {}", path.display()),
        FixAction::FileRemove { path } => format!("remove {}", path.display()),
        FixAction::SymlinkRecreate { path, target } => {
            format!("symlink {} -> {}", path.display(), target)
        }
        FixAction::SyncPrompt {
            prompt_id,
            agent_id,
        } => format!("sync prompt {prompt_id} for {agent_id}"),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    use chrono::Utc;

    use super::*;
    use crate::report::{AuditSummary, FindingCategory, Severity};
    use agentry_core::models::{AgentSpec, PromptFormat, SyncStatus};
    use agentry_sync::executor::check_sync_status;

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(prefix: &str) -> Self {
            let path = std::env::temp_dir().join(format!("{}_{}", prefix, std::process::id()));
            std::fs::create_dir_all(&path).expect("failed to create temp dir");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn finding(check_id: &str, fix: Option<FixAction>, auto_fixable: bool) -> AuditFinding {
        AuditFinding {
            check_id: check_id.to_string(),
            severity: Severity::Warning,
            category: FindingCategory::Config,
            agent_id: None,
            message: format!("finding {check_id}"),
            remediation: "run the fix".to_string(),
            auto_fixable,
            fix,
            evidence: None,
        }
    }

    fn report(findings: Vec<AuditFinding>) -> AuditReport {
        AuditReport {
            generated_at: Utc::now(),
            machine_id: "test-machine".to_string(),
            agents: Vec::new(),
            global_findings: findings,
            summary: AuditSummary {
                total_findings: 0,
                by_severity: BTreeMap::new(),
                by_category: BTreeMap::new(),
                auto_fixable_count: 0,
                healthy_agents: 0,
                degraded_agents: 0,
            },
            schema_version: 1,
        }
    }

    #[test]
    fn test_is_safe_shell_arg_rejects_semicolon_injection() {
        assert!(!is_safe_shell_arg("foo; rm -rf ~"));
        assert!(!is_safe_shell_command("foo; rm -rf ~"));
    }

    #[test]
    fn test_is_safe_shell_arg_rejects_command_substitution() {
        assert!(!is_safe_shell_arg("foo$(reboot)"));
        assert!(!is_safe_shell_command("foo$(reboot)"));
    }

    #[test]
    fn test_is_safe_shell_arg_accepts_safe_names() {
        assert!(is_safe_shell_arg("normal-name_1.0"));
        assert!(is_safe_shell_arg("path/with/slashes"));
    }

    #[test]
    fn test_is_safe_shell_command_accepts_producer_commands() {
        assert!(is_safe_shell_command(
            "npm install -g @anthropic-ai/claude-code"
        ));
        assert!(is_safe_shell_command("npm update -g @openai/codex"));
        assert!(is_safe_shell_command("pip3 install deepagents-cli==1.2.3"));
    }

    #[test]
    fn test_is_safe_shell_command_rejects_pipe_and_semicolon() {
        assert!(!is_safe_shell_command("curl -fsSL https://x | sh"));
        assert!(!is_safe_shell_command("echo foo; echo injected"));
    }

    #[test]
    fn test_producer_commands_pass_gate() {
        use agentry_core::models::InstallMethod;
        let methods = [
            InstallMethod::Npm {
                package: "@anthropic-ai/claude-code".to_string(),
            },
            InstallMethod::Npm {
                package: "@google/gemini-cli".to_string(),
            },
            InstallMethod::Npm {
                package: "@openai/codex".to_string(),
            },
            InstallMethod::Npm {
                package: "@sourcegraph/amp".to_string(),
            },
            InstallMethod::Npm {
                package: "@continuedev/cli".to_string(),
            },
            InstallMethod::Pip {
                package: "deepagents-cli".to_string(),
            },
        ];
        for method in &methods {
            assert!(is_safe_shell_command(&method.update_command()));
            assert!(is_safe_shell_command(&method.install_command(None)));
            assert!(is_safe_shell_command(
                &method.install_command(Some("1.2.3"))
            ));
        }
    }

    #[test]
    fn test_apply_fix_shell_command_success() {
        let finding = finding(
            "install_tool",
            Some(FixAction::ShellCommand {
                description: "no-op".to_string(),
                command: "true".to_string(),
            }),
            true,
        );
        let outcome = apply_fix(&finding, Path::new("/tmp"));
        assert!(outcome.success, "{}", outcome.message);
    }

    #[test]
    fn test_apply_fix_shell_command_refuses_metacharacters() {
        let command = "echo foo; echo injected";
        let finding = finding(
            "install_tool",
            Some(FixAction::ShellCommand {
                description: "sneaky".to_string(),
                command: command.to_string(),
            }),
            true,
        );
        let outcome = apply_fix(&finding, Path::new("/tmp"));
        assert!(!outcome.success);
        assert!(outcome.message.contains("refused unsafe shell command"));
    }

    #[test]
    fn test_apply_fix_file_write_creates_dirs_and_content() {
        let tmp = TempDir::new("agentry_test_fix_file_write");
        let path = tmp.path().join("nested/dir/target.md");
        let finding = finding(
            "config_write",
            Some(FixAction::FileWrite {
                path: path.clone(),
                content: "config body".to_string(),
            }),
            true,
        );
        let outcome = apply_fix(&finding, tmp.path());
        assert!(outcome.success, "{}", outcome.message);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "config body");
    }

    #[test]
    fn test_apply_fix_file_remove() {
        let tmp = TempDir::new("agentry_test_fix_file_remove");
        let path = tmp.path().join("stale.md");
        std::fs::write(&path, "x").unwrap();
        let finding = finding(
            "orphan_cleanup",
            Some(FixAction::FileRemove { path: path.clone() }),
            true,
        );
        let outcome = apply_fix(&finding, tmp.path());
        assert!(outcome.success, "{}", outcome.message);
        assert!(!path.exists());
    }

    #[test]
    fn test_apply_fix_symlink_recreate_resolves_target() {
        let tmp = TempDir::new("agentry_test_fix_symlink");
        let real = tmp.path().join("real.txt");
        std::fs::write(&real, "data").unwrap();
        let link = tmp.path().join("skills");
        std::os::unix::fs::symlink("missing-target", &link).unwrap();

        let finding = finding(
            "skills_link",
            Some(FixAction::SymlinkRecreate {
                path: link.clone(),
                target: "real.txt".to_string(),
            }),
            true,
        );
        let outcome = apply_fix(&finding, tmp.path());
        assert!(outcome.success, "{}", outcome.message);
        assert_eq!(std::fs::read_link(&link).unwrap(), Path::new("real.txt"));
        assert_eq!(std::fs::read_to_string(&link).unwrap(), "data");
    }

    #[test]
    fn test_apply_fix_symlink_recreate_missing_target_is_unsuccessful() {
        let tmp = TempDir::new("agentry_test_fix_symlink_missing");
        let link = tmp.path().join("skills");
        let finding = finding(
            "skills_link",
            Some(FixAction::SymlinkRecreate {
                path: link.clone(),
                target: "../../.agents/skills/python3".to_string(),
            }),
            true,
        );
        let outcome = apply_fix(&finding, tmp.path());
        assert!(!outcome.success);
        assert!(outcome
            .message
            .contains("target does not exist after recreation"));
        assert!(link.is_symlink());
        assert!(!link.exists());
    }

    #[test]
    fn test_apply_fix_refuses_absolute_symlink_target() {
        let tmp = TempDir::new("agentry_test_fix_symlink_abs");
        let link = tmp.path().join("skills");
        let finding = finding(
            "skills_link",
            Some(FixAction::SymlinkRecreate {
                path: link.clone(),
                target: "/etc/passwd".to_string(),
            }),
            true,
        );
        let outcome = apply_fix(&finding, tmp.path());
        assert!(!outcome.success);
        assert!(outcome.message.contains("refused absolute symlink target"));
        assert!(!link.exists());
    }

    #[test]
    fn test_apply_fix_refuses_symlink_outside_home() {
        let tmp = TempDir::new("agentry_test_fix_symlink_outside");
        let home = tmp.path().join("home");
        let link = tmp.path().join("elsewhere/skills");
        let finding = finding(
            "skills_link",
            Some(FixAction::SymlinkRecreate {
                path: link,
                target: "real.txt".to_string(),
            }),
            true,
        );
        let outcome = apply_fix(&finding, &home);
        assert!(!outcome.success);
        assert!(outcome.message.contains("refused symlink outside"));
    }

    #[test]
    fn test_apply_sync_prompt_with_agents_syncs_destination() {
        let tmp = TempDir::new("agentry_test_fix_sync_prompt");
        let canonical_dir = tmp.path().join(".agents").join("prompts");
        std::fs::create_dir_all(&canonical_dir).unwrap();
        std::fs::write(
            canonical_dir.join("GEMINI.md"),
            "# GEMINI\n\nCanonical prompt",
        )
        .unwrap();

        let agents = vec![DetectedAgent {
            spec: AgentSpec {
                id: "gemini-cli".to_string(),
                name: "Gemini CLI".to_string(),
                cli_binary: "gemini".to_string(),
                config_dir: ".gemini".to_string(),
                prompt_filename: "GEMINI.md".to_string(),
                prompt_format: PromptFormat::PlainMd,
                skills_dir_name: None,
                max_size: None,
                install_methods: vec![],
            },
            installed: true,
            version: None,
            config_dir_exists: true,
            prompt_file_exists: false,
            skills_dir: None,
            skills_symlink_pattern: None,
            installed_skills: vec![],
            detected_methods: vec![],
        }];

        let outcome = apply_sync_prompt_with_agents(
            "GEMINI",
            "gemini-cli",
            "prompt_sync",
            tmp.path(),
            &agents,
        );
        assert!(outcome.success, "{}", outcome.message);
        assert_eq!(outcome.check_id, "prompt_sync");
        let dest = tmp.path().join(".gemini").join("GEMINI.md");
        assert_eq!(
            std::fs::read_to_string(&dest).unwrap(),
            "# GEMINI\n\nCanonical prompt"
        );
        assert!(outcome.message.contains("synced GEMINI to"));
    }

    #[test]
    fn test_apply_sync_prompt_unknown_prompt_is_unsuccessful() {
        let tmp = TempDir::new("agentry_test_fix_sync_unknown");
        let agents: Vec<DetectedAgent> = Vec::new();
        let outcome =
            apply_sync_prompt_with_agents("nope", "gemini-cli", "prompt_sync", tmp.path(), &agents);
        assert!(!outcome.success);
        assert!(outcome.message.contains("not found"));
    }

    #[test]
    fn test_apply_sync_prompt_shared_destination_is_refused() {
        let tmp = TempDir::new("agentry_test_fix_sync_shared");
        let canonical_dir = tmp.path().join(".agents").join("prompts");
        std::fs::create_dir_all(&canonical_dir).unwrap();
        std::fs::write(canonical_dir.join("ALPHA.md"), "alpha body").unwrap();
        std::fs::write(canonical_dir.join("BETA.md"), "beta body").unwrap();

        let agents = vec![DetectedAgent {
            spec: AgentSpec {
                id: "claude-code".to_string(),
                name: "Claude Code".to_string(),
                cli_binary: "claude".to_string(),
                config_dir: ".claude".to_string(),
                prompt_filename: "CLAUDE.md".to_string(),
                prompt_format: PromptFormat::PlainMd,
                skills_dir_name: None,
                max_size: None,
                install_methods: vec![],
            },
            installed: true,
            version: None,
            config_dir_exists: true,
            prompt_file_exists: false,
            skills_dir: None,
            skills_symlink_pattern: None,
            installed_skills: vec![],
            detected_methods: vec![],
        }];

        let outcome = apply_sync_prompt_with_agents(
            "ALPHA",
            "claude-code",
            "sync.drift",
            tmp.path(),
            &agents,
        );
        assert!(!outcome.success);
        assert!(outcome.message.contains("shared by 2 prompts"));
        assert!(outcome.message.contains("agentry sync --all"));
        let dest = tmp.path().join(".claude").join("CLAUDE.md");
        assert!(!dest.exists());
    }

    #[test]
    fn test_apply_sync_prompt_single_destination_persists_and_clears_drift() {
        let tmp = TempDir::new("agentry_test_fix_sync_persist");
        let canonical_dir = tmp.path().join(".agents").join("prompts");
        std::fs::create_dir_all(&canonical_dir).unwrap();
        std::fs::write(
            canonical_dir.join("GEMINI.md"),
            "# GEMINI\n\nCanonical prompt",
        )
        .unwrap();

        let agents = vec![DetectedAgent {
            spec: AgentSpec {
                id: "gemini-cli".to_string(),
                name: "Gemini CLI".to_string(),
                cli_binary: "gemini".to_string(),
                config_dir: ".gemini".to_string(),
                prompt_filename: "GEMINI.md".to_string(),
                prompt_format: PromptFormat::PlainMd,
                skills_dir_name: None,
                max_size: None,
                install_methods: vec![],
            },
            installed: true,
            version: None,
            config_dir_exists: true,
            prompt_file_exists: false,
            skills_dir: None,
            skills_symlink_pattern: None,
            installed_skills: vec![],
            detected_methods: vec![],
        }];

        let outcome = apply_sync_prompt_with_agents(
            "GEMINI",
            "gemini-cli",
            "sync.drift",
            tmp.path(),
            &agents,
        );
        assert!(outcome.success, "{}", outcome.message);
        let dest = tmp.path().join(".gemini").join("GEMINI.md");
        assert_eq!(
            std::fs::read_to_string(&dest).unwrap(),
            "# GEMINI\n\nCanonical prompt"
        );

        let prompts = discover_prompts(tmp.path(), &[tmp.path().join(DEFAULT_PROJECT_DIR)]);
        let prompt = prompts.iter().find(|p| p.name == "GEMINI").unwrap();
        let plan = plan_sync(prompt, &agents, tmp.path());
        let statuses = check_sync_status(prompt, &plan.mappings);
        assert!(statuses.iter().all(|m| m.status == SyncStatus::UpToDate));
    }

    #[test]
    fn test_apply_sync_prompt_no_mapping_is_unsuccessful() {
        let tmp = TempDir::new("agentry_test_fix_sync_no_mapping");
        let canonical_dir = tmp.path().join(".agents").join("prompts");
        std::fs::create_dir_all(&canonical_dir).unwrap();
        std::fs::write(
            canonical_dir.join("GEMINI.md"),
            "# GEMINI\n\nCanonical prompt",
        )
        .unwrap();

        let agents = vec![DetectedAgent {
            spec: AgentSpec {
                id: "unknown-agent".to_string(),
                name: "Unknown Agent".to_string(),
                cli_binary: "unknown-agent".to_string(),
                config_dir: ".unknown-agent".to_string(),
                prompt_filename: "AGENTS.md".to_string(),
                prompt_format: PromptFormat::PlainMd,
                skills_dir_name: None,
                max_size: None,
                install_methods: vec![],
            },
            installed: true,
            version: None,
            config_dir_exists: true,
            prompt_file_exists: false,
            skills_dir: None,
            skills_symlink_pattern: None,
            installed_skills: vec![],
            detected_methods: vec![],
        }];

        let outcome = apply_sync_prompt_with_agents(
            "GEMINI",
            "unknown-agent",
            "prompt_sync",
            tmp.path(),
            &agents,
        );
        assert!(!outcome.success);
        assert!(outcome.message.contains("no sync mapping"));
    }

    #[test]
    fn test_apply_fix_without_fix_action_is_unsuccessful() {
        let finding = finding("no_fix", None, false);
        let outcome = apply_fix(&finding, Path::new("/tmp"));
        assert!(!outcome.success);
    }

    #[test]
    fn test_fixable_findings_filters_mixed_report() {
        let auto = finding(
            "fixable",
            Some(FixAction::FileRemove {
                path: PathBuf::from("/tmp/x"),
            }),
            true,
        );
        let no_fix = finding("no_fix", None, true);
        let not_auto = finding(
            "not_auto",
            Some(FixAction::FileRemove {
                path: PathBuf::from("/tmp/y"),
            }),
            false,
        );

        let rep = report(vec![auto, no_fix, not_auto]);
        let fixable = fixable_findings(&rep);
        assert_eq!(fixable.len(), 1);
        assert_eq!(fixable[0].check_id, "fixable");
    }

    #[test]
    fn test_fixable_findings_orders_by_severity() {
        let mut suggestion = finding(
            "suggestion",
            Some(FixAction::FileRemove {
                path: PathBuf::from("/tmp/s"),
            }),
            true,
        );
        suggestion.severity = Severity::Suggestion;
        let mut critical = finding(
            "critical",
            Some(FixAction::FileRemove {
                path: PathBuf::from("/tmp/c1"),
            }),
            true,
        );
        critical.severity = Severity::Critical;
        let mut info = finding(
            "info",
            Some(FixAction::FileRemove {
                path: PathBuf::from("/tmp/i"),
            }),
            true,
        );
        info.severity = Severity::Info;
        let mut warning = finding(
            "warning",
            Some(FixAction::FileRemove {
                path: PathBuf::from("/tmp/w"),
            }),
            true,
        );
        warning.severity = Severity::Warning;
        let mut critical2 = finding(
            "critical2",
            Some(FixAction::FileRemove {
                path: PathBuf::from("/tmp/c2"),
            }),
            true,
        );
        critical2.severity = Severity::Critical;

        let rep = report(vec![suggestion, critical, info, warning, critical2]);
        let fixable = fixable_findings(&rep);
        let ids: Vec<&str> = fixable.iter().map(|f| f.check_id.as_str()).collect();
        assert_eq!(
            ids,
            ["critical", "critical2", "warning", "info", "suggestion"]
        );
    }

    #[test]
    fn test_apply_fixes_yes_applies_all() {
        let tmp = TempDir::new("agentry_test_fix_apply_yes");
        let first = tmp.path().join("first.md");
        let second = tmp.path().join("second.md");
        let findings = vec![
            finding(
                "write_first",
                Some(FixAction::FileWrite {
                    path: first.clone(),
                    content: "1".to_string(),
                }),
                true,
            ),
            finding(
                "write_second",
                Some(FixAction::FileWrite {
                    path: second.clone(),
                    content: "2".to_string(),
                }),
                true,
            ),
        ];
        let refs: Vec<&AuditFinding> = findings.iter().collect();
        let outcomes = apply_fixes(&refs, tmp.path(), true);
        assert_eq!(outcomes.len(), 2);
        assert!(outcomes.iter().all(|o| o.success));
        assert_eq!(std::fs::read_to_string(&first).unwrap(), "1");
        assert_eq!(std::fs::read_to_string(&second).unwrap(), "2");
    }

    #[test]
    fn test_apply_fixes_confirm_no_skips_all() {
        let tmp = TempDir::new("agentry_test_fix_apply_no");
        let first = tmp.path().join("first.md");
        let second = tmp.path().join("second.md");
        let findings = vec![
            finding(
                "write_first",
                Some(FixAction::FileWrite {
                    path: first.clone(),
                    content: "1".to_string(),
                }),
                true,
            ),
            finding(
                "write_second",
                Some(FixAction::FileWrite {
                    path: second.clone(),
                    content: "2".to_string(),
                }),
                true,
            ),
        ];
        let refs: Vec<&AuditFinding> = findings.iter().collect();
        let outcomes = apply_fixes_with_input(&refs, tmp.path(), false, &mut || "n".to_string());
        assert_eq!(outcomes.len(), 2);
        assert!(outcomes.iter().all(|o| !o.success));
        assert!(outcomes.iter().all(|o| o.message == "skipped by user"));
        assert!(!first.exists());
        assert!(!second.exists());
    }

    #[test]
    fn test_apply_fixes_confirm_yes_applies() {
        let tmp = TempDir::new("agentry_test_fix_apply_conf_yes");
        let path = tmp.path().join("confirmed.md");
        let findings = [finding(
            "write_confirmed",
            Some(FixAction::FileWrite {
                path: path.clone(),
                content: "ok".to_string(),
            }),
            true,
        )];
        let refs: Vec<&AuditFinding> = findings.iter().collect();
        let outcomes = apply_fixes_with_input(&refs, tmp.path(), false, &mut || "y\n".to_string());
        assert_eq!(outcomes.len(), 1);
        assert!(outcomes[0].success, "{}", outcomes[0].message);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "ok");
    }

    #[test]
    fn test_confirm_reads_answers() {
        assert!(confirm(&mut || "y".to_string()));
        assert!(confirm(&mut || "yes".to_string()));
        assert!(!confirm(&mut || "n".to_string()));
        assert!(!confirm(&mut || "N".to_string()));
        assert!(!confirm(&mut || String::new()));
    }

    #[test]
    fn test_validate_refuses_unsafe_shell_command() {
        let fix = FixAction::ShellCommand {
            description: "sneaky".to_string(),
            command: "echo foo; echo injected".to_string(),
        };
        let err = validate(&fix, Path::new("/tmp")).unwrap_err();
        assert!(err.contains("refused unsafe shell command"));
    }

    #[test]
    fn test_validate_accepts_safe_shell_command() {
        let fix = FixAction::ShellCommand {
            description: "noop".to_string(),
            command: "true".to_string(),
        };
        assert!(validate(&fix, Path::new("/tmp")).is_ok());
    }

    #[test]
    fn test_validate_refuses_symlink_outside_home() {
        let fix = FixAction::SymlinkRecreate {
            path: PathBuf::from("/elsewhere/skills"),
            target: "real.txt".to_string(),
        };
        let err = validate(&fix, Path::new("/home/user")).unwrap_err();
        assert!(err.contains("refused symlink outside"));
    }

    #[test]
    fn test_validate_refuses_absolute_symlink_target() {
        let fix = FixAction::SymlinkRecreate {
            path: PathBuf::from("/home/user/skills"),
            target: "/etc/passwd".to_string(),
        };
        let err = validate(&fix, Path::new("/home/user")).unwrap_err();
        assert!(err.contains("refused absolute symlink target"));
    }

    #[test]
    fn test_validate_refuses_symlink_target_with_empty_components() {
        let fix = FixAction::SymlinkRecreate {
            path: PathBuf::from("/home/user/skills"),
            target: "a//b".to_string(),
        };
        let err = validate(&fix, Path::new("/home/user")).unwrap_err();
        assert!(err.contains("refused symlink target with empty components"));
    }

    #[test]
    fn test_validate_accepts_in_bounds_symlink() {
        let fix = FixAction::SymlinkRecreate {
            path: PathBuf::from("/home/user/.agents/skills/x"),
            target: "../../.agents/skills/x/SKILL.md".to_string(),
        };
        assert!(validate(&fix, Path::new("/home/user")).is_ok());
    }

    #[test]
    fn test_validate_accepts_file_write_and_remove_pending_path_bounds() {
        let write = FixAction::FileWrite {
            path: PathBuf::from("/anywhere/file.md"),
            content: "body".to_string(),
        };
        let remove = FixAction::FileRemove {
            path: PathBuf::from("/anywhere/file.md"),
        };
        assert!(validate(&write, Path::new("/home/user")).is_ok());
        assert!(validate(&remove, Path::new("/home/user")).is_ok());
    }

    #[test]
    fn test_validate_accepts_sync_prompt() {
        let fix = FixAction::SyncPrompt {
            prompt_id: "GEMINI".to_string(),
            agent_id: "gemini-cli".to_string(),
        };
        assert!(validate(&fix, Path::new("/home/user")).is_ok());
    }
}
