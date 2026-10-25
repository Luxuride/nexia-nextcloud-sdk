//! Internal command messages passed from caller handles to the background orchestrator actor.

use std::path::PathBuf;
use tokio::sync::oneshot;

use crate::error::OrchestratorResult;
use crate::mount::{MountDescriptor, MountId, MountState, SyncFileStatus};
use nextcloud_sdk_vfs::VirtualEntrySummary;

/// Internal command messages dispatched across the orchestrator actor channel.
pub enum OrchestratorCommand {
    /// Adds a new mount point.
    AddMount {
        /// Configuration descriptor for the new mount.
        descriptor: MountDescriptor,
        /// Response channel returning the created mount identifier.
        responder: oneshot::Sender<OrchestratorResult<MountId>>,
    },

    /// Removes an existing mount point.
    RemoveMount {
        /// Target mount identifier to remove.
        id: MountId,
        /// Response channel returning the removed descriptor.
        responder: oneshot::Sender<OrchestratorResult<MountDescriptor>>,
    },

    /// Lists all configured mounts and their current runtime state.
    ListMounts {
        /// Response channel returning all mounts.
        responder: oneshot::Sender<Vec<(MountDescriptor, MountState)>>,
    },

    /// Retrieves an individual mount by its identifier.
    GetMount {
        /// Mount identifier.
        id: MountId,
        /// Response channel returning the descriptor and state if found.
        responder: oneshot::Sender<Option<(MountDescriptor, MountState)>>,
    },

    /// Queries the file synchronization status and icon descriptor for a path within a mount.
    GetFileStatus {
        /// Associated mount identifier.
        mount_id: MountId,
        /// Relative path within the mount.
        relative_path: String,
        /// Response channel returning the computed file sync status.
        responder: oneshot::Sender<SyncFileStatus>,
    },

    /// Queries the file synchronization status for an arbitrary local path across all mounts.
    GetStatusForLocalPath {
        /// Local filesystem path.
        local_path: PathBuf,
        /// Response channel returning the resolved status if inside a mount.
        responder: oneshot::Sender<Option<SyncFileStatus>>,
    },

    /// Sets or updates the file synchronization status for a specific file inside a mount.
    SetFileStatus {
        /// Associated mount identifier.
        mount_id: MountId,
        /// Relative path within the mount.
        relative_path: String,
        /// New synchronization status.
        status: SyncFileStatus,
        /// Response channel acknowledging receipt.
        responder: oneshot::Sender<()>,
    },

    /// Lists direct children entries in the virtual filesystem for a mount.
    VfsListChildren {
        /// Associated mount identifier.
        mount_id: MountId,
        /// Directory path to list.
        path: String,
        /// Response channel returning entries or error.
        responder: oneshot::Sender<OrchestratorResult<Vec<VirtualEntrySummary>>>,
    },

    /// Repopulates the virtual tree for a mount from remote WebDAV resource summaries.
    VfsPopulateResources {
        /// Associated mount identifier.
        mount_id: MountId,
        /// Remote WebDAV base path prefix.
        dav_prefix: String,
        /// List of remote resource summaries.
        resources: Vec<nextcloud_sdk_vfs::DavResourceSummary>,
        /// Response channel returning the number of entries populated.
        responder: oneshot::Sender<OrchestratorResult<usize>>,
    },

    /// Retrieves a complete snapshot clone of the in-memory virtual file tree.
    VfsSnapshot {
        /// Associated mount identifier.
        mount_id: MountId,
        /// Response channel returning the virtual tree clone or error.
        responder: oneshot::Sender<OrchestratorResult<nextcloud_sdk_vfs::VirtualTree>>,
    },

    /// Retrieves a single entry summary from the virtual filesystem.
    VfsGetEntry {
        /// Associated mount identifier.
        mount_id: MountId,
        /// Virtual path to query.
        path: String,
        /// Response channel returning entry if found.
        responder: oneshot::Sender<Option<VirtualEntrySummary>>,
    },

    /// Queries whether a directory node has already been populated with children.
    VfsIsDirectoryPopulated {
        /// Associated mount identifier.
        mount_id: MountId,
        /// Directory path to query.
        path: String,
        /// Response channel returning whether the directory is populated.
        responder: oneshot::Sender<bool>,
    },

    /// Retrieves all directory paths in the tree that have not yet had their children populated.
    VfsGetUnpopulatedDirectories {
        /// Associated mount identifier.
        mount_id: MountId,
        /// Response channel returning unpopulated directory paths.
        responder: oneshot::Sender<Vec<String>>,
    },

    /// Hydrates a virtual file locally with provided content bytes.
    VfsHydrateLocal {
        /// Associated mount identifier.
        mount_id: MountId,
        /// Relative path of the file to hydrate.
        path: String,
        /// Raw file bytes.
        content: Vec<u8>,
        /// Response channel returning the local cached file path.
        responder: oneshot::Sender<OrchestratorResult<PathBuf>>,
    },

    /// Dehydrates a locally cached file, reverting it to a placeholder.
    VfsDehydrateLocal {
        /// Associated mount identifier.
        mount_id: MountId,
        /// Relative path of the file to dehydrate.
        path: String,
        /// Response channel acknowledging completion.
        responder: oneshot::Sender<OrchestratorResult<()>>,
    },

    /// Creates a new directory in the virtual tree for a mount.
    VfsCreateDirectory {
        /// Associated mount identifier.
        mount_id: MountId,
        /// Virtual path of the directory.
        path: String,
        /// Response channel acknowledging creation.
        responder: oneshot::Sender<OrchestratorResult<()>>,
    },

    /// Renames a virtual file or directory within a mount.
    VfsRename {
        /// Associated mount identifier.
        mount_id: MountId,
        /// Existing virtual path.
        old_path: String,
        /// New base name.
        new_name: String,
        /// Response channel acknowledging rename.
        responder: oneshot::Sender<OrchestratorResult<()>>,
    },

    /// Removes a virtual file or directory within a mount.
    VfsRemove {
        /// Associated mount identifier.
        mount_id: MountId,
        /// Virtual path to delete.
        path: String,
        /// Response channel acknowledging deletion.
        responder: oneshot::Sender<OrchestratorResult<()>>,
    },

    /// Triggers a folder synchronization cycle for a mount.
    SyncMount {
        /// Target mount identifier.
        mount_id: MountId,
        /// Response channel acknowledging cycle completion.
        responder: oneshot::Sender<OrchestratorResult<()>>,
    },

    /// Specifically pauses synchronization for a single mount point.
    PauseMount {
        /// Target mount identifier.
        id: MountId,
        /// Response channel acknowledging pause.
        responder: oneshot::Sender<OrchestratorResult<()>>,
    },

    /// Specifically resumes synchronization for a single mount point.
    ResumeMount {
        /// Target mount identifier.
        id: MountId,
        /// Response channel acknowledging resume.
        responder: oneshot::Sender<OrchestratorResult<()>>,
    },

    /// Queries whether a specific mount point is currently paused.
    IsMountPaused {
        /// Target mount identifier.
        id: MountId,
        /// Response channel returning whether the mount is paused.
        responder: oneshot::Sender<bool>,
    },

    /// Globally pauses synchronization across all mounts and background download queues.
    PauseAllSync {
        /// Response channel acknowledging global pause.
        responder: oneshot::Sender<OrchestratorResult<()>>,
    },

    /// Globally resumes synchronization across all mounts and background download queues.
    ResumeAllSync {
        /// Response channel acknowledging global resume.
        responder: oneshot::Sender<OrchestratorResult<()>>,
    },

    /// Queries whether synchronization is globally paused.
    IsGlobalSyncPaused {
        /// Response channel returning whether global sync is paused.
        responder: oneshot::Sender<bool>,
    },

    /// Calculates the disk space usage and cached file capacity for a specific mount.
    GetMountStorageUsage {
        /// Associated mount identifier.
        mount_id: MountId,
        /// Response channel returning the computed storage usage breakdown.
        responder: oneshot::Sender<OrchestratorResult<crate::mount::MountStorageUsage>>,
    },

    /// Clears cached files or hydrated local replicas for a mount.
    ClearMountCache {
        /// Associated mount identifier.
        mount_id: MountId,
        /// If `true`, dehydrates and removes local hydrated files.
        clear_hydrated: bool,
        /// Response channel returning the number of bytes freed.
        responder: oneshot::Sender<OrchestratorResult<u64>>,
    },

    /// Clears shared cached thumbnails across all mounts.
    ClearThumbnailCache {
        /// Response channel returning the number of bytes freed.
        responder: oneshot::Sender<OrchestratorResult<u64>>,
    },

    /// Retrieves the current progress snapshot for an individual mount point.
    GetMountProgress {
        /// Associated mount identifier.
        mount_id: MountId,
        /// Response channel returning the live progress snapshot.
        responder: oneshot::Sender<OrchestratorResult<crate::mount::MountProgressSnapshot>>,
    },

    /// Gracefully stops the orchestrator worker event loop.
    Shutdown {
        /// Response channel acknowledging shutdown.
        responder: oneshot::Sender<()>,
    },
}
