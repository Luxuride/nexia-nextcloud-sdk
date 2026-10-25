//! Representation of a queued file download task.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use super::priority::DownloadPriority;
use super::progress::DownloadProgress;
use super::status::DownloadStatus;
use super::task_id::DownloadTaskId;

/// A file download request tracked by the synchronization download queue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadTask {
    /// Unique identifier for this download task.
    pub id: DownloadTaskId,
    /// Canonical remote path on the Nextcloud instance (e.g. `"/Documents/contract.pdf"`).
    pub remote_path: String,
    /// Destination path on the local filesystem.
    pub local_destination: PathBuf,
    /// Expected byte size as reported by remote WebDAV metadata.
    pub expected_size: Option<u64>,
    /// Priority tier for queue scheduling.
    pub priority: DownloadPriority,
    /// Current execution state.
    pub status: DownloadStatus,
    /// Milliseconds since Unix epoch when this task was created.
    pub created_at_epoch_ms: u64,
    /// Most recent measured transfer speed in bytes per second.
    #[serde(default)]
    pub speed_bps: Option<u64>,
}

impl DownloadTask {
    /// Creates a new download task with default normal priority and pending status.
    pub fn new(
        id: DownloadTaskId,
        remote_path: impl Into<String>,
        local_destination: impl Into<PathBuf>,
    ) -> Self {
        let created_at_epoch_ms = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        Self {
            id,
            remote_path: remote_path.into(),
            local_destination: local_destination.into(),
            expected_size: None,
            priority: DownloadPriority::Normal,
            status: DownloadStatus::Pending,
            created_at_epoch_ms,
            speed_bps: None,
        }
    }

    /// Sets the expected file size in bytes.
    #[must_use]
    pub const fn with_expected_size(mut self, size: Option<u64>) -> Self {
        self.expected_size = size;
        self
    }

    /// Sets the scheduling priority.
    #[must_use]
    pub const fn with_priority(mut self, priority: DownloadPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Destination local path.
    pub fn local_destination(&self) -> &Path {
        &self.local_destination
    }

    /// Returns the current progress snapshot for this task.
    pub fn progress(&self) -> DownloadProgress {
        let (transferred, total) = match &self.status {
            DownloadStatus::Downloading {
                transferred_bytes,
                total_bytes,
            } => (*transferred_bytes, *total_bytes),
            DownloadStatus::Completed { total_bytes } => (*total_bytes, Some(*total_bytes)),
            _ => (0, self.expected_size),
        };

        DownloadProgress::new(
            self.id,
            &self.remote_path,
            transferred,
            total.or(self.expected_size),
            self.speed_bps,
        )
    }

    /// Number of bytes transferred so far.
    pub fn transferred_bytes(&self) -> u64 {
        match &self.status {
            DownloadStatus::Downloading {
                transferred_bytes, ..
            } => *transferred_bytes,
            DownloadStatus::Completed { total_bytes } => *total_bytes,
            _ => 0,
        }
    }

    /// Returns `true` if this task is waiting to start.
    pub const fn is_pending(&self) -> bool {
        self.status.is_pending()
    }

    /// Returns `true` if this task is currently receiving bytes.
    pub const fn is_active(&self) -> bool {
        self.status.is_downloading()
    }

    /// Returns `true` if this task has finished successfully.
    pub const fn is_completed(&self) -> bool {
        self.status.is_completed()
    }

    /// Returns `true` if this task failed.
    pub const fn is_failed(&self) -> bool {
        self.status.is_failed()
    }

    /// Returns `true` if this task is in a terminal state (completed, failed, or cancelled).
    pub const fn is_terminal(&self) -> bool {
        self.status.is_terminal()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_task_lifecycle_and_progress() {
        let mut task = DownloadTask::new(
            DownloadTaskId::new(10),
            "/Documents/spec.pdf",
            "/tmp/spec.pdf",
        )
        .with_expected_size(Some(2048))
        .with_priority(DownloadPriority::High);

        assert!(task.is_pending());
        assert_eq!(task.priority, DownloadPriority::High);
        assert_eq!(task.transferred_bytes(), 0);

        let prog = task.progress();
        assert_eq!(prog.total_bytes, Some(2048));
        assert_eq!(prog.transferred_bytes, 0);

        task.status = DownloadStatus::Downloading {
            transferred_bytes: 1024,
            total_bytes: Some(2048),
        };
        task.speed_bps = Some(512);

        assert!(task.is_active());
        assert_eq!(task.transferred_bytes(), 1024);
        let active_prog = task.progress();
        assert_eq!(active_prog.percentage(), Some(50.0));
        assert_eq!(active_prog.bytes_per_second, Some(512));

        task.status = DownloadStatus::Completed { total_bytes: 2048 };
        assert!(task.is_completed());
        assert!(task.is_terminal());
        assert_eq!(task.transferred_bytes(), 2048);
    }
}
