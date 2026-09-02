use std::path::Path;

use agentry_harness::context::{write_config as write_harness_config, AuditorSection};

pub type AuditorConfig = AuditorSection;

pub fn load_config(home_dir: &Path) -> AuditorConfig {
    agentry_harness::context::load_config(home_dir).auditor
}

pub fn write_config(home_dir: &Path, config: &AuditorConfig) -> Result<(), String> {
    let mut merged = agentry_harness::context::load_config(home_dir);
    merged.auditor = config.clone();
    write_harness_config(home_dir, &merged)
}

pub fn canonical_prompt_path(home_dir: &Path) -> std::path::PathBuf {
    home_dir
        .join(".agents")
        .join("prompts")
        .join("agentry-auditor.md")
}

pub fn write_canonical_prompt_if_absent(home_dir: &Path) -> Result<bool, String> {
    let path = canonical_prompt_path(home_dir);
    if path.exists() {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    std::fs::write(&path, crate::PROMPT_ASSET)
        .map_err(|err| format!("failed to write {}: {err}", path.display()))?;
    Ok(true)
}

pub fn adopt_orphaned_collection(home_dir: &Path) -> Result<bool, String> {
    use agentry_skills::lockfile::{
        compute_skill_hash, read_lockfile, upsert_skill, write_lockfile, SkillLockEntry,
    };
    let skills_root = home_dir.join(".agents").join("skills");
    let collection_dir = skills_root.join("context-engineering-collection");
    if !collection_dir.is_dir() {
        return Ok(false);
    }
    let mut lockfile =
        read_lockfile(home_dir).map_err(|err| format!("failed to read lockfile: {err}"))?;
    if lockfile
        .skills
        .contains_key("context-engineering-collection")
    {
        return Ok(false);
    }
    let hash = compute_skill_hash(&collection_dir)
        .map_err(|err| format!("failed to hash collection: {err}"))?;
    let now = chrono::Utc::now().to_rfc3339();
    upsert_skill(
        &mut lockfile,
        "context-engineering-collection",
        SkillLockEntry {
            source: "local".to_string(),
            source_type: "local".to_string(),
            source_url: String::new(),
            skill_path: "skills/context-engineering-collection/SKILL.md".to_string(),
            skill_folder_hash: hash,
            installed_at: now.clone(),
            updated_at: now,
        },
    );
    write_lockfile(home_dir, &lockfile)
        .map_err(|err| format!("failed to write lockfile: {err}"))?;
    Ok(true)
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
    fn defaults_apply_when_file_missing() {
        let home = temp_home("agentry_test_audcfg_missing");
        let config = load_config(&home);
        assert_eq!(config.timeout_secs, 120);
        assert_eq!(config.max_findings, 20);
        assert!(config.host_cli.is_none());
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn load_parses_auditor_section() {
        let home = temp_home("agentry_test_audcfg_parse");
        let path = agentry_harness::context::config_path(&home);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            "[auditor]\nhost_cli = \"codex\"\ntimeout_secs = 60\nmax_findings = 5\n",
        )
        .unwrap();
        let config = load_config(&home);
        assert_eq!(config.host_cli.as_deref(), Some("codex"));
        assert_eq!(config.timeout_secs, 60);
        assert_eq!(config.max_findings, 5);
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn write_config_preserves_other_sections() {
        let home = temp_home("agentry_test_audcfg_write");
        let path = agentry_harness::context::config_path(&home);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "[harness]\nenabled_agents = [\"codex\"]\n").unwrap();
        let config = AuditorConfig {
            host_cli: Some("ollama".to_string()),
            ..Default::default()
        };
        write_config(&home, &config).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("enabled_agents"));
        assert!(content.contains("host_cli"));
        let loaded = load_config(&home);
        assert_eq!(loaded.host_cli.as_deref(), Some("ollama"));
        let harness = agentry_harness::context::load_config(&home);
        assert_eq!(harness.harness.enabled_agents, vec!["codex"]);
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn canonical_prompt_written_only_if_absent() {
        let home = temp_home("agentry_test_audcfg_prompt");
        let written = write_canonical_prompt_if_absent(&home).unwrap();
        assert!(written);
        let path = canonical_prompt_path(&home);
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("agentry-role: auditor"));
        let again = write_canonical_prompt_if_absent(&home).unwrap();
        assert!(!again);
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn adopt_orphaned_collection_adds_lockfile_entry() {
        let home = temp_home("agentry_test_audcfg_adopt");
        let collection = home
            .join(".agents")
            .join("skills")
            .join("context-engineering-collection");
        std::fs::create_dir_all(&collection).unwrap();
        std::fs::write(collection.join("SKILL.md"), "# Collection\n").unwrap();
        let adopted = adopt_orphaned_collection(&home).unwrap();
        assert!(adopted);
        let lockfile = agentry_skills::lockfile::read_lockfile(&home).unwrap();
        assert!(lockfile
            .skills
            .contains_key("context-engineering-collection"));
        let again = adopt_orphaned_collection(&home).unwrap();
        assert!(!again);
        std::fs::remove_dir_all(&home).unwrap();
    }
}
