use std::io;

/// Errors that can occur in the agentry-acp crate.
#[derive(Debug, thiserror::Error)]
pub enum AcpError {
    /// Failed to access the message queue at the given path.
    #[error("queue error at {path}")]
    Queue {
        path: String,
        #[source]
        source: io::Error,
    },

    /// Failed to serialize or deserialize a message.
    #[error("serialization failed: {reason}")]
    Serialization { reason: String },

    /// Failed to route a message between agents.
    #[error("routing failed from {from} to {to}: {reason}")]
    Routing {
        from: String,
        to: String,
        reason: String,
    },

    /// Failed during orchestration of agent communication.
    #[error("orchestration failed: {reason}")]
    Orchestration { reason: String },

    /// General I/O error associated with a specific path.
    #[error("I/O error for {path}")]
    Io {
        path: String,
        #[source]
        source: io::Error,
    },
}
