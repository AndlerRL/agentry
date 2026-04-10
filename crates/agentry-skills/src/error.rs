use std::io;

/// Errors that can occur in the agentry-skills crate.
#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    /// Failed to read or write the lockfile.
    #[error("lockfile error at {path}")]
    Lockfile {
        path: String,
        #[source]
        source: io::Error,
    },

    /// Failed to parse the lockfile contents.
    #[error("failed to parse lockfile at {path}: {reason}")]
    LockfileParse { path: String, reason: String },

    /// Failed to clone a git repository.
    #[error("failed to clone repository {repo}: {reason}")]
    GitClone { repo: String, reason: String },

    /// Failed to install a skill.
    #[error("failed to install skill {skill}: {reason}")]
    Install { skill: String, reason: String },

    /// The requested skill is not installed.
    #[error("skill not installed: {skill}")]
    NotInstalled { skill: String },

    /// Failed to compute a file hash.
    #[error("hash computation failed for {path}: {reason}")]
    HashFailed { path: String, reason: String },

    /// General I/O error associated with a specific path.
    #[error("I/O error for {path}")]
    Io {
        path: String,
        #[source]
        source: io::Error,
    },
}
