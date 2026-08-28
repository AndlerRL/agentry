use std::path::PathBuf;

use agentry_core::models::{DetectedAgent, PromptFormat};

use crate::engine::CheckContext;
use crate::report::{AuditFinding, FindingCategory, FixAction, Severity};

pub fn run(ctx: &CheckContext) -> Vec<AuditFinding> {
    let mut findings = Vec::new();
    for agent in &ctx.agents {
        findings.extend(missing(ctx, agent));
        findings.extend(empty(ctx, agent));
        findings.extend(oversized(ctx, agent));
        findings.extend(frontmatter_invalid(ctx, agent));
        findings.extend(format_mismatch(ctx, agent));
    }
    findings
}

fn missing(ctx: &CheckContext, agent: &DetectedAgent) -> Vec<AuditFinding> {
    let path = prompt_path(ctx, agent);
    let exists = if is_directory_prompt(&agent.spec.prompt_filename) {
        path.is_dir()
    } else {
        path.exists()
    };
    if exists {
        return Vec::new();
    }
    let prompt_id = canonical_prompt_id(ctx, &agent.spec.prompt_filename);
    let remediation = format!(
        "Run 'agentry sync --prompt {}' to sync '{}' from the canonical store",
        prompt_id, agent.spec.prompt_filename
    );
    vec![AuditFinding {
        check_id: "prompt.missing".to_string(),
        severity: Severity::Warning,
        category: FindingCategory::PromptFile,
        agent_id: Some(agent.spec.id.clone()),
        message: format!(
            "{} prompt file '{}' is missing at '{}'",
            agent.spec.name,
            agent.spec.prompt_filename,
            path.display()
        ),
        remediation,
        auto_fixable: true,
        fix: Some(FixAction::SyncPrompt {
            prompt_id,
            agent_id: agent.spec.id.clone(),
        }),
        evidence: Some(format!("{} does not exist", path.display())),
    }]
}

fn empty(ctx: &CheckContext, agent: &DetectedAgent) -> Vec<AuditFinding> {
    if is_directory_prompt(&agent.spec.prompt_filename) {
        return Vec::new();
    }
    let path = prompt_path(ctx, agent);
    if !path.is_file() {
        return Vec::new();
    }
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    if !content.trim().is_empty() {
        return Vec::new();
    }
    let prompt_id = canonical_prompt_id(ctx, &agent.spec.prompt_filename);
    let remediation = format!(
        "Run 'agentry sync --prompt {}' or edit '{}' to restore its content",
        prompt_id,
        path.display()
    );
    vec![AuditFinding {
        check_id: "prompt.empty".to_string(),
        severity: Severity::Warning,
        category: FindingCategory::PromptFile,
        agent_id: Some(agent.spec.id.clone()),
        message: format!(
            "{} prompt file '{}' is empty or whitespace-only",
            agent.spec.name,
            path.display()
        ),
        remediation,
        auto_fixable: true,
        fix: Some(FixAction::SyncPrompt {
            prompt_id,
            agent_id: agent.spec.id.clone(),
        }),
        evidence: Some(format!(
            "bytes={} whitespace_only=true path={}",
            content.len(),
            path.display()
        )),
    }]
}

fn oversized(ctx: &CheckContext, agent: &DetectedAgent) -> Vec<AuditFinding> {
    if is_directory_prompt(&agent.spec.prompt_filename) {
        return Vec::new();
    }
    let Some(max_size) = agent.spec.max_size else {
        return Vec::new();
    };
    let path = prompt_path(ctx, agent);
    if !path.is_file() {
        return Vec::new();
    }
    let Ok(meta) = std::fs::metadata(&path) else {
        return Vec::new();
    };
    let size = meta.len();
    if size <= max_size as u64 {
        return Vec::new();
    }
    vec![AuditFinding {
        check_id: "prompt.oversized".to_string(),
        severity: Severity::Warning,
        category: FindingCategory::PromptFile,
        agent_id: Some(agent.spec.id.clone()),
        message: format!(
            "{} prompt file '{}' is {} bytes, above the {} byte limit",
            agent.spec.name,
            path.display(),
            size,
            max_size
        ),
        remediation: format!("Trim '{}' to fit within {} bytes", path.display(), max_size),
        auto_fixable: false,
        fix: None,
        evidence: Some(format!(
            "size={} max_size={} path={}",
            size,
            max_size,
            path.display()
        )),
    }]
}

fn frontmatter_invalid(ctx: &CheckContext, agent: &DetectedAgent) -> Vec<AuditFinding> {
    if !matches!(
        agent.spec.prompt_format,
        PromptFormat::FrontmatterMd | PromptFormat::Mdc
    ) {
        return Vec::new();
    }
    if is_directory_prompt(&agent.spec.prompt_filename) {
        return Vec::new();
    }
    let path = prompt_path(ctx, agent);
    if !path.is_file() {
        return Vec::new();
    }
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let problem = match frontmatter_status(&content) {
        None => return Vec::new(),
        Some(FrontmatterStatus::Unterminated) => {
            "frontmatter block is unterminated (no closing '---')".to_string()
        }
        Some(FrontmatterStatus::Closed { yaml }) => {
            match serde_yaml::from_str::<serde_yaml::Value>(&yaml) {
                Ok(_) => return Vec::new(),
                Err(err) => format!("frontmatter YAML failed to parse: {}", err),
            }
        }
    };
    vec![AuditFinding {
        check_id: "prompt.frontmatter_invalid".to_string(),
        severity: Severity::Warning,
        category: FindingCategory::PromptFile,
        agent_id: Some(agent.spec.id.clone()),
        message: format!(
            "{} prompt file '{}' has invalid frontmatter: {}",
            agent.spec.name,
            path.display(),
            problem
        ),
        remediation: format!(
            "Fix the YAML between the '---' markers in '{}'",
            path.display()
        ),
        auto_fixable: false,
        fix: None,
        evidence: Some(format!("path={} reason={}", path.display(), problem)),
    }]
}

fn format_mismatch(ctx: &CheckContext, agent: &DetectedAgent) -> Vec<AuditFinding> {
    if is_directory_prompt(&agent.spec.prompt_filename) {
        return Vec::new();
    }
    let path = prompt_path(ctx, agent);
    if !path.is_file() {
        return Vec::new();
    }
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let declared = agent.spec.prompt_format;
    let (detected, mismatched) = match frontmatter_status(&content) {
        None => (
            PromptFormat::PlainMd,
            matches!(declared, PromptFormat::FrontmatterMd | PromptFormat::Mdc),
        ),
        Some(FrontmatterStatus::Unterminated) => return Vec::new(),
        Some(FrontmatterStatus::Closed { .. }) => (
            classify_frontmatter(&content),
            declared == PromptFormat::PlainMd,
        ),
    };
    if !mismatched {
        return Vec::new();
    }
    let prompt_id = canonical_prompt_id(ctx, &agent.spec.prompt_filename);
    let remediation = format!(
        "Run 'agentry sync --prompt {}' to convert '{}' to the declared {} format",
        prompt_id,
        path.display(),
        declared
    );
    vec![AuditFinding {
        check_id: "prompt.format_mismatch".to_string(),
        severity: Severity::Info,
        category: FindingCategory::PromptFile,
        agent_id: Some(agent.spec.id.clone()),
        message: format!(
            "{} prompt file '{}' parses as {} but the spec declares {}",
            agent.spec.name,
            path.display(),
            detected,
            declared
        ),
        remediation,
        auto_fixable: true,
        fix: Some(FixAction::SyncPrompt {
            prompt_id,
            agent_id: agent.spec.id.clone(),
        }),
        evidence: Some(format!("declared={} detected={}", declared, detected)),
    }]
}

enum FrontmatterStatus {
    Closed { yaml: String },
    Unterminated,
}

fn frontmatter_status(content: &str) -> Option<FrontmatterStatus> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }
    let after_marker = &trimmed[3..];
    let offset = after_marker
        .find(|c: char| !c.is_whitespace())
        .unwrap_or(after_marker.len());
    let rest = &trimmed[3 + offset..];
    if rest.starts_with("---") {
        return Some(FrontmatterStatus::Closed {
            yaml: String::new(),
        });
    }
    match rest.find("\n---") {
        Some(close) => Some(FrontmatterStatus::Closed {
            yaml: rest[..close].to_string(),
        }),
        None => Some(FrontmatterStatus::Unterminated),
    }
}

fn classify_frontmatter(content: &str) -> PromptFormat {
    if content.contains("<expertise>")
        || content.contains("<base_rules>")
        || content.contains("<rules>")
    {
        return PromptFormat::XmlTagMd;
    }
    if content.contains("globs:") || content.contains("alwaysApply:") {
        return PromptFormat::Mdc;
    }
    PromptFormat::FrontmatterMd
}

fn is_directory_prompt(prompt_filename: &str) -> bool {
    prompt_filename.ends_with('/') || matches!(prompt_filename, "prompts" | "rules")
}

fn prompt_path(ctx: &CheckContext, agent: &DetectedAgent) -> PathBuf {
    ctx.home_dir
        .join(&agent.spec.config_dir)
        .join(&agent.spec.prompt_filename)
}

fn canonical_prompt_id(ctx: &CheckContext, prompt_filename: &str) -> String {
    ctx.prompts
        .iter()
        .find(|prompt| prompt.canonical_filename() == prompt_filename)
        .map(|prompt| prompt.id.clone())
        .unwrap_or_else(|| prompt_filename.trim_end_matches(".md").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentry_core::models::{AgentSpec, DetectedAgent, InstallMethod};
    use std::path::PathBuf;

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

    fn spec(id: &str, config_dir: &str, prompt_filename: &str) -> AgentSpec {
        AgentSpec {
            id: id.to_string(),
            name: id.to_string(),
            cli_binary: id.to_string(),
            config_dir: config_dir.to_string(),
            prompt_filename: prompt_filename.to_string(),
            prompt_format: PromptFormat::PlainMd,
            skills_dir_name: None,
            max_size: None,
            install_methods: vec![InstallMethod::Npm {
                package: id.to_string(),
            }],
        }
    }

    fn agent(spec: AgentSpec) -> DetectedAgent {
        DetectedAgent {
            spec,
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

    fn ctx(home: PathBuf, agents: Vec<DetectedAgent>) -> CheckContext {
        CheckContext {
            home_dir: home,
            agents,
            prompts: Vec::new(),
            version_lookup: None,
            binary_on_path: Vec::new(),
        }
    }

    #[test]
    fn missing_fires_when_prompt_file_absent() {
        let tmp = TempDir::new("agentry_audit_prompt_missing_file");
        std::fs::create_dir_all(tmp.path().join(".gemini")).unwrap();
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![agent(spec("gemini-cli", ".gemini", "GEMINI.md"))],
        ));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].check_id, "prompt.missing");
        assert_eq!(findings[0].severity, Severity::Warning);
        assert_eq!(findings[0].category, FindingCategory::PromptFile);
        assert!(findings[0].auto_fixable);
        assert!(!findings[0].message.is_empty());
        assert!(!findings[0].remediation.is_empty());
        match &findings[0].fix {
            Some(FixAction::SyncPrompt {
                prompt_id,
                agent_id,
            }) => {
                assert_eq!(prompt_id, "GEMINI");
                assert_eq!(agent_id, "gemini-cli");
            }
            other => panic!("expected SyncPrompt fix, got {:?}", other),
        }
    }

    #[test]
    fn missing_fires_when_prompt_dir_absent() {
        let tmp = TempDir::new("agentry_audit_prompt_missing_dir");
        std::fs::create_dir_all(tmp.path().join(".continue")).unwrap();
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![agent(spec("continue", ".continue", "prompts"))],
        ));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].check_id, "prompt.missing");
        match &findings[0].fix {
            Some(FixAction::SyncPrompt {
                prompt_id,
                agent_id,
            }) => {
                assert_eq!(prompt_id, "prompts");
                assert_eq!(agent_id, "continue");
            }
            other => panic!("expected SyncPrompt fix, got {:?}", other),
        }
    }

    #[test]
    fn directory_prompts_skip_content_checks() {
        let tmp = TempDir::new("agentry_audit_prompt_dir_skip");
        std::fs::create_dir_all(tmp.path().join(".continue").join("prompts")).unwrap();
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![agent(spec("continue", ".continue", "prompts"))],
        ));
        assert!(findings.is_empty());
    }

    #[test]
    fn empty_fires_for_whitespace_only_file() {
        let tmp = TempDir::new("agentry_audit_prompt_empty_ws");
        let dir = tmp.path().join(".claude");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("CLAUDE.md"), "  \n\t\n  ").unwrap();
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![agent(spec("claude-code", ".claude", "CLAUDE.md"))],
        ));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].check_id, "prompt.empty");
        assert_eq!(findings[0].severity, Severity::Warning);
        assert!(findings[0].auto_fixable);
    }

    #[test]
    fn empty_fires_for_zero_byte_file() {
        let tmp = TempDir::new("agentry_audit_prompt_empty_zero");
        let dir = tmp.path().join(".claude");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("CLAUDE.md"), "").unwrap();
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![agent(spec("claude-code", ".claude", "CLAUDE.md"))],
        ));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].check_id, "prompt.empty");
    }

    #[test]
    fn oversized_fires_strictly_above_limit() {
        let tmp = TempDir::new("agentry_audit_prompt_oversized_fire");
        let dir = tmp.path().join(".codex");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("AGENTS.md"), "a".repeat(32769)).unwrap();
        let mut codex = spec("codex", ".codex", "AGENTS.md");
        codex.max_size = Some(32768);
        let findings = run(&ctx(tmp.path().clone(), vec![agent(codex)]));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].check_id, "prompt.oversized");
        assert_eq!(findings[0].severity, Severity::Warning);
        assert!(!findings[0].auto_fixable);
        assert!(findings[0].fix.is_none());
        let evidence = findings[0].evidence.as_deref().unwrap_or_default();
        assert!(evidence.contains("size=32769"));
        assert!(evidence.contains("max_size=32768"));
    }

    #[test]
    fn oversized_skips_when_size_equals_limit() {
        let tmp = TempDir::new("agentry_audit_prompt_oversized_limit");
        let dir = tmp.path().join(".codex");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("AGENTS.md"), "a".repeat(32768)).unwrap();
        let mut codex = spec("codex", ".codex", "AGENTS.md");
        codex.max_size = Some(32768);
        let findings = run(&ctx(tmp.path().clone(), vec![agent(codex)]));
        assert!(findings.is_empty());
    }

    #[test]
    fn frontmatter_invalid_fires_when_unterminated() {
        let tmp = TempDir::new("agentry_audit_prompt_fm_unterminated");
        let dir = tmp.path().join(".opencode");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("AGENTS.md"),
            "---\nname: test\nbody without closing marker",
        )
        .unwrap();
        let mut opencode = spec("opencode", ".opencode", "AGENTS.md");
        opencode.prompt_format = PromptFormat::FrontmatterMd;
        let findings = run(&ctx(tmp.path().clone(), vec![agent(opencode)]));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].check_id, "prompt.frontmatter_invalid");
        assert_eq!(findings[0].severity, Severity::Warning);
        assert!(!findings[0].auto_fixable);
        assert!(findings[0].fix.is_none());
        assert!(!findings
            .iter()
            .any(|f| f.check_id == "prompt.format_mismatch"));
    }

    #[test]
    fn frontmatter_invalid_skips_empty_frontmatter_block() {
        let tmp = TempDir::new("agentry_audit_prompt_fm_empty_block");
        let dir = tmp.path().join(".opencode");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("AGENTS.md"), "---\n---\nbody").unwrap();
        let mut opencode = spec("opencode", ".opencode", "AGENTS.md");
        opencode.prompt_format = PromptFormat::FrontmatterMd;
        let findings = run(&ctx(tmp.path().clone(), vec![agent(opencode)]));
        assert!(findings.is_empty());
    }

    #[test]
    fn frontmatter_invalid_fires_on_malformed_yaml() {
        let tmp = TempDir::new("agentry_audit_prompt_fm_malformed");
        let dir = tmp.path().join(".opencode");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("AGENTS.md"),
            "---\nname: [unclosed\n---\n\nBody text",
        )
        .unwrap();
        let mut opencode = spec("opencode", ".opencode", "AGENTS.md");
        opencode.prompt_format = PromptFormat::FrontmatterMd;
        let findings = run(&ctx(tmp.path().clone(), vec![agent(opencode)]));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].check_id, "prompt.frontmatter_invalid");
    }

    #[test]
    fn format_mismatch_skips_leading_horizontal_rule() {
        let tmp = TempDir::new("agentry_audit_prompt_mismatch_hr");
        let dir = tmp.path().join(".claude");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("CLAUDE.md"),
            "---\nSection marker without closing dashes",
        )
        .unwrap();
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![agent(spec("claude-code", ".claude", "CLAUDE.md"))],
        ));
        assert!(findings.is_empty());
    }

    #[test]
    fn format_mismatch_fires_when_plain_file_declared_frontmatter() {
        let tmp = TempDir::new("agentry_audit_prompt_mismatch_plain");
        let dir = tmp.path().join(".opencode");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("AGENTS.md"), "# Plain markdown only").unwrap();
        let mut opencode = spec("opencode", ".opencode", "AGENTS.md");
        opencode.prompt_format = PromptFormat::FrontmatterMd;
        let findings = run(&ctx(tmp.path().clone(), vec![agent(opencode)]));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].check_id, "prompt.format_mismatch");
        assert_eq!(findings[0].severity, Severity::Info);
        assert!(findings[0].auto_fixable);
        match &findings[0].fix {
            Some(FixAction::SyncPrompt {
                prompt_id,
                agent_id,
            }) => {
                assert_eq!(prompt_id, "AGENTS");
                assert_eq!(agent_id, "opencode");
            }
            other => panic!("expected SyncPrompt fix, got {:?}", other),
        }
        let evidence = findings[0].evidence.as_deref().unwrap_or_default();
        assert!(evidence.contains("declared=Frontmatter+MD"));
        assert!(evidence.contains("detected=Plain Markdown"));
    }

    #[test]
    fn format_mismatch_fires_when_frontmatter_file_declared_plain() {
        let tmp = TempDir::new("agentry_audit_prompt_mismatch_frontmatter");
        let dir = tmp.path().join(".claude");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("CLAUDE.md"),
            "---\nname: gemini\n---\n\n# Body text",
        )
        .unwrap();
        let findings = run(&ctx(
            tmp.path().clone(),
            vec![agent(spec("claude-code", ".claude", "CLAUDE.md"))],
        ));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].check_id, "prompt.format_mismatch");
        assert_eq!(findings[0].severity, Severity::Info);
        let evidence = findings[0].evidence.as_deref().unwrap_or_default();
        assert!(evidence.contains("declared=Plain Markdown"));
        assert!(evidence.contains("detected=Frontmatter+MD"));
    }
}
