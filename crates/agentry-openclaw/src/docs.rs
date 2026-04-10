use std::path::Path;

use anyhow::{Context, Result};

use crate::discovery::DocType;

/// Read a workspace document's content.
pub fn read_doc(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))
}

/// Write content to a workspace document.
pub fn write_doc(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content).with_context(|| format!("Failed to write {}", path.display()))
}

/// Get the canonical filename for a doc type.
pub fn doc_filename(doc_type: DocType) -> &'static str {
    match doc_type {
        DocType::Agents => "AGENTS.md",
        DocType::Soul => "SOUL.md",
        DocType::Tools => "TOOLS.md",
        DocType::Identity => "IDENTITY.md",
        DocType::Memory => "MEMORY.md",
        DocType::User => "USER.md",
        DocType::Heartbeat => "HEARTBEAT.md",
        DocType::Boot => "BOOT.md",
        DocType::Bootstrap => "BOOTSTRAP.md",
        DocType::Other => "",
    }
}

/// Get a human-readable description for a doc type.
pub fn doc_description(doc_type: DocType) -> &'static str {
    match doc_type {
        DocType::Agents => "Operating instructions, workflow rules, memory management",
        DocType::Soul => "Personality, values, communication style, behavioral boundaries",
        DocType::Tools => "Notes about local tools, environment conventions",
        DocType::Identity => "Agent name, emoji, role, how it introduces itself",
        DocType::Memory => "Curated long-term memory (persistent facts)",
        DocType::User => "About the user — preferences, context, schedule",
        DocType::Heartbeat => "Tiny checklist for proactive heartbeat runs",
        DocType::Boot => "Startup ritual on gateway restart",
        DocType::Bootstrap => "One-time first-run interview script",
        DocType::Other => "Additional document",
    }
}

/// Validate a .lobster YAML workflow file.
pub fn validate_lobster(path: &Path) -> Result<LobsterValidation> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;

    let parsed: serde_yaml::Value = serde_yaml::from_str(&content)
        .with_context(|| format!("Invalid YAML in {}", path.display()))?;

    let mut warnings = Vec::new();
    let mut has_name = false;
    let mut has_steps = false;
    let mut step_count = 0;

    if let Some(name) = parsed.get("name") {
        if name.is_string() || name.is_null() {
            has_name = true;
        } else {
            warnings.push("'name' should be a string".to_string());
        }
    } else {
        warnings.push("Missing 'name' field".to_string());
    }

    if let Some(steps) = parsed.get("steps") {
        if let Some(steps_arr) = steps.as_sequence() {
            has_steps = true;
            step_count = steps_arr.len();
            for (i, step) in steps_arr.iter().enumerate() {
                if step.get("id").is_none() {
                    warnings.push(format!("Step {} missing 'id' field", i + 1));
                }
                if step.get("run").is_none()
                    && step.get("command").is_none()
                    && step.get("pipeline").is_none()
                    && step.get("lobster").is_none()
                {
                    warnings.push(format!(
                        "Step {} missing action (run/command/pipeline/lobster)",
                        i + 1
                    ));
                }
            }
        } else {
            warnings.push("'steps' should be an array".to_string());
        }
    } else {
        warnings.push("Missing 'steps' field".to_string());
    }

    Ok(LobsterValidation {
        valid: has_name && has_steps && warnings.len() <= 2,
        has_name,
        has_steps,
        step_count,
        warnings,
    })
}

/// Result of validating a .lobster workflow.
#[derive(Debug, Clone)]
pub struct LobsterValidation {
    pub valid: bool,
    pub has_name: bool,
    pub has_steps: bool,
    pub step_count: usize,
    pub warnings: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_doc_filename() {
        assert_eq!(doc_filename(DocType::Agents), "AGENTS.md");
        assert_eq!(doc_filename(DocType::Soul), "SOUL.md");
        assert_eq!(doc_filename(DocType::Tools), "TOOLS.md");
    }

    #[test]
    fn test_doc_description() {
        assert!(doc_description(DocType::Agents).contains("instructions"));
        assert!(doc_description(DocType::Soul).contains("Personality"));
    }

    #[test]
    fn test_validate_lobster_valid() {
        let tmp = std::env::temp_dir().join("agentry_test_lobster");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let lobster_content = r#"
name: test-workflow
args:
  tag:
    default: "family"
steps:
  - id: collect
    command: echo hello
  - id: approve
    approval: required
"#;
        let path = tmp.join("test.lobster");
        std::fs::write(&path, lobster_content).unwrap();

        let result = validate_lobster(&path).unwrap();
        assert!(result.valid);
        assert!(result.has_name);
        assert!(result.has_steps);
        assert_eq!(result.step_count, 2);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_validate_lobster_missing_fields() {
        let tmp = std::env::temp_dir().join("agentry_test_lobster_bad");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let lobster_content = "just: some\nrandom: yaml\n";
        let path = tmp.join("bad.lobster");
        std::fs::write(&path, lobster_content).unwrap();

        let result = validate_lobster(&path).unwrap();
        assert!(!result.valid);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_read_write_doc() {
        let tmp = std::env::temp_dir().join("agentry_test_doc");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let path = tmp.join("AGENTS.md");
        write_doc(&path, "# Test Agents\nHello world").unwrap();
        let content = read_doc(&path).unwrap();
        assert!(content.contains("# Test Agents"));

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
