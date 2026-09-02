use agentry_audit::engine::{build_context, run_audit};
use agentry_audit::report::AuditReport;

use crate::action::{ActionInput, HarnessError};
use crate::context::HarnessContext;

pub fn run_audit_input(
    ctx: &HarnessContext,
    input: &ActionInput,
) -> Result<AuditReport, HarnessError> {
    let ActionInput::AuditRun { agent_id: None } = input else {
        return Err(HarnessError::Unsupported(
            "audit.run supports only unscoped runs; agent-scoped audits are not supported"
                .to_string(),
        ));
    };
    Ok(run_audit(&build_context(
        &ctx.home_dir,
        ctx.prompts.clone(),
    )))
}
