use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

/// Top-level structure of `~/.agents/.skill-lock.json` (version 3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillLockfile {
    pub version: u32,
    #[serde(default)]
    pub skills: BTreeMap<String, SkillLockEntry>,
    #[serde(default)]
    pub dismissed: BTreeMap<String, serde_json::Value>,
    #[serde(default, rename = "lastSelectedAgents")]
    pub last_selected_agents: Vec<String>,
}

/// A single skill entry in the lockfile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillLockEntry {
    pub source: String,
    #[serde(rename = "sourceType")]
    pub source_type: String,
    #[serde(rename = "sourceUrl")]
    pub source_url: String,
    #[serde(rename = "skillPath")]
    pub skill_path: String,
    #[serde(rename = "skillFolderHash")]
    pub skill_folder_hash: String,
    #[serde(rename = "installedAt")]
    pub installed_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

/// Path to the lockfile.
pub fn lockfile_path(home_dir: &Path) -> std::path::PathBuf {
    home_dir.join(".agents").join(".skill-lock.json")
}

/// Read and parse the lockfile. Returns a default empty structure if the file doesn't exist.
pub fn read_lockfile(home_dir: &Path) -> Result<SkillLockfile> {
    let path = lockfile_path(home_dir);
    if !path.exists() {
        return Ok(SkillLockfile {
            version: 3,
            skills: BTreeMap::new(),
            dismissed: BTreeMap::new(),
            last_selected_agents: Vec::new(),
        });
    }

    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read lockfile at {}", path.display()))?;
    let lockfile: SkillLockfile = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse lockfile at {}", path.display()))?;
    Ok(lockfile)
}

/// Write the lockfile back to disk, preserving the v3 schema format.
pub fn write_lockfile(home_dir: &Path, lockfile: &SkillLockfile) -> Result<()> {
    let path = lockfile_path(home_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(lockfile)?;
    std::fs::write(&path, content)?;
    Ok(())
}

/// Compute the SHA-1 hash of a skill folder (all files, recursively, sorted).
pub fn compute_skill_hash(skill_dir: &Path) -> Result<String> {
    let mut hasher = Sha1::new();
    let mut files: Vec<_> = walkdir(skill_dir)?;
    files.sort();

    for file_path in &files {
        let content = std::fs::read(file_path)
            .with_context(|| format!("Failed to read {}", file_path.display()))?;
        hasher.update(&content);
    }

    let result = hasher.finalize();
    Ok(format!("{:x}", result))
}

/// Recursively collect all file paths under a directory.
fn walkdir(dir: &Path) -> Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    if !dir.is_dir() {
        return Ok(files);
    }

    let entries = std::fs::read_dir(dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            files.extend(walkdir(&path)?);
        } else {
            files.push(path);
        }
    }
    Ok(files)
}

/// Add or update a skill entry in the lockfile.
pub fn upsert_skill(lockfile: &mut SkillLockfile, name: &str, entry: SkillLockEntry) {
    lockfile.skills.insert(name.to_string(), entry);
}

/// Remove a skill entry from the lockfile.
pub fn remove_skill(lockfile: &mut SkillLockfile, name: &str) -> bool {
    lockfile.skills.remove(name).is_some()
}

/// Check if a skill is installed (exists in the lockfile).
pub fn is_skill_installed(lockfile: &SkillLockfile, name: &str) -> bool {
    lockfile.skills.contains_key(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_lockfile_missing() {
        let tmp = std::env::temp_dir().join("agentry_test_no_lockfile");
        let lockfile = read_lockfile(&tmp).unwrap();
        assert_eq!(lockfile.version, 3);
        assert!(lockfile.skills.is_empty());
    }

    #[test]
    fn test_write_and_read_lockfile() {
        let tmp = std::env::temp_dir().join("agentry_test_lockfile_rw");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let mut lockfile = SkillLockfile {
            version: 3,
            skills: BTreeMap::new(),
            dismissed: BTreeMap::new(),
            last_selected_agents: vec!["claude-code".to_string()],
        };

        let entry = SkillLockEntry {
            source: "vercel-labs/agent-skills".to_string(),
            source_type: "github".to_string(),
            source_url: "https://github.com/vercel-labs/agent-skills.git".to_string(),
            skill_path: "skills/deploy-to-vercel/SKILL.md".to_string(),
            skill_folder_hash: "abc123".to_string(),
            installed_at: "2026-03-28T19:39:31.754Z".to_string(),
            updated_at: "2026-03-28T19:47:46.434Z".to_string(),
        };
        lockfile
            .skills
            .insert("deploy-to-vercel".to_string(), entry);

        write_lockfile(&tmp, &lockfile).unwrap();
        let read_back = read_lockfile(&tmp).unwrap();

        assert_eq!(read_back.version, 3);
        assert_eq!(read_back.skills.len(), 1);
        assert!(read_back.skills.contains_key("deploy-to-vercel"));
        assert_eq!(
            read_back.skills["deploy-to-vercel"].source,
            "vercel-labs/agent-skills"
        );
        assert_eq!(read_back.last_selected_agents, vec!["claude-code"]);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_compute_skill_hash() {
        let tmp = std::env::temp_dir().join("agentry_test_skill_hash");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("SKILL.md"), "# Test Skill\nHello world").unwrap();

        let hash1 = compute_skill_hash(&tmp).unwrap();
        assert!(!hash1.is_empty());

        // Same content → same hash
        let hash2 = compute_skill_hash(&tmp).unwrap();
        assert_eq!(hash1, hash2);

        // Different content → different hash
        std::fs::write(tmp.join("SKILL.md"), "# Modified Skill\nDifferent content").unwrap();
        let hash3 = compute_skill_hash(&tmp).unwrap();
        assert_ne!(hash1, hash3);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_upsert_and_remove_skill() {
        let mut lockfile = SkillLockfile {
            version: 3,
            skills: BTreeMap::new(),
            dismissed: BTreeMap::new(),
            last_selected_agents: Vec::new(),
        };

        let entry = SkillLockEntry {
            source: "test/repo".to_string(),
            source_type: "github".to_string(),
            source_url: "https://github.com/test/repo.git".to_string(),
            skill_path: "skills/test/SKILL.md".to_string(),
            skill_folder_hash: "hash123".to_string(),
            installed_at: "2026-01-01T00:00:00.000Z".to_string(),
            updated_at: "2026-01-01T00:00:00.000Z".to_string(),
        };

        assert!(!is_skill_installed(&lockfile, "test"));
        upsert_skill(&mut lockfile, "test", entry);
        assert!(is_skill_installed(&lockfile, "test"));
        assert!(remove_skill(&mut lockfile, "test"));
        assert!(!is_skill_installed(&lockfile, "test"));
    }
}
