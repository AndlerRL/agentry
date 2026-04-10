use std::io;

/// Errors that can occur in the agentry-core crate.
#[derive(Debug, thiserror::Error)]
pub enum AgentryCoreError {
    /// Failed to convert between formats (e.g. YAML <-> JSON <-> TOML).
    #[error("failed to convert format from {from} to {to}")]
    FormatConversion { from: String, to: String },

    /// Failed to discover configuration or workspace entries at the given path.
    #[error("discovery failed for path {path}")]
    Discovery {
        path: String,
        #[source]
        source: io::Error,
    },

    /// Failed to parse a file at the given path.
    #[error("failed to parse {path}: {reason}")]
    Parse { path: String, reason: String },

    /// General I/O error associated with a specific path.
    #[error("I/O error for {path}")]
    Io {
        path: String,
        #[source]
        source: io::Error,
    },
}
