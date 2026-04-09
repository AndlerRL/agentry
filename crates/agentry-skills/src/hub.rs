use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::lockfile;

/// Known skill source repositories (from the existing lockfile's sources).
pub const KNOWN_SKILL_SOURCES: &[&str] = &[
    "vercel-labs/agent-skills",
    "vercel-labs/skills",
    "vercel-labs/agentic-commerce-skills",
    "vercel-labs/agent-browser",
    "vercel/ai",
    "vercel/ai-elements",
    "vercel/components.build",
    "anthropics/skills",
    "CloudAI-X/threejs-skills",
    "SHADOWPR0/beautiful_prose",
    "blader/humanizer",
    "julianromli/ai-skills",
    "AgriciDaniel/claude-seo",
];

/// A source repository that provides skills.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSource {
    /// Short name (e.g. "vercel-labs/agent-skills")
    pub name: String,
    /// Full git URL
    pub url: String,
    /// Whether this is a built-in or custom source
    pub is_custom: bool,
}

impl SkillSource {
    pub fn from_short_name(name: &str) -> Self {
        Self {
            name: name.to_string(),
            url: format!("https://github.com/{}.git", name),
            is_custom: false,
        }
    }
}

/// A skill available from a source (may or may not be installed).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableSkill {
    pub name: String,
    pub source: String,
    pub source_url: String,
    /// Relative path within the repo (e.g. "skills/deploy-to-vercel/SKILL.md")
    pub skill_path: String,
    /// Description extracted from SKILL.md
    pub description: String,
    /// Whether this skill is currently installed
    pub installed: bool,
    /// Hash of the installed skill folder (if installed)
    pub installed_hash: Option<String>,
    /// Install path (if installed)
    pub install_path: Option<PathBuf>,
}

/// The skill hub: registry of available and installed skills.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillHub {
    /// All known sources (built-in + custom)
    pub sources: Vec<SkillSource>,
    /// All skills (both available and installed), keyed by name
    pub skills: BTreeMap<String, AvailableSkill>,
}

impl SkillHub {
    /// Build a SkillHub by scanning the lockfile and installed skills.
    pub fn load(home_dir: &Path, extra_sources: &[String]) -> Result<Self> {
        let lockfile = lockfile::read_lockfile(home_dir)?;

        // Build sources list
        let mut sources: Vec<SkillSource> = KNOWN_SKILL_SOURCES
            .iter()
            .map(|s| SkillSource::from_short_name(s))
            .collect();

        // Add custom sources from config
        for custom in extra_sources {
            let source = SkillSource {
                name: custom.clone(),
                url: format!("https://github.com/{}.git", custom),
                is_custom: true,
            };
            if !sources.iter().any(|s| s.name == *custom) {
                sources.push(source);
            }
        }

        // Build skills from lockfile entries + scan installed dirs
        let skills_dir = home_dir.join(".agents").join("skills");
        let mut skills = BTreeMap::new();

        // First, add all installed skills from the lockfile
        for (name, entry) in &lockfile.skills {
            let install_path = skills_dir.join(name);
            let description = read_skill_description(&install_path);

            let skill = AvailableSkill {
                name: name.clone(),
                source: entry.source.clone(),
                source_url: entry.source_url.clone(),
                skill_path: entry.skill_path.clone(),
                description,
                installed: true,
                installed_hash: Some(entry.skill_folder_hash.clone()),
                install_path: Some(install_path),
            };
            skills.insert(name.clone(), skill);
        }

        // Then, scan for any skills on disk not in the lockfile (orphaned)
        if skills_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&skills_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if !skills.contains_key(&name) && entry.path().is_dir() {
                        let skill_md = entry.path().join("SKILL.md");
                        if skill_md.exists() {
                            let description = read_skill_description(&entry.path());
                            let skill = AvailableSkill {
                                name: name.clone(),
                                source: String::new(),
                                source_url: String::new(),
                                skill_path: String::new(),
                                description,
                                installed: true,
                                installed_hash: None,
                                install_path: Some(entry.path()),
                            };
                            skills.insert(name, skill);
                        }
                    }
                }
            }
        }

        Ok(Self { sources, skills })
    }

    /// Get a skill by name.
    pub fn get(&self, name: &str) -> Option<&AvailableSkill> {
        self.skills.get(name)
    }

    /// Get all installed skills.
    pub fn installed(&self) -> Vec<&AvailableSkill> {
        self.skills.values().filter(|s| s.installed).collect()
    }

    /// Get all skills from a specific source.
    pub fn skills_from_source(&self, source: &str) -> Vec<&AvailableSkill> {
        self.skills
            .values()
            .filter(|s| s.source == source)
            .collect()
    }

    /// Count of installed skills.
    pub fn installed_count(&self) -> usize {
        self.skills.values().filter(|s| s.installed).count()
    }

    /// Total count of skills.
    pub fn total_count(&self) -> usize {
        self.skills.len()
    }
}

/// Read the first line description from a SKILL.md file.
fn read_skill_description(skill_dir: &Path) -> String {
    let skill_md = skill_dir.join("SKILL.md");
    if let Ok(content) = std::fs::read_to_string(&skill_md) {
        // Take the first non-empty line as description
        for line in content.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                return trimmed.to_string();
            }
            // Use first heading if no body text
            if trimmed.starts_with("# ") {
                return trimmed.trim_start_matches("# ").to_string();
            }
        }
    }
    String::new()
}

/// Parse a skill name from a git repo's skill path.
/// e.g. "skills/deploy-to-vercel/SKILL.md" → "deploy-to-vercel"
pub fn skill_name_from_path(skill_path: &str) -> Option<String> {
    let parts: Vec<&str> = skill_path.split('/').collect();
    if parts.len() >= 2 {
        Some(parts[parts.len() - 2].to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_name_from_path() {
        assert_eq!(
            skill_name_from_path("skills/deploy-to-vercel/SKILL.md"),
            Some("deploy-to-vercel".to_string())
        );
        assert_eq!(
            skill_name_from_path("skills/threejs-animation/SKILL.md"),
            Some("threejs-animation".to_string())
        );
        assert_eq!(skill_name_from_path("SKILL.md"), None);
    }

    #[test]
    fn test_skill_hub_loads_installed() {
        // Use a temp dir so the test works on CI where there's no lockfile
        let tmp = std::env::temp_dir().join("agentry_test_hub_load");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        // With no lockfile, hub should have 0 skills but known sources
        let hub = SkillHub::load(&tmp, &[]).unwrap();
        assert_eq!(hub.installed_count(), 0);
        assert!(!hub.sources.is_empty());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_known_sources_count() {
        assert_eq!(KNOWN_SKILL_SOURCES.len(), 13);
    }

    #[test]
    fn test_custom_sources() {
        let tmp = std::env::temp_dir().join("agentry_test_hub_custom");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let custom = vec!["my-org/custom-skills".to_string()];
        let hub = SkillHub::load(&tmp, &custom).unwrap();
        assert!(hub.sources.iter().any(|s| s.name == "my-org/custom-skills" && s.is_custom));
    }
}