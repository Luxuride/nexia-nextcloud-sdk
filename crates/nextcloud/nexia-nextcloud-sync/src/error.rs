//! Error types for synchronization, reconciliation, and journal persistence.

use std::error::Error;
use std::fmt;
use std::path::PathBuf;

/// Errors produced during folder synchronization or journal operations.
#[derive(Debug)]
pub enum SyncError {
    /// Local filesystem I/O error with context path and explanation.
    LocalIo {
        /// Host disk path where I/O failed.
        path: PathBuf,
        /// Description of the operation that failed.
        context: String,
        /// Underlying standard I/O error.
        source: std::io::Error,
    },
    /// Journal serialization or deserialization failure.
    Serialization {
        /// Context description.
        context: String,
        /// Underlying serde error.
        source: serde_json::Error,
    },
    /// Invalid canonical or relative path format.
    InvalidPath(String),
    /// A requested item was not found.
    NotFound(String),
    /// Safety safeguard or reconciliation discrepancy.
    Reconciliation(String),
}

impl fmt::Display for SyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LocalIo {
                path,
                context,
                source,
            } => {
                write!(
                    f,
                    "local I/O error at `{}` during {context}: {source}",
                    path.display()
                )
            }
            Self::Serialization { context, source } => {
                write!(
                    f,
                    "sync journal serialization error during {context}: {source}"
                )
            }
            Self::InvalidPath(msg) => write!(f, "invalid sync path: {msg}"),
            Self::NotFound(path) => write!(f, "sync target not found: `{path}`"),
            Self::Reconciliation(msg) => write!(f, "sync reconciliation safeguard: {msg}"),
        }
    }
}

impl Error for SyncError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::LocalIo { source, .. } => Some(source),
            Self::Serialization { source, .. } => Some(source),
            Self::InvalidPath(_) | Self::NotFound(_) | Self::Reconciliation(_) => None,
        }
    }
}
