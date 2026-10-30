//! Local filesystem monitoring for synchronization triggers and on-demand placeholder hydration.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tracing::warn;

use crate::error::{OrchestratorError, OrchestratorResult};

/// Callback invoked when a local virtual placeholder needs on-demand hydration.
pub type HydrationCallback = Arc<dyn Fn(PathBuf) + Send + Sync + 'static>;

/// Callback invoked when local filesystem modifications require a synchronization cycle for a mount ID.
pub type SyncCallback = Arc<dyn Fn(String) + Send + Sync + 'static>;

/// Monitored filesystem watcher tracking active local mount directories.
pub struct FileSystemWatcher {
    watcher: Option<RecommendedWatcher>,
    watched_paths: Arc<Mutex<HashMap<PathBuf, String>>>,
    on_hydrate: Option<HydrationCallback>,
    on_sync: Option<SyncCallback>,
}

impl std::fmt::Debug for FileSystemWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileSystemWatcher")
            .field("is_running", &self.watcher.is_some())
            .finish()
    }
}

impl Default for FileSystemWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl FileSystemWatcher {
    /// Creates an unstarted filesystem watcher.
    #[must_use]
    pub fn new() -> Self {
        Self {
            watcher: None,
            watched_paths: Arc::new(Mutex::new(HashMap::new())),
            on_hydrate: None,
            on_sync: None,
        }
    }

    /// Configures the callback triggered when an unhydrated placeholder is accessed or opened.
    #[must_use]
    pub fn with_hydration_handler(mut self, handler: HydrationCallback) -> Self {
        self.on_hydrate = Some(handler);
        self
    }

    /// Configures the callback triggered when local file changes require an upload sync cycle.
    #[must_use]
    pub fn with_sync_handler(mut self, handler: SyncCallback) -> Self {
        self.on_sync = Some(handler);
        self
    }

    /// Starts monitoring local directories for changes using an asynchronous debounced worker.
    pub fn start(&mut self) -> OrchestratorResult<()> {
        let (tx, mut rx) = mpsc::channel::<(EventKind, PathBuf)>(100);
        let watched_map = self.watched_paths.clone();

        let event_handler = move |res: Result<Event, notify::Error>| match res {
            Ok(event) => {
                if matches!(
                    event.kind,
                    EventKind::Create(_)
                        | EventKind::Modify(_)
                        | EventKind::Remove(_)
                        | EventKind::Access(_)
                ) {
                    for path in event.paths {
                        if path.components().any(|c| {
                            let s = c.as_os_str().to_string_lossy();
                            s.starts_with('.')
                                || s.ends_with(".part")
                                || s.ends_with(".crdownload")
                                || s.ends_with("journal.json")
                                || s.ends_with("tree.json")
                        }) {
                            continue;
                        }
                        let _ = tx.blocking_send((event.kind, path));
                    }
                }
            }
            Err(e) => {
                warn!(error = %e, "Filesystem watcher error received");
            }
        };

        let mut watcher =
            RecommendedWatcher::new(event_handler, Config::default()).map_err(|e| {
                OrchestratorError::Watcher {
                    context: "creating recommended watcher".to_string(),
                    message: e.to_string(),
                }
            })?;

        // Re-register any existing watched paths
        if let Ok(guard) = watched_map.lock() {
            for path in guard.keys() {
                if path.exists() {
                    let _ = watcher.watch(path, RecursiveMode::Recursive);
                }
            }
        }

        self.watcher = Some(watcher);

        let map_for_worker = self.watched_paths.clone();
        let hydrate_cb = self.on_hydrate.clone();
        let sync_cb = self.on_sync.clone();
        let hydrating_paths = Arc::new(Mutex::new(HashSet::<PathBuf>::new()));

        tokio::spawn(async move {
            let mut pending_mounts: HashSet<String> = HashSet::new();

            loop {
                tokio::select! {
                    received = rx.recv() => {
                        // `None` means every watch callback was dropped. Both
                        // branches can then be disabled at once, which makes
                        // `select!` panic unless the channel close ends the task.
                        let Some((kind, changed_path)) = received else {
                            break;
                        };
                        let is_placeholder = is_unhydrated_placeholder(&changed_path);
                        if is_placeholder {
                            if let Some(ref handler) = hydrate_cb {
                                let should_hydrate = if let Ok(mut set) = hydrating_paths.lock() {
                                    set.insert(changed_path.clone())
                                } else {
                                    false
                                };
                                if should_hydrate {
                                    let cb = handler.clone();
                                    let path_clone = changed_path.clone();
                                    let set_clone = hydrating_paths.clone();
                                    tokio::spawn(async move {
                                        cb(path_clone.clone());
                                        tokio::time::sleep(Duration::from_secs(5)).await;
                                        if let Ok(mut set) = set_clone.lock() {
                                            set.remove(&path_clone);
                                        }
                                    });
                                }
                            }
                        } else if !matches!(kind, EventKind::Access(_))
                            && let Ok(guard) = map_for_worker.lock()
                        {
                            for (root, mount_id) in guard.iter() {
                                if changed_path.starts_with(root) {
                                    pending_mounts.insert(mount_id.clone());
                                }
                            }
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_millis(1500)), if !pending_mounts.is_empty() => {
                        let to_sync: Vec<String> = pending_mounts.drain().collect();
                        for m_id in to_sync {
                            if let Some(ref handler) = sync_cb {
                                handler(m_id);
                            }
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Registers a local directory path to be monitored for a specific mount ID.
    pub fn watch_folder(&mut self, local_path: &Path, mount_id: &str) -> OrchestratorResult<()> {
        let canonical = local_path.to_path_buf();
        if let Ok(mut guard) = self.watched_paths.lock() {
            guard.insert(canonical.clone(), mount_id.to_string());
        }

        if let Some(ref mut watcher) = self.watcher
            && local_path.exists()
        {
            watcher
                .watch(local_path, RecursiveMode::Recursive)
                .map_err(|e| OrchestratorError::Watcher {
                    context: format!("watching directory {}", local_path.display()),
                    message: e.to_string(),
                })?;
        }
        Ok(())
    }

    /// Unregisters and stops monitoring a local directory path.
    pub fn unwatch_folder(&mut self, local_path: &Path) {
        let canonical = local_path.to_path_buf();
        if let Ok(mut guard) = self.watched_paths.lock() {
            guard.remove(&canonical);
        }

        if let Some(ref mut watcher) = self.watcher {
            let _ = watcher.unwatch(local_path);
        }
    }
}

#[cfg(unix)]
fn is_unhydrated_placeholder(path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    if let Ok(meta) = std::fs::symlink_metadata(path)
        && meta.is_file()
        && meta.len() > 0
    {
        return meta.blocks() == 0;
    }
    false
}

#[cfg(not(unix))]
fn is_unhydrated_placeholder(_path: &Path) -> bool {
    false
}
