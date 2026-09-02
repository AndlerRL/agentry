use agentry_audit::fix::{
    apply_fix, default_allowlist, validate, validate_with_allowlist, FixOutcome,
};
use agentry_audit::report::{AuditFinding, FindingCategory, FixAction};

use crate::action::{ActionOutput, HarnessError};
use crate::context::HarnessContext;

pub fn finding_for_check(
    ctx: &HarnessContext,
    check_id: &str,
) -> Result<AuditFinding, HarnessError> {
    let report = ctx.report.as_ref().ok_or_else(|| {
        HarnessError::InvalidInput(
            "no audit report loaded; run audit.run before fix actions".to_string(),
        )
    })?;
    report
        .agents
        .iter()
        .flat_map(|agent| agent.findings.iter())
        .chain(report.global_findings.iter())
        .find(|finding| finding.check_id == check_id)
        .cloned()
        .ok_or_else(|| {
            HarnessError::InvalidInput(format!("check '{check_id}' not found in audit report"))
        })
}

pub fn apply_finding(
    ctx: &HarnessContext,
    finding: &AuditFinding,
) -> Result<ActionOutput, HarnessError> {
    let Some(fix) = finding.fix.as_ref().or(finding.suggested_fix.as_ref()) else {
        return Err(HarnessError::InvalidInput(format!(
            "finding {} has no fix action",
            finding.check_id
        )));
    };
    if finding.category == FindingCategory::Audited {
        let allowlist = default_allowlist(&ctx.home_dir);
        if let Err(reason) = validate_with_allowlist(fix, &ctx.home_dir, &allowlist) {
            return Err(HarnessError::ExecutionFailed(reason));
        }
    } else if let Err(reason) = validate(fix, &ctx.home_dir) {
        return Err(HarnessError::ExecutionFailed(reason));
    }
    let outcome: FixOutcome = apply_fix(finding, &ctx.home_dir);
    Ok(ActionOutput::FixApplied(outcome))
}

pub fn apply_all_fixable(ctx: &HarnessContext) -> Result<ActionOutput, HarnessError> {
    let report = ctx.report.as_ref().ok_or_else(|| {
        HarnessError::InvalidInput(
            "no audit report loaded; run audit.run before fix actions".to_string(),
        )
    })?;
    let fixable = agentry_audit::fix::fixable_findings(report);
    let mut outcomes = Vec::new();
    for finding in fixable {
        if let Some(fix) = &finding.fix {
            if finding.category == FindingCategory::Audited {
                let allowlist = default_allowlist(&ctx.home_dir);
                if let Err(reason) = validate_with_allowlist(fix, &ctx.home_dir, &allowlist) {
                    outcomes.push(FixOutcome {
                        check_id: finding.check_id.clone(),
                        agent_id: finding.agent_id.clone(),
                        success: false,
                        message: reason,
                    });
                    continue;
                }
            } else if let Err(reason) = validate(fix, &ctx.home_dir) {
                outcomes.push(FixOutcome {
                    check_id: finding.check_id.clone(),
                    agent_id: finding.agent_id.clone(),
                    success: false,
                    message: reason,
                });
                continue;
            }
        }
        outcomes.push(apply_fix(finding, &ctx.home_dir));
    }
    Ok(ActionOutput::FixAppliedAll { outcomes })
}

pub fn fix_description(fix: &FixAction) -> String {
    match fix {
        FixAction::ShellCommand {
            description,
            command,
        } => format!("{description}: {command}"),
        FixAction::FileWrite { path, .. } => format!("write {}", path.display()),
        FixAction::FileRemove { path } => format!("remove {}", path.display()),
        FixAction::SymlinkRecreate { path, target } => {
            format!("symlink {} -> {}", path.display(), target)
        }
        FixAction::SyncPrompt {
            prompt_id,
            agent_id,
        } => format!("sync prompt {prompt_id} for {agent_id}"),
    }
}
