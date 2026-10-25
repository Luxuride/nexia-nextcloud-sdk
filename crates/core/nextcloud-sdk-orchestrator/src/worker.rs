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

                match mount_manager.add_mount(descriptor.clone()) {
                    Ok(new_id) => {
                        vfs_managers.insert(new_id.clone(), vfs);
                        let _ = event_tx.send(OrchestratorEvent::MountAdded(descriptor));
                        let _ = responder.send(Ok(new_id));
                    }
                    Err(e) => {
                        let _ = responder.send(Err(e));
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

            OrchestratorCommand::VfsGetEntry {
                mount_id,
                path,
                responder,
            } => {
                let entry = vfs_managers.get(&mount_id).and_then(|vfs| vfs.entry(&path));
                let _ = responder.send(entry);
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

            OrchestratorCommand::Shutdown { responder } => {
                debug!("received shutdown command in orchestrator actor");
                let _ = responder.send(());
                break;
            }
        }
    }

    info!("nextcloud orchestrator actor terminated gracefully");
}
