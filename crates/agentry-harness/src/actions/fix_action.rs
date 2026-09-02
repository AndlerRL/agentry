use crate::action::{
    ActionInput, ActionKind, ActionOutput, Confirmation, HarnessAction, HarnessError,
};
use crate::actions::fix::{apply_all_fixable, apply_finding, finding_for_check, fix_description};
use crate::context::HarnessContext;
use crate::gate::GateTicket;

pub struct FixApplyAction;

impl HarnessAction for FixApplyAction {
    fn id(&self) -> &'static str {
        "fix.apply"
    }

    fn kind(&self) -> ActionKind {
        ActionKind::Systematic
    }

    fn describe(&self, input: &ActionInput) -> String {
        match input {
            ActionInput::FixApply { check_id } => {
                format!("apply fix for {check_id}")
            }
            _ => "fix.apply requires FixApply input".to_string(),
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
            let ActionInput::FixApply { check_id } = input else {
                return Err(HarnessError::InvalidInput(
                    "fix.apply requires FixApply input".to_string(),
                ));
            };
            let finding = finding_for_check(ctx, &check_id)?;
            apply_finding(ctx, &finding)
        })
    }
}

pub struct FixApplyAllAction;

impl HarnessAction for FixApplyAllAction {
    fn id(&self) -> &'static str {
        "fix.apply_all"
    }

    fn kind(&self) -> ActionKind {
        ActionKind::Systematic
    }

    fn describe(&self, input: &ActionInput) -> String {
        match input {
            ActionInput::FixApplyAll => {
                "apply all auto-fixable findings from the loaded audit report".to_string()
            }
            _ => "fix.apply_all requires FixApplyAll input".to_string(),
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
            match input {
                ActionInput::FixApplyAll => apply_all_fixable(ctx),
                _ => Err(HarnessError::InvalidInput(
                    "fix.apply_all requires FixApplyAll input".to_string(),
                )),
            }
        })
    }
}

pub fn describe_finding_fix(finding: &agentry_audit::report::AuditFinding) -> String {
    match &finding.fix {
        Some(fix) => format!(
            "apply fix for {}: {}",
            finding.check_id,
            fix_description(fix)
        ),
        None => format!("finding {} has no fix action", finding.check_id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::ActionInput;
    use agentry_audit::report::{AuditFinding, FindingCategory, Severity};

    fn temp_home(prefix: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("{}_{}", prefix, std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn finding_with_fix(check_id: &str, fix: agentry_audit::report::FixAction) -> AuditFinding {
        AuditFinding {
            check_id: check_id.to_string(),
            severity: Severity::Warning,
            category: FindingCategory::Config,
            agent_id: None,
            message: format!("finding {check_id}"),
            remediation: "run the fix".to_string(),
            auto_fixable: true,
            fix: Some(fix),
            suggested_fix: None,
            evidence: None,
        }
    }

    fn report_with(findings: Vec<AuditFinding>) -> agentry_audit::report::AuditReport {
        use agentry_audit::report::{AuditReport, AuditSummary};
        use chrono::Utc;
        AuditReport {
            generated_at: Utc::now(),
            machine_id: "test-machine".to_string(),
            agents: Vec::new(),
            global_findings: findings,
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
    fn ids_kinds_and_confirmations() {
        let apply = FixApplyAction;
        let apply_all = FixApplyAllAction;
        assert_eq!(apply.id(), "fix.apply");
        assert_eq!(apply_all.id(), "fix.apply_all");
        assert_eq!(apply.kind(), ActionKind::Systematic);
        assert_eq!(apply_all.kind(), ActionKind::Systematic);
        assert_eq!(
            apply.confirmation(&ActionInput::FixApply {
                check_id: "x".to_string()
            }),
            Confirmation::Single
        );
        assert_eq!(
            apply_all.confirmation(&ActionInput::FixApplyAll),
            Confirmation::Single
        );
    }

    #[test]
    fn describe_covers_inputs() {
        let apply = FixApplyAction;
        let apply_all = FixApplyAllAction;
        assert_eq!(
            apply.describe(&ActionInput::FixApply {
                check_id: "skills_link".to_string()
            }),
            "apply fix for skills_link"
        );
        assert_eq!(
            apply_all.describe(&ActionInput::FixApplyAll),
            "apply all auto-fixable findings from the loaded audit report"
        );
    }

    #[tokio::test]
    async fn fix_apply_executes_finding_fix() {
        let home = temp_home("agentry_test_fix_action_apply");
        let path = home.join("stale.md");
        std::fs::write(&path, "x").unwrap();
        let finding = finding_with_fix(
            "orphan_cleanup",
            agentry_audit::report::FixAction::FileRemove { path: path.clone() },
        );
        let ctx = HarnessContext::new(home.clone(), Vec::new(), Vec::new())
            .with_report(Some(report_with(vec![finding])));
        let action = FixApplyAction;
        let ticket = GateTicket::new("fix.apply".to_string(), "t".to_string());
        let input = ActionInput::FixApply {
            check_id: "orphan_cleanup".to_string(),
        };
        let output = action.execute(&ctx, input, &ticket).await.unwrap();
        match output {
            ActionOutput::FixApplied(outcome) => {
                assert!(outcome.success, "{}", outcome.message);
                assert_eq!(outcome.check_id, "orphan_cleanup");
            }
            other => panic!("unexpected output: {other:?}"),
        }
        assert!(!path.exists());
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[tokio::test]
    async fn fix_apply_unknown_check_is_invalid() {
        let home = temp_home("agentry_test_fix_action_unknown");
        let ctx = HarnessContext::new(home.clone(), Vec::new(), Vec::new())
            .with_report(Some(report_with(Vec::new())));
        let action = FixApplyAction;
        let ticket = GateTicket::new("fix.apply".to_string(), "t".to_string());
        let input = ActionInput::FixApply {
            check_id: "nope".to_string(),
        };
        let err = action.execute(&ctx, input, &ticket).await.unwrap_err();
        assert!(err.to_string().contains("not found"));
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[tokio::test]
    async fn fix_apply_without_report_is_invalid() {
        let home = temp_home("agentry_test_fix_action_no_report");
        let ctx = HarnessContext::new(home.clone(), Vec::new(), Vec::new());
        let action = FixApplyAction;
        let ticket = GateTicket::new("fix.apply".to_string(), "t".to_string());
        let input = ActionInput::FixApply {
            check_id: "orphan_cleanup".to_string(),
        };
        let err = action.execute(&ctx, input, &ticket).await.unwrap_err();
        assert!(err.to_string().contains("no audit report loaded"));
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[tokio::test]
    async fn fix_apply_all_runs_sequential_apply_fix() {
        let home = temp_home("agentry_test_fix_action_all");
        let first = home.join("first.md");
        let second = home.join("second.md");
        let findings = vec![
            finding_with_fix(
                "write_first",
                agentry_audit::report::FixAction::FileWrite {
                    path: first.clone(),
                    content: "1".to_string(),
                },
            ),
            finding_with_fix(
                "write_second",
                agentry_audit::report::FixAction::FileWrite {
                    path: second.clone(),
                    content: "2".to_string(),
                },
            ),
        ];
        let ctx = HarnessContext::new(home.clone(), Vec::new(), Vec::new())
            .with_report(Some(report_with(findings)));
        let action = FixApplyAllAction;
        let ticket = GateTicket::new("fix.apply_all".to_string(), "t".to_string());
        let output = action
            .execute(&ctx, ActionInput::FixApplyAll, &ticket)
            .await
            .unwrap();
        match output {
            ActionOutput::FixAppliedAll { outcomes } => {
                assert_eq!(outcomes.len(), 2);
                assert!(outcomes.iter().all(|o| o.success));
            }
            other => panic!("unexpected output: {other:?}"),
        }
        assert_eq!(std::fs::read_to_string(&first).unwrap(), "1");
        assert_eq!(std::fs::read_to_string(&second).unwrap(), "2");
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[tokio::test]
    async fn fix_apply_all_without_report_is_invalid() {
        let home = temp_home("agentry_test_fix_action_all_no_report");
        let ctx = HarnessContext::new(home.clone(), Vec::new(), Vec::new());
        let action = FixApplyAllAction;
        let ticket = GateTicket::new("fix.apply_all".to_string(), "t".to_string());
        let err = action
            .execute(&ctx, ActionInput::FixApplyAll, &ticket)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("no audit report loaded"));
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[tokio::test]
    async fn fix_apply_all_refuses_wrong_input() {
        let home = temp_home("agentry_test_fix_action_all_wrong_input");
        let ctx = HarnessContext::new(home.clone(), Vec::new(), Vec::new());
        let action = FixApplyAllAction;
        let ticket = GateTicket::new("fix.apply_all".to_string(), "t".to_string());
        let err = action
            .execute(&ctx, ActionInput::FixApplyAll, &ticket)
            .await;
        let _ = err;
        let err = action
            .execute(
                &ctx,
                ActionInput::FixApply {
                    check_id: "x".to_string(),
                },
                &ticket,
            )
            .await
            .unwrap_err();
        assert!(matches!(err, HarnessError::InvalidInput(_)));
        std::fs::remove_dir_all(&home).unwrap();
    }
}
