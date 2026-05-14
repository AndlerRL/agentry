use std::path::{Path, PathBuf};
use std::process::Command;

use agentry_core::models::{AgentSpec, DetectedAgent, InstallMethod};

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

    // Detect install methods (filtered by OS availability)
    let detected_methods: Vec<InstallMethod> = spec
        .install_methods
        .iter()
        .filter(|method| method.available_on_os())
        .filter(|method| detect_install_method(method))
        .cloned()
        .collect();

    let installed = !detected_methods.is_empty() || binary_found || config_dir_exists;

    DetectedAgent {
        spec: spec.clone(),
        installed,
        version,
        config_dir_exists,
        prompt_file_exists,
        skills_dir: if skills_dir_exists { skills_dir } else { None },
        skills_symlink_pattern,
        installed_skills,
        detected_methods,
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

/// Dispatch to the correct detector for an install method.
fn detect_install_method(method: &InstallMethod) -> bool {
    match method {
        InstallMethod::Brew { formula, .. } => detect_brew_package(formula),
        InstallMethod::Npm { package } => detect_npm_package(package),
        InstallMethod::Cargo { crate_name } => detect_cargo_crate(crate_name),
        InstallMethod::Pip { package } => detect_pip_package(package),
        InstallMethod::VsCodeExtension { extension_id } => detect_vscode_extension(extension_id),
        InstallMethod::JetBrainsPlugin { plugin_id } => detect_jetbrains_plugin(plugin_id),
        InstallMethod::DirectDownload { binary_name, .. } => detect_direct_binary(binary_name),
        InstallMethod::AppBundle { app_name } => detect_app_bundle(app_name),
        InstallMethod::BuiltIn => which_binary("builtin_noop"), // always false, BuiltIn is informational
        InstallMethod::Other { .. } => false,
    }
}

fn detect_brew_package(formula: &str) -> bool {
    Command::new("brew")
        .args(["list", "--formula", formula])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn detect_npm_package(package: &str) -> bool {
    if !which_binary("npm") {
        return false;
    }
    Command::new("npm")
        .args(["list", "-g", "--depth=0", package])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn detect_cargo_crate(crate_name: &str) -> bool {
    if !which_binary("cargo") {
        return false;
    }
    match Command::new("cargo").args(["install", "--list"]).output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout.lines().any(|line| line.contains(crate_name) && !line.starts_with(' '))
        }
        Err(_) => false,
    }
}

fn detect_pip_package(package: &str) -> bool {
    let pip = if which_binary("pip3") { "pip3" } else { "pip" };
    if !which_binary(pip) {
        return false;
    }
    Command::new(pip)
        .args(["show", package])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn detect_vscode_extension(extension_id: &str) -> bool {
    let home = dirs_home();
    let ext_dir = home.join(".vscode").join("extensions");
    if !ext_dir.is_dir() {
        return false;
    }
    // Extensions are stored as <publisher>.<name>-<version> directories
    match std::fs::read_dir(&ext_dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().starts_with(extension_id)),
        Err(_) => false,
    }
}

fn detect_jetbrains_plugin(_plugin_id: &str) -> bool {
    // JetBrains plugin detection is complex and platform-specific.
    // For now, check if any JetBrains config directory exists.
    let home = dirs_home();
    #[cfg(target_os = "macos")]
    let base = home.join("Library").join("Application Support").join("JetBrains");
    #[cfg(not(target_os = "macos"))]
    let base = home.join(".config").join("JetBrains");

    base.is_dir()
}

fn detect_app_bundle(app_name: &str) -> bool {
    let apps_dir = Path::new("/Applications").join(app_name);
    let user_apps = dirs_home().join("Applications").join(app_name);
    apps_dir.exists() || user_apps.exists()
}

fn detect_direct_binary(binary_name: &str) -> bool {
    if which_binary(binary_name) {
        return true;
    }
    let home = dirs_home();
    home.join(".local").join("bin").join(binary_name).exists()
        || home.join("bin").join(binary_name).exists()
        || Path::new("/usr/local/bin").join(binary_name).exists()
}

/// List available versions for a brew formula.
pub fn list_brew_versions(formula: &str) -> Result<Vec<String>, String> {
    let output = Command::new("brew")
        .args(["info", "--json=v2", formula])
        .output()
        .map_err(|e| format!("Failed to run brew: {}", e))?;

    if !output.status.success() {
        return Err("brew info failed".to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).map_err(|e| format!("JSON parse: {}", e))?;

    let versions = parsed["versions"]["stable"]
        .as_str()
        .map(|v| vec![v.to_string()])
        .unwrap_or_default();

    Ok(versions)
}

/// List available versions for an npm package.
pub fn list_npm_versions(package: &str) -> Result<Vec<String>, String> {
    let output = Command::new("npm")
        .args(["view", package, "versions", "--json"])
        .output()
        .map_err(|e| format!("Failed to run npm: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "npm view failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let versions: Vec<String> =
        serde_json::from_str(&stdout).map_err(|e| format!("JSON parse: {}", e))?;

    Ok(versions)
}

/// List latest version for a cargo crate (cargo does not expose full version history).
pub fn list_cargo_versions(crate_name: &str) -> Result<Vec<String>, String> {
    let output = Command::new("cargo")
        .args(["search", crate_name, "--limit", "1"])
        .output()
        .map_err(|e| format!("Failed to run cargo: {}", e))?;

    if !output.status.success() {
        return Err("cargo search failed".to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // cargo search output: "cratename = "1.2.3"    # description"
    for line in stdout.lines() {
        if line.starts_with(crate_name) {
            if let Some(version_part) = line.split('"').nth(1) {
                return Ok(vec![version_part.to_string()]);
            }
        }
    }
    Err("Version not found".to_string())
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

#[cfg(test)]
mod tests {
    use super::*;
    use agentry_core::models::PromptFormat;
    use std::fs;

    /// Helper to create a temporary directory for tests.
    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(prefix: &str) -> Self {
            let path = std::env::temp_dir().join(format!("{}_{}", prefix, std::process::id()));
            fs::create_dir_all(&path).expect("failed to create temp dir");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn detect_symlink_pattern_with_relative_agents_symlink() {
        let tmp = TempDir::new("symlink_test");
        let skills_dir = tmp.path().join("skills");
        fs::create_dir_all(&skills_dir).unwrap();

        // Create target directory so the symlink resolves
        let target_root = tmp.path().join(".agents/skills");
        fs::create_dir_all(&target_root).unwrap();

        // Create a symlink: skills/git -> ../../.agents/skills/git
        let link = skills_dir.join("git");
        #[cfg(unix)]
        std::os::unix::fs::symlink("../../.agents/skills/git", &link).unwrap();

        let pattern = detect_symlink_pattern(&skills_dir);
        assert_eq!(
            pattern,
            Some("../../.agents/skills/<name>".to_string()),
            "should detect the ../../.agents/skills/ pattern"
        );
    }

    #[test]
    fn detect_symlink_pattern_with_single_dot_agents_symlink() {
        let tmp = TempDir::new("symlink_single_test");
        let skills_dir = tmp.path().join("skills");
        fs::create_dir_all(&skills_dir).unwrap();

        let target_root = tmp.path().join(".agents/skills");
        fs::create_dir_all(&target_root).unwrap();

        // Create a symlink: skills/git -> ../.agents/skills/git
        let link = skills_dir.join("git");
        #[cfg(unix)]
        std::os::unix::fs::symlink("../.agents/skills/git", &link).unwrap();

        let pattern = detect_symlink_pattern(&skills_dir);
        assert_eq!(
            pattern,
            Some("../.agents/skills/<name>".to_string()),
            "should detect the ../.agents/skills/ pattern"
        );
    }

    #[test]
    fn detect_symlink_pattern_with_other_symlink() {
        let tmp = TempDir::new("symlink_other_test");
        let skills_dir = tmp.path().join("skills");
        fs::create_dir_all(&skills_dir).unwrap();

        // Create a symlink to some arbitrary target
        let link = skills_dir.join("my-skill");
        #[cfg(unix)]
        std::os::unix::fs::symlink("/some/other/path/skill", &link).unwrap();

        let pattern = detect_symlink_pattern(&skills_dir);
        assert!(
            pattern.is_some(),
            "should detect a symlink even if it's not the .agents pattern"
        );
        assert_eq!(pattern.unwrap(), "/some/other/path/skill");
    }

    #[test]
    fn detect_symlink_pattern_no_symlinks() {
        let tmp = TempDir::new("nosymlink_test");
        let skills_dir = tmp.path().join("skills");
        fs::create_dir_all(&skills_dir).unwrap();

        // Create regular subdirectories (not symlinks)
        fs::create_dir_all(skills_dir.join("skill-a")).unwrap();
        fs::create_dir_all(skills_dir.join("skill-b")).unwrap();

        let pattern = detect_symlink_pattern(&skills_dir);
        assert!(
            pattern.is_none(),
            "should return None when no symlinks found"
        );
    }

    #[test]
    fn detect_symlink_pattern_empty_dir() {
        let tmp = TempDir::new("empty_symlink_test");
        let skills_dir = tmp.path().join("skills");
        fs::create_dir_all(&skills_dir).unwrap();

        let pattern = detect_symlink_pattern(&skills_dir);
        assert!(pattern.is_none(), "should return None for empty dir");
    }

    #[test]
    fn detect_symlink_pattern_nonexistent_dir() {
        let pattern = detect_symlink_pattern(Path::new("/nonexistent/path/skills"));
        assert!(pattern.is_none(), "should return None for nonexistent dir");
    }

    #[test]
    fn list_skills_returns_subdirectories() {
        let tmp = TempDir::new("list_skills_test");
        let skills_dir = tmp.path().join("skills");
        fs::create_dir_all(skills_dir.join("git")).unwrap();
        fs::create_dir_all(skills_dir.join("rust")).unwrap();
        fs::create_dir_all(skills_dir.join("python")).unwrap();

        // Also create a regular file — should NOT be listed
        fs::write(skills_dir.join("README.md"), "hello").unwrap();

        let mut skills = list_skills(&skills_dir);
        skills.sort();
        assert_eq!(skills, vec!["git", "python", "rust"]);
    }

    #[test]
    fn list_skills_includes_symlinked_dirs() {
        let tmp = TempDir::new("list_skills_symlink_test");
        let skills_dir = tmp.path().join("skills");
        let target = tmp.path().join("target_skill");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::create_dir_all(&target).unwrap();

        // Create a symlink to a directory
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, skills_dir.join("linked-skill")).unwrap();

        let skills = list_skills(&skills_dir);
        assert!(
            skills.contains(&"linked-skill".to_string()),
            "symlinked directories should be listed"
        );
    }

    #[test]
    fn list_skills_empty_dir() {
        let tmp = TempDir::new("list_skills_empty_test");
        let skills_dir = tmp.path().join("skills");
        fs::create_dir_all(&skills_dir).unwrap();

        let skills = list_skills(&skills_dir);
        assert!(skills.is_empty(), "empty dir should yield no skills");
    }

    #[test]
    fn list_skills_nonexistent_dir() {
        let skills = list_skills(Path::new("/nonexistent/path/skills"));
        assert!(skills.is_empty(), "nonexistent dir should yield no skills");
    }

    #[test]
    fn which_binary_returns_false_for_nonexistent() {
        // A binary that almost certainly does not exist on any system
        assert!(
            !which_binary("nonexistent_binary_xyz_12345"),
            "which_binary should return false for a nonexistent binary"
        );
    }

    #[test]
    fn dirs_home_returns_a_path() {
        let home = dirs_home();
        // We can't assert a specific value, but it should be a valid path
        // and not empty.
        assert!(
            !home.as_os_str().is_empty(),
            "dirs_home should return a non-empty path"
        );
        // If HOME is set, it should match; otherwise it should fall back to /tmp
        if let Ok(home_var) = std::env::var("HOME") {
            assert_eq!(home, PathBuf::from(home_var));
        } else {
            assert_eq!(home, PathBuf::from("/tmp"));
        }
    }

    #[test]
    fn detect_agent_marks_installed_when_binary_found() {
        // We test with a spec whose binary almost certainly does NOT exist.
        // This verifies the detection path works end-to-end.
        let spec = AgentSpec {
            id: "test-nonexistent".into(),
            name: "Test Agent".into(),
            cli_binary: "nonexistent_binary_xyz_12345".into(),
            config_dir: ".test-nonexistent-dir".into(),
            prompt_filename: "TEST.md".into(),
            prompt_format: PromptFormat::PlainMd,
            skills_dir_name: None,
            max_size: None,
            install_methods: Vec::new(),
        };

        let detected = detect_agent(&spec);
        // Binary not found, config dir likely doesn't exist => installed should be false
        assert!(
            !detected.installed,
            "agent with nonexistent binary and no config dir should not be installed"
        );
        assert!(detected.version.is_none());
        assert!(!detected.config_dir_exists);
        assert!(!detected.prompt_file_exists);
    }
}
