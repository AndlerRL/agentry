use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::protocol::{AgentCapability, TaskAssignPayload};
use crate::router::route_prompt;

/// A step in a generated workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pipeline: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<serde_json::Value>,
}

/// A generated .lobster workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LobsterWorkflow {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<serde_json::Value>,
    pub steps: Vec<WorkflowStep>,
}

/// Task decomposition result.
#[derive(Debug, Clone)]
pub struct TaskDecomposition {
    pub subtasks: Vec<Subtask>,
    pub workflow: LobsterWorkflow,
}

/// A single subtask.
#[derive(Debug, Clone)]
pub struct Subtask {
    pub id: String,
    pub description: String,
    pub assigned_agent: String,
    pub task_type: String,
    pub depends_on: Vec<String>,
}

/// Decompose a high-level task into subtasks and generate a .lobster workflow.
pub fn decompose_task(
    task: &str,
    capabilities: &[AgentCapability],
) -> TaskDecomposition {
    // Determine task categories from the description
    let task_lower = task.to_lowercase();
    let mut subtasks = Vec::new();
    let mut steps = Vec::new();

    if task_lower.contains("review") || task_lower.contains("audit") {
        // Code review workflow: analyze → review → approve
        let reviewer = route_prompt(capabilities, "code_review", task)
            .map(|c| c.agent_id.clone())
            .unwrap_or_else(|| "claude-code".to_string());

        subtasks.push(Subtask {
            id: "analyze".to_string(),
            description: format!("Analyze the codebase for: {}", task),
            assigned_agent: reviewer.clone(),
            task_type: "analysis".to_string(),
            depends_on: vec![],
        });

        subtasks.push(Subtask {
            id: "review".to_string(),
            description: format!("Review findings for: {}", task),
            assigned_agent: reviewer,
            task_type: "review".to_string(),
            depends_on: vec!["analyze".to_string()],
        });

        steps.push(WorkflowStep {
            id: "analyze".to_string(),
            run: Some(format!("agentry task assign --type analysis --prompt \"{}\"", task)),
            pipeline: None,
            stdin: None,
            approval: None,
            when: None,
            env: None,
        });

        steps.push(WorkflowStep {
            id: "review".to_string(),
            run: Some(format!("agentry task assign --type review --prompt \"{}\"", task)),
            pipeline: None,
            stdin: Some("$analyze.stdout".to_string()),
            approval: Some("Review findings before proceeding?".to_string()),
            when: None,
            env: None,
        });

    } else if task_lower.contains("deploy") || task_lower.contains("release") {
        // Deploy workflow: build → test → approve → deploy
        let deployer = route_prompt(capabilities, "terminal", task)
            .map(|c| c.agent_id.clone())
            .unwrap_or_else(|| "warp".to_string());

        subtasks.push(Subtask {
            id: "build".to_string(),
            description: "Build the project".to_string(),
            assigned_agent: deployer.clone(),
            task_type: "build".to_string(),
            depends_on: vec![],
        });

        subtasks.push(Subtask {
            id: "test".to_string(),
            description: "Run tests".to_string(),
            assigned_agent: deployer.clone(),
            task_type: "test".to_string(),
            depends_on: vec!["build".to_string()],
        });

        subtasks.push(Subtask {
            id: "deploy".to_string(),
            description: "Deploy to production".to_string(),
            assigned_agent: deployer,
            task_type: "deploy".to_string(),
            depends_on: vec!["test".to_string()],
        });

        steps.push(WorkflowStep {
            id: "build".to_string(),
            run: Some("cargo build --release".to_string()),
            pipeline: None,
            stdin: None,
            approval: None,
            when: None,
            env: None,
        });

        steps.push(WorkflowStep {
            id: "test".to_string(),
            run: Some("cargo test".to_string()),
            pipeline: None,
            stdin: None,
            approval: None,
            when: None,
            env: None,
        });

        steps.push(WorkflowStep {
            id: "approve".to_string(),
            run: None,
            pipeline: None,
            stdin: None,
            approval: Some("Ready to deploy?".to_string()),
            when: Some("$test.stdout".to_string()),
            env: None,
        });

        steps.push(WorkflowStep {
            id: "deploy".to_string(),
            run: Some("cargo dist build".to_string()),
            pipeline: None,
            stdin: None,
            approval: None,
            when: Some("$approve.approved".to_string()),
            env: None,
        });

    } else if task_lower.contains("implement") || task_lower.contains("build") || task_lower.contains("create") {
        // Implementation workflow: plan → implement → test
        let coder = route_prompt(capabilities, "code_generation", task)
            .map(|c| c.agent_id.clone())
            .unwrap_or_else(|| "claude-code".to_string());

        subtasks.push(Subtask {
            id: "plan".to_string(),
            description: format!("Plan implementation for: {}", task),
            assigned_agent: coder.clone(),
            task_type: "planning".to_string(),
            depends_on: vec![],
        });

        subtasks.push(Subtask {
            id: "implement".to_string(),
            description: format!("Implement: {}", task),
            assigned_agent: coder.clone(),
            task_type: "code_generation".to_string(),
            depends_on: vec!["plan".to_string()],
        });

        subtasks.push(Subtask {
            id: "test".to_string(),
            description: "Verify implementation".to_string(),
            assigned_agent: coder,
            task_type: "testing".to_string(),
            depends_on: vec!["implement".to_string()],
        });

        steps.push(WorkflowStep {
            id: "plan".to_string(),
            run: Some(format!("agentry task assign --type planning --prompt \"{}\"", task)),
            pipeline: None,
            stdin: None,
            approval: None,
            when: None,
            env: None,
        });

        steps.push(WorkflowStep {
            id: "implement".to_string(),
            run: Some(format!("agentry task assign --type code_generation --prompt \"{}\"", task)),
            pipeline: None,
            stdin: Some("$plan.stdout".to_string()),
            approval: None,
            when: None,
            env: None,
        });

        steps.push(WorkflowStep {
            id: "test".to_string(),
            run: Some("cargo test".to_string()),
            pipeline: None,
            stdin: None,
            approval: None,
            when: None,
            env: None,
        });

    } else {
        // Generic single-step workflow
        let agent = route_prompt(capabilities, "general", task)
            .map(|c| c.agent_id.clone())
            .unwrap_or_else(|| "claude-code".to_string());

        subtasks.push(Subtask {
            id: "execute".to_string(),
            description: task.to_string(),
            assigned_agent: agent,
            task_type: "general".to_string(),
            depends_on: vec![],
        });

        steps.push(WorkflowStep {
            id: "execute".to_string(),
            run: Some(format!("agentry task assign --prompt \"{}\"", task)),
            pipeline: None,
            stdin: None,
            approval: None,
            when: None,
            env: None,
        });
    }

    let workflow_name = format!("agentry-{}", task_lower.split_whitespace().take(3).collect::<Vec<_>>().join("-"));

    TaskDecomposition {
        subtasks,
        workflow: LobsterWorkflow {
            name: workflow_name,
            args: Some(serde_json::json!({
                "task": { "default": task }
            })),
            steps,
        },
    }
}

/// Save a workflow to a .lobster file.
pub fn save_workflow(workflow: &LobsterWorkflow, path: &std::path::Path) -> Result<()> {
    let content = serde_yaml::to_string(workflow)
        .with_context(|| "Failed to serialize workflow to YAML")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    Ok(())
}

/// Generate TaskAssign messages from a decomposition.
pub fn decomposition_to_assignments(
    decomposition: &TaskDecomposition,
    from_agent: &str,
) -> Vec<TaskAssignPayload> {
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    decomposition
        .subtasks
        .iter()
        .map(|subtask| TaskAssignPayload {
            id: subtask.id.clone(),
            from_agent: from_agent.to_string(),
            to_agent: subtask.assigned_agent.clone(),
            task_type: subtask.task_type.clone(),
            description: subtask.description.clone(),
            input: None,
            deadline: None,
            priority: crate::protocol::MessagePriority::Normal,
            timestamp: now.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_capabilities() -> Vec<AgentCapability> {
        vec![
            AgentCapability {
                agent_id: "claude-code".to_string(),
                agent_name: "Claude Code".to_string(),
                capabilities: vec![
                    "code_generation".to_string(),
                    "code_review".to_string(),
                    "debugging".to_string(),
                ],
                skills: vec![],
                model: Some("2.1.50".to_string()),
            },
            AgentCapability {
                agent_id: "warp".to_string(),
                agent_name: "Warp".to_string(),
                capabilities: vec![
                    "terminal".to_string(),
                    "devops".to_string(),
                ],
                skills: vec![],
                model: None,
            },
        ]
    }

    #[test]
    fn test_decompose_review_task() {
        let caps = test_capabilities();
        let result = decompose_task("Review the authentication module", &caps);
        assert_eq!(result.subtasks.len(), 2);
        assert_eq!(result.subtasks[0].task_type, "analysis");
        assert_eq!(result.subtasks[1].task_type, "review");
        assert_eq!(result.workflow.steps.len(), 2);
        assert!(result.workflow.name.starts_with("agentry-"));
    }

    #[test]
    fn test_decompose_deploy_task() {
        let caps = test_capabilities();
        let result = decompose_task("Deploy to production", &caps);
        assert_eq!(result.subtasks.len(), 3);
        assert_eq!(result.workflow.steps.len(), 4); // build, test, approve, deploy
    }

    #[test]
    fn test_decompose_implement_task() {
        let caps = test_capabilities();
        let result = decompose_task("Implement user authentication", &caps);
        assert_eq!(result.subtasks.len(), 3);
        assert_eq!(result.subtasks[0].task_type, "planning");
        assert_eq!(result.subtasks[1].task_type, "code_generation");
    }

    #[test]
    fn test_save_workflow() {
        let tmp = std::env::temp_dir().join("agentry_test_workflow");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let workflow = LobsterWorkflow {
            name: "test-workflow".to_string(),
            args: None,
            steps: vec![WorkflowStep {
                id: "step1".to_string(),
                run: Some("echo hello".to_string()),
                pipeline: None,
                stdin: None,
                approval: None,
                when: None,
                env: None,
            }],
        };

        let path = tmp.join("test.lobster");
        save_workflow(&workflow, &path).unwrap();
        assert!(path.exists());

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("test-workflow"));
        assert!(content.contains("step1"));

        let _ = std::fs::remove_dir_all(&tmp);
    }
}