//! Orchestrator configuration for runtime initialization.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::mount::MountDescriptor;

/// Configuration parameters for initializing the background Nextcloud orchestrator.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchestratorConfig {
    /// Directory used for storing persistent metadata (`tree.json`, SQLite journals, cache).
    pub base_cache_dir: PathBuf,

    /// Initial list of mount point descriptors.
    pub mounts: Vec<MountDescriptor>,

    /// Periodic background synchronization interval in seconds (0 to disable automatic timer).
    pub sync_interval_seconds: u64,

    /// Number of worker threads dedicated to the orchestrator Tokio runtime.
    pub worker_threads: usize,
}

impl OrchestratorConfig {
    /// Creates a default orchestrator configuration using the specified cache directory.
    #[must_use]
    pub fn new(base_cache_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_cache_dir: base_cache_dir.into(),
            mounts: Vec::new(),
            sync_interval_seconds: 60,
            worker_threads: 2,
        }
    }

    /// Adds a mount point descriptor to the configuration.
    #[must_use]
    pub fn with_mount(mut self, mount: MountDescriptor) -> Self {
        self.mounts.push(mount);
        self
    }

    /// Configures the periodic synchronization interval.
    #[must_use]
    pub fn with_sync_interval_seconds(mut self, interval_secs: u64) -> Self {
        self.sync_interval_seconds = interval_secs;
        self
    }

    /// Configures the number of runtime worker threads.
    #[must_use]
    pub fn with_worker_threads(mut self, threads: usize) -> Self {
        self.worker_threads = threads.max(1);
        self
    }
}
