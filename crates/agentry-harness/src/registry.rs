use crate::action::{ActionInput, ActionOutput, Confirmation, HarnessAction, HarnessError};
use crate::context::HarnessContext;
use crate::gate::{assert_ticket_for, record_consent, GateTicket};

pub struct HarnessRegistry {
    actions: Vec<Box<dyn HarnessAction>>,
}

#[derive(Debug)]
pub struct PendingInvocation {
    pub action_id: &'static str,
    pub describe: String,
    pub confirmation: Confirmation,
}

impl HarnessRegistry {
    pub fn new() -> Self {
        Self {
            actions: Vec::new(),
        }
    }

    pub fn register(&mut self, action: Box<dyn HarnessAction>) {
        self.actions.push(action);
    }

    pub fn get(&self, action_id: &str) -> Option<&dyn HarnessAction> {
        self.actions
            .iter()
            .find(|action| action.id() == action_id)
            .map(|action| action.as_ref())
    }

    pub fn list(&self) -> Vec<(&'static str, crate::action::ActionKind)> {
        self.actions
            .iter()
            .map(|action| (action.id(), action.kind()))
            .collect()
    }

    pub fn prepare(
        &self,
        action_id: &str,
        input: &ActionInput,
    ) -> Result<PendingInvocation, HarnessError> {
        let action = self
            .get(action_id)
            .ok_or_else(|| HarnessError::InvalidInput(format!("unknown action '{action_id}'")))?;
        Ok(PendingInvocation {
            action_id: action.id(),
            describe: action.describe(input),
            confirmation: action.confirmation(input),
        })
    }

    pub async fn invoke_confirmed(
        &self,
        ctx: &HarnessContext,
        action_id: &str,
        input: ActionInput,
    ) -> Result<ActionOutput, HarnessError> {
        let action = self
            .get(action_id)
            .ok_or_else(|| HarnessError::InvalidInput(format!("unknown action '{action_id}'")))?;
        let consent_id = record_consent(&ctx.home_dir, action_id, "granted")?;
        let ticket = GateTicket::new(action_id.to_string(), consent_id);
        assert_ticket_for(&ticket, action.id(), &ctx.home_dir)?;
        action.execute(ctx, input, &ticket).await
    }
}

impl Default for HarnessRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{ActionKind, Confirmation};
    use crate::context::HarnessContext;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    struct RecordingAction {
        calls: Arc<AtomicUsize>,
    }

    impl HarnessAction for RecordingAction {
        fn id(&self) -> &'static str {
            "test.action"
        }

        fn kind(&self) -> ActionKind {
            ActionKind::Systematic
        }

        fn describe(&self, _input: &ActionInput) -> String {
            "test action".to_string()
        }

        fn confirmation(&self, _input: &ActionInput) -> Confirmation {
            Confirmation::Single
        }

        fn execute(
            &self,
            _ctx: &HarnessContext,
            _input: ActionInput,
            _ticket: &GateTicket,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<ActionOutput, HarnessError>> + Send + '_>,
        > {
            let calls = self.calls.clone();
            Box::pin(async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(ActionOutput::SyncExecuted {
                    applied: 1,
                    skipped: 0,
                })
            })
        }
    }

    fn temp_home(prefix: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("{}_{}", prefix, std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn fixture_registry() -> (HarnessRegistry, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        let mut registry = HarnessRegistry::new();
        registry.register(Box::new(RecordingAction {
            calls: calls.clone(),
        }));
        (registry, calls)
    }

    #[tokio::test]
    async fn prepare_returns_pending_without_side_effects() {
        let (registry, calls) = fixture_registry();
        let home = temp_home("agentry_test_reg_prepare");
        let ctx = HarnessContext::new(home.clone(), Vec::new(), Vec::new());
        let pending = registry
            .prepare("test.action", &ActionInput::FixApplyAll)
            .unwrap();
        assert_eq!(pending.action_id, "test.action");
        assert_eq!(pending.confirmation, Confirmation::Single);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(
            crate::gate::consent_path(&home).exists() == false,
            "prepare must not record consent"
        );
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[tokio::test]
    async fn prepare_fails_on_unknown_action() {
        let (registry, _) = fixture_registry();
        let home = temp_home("agentry_test_reg_unknown");
        let ctx = HarnessContext::new(home.clone(), Vec::new(), Vec::new());
        let err = registry
            .prepare("nope", &ActionInput::FixApplyAll)
            .unwrap_err();
        assert!(err.to_string().contains("unknown action"));
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[tokio::test]
    async fn invoke_confirmed_mints_ticket_and_executes() {
        let (registry, calls) = fixture_registry();
        let home = temp_home("agentry_test_reg_invoke");
        let ctx = HarnessContext::new(home.clone(), Vec::new(), Vec::new());
        let output = registry
            .invoke_confirmed(&ctx, "test.action", ActionInput::FixApplyAll)
            .await
            .unwrap();
        assert!(matches!(
            output,
            ActionOutput::SyncExecuted { applied: 1, .. }
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let consents = crate::gate::load_consents(&home).unwrap();
        assert_eq!(consents.len(), 1);
        assert_eq!(consents[0].action_id, "test.action");
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[tokio::test]
    async fn invoke_confirmed_fails_closed_on_unknown_action() {
        let (registry, _) = fixture_registry();
        let home = temp_home("agentry_test_reg_invoke_unknown");
        let ctx = HarnessContext::new(home.clone(), Vec::new(), Vec::new());
        let err = registry
            .invoke_confirmed(&ctx, "nope", ActionInput::FixApplyAll)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("unknown action"));
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn list_reports_registered_actions() {
        let (registry, _) = fixture_registry();
        let listed = registry.list();
        assert_eq!(listed, vec![("test.action", ActionKind::Systematic)]);
    }

    #[test]
    fn get_returns_registered_action() {
        let (registry, _) = fixture_registry();
        assert!(registry.get("test.action").is_some());
        assert!(registry.get("nope").is_none());
    }
}
