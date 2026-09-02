use agentry_openclaw::discovery::{discover_workspaces, LobsterWorkflow, OpenClawWorkspace};
use agentry_openclaw::docs::validate_lobster;

use crate::engine::CheckContext;
use crate::report::{AuditFinding, FindingCategory, Severity};

pub fn run(ctx: &CheckContext) -> Vec<AuditFinding> {
    let Ok(workspaces) = discover_workspaces(&ctx.home_dir) else {
        return Vec::new();
    };
    let mut findings = Vec::new();
    for workspace in &workspaces {
        findings.extend(lobster_invalid(workspace));
        findings.extend(workspace_incomplete(workspace));
    }
    findings
}

fn lobster_invalid(workspace: &OpenClawWorkspace) -> Vec<AuditFinding> {
    workspace
        .lobster_workflows
        .iter()
        .filter_map(|workflow| lobster_finding(workspace, workflow))
        .collect()
}

fn lobster_finding(
    workspace: &OpenClawWorkspace,
    workflow: &LobsterWorkflow,
) -> Option<AuditFinding> {
    let problem = match validate_lobster(&workflow.path) {
        Ok(validation) if !validation.valid => validation.warnings.join("; "),
        Ok(_) => return None,
        Err(err) => err.to_string(),
    };
    Some(AuditFinding {
        check_id: "openclaw.lobster_invalid".to_string(),
        severity: Severity::Warning,
        category: FindingCategory::OpenClaw,
        agent_id: Some(workspace.id.clone()),
        message: format!(
            "Lobster workflow '{}' in workspace '{}' is invalid",
            workflow.name, workspace.name
        ),
        remediation: format!(
            "Fix the YAML in '{}' per the validation warnings",
            workflow.path.display()
        ),
        auto_fixable: false,
        fix: None,
        suggested_fix: None,
        evidence: Some(format!(
            "workflow={} path={} problem={}",
            workflow.name,
            workflow.path.display(),
            problem
        )),
    })
}

fn workspace_incomplete(workspace: &OpenClawWorkspace) -> Vec<AuditFinding> {
    let missing = missing_core_docs(workspace);
    if missing.is_empty() {
        return Vec::new();
    }
    let missing_list = missing.join(", ");
    vec![AuditFinding {
        check_id: "openclaw.workspace_incomplete".to_string(),
        severity: Severity::Info,
        category: FindingCategory::OpenClaw,
        agent_id: Some(workspace.id.clone()),
        message: format!(
            "Workspace '{}' is missing core docs: {}",
            workspace.name, missing_list
        ),
        remediation: format!(
            "Create the missing core docs in '{}': {}",
            workspace.workspace_path.display(),
            missing_list
        ),
        auto_fixable: false,
        fix: None,
        suggested_fix: None,
        evidence: Some(format!(
            "workspace={} path={} has_agents_md={} has_soul_md={}",
            workspace.id,
            workspace.workspace_path.display(),
            workspace.has_agents_md,
            workspace.has_soul_md
        )),
    }]
}

fn missing_core_docs(workspace: &OpenClawWorkspace) -> Vec<&'static str> {
    let mut missing = Vec::new();
    if !workspace.has_agents_md {
        missing.push("AGENTS.md");
    }
    if !workspace.has_soul_md {
        missing.push("SOUL.md");
    }
    missing
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

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

    fn ctx(home: PathBuf) -> CheckContext {
        CheckContext {
            home_dir: home,
            agents: Vec::new(),
            prompts: Vec::new(),
            version_lookup: None,
            binary_on_path: Vec::new(),
        }
    }

    fn write_config(home: &Path, content: &str) {
        let dir = home.join(".openclaw");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("openclaw.json"), content).unwrap();
    }

    fn workspace_config() -> &'static str {
        r#"{"agents": {"list": [{"id": "main", "workspace": "~/ws"}]}}"#
    }

    fn temp_workspace(tmp: &TempDir) -> PathBuf {
        let ws = tmp.path().join("ws");
        std::fs::create_dir_all(&ws).unwrap();
        ws
    }

    #[test]
    fn workspace_incomplete_fires_when_core_docs_missing() {
        let tmp = TempDir::new("agentry_audit_oc_incomplete_fires");
        let ws = temp_workspace(&tmp);
        std::fs::write(ws.join("AGENTS.md"), "# Agents").unwrap();
        write_config(tmp.path(), workspace_config());
        let findings = run(&ctx(tmp.path().clone()));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].check_id, "openclaw.workspace_incomplete");
        assert_eq!(findings[0].severity, Severity::Info);
        assert_eq!(findings[0].category, FindingCategory::OpenClaw);
        assert_eq!(findings[0].agent_id.as_deref(), Some("main"));
        assert!(!findings[0].auto_fixable);
        assert!(findings[0].fix.is_none());
        assert!(!findings[0].message.is_empty());
        assert!(!findings[0].remediation.is_empty());
        let message = &findings[0].message;
        assert!(message.contains("SOUL.md"));
        assert!(!message.contains("AGENTS.md is missing"));
        let evidence = findings[0].evidence.as_deref().unwrap_or_default();
        assert!(evidence.contains("has_agents_md=true"));
        assert!(evidence.contains("has_soul_md=false"));
    }

    #[test]
    fn workspace_incomplete_skips_complete_workspace() {
        let tmp = TempDir::new("agentry_audit_oc_incomplete_complete");
        let ws = temp_workspace(&tmp);
        std::fs::write(ws.join("AGENTS.md"), "# Agents").unwrap();
        std::fs::write(ws.join("SOUL.md"), "# Soul").unwrap();
        write_config(tmp.path(), workspace_config());
        let findings = run(&ctx(tmp.path().clone()));
        assert!(findings.is_empty());
    }

    #[test]
    fn lobster_invalid_fires_on_bad_workflow() {
        let tmp = TempDir::new("agentry_audit_oc_lobster_fires");
        let ws = temp_workspace(&tmp);
        std::fs::write(ws.join("AGENTS.md"), "# Agents").unwrap();
        std::fs::write(ws.join("SOUL.md"), "# Soul").unwrap();
        std::fs::write(ws.join("broken.lobster"), "just: some\nrandom: yaml\n").unwrap();
        write_config(tmp.path(), workspace_config());
        let findings = run(&ctx(tmp.path().clone()));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].check_id, "openclaw.lobster_invalid");
        assert_eq!(findings[0].severity, Severity::Warning);
        assert_eq!(findings[0].category, FindingCategory::OpenClaw);
        assert_eq!(findings[0].agent_id.as_deref(), Some("main"));
        assert!(!findings[0].auto_fixable);
        assert!(findings[0].fix.is_none());
        assert!(!findings[0].message.is_empty());
        assert!(!findings[0].remediation.is_empty());
        let evidence = findings[0].evidence.as_deref().unwrap_or_default();
        assert!(evidence.contains("workflow=broken"));
        assert!(evidence.contains("problem=Missing 'name' field"));
    }

    #[test]
    fn lobster_invalid_skips_valid_workflow() {
        let tmp = TempDir::new("agentry_audit_oc_lobster_valid");
        let ws = temp_workspace(&tmp);
        std::fs::write(ws.join("AGENTS.md"), "# Agents").unwrap();
        std::fs::write(ws.join("SOUL.md"), "# Soul").unwrap();
        let workflow = "name: deploy\nsteps:\n  - id: run\n    command: echo hi\n";
        std::fs::write(ws.join("good.lobster"), workflow).unwrap();
        write_config(tmp.path(), workspace_config());
        let findings = run(&ctx(tmp.path().clone()));
        assert!(findings.is_empty());
    }

    #[test]
    fn skips_gracefully_when_config_has_json5_comments() {
        let tmp = TempDir::new("agentry_audit_oc_json5_skip");
        let ws = temp_workspace(&tmp);
        std::fs::write(ws.join("AGENTS.md"), "# Agents").unwrap();
        write_config(
            tmp.path(),
            "{\n  // workspace entry\n  \"agents\": {\"list\": []}\n}",
        );
        let findings = run(&ctx(tmp.path().clone()));
        assert!(findings.is_empty());
    }

    #[test]
    fn skips_when_no_openclaw_config_exists() {
        let tmp = TempDir::new("agentry_audit_oc_noconfig_skip");
        temp_workspace(&tmp);
        let findings = run(&ctx(tmp.path().clone()));
        assert!(findings.is_empty());
    }
}
