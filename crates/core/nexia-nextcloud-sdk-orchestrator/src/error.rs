//! Error types for the Nextcloud orchestrator runtime.

use std::fmt;
use std::path::PathBuf;

use crate::mount::MountId;

/// Result alias for fallible orchestrator operations.
pub type OrchestratorResult<T> = Result<T, OrchestratorError>;

/// Strongly-typed error variants emitted by the background orchestrator.
#[derive(Debug)]
pub enum OrchestratorError {
    /// The background worker thread has shut down or dropped the command channel.
    WorkerShutdown(String),

    /// A local filesystem I/O operation failed.
    Io {
        /// The filesystem path involved.
        path: PathBuf,
        /// Contextual description of the failed operation.
        context: String,
        /// Underlying I/O error.
        source: std::io::Error,
    },

    /// The requested mount identifier was not found.
    MountNotFound {
        /// The missing mount identifier.
        id: MountId,
    },

    /// A mount point with the specified identifier or local target already exists.
    MountAlreadyExists {
        /// Specific reason or identifier conflict.
        reason: String,
    },

    /// The specified account identifier was not found.
    AccountNotFound {
        /// The missing account ID.
        id: String,
    },

    /// A virtual filesystem tree operation failed.
    Vfs {
        /// Name of the VFS operation.
        operation: String,
        /// Source VFS error.
        source: nexia_nextcloud_sdk_vfs::VfsError,
    },

    /// A folder synchronization operation failed.
    Sync {
        /// Name of the sync operation.
        operation: String,
        /// Source sync error.
        source: nexia_nextcloud_sync::SyncError,
    },

    /// A filesystem watcher error occurred.
    Watcher {
        /// Contextual description of the watcher failure.
        context: String,
        /// Underlying error message.
        message: String,
    },

    /// An invalid remote or local path was provided.
    InvalidPath {
        /// Path that was rejected.
        path: String,
        /// Explanation why the path was invalid.
        reason: String,
    },

    /// A remote Nextcloud API or transport error occurred.
    Api {
        /// Context or name of the operation.
        operation: String,
        /// Underlying SDK core error.
        source: nexia_nextcloud_core::Error,
    },

    /// Serialization or deserialization failed.
    Json {
        /// Underlying serde error.
        source: serde_json::Error,
    },
}

impl fmt::Display for OrchestratorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WorkerShutdown(msg) => {
                write!(f, "orchestrator background worker has stopped: {msg}")
            }
            Self::Io {
                path,
                context,
                source,
            } => {
                write!(
                    f,
                    "filesystem I/O error at '{}' during '{context}': {source}",
                    path.display()
                )
            }
            Self::MountNotFound { id } => {
                write!(f, "mount point with ID '{id}' was not found")
            }
            Self::MountAlreadyExists { reason } => {
                write!(f, "mount point already exists: {reason}")
            }
            Self::AccountNotFound { id } => {
                write!(f, "account '{id}' was not found")
            }
            Self::Vfs { operation, source } => {
                write!(f, "virtual filesystem error during '{operation}': {source}")
            }
            Self::Sync { operation, source } => {
                write!(f, "synchronization error during '{operation}': {source}")
            }
            Self::Watcher { context, message } => {
                write!(f, "filesystem watcher error during '{context}': {message}")
            }
            Self::InvalidPath { path, reason } => {
                write!(f, "invalid path '{path}': {reason}")
            }
            Self::Api { operation, source } => {
                write!(f, "Nextcloud API error during '{operation}': {source}")
            }
            Self::Json { source } => {
                write!(f, "JSON serialization error: {source}")
            }
        }
    }
}

impl std::error::Error for OrchestratorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Vfs { source, .. } => Some(source),
            Self::Sync { source, .. } => Some(source),
            Self::Api { source, .. } => Some(source),
            Self::Json { source } => Some(source),
            Self::WorkerShutdown(_)
            | Self::MountNotFound { .. }
            | Self::MountAlreadyExists { .. }
            | Self::AccountNotFound { .. }
            | Self::Watcher { .. }
            | Self::InvalidPath { .. } => None,
        }
    }
}

impl From<nexia_nextcloud_core::Error> for OrchestratorError {
    fn from(source: nexia_nextcloud_core::Error) -> Self {
        Self::Api {
            operation: "transport execution".to_string(),
            source,
        }
    }
}

impl From<serde_json::Error> for OrchestratorError {
    fn from(source: serde_json::Error) -> Self {
        Self::Json { source }
    }
}
