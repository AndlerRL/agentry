use std::collections::BTreeMap;

use agentry_core::models::{SyncAction, SyncMapping, UnifiedPrompt};
use agentry_sync::executor::{check_sync_status, execute_sync};

use crate::action::{ActionInput, HarnessError};
use crate::context::HarnessContext;

pub struct SyncExecuteOutput {
    pub applied: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

pub fn execute_sync_input(
    ctx: &HarnessContext,
    input: &ActionInput,
) -> Result<SyncExecuteOutput, HarnessError> {
    let ActionInput::SyncExecute {
        prompt_id,
        mappings,
    } = input
    else {
        return Err(HarnessError::InvalidInput(
            "sync.execute requires SyncExecute input".to_string(),
        ));
    };

    if !mappings.is_empty() {
        return execute_explicit_mappings(ctx, mappings);
    }

    let Some(prompt_id) = prompt_id else {
        return Err(HarnessError::InvalidInput(
            "sync.execute requires either explicit mappings or a prompt_id".to_string(),
        ));
    };

    let prompt = find_prompt(ctx, prompt_id)?;
    let plan = agentry_sync::planner::plan_sync(prompt, &ctx.detected_agents, &ctx.home_dir);
    let checked = check_sync_status(prompt, &plan.mappings);
    let executable: Vec<SyncMapping> = checked
        .iter()
        .filter(|m| m.action != SyncAction::Skip)
        .cloned()
        .collect();
    Ok(summarize_results(execute_sync(prompt, &executable, false)))
}

fn execute_explicit_mappings(
    ctx: &HarnessContext,
    mappings: &[SyncMapping],
) -> Result<SyncExecuteOutput, HarnessError> {
    let mut grouped: BTreeMap<String, Vec<SyncMapping>> = BTreeMap::new();
    for mapping in mappings {
        grouped
            .entry(mapping.prompt_id.clone())
            .or_default()
            .push(mapping.clone());
    }
    let mut output = SyncExecuteOutput {
        applied: 0,
        skipped: 0,
        errors: Vec::new(),
    };
    for (prompt_id, group) in &grouped {
        let prompt = find_prompt(ctx, prompt_id)?;
        summarize_into(&mut output, execute_sync(prompt, group, false));
    }
    Ok(output)
}

fn find_prompt<'a>(
    ctx: &'a HarnessContext,
    prompt_id: &str,
) -> Result<&'a UnifiedPrompt, HarnessError> {
    ctx.prompts
        .iter()
        .find(|p| p.id == prompt_id || p.name == prompt_id)
        .ok_or_else(|| HarnessError::InvalidInput(format!("prompt '{prompt_id}' not found")))
}

fn summarize_results(results: Vec<agentry_sync::executor::SyncResult>) -> SyncExecuteOutput {
    let mut output = SyncExecuteOutput {
        applied: 0,
        skipped: 0,
        errors: Vec::new(),
    };
    summarize_into(&mut output, results);
    output
}

fn summarize_into(
    output: &mut SyncExecuteOutput,
    results: Vec<agentry_sync::executor::SyncResult>,
) {
    for result in results {
        if result.success {
            if result.mapping.action == SyncAction::Skip {
                output.skipped += 1;
            } else {
                output.applied += 1;
            }
        } else {
            output
                .errors
                .push(format!("{}: {}", result.mapping.agent_id, result.message));
        }
    }
}

pub fn finish(output: SyncExecuteOutput) -> Result<SyncExecuteOutput, HarnessError> {
    if output.errors.is_empty() {
        Ok(output)
    } else {
        Err(HarnessError::ExecutionFailed(output.errors.join("; ")))
    }
}
