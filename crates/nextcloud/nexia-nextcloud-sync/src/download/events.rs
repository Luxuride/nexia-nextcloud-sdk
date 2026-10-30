//! Asynchronous event notification types emitted by the download queue.

use serde::{Deserialize, Serialize};

use super::progress::DownloadProgress;
use super::task::DownloadTask;
use super::task_id::DownloadTaskId;

/// Event emitted when the state of the download queue or an individual download changes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum DownloadQueueEvent {
    /// A new download task was added to the queue.
    Enqueued(DownloadTask),
    /// A task started actively receiving data from the server.
    Started {
        /// Identifier of the started task.
        task_id: DownloadTaskId,
        /// Remote path being downloaded.
        remote_path: String,
    },
    /// Incremental byte transfer progress update for an active download.
    Progress(DownloadProgress),
    /// A task completed successfully.
    Completed {
        /// Identifier of the finished task.
        task_id: DownloadTaskId,
        /// Remote path that was downloaded.
        remote_path: String,
        /// Total bytes transferred and written to disk.
        total_bytes: u64,
    },
    /// A task failed with an error.
    Failed {
        /// Identifier of the failed task.
        task_id: DownloadTaskId,
        /// Remote path of the failed item.
        remote_path: String,
        /// Error message.
        error: String,
    },
    /// A task was cancelled.
    Cancelled {
        /// Identifier of the cancelled task.
        task_id: DownloadTaskId,
    },
    /// A task was paused.
    Paused {
        /// Identifier of the paused task.
        task_id: DownloadTaskId,
    },
    /// A task was resumed from paused state.
    Resumed {
        /// Identifier of the resumed task.
        task_id: DownloadTaskId,
    },
    /// All queued items have finished processing and the queue is currently idle.
    QueueDrained,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_variants() {
        let ev = DownloadQueueEvent::Started {
            task_id: DownloadTaskId::new(1),
            remote_path: "/Photos/cat.jpg".to_string(),
        };
        match ev {
            DownloadQueueEvent::Started { task_id, .. } => assert_eq!(task_id.raw(), 1),
            _ => panic!("unexpected event variant"),
        }
    }
}
