use std::io;

/// Errors that can occur in the agentry-sync crate.
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    /// Failed to build or execute a sync plan.
    #[error("sync plan failed: {reason}")]
    Plan { reason: String },

    /// Failed to copy a file from source to destination.
    #[error("failed to copy {src} to {dest}: {reason}")]
    CopyFailed {
        src: String,
        dest: String,
        reason: String,
    },

    /// Failed to convert between configuration formats.
    #[error("failed to convert format from {from} to {to}: {reason}")]
    FormatConversion {
        from: String,
        to: String,
        reason: String,
    },

    /// General I/O error associated with a specific path.
    #[error("I/O error for {path}")]
    Io {
        path: String,
        #[source]
        source: io::Error,
    },
}
