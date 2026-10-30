//! Download queue management module.

use nexia_nextcloud_sync::{
    DownloadPriority, DownloadQueueSnapshot, DownloadTaskId, SyncDownloadQueue,
};
use std::path::PathBuf;
use std::sync::Arc;

/// High-level download queue coordinator for the orchestrator runtime.
#[derive(Clone, Debug)]
pub struct DownloadQueueCoordinator {
    queue: Arc<SyncDownloadQueue>,
}

impl DownloadQueueCoordinator {
    /// Creates a new download queue coordinator wrapping a thread-safe download queue.
    #[must_use]
    pub fn new() -> Self {
        Self {
            queue: Arc::new(SyncDownloadQueue::new()),
        }
    }

    /// Access the underlying sync download queue.
    #[must_use]
    pub fn queue(&self) -> &Arc<SyncDownloadQueue> {
        &self.queue
    }

    /// Enqueues a new remote file for download.
    pub async fn enqueue(
        &self,
        remote_path: impl Into<String>,
        local_destination: impl Into<PathBuf>,
        expected_bytes: Option<u64>,
        priority: DownloadPriority,
    ) -> DownloadTaskId {
        self.queue
            .enqueue(remote_path, local_destination, expected_bytes, priority)
            .await
    }

    /// Cancels an active or pending download task.
    pub async fn cancel(&self, id: DownloadTaskId) -> bool {
        self.queue.cancel(id).await
    }

    /// Pauses an active or pending download task.
    pub async fn pause(&self, id: DownloadTaskId) -> bool {
        self.queue.pause(id).await
    }

    /// Resumes a paused download task.
    pub async fn resume(&self, id: DownloadTaskId) -> bool {
        self.queue.resume(id).await
    }

    /// Obtains a current aggregate snapshot of queue statistics.
    pub async fn snapshot(&self) -> DownloadQueueSnapshot {
        self.queue.snapshot().await
    }
}

impl Default for DownloadQueueCoordinator {
    fn default() -> Self {
        Self::new()
    }
}
