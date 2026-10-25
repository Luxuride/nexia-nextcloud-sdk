//! Point-in-time snapshot of the entire download queue and aggregate progress.

use serde::{Deserialize, Serialize};

use super::progress::format_binary_bytes;
use super::task::DownloadTask;

/// Complete snapshot of download queue state and aggregate progress across all tasks.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadQueueSnapshot {
    /// Total number of tasks tracked in the queue.
    pub total_tasks: usize,
    /// Number of tasks waiting in queue.
    pub pending_tasks: usize,
    /// Number of tasks actively downloading.
    pub active_tasks: usize,
    /// Number of successfully completed tasks.
    pub completed_tasks: usize,
    /// Number of failed tasks.
    pub failed_tasks: usize,
    /// Total expected byte volume across all tasks with known sizes.
    pub total_bytes_expected: u64,
    /// Total bytes transferred across all tasks so far.
    pub total_bytes_transferred: u64,
    /// List of all tasks with their current individual statuses and progress.
    pub tasks: Vec<DownloadTask>,
}

impl DownloadQueueSnapshot {
    /// Returns `true` if there are no tasks in the queue.
    pub const fn is_empty(&self) -> bool {
        self.total_tasks == 0
    }

    /// Returns `true` if there are no pending or active downloads remaining.
    pub const fn is_drained(&self) -> bool {
        self.pending_tasks == 0 && self.active_tasks == 0
    }

    /// Aggregate progress ratio between `0.0` and `1.0`, or `None` if expected bytes are 0.
    pub fn overall_fraction(&self) -> Option<f64> {
        if self.total_bytes_expected == 0 {
            if self.is_drained() && self.completed_tasks > 0 {
                Some(1.0)
            } else {
                None
            }
        } else {
            let frac = self.total_bytes_transferred as f64 / self.total_bytes_expected as f64;
            Some(frac.clamp(0.0, 1.0))
        }
    }

    /// Aggregate progress percentage between `0.0` and `100.0`.
    pub fn overall_percentage(&self) -> Option<f64> {
        self.overall_fraction().map(|f| f * 100.0)
    }

    /// Formats a human-readable progress summary (e.g. `"45.0 MB of 100.0 MB (45%)"`).
    pub fn formatted_overall_progress(&self) -> String {
        let transferred = format_binary_bytes(self.total_bytes_transferred);
        if self.total_bytes_expected > 0 {
            let expected = format_binary_bytes(self.total_bytes_expected);
            let pct = self.overall_percentage().unwrap_or(0.0);
            format!("{transferred} of {expected} ({pct:.0}%)")
        } else {
            format!("{transferred} transferred")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::download::task_id::DownloadTaskId;

    #[test]
    fn empty_snapshot() {
        let snap = DownloadQueueSnapshot::default();
        assert!(snap.is_empty());
        assert!(snap.is_drained());
        assert_eq!(snap.overall_fraction(), None);
        assert_eq!(snap.formatted_overall_progress(), "0 B transferred");
    }

    #[test]
    fn aggregate_progress_calculation() {
        let snap = DownloadQueueSnapshot {
            total_tasks: 2,
            pending_tasks: 1,
            active_tasks: 1,
            completed_tasks: 0,
            failed_tasks: 0,
            total_bytes_expected: 100 * 1024 * 1024,
            total_bytes_transferred: 50 * 1024 * 1024,
            tasks: vec![DownloadTask::new(
                DownloadTaskId::new(1),
                "/file.bin",
                "/tmp/file.bin",
            )],
        };

        assert!(!snap.is_drained());
        assert_eq!(snap.overall_fraction(), Some(0.5));
        assert_eq!(snap.overall_percentage(), Some(50.0));
        assert_eq!(
            snap.formatted_overall_progress(),
            "50.0 MB of 100.0 MB (50%)"
        );
    }
}
