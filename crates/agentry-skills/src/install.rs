use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::lockfile::{self, SkillLockEntry};

/// Result of an install/update/remove operation.
#[derive(Debug)]
pub struct SkillOpResult {
    pub skill_name: String,
    pub success: bool,
    pub message: String,
}

/// Install a skill from a git source repository.
///
/// 1. Clone the repo to a temp dir
/// 2. Copy the skill folder to ~/.agents/skills/<name>/
/// 3. Update the lockfile
/// 4. Create symlinks in each agent's skills/ directory
pub fn install_skill(
    home_dir: &Path,
    source_repo: &str,
    skill_path: &str,
    agents_with_skills_dir: &[PathBuf],
) -> Result<SkillOpResult> {
    let skill_name = extract_skill_name(skill_path);
    let skills_dir = home_dir.join(".agents").join("skills");
    let target_dir = skills_dir.join(&skill_name);

    // Check if already installed
    let mut lockfile = lockfile::read_lockfile(home_dir)?;
    if lockfile::is_skill_installed(&lockfile, &skill_name) {
        return Ok(SkillOpResult {
            skill_name: skill_name.clone(),
            success: false,
            message: format!("Skill '{}' is already installed", skill_name),
        });
    }

    // Clone the repo to a temp directory
    let temp_dir = std::env::temp_dir().join(format!("agentry-skill-{}", &skill_name));
    let _ = std::fs::remove_dir_all(&temp_dir);

    let source_url = if source_repo.starts_with("https://") || source_repo.starts_with("git://") {
        source_repo.to_string()
    } else {
        format!("https://github.com/{}.git", source_repo)
    };

    // Use git2 to clone (shallow clone with depth 1)
    let mut remote_callbacks = git2::RemoteCallbacks::new();
    remote_callbacks.credentials(|_url, username_from_url, _allowed_types| {
        // Try SSH key first, then allow default credentials
        git2::Cred::ssh_key_from_agent(username_from_url.unwrap_or("git"))
            .or_else(|_| git2::Cred::default())
    });

    let mut fetch_opts = git2::FetchOptions::new();
    fetch_opts.remote_callbacks(remote_callbacks);
    fetch_opts.depth(1);

    let mut builder = git2::build::RepoBuilder::new();
    builder.fetch_options(fetch_opts);

    match builder.clone(&source_url, &temp_dir) {
        Ok(_) => {}
        Err(e) => {
            let _ = std::fs::remove_dir_all(&temp_dir);
            return Ok(SkillOpResult {
                skill_name: skill_name.clone(),
                success: false,
                message: format!("Failed to clone {}: {}", source_url, e),
            });
        }
    };

    // Copy the skill folder
    let repo_skill_dir = temp_dir.join(skill_path);
    if !repo_skill_dir.is_dir() {
        // Try without the SKILL.md suffix
        let skill_path_buf = temp_dir.join(skill_path);
        let parent_dir = skill_path_buf.parent().unwrap_or(Path::new("."));
        if !parent_dir.is_dir() {
            let _ = std::fs::remove_dir_all(&temp_dir);
            return Ok(SkillOpResult {
                skill_name: skill_name.clone(),
                success: false,
                message: format!("Skill path '{}' not found in repo", skill_path),
            });
        }
    }

    // Create target directory
    if let Err(e) = std::fs::create_dir_all(&target_dir) {
        let _ = std::fs::remove_dir_all(&temp_dir);
        return Ok(SkillOpResult {
            skill_name: skill_name.clone(),
            success: false,
            message: format!("Failed to create target dir: {}", e),
        });
    }

    // Copy files from repo skill dir to target
    let repo_skill_parent = repo_skill_dir
        .parent()
        .context("No parent dir for skill path")?;
    if let Err(e) = copy_dir_recursive(repo_skill_parent, &target_dir) {
        let _ = std::fs::remove_dir_all(&temp_dir);
        let _ = std::fs::remove_dir_all(&target_dir);
        return Ok(SkillOpResult {
            skill_name: skill_name.clone(),
            success: false,
            message: format!("Failed to copy skill files: {}", e),
        });
    }

    // Compute hash
    let hash = match lockfile::compute_skill_hash(&target_dir) {
        Ok(h) => h,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&temp_dir);
            return Ok(SkillOpResult {
                skill_name: skill_name.clone(),
                success: false,
                message: format!("Failed to compute skill hash: {}", e),
            });
        }
    };

    // Update lockfile
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let entry = SkillLockEntry {
        source: source_repo.to_string(),
        source_type: "github".to_string(),
        source_url,
        skill_path: skill_path.to_string(),
        skill_folder_hash: hash,
        installed_at: now.clone(),
        updated_at: now,
    };
    lockfile::upsert_skill(&mut lockfile, &skill_name, entry);

    if let Err(e) = lockfile::write_lockfile(home_dir, &lockfile) {
        return Ok(SkillOpResult {
            skill_name: skill_name.clone(),
            success: false,
            message: format!("Failed to update lockfile: {}", e),
        });
    }

    // Create symlinks in each agent's skills/ dir
    let symlink_results = create_skill_symlinks(&skill_name, agents_with_skills_dir);
    let symlink_msg = if symlink_results.is_empty() {
        "No agent skills dirs found".to_string()
    } else {
        let ok_count = symlink_results.iter().filter(|r| r.success).count();
        format!("Symlinked to {}/{} agent dirs", ok_count, symlink_results.len())
    };

    // Cleanup temp dir
    let _ = std::fs::remove_dir_all(&temp_dir);

    Ok(SkillOpResult {
        skill_name,
        success: true,
        message: format!("Installed successfully. {}", symlink_msg),
    })
}

/// Update a skill by re-cloning from the source.
pub fn update_skill(
    home_dir: &Path,
    skill_name: &str,
    agents_with_skills_dir: &[PathBuf],
) -> Result<SkillOpResult> {
    let mut lockfile = lockfile::read_lockfile(home_dir)?;

    let entry = match lockfile.skills.get(skill_name) {
        Some(e) => e.clone(),
        None => {
            return Ok(SkillOpResult {
                skill_name: skill_name.to_string(),
                success: false,
                message: format!("Skill '{}' is not installed", skill_name),
            });
        }
    };

    // Remove existing skill folder
    let skills_dir = home_dir.join(".agents").join("skills");
    let target_dir = skills_dir.join(skill_name);
    let old_hash = entry.skill_folder_hash.clone();

    let _ = std::fs::remove_dir_all(&target_dir);

    // Re-install using the same source info
    let temp_dir = std::env::temp_dir().join(format!("agentry-skill-update-{}", skill_name));
    let _ = std::fs::remove_dir_all(&temp_dir);

    let mut remote_callbacks = git2::RemoteCallbacks::new();
    remote_callbacks.credentials(|_url, username_from_url, _allowed_types| {
        git2::Cred::ssh_key_from_agent(username_from_url.unwrap_or("git"))
            .or_else(|_| git2::Cred::default())
    });

    let mut fetch_opts = git2::FetchOptions::new();
    fetch_opts.remote_callbacks(remote_callbacks);
    fetch_opts.depth(1);

    let mut builder = git2::build::RepoBuilder::new();
    builder.fetch_options(fetch_opts);

    match builder.clone(&entry.source_url, &temp_dir) {
        Ok(_) => {}
        Err(e) => {
            let _ = std::fs::remove_dir_all(&temp_dir);
            return Ok(SkillOpResult {
                skill_name: skill_name.to_string(),
                success: false,
                message: format!("Failed to clone {}: {}", entry.source_url, e),
            });
        }
    };

    // Copy from repo
    let repo_skill_parent = temp_dir.join(&entry.skill_path);
    let repo_skill_dir = repo_skill_parent
        .parent()
        .context("No parent dir for skill path")?;

    if let Err(e) = std::fs::create_dir_all(&target_dir) {
        let _ = std::fs::remove_dir_all(&temp_dir);
        return Ok(SkillOpResult {
            skill_name: skill_name.to_string(),
            success: false,
            message: format!("Failed to create target dir: {}", e),
        });
    }

    if let Err(e) = copy_dir_recursive(repo_skill_dir, &target_dir) {
        let _ = std::fs::remove_dir_all(&temp_dir);
        return Ok(SkillOpResult {
            skill_name: skill_name.to_string(),
            success: false,
            message: format!("Failed to copy skill files: {}", e),
        });
    }

    // Compute new hash
    let new_hash = lockfile::compute_skill_hash(&target_dir)?;

    // Update lockfile
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let updated_entry = SkillLockEntry {
        skill_folder_hash: new_hash.clone(),
        updated_at: now,
        ..entry
    };
    lockfile::upsert_skill(&mut lockfile, skill_name, updated_entry);
    lockfile::write_lockfile(home_dir, &lockfile)?;

    // Re-create symlinks
    create_skill_symlinks(skill_name, agents_with_skills_dir);

    let _ = std::fs::remove_dir_all(&temp_dir);

    let changed = old_hash != new_hash;
    let change_msg = if changed {
        format!("Updated (hash changed: {} → {})", old_hash, new_hash)
    } else {
        "Already up to date (hash unchanged)".to_string()
    };

    Ok(SkillOpResult {
        skill_name: skill_name.to_string(),
        success: true,
        message: change_msg,
    })
}

/// Update all installed skills.
pub fn update_all_skills(
    home_dir: &Path,
    agents_with_skills_dir: &[PathBuf],
) -> Vec<SkillOpResult> {
    let lockfile = lockfile::read_lockfile(home_dir).ok();
    let skill_names: Vec<String> = lockfile
        .map(|l| l.skills.keys().cloned().collect())
        .unwrap_or_default();

    skill_names
        .iter()
        .map(|name| {
            update_skill(home_dir, name, agents_with_skills_dir)
                .unwrap_or_else(|e| SkillOpResult {
                    skill_name: name.clone(),
                    success: false,
                    message: format!("Error: {}", e),
                })
        })
        .collect()
}

/// Remove a skill: delete folder, remove symlinks, update lockfile.
pub fn remove_skill(
    home_dir: &Path,
    skill_name: &str,
    agents_with_skills_dir: &[PathBuf],
) -> Result<SkillOpResult> {
    let mut lockfile = lockfile::read_lockfile(home_dir)?;

    // Remove skill folder
    let skills_dir = home_dir.join(".agents").join("skills");
    let target_dir = skills_dir.join(skill_name);
    if target_dir.exists() {
        std::fs::remove_dir_all(&target_dir)?;
    }

    // Remove symlinks from agent dirs
    for agent_skills_dir in agents_with_skills_dir {
        let symlink_path = agent_skills_dir.join(skill_name);
        if symlink_path.exists() || symlink_path.symlink_metadata().is_ok() {
            let _ = std::fs::remove_file(&symlink_path);
        }
    }

    // Update lockfile
    let removed = lockfile::remove_skill(&mut lockfile, skill_name);
    lockfile::write_lockfile(home_dir, &lockfile)?;

    Ok(SkillOpResult {
        skill_name: skill_name.to_string(),
        success: removed,
        message: if removed {
            format!("Removed skill '{}'", skill_name)
        } else {
            format!("Skill '{}' was not in lockfile", skill_name)
        },
    })
}

/// Create relative symlinks for a skill in each agent's skills/ directory.
/// Follows the pattern: ~/.claude/skills/<name> → ../../.agents/skills/<name>
fn create_skill_symlinks(skill_name: &str, agents_with_skills_dir: &[PathBuf]) -> Vec<SkillOpResult> {
    let mut results = Vec::new();

    for agent_skills_dir in agents_with_skills_dir {
        let symlink_path = agent_skills_dir.join(skill_name);
        let link_target = Path::new("../../.agents/skills/").join(skill_name);

        // Ensure the agent skills dir exists
        if let Err(e) = std::fs::create_dir_all(agent_skills_dir) {
            results.push(SkillOpResult {
                skill_name: skill_name.to_string(),
                success: false,
                message: format!(
                    "Failed to create {}: {}",
                    agent_skills_dir.display(),
                    e
                ),
            });
            continue;
        }

        // Remove existing symlink/file
        if symlink_path.exists() || symlink_path.symlink_metadata().is_ok() {
            if let Err(e) = std::fs::remove_file(&symlink_path) {
                results.push(SkillOpResult {
                    skill_name: skill_name.to_string(),
                    success: false,
                    message: format!("Failed to remove existing symlink: {}", e),
                });
                continue;
            }
        }

        // Create relative symlink
        #[cfg(unix)]
        let result = std::os::unix::fs::symlink(&link_target, &symlink_path);
        #[cfg(windows)]
        let result = std::os::windows::fs::symlink_file(&link_target, &symlink_path);

        match result {
            Ok(()) => results.push(SkillOpResult {
                skill_name: skill_name.to_string(),
                success: true,
                message: format!("Symlinked in {}", agent_skills_dir.display()),
            }),
            Err(e) => results.push(SkillOpResult {
                skill_name: skill_name.to_string(),
                success: false,
                message: format!("Symlink error: {}", e),
            }),
        }
    }

    results
}

/// Extract the skill folder name from a skill path.
/// e.g. "skills/deploy-to-vercel/SKILL.md" → "deploy-to-vercel"
fn extract_skill_name(skill_path: &str) -> String {
    let parts: Vec<&str> = skill_path.split('/').collect();
    if parts.len() >= 2 {
        parts[parts.len() - 2].to_string()
    } else {
        // Fallback: use the path itself sanitized
        skill_path
            .replace('/', "-")
            .replace(".md", "")
            .trim_matches('-')
            .to_string()
    }
}

/// Recursively copy a directory.
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    if !dst.exists() {
        std::fs::create_dir_all(dst)?;
    }

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_skill_name() {
        assert_eq!(
            extract_skill_name("skills/deploy-to-vercel/SKILL.md"),
            "deploy-to-vercel"
        );
        assert_eq!(
            extract_skill_name("skills/threejs-animation/SKILL.md"),
            "threejs-animation"
        );
    }

    #[test]
    fn test_copy_dir_recursive() {
        let src = std::env::temp_dir().join("agentry_test_copy_src");
        let dst = std::env::temp_dir().join("agentry_test_copy_dst");
        let _ = std::fs::remove_dir_all(&src);
        let _ = std::fs::remove_dir_all(&dst);

        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("SKILL.md"), "# Test").unwrap();
        std::fs::create_dir_all(src.join("subdir")).unwrap();
        std::fs::write(src.join("subdir/data.txt"), "data").unwrap();

        copy_dir_recursive(&src, &dst).unwrap();

        assert!(dst.join("SKILL.md").exists());
        assert!(dst.join("subdir/data.txt").exists());
        assert_eq!(std::fs::read_to_string(dst.join("SKILL.md")).unwrap(), "# Test");

        let _ = std::fs::remove_dir_all(&src);
        let _ = std::fs::remove_dir_all(&dst);
    }

    #[test]
    fn test_install_already_installed() {
        let tmp = std::env::temp_dir().join("agentry_test_install_exists");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        // Write a lockfile with an existing skill
        let mut lockfile = lockfile::SkillLockfile {
            version: 3,
            skills: std::collections::BTreeMap::new(),
            dismissed: std::collections::BTreeMap::new(),
            last_selected_agents: Vec::new(),
        };
        let entry = lockfile::SkillLockEntry {
            source: "test/repo".to_string(),
            source_type: "github".to_string(),
            source_url: "https://github.com/test/repo.git".to_string(),
            skill_path: "skills/test/SKILL.md".to_string(),
            skill_folder_hash: "abc".to_string(),
            installed_at: "2026-01-01T00:00:00.000Z".to_string(),
            updated_at: "2026-01-01T00:00:00.000Z".to_string(),
        };
        lockfile.skills.insert("test".to_string(), entry);
        lockfile::write_lockfile(&tmp, &lockfile).unwrap();

        let result = install_skill(&tmp, "test/repo", "skills/test/SKILL.md", &[]);
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(!r.success); // Should fail because already installed
        assert!(r.message.contains("already installed"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_remove_skill_not_installed() {
        let tmp = std::env::temp_dir().join("agentry_test_remove_missing");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let result = remove_skill(&tmp, "nonexistent", &[]);
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(!r.success); // Not in lockfile
        assert!(r.message.contains("not in lockfile"));

        let _ = std::fs::remove_dir_all(&tmp);
    }
}