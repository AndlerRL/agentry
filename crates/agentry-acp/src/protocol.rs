use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Message types for the Agent Communication Protocol.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "payload")]
pub enum AcpMessage {
    /// Request a prompt completion from an agent.
    PromptRequest(PromptPayload),
    /// Response to a prompt request.
    PromptResponse(PromptResponsePayload),
    /// Look up skills for a task.
    SkillLookup(SkillLookupPayload),
    /// Result of a skill lookup.
    SkillLookupResult(SkillLookupResultPayload),
    /// Assign a task to an agent.
    TaskAssign(TaskAssignPayload),
    /// Result of a task assignment.
    TaskResult(TaskResultPayload),
    /// Trigger a workflow execution.
    WorkflowTrigger(WorkflowTriggerPayload),
    /// Status update for a running workflow.
    WorkflowStatus(WorkflowStatusPayload),
}

/// Prompt request/response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PromptPayload {
    pub id: String,
    pub from_agent: String,
    pub to_agent: String,
    pub prompt: String,
    pub context: Option<String>,
    pub priority: MessagePriority,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PromptResponsePayload {
    pub request_id: String,
    pub from_agent: String,
    pub to_agent: String,
    pub response: String,
    pub success: bool,
    pub timestamp: String,
}

/// Skill lookup payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillLookupPayload {
    pub id: String,
    pub from_agent: String,
    pub task_description: String,
    pub required_capabilities: Vec<String>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillLookupResultPayload {
    pub lookup_id: String,
    pub from_agent: String,
    pub matched_agents: Vec<AgentCapability>,
    pub timestamp: String,
}

/// Task assignment/result payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskAssignPayload {
    pub id: String,
    pub from_agent: String,
    pub to_agent: String,
    pub task_type: String,
    pub description: String,
    pub input: Option<String>,
    pub deadline: Option<String>,
    pub priority: MessagePriority,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskResultPayload {
    pub task_id: String,
    pub from_agent: String,
    pub to_agent: String,
    pub result: String,
    pub success: bool,
    pub duration_ms: Option<u64>,
    pub timestamp: String,
}

/// Workflow trigger/status payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkflowTriggerPayload {
    pub id: String,
    pub name: String,
    pub triggered_by: String,
    pub args: Option<serde_json::Value>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkflowStatusPayload {
    pub workflow_id: String,
    pub step_id: String,
    pub status: WorkflowStepStatus,
    pub output: Option<String>,
    pub timestamp: String,
}

/// Agent capability description.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentCapability {
    pub agent_id: String,
    pub agent_name: String,
    pub capabilities: Vec<String>,
    pub skills: Vec<String>,
    pub model: Option<String>,
}

/// Message priority level.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessagePriority {
    Low,
    Normal,
    High,
    Urgent,
}

/// Workflow step status.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum WorkflowStepStatus {
    Pending,
    Running,
    WaitingApproval,
    Approved,
    Rejected,
    Completed,
    Failed,
}

/// Queue directory structure under ~/.agents/acp/
pub fn acp_dir(home_dir: &Path) -> std::path::PathBuf {
    home_dir.join(".agents").join("acp")
}

pub fn queue_dir(home_dir: &Path) -> std::path::PathBuf {
    acp_dir(home_dir).join("queue")
}

pub fn inbox_dir(home_dir: &Path, agent_id: &str) -> std::path::PathBuf {
    acp_dir(home_dir).join("inbox").join(agent_id)
}

/// Initialize the ACP directory structure.
pub fn init_acp_dirs(home_dir: &Path) -> Result<()> {
    let dirs = [
        queue_dir(home_dir),
        acp_dir(home_dir).join("inbox"),
        acp_dir(home_dir).join("outbox"),
    ];
    for dir in &dirs {
        std::fs::create_dir_all(dir)?;
    }
    Ok(())
}

/// Write a message to the queue.
pub fn enqueue_message(home_dir: &Path, message: &AcpMessage) -> Result<String> {
    let q_dir = queue_dir(home_dir);
    std::fs::create_dir_all(&q_dir)?;

    let id = message_id(message);
    let path = q_dir.join(format!("{}.json", id));
    let content = serde_json::to_string_pretty(message)?;
    std::fs::write(&path, content)?;
    Ok(id)
}

/// Deliver a message to an agent's inbox.
pub fn deliver_to_inbox(home_dir: &Path, agent_id: &str, message: &AcpMessage) -> Result<String> {
    let inbox = inbox_dir(home_dir, agent_id);
    std::fs::create_dir_all(&inbox)?;

    let id = message_id(message);
    let path = inbox.join(format!("{}.json", id));
    let content = serde_json::to_string_pretty(message)?;
    std::fs::write(&path, content)?;
    Ok(id)
}

/// Read all messages from an agent's inbox.
pub fn read_inbox(home_dir: &Path, agent_id: &str) -> Result<Vec<AcpMessage>> {
    let inbox = inbox_dir(home_dir, agent_id);
    if !inbox.exists() {
        return Ok(Vec::new());
    }

    let mut messages = Vec::new();
    for entry in std::fs::read_dir(&inbox)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            let content = std::fs::read_to_string(&path)?;
            if let Ok(msg) = serde_json::from_str::<AcpMessage>(&content) {
                messages.push(msg);
            }
        }
    }

    // Sort by timestamp
    messages.sort_by(|a, b| a.timestamp().cmp(b.timestamp()));
    Ok(messages)
}

/// Read all messages from the queue.
pub fn read_queue(home_dir: &Path) -> Result<Vec<AcpMessage>> {
    let q_dir = queue_dir(home_dir);
    if !q_dir.exists() {
        return Ok(Vec::new());
    }

    let mut messages = Vec::new();
    for entry in std::fs::read_dir(&q_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            let content = std::fs::read_to_string(&path)?;
            if let Ok(msg) = serde_json::from_str::<AcpMessage>(&content) {
                messages.push(msg);
            }
        }
    }

    messages.sort_by(|a, b| a.timestamp().cmp(b.timestamp()));
    Ok(messages)
}

/// Remove a message from the queue by ID.
pub fn dequeue_message(home_dir: &Path, id: &str) -> Result<bool> {
    let path = queue_dir(home_dir).join(format!("{}.json", id));
    if path.exists() {
        std::fs::remove_file(&path)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Clear an agent's inbox.
pub fn clear_inbox(home_dir: &Path, agent_id: &str) -> Result<u32> {
    let inbox = inbox_dir(home_dir, agent_id);
    if !inbox.exists() {
        return Ok(0);
    }

    let mut count = 0u32;
    for entry in std::fs::read_dir(&inbox)? {
        let entry = entry?;
        if entry.path().extension().and_then(|e| e.to_str()) == Some("json") {
            std::fs::remove_file(entry.path())?;
            count += 1;
        }
    }
    Ok(count)
}

impl AcpMessage {
    /// Get the timestamp of this message.
    pub fn timestamp(&self) -> &str {
        match self {
            AcpMessage::PromptRequest(p) => &p.timestamp,
            AcpMessage::PromptResponse(p) => &p.timestamp,
            AcpMessage::SkillLookup(p) => &p.timestamp,
            AcpMessage::SkillLookupResult(p) => &p.timestamp,
            AcpMessage::TaskAssign(p) => &p.timestamp,
            AcpMessage::TaskResult(p) => &p.timestamp,
            AcpMessage::WorkflowTrigger(p) => &p.timestamp,
            AcpMessage::WorkflowStatus(p) => &p.timestamp,
        }
    }

    /// Get the source agent ID.
    pub fn from_agent(&self) -> &str {
        match self {
            AcpMessage::PromptRequest(p) => &p.from_agent,
            AcpMessage::PromptResponse(p) => &p.from_agent,
            AcpMessage::SkillLookup(p) => &p.from_agent,
            AcpMessage::SkillLookupResult(p) => &p.from_agent,
            AcpMessage::TaskAssign(p) => &p.from_agent,
            AcpMessage::TaskResult(p) => &p.from_agent,
            AcpMessage::WorkflowTrigger(p) => &p.triggered_by,
            AcpMessage::WorkflowStatus(_) => "",
        }
    }

    /// Get the message type name.
    pub fn type_name(&self) -> &'static str {
        match self {
            AcpMessage::PromptRequest(_) => "PromptRequest",
            AcpMessage::PromptResponse(_) => "PromptResponse",
            AcpMessage::SkillLookup(_) => "SkillLookup",
            AcpMessage::SkillLookupResult(_) => "SkillLookupResult",
            AcpMessage::TaskAssign(_) => "TaskAssign",
            AcpMessage::TaskResult(_) => "TaskResult",
            AcpMessage::WorkflowTrigger(_) => "WorkflowTrigger",
            AcpMessage::WorkflowStatus(_) => "WorkflowStatus",
        }
    }
}

/// Generate a unique message ID based on type and timestamp.
fn message_id(message: &AcpMessage) -> String {
    let ts = message.timestamp();
    let type_name = message.type_name();
    let hash = sha1::Sha1::digest(format!("{:?}{}", message, ts).as_bytes());
    format!("{}-{:x}", type_name.to_lowercase(), hash)
}

// Sha1 import
use sha1::Digest;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn home_dir() -> PathBuf {
        std::env::temp_dir().join("agentry_test_acp")
    }

    #[test]
    fn test_message_serialization() {
        let msg = AcpMessage::PromptRequest(PromptPayload {
            id: "test-1".to_string(),
            from_agent: "claude-code".to_string(),
            to_agent: "gemini-cli".to_string(),
            prompt: "Review this code".to_string(),
            context: Some("fn main() {}".to_string()),
            priority: MessagePriority::Normal,
            timestamp: "2026-04-09T12:00:00Z".to_string(),
        });

        let json = serde_json::to_string_pretty(&msg).unwrap();
        assert!(json.contains("PromptRequest"));
        assert!(json.contains("claude-code"));

        let deserialized: AcpMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn test_enqueue_and_read() {
        let home = home_dir();
        let _ = std::fs::remove_dir_all(&home);

        init_acp_dirs(&home).unwrap();

        let msg = AcpMessage::TaskAssign(TaskAssignPayload {
            id: "task-1".to_string(),
            from_agent: "agentry".to_string(),
            to_agent: "claude-code".to_string(),
            task_type: "code_review".to_string(),
            description: "Review the sync module".to_string(),
            input: None,
            deadline: None,
            priority: MessagePriority::High,
            timestamp: "2026-04-09T12:00:00Z".to_string(),
        });

        let id = enqueue_message(&home, &msg).unwrap();
        assert!(!id.is_empty());

        let queue = read_queue(&home).unwrap();
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].type_name(), "TaskAssign");

        let dequeued = dequeue_message(&home, &id).unwrap();
        assert!(dequeued);

        let queue_after = read_queue(&home).unwrap();
        assert!(queue_after.is_empty());

        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn test_deliver_and_read_inbox() {
        let home = home_dir();
        let _ = std::fs::remove_dir_all(&home);

        init_acp_dirs(&home).unwrap();

        let msg = AcpMessage::PromptResponse(PromptResponsePayload {
            request_id: "req-1".to_string(),
            from_agent: "gemini-cli".to_string(),
            to_agent: "claude-code".to_string(),
            response: "LGTM".to_string(),
            success: true,
            timestamp: "2026-04-09T12:01:00Z".to_string(),
        });

        deliver_to_inbox(&home, "claude-code", &msg).unwrap();

        let inbox = read_inbox(&home, "claude-code").unwrap();
        assert_eq!(inbox.len(), 1);

        let cleared = clear_inbox(&home, "claude-code").unwrap();
        assert_eq!(cleared, 1);

        let inbox_after = read_inbox(&home, "claude-code").unwrap();
        assert!(inbox_after.is_empty());

        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn test_workflow_trigger() {
        let msg = AcpMessage::WorkflowTrigger(WorkflowTriggerPayload {
            id: "wf-1".to_string(),
            name: "code-review-pipeline".to_string(),
            triggered_by: "agentry".to_string(),
            args: Some(serde_json::json!({"reviewer": "claude-code"})),
            timestamp: "2026-04-09T12:00:00Z".to_string(),
        });

        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: AcpMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, deserialized);
    }
}