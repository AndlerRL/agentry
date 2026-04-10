use std::io;

/// Errors that can occur in the agentry-openclaw crate.
#[derive(Debug, thiserror::Error)]
pub enum OpenClawError {
    /// Failed to parse an OpenClaw configuration file.
    #[error("failed to parse config at {path}: {reason}")]
    ConfigParse { path: String, reason: String },

    /// The requested workspace was not found.
    #[error("workspace not found: {id}")]
    WorkspaceNotFound { id: String },

    /// Failed to read a documentation file.
    #[error("failed to read document at {path}")]
    DocRead {
        path: String,
        #[source]
        source: io::Error,
    },

    /// Failed to write a documentation file.
    #[error("failed to write document at {path}")]
    DocWrite {
        path: String,
        #[source]
        source: io::Error,
    },

    /// A validation check failed.
    #[error("validation failed: {reason}")]
    Validation { reason: String },

    /// General I/O error associated with a specific path.
    #[error("I/O error for {path}")]
    Io {
        path: String,
        #[source]
        source: io::Error,
    },
}
