use std::path::{Path, PathBuf};

use crate::format::{converter_for, FormatConverter, PlainMarkdownConverter};
use crate::models::{PromptFormat, PromptScope, UnifiedPrompt};

/// Scan for prompts across all known locations.
pub fn discover_prompts(home_dir: &Path, project_dirs: &[PathBuf]) -> Vec<UnifiedPrompt> {
    let mut prompts = Vec::new();

    // 1. ~/.agents/prompts/ — canonical store
    let canonical_dir = home_dir.join(".agents").join("prompts");
    if canonical_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&canonical_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "md").unwrap_or(false) {
                    if let Some(prompt) =
                        load_prompt_file(&path, PromptFormat::PlainMd, PromptScope::Global)
                    {
                        prompts.push(prompt);
                    }
                }
            }
        }
    }

    // 2. ~/.continue/prompts/ — Continue prompts (YAML frontmatter + XML tags)
    let continue_prompts = home_dir.join(".continue").join("prompts");
    if continue_prompts.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&continue_prompts) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "md").unwrap_or(false) {
                    if let Some(prompt) =
                        load_prompt_file(&path, PromptFormat::XmlTagMd, PromptScope::Global)
                    {
                        prompts.push(prompt);
                    }
                }
            }
        }
    }

    // 3. ~/.continue/rules/ — Continue rules (YAML frontmatter + <base_rules>)
    let continue_rules = home_dir.join(".continue").join("rules");
    if continue_rules.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&continue_rules) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "md").unwrap_or(false) {
                    if let Some(prompt) =
                        load_prompt_file(&path, PromptFormat::XmlTagMd, PromptScope::Global)
                    {
                        prompts.push(prompt);
                    }
                }
            }
        }
    }

    // 4. ~/.claude/CLAUDE.md — Claude global prompt (Plain MD)
    let claude_md = home_dir.join(".claude").join("CLAUDE.md");
    if claude_md.exists() {
        if let Some(prompt) =
            load_prompt_file(&claude_md, PromptFormat::PlainMd, PromptScope::Global)
        {
            prompts.push(prompt);
        }
    }

    // 5. ~/.gemini/GEMINI.md — Gemini global prompt
    let gemini_md = home_dir.join(".gemini").join("GEMINI.md");
    if gemini_md.exists() {
        if let Some(prompt) =
            load_prompt_file(&gemini_md, PromptFormat::PlainMd, PromptScope::Global)
        {
            prompts.push(prompt);
        }
    }

    // 6. Project-level prompts
    for project_dir in project_dirs {
        if let Ok(entries) = std::fs::read_dir(project_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    // Check for project-level prompt files
                    for filename in &["CLAUDE.md", "AGENTS.md", "GEMINI.md", "SOUL.md"] {
                        let prompt_file = path.join(filename);
                        if prompt_file.exists() {
                            if let Some(prompt) = load_prompt_file(
                                &prompt_file,
                                PromptFormat::PlainMd,
                                PromptScope::Project { root: path.clone() },
                            ) {
                                prompts.push(prompt);
                            }
                        }
                    }
                }
            }
        }
    }

    let mut seen = std::collections::HashSet::new();
    prompts.retain(|p| seen.insert((p.name.clone(), p.scope.clone())));
    prompts
}

/// Load a single prompt file from disk.
fn load_prompt_file(
    path: &Path,
    format: PromptFormat,
    scope: PromptScope,
) -> Option<UnifiedPrompt> {
    let content = std::fs::read_to_string(path).ok()?;
    let name = path.file_stem()?.to_str()?.to_string();

    // Auto-detect format based on content
    let detected_format = detect_format(&content, format);
    let converter = converter_for(detected_format);

    let mut prompt = converter
        .parse(&name, &content, Some(path.to_path_buf()))
        .ok()?;
    prompt.scope = scope;
    prompt.source_format = detected_format;

    // If the file is in the canonical prompts dir, use PlainMd as the canonical format
    if let Some(parent) = path.parent() {
        if parent.ends_with(".agents/prompts") {
            prompt.source_format = PromptFormat::PlainMd;
        }
    }

    Some(prompt)
}

/// Detect the actual format of a file based on its content.
fn detect_format(content: &str, fallback: PromptFormat) -> PromptFormat {
    let trimmed = content.trim_start();

    // If it starts with ---, it has frontmatter
    if trimmed.starts_with("---") {
        // Check if it contains XML tags in the body
        if content.contains("<expertise>")
            || content.contains("<base_rules>")
            || content.contains("<rules>")
        {
            return PromptFormat::XmlTagMd;
        }
        // If it has "globs:" in frontmatter, it's MDC
        if content.contains("globs:") || content.contains("alwaysApply:") {
            return PromptFormat::Mdc;
        }
        // Otherwise frontmatter + markdown
        return PromptFormat::FrontmatterMd;
    }

    // Plain markdown
    fallback
}

/// Save a prompt to the canonical store.
pub fn save_prompt(home_dir: &Path, prompt: &UnifiedPrompt) -> anyhow::Result<PathBuf> {
    let canonical_dir = home_dir.join(".agents").join("prompts");
    std::fs::create_dir_all(&canonical_dir)?;

    let filename = prompt.canonical_filename();
    let path = canonical_dir.join(&filename);

    let converter = PlainMarkdownConverter;
    let content = converter.serialize(prompt)?;
    std::fs::write(&path, content)?;

    Ok(path)
}

/// Delete a prompt from the canonical store.
pub fn delete_prompt(home_dir: &Path, name: &str) -> anyhow::Result<()> {
    let canonical_dir = home_dir.join(".agents").join("prompts");
    let path = canonical_dir.join(format!("{}.md", name));

    if path.exists() {
        std::fs::remove_file(&path)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_discover_prompts_dedups_synced_copies() {
        let tmp = TempDir::new("agentry_test_discovery_dedup");
        let canonical_dir = tmp.path().join(".agents").join("prompts");
        let gemini_dir = tmp.path().join(".gemini");
        std::fs::create_dir_all(&canonical_dir).unwrap();
        std::fs::create_dir_all(&gemini_dir).unwrap();

        let canonical_path = canonical_dir.join("GEMINI.md");
        let synced_path = gemini_dir.join("GEMINI.md");
        std::fs::write(&canonical_path, "# GEMINI\n\nCanonical prompt").unwrap();
        std::fs::write(&synced_path, "# GEMINI\n\nSynced copy").unwrap();

        let prompts = discover_prompts(tmp.path(), &[]);
        let gemini: Vec<&UnifiedPrompt> = prompts.iter().filter(|p| p.name == "GEMINI").collect();

        assert_eq!(gemini.len(), 1);
        assert_eq!(
            gemini[0].source_path.as_deref(),
            Some(canonical_path.as_path())
        );
    }

    #[test]
    fn test_discover_prompts_keeps_same_name_across_scopes() {
        let tmp = TempDir::new("agentry_test_discovery_scopes");
        let canonical_dir = tmp.path().join(".agents").join("prompts");
        let project_dir = tmp.path().join("Development").join("some-project");
        std::fs::create_dir_all(&canonical_dir).unwrap();
        std::fs::create_dir_all(&project_dir).unwrap();

        let canonical_path = canonical_dir.join("GEMINI.md");
        let project_path = project_dir.join("GEMINI.md");
        std::fs::write(&canonical_path, "# GEMINI\n\nCanonical prompt").unwrap();
        std::fs::write(&project_path, "# GEMINI\n\nProject prompt").unwrap();

        let prompts = discover_prompts(tmp.path(), &[tmp.path().join("Development")]);
        let gemini: Vec<&UnifiedPrompt> = prompts.iter().filter(|p| p.name == "GEMINI").collect();

        assert_eq!(gemini.len(), 2);
        assert!(gemini.iter().any(|p| {
            p.scope == PromptScope::Global
                && p.source_path.as_deref() == Some(canonical_path.as_path())
        }));
        assert!(gemini.iter().any(|p| {
            p.scope
                == PromptScope::Project {
                    root: project_dir.clone(),
                }
                && p.source_path.as_deref() == Some(project_path.as_path())
        }));
    }

    #[test]
    fn test_detect_format_plain() {
        assert_eq!(
            detect_format("# Hello\n\nWorld", PromptFormat::PlainMd),
            PromptFormat::PlainMd
        );
    }

    #[test]
    fn test_detect_format_frontmatter() {
        assert_eq!(
            detect_format("---\nname: test\n---\n\nHello", PromptFormat::PlainMd),
            PromptFormat::FrontmatterMd
        );
    }

    #[test]
    fn test_detect_format_xml_tag() {
        assert_eq!(
            detect_format(
                "---\nname: test\n---\n\n<expertise>\nHello\n</expertise>",
                PromptFormat::PlainMd
            ),
            PromptFormat::XmlTagMd
        );
    }

    #[test]
    fn test_detect_format_mdc() {
        assert_eq!(
            detect_format(
                "---\nglobs: \"**/*.ts\"\n---\n\nUse strict TS",
                PromptFormat::PlainMd
            ),
            PromptFormat::Mdc
        );
    }
}
