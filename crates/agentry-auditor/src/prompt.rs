use std::path::Path;

use agentry_audit::report::AuditReport;

use crate::context::AuditorContext;

pub fn build_prompt(ctx: &AuditorContext, skills_root: &Path) -> String {
    let mut parts = vec![
        crate::PROMPT_ASSET.to_string(),
        String::new(),
        "## Audit report".to_string(),
        String::new(),
    ];
    let report_json = serde_json::to_string_pretty(&ctx.report).unwrap_or_default();
    parts.push(report_json);
    if let Some(focus) = &ctx.focus {
        parts.push(String::new());
        parts.push("## Focus finding".to_string());
        parts.push(String::new());
        parts.push(serde_json::to_string_pretty(focus).unwrap_or_default());
    }
    if !ctx.excerpts.is_empty() {
        parts.push(String::new());
        parts.push("## File excerpts".to_string());
        for excerpt in &ctx.excerpts {
            parts.push(String::new());
            parts.push(format!("### {}", excerpt.path.display()));
            if excerpt.withheld {
                parts.push("(content withheld)".to_string());
            } else if let Some(content) = &excerpt.content {
                parts.push(content.clone());
            }
        }
    }
    if !ctx.skills_inventory.is_empty() {
        parts.push(String::new());
        parts.push("## Installed skills".to_string());
        parts.push(ctx.skills_inventory.join(", "));
    }
    parts.push(String::new());
    parts.push(format!(
        "## Skill files to load\n- {}\n- {}",
        skills_root.join("skill-creator").join("SKILL.md").display(),
        skills_root
            .join("context-engineering-collection")
            .join("SKILL.md")
            .display()
    ));
    parts.join("\n")
}

pub fn report_to_json(report: &AuditReport) -> String {
    serde_json::to_string(report).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::package;

    fn temp_home(prefix: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("{}_{}", prefix, std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn empty_report() -> AuditReport {
        use agentry_audit::report::AuditSummary;
        AuditReport {
            generated_at: chrono::Utc::now(),
            machine_id: "m".to_string(),
            agents: vec![],
            global_findings: vec![],
            summary: AuditSummary {
                total_findings: 0,
                by_severity: std::collections::BTreeMap::new(),
                by_category: std::collections::BTreeMap::new(),
                auto_fixable_count: 0,
                healthy_agents: 0,
                degraded_agents: 0,
            },
            schema_version: 2,
        }
    }

    #[test]
    fn prompt_contains_asset_and_report() {
        let home = temp_home("agentry_test_prompt_build");
        let ctx = package(empty_report(), None, &[], vec!["skill-creator".to_string()]);
        let prompt = build_prompt(&ctx, &home);
        assert!(prompt.contains("agentry-role: auditor"));
        assert!(prompt.contains("## Audit report"));
        assert!(prompt.contains("## Installed skills"));
        assert!(prompt.contains("skill-creator"));
        assert!(prompt.contains("context-engineering-collection"));
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn prompt_includes_focus_and_excerpts() {
        let home = temp_home("agentry_test_prompt_focus");
        let excerpt = home.join("config.md");
        std::fs::write(&excerpt, "body").unwrap();
        let ctx = package(empty_report(), None, &[excerpt], vec![]);
        let prompt = build_prompt(&ctx, &home);
        assert!(prompt.contains("## File excerpts"));
        assert!(prompt.contains("config.md"));
        std::fs::remove_dir_all(&home).unwrap();
    }
}
