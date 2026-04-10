use std::io;

/// Errors that can occur in the agentry-agents crate.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    /// Failed to detect an agent with the given identifier.
    #[error("agent detection failed for {agent_id}: {reason}")]
    Detection { agent_id: String, reason: String },

    /// Failed to read an agent configuration file.
    #[error("failed to read agent config at {path}")]
    ConfigRead {
        path: String,
        #[source]
        source: io::Error,
    },

    /// General I/O error associated with a specific path.
    #[error("I/O error for {path}")]
    Io {
        path: String,
        #[source]
        source: io::Error,
    },
}
