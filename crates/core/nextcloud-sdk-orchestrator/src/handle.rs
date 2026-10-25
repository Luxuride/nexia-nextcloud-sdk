//! High-level handle for interacting with the background Nextcloud orchestrator.

use std::path::PathBuf;
use tokio::sync::{broadcast, mpsc, oneshot};

use crate::command::OrchestratorCommand;
use crate::error::{OrchestratorError, OrchestratorResult};
use crate::event::OrchestratorEvent;
use crate::mount::{MountDescriptor, MountId, MountState, SyncFileStatus};
use nextcloud_sdk_vfs::VirtualEntrySummary;

/// Cloneable, thread-safe handle for communicating with the background orchestrator actor.
#[derive(Clone, Debug)]
pub struct OrchestratorHandle {
    cmd_tx: mpsc::Sender<OrchestratorCommand>,
    event_tx: broadcast::Sender<OrchestratorEvent>,
}

impl OrchestratorHandle {
    /// Creates a new orchestrator handle with the given command and event channels.
    #[must_use]
    pub fn new(
        cmd_tx: mpsc::Sender<OrchestratorCommand>,
        event_tx: broadcast::Sender<OrchestratorEvent>,
    ) -> Self {
        Self { cmd_tx, event_tx }
    }

    /// Access the mount points management interface.
    #[must_use]
    pub fn mounts(&self) -> MountsHandle {
        MountsHandle {
            cmd_tx: self.cmd_tx.clone(),
        }
    }

    /// Access the virtual filesystem interface for a specific mount point.
    #[must_use]
    pub fn vfs(&self, mount_id: MountId) -> VfsHandle {
        VfsHandle {
            mount_id,
            cmd_tx: self.cmd_tx.clone(),
        }
    }

    /// Subscribes to the broadcast stream of orchestrator lifecycle and synchronization events.
    #[must_use]
    pub fn subscribe_events(&self) -> broadcast::Receiver<OrchestratorEvent> {
        self.event_tx.subscribe()
    }

    /// Globally pauses synchronization across all mounts and background queues.
    pub async fn pause_all_sync(&self) -> OrchestratorResult<()> {
        let (responder, rx) = oneshot::channel();
        self.cmd_tx
            .send(OrchestratorCommand::PauseAllSync { responder })
            .await
            .map_err(|_| {
                OrchestratorError::WorkerShutdown(
                    "orchestrator actor is not responding".to_string(),
                )
            })?;
        rx.await
            .map_err(|_| OrchestratorError::WorkerShutdown("actor dropped response".to_string()))?
    }

    /// Globally resumes synchronization across all mounts and background queues.
    pub async fn resume_all_sync(&self) -> OrchestratorResult<()> {
        let (responder, rx) = oneshot::channel();
        self.cmd_tx
            .send(OrchestratorCommand::ResumeAllSync { responder })
            .await
            .map_err(|_| {
                OrchestratorError::WorkerShutdown(
                    "orchestrator actor is not responding".to_string(),
                )
            })?;
        rx.await
            .map_err(|_| OrchestratorError::WorkerShutdown("actor dropped response".to_string()))?
    }

    /// Queries whether synchronization is globally paused.
    pub async fn is_sync_paused(&self) -> OrchestratorResult<bool> {
        let (responder, rx) = oneshot::channel();
        self.cmd_tx
            .send(OrchestratorCommand::IsGlobalSyncPaused { responder })
            .await
            .map_err(|_| {
                OrchestratorError::WorkerShutdown(
                    "orchestrator actor is not responding".to_string(),
                )
            })?;
        rx.await
            .map_err(|_| OrchestratorError::WorkerShutdown("actor dropped response".to_string()))
    }

    /// Sends a graceful shutdown command to the background worker actor.
    pub async fn shutdown(&self) -> OrchestratorResult<()> {
        let (responder, rx) = oneshot::channel();
        self.cmd_tx
            .send(OrchestratorCommand::Shutdown { responder })
            .await
            .map_err(|_| {
                OrchestratorError::WorkerShutdown(
                    "orchestrator actor is not responding".to_string(),
                )
            })?;

        rx.await.map_err(|_| {
            OrchestratorError::WorkerShutdown("actor dropped shutdown response channel".to_string())
        })
    }

    /// Synchronous non-blocking shutdown helper suitable for drop or cleanup.
    pub(crate) fn shutdown_blocking(&self) -> OrchestratorResult<()> {
        let (responder, _) = oneshot::channel();
        let _ = self
            .cmd_tx
            .try_send(OrchestratorCommand::Shutdown { responder });
        Ok(())
    }
}

/// Interface for managing mount points and querying file sync statuses.
#[derive(Clone, Debug)]
pub struct MountsHandle {
    cmd_tx: mpsc::Sender<OrchestratorCommand>,
}

impl MountsHandle {
    /// Registers a new mount point.
    pub async fn add_mount(&self, descriptor: MountDescriptor) -> OrchestratorResult<MountId> {
        let (responder, rx) = oneshot::channel();
        self.send_cmd(OrchestratorCommand::AddMount {
            descriptor,
            responder,
        })
        .await?;
        rx.await
            .map_err(|_| OrchestratorError::WorkerShutdown("actor dropped response".to_string()))?
    }

    /// Removes an existing mount point.
    pub async fn remove_mount(&self, id: MountId) -> OrchestratorResult<MountDescriptor> {
        let (responder, rx) = oneshot::channel();
        self.send_cmd(OrchestratorCommand::RemoveMount { id, responder })
            .await?;
        rx.await
            .map_err(|_| OrchestratorError::WorkerShutdown("actor dropped response".to_string()))?
    }

    /// Lists all active mounts and their runtime states.
    pub async fn list_mounts(&self) -> OrchestratorResult<Vec<(MountDescriptor, MountState)>> {
        let (responder, rx) = oneshot::channel();
        self.send_cmd(OrchestratorCommand::ListMounts { responder })
            .await?;
        rx.await
            .map_err(|_| OrchestratorError::WorkerShutdown("actor dropped response".to_string()))
    }

    /// Retrieves an individual mount by its ID.
    pub async fn get_mount(
        &self,
        id: MountId,
    ) -> OrchestratorResult<Option<(MountDescriptor, MountState)>> {
        let (responder, rx) = oneshot::channel();
        self.send_cmd(OrchestratorCommand::GetMount { id, responder })
            .await?;
        rx.await
            .map_err(|_| OrchestratorError::WorkerShutdown("actor dropped response".to_string()))
    }

    /// Queries the file sync status and icon for a relative path within a mount.
    pub async fn file_status(
        &self,
        mount_id: MountId,
        relative_path: impl Into<String>,
    ) -> OrchestratorResult<SyncFileStatus> {
        let (responder, rx) = oneshot::channel();
        self.send_cmd(OrchestratorCommand::GetFileStatus {
            mount_id,
            relative_path: relative_path.into(),
            responder,
        })
        .await?;
        rx.await
            .map_err(|_| OrchestratorError::WorkerShutdown("actor dropped response".to_string()))
    }

    /// Resolves the synchronization status for an arbitrary local path across all mounts.
    pub async fn status_for_local_path(
        &self,
        local_path: impl Into<PathBuf>,
    ) -> OrchestratorResult<Option<SyncFileStatus>> {
        let (responder, rx) = oneshot::channel();
        self.send_cmd(OrchestratorCommand::GetStatusForLocalPath {
            local_path: local_path.into(),
            responder,
        })
        .await?;
        rx.await
            .map_err(|_| OrchestratorError::WorkerShutdown("actor dropped response".to_string()))
    }

    /// Updates or records the sync status of a file within a mount.
    pub async fn set_file_status(
        &self,
        mount_id: MountId,
        relative_path: impl Into<String>,
        status: SyncFileStatus,
    ) -> OrchestratorResult<()> {
        let (responder, rx) = oneshot::channel();
        self.send_cmd(OrchestratorCommand::SetFileStatus {
            mount_id,
            relative_path: relative_path.into(),
            status,
            responder,
        })
        .await?;
        rx.await
            .map_err(|_| OrchestratorError::WorkerShutdown("actor dropped response".to_string()))
    }

    /// Triggers a synchronization cycle for a specific mount.
    pub async fn sync_mount(&self, mount_id: MountId) -> OrchestratorResult<()> {
        let (responder, rx) = oneshot::channel();
        self.send_cmd(OrchestratorCommand::SyncMount {
            mount_id,
            responder,
        })
        .await?;
        rx.await
            .map_err(|_| OrchestratorError::WorkerShutdown("actor dropped response".to_string()))?
    }

    /// Specifically pauses synchronization for a single mount point.
    pub async fn pause_mount(&self, id: MountId) -> OrchestratorResult<()> {
        let (responder, rx) = oneshot::channel();
        self.send_cmd(OrchestratorCommand::PauseMount { id, responder })
            .await?;
        rx.await
            .map_err(|_| OrchestratorError::WorkerShutdown("actor dropped response".to_string()))?
    }

    /// Specifically resumes synchronization for a single mount point.
    pub async fn resume_mount(&self, id: MountId) -> OrchestratorResult<()> {
        let (responder, rx) = oneshot::channel();
        self.send_cmd(OrchestratorCommand::ResumeMount { id, responder })
            .await?;
        rx.await
            .map_err(|_| OrchestratorError::WorkerShutdown("actor dropped response".to_string()))?
    }

    /// Queries whether a specific mount point is currently paused.
    pub async fn is_paused(&self, id: MountId) -> OrchestratorResult<bool> {
        let (responder, rx) = oneshot::channel();
        self.send_cmd(OrchestratorCommand::IsMountPaused { id, responder })
            .await?;
        rx.await
            .map_err(|_| OrchestratorError::WorkerShutdown("actor dropped response".to_string()))
    }

    /// Computes disk space usage and cached file capacity for a mount point.
    pub async fn storage_usage(
        &self,
        id: MountId,
    ) -> OrchestratorResult<crate::mount::MountStorageUsage> {
        let (responder, rx) = oneshot::channel();
        self.send_cmd(OrchestratorCommand::GetMountStorageUsage {
            mount_id: id,
            responder,
        })
        .await?;
        rx.await
            .map_err(|_| OrchestratorError::WorkerShutdown("actor dropped response".to_string()))?
    }

    /// Clears cached files or hydrated local replicas for a mount point, returning freed bytes.
    pub async fn clear_cache(&self, id: MountId, clear_hydrated: bool) -> OrchestratorResult<u64> {
        let (responder, rx) = oneshot::channel();
        self.send_cmd(OrchestratorCommand::ClearMountCache {
            mount_id: id,
            clear_hydrated,
            responder,
        })
        .await?;
        rx.await
            .map_err(|_| OrchestratorError::WorkerShutdown("actor dropped response".to_string()))?
    }

    /// Clears shared cached thumbnails across all mounts, returning freed bytes.
    pub async fn clear_thumbnail_cache(&self) -> OrchestratorResult<u64> {
        let (responder, rx) = oneshot::channel();
        self.send_cmd(OrchestratorCommand::ClearThumbnailCache { responder })
            .await?;
        rx.await
            .map_err(|_| OrchestratorError::WorkerShutdown("actor dropped response".to_string()))?
    }

    /// Retrieves the current progress snapshot for an individual mount point.
    pub async fn progress(
        &self,
        id: MountId,
    ) -> OrchestratorResult<crate::mount::MountProgressSnapshot> {
        let (responder, rx) = oneshot::channel();
        self.send_cmd(OrchestratorCommand::GetMountProgress {
            mount_id: id,
            responder,
        })
        .await?;
        rx.await
            .map_err(|_| OrchestratorError::WorkerShutdown("actor dropped response".to_string()))?
    }

    async fn send_cmd(&self, cmd: OrchestratorCommand) -> OrchestratorResult<()> {
        self.cmd_tx.send(cmd).await.map_err(|_| {
            OrchestratorError::WorkerShutdown("orchestrator actor is closed".to_string())
        })
    }
}

/// Interface for querying and mutating virtual filesystem items within a specific mount.
#[derive(Clone, Debug)]
pub struct VfsHandle {
    mount_id: MountId,
    cmd_tx: mpsc::Sender<OrchestratorCommand>,
}

impl VfsHandle {
    /// Returns the target mount identifier.
    #[must_use]
    pub fn mount_id(&self) -> &MountId {
        &self.mount_id
    }

    /// Lists direct children entries in the virtual directory.
    pub async fn list_children(
        &self,
        path: impl Into<String>,
    ) -> OrchestratorResult<Vec<VirtualEntrySummary>> {
        let (responder, rx) = oneshot::channel();
        self.send_cmd(OrchestratorCommand::VfsListChildren {
            mount_id: self.mount_id.clone(),
            path: path.into(),
            responder,
        })
        .await?;
        rx.await
            .map_err(|_| OrchestratorError::WorkerShutdown("actor dropped response".to_string()))?
    }

    /// Clears and repopulates the virtual file tree with remote WebDAV resource summaries.
    pub async fn populate_from_resources(
        &self,
        dav_prefix: impl Into<String>,
        resources: Vec<nextcloud_sdk_vfs::DavResourceSummary>,
    ) -> OrchestratorResult<usize> {
        let (responder, rx) = oneshot::channel();
        self.send_cmd(OrchestratorCommand::VfsPopulateResources {
            mount_id: self.mount_id.clone(),
            dav_prefix: dav_prefix.into(),
            resources,
            responder,
        })
        .await?;
        rx.await
            .map_err(|_| OrchestratorError::WorkerShutdown("actor dropped response".to_string()))?
    }

    /// Retrieves a complete snapshot clone of the in-memory virtual tree.
    pub async fn snapshot(&self) -> OrchestratorResult<nextcloud_sdk_vfs::VirtualTree> {
        let (responder, rx) = oneshot::channel();
        self.send_cmd(OrchestratorCommand::VfsSnapshot {
            mount_id: self.mount_id.clone(),
            responder,
        })
        .await?;
        rx.await
            .map_err(|_| OrchestratorError::WorkerShutdown("actor dropped response".to_string()))?
    }

    /// Retrieves an entry summary from the virtual filesystem.
    pub async fn entry(
        &self,
        path: impl Into<String>,
    ) -> OrchestratorResult<Option<VirtualEntrySummary>> {
        let (responder, rx) = oneshot::channel();
        self.send_cmd(OrchestratorCommand::VfsGetEntry {
            mount_id: self.mount_id.clone(),
            path: path.into(),
            responder,
        })
        .await?;
        rx.await
            .map_err(|_| OrchestratorError::WorkerShutdown("actor dropped response".to_string()))
    }

    /// Queries whether a directory in the virtual tree has had its children populated from remote WebDAV.
    pub async fn is_directory_populated(
        &self,
        path: impl Into<String>,
    ) -> OrchestratorResult<bool> {
        let (responder, rx) = oneshot::channel();
        self.send_cmd(OrchestratorCommand::VfsIsDirectoryPopulated {
            mount_id: self.mount_id.clone(),
            path: path.into(),
            responder,
        })
        .await?;
        rx.await
            .map_err(|_| OrchestratorError::WorkerShutdown("actor dropped response".to_string()))
    }

    /// Retrieves all directory paths in the tree that have not yet had their children populated.
    pub async fn unpopulated_directories(&self) -> OrchestratorResult<Vec<String>> {
        let (responder, rx) = oneshot::channel();
        self.send_cmd(OrchestratorCommand::VfsGetUnpopulatedDirectories {
            mount_id: self.mount_id.clone(),
            responder,
        })
        .await?;
        rx.await
            .map_err(|_| OrchestratorError::WorkerShutdown("actor dropped response".to_string()))
    }

    /// Hydrates a file locally by storing provided bytes into local cache storage.
    pub async fn hydrate_local(
        &self,
        path: impl Into<String>,
        content: Vec<u8>,
    ) -> OrchestratorResult<PathBuf> {
        let (responder, rx) = oneshot::channel();
        self.send_cmd(OrchestratorCommand::VfsHydrateLocal {
            mount_id: self.mount_id.clone(),
            path: path.into(),
            content,
            responder,
        })
        .await?;
        rx.await
            .map_err(|_| OrchestratorError::WorkerShutdown("actor dropped response".to_string()))?
    }

    /// Dehydrates a locally cached file, deleting local bytes and reverting to a placeholder.
    pub async fn dehydrate_local(&self, path: impl Into<String>) -> OrchestratorResult<()> {
        let (responder, rx) = oneshot::channel();
        self.send_cmd(OrchestratorCommand::VfsDehydrateLocal {
            mount_id: self.mount_id.clone(),
            path: path.into(),
            responder,
        })
        .await?;
        rx.await
            .map_err(|_| OrchestratorError::WorkerShutdown("actor dropped response".to_string()))?
    }

    /// Creates a directory in the virtual tree.
    pub async fn create_directory(&self, path: impl Into<String>) -> OrchestratorResult<()> {
        let (responder, rx) = oneshot::channel();
        self.send_cmd(OrchestratorCommand::VfsCreateDirectory {
            mount_id: self.mount_id.clone(),
            path: path.into(),
            responder,
        })
        .await?;
        rx.await
            .map_err(|_| OrchestratorError::WorkerShutdown("actor dropped response".to_string()))?
    }

    /// Renames a virtual file or directory entry.
    pub async fn rename(
        &self,
        old_path: impl Into<String>,
        new_name: impl Into<String>,
    ) -> OrchestratorResult<()> {
        let (responder, rx) = oneshot::channel();
        self.send_cmd(OrchestratorCommand::VfsRename {
            mount_id: self.mount_id.clone(),
            old_path: old_path.into(),
            new_name: new_name.into(),
            responder,
        })
        .await?;
        rx.await
            .map_err(|_| OrchestratorError::WorkerShutdown("actor dropped response".to_string()))?
    }

    /// Deletes a virtual file or directory entry.
    pub async fn remove(&self, path: impl Into<String>) -> OrchestratorResult<()> {
        let (responder, rx) = oneshot::channel();
        self.send_cmd(OrchestratorCommand::VfsRemove {
            mount_id: self.mount_id.clone(),
            path: path.into(),
            responder,
        })
        .await?;
        rx.await
            .map_err(|_| OrchestratorError::WorkerShutdown("actor dropped response".to_string()))?
    }

    async fn send_cmd(&self, cmd: OrchestratorCommand) -> OrchestratorResult<()> {
        self.cmd_tx.send(cmd).await.map_err(|_| {
            OrchestratorError::WorkerShutdown("orchestrator actor is closed".to_string())
        })
    }
}
