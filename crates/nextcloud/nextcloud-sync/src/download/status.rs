//! Lifecycle status of a queued sync download item.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Current execution state of a queued download task.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DownloadStatus {
    /// In queue waiting for an available download slot.
    Pending,
    /// Actively receiving byte stream from the remote server.
    Downloading {
        /// Number of bytes received so far.
        transferred_bytes: u64,
        /// Expected total transfer size, if known.
        total_bytes: Option<u64>,
    },
    /// Finished successfully.
    Completed {
        /// Total bytes written to the local destination file.
        total_bytes: u64,
    },
    /// Failed due to a network, disk, or authorization error.
    Failed {
        /// Human-readable diagnostic description of the failure.
        error: String,
    },
    /// Temporarily paused by the user or transfer policy.
    Paused,
    /// Cancelled before completion.
    Cancelled,
}

impl DownloadStatus {
    /// Returns `true` if the task is waiting in queue.
    pub const fn is_pending(&self) -> bool {
        matches!(self, Self::Pending)
    }

    /// Returns `true` if the task is actively downloading.
    pub const fn is_downloading(&self) -> bool {
        matches!(self, Self::Downloading { .. })
    }

    /// Returns `true` if the download completed successfully.
    pub const fn is_completed(&self) -> bool {
        matches!(self, Self::Completed { .. })
    }

    /// Returns `true` if the download failed.
    pub const fn is_failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }

    /// Returns `true` if the download is paused.
    pub const fn is_paused(&self) -> bool {
        matches!(self, Self::Paused)
    }

    /// Returns `true` if the download was cancelled.
    pub const fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled)
    }

    /// Returns `true` if the download has reached a final state (completed, failed, or cancelled).
    pub const fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed { .. } | Self::Failed { .. } | Self::Cancelled
        )
    }

    /// User-facing label summarizing the state.
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Pending => "Queued",
            Self::Downloading { .. } => "Downloading",
            Self::Completed { .. } => "Completed",
            Self::Failed { .. } => "Failed",
            Self::Paused => "Paused",
            Self::Cancelled => "Cancelled",
        }
    }
}

impl fmt::Display for DownloadStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_predicates_and_labels() {
        let pending = DownloadStatus::Pending;
        assert!(pending.is_pending());
        assert!(!pending.is_terminal());
        assert_eq!(pending.to_string(), "Queued");

        let dl = DownloadStatus::Downloading {
            transferred_bytes: 512,
            total_bytes: Some(1024),
        };
        assert!(dl.is_downloading());
        assert!(!dl.is_terminal());
        assert_eq!(dl.to_string(), "Downloading");

        let done = DownloadStatus::Completed { total_bytes: 1024 };
        assert!(done.is_completed());
        assert!(done.is_terminal());

        let failed = DownloadStatus::Failed {
            error: "connection dropped".to_string(),
        };
        assert!(failed.is_failed());
        assert!(failed.is_terminal());
    }
}
