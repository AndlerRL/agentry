use agentry_audit::engine::{build_context, run_audit};
use agentry_audit::report::AuditReport;

use crate::action::{ActionInput, HarnessError};
use crate::context::HarnessContext;

pub fn run_audit_input(
    ctx: &HarnessContext,
    input: &ActionInput,
) -> Result<AuditReport, HarnessError> {
    let ActionInput::AuditRun { agent_id } = input else {
        return Err(HarnessError::InvalidInput(
            "audit.run requires AuditRun input".to_string(),
        ));
    };
    let mut report = run_audit(&build_context(&ctx.home_dir, ctx.prompts.clone()));
    if let Some(agent_id) = agent_id {
        if !report
            .agents
            .iter()
            .any(|agent| &agent.agent_id == agent_id)
        {
            return Err(HarnessError::InvalidInput(format!(
                "unknown agent id '{agent_id}'"
            )));
        }
        report.agents.retain(|agent| &agent.agent_id == agent_id);
        report.global_findings.clear();
        recompute_summary(&mut report);
    }
    Ok(report)
}

fn recompute_summary(report: &mut AuditReport) {
    let findings: Vec<&agentry_audit::report::AuditFinding> = report
        .agents
        .iter()
        .flat_map(|agent| agent.findings.iter())
        .collect();
    let mut by_severity = std::collections::BTreeMap::new();
    let mut by_category = std::collections::BTreeMap::new();
    for finding in &findings {
        *by_severity.entry(finding.severity).or_insert(0) += 1;
        *by_category.entry(finding.category).or_insert(0) += 1;
    }
    report.summary = agentry_audit::report::AuditSummary {
        total_findings: findings.len(),
        by_severity,
        by_category,
        auto_fixable_count: findings.iter().filter(|f| f.auto_fixable).count(),
        healthy_agents: report
            .agents
            .iter()
            .filter(|agent| agent.grade == agentry_audit::report::HealthGrade::Healthy)
            .count(),
        degraded_agents: report
            .agents
            .iter()
            .filter(|agent| agent.grade == agentry_audit::report::HealthGrade::Degraded)
            .count(),
    };
}
