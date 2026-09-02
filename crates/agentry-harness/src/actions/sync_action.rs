use crate::action::{
    ActionInput, ActionKind, ActionOutput, Confirmation, HarnessAction, HarnessError,
};
use crate::actions::sync::{execute_sync_input, finish};
use crate::context::HarnessContext;
use crate::gate::GateTicket;

pub struct SyncExecuteAction;

impl HarnessAction for SyncExecuteAction {
    fn id(&self) -> &'static str {
        "sync.execute"
    }

    fn kind(&self) -> ActionKind {
        ActionKind::Systematic
    }

    fn describe(&self, input: &ActionInput) -> String {
        match input {
            ActionInput::SyncExecute {
                prompt_id,
                mappings,
            } => {
                if mappings.is_empty() {
                    match prompt_id {
                        Some(id) => format!("sync prompt '{id}' to all mapped agents"),
                        None => "sync all prompts to all mapped agents".to_string(),
                    }
                } else if mappings.len() == 1 {
                    let mapping = &mappings[0];
                    format!(
                        "sync '{}' to {} ({})",
                        mapping.prompt_id,
                        mapping.agent_id,
                        mapping.destination.display()
                    )
                } else {
                    format!("execute {} sync mappings", mappings.len())
                }
            }
            _ => "sync.execute requires SyncExecute input".to_string(),
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
            let output = finish(execute_sync_input(ctx, &input)?)?;
            Ok(ActionOutput::SyncExecuted {
                applied: output.applied,
                skipped: output.skipped,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::ActionInput;
    use agentry_core::models::{AgentSpec, DetectedAgent, PromptFormat, SyncAction, SyncStatus};

    fn temp_home(prefix: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("{}_{}", prefix, std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn detected(agent_id: &str, config_dir: &str, prompt_filename: &str) -> DetectedAgent {
        DetectedAgent {
            spec: AgentSpec {
                id: agent_id.to_string(),
                name: agent_id.to_string(),
                cli_binary: agent_id.to_string(),
                config_dir: config_dir.to_string(),
                prompt_filename: prompt_filename.to_string(),
                prompt_format: PromptFormat::PlainMd,
                skills_dir_name: None,
                max_size: None,
                install_methods: vec![],
            },
            installed: true,
            version: None,
            config_dir_exists: true,
            prompt_file_exists: false,
            skills_dir: None,
            skills_symlink_pattern: None,
            installed_skills: vec![],
            detected_methods: vec![],
        }
    }

    fn canonical_prompt(
        home: &std::path::Path,
        name: &str,
        body: &str,
    ) -> agentry_core::models::UnifiedPrompt {
        let dir = home.join(".agents").join("prompts");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(format!("{name}.md")), body).unwrap();
        agentry_core::discovery::discover_prompts(home, &[])
            .into_iter()
            .find(|p| p.name == name)
            .unwrap()
    }

    fn mapping(
        prompt_id: &str,
        agent_id: &str,
        destination: std::path::PathBuf,
    ) -> agentry_core::models::SyncMapping {
        agentry_core::models::SyncMapping {
            prompt_id: prompt_id.to_string(),
            agent_id: agent_id.to_string(),
            destination,
            target_format: PromptFormat::PlainMd,
            action: SyncAction::Copy,
            status: SyncStatus::Missing,
        }
    }

    #[test]
    fn id_and_kind_and_confirmation() {
        let action = SyncExecuteAction;
        assert_eq!(action.id(), "sync.execute");
        assert_eq!(action.kind(), ActionKind::Systematic);
        assert_eq!(
            action.confirmation(&ActionInput::FixApplyAll),
            Confirmation::Single
        );
    }

    #[test]
    fn describe_covers_all_input_shapes() {
        let action = SyncExecuteAction;
        let plan_input = ActionInput::SyncExecute {
            prompt_id: Some("GEMINI".to_string()),
            mappings: Vec::new(),
        };
        assert_eq!(
            action.describe(&plan_input),
            "sync prompt 'GEMINI' to all mapped agents"
        );
        let explicit = ActionInput::SyncExecute {
            prompt_id: None,
            mappings: vec![mapping(
                "GEMINI",
                "gemini-cli",
                std::path::PathBuf::from("/x/GEMINI.md"),
            )],
        };
        assert_eq!(
            action.describe(&explicit),
            "sync 'GEMINI' to gemini-cli (/x/GEMINI.md)"
        );
        let all = ActionInput::SyncExecute {
            prompt_id: None,
            mappings: Vec::new(),
        };
        assert_eq!(
            action.describe(&all),
            "sync all prompts to all mapped agents"
        );
    }

    #[tokio::test]
    async fn execute_with_explicit_mappings_writes_destination() {
        let home = temp_home("agentry_test_sync_action_explicit");
        let prompt = canonical_prompt(&home, "GEMINI", "# GEMINI\n\nbody");
        let dest = home.join(".gemini").join("GEMINI.md");
        let ctx = HarnessContext::new(
            home.clone(),
            vec![detected("gemini-cli", ".gemini", "GEMINI.md")],
            vec![prompt],
        );
        let action = SyncExecuteAction;
        let ticket = GateTicket::new("sync.execute".to_string(), "t".to_string());
        let input = ActionInput::SyncExecute {
            prompt_id: None,
            mappings: vec![mapping("GEMINI", "gemini-cli", dest.clone())],
        };
        let output = action.execute(&ctx, input, &ticket).await.unwrap();
        match output {
            ActionOutput::SyncExecuted { applied, skipped } => {
                assert_eq!(applied, 1);
                assert_eq!(skipped, 0);
            }
            other => panic!("unexpected output: {other:?}"),
        }
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "# GEMINI\n\nbody");
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[tokio::test]
    async fn execute_from_prompt_id_plans_and_executes() {
        let home = temp_home("agentry_test_sync_action_plan");
        let prompt = canonical_prompt(&home, "GEMINI", "# GEMINI\n\nbody");
        let ctx = HarnessContext::new(
            home.clone(),
            vec![detected("gemini-cli", ".gemini", "GEMINI.md")],
            vec![prompt],
        );
        let action = SyncExecuteAction;
        let ticket = GateTicket::new("sync.execute".to_string(), "t".to_string());
        let input = ActionInput::SyncExecute {
            prompt_id: Some("GEMINI".to_string()),
            mappings: Vec::new(),
        };
        let output = action.execute(&ctx, input, &ticket).await.unwrap();
        match output {
            ActionOutput::SyncExecuted { applied, .. } => assert_eq!(applied, 1),
            other => panic!("unexpected output: {other:?}"),
        }
        assert!(home.join(".gemini").join("GEMINI.md").exists());
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[tokio::test]
    async fn execute_without_prompt_id_or_mappings_is_invalid() {
        let home = temp_home("agentry_test_sync_action_invalid");
        let ctx = HarnessContext::new(home.clone(), Vec::new(), Vec::new());
        let action = SyncExecuteAction;
        let ticket = GateTicket::new("sync.execute".to_string(), "t".to_string());
        let input = ActionInput::SyncExecute {
            prompt_id: None,
            mappings: Vec::new(),
        };
        let err = action.execute(&ctx, input, &ticket).await.unwrap_err();
        assert!(matches!(err, HarnessError::InvalidInput(_)));
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[tokio::test]
    async fn execute_unknown_prompt_is_invalid() {
        let home = temp_home("agentry_test_sync_action_unknown_prompt");
        let ctx = HarnessContext::new(home.clone(), Vec::new(), Vec::new());
        let action = SyncExecuteAction;
        let ticket = GateTicket::new("sync.execute".to_string(), "t".to_string());
        let input = ActionInput::SyncExecute {
            prompt_id: Some("nope".to_string()),
            mappings: Vec::new(),
        };
        let err = action.execute(&ctx, input, &ticket).await.unwrap_err();
        assert!(err.to_string().contains("not found"));
        std::fs::remove_dir_all(&home).unwrap();
    }
}
