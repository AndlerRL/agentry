use agentry_audit::fix::FixOutcome;
use agentry_audit::report::AuditReport;
use agentry_core::models::SyncMapping;
use serde::{Deserialize, Serialize};

use crate::context::HarnessContext;
use crate::gate::GateTicket;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    Systematic,
    Agentic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirmation {
    None,
    Single,
    PerItem,
    Unsupported,
}

#[derive(Debug)]
pub enum HarnessError {
    InvalidInput(String),
    ExecutionFailed(String),
    Unsupported(String),
}

impl std::fmt::Display for HarnessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HarnessError::InvalidInput(msg) => write!(f, "invalid input: {msg}"),
            HarnessError::ExecutionFailed(msg) => write!(f, "execution failed: {msg}"),
            HarnessError::Unsupported(msg) => write!(f, "unsupported: {msg}"),
        }
    }
}

impl std::error::Error for HarnessError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ActionInput {
    SyncExecute {
        prompt_id: Option<String>,
        #[serde(default)]
        mappings: Vec<SyncMapping>,
    },
    AuditRun {
        agent_id: Option<String>,
    },
    FixApply {
        check_id: String,
    },
    FixApplyAll,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum ActionOutput {
    SyncExecuted { applied: usize, skipped: usize },
    AuditCompleted(AuditReport),
    FixApplied(FixOutcome),
    FixAppliedAll { outcomes: Vec<FixOutcome> },
}

pub trait HarnessAction: Send + Sync {
    fn id(&self) -> &'static str;
    fn kind(&self) -> ActionKind;
    fn describe(&self, input: &ActionInput) -> String;
    fn confirmation(&self, input: &ActionInput) -> Confirmation;
    fn execute<'a>(
        &'a self,
        ctx: &'a HarnessContext,
        input: ActionInput,
        ticket: &'a GateTicket,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<ActionOutput, HarnessError>> + Send + 'a>,
    >;
}
