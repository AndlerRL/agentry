use std::path::{Path, PathBuf};

use agentry_agents::detector::verify_symlink_target;
use agentry_core::models::DetectedAgent;
use agentry_skills::hub::{AvailableSkill, SkillHub};
use agentry_skills::lockfile::{compute_skill_hash, read_lockfile, SkillLockEntry};

use crate::engine::CheckContext;
use crate::report::{AuditFinding, FindingCategory, FixAction, Severity};

pub fn run(ctx: &CheckContext) -> Vec<AuditFinding> {
    let mut findings = Vec::new();
    for agent in &ctx.agents {
        findings.extend(symlink_broken(ctx, agent));
    }
    findings.extend(orphaned(ctx));
    findings.extend(hash_mismatch(ctx));
    findings
}

fn symlink_broken(ctx: &CheckContext, agent: &DetectedAgent) -> Vec<AuditFinding> {
    let Some(dir) = skills_dir(ctx, agent) else {
        return Vec::new();
    };
    let mut links = Vec::new();
    collect_symlinks(&dir, &mut links);
    links
        .iter()
        .filter(|link| !verify_symlink_target(link))
        .map(|link| broken_link_finding(agent, &dir, link))
        .collect()
}

fn skills_dir(ctx: &CheckContext, agent: &DetectedAgent) -> Option<PathBuf> {
    let dir = agent.skills_dir.clone().or_else(|| {
        agent
            .spec
            .skills_dir_name
            .as_ref()
            .map(|name| ctx.home_dir.join(&agent.spec.config_dir).join(name))
    })?;
    dir.is_dir().then_some(dir)
}

fn collect_symlinks(dir: &Path, links: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_symlink() {
            links.push(path);
        } else if path.is_dir() {
            collect_symlinks(&path, links);
        }
    }
}

fn broken_link_finding(agent: &DetectedAgent, skills_root: &Path, link: &Path) -> AuditFinding {
    let name = link
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    let depth = link
        .strip_prefix(skills_root)
        .map(|relative| relative.components().count().saturating_sub(1))
        .unwrap_or(0);
    let prefix = "../".repeat(depth + 2);
    let target = format!("{prefix}.agents/skills/{name}");
    AuditFinding {
        check_id: "skills.symlink_broken".to_string(),
        severity: Severity::Warning,
        category: FindingCategory::Skills,
        agent_id: Some(agent.spec.id.clone()),
        message: format!(
            "{} skills symlink '{}' does not resolve",
            agent.spec.name,
            link.display()
        ),
        remediation: format!(
            "Recreate the symlink '{}' to point to '{}'",
            link.display(),
            target
        ),
        auto_fixable: true,
        fix: Some(FixAction::SymlinkRecreate {
            path: link.to_path_buf(),
            target,
        }),
        evidence: Some(format!("symlink={} resolves=false", link.display())),
    }
}

fn orphaned(ctx: &CheckContext) -> Vec<AuditFinding> {
    let hub = match SkillHub::load(&ctx.home_dir, &[]) {
        Ok(hub) => hub,
        Err(_) => return Vec::new(),
    };
    hub.skills
        .values()
        .filter(|skill| skill.installed && skill.installed_hash.is_none())
        .filter_map(orphan_finding)
        .collect()
}

fn orphan_finding(skill: &AvailableSkill) -> Option<AuditFinding> {
    let path = skill.install_path.as_ref()?;
    Some(AuditFinding {
        check_id: "skills.orphaned".to_string(),
        severity: Severity::Info,
        category: FindingCategory::Skills,
        agent_id: None,
        message: format!(
            "Skill '{}' exists on disk at '{}' but is missing from the skill lockfile",
            skill.name,
            path.display()
        ),
        remediation: format!(
            "Run 'agentry skills remove {}' or reinstall the skill to record it in the lockfile",
            skill.name
        ),
        auto_fixable: false,
        fix: None,
        evidence: Some(format!(
            "install_path={} lockfile_entry=false note=only_directories_containing_SKILL.md_are_detected",
            path.display()
        )),
    })
}

fn hash_mismatch(ctx: &CheckContext) -> Vec<AuditFinding> {
    let lockfile = match read_lockfile(&ctx.home_dir) {
        Ok(lockfile) => lockfile,
        Err(_) => return Vec::new(),
    };
    let skills_root = ctx.home_dir.join(".agents").join("skills");
    lockfile
        .skills
        .iter()
        .filter_map(|(name, entry)| hash_finding(name, entry, &skills_root))
        .collect()
}

fn hash_finding(name: &str, entry: &SkillLockEntry, skills_root: &Path) -> Option<AuditFinding> {
    let dir = skills_root.join(name);
    let Ok(actual) = compute_skill_hash(&dir) else {
        return None;
    };
    if actual == entry.skill_folder_hash {
        return None;
    }
    let missing_note = if dir.is_dir() {
        String::new()
    } else {
        " note=skill_dir_missing_on_disk_empty_hash_used".to_string()
    };
    Some(AuditFinding {
        check_id: "skills.hash_mismatch".to_string(),
        severity: Severity::Warning,
        category: FindingCategory::Skills,
        agent_id: None,
        message: format!("Skill '{}' folder hash does not match the lockfile", name),
        remediation: format!("Run 'agentry skills update {}'", name),
        auto_fixable: true,
        fix: Some(FixAction::ShellCommand {
            description: format!("Update skill '{}' to restore the locked hash", name),
            command: format!("agentry skills update {}", name),
        }),
        evidence: Some(format!(
            "skill={} expected={} actual={} path={}{}",
            name,
            entry.skill_folder_hash,
            actual,
            dir.display(),
            missing_note
        )),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentry_core::models::{AgentSpec, PromptFormat};
    use agentry_skills::lockfile::{write_lockfile, SkillLockfile};
    use std::collections::BTreeMap;

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

    fn skills_agent(id: &str, config_dir: &str) -> DetectedAgent {
        DetectedAgent {
            spec: AgentSpec {
                id: id.to_string(),
                name: id.to_string(),
                cli_binary: id.to_string(),
                config_dir: config_dir.to_string(),
                prompt_filename: "CLAUDE.md".to_string(),
                prompt_format: PromptFormat::PlainMd,
                skills_dir_name: Some("skills".to_string()),
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

    fn plain_agent(id: &str, config_dir: &str) -> DetectedAgent {
        let mut agent = skills_agent(id, config_dir);
        agent.spec.skills_dir_name = None;
        agent
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

    fn lock_entry(hash: &str) -> SkillLockEntry {
        SkillLockEntry {
            source: "test/repo".to_string(),
            source_type: "github".to_string(),
            source_url: "https://github.com/test/repo.git".to_string(),
            skill_path: "skills/test/SKILL.md".to_string(),
            skill_folder_hash: hash.to_string(),
            installed_at: "2026-01-01T00:00:00.000Z".to_string(),
            updated_at: "2026-01-01T00:00:00.000Z".to_string(),
        }
    }

    fn write_lock_with(home: &Path, name: &str, hash: &str) {
        let mut lockfile = SkillLockfile {
            version: 3,
            skills: BTreeMap::new(),
            dismissed: BTreeMap::new(),
            last_selected_agents: Vec::new(),
        };
        lockfile.skills.insert(name.to_string(), lock_entry(hash));
        write_lockfile(home, &lockfile).unwrap();
    }

    #[test]
    fn symlink_broken_fires_when_target_does_not_resolve() {
        let tmp = TempDir::new("agentry_audit_skills_broken_fires");
        let skills = tmp.path().join(".claude").join("skills");
        std::fs::create_dir_all(&skills).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(
            tmp.path().join(".agents").join("skills").join("git"),
            skills.join("git"),
        )
        .unwrap();
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![skills_agent("claude-code", ".claude")],
        ));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].check_id, "skills.symlink_broken");
        assert_eq!(findings[0].severity, Severity::Warning);
        assert_eq!(findings[0].category, FindingCategory::Skills);
        assert_eq!(findings[0].agent_id.as_deref(), Some("claude-code"));
        assert!(findings[0].auto_fixable);
        assert!(!findings[0].message.is_empty());
        assert!(!findings[0].remediation.is_empty());
        match &findings[0].fix {
            Some(FixAction::SymlinkRecreate { path, target }) => {
                assert_eq!(path, &skills.join("git"));
                assert_eq!(target, "../../.agents/skills/git");
            }
            other => panic!("expected SymlinkRecreate fix, got {:?}", other),
        }
    }

    #[test]
    fn symlink_broken_skips_when_target_resolves() {
        let tmp = TempDir::new("agentry_audit_skills_broken_valid");
        let skills = tmp.path().join(".claude").join("skills");
        let target = tmp.path().join(".agents").join("skills").join("git");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::create_dir_all(&skills).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, skills.join("git")).unwrap();
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![skills_agent("claude-code", ".claude")],
        ));
        assert!(findings.is_empty());
    }

    #[test]
    fn symlink_broken_skips_regular_files_and_dirs() {
        let tmp = TempDir::new("agentry_audit_skills_broken_regular");
        let skills = tmp.path().join(".claude").join("skills");
        std::fs::create_dir_all(skills.join("real-skill")).unwrap();
        std::fs::write(skills.join("README.md"), "notes").unwrap();
        std::fs::write(skills.join("real-skill").join("SKILL.md"), "x").unwrap();
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![skills_agent("claude-code", ".claude")],
        ));
        assert!(findings.is_empty());
    }

    #[test]
    fn symlink_broken_scans_nested_directories() {
        let tmp = TempDir::new("agentry_audit_skills_broken_nested");
        let skills = tmp.path().join(".claude").join("skills");
        let nested = skills.join("group");
        std::fs::create_dir_all(&nested).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(
            tmp.path().join(".agents").join("skills").join("missing"),
            nested.join("inner"),
        )
        .unwrap();
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![skills_agent("claude-code", ".claude")],
        ));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].check_id, "skills.symlink_broken");
        match &findings[0].fix {
            Some(FixAction::SymlinkRecreate { target, .. }) => {
                assert_eq!(target, "../../../.agents/skills/inner");
            }
            other => panic!("expected SymlinkRecreate fix, got {:?}", other),
        }
    }

    #[test]
    fn symlink_broken_skips_agents_without_skills_dir() {
        let tmp = TempDir::new("agentry_audit_skills_broken_noskills");
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![plain_agent("codex", ".codex")],
        ));
        assert!(findings.is_empty());
    }

    #[test]
    fn symlink_broken_terminates_on_self_referential_link() {
        let tmp = TempDir::new("agentry_audit_skills_broken_loop");
        let skills = tmp.path().join(".claude").join("skills");
        std::fs::create_dir_all(&skills).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(tmp.path().join(".claude"), skills.join("loop")).unwrap();
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![skills_agent("claude-code", ".claude")],
        ));
        assert!(findings.is_empty());
    }

    #[test]
    fn orphaned_fires_for_untracked_skill_directory() {
        let tmp = TempDir::new("agentry_audit_skills_orphan_fires");
        let stray = tmp.path().join(".agents").join("skills").join("stray");
        std::fs::create_dir_all(&stray).unwrap();
        std::fs::write(stray.join("SKILL.md"), "# Stray Skill").unwrap();
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![skills_agent("claude-code", ".claude")],
        ));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].check_id, "skills.orphaned");
        assert_eq!(findings[0].severity, Severity::Info);
        assert_eq!(findings[0].category, FindingCategory::Skills);
        assert_eq!(findings[0].agent_id, None);
        assert!(!findings[0].auto_fixable);
        assert!(findings[0].fix.is_none());
        assert!(!findings[0].message.is_empty());
        assert!(!findings[0].remediation.is_empty());
        let evidence = findings[0].evidence.as_deref().unwrap_or_default();
        assert!(evidence.contains("lockfile_entry=false"));
        assert!(evidence.contains("SKILL.md"));
    }

    #[test]
    fn orphaned_skips_skill_tracked_in_lockfile() {
        let tmp = TempDir::new("agentry_audit_skills_orphan_tracked");
        let locked = tmp.path().join(".agents").join("skills").join("locked");
        std::fs::create_dir_all(&locked).unwrap();
        std::fs::write(locked.join("SKILL.md"), "# Locked Skill").unwrap();
        let real = compute_skill_hash(&locked).unwrap();
        write_lock_with(tmp.path(), "locked", &real);
        let findings = run(&ctx(tmp.path().clone(), Vec::new()));
        assert!(findings.is_empty());
    }

    #[test]
    fn orphaned_skips_directory_without_skill_md() {
        let tmp = TempDir::new("agentry_audit_skills_orphan_nodoc");
        let no_doc = tmp.path().join(".agents").join("skills").join("nodoc");
        std::fs::create_dir_all(&no_doc).unwrap();
        std::fs::write(no_doc.join("README.md"), "no skill markdown").unwrap();
        let findings = run(&ctx(tmp.path().clone(), Vec::new()));
        assert!(findings.is_empty());
    }

    #[test]
    fn hash_mismatch_fires_when_folder_differs_from_lockfile() {
        let tmp = TempDir::new("agentry_audit_skills_hash_fires");
        let skill = tmp.path().join(".agents").join("skills").join("my-skill");
        std::fs::create_dir_all(&skill).unwrap();
        std::fs::write(skill.join("SKILL.md"), "# My Skill").unwrap();
        write_lock_with(tmp.path(), "my-skill", "abc123");
        let findings = run(&ctx(tmp.path().clone(), Vec::new()));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].check_id, "skills.hash_mismatch");
        assert_eq!(findings[0].severity, Severity::Warning);
        assert_eq!(findings[0].category, FindingCategory::Skills);
        assert_eq!(findings[0].agent_id, None);
        assert!(findings[0].auto_fixable);
        let evidence = findings[0].evidence.as_deref().unwrap_or_default();
        assert!(evidence.contains("expected=abc123"));
        assert!(evidence.contains("skill=my-skill"));
        match &findings[0].fix {
            Some(FixAction::ShellCommand { command, .. }) => {
                assert_eq!(command, "agentry skills update my-skill");
            }
            other => panic!("expected ShellCommand fix, got {:?}", other),
        }
    }

    #[test]
    fn hash_mismatch_skips_when_hash_matches() {
        let tmp = TempDir::new("agentry_audit_skills_hash_match");
        let skill = tmp.path().join(".agents").join("skills").join("my-skill");
        std::fs::create_dir_all(&skill).unwrap();
        std::fs::write(skill.join("SKILL.md"), "# My Skill").unwrap();
        let real = compute_skill_hash(&skill).unwrap();
        write_lock_with(tmp.path(), "my-skill", &real);
        let findings = run(&ctx(tmp.path().clone(), Vec::new()));
        assert!(findings.is_empty());
    }

    #[test]
    fn hash_mismatch_fires_for_missing_skill_dir() {
        let tmp = TempDir::new("agentry_audit_skills_hash_missing");
        write_lock_with(tmp.path(), "ghost", "abc123");
        let findings = run(&ctx(tmp.path().clone(), Vec::new()));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].check_id, "skills.hash_mismatch");
        let evidence = findings[0].evidence.as_deref().unwrap_or_default();
        assert!(evidence.contains("skill_dir_missing_on_disk_empty_hash_used"));
    }
}
