//! Orchestrator lifecycle and synchronization events.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::mount::{MountDescriptor, MountId, MountStatus, SyncFileStatus};
use nexia_nextcloud_sync::DownloadProgress;

/// Reactive event broadcast by the orchestrator worker thread to frontends, UIs, and watchers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum OrchestratorEvent {
    /// A new mount point was configured and registered.
    MountAdded(MountDescriptor),

    /// A mount point was unmounted and removed.
    MountRemoved(MountId),

    /// Operational status of a mount point transitioned.
    MountStatusChanged {
        /// Identifier of the mount.
        id: MountId,
        /// New status of the mount.
        status: MountStatus,
    },

    /// The synchronization status of a file or folder inside a mount changed.
    FileStatusChanged {
        /// Associated mount identifier.
        mount_id: MountId,
        /// Relative path within the mount.
        path: String,
        /// Updated synchronization status and icon descriptor.
        status: SyncFileStatus,
    },

    /// A virtual file was downloaded and hydrated into local storage.
    FileHydrated {
        /// Associated mount identifier.
        mount_id: MountId,
        /// Remote WebDAV path of the file.
        remote_path: String,
        /// Local filesystem path where the file is now available.
        local_path: PathBuf,
    },

    /// A hydrated file was released back to a lightweight metadata placeholder.
    FileDehydrated {
        /// Associated mount identifier.
        mount_id: MountId,
        /// Remote WebDAV path of the dehydrated file.
        remote_path: String,
    },

    /// A folder synchronization cycle commenced.
    SyncCycleStarted {
        /// Mount identifier running the cycle.
        mount_id: MountId,
    },

    /// A folder synchronization cycle completed.
    SyncCycleCompleted {
        /// Mount identifier that finished synchronizing.
        mount_id: MountId,
        /// Total number of items processed during this cycle.
        items_processed: u64,
    },

    /// The virtual tree structure for an account changed (new folder, deletion, rename).
    VfsTreeChanged {
        /// Associated account ID.
        account_id: String,
        /// Root path of the modified subtree.
        path: String,
    },

    /// The active account profile was switched.
    AccountChanged {
        /// Newly active account ID.
        active_account_id: String,
    },

    /// Global synchronization has been paused across all mounts.
    GlobalSyncPaused,

    /// Global synchronization has been resumed across all mounts.
    GlobalSyncResumed,

    /// Progress update emitted by a queued file download.
    DownloadProgress(DownloadProgress),

    /// Live progress update for a mount point.
    MountProgress(crate::mount::MountProgressSnapshot),
}
