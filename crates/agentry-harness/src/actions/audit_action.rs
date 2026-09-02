use crate::action::{
    ActionInput, ActionKind, ActionOutput, Confirmation, HarnessAction, HarnessError,
};
use crate::actions::audit::run_audit_input;
use crate::context::HarnessContext;
use crate::gate::GateTicket;

pub struct AuditRunAction;

impl HarnessAction for AuditRunAction {
    fn id(&self) -> &'static str {
        "audit.run"
    }

    fn kind(&self) -> ActionKind {
        ActionKind::Systematic
    }

    fn describe(&self, input: &ActionInput) -> String {
        match input {
            ActionInput::AuditRun { agent_id: None } => {
                "run the audit across all detected agents".to_string()
            }
            ActionInput::AuditRun {
                agent_id: Some(agent),
            } => format!("run the audit for agent '{agent}'"),
            _ => "audit.run requires AuditRun input".to_string(),
        }
    }

    fn confirmation(&self, input: &ActionInput) -> Confirmation {
        match input {
            ActionInput::AuditRun { agent_id: None } => Confirmation::None,
            ActionInput::AuditRun { agent_id: Some(_) } => Confirmation::Unsupported,
            _ => Confirmation::Unsupported,
        }
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
            let report = run_audit_input(ctx, &input)?;
            Ok(ActionOutput::AuditCompleted(report))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::ActionInput;

    fn temp_home(prefix: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("{}_{}", prefix, std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn id_kind_and_confirmation() {
        let action = AuditRunAction;
        assert_eq!(action.id(), "audit.run");
        assert_eq!(action.kind(), ActionKind::Systematic);
        assert_eq!(
            action.confirmation(&ActionInput::AuditRun { agent_id: None }),
            Confirmation::None
        );
        assert_eq!(
            action.confirmation(&ActionInput::AuditRun {
                agent_id: Some("claude-code".to_string())
            }),
            Confirmation::Unsupported
        );
    }

    #[test]
    fn describe_covers_inputs() {
        let action = AuditRunAction;
        assert_eq!(
            action.describe(&ActionInput::AuditRun { agent_id: None }),
            "run the audit across all detected agents"
        );
        assert_eq!(
            action.describe(&ActionInput::AuditRun {
                agent_id: Some("codex".to_string())
            }),
            "run the audit for agent 'codex'"
        );
    }

    #[tokio::test]
    async fn execute_runs_audit_and_returns_report() {
        let home = temp_home("agentry_test_audit_action_run");
        let ctx = HarnessContext::new(home.clone(), Vec::new(), Vec::new());
        let action = AuditRunAction;
        let ticket = GateTicket::new("audit.run".to_string(), "t".to_string());
        let output = action
            .execute(&ctx, ActionInput::AuditRun { agent_id: None }, &ticket)
            .await
            .unwrap();
        match output {
            ActionOutput::AuditCompleted(report) => {
                assert_eq!(report.schema_version, 1);
            }
            other => panic!("unexpected output: {other:?}"),
        }
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[tokio::test]
    async fn execute_agent_scoped_is_unsupported() {
        let home = temp_home("agentry_test_audit_action_scoped");
        let ctx = HarnessContext::new(home.clone(), Vec::new(), Vec::new());
        let action = AuditRunAction;
        let ticket = GateTicket::new("audit.run".to_string(), "t".to_string());
        let err = action
            .execute(
                &ctx,
                ActionInput::AuditRun {
                    agent_id: Some("claude-code".to_string()),
                },
                &ticket,
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("unknown agent id"));
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[tokio::test]
    async fn execute_refuses_wrong_input() {
        let home = temp_home("agentry_test_audit_action_wrong_input");
        let ctx = HarnessContext::new(home.clone(), Vec::new(), Vec::new());
        let action = AuditRunAction;
        let ticket = GateTicket::new("audit.run".to_string(), "t".to_string());
        let err = action
            .execute(&ctx, ActionInput::FixApplyAll, &ticket)
            .await
            .unwrap_err();
        assert!(matches!(err, HarnessError::InvalidInput(_)));
        std::fs::remove_dir_all(&home).unwrap();
    }
}
