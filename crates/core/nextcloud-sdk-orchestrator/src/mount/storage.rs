//! Storage utilization and capacity statistics for a Nextcloud mount point.

use serde::{Deserialize, Serialize};

use crate::mount::MountId;

/// Disk usage breakdown and cache capacity for an individual mount point.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MountStorageUsage {
    /// Associated mount identifier.
    pub mount_id: MountId,
    /// Total bytes used by local files or hydrated placeholders on disk.
    pub local_bytes: u64,
    /// Count of locally hydrated files or local synced files.
    pub hydrated_files_count: usize,
    /// Total bytes used by shared thumbnail previews across the client cache.
    pub shared_thumbnail_bytes: u64,
}

impl MountStorageUsage {
    /// Creates a new storage usage report for a mount point.
    #[must_use]
    pub fn new(
        mount_id: MountId,
        local_bytes: u64,
        hydrated_files_count: usize,
        shared_thumbnail_bytes: u64,
    ) -> Self {
        Self {
            mount_id,
            local_bytes,
            hydrated_files_count,
            shared_thumbnail_bytes,
        }
    }
}
