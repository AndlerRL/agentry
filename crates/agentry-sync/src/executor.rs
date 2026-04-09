use std::path::Path;

use agentry_core::format::convert_to;
use agentry_core::models::{SyncAction, SyncMapping, SyncStatus, UnifiedPrompt};

/// Result of a single sync operation.
#[derive(Debug)]
pub struct SyncResult {
    pub mapping: SyncMapping,
    pub success: bool,
    pub message: String,
}

/// Execute a sync plan — write/copy/symlink each mapping.
pub fn execute_sync(
    prompt: &UnifiedPrompt,
    mappings: &[SyncMapping],
    dry_run: bool,
) -> Vec<SyncResult> {
    mappings
        .iter()
        .map(|mapping| execute_mapping(prompt, mapping, dry_run))
        .collect()
}

/// Execute a single sync mapping.
fn execute_mapping(
    prompt: &UnifiedPrompt,
    mapping: &SyncMapping,
    dry_run: bool,
) -> SyncResult {
    match mapping.action {
        SyncAction::Skip => SyncResult {
            mapping: mapping.clone(),
            success: true,
            message: "Skipped (source agent)".to_string(),
        },
        SyncAction::Source => SyncResult {
            mapping: mapping.clone(),
            success: true,
            message: "Source — no action needed".to_string(),
        },
        SyncAction::Copy => copy_prompt(prompt, mapping, dry_run),
        SyncAction::Symlink => symlink_prompt(mapping, dry_run),
    }
}

/// Copy a prompt to the destination with format conversion.
fn copy_prompt(
    prompt: &UnifiedPrompt,
    mapping: &SyncMapping,
    dry_run: bool,
) -> SyncResult {
    // Convert prompt to target format
    let content = match convert_to(prompt, mapping.target_format) {
        Ok(c) => c,
        Err(e) => {
            return SyncResult {
                mapping: mapping.clone(),
                success: false,
                message: format!("Format conversion error: {}", e),
            }
        }
    };

    // Check for size limits
    if let Some(parent_spec) = get_agent_max_size(&mapping.agent_id) {
        if content.len() > parent_spec {
            return SyncResult {
                mapping: mapping.clone(),
                success: false,
                message: format!(
                    "Content exceeds size limit ({} > {} bytes)",
                    content.len(),
                    parent_spec
                ),
            };
        }
    }

    if dry_run {
        return SyncResult {
            mapping: mapping.clone(),
            success: true,
            message: format!(
                "[DRY RUN] Would write {} bytes to {}",
                content.len(),
                mapping.destination.display()
            ),
        };
    }

    // Ensure parent directory exists
    if let Some(parent) = mapping.destination.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return SyncResult {
                mapping: mapping.clone(),
                success: false,
                message: format!("Failed to create directory: {}", e),
            };
        }
    }

    // Write the file
    match std::fs::write(&mapping.destination, &content) {
        Ok(()) => SyncResult {
            mapping: mapping.clone(),
            success: true,
            message: format!("Written to {}", mapping.destination.display()),
        },
        Err(e) => SyncResult {
            mapping: mapping.clone(),
            success: false,
            message: format!("Write error: {}", e),
        },
    }
}

/// Create a relative symlink from destination to source.
fn symlink_prompt(mapping: &SyncMapping, dry_run: bool) -> SyncResult {
    if dry_run {
        return SyncResult {
            mapping: mapping.clone(),
            success: true,
            message: format!(
                "[DRY RUN] Would symlink to {}",
                mapping.destination.display()
            ),
        };
    }

    // Remove existing file/symlink
    if mapping.destination.exists() || mapping.destination.symlink_metadata().is_ok() {
        if let Err(e) = std::fs::remove_file(&mapping.destination) {
            return SyncResult {
                mapping: mapping.clone(),
                success: false,
                message: format!("Failed to remove existing: {}", e),
            };
        }
    }

    // Ensure parent directory exists
    if let Some(parent) = mapping.destination.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return SyncResult {
                mapping: mapping.clone(),
                success: false,
                message: format!("Failed to create directory: {}", e),
            };
        }
    }

    // Compute relative path
    // Default pattern: ../../.agents/skills/<name>
    let link_target = mapping.destination.file_name()
        .and_then(|n| Path::new("../../.agents/skills/").join(n).to_str().map(String::from))
        .unwrap_or_else(|| "../.agents/skills/".to_string());

    #[cfg(unix)]
    let result = std::os::unix::fs::symlink(&link_target, &mapping.destination);
    #[cfg(windows)]
    let result = std::os::windows::fs::symlink_file(&link_target, &mapping.destination);

    match result {
        Ok(()) => SyncResult {
            mapping: mapping.clone(),
            success: true,
            message: format!("Symlinked to {}", mapping.destination.display()),
        },
        Err(e) => SyncResult {
            mapping: mapping.clone(),
            success: false,
            message: format!("Symlink error: {}", e),
        },
    }
}

/// Check the status of each mapping by comparing source and destination content.
pub fn check_sync_status(prompt: &UnifiedPrompt, mappings: &[SyncMapping]) -> Vec<SyncMapping> {
    mappings
        .iter()
        .map(|m| {
            let status = if !m.destination.exists() {
                SyncStatus::Missing
            } else {
                // Compare converted content with destination
                match convert_to(prompt, m.target_format) {
                    Ok(expected) => match std::fs::read_to_string(&m.destination) {
                        Ok(existing) => {
                            if existing.trim() == expected.trim() {
                                SyncStatus::UpToDate
                            } else {
                                SyncStatus::Outdated
                            }
                        }
                        Err(_) => SyncStatus::Conflict,
                    },
                    Err(_) => SyncStatus::Conflict,
                }
            };

            SyncMapping {
                status,
                ..m.clone()
            }
        })
        .collect()
}

/// Get max size for agents with known limits.
fn get_agent_max_size(agent_id: &str) -> Option<usize> {
    match agent_id {
        "codex" => Some(32768), // 32 KiB
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn make_prompt(name: &str) -> UnifiedPrompt {
        UnifiedPrompt {
            id: name.to_string(),
            name: name.to_string(),
            description: String::new(),
            frontmatter: BTreeMap::new(),
            body: "Test content".to_string(),
            xml_tags: vec![],
            scope: agentry_core::models::PromptScope::Global,
            source_format: agentry_core::models::PromptFormat::PlainMd,
            source_path: None,
        }
    }

    #[test]
    fn test_skip_action() {
        let prompt = make_prompt("test");
        let mapping = SyncMapping {
            prompt_id: "test".to_string(),
            agent_id: "continue".to_string(),
            destination: PathBuf::from("/tmp/test"),
            target_format: agentry_core::models::PromptFormat::PlainMd,
            action: SyncAction::Skip,
            status: agentry_core::models::SyncStatus::Missing,
        };
        let result = execute_sync(&prompt, &[mapping.clone()], false);
        assert!(result[0].success);
        assert!(result[0].message.contains("Skipped"));
    }

    #[test]
    fn test_dry_run_copy() {
        let prompt = make_prompt("test");
        let mapping = SyncMapping {
            prompt_id: "test".to_string(),
            agent_id: "claude-code".to_string(),
            destination: PathBuf::from("/tmp/test_sync_dry_run/CLAUDE.md"),
            target_format: agentry_core::models::PromptFormat::PlainMd,
            action: SyncAction::Copy,
            status: agentry_core::models::SyncStatus::Missing,
        };
        let result = execute_sync(&prompt, &[mapping], true);
        assert!(result[0].success);
        assert!(result[0].message.contains("DRY RUN"));
    }

    #[test]
    fn test_copy_creates_directory() {
        let tmp_dir = std::env::temp_dir().join("agentry_test_sync");
        let dest = tmp_dir.join("subdir").join("CLAUDE.md");
        let prompt = make_prompt("test");
        let mapping = SyncMapping {
            prompt_id: "test".to_string(),
            agent_id: "claude-code".to_string(),
            destination: dest.clone(),
            target_format: agentry_core::models::PromptFormat::PlainMd,
            action: SyncAction::Copy,
            status: agentry_core::models::SyncStatus::Missing,
        };
        let result = execute_sync(&prompt, &[mapping], false);
        assert!(result[0].success);
        assert!(dest.exists());
        // Cleanup
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }
}