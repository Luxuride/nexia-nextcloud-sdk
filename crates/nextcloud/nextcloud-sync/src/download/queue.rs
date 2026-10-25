//! Thread-safe synchronization download queue managing scheduling, events, and live progress.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast, watch};

use super::events::DownloadQueueEvent;
use super::executor::DownloadExecutor;
use super::priority::DownloadPriority;
use super::progress::DownloadProgress;
use super::snapshot::DownloadQueueSnapshot;
use super::status::DownloadStatus;
use super::task::DownloadTask;
use super::task_id::DownloadTaskId;
use crate::error::SyncError;

const BROADCAST_CAPACITY: usize = 256;

struct QueueInner {
    tasks: HashMap<DownloadTaskId, DownloadTask>,
    insertion_order: Vec<DownloadTaskId>,
    next_id: u64,
}

impl QueueInner {
    fn build_snapshot(&self) -> DownloadQueueSnapshot {
        let mut pending_tasks = 0;
        let mut active_tasks = 0;
        let mut completed_tasks = 0;
        let mut failed_tasks = 0;
        let mut total_bytes_expected = 0u64;
        let mut total_bytes_transferred = 0u64;

        let mut tasks = Vec::with_capacity(self.insertion_order.len());
        for id in &self.insertion_order {
            if let Some(task) = self.tasks.get(id) {
                match &task.status {
                    DownloadStatus::Pending => pending_tasks += 1,
                    DownloadStatus::Downloading { .. } => active_tasks += 1,
                    DownloadStatus::Completed { .. } => completed_tasks += 1,
                    DownloadStatus::Failed { .. } => failed_tasks += 1,
                    _ => {}
                }

                if let Some(expected) = task.expected_size {
                    total_bytes_expected = total_bytes_expected.saturating_add(expected);
                } else if let DownloadStatus::Completed { total_bytes } = task.status {
                    total_bytes_expected = total_bytes_expected.saturating_add(total_bytes);
                }

                total_bytes_transferred =
                    total_bytes_transferred.saturating_add(task.transferred_bytes());
                tasks.push(task.clone());
            }
        }

        DownloadQueueSnapshot {
            total_tasks: tasks.len(),
            pending_tasks,
            active_tasks,
            completed_tasks,
            failed_tasks,
            total_bytes_expected,
            total_bytes_transferred,
            tasks,
        }
    }
}

/// Asynchronous, observable queue coordinating file downloads and tracking byte-level progress.
#[derive(Clone)]
pub struct SyncDownloadQueue {
    inner: Arc<RwLock<QueueInner>>,
    event_tx: broadcast::Sender<DownloadQueueEvent>,
    snapshot_tx: watch::Sender<DownloadQueueSnapshot>,
}

impl std::fmt::Debug for SyncDownloadQueue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncDownloadQueue").finish()
    }
}

impl Default for SyncDownloadQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncDownloadQueue {
    /// Constructs a new empty download queue.
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        let (snapshot_tx, _) = watch::channel(DownloadQueueSnapshot::default());

        Self {
            inner: Arc::new(RwLock::new(QueueInner {
                tasks: HashMap::new(),
                insertion_order: Vec::new(),
                next_id: 1,
            })),
            event_tx,
            snapshot_tx,
        }
    }

    /// Subscribes to discrete queue lifecycle events (enqueued, started, progress, completed, failed).
    pub fn subscribe_events(&self) -> broadcast::Receiver<DownloadQueueEvent> {
        self.event_tx.subscribe()
    }

    /// Subscribes to the latest complete [`DownloadQueueSnapshot`] via a watch channel.
    pub fn subscribe_snapshot(&self) -> watch::Receiver<DownloadQueueSnapshot> {
        self.snapshot_tx.subscribe()
    }

    fn notify_snapshot_update(&self, inner: &QueueInner) {
        let snap = inner.build_snapshot();
        let _ = self.snapshot_tx.send(snap);
    }

    /// Enqueues a new download operation and returns its unique task identifier.
    pub async fn enqueue(
        &self,
        remote_path: impl Into<String>,
        local_destination: impl Into<PathBuf>,
        expected_size: Option<u64>,
        priority: DownloadPriority,
    ) -> DownloadTaskId {
        let mut inner = self.inner.write().await;
        let id = DownloadTaskId::new(inner.next_id);
        inner.next_id += 1;

        let task = DownloadTask::new(id, remote_path, local_destination)
            .with_expected_size(expected_size)
            .with_priority(priority);

        inner.tasks.insert(id, task.clone());
        inner.insertion_order.push(id);
        let _ = self.event_tx.send(DownloadQueueEvent::Enqueued(task));
        self.notify_snapshot_update(&inner);

        id
    }

    /// Returns a point-in-time snapshot of the entire queue.
    pub async fn snapshot(&self) -> DownloadQueueSnapshot {
        let inner = self.inner.read().await;
        inner.build_snapshot()
    }

    /// Retrieves an individual task by its ID.
    pub async fn task(&self, id: DownloadTaskId) -> Option<DownloadTask> {
        let inner = self.inner.read().await;
        inner.tasks.get(&id).cloned()
    }

    /// Retrieves the current progress for a specific task.
    pub async fn task_progress(&self, id: DownloadTaskId) -> Option<DownloadProgress> {
        let inner = self.inner.read().await;
        inner.tasks.get(&id).map(DownloadTask::progress)
    }

    /// Returns a list of all currently active download progress snapshots.
    pub async fn active_downloads(&self) -> Vec<DownloadProgress> {
        let inner = self.inner.read().await;
        inner
            .tasks
            .values()
            .filter(|t| t.is_active())
            .map(DownloadTask::progress)
            .collect()
    }

    /// Cancels a queued or active download.
    pub async fn cancel(&self, id: DownloadTaskId) -> bool {
        let mut inner = self.inner.write().await;
        if let Some(task) = inner.tasks.get_mut(&id)
            && !task.is_terminal()
        {
            task.status = DownloadStatus::Cancelled;
            let _ = self
                .event_tx
                .send(DownloadQueueEvent::Cancelled { task_id: id });
            self.notify_snapshot_update(&inner);
            return true;
        }
        false
    }

    /// Pauses an active or pending download.
    pub async fn pause(&self, id: DownloadTaskId) -> bool {
        let mut inner = self.inner.write().await;
        if let Some(task) = inner.tasks.get_mut(&id)
            && (task.is_pending() || task.is_active())
        {
            task.status = DownloadStatus::Paused;
            let _ = self
                .event_tx
                .send(DownloadQueueEvent::Paused { task_id: id });
            self.notify_snapshot_update(&inner);
            return true;
        }
        false
    }

    /// Resumes a paused download back to pending.
    pub async fn resume(&self, id: DownloadTaskId) -> bool {
        let mut inner = self.inner.write().await;
        if let Some(task) = inner.tasks.get_mut(&id)
            && task.status.is_paused()
        {
            task.status = DownloadStatus::Pending;
            let _ = self
                .event_tx
                .send(DownloadQueueEvent::Resumed { task_id: id });
            self.notify_snapshot_update(&inner);
            return true;
        }
        false
    }

    /// Removes completed tasks from the queue history.
    pub async fn clear_completed(&self) {
        let mut inner = self.inner.write().await;
        inner.tasks.retain(|_, t| !t.is_completed());
        let active_keys: std::collections::HashSet<DownloadTaskId> =
            inner.tasks.keys().copied().collect();
        inner.insertion_order.retain(|id| active_keys.contains(id));
        self.notify_snapshot_update(&inner);
    }

    /// Updates incremental progress for an active download task and emits progress notifications.
    pub async fn update_progress(
        &self,
        id: DownloadTaskId,
        transferred_bytes: u64,
        total_bytes: Option<u64>,
        speed_bps: Option<u64>,
    ) {
        let mut inner = self.inner.write().await;
        if let Some(task) = inner.tasks.get_mut(&id) {
            task.status = DownloadStatus::Downloading {
                transferred_bytes,
                total_bytes,
            };
            task.speed_bps = speed_bps;
            let progress = task.progress();
            let _ = self.event_tx.send(DownloadQueueEvent::Progress(progress));
            self.notify_snapshot_update(&inner);
        }
    }

    /// Marks a task as actively downloading.
    pub async fn mark_started(&self, id: DownloadTaskId) {
        let mut inner = self.inner.write().await;
        if let Some(task) = inner.tasks.get_mut(&id) {
            let total = task.expected_size;
            task.status = DownloadStatus::Downloading {
                transferred_bytes: 0,
                total_bytes: total,
            };
            let remote = task.remote_path.clone();
            let _ = self.event_tx.send(DownloadQueueEvent::Started {
                task_id: id,
                remote_path: remote,
            });
            self.notify_snapshot_update(&inner);
        }
    }

    /// Marks a task as successfully completed.
    pub async fn mark_completed(&self, id: DownloadTaskId, total_bytes: u64) {
        let mut inner = self.inner.write().await;
        if let Some(task) = inner.tasks.get_mut(&id) {
            task.status = DownloadStatus::Completed { total_bytes };
            task.speed_bps = None;
            let remote = task.remote_path.clone();
            let _ = self.event_tx.send(DownloadQueueEvent::Completed {
                task_id: id,
                remote_path: remote,
                total_bytes,
            });
            self.notify_snapshot_update(&inner);
        }
    }

    /// Marks a task as failed.
    pub async fn mark_failed(&self, id: DownloadTaskId, error: impl Into<String>) {
        let mut inner = self.inner.write().await;
        let err_msg = error.into();
        if let Some(task) = inner.tasks.get_mut(&id) {
            task.status = DownloadStatus::Failed {
                error: err_msg.clone(),
            };
            task.speed_bps = None;
            let remote = task.remote_path.clone();
            let _ = self.event_tx.send(DownloadQueueEvent::Failed {
                task_id: id,
                remote_path: remote,
                error: err_msg,
            });
            self.notify_snapshot_update(&inner);
        }
    }

    /// Returns the next pending task with the highest priority and oldest arrival time.
    pub async fn next_pending_task(&self) -> Option<DownloadTask> {
        let inner = self.inner.read().await;
        inner
            .tasks
            .values()
            .filter(|t| t.is_pending())
            .max_by(|a, b| {
                a.priority
                    .cmp(&b.priority)
                    .then_with(|| b.created_at_epoch_ms.cmp(&a.created_at_epoch_ms))
            })
            .cloned()
    }

    /// Dispatches and processes the next pending task using `executor`.
    ///
    /// Returns `Ok(Some(task_id))` if a task was processed, or `Ok(None)` if no pending tasks remain.
    pub async fn process_next<E: DownloadExecutor>(
        &self,
        executor: &E,
    ) -> Result<Option<DownloadTaskId>, SyncError> {
        let task = match self.next_pending_task().await {
            Some(t) => t,
            None => return Ok(None),
        };

        let task_id = task.id;
        self.mark_started(task_id).await;

        let queue = self.clone();
        let progress_cb = Box::new(move |transferred: u64, total: Option<u64>| {
            let q = queue.clone();
            tokio::spawn(async move {
                q.update_progress(task_id, transferred, total, None).await;
            });
        });

        match executor
            .execute_download(&task.remote_path, &task.local_destination, progress_cb)
            .await
        {
            Ok(total_bytes) => {
                self.mark_completed(task_id, total_bytes).await;
                Ok(Some(task_id))
            }
            Err(err) => {
                let err_str = err.to_string();
                self.mark_failed(task_id, err_str).await;
                Err(err)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::download::executor::MockDownloadExecutor;

    #[tokio::test]
    async fn queue_enqueue_and_snapshot() {
        let queue = SyncDownloadQueue::new();
        let id1 = queue
            .enqueue(
                "/file1.bin",
                "/tmp/file1.bin",
                Some(1024),
                DownloadPriority::Normal,
            )
            .await;
        let id2 = queue
            .enqueue(
                "/urgent.zip",
                "/tmp/urgent.zip",
                Some(4096),
                DownloadPriority::Urgent,
            )
            .await;

        let snap = queue.snapshot().await;
        assert_eq!(snap.total_tasks, 2);
        assert_eq!(snap.pending_tasks, 2);
        assert_eq!(snap.total_bytes_expected, 5120);

        // Next pending task should be the urgent one
        let next = queue.next_pending_task().await.unwrap();
        assert_eq!(next.id, id2);

        // Process next using mock executor
        let executor = MockDownloadExecutor { total_bytes: 4096 };
        let res = queue.process_next(&executor).await.unwrap();
        assert_eq!(res, Some(id2));

        let snap2 = queue.snapshot().await;
        assert_eq!(snap2.completed_tasks, 1);
        assert_eq!(snap2.pending_tasks, 1);
        assert_eq!(snap2.total_bytes_transferred, 4096);

        // Cancel remaining task
        assert!(queue.cancel(id1).await);
        let snap3 = queue.snapshot().await;
        assert_eq!(snap3.pending_tasks, 0);
    }
}
