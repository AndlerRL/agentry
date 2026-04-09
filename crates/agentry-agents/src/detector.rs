use std::path::{Path, PathBuf};
use std::process::Command;

use agentry_core::models::{AgentSpec, DetectedAgent};

use crate::spec::all_agent_specs;

/// Detect a single agent on the system.
pub fn detect_agent(spec: &AgentSpec) -> DetectedAgent {
    let home = dirs_home();
    let config_dir = home.join(&spec.config_dir);
    let config_dir_exists = config_dir.is_dir();

    // Check if CLI binary is on PATH
    let binary_found = which_binary(&spec.cli_binary);

    // Try to get version
    let version = if binary_found {
        get_version(&spec.cli_binary)
    } else {
        None
    };

    // Check prompt file
    let prompt_path = config_dir.join(&spec.prompt_filename);
    let prompt_file_exists = prompt_path.exists();

    // Check skills directory
    let skills_dir = spec
        .skills_dir_name
        .as_ref()
        .map(|name| config_dir.join(name));
    let skills_dir_exists = skills_dir.as_ref().is_some_and(|d| d.is_dir());

    // Detect symlink pattern in skills dir
    let skills_symlink_pattern = if skills_dir_exists {
        detect_symlink_pattern(skills_dir.as_ref().unwrap())
    } else {
        None
    };

    // List installed skills
    let installed_skills = if skills_dir_exists {
        list_skills(skills_dir.as_ref().unwrap())
    } else {
        Vec::new()
    };

    let installed = binary_found || config_dir_exists;

    DetectedAgent {
        spec: spec.clone(),
        installed,
        version,
        config_dir_exists,
        prompt_file_exists,
        skills_dir: if skills_dir_exists { skills_dir } else { None },
        skills_symlink_pattern,
        installed_skills,
    }
}

/// Detect all known agents on the system (parallel checks).
pub async fn detect_all_agents() -> Vec<DetectedAgent> {
    let specs = all_agent_specs();
    // Run detection in parallel using tokio tasks
    let handles: Vec<_> = specs
        .into_iter()
        .map(|spec| tokio::task::spawn_blocking(move || detect_agent(&spec)))
        .collect();

    let mut results = Vec::new();
    for handle in handles {
        match handle.await {
            Ok(detected) => results.push(detected),
            Err(e) => eprintln!("Detection task failed: {}", e),
        }
    }

    // Sort: installed first, then by name
    results.sort_by(|a, b| {
        b.installed
            .cmp(&a.installed)
            .then(a.spec.name.cmp(&b.spec.name))
    });

    results
}

/// Get the home directory.
fn dirs_home() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}

/// Check if a binary exists on PATH.
fn which_binary(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Try to get the version of a CLI binary.
fn get_version(binary: &str) -> Option<String> {
    let output = Command::new(binary).arg("--version").output().ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Take first line, extract version-like substring
    let first_line = stdout.lines().next()?;
    let version = first_line
        .split_whitespace()
        .find(|s| s.chars().any(|c| c.is_ascii_digit()))?;
    Some(version.to_string())
}

/// Detect the symlink pattern used in a skills directory.
fn detect_symlink_pattern(skills_dir: &Path) -> Option<String> {
    let entries: Vec<_> = std::fs::read_dir(skills_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .take(5)
        .collect();

    for entry in &entries {
        let path = entry.path();
        if path.is_symlink() {
            if let Ok(target) = std::fs::read_link(&path) {
                let target_str = target.to_string_lossy();
                if target_str.contains("../../.agents/skills/") {
                    return Some("../../.agents/skills/<name>".to_string());
                }
                if target_str.contains("../.agents/skills/") {
                    return Some("../.agents/skills/<name>".to_string());
                }
                return Some(target_str.to_string());
            }
        }
    }
    None
}

/// List skill names from a skills directory.
fn list_skills(skills_dir: &Path) -> Vec<String> {
    std::fs::read_dir(skills_dir)
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir() || e.path().is_symlink())
                .filter_map(|e| e.file_name().to_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}
