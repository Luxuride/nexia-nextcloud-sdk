//! Background OS worker thread and Tokio actor runtime for Nextcloud orchestration.

use std::collections::HashMap;
use std::sync::Arc;
use std::thread::JoinHandle;
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, error, info};

use crate::command::OrchestratorCommand;
use crate::config::OrchestratorConfig;
use crate::download::DownloadQueueCoordinator;
use crate::error::{OrchestratorError, OrchestratorResult};
use crate::event::OrchestratorEvent;
use crate::handle::OrchestratorHandle;
use crate::mount::{MountId, MountManager, MountStatus, SyncFileStatus};
use crate::sync::FolderSyncRunner;
use crate::vfs::VfsManager;

/// Master Nextcloud orchestrator running on its own dedicated OS thread and Tokio runtime.
pub struct NextcloudOrchestrator {
    handle: OrchestratorHandle,
    worker_thread: Option<JoinHandle<()>>,
}

impl NextcloudOrchestrator {
    /// Launches the background orchestrator actor thread using the provided configuration.
    pub fn start(config: OrchestratorConfig) -> OrchestratorResult<Self> {
        let (cmd_tx, cmd_rx) = mpsc::channel(256);
        let (event_tx, _) = broadcast::channel(512);

        let handle = OrchestratorHandle::new(cmd_tx, event_tx.clone());
        let event_tx_worker = event_tx.clone();

        let thread_builder = std::thread::Builder::new().name("nextcloud-sdk-worker".to_string());
        let worker_thread = thread_builder
            .spawn(move || {
                let rt_result = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(config.worker_threads)
                    .enable_all()
                    .build();

                let rt = match rt_result {
                    Ok(runtime) => runtime,
                    Err(e) => {
                        error!("failed to initialize orchestrator tokio runtime: {e}");
                        return;
                    }
                };

                rt.block_on(async move {
                    run_orchestrator_actor(config, cmd_rx, event_tx_worker).await;
                });
            })
            .map_err(|err| {
                OrchestratorError::WorkerShutdown(format!("failed to spawn thread: {err}"))
            })?;

        Ok(Self {
            handle,
            worker_thread: Some(worker_thread),
        })
    }

    /// Obtains a cloneable, thread-safe handle to communicate with the orchestrator.
    #[must_use]
    pub fn handle(&self) -> OrchestratorHandle {
        self.handle.clone()
    }

    /// Gracefully shuts down the background worker thread and waits for termination.
    pub fn shutdown(mut self) -> OrchestratorResult<()> {
        let _ = self.handle.shutdown_blocking();
        if let Some(thread) = self.worker_thread.take() {
            let _ = thread.join();
        }
        Ok(())
    }
}

impl Drop for NextcloudOrchestrator {
    fn drop(&mut self) {
        if let Some(thread) = self.worker_thread.take() {
            let _ = self.handle.shutdown_blocking();
            let _ = thread.join();
        }
    }
}

async fn run_orchestrator_actor(
    config: OrchestratorConfig,
    mut cmd_rx: mpsc::Receiver<OrchestratorCommand>,
    event_tx: broadcast::Sender<OrchestratorEvent>,
) {
    info!(
        cache_dir = %config.base_cache_dir.display(),
        "nextcloud orchestrator worker thread started"
    );

    let mut mount_manager = MountManager::new();
    let mut vfs_managers: HashMap<MountId, VfsManager> = HashMap::new();
    let _download_coordinator = Arc::new(DownloadQueueCoordinator::new());

    // Register initial mounts from configuration
    for mount in config.mounts {
        let mount_id = mount.id.clone();
        let mount_dir = config.base_cache_dir.join("mounts").join(mount_id.as_str());
        let vfs = VfsManager::new(mount_dir.join("vfs"), &mount.remote_path);

        if let Ok(id) = mount_manager.add_mount(mount.clone()) {
            vfs_managers.insert(id, vfs);
            let _ = event_tx.send(OrchestratorEvent::MountAdded(mount));
        }
    }

    while let Some(command) = cmd_rx.recv().await {
        match command {
            OrchestratorCommand::AddMount {
                descriptor,
                responder,
            } => {
                let id = descriptor.id.clone();
                let mount_dir = config.base_cache_dir.join("mounts").join(id.as_str());
                let vfs = VfsManager::new(mount_dir.join("vfs"), &descriptor.remote_path);

                if mount_manager.get_mount(&id).is_some() {
                    vfs_managers.entry(id.clone()).or_insert(vfs);
                    let _ = responder.send(Ok(id));
                    continue;
                }

                match mount_manager.add_mount(descriptor.clone()) {
                    Ok(new_id) => {
                        vfs_managers.insert(new_id.clone(), vfs);
                        let _ = event_tx.send(OrchestratorEvent::MountAdded(descriptor));
                        let _ = responder.send(Ok(new_id));
                    }
                    Err(_e) => {
                        vfs_managers.entry(id.clone()).or_insert(vfs);
                        let _ = responder.send(Ok(id));
                    }
                }
            }

            OrchestratorCommand::RemoveMount { id, responder } => {
                match mount_manager.remove_mount(&id) {
                    Ok(desc) => {
                        vfs_managers.remove(&id);
                        let _ = event_tx.send(OrchestratorEvent::MountRemoved(id));
                        let _ = responder.send(Ok(desc));
                    }
                    Err(e) => {
                        let _ = responder.send(Err(e));
                    }
                }
            }

            OrchestratorCommand::ListMounts { responder } => {
                let mounts = mount_manager.list_mounts();
                let _ = responder.send(mounts);
            }

            OrchestratorCommand::GetMount { id, responder } => {
                let mount = mount_manager
                    .get_mount(&id)
                    .map(|(d, s)| (d.clone(), s.clone()));
                let _ = responder.send(mount);
            }

            OrchestratorCommand::GetFileStatus {
                mount_id,
                relative_path,
                responder,
            } => {
                let status = mount_manager.file_status(&mount_id, &relative_path);
                let _ = responder.send(status);
            }

            OrchestratorCommand::GetStatusForLocalPath {
                local_path,
                responder,
            } => {
                let status = mount_manager.status_for_local_path(&local_path);
                let _ = responder.send(status);
            }

            OrchestratorCommand::SetFileStatus {
                mount_id,
                relative_path,
                status,
                responder,
            } => {
                mount_manager.set_file_status(
                    mount_id.clone(),
                    relative_path.clone(),
                    status.clone(),
                );
                let _ = event_tx.send(OrchestratorEvent::FileStatusChanged {
                    mount_id,
                    path: relative_path,
                    status,
                });
                let _ = responder.send(());
            }

            OrchestratorCommand::VfsListChildren {
                mount_id,
                path,
                responder,
            } => {
                if let Some(vfs) = vfs_managers.get(&mount_id) {
                    let children = vfs.list_children(&path);
                    let _ = responder.send(Ok(children));
                } else {
                    let _ = responder.send(Err(OrchestratorError::MountNotFound { id: mount_id }));
                }
            }

            OrchestratorCommand::VfsPopulateResources {
                mount_id,
                dav_prefix,
                resources,
                responder,
            } => {
                if let Some(vfs) = vfs_managers.get_mut(&mount_id) {
                    let count = vfs
                        .tree_mut()
                        .populate_from_resources(&dav_prefix, &resources);
                    let _ = vfs.persist_tree();
                    let _ = responder.send(Ok(count));
                } else {
                    let _ = responder.send(Err(OrchestratorError::MountNotFound { id: mount_id }));
                }
            }

            OrchestratorCommand::VfsGetEntry {
                mount_id,
                path,
                responder,
            } => {
                let entry = vfs_managers.get(&mount_id).and_then(|vfs| vfs.entry(&path));
                let _ = responder.send(entry);
            }

            OrchestratorCommand::VfsIsDirectoryPopulated {
                mount_id,
                path,
                responder,
            } => {
                let is_pop = vfs_managers
                    .get(&mount_id)
                    .map(|vfs| vfs.is_directory_populated(&path))
                    .unwrap_or(false);
                let _ = responder.send(is_pop);
            }

            OrchestratorCommand::VfsGetUnpopulatedDirectories {
                mount_id,
                responder,
            } => {
                let unpopulated = vfs_managers
                    .get(&mount_id)
                    .map(|vfs| vfs.unpopulated_directories())
                    .unwrap_or_default();
                let _ = responder.send(unpopulated);
            }

            OrchestratorCommand::VfsHydrateLocal {
                mount_id,
                path,
                content,
                responder,
            } => {
                if let Some(vfs) = vfs_managers.get_mut(&mount_id) {
                    match vfs.hydrate_local(&path, &content) {
                        Ok(local_path) => {
                            mount_manager.set_file_status(
                                mount_id.clone(),
                                path.clone(),
                                SyncFileStatus::Synced,
                            );
                            let _ = event_tx.send(OrchestratorEvent::FileHydrated {
                                mount_id,
                                remote_path: path,
                                local_path: local_path.clone(),
                            });
                            let _ = responder.send(Ok(local_path));
                        }
                        Err(e) => {
                            let _ = responder.send(Err(e));
                        }
                    }
                } else {
                    let _ = responder.send(Err(OrchestratorError::MountNotFound { id: mount_id }));
                }
            }

            OrchestratorCommand::VfsDehydrateLocal {
                mount_id,
                path,
                responder,
            } => {
                if let Some(vfs) = vfs_managers.get_mut(&mount_id) {
                    match vfs.dehydrate_local(&path) {
                        Ok(()) => {
                            mount_manager.set_file_status(
                                mount_id.clone(),
                                path.clone(),
                                SyncFileStatus::OnlineOnly,
                            );
                            let _ = event_tx.send(OrchestratorEvent::FileDehydrated {
                                mount_id,
                                remote_path: path,
                            });
                            let _ = responder.send(Ok(()));
                        }
                        Err(e) => {
                            let _ = responder.send(Err(e));
                        }
                    }
                } else {
                    let _ = responder.send(Err(OrchestratorError::MountNotFound { id: mount_id }));
                }
            }

            OrchestratorCommand::VfsCreateDirectory {
                mount_id,
                path,
                responder,
            } => {
                if let Some(vfs) = vfs_managers.get_mut(&mount_id) {
                    let res = vfs.create_directory(&path);
                    let _ = responder.send(res);
                } else {
                    let _ = responder.send(Err(OrchestratorError::MountNotFound { id: mount_id }));
                }
            }

            OrchestratorCommand::VfsRename {
                mount_id,
                old_path,
                new_name,
                responder,
            } => {
                if let Some(vfs) = vfs_managers.get_mut(&mount_id) {
                    let res = vfs.rename(&old_path, &new_name);
                    let _ = responder.send(res);
                } else {
                    let _ = responder.send(Err(OrchestratorError::MountNotFound { id: mount_id }));
                }
            }

            OrchestratorCommand::VfsRemove {
                mount_id,
                path,
                responder,
            } => {
                if let Some(vfs) = vfs_managers.get_mut(&mount_id) {
                    let res = vfs.remove(&path);
                    let _ = responder.send(res);
                } else {
                    let _ = responder.send(Err(OrchestratorError::MountNotFound { id: mount_id }));
                }
            }

            OrchestratorCommand::SyncMount {
                mount_id,
                responder,
            } => {
                if mount_manager.is_mount_paused(&mount_id) {
                    let _ = responder.send(Err(OrchestratorError::InvalidPath {
                        path: mount_id.to_string(),
                        reason: "synchronization is paused for this mount".to_string(),
                    }));
                    continue;
                }

                let mount_opt = mount_manager.get_mount(&mount_id).map(|(d, _)| d.clone());
                if let Some(mount) = mount_opt {
                    let _ = mount_manager.set_mount_status(&mount_id, MountStatus::Syncing);
                    let _ = event_tx.send(OrchestratorEvent::MountStatusChanged {
                        id: mount_id.clone(),
                        status: MountStatus::Syncing,
                    });
                    let _ = event_tx.send(OrchestratorEvent::SyncCycleStarted {
                        mount_id: mount_id.clone(),
                    });

                    let journal_path = config
                        .base_cache_dir
                        .join("mounts")
                        .join(mount_id.as_str())
                        .join("journal.db");

                    let runner = FolderSyncRunner::new(
                        mount.local_target.clone(),
                        mount.remote_path.clone(),
                        journal_path,
                    );

                    let sync_res = runner.run_local_reconciliation();
                    match sync_res {
                        Ok(actions) => {
                            let _ = mount_manager.set_mount_status(&mount_id, MountStatus::Idle);
                            let _ = event_tx.send(OrchestratorEvent::SyncCycleCompleted {
                                mount_id: mount_id.clone(),
                                items_processed: actions.len() as u64,
                            });
                            let _ = event_tx.send(OrchestratorEvent::MountStatusChanged {
                                id: mount_id,
                                status: MountStatus::Idle,
                            });
                            let _ = responder.send(Ok(()));
                        }
                        Err(e) => {
                            let err_msg = e.to_string();
                            let _ = mount_manager
                                .set_mount_status(&mount_id, MountStatus::Error(err_msg.clone()));
                            let _ = event_tx.send(OrchestratorEvent::MountStatusChanged {
                                id: mount_id,
                                status: MountStatus::Error(err_msg),
                            });
                            let _ = responder.send(Err(e));
                        }
                    }
                } else {
                    let _ = responder.send(Err(OrchestratorError::MountNotFound { id: mount_id }));
                }
            }

            OrchestratorCommand::PauseMount { id, responder } => {
                match mount_manager.pause_mount(&id) {
                    Ok(()) => {
                        let _ = event_tx.send(OrchestratorEvent::MountStatusChanged {
                            id,
                            status: MountStatus::Paused,
                        });
                        let _ = responder.send(Ok(()));
                    }
                    Err(e) => {
                        let _ = responder.send(Err(e));
                    }
                }
            }

            OrchestratorCommand::ResumeMount { id, responder } => {
                match mount_manager.resume_mount(&id) {
                    Ok(()) => {
                        let _ = event_tx.send(OrchestratorEvent::MountStatusChanged {
                            id,
                            status: MountStatus::Idle,
                        });
                        let _ = responder.send(Ok(()));
                    }
                    Err(e) => {
                        let _ = responder.send(Err(e));
                    }
                }
            }

            OrchestratorCommand::IsMountPaused { id, responder } => {
                let is_paused = mount_manager.is_mount_paused(&id);
                let _ = responder.send(is_paused);
            }

            OrchestratorCommand::PauseAllSync { responder } => {
                mount_manager.pause_all();
                let _ = event_tx.send(OrchestratorEvent::GlobalSyncPaused);
                let _ = responder.send(Ok(()));
            }

            OrchestratorCommand::ResumeAllSync { responder } => {
                mount_manager.resume_all();
                let _ = event_tx.send(OrchestratorEvent::GlobalSyncResumed);
                let _ = responder.send(Ok(()));
            }

            OrchestratorCommand::IsGlobalSyncPaused { responder } => {
                let is_paused = mount_manager.is_globally_paused();
                let _ = responder.send(is_paused);
            }

            OrchestratorCommand::GetMountStorageUsage {
                mount_id,
                responder,
            } => {
                if let Some((desc, _state)) = mount_manager.get_mount(&mount_id) {
                    let desc = desc.clone();
                    let vfs_dir = config
                        .base_cache_dir
                        .join("mounts")
                        .join(mount_id.as_str())
                        .join("vfs");
                    let (hydrated_count, local_bytes) =
                        if desc.local_target.exists() && desc.local_target != vfs_dir {
                            let (count, bytes) = scan_dir_usage(&desc.local_target);
                            if count > 0 || bytes > 0 {
                                (count, bytes)
                            } else if let Some(vfs) = vfs_managers.get(&mount_id) {
                                vfs.calculate_hydrated_storage()
                            } else {
                                (0, 0)
                            }
                        } else if let Some(vfs) = vfs_managers.get(&mount_id) {
                            let (vfs_count, vfs_bytes) = vfs.calculate_hydrated_storage();
                            if (vfs_count > 0 || vfs_bytes > 0) || !desc.local_target.exists() {
                                (vfs_count, vfs_bytes)
                            } else {
                                scan_dir_usage(&desc.local_target)
                            }
                        } else if desc.local_target.exists() {
                            scan_dir_usage(&desc.local_target)
                        } else {
                            (0, 0)
                        };

                    let shared_thumbnail_bytes =
                        calculate_dir_bytes(&config.base_cache_dir.join("thumbnails"));
                    let usage = crate::mount::MountStorageUsage::new(
                        mount_id,
                        local_bytes,
                        hydrated_count,
                        shared_thumbnail_bytes,
                    );
                    let _ = responder.send(Ok(usage));
                } else {
                    let _ = responder.send(Err(OrchestratorError::MountNotFound { id: mount_id }));
                }
            }

            OrchestratorCommand::ClearMountCache {
                mount_id,
                clear_hydrated: _,
                responder,
            } => {
                if let Some(vfs) = vfs_managers.get_mut(&mount_id) {
                    let (_count, freed) = vfs.clear_all_hydrated();
                    let _ = responder.send(Ok(freed));
                } else if mount_manager.get_mount(&mount_id).is_some() {
                    let journal = config
                        .base_cache_dir
                        .join("mounts")
                        .join(mount_id.as_str())
                        .join("sync.journal");
                    let mut freed = 0;
                    if journal.exists() {
                        if let Ok(meta) = std::fs::metadata(&journal) {
                            freed += meta.len();
                        }
                        let _ = std::fs::remove_file(&journal);
                    }
                    let _ = responder.send(Ok(freed));
                } else {
                    let _ = responder.send(Err(OrchestratorError::MountNotFound { id: mount_id }));
                }
            }

            OrchestratorCommand::ClearThumbnailCache { responder } => {
                let thumbs_dir = config.base_cache_dir.join("thumbnails");
                let freed = clear_dir_files(&thumbs_dir);
                let _ = responder.send(Ok(freed));
            }

            OrchestratorCommand::GetMountProgress {
                mount_id,
                responder,
            } => {
                if let Some((_desc, state)) = mount_manager.get_mount(&mount_id) {
                    let progress =
                        crate::mount::MountProgressSnapshot::new(mount_id, state.status.clone());
                    let _ = responder.send(Ok(progress));
                } else {
                    let _ = responder.send(Err(OrchestratorError::MountNotFound { id: mount_id }));
                }
            }

            OrchestratorCommand::Shutdown { responder } => {
                debug!("received shutdown command in orchestrator actor");
                let _ = responder.send(());
                break;
            }
        }
    }

    info!("nextcloud orchestrator actor terminated gracefully");
}

fn scan_dir_usage(dir: &std::path::Path) -> (usize, u64) {
    let mut count = 0;
    let mut bytes = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_dir() {
                    let (sub_count, sub_bytes) = scan_dir_usage(&entry.path());
                    count += sub_count;
                    bytes += sub_bytes;
                } else if file_type.is_file()
                    && let Ok(meta) = entry.metadata()
                {
                    let len = meta.len();
                    if len > 0 {
                        count += 1;
                    }
                    bytes += len;
                }
            }
        }
    }
    (count, bytes)
}

fn calculate_dir_bytes(dir: &std::path::Path) -> u64 {
    let mut bytes = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_dir() {
                    bytes += calculate_dir_bytes(&entry.path());
                } else if file_type.is_file()
                    && let Ok(meta) = entry.metadata()
                {
                    bytes += meta.len();
                }
            }
        }
    }
    bytes
}

fn clear_dir_files(dir: &std::path::Path) -> u64 {
    let mut freed = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type()
                && file_type.is_file()
            {
                if let Ok(meta) = entry.metadata() {
                    freed += meta.len();
                }
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    freed
}
