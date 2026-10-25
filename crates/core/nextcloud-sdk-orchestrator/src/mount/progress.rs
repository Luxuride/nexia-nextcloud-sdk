//! Real-time synchronization progress and transfer metrics for a mount point.

use serde::{Deserialize, Serialize};

use crate::mount::{MountId, MountStatus};

/// Snapshot of synchronization activity and file transfer counts for a mount point.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MountProgressSnapshot {
    /// Associated mount identifier.
    pub mount_id: MountId,
    /// Total items queued for processing in the current synchronization cycle.
    pub total_items: usize,
    /// Number of items successfully synchronized or downloaded.
    pub completed_items: usize,
    /// Number of items currently transferring or in-flight.
    pub in_progress_items: usize,
    /// Total estimated byte volume of all scheduled transfers.
    pub bytes_total: u64,
    /// Number of bytes successfully transferred so far.
    pub bytes_transferred: u64,
    /// Relative paths of files currently actively syncing or hydrating.
    pub active_items: Vec<String>,
    /// High-level runtime status of this mount.
    pub status: MountStatus,
}

impl MountProgressSnapshot {
    /// Creates a new progress snapshot for a mount point.
    #[must_use]
    pub fn new(mount_id: MountId, status: MountStatus) -> Self {
        Self {
            mount_id,
            total_items: 0,
            completed_items: 0,
            in_progress_items: 0,
            bytes_total: 0,
            bytes_transferred: 0,
            active_items: Vec::new(),
            status,
        }
    }
}
