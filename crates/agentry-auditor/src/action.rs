use std::path::PathBuf;

use agentry_audit::report::{AuditFinding, AuditReport, FindingCategory, Severity};
use agentry_harness::action::{
    ActionInput, ActionKind, ActionOutput, Confirmation, HarnessAction, HarnessError,
};
use agentry_harness::context::HarnessContext;
use agentry_harness::gate::GateTicket;
use agentry_harness::hosts::invoke::{invoke_headless, with_suspended_terminal, InvokeError};
use agentry_harness::hosts::{config_hosts, first_installed, host_by_id, resolve_hosts};

use crate::config::{load_config, AuditorConfig};
use crate::context::{package, prioritized_findings, AuditorContext};
use crate::parse::{parse_findings, ParseReport};
use crate::prompt::build_prompt;

pub struct AuditorReviewAction;

fn no_host_finding() -> AuditFinding {
    AuditFinding {
        check_id: "auditor.no_host".to_string(),
        severity: Severity::Info,
        category: FindingCategory::Audited,
        agent_id: None,
        message: "no supported agent CLI is installed; install one to enable LLM-assisted audit"
            .to_string(),
        remediation: "install a supported agent CLI (claude, codex, gemini, zai, or ollama)"
            .to_string(),
        auto_fixable: false,
        fix: None,
        suggested_fix: None,
        evidence: None,
    }
}

fn run_failed_finding(message: &str, stderr: &str) -> AuditFinding {
    let excerpt: String = stderr.chars().take(500).collect();
    AuditFinding {
        check_id: "auditor.run_failed".to_string(),
        severity: Severity::Info,
        category: FindingCategory::Audited,
        agent_id: None,
        message: message.to_string(),
        remediation: "check the host CLI configuration and retry".to_string(),
        auto_fixable: false,
        fix: None,
        suggested_fix: None,
        evidence: Some(excerpt),
    }
}

fn unparseable_finding(stdout: &str) -> AuditFinding {
    let excerpt: String = stdout.chars().take(200).collect();
    AuditFinding {
        check_id: "auditor.run_failed".to_string(),
        severity: Severity::Info,
        category: FindingCategory::Audited,
        agent_id: None,
        message: "host exited 0 but the review output yielded no parseable findings; treat as inconclusive"
            .to_string(),
        remediation: "check the host CLI configuration and retry".to_string(),
        auto_fixable: false,
        fix: None,
        suggested_fix: None,
        evidence: Some(excerpt),
    }
}

pub fn merge_findings(report: &mut AuditReport, findings: Vec<AuditFinding>) -> usize {
    let mut added = 0usize;
    for finding in findings {
        let existing = report
            .agents
            .iter()
            .flat_map(|agent| agent.findings.iter())
            .chain(report.global_findings.iter())
            .any(|f| f.check_id == finding.check_id && f.agent_id == finding.agent_id);
        if existing {
            continue;
        }
        match &finding.agent_id {
            Some(agent_id) => {
                if let Some(agent) = report.agents.iter_mut().find(|a| &a.agent_id == agent_id) {
                    agent.findings.push(finding);
                    added += 1;
                } else {
                    report.global_findings.push(finding);
                    added += 1;
                }
            }
            None => {
                report.global_findings.push(finding);
                added += 1;
            }
        }
    }
    if added > 0 {
        report.summary = agentry_audit::engine::build_summary(
            report
                .global_findings
                .iter()
                .chain(report.agents.iter().flat_map(|agent| agent.findings.iter())),
            &report.agents,
        );
    }
    added
}

fn skills_inventory(ctx: &HarnessContext) -> Vec<String> {
    let lockfile = agentry_skills::lockfile::read_lockfile(&ctx.home_dir).ok();
    lockfile
        .map(|lockfile| lockfile.skills.keys().cloned().collect())
        .unwrap_or_default()
}

fn excerpt_paths(ctx: &HarnessContext, focus: Option<&AuditFinding>) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = Vec::new();
    if let Some(focus) = focus {
        if let Some(evidence) = &focus.evidence {
            let path = PathBuf::from(evidence);
            if path.is_file() {
                paths.push(path);
            }
        }
    }
    for agent in &ctx.detected_agents {
        let config_dir = ctx.home_dir.join(&agent.spec.config_dir);
        if config_dir.is_dir() {
            paths.push(config_dir.join(&agent.spec.prompt_filename));
        }
    }
    paths
}

fn build_auditor_context(ctx: &HarnessContext, focus: Option<AuditFinding>) -> AuditorContext {
    let report = ctx.report.clone().unwrap_or_else(|| {
        agentry_audit::engine::run_audit(&agentry_audit::engine::build_context(
            &ctx.home_dir,
            ctx.prompts.clone(),
        ))
    });
    let paths = excerpt_paths(ctx, focus.as_ref());
    package(report, focus, &paths, skills_inventory(ctx))
}

fn resolve_host<'a>(
    _ctx: &'a HarnessContext,
    config: &AuditorConfig,
    hosts: &'a [agentry_harness::hosts::HostProfile],
) -> Option<&'a agentry_harness::hosts::HostProfile> {
    if let Some(host_cli) = &config.host_cli {
        if let Some(host) = host_by_id(hosts, host_cli) {
            if agentry_harness::hosts::is_installed(host) {
                return Some(host);
            }
        }
    }
    first_installed(hosts)
}

impl HarnessAction for AuditorReviewAction {
    fn id(&self) -> &'static str {
        "auditor.review"
    }

    fn kind(&self) -> ActionKind {
        ActionKind::Agentic
    }

    fn describe(&self, input: &ActionInput) -> String {
        match input {
            ActionInput::AuditorReview {
                focus_check_id: Some(check_id),
            } => format!("review audit findings with the host LLM (focus {check_id})"),
            ActionInput::AuditorReview {
                focus_check_id: None,
            } => "review audit findings with the host LLM".to_string(),
            _ => "auditor.review requires AuditorReview input".to_string(),
        }
    }

    fn confirmation(&self, _input: &ActionInput) -> Confirmation {
        Confirmation::Single
    }

    fn execute<'a>(
        &'a self,
        ctx: &'a HarnessContext,
        input: ActionInput,
        _ticket: &'a GateTicket,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<ActionOutput, HarnessError>> + Send + 'a>,
    > {
        Box::pin(async move {
            let ActionInput::AuditorReview { focus_check_id } = input else {
                return Err(HarnessError::InvalidInput(
                    "auditor.review requires AuditorReview input".to_string(),
                ));
            };
            let config = load_config(&ctx.home_dir);
            let hosts = resolve_hosts(&config_hosts(&ctx.home_dir));
            let Some(host) = resolve_host(ctx, &config, &hosts) else {
                let mut report = ctx.report.clone().unwrap_or_else(|| {
                    agentry_audit::engine::run_audit(&agentry_audit::engine::build_context(
                        &ctx.home_dir,
                        ctx.prompts.clone(),
                    ))
                });
                let added = merge_findings(&mut report, vec![no_host_finding()]);
                return Ok(ActionOutput::AuditorMerged { added, report });
            };
            let focus = focus_check_id.and_then(|check_id| {
                prioritized_findings(ctx.report.as_ref().unwrap_or(
                    &agentry_audit::engine::run_audit(&agentry_audit::engine::build_context(
                        &ctx.home_dir,
                        ctx.prompts.clone(),
                    )),
                ))
                .into_iter()
                .find(|finding| finding.check_id == check_id)
            });
            let auditor_ctx = build_auditor_context(ctx, focus);
            let prompt = build_prompt(&auditor_ctx, &ctx.home_dir.join(".agents").join("skills"));
            let model = config
                .model
                .clone()
                .or_else(|| ctx.config.local.model.clone());
            let result = with_suspended_terminal(|| {
                tokio::task::block_in_place(|| {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build();
                    match rt {
                        Ok(rt) => rt.block_on(invoke_headless(
                            host,
                            config.command_template.as_deref(),
                            model.as_deref(),
                            &prompt,
                            config.timeout_secs,
                        )),
                        Err(err) => Err(InvokeError::Io(err.to_string())),
                    }
                })
            });
            let mut report = ctx.report.clone().unwrap_or_else(|| {
                agentry_audit::engine::run_audit(&agentry_audit::engine::build_context(
                    &ctx.home_dir,
                    ctx.prompts.clone(),
                ))
            });
            let findings = match result {
                Ok(invoke_result) => {
                    match parse_findings(&invoke_result.stdout, &ctx.home_dir, config.max_findings)
                    {
                        ParseReport::Findings(findings) => findings,
                        ParseReport::Unparseable => {
                            vec![unparseable_finding(&invoke_result.stdout)]
                        }
                    }
                }
                Err(InvokeError::Exit { stderr, .. }) => {
                    vec![run_failed_finding("host invocation failed", &stderr)]
                }
                Err(err) => vec![run_failed_finding(&err.to_string(), &err.to_string())],
            };
            let added = merge_findings(&mut report, findings);
            Ok(ActionOutput::AuditorMerged { added, report })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentry_harness::hosts::HostProfile;

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

    fn audited(check_id: &str) -> AuditFinding {
        AuditFinding {
            check_id: check_id.to_string(),
            severity: Severity::Suggestion,
            category: FindingCategory::Audited,
            agent_id: None,
            message: "m".to_string(),
            remediation: "r".to_string(),
            auto_fixable: false,
            fix: None,
            suggested_fix: None,
            evidence: None,
        }
    }

    #[test]
    fn id_kind_and_confirmation() {
        let action = AuditorReviewAction;
        assert_eq!(action.id(), "auditor.review");
        assert_eq!(action.kind(), ActionKind::Agentic);
        assert_eq!(
            action.confirmation(&ActionInput::AuditorReview {
                focus_check_id: None
            }),
            Confirmation::Single
        );
    }

    #[test]
    fn describe_discloses_egress() {
        let action = AuditorReviewAction;
        let description = action.describe(&ActionInput::AuditorReview {
            focus_check_id: None,
        });
        assert!(description.contains("review audit findings"));
    }

    #[test]
    fn merge_findings_appends_and_dedupes() {
        let mut report = empty_report();
        let added = merge_findings(
            &mut report,
            vec![audited("auditor.a"), audited("auditor.b")],
        );
        assert_eq!(added, 2);
        assert_eq!(report.global_findings.len(), 2);
        let again = merge_findings(&mut report, vec![audited("auditor.a")]);
        assert_eq!(again, 0);
        assert_eq!(report.global_findings.len(), 2);
    }

    #[test]
    fn merge_findings_recomputes_summary() {
        let mut report = empty_report();
        merge_findings(&mut report, vec![audited("auditor.a")]);
        assert_eq!(report.summary.total_findings, 1);
        assert_eq!(report.summary.by_category[&FindingCategory::Audited], 1);
    }

    #[test]
    fn no_host_finding_is_info_not_error() {
        let finding = no_host_finding();
        assert_eq!(finding.check_id, "auditor.no_host");
        assert_eq!(finding.severity, Severity::Info);
        assert_eq!(finding.category, FindingCategory::Audited);
    }

    #[test]
    fn run_failed_finding_carries_stderr_excerpt() {
        let finding = run_failed_finding("boom", "error: kaboom");
        assert_eq!(finding.check_id, "auditor.run_failed");
        assert!(finding.evidence.as_deref().unwrap().contains("kaboom"));
    }

    #[test]
    fn unparseable_is_run_failed_with_evidence() {
        let finding = unparseable_finding(&"x".repeat(300));
        assert_eq!(finding.check_id, "auditor.run_failed");
        assert_eq!(finding.severity, Severity::Info);
        assert_eq!(finding.category, FindingCategory::Audited);
        assert_eq!(finding.evidence.as_deref().unwrap().chars().count(), 200);
    }

    fn fake_host(prefix: &str) -> std::path::PathBuf {
        let home = temp_home(prefix);
        let path = home.join("fake_host");
        std::fs::write(&path, "#!/bin/sh\ncat >/dev/null\ncat \"$1\"\nexit 0\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).unwrap();
        path
    }

    fn auditor_ctx(
        home: &std::path::Path,
        script: &std::path::Path,
        payload: &str,
    ) -> HarnessContext {
        std::fs::create_dir_all(home.join(".agents")).unwrap();
        let payload_path = home.join("payload.txt");
        std::fs::write(&payload_path, payload).unwrap();
        std::fs::write(
            home.join(".agents").join("agentry.toml"),
            format!(
                "[hosts]\npriority = [\"ollama\"]\n\n[hosts.ollama]\ndetect_binary = \"{}\"\n\n[auditor]\nhost_cli = \"ollama\"\ncommand_template = \"{} {}\"\n",
                script.display(),
                script.display(),
                payload_path.display(),
            ),
        )
        .unwrap();
        HarnessContext::new(home.to_path_buf(), Vec::new(), Vec::new())
    }

    #[tokio::test]
    async fn execute_without_host_returns_no_host_finding() {
        let home = temp_home("agentry_test_auditor_no_host");
        let config_path = home.join(".agents").join("agentry.toml");
        std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        let mut toml = String::from("[hosts]\npriority = [\"claude-code\", \"codex\", \"gemini-cli\", \"zai\", \"ollama\"]\n");
        for id in ["claude-code", "codex", "gemini-cli", "zai", "ollama"] {
            toml.push_str(&format!(
                "\n[hosts.{id}]\ndetect_binary = \"definitely-not-installed-xyz\"\n"
            ));
        }
        std::fs::write(&config_path, toml).unwrap();
        let ctx = HarnessContext::new(home.clone(), Vec::new(), Vec::new())
            .with_report(Some(empty_report()));
        let mut registry = agentry_harness::HarnessRegistry::new();
        registry.register(Box::new(AuditorReviewAction));
        let output = registry
            .invoke_confirmed(
                &ctx,
                "auditor.review",
                ActionInput::AuditorReview {
                    focus_check_id: None,
                },
            )
            .await
            .unwrap();
        match output {
            ActionOutput::AuditorMerged { added, report } => {
                assert_eq!(added, 1);
                assert!(report
                    .global_findings
                    .iter()
                    .any(|f| f.check_id == "auditor.no_host"));
            }
            other => panic!("unexpected output: {other:?}"),
        }
        std::fs::remove_dir_all(&home).unwrap();
    }

    async fn run_with_host(
        home: &std::path::Path,
        prefix: &str,
        payload: &str,
    ) -> (usize, AuditReport) {
        let script = fake_host(prefix);
        let ctx = auditor_ctx(home, &script, payload).with_report(Some(empty_report()));
        let mut registry = agentry_harness::HarnessRegistry::new();
        registry.register(Box::new(AuditorReviewAction));
        let output = registry
            .invoke_confirmed(
                &ctx,
                "auditor.review",
                ActionInput::AuditorReview {
                    focus_check_id: None,
                },
            )
            .await
            .unwrap();
        std::fs::remove_dir_all(&script.parent().unwrap()).unwrap();
        match output {
            ActionOutput::AuditorMerged { added, report } => (added, report),
            other => panic!("unexpected output: {other:?}"),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn exit_zero_garbage_stdout_yields_run_failed() {
        let home = temp_home("agentry_test_auditor_garbage");
        let (added, report) =
            run_with_host(&home, "agentry_test_auditor_host_garbage", "no json at all").await;
        assert_eq!(added, 1);
        let finding = report
            .global_findings
            .iter()
            .find(|f| f.check_id == "auditor.run_failed")
            .expect("run_failed finding");
        assert_eq!(finding.severity, Severity::Info);
        assert!(finding
            .evidence
            .as_deref()
            .unwrap()
            .contains("no json at all"));
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn exit_zero_valid_empty_array_is_all_clear() {
        let home = temp_home("agentry_test_auditor_all_clear");
        let (added, report) =
            run_with_host(&home, "agentry_test_auditor_host_clear", "verdict: []").await;
        assert_eq!(added, 0);
        assert!(!report
            .global_findings
            .iter()
            .any(|f| f.check_id == "auditor.run_failed"));
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn exit_zero_malformed_array_yields_run_failed() {
        let home = temp_home("agentry_test_auditor_malformed");
        let (added, report) = run_with_host(
            &home,
            "agentry_test_auditor_host_malformed",
            "[\"unterminated",
        )
        .await;
        assert_eq!(added, 1);
        assert!(report
            .global_findings
            .iter()
            .any(|f| f.check_id == "auditor.run_failed"));
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn resolve_host_prefers_configured_installed() {
        let home = temp_home("agentry_test_auditor_resolve");
        let ctx = HarnessContext::new(home.clone(), Vec::new(), Vec::new());
        let config = AuditorConfig {
            host_cli: Some("ollama".to_string()),
            ..Default::default()
        };
        let hosts = vec![
            HostProfile {
                id: "claude-code".to_string(),
                display_name: "Claude Code".to_string(),
                kind: agentry_harness::hosts::HostKind::AgentCli,
                detect_binary: "definitely-not-installed-xyz".to_string(),
                headless_command: Some("claude -p".to_string()),
                model_argument: None,
                transport: agentry_harness::hosts::Transport::Stdin,
            },
            HostProfile {
                id: "ollama".to_string(),
                display_name: "Ollama".to_string(),
                kind: agentry_harness::hosts::HostKind::LocalRuntime,
                detect_binary: "definitely-not-installed-ollama-xyz".to_string(),
                headless_command: Some("ollama run {model}".to_string()),
                model_argument: Some("{model}".to_string()),
                transport: agentry_harness::hosts::Transport::Stdin,
            },
        ];
        let resolved = resolve_host(&ctx, &config, &hosts);
        assert!(resolved.is_none());
        std::fs::remove_dir_all(&home).unwrap();
    }
}
