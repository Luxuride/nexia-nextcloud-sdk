//! High-level background orchestrator, actor runtime, and multi-mount synchronization engine for Nextcloud.
//!
//! Provides a dedicated worker thread with an isolated asynchronous Tokio runtime,
//! managing multiple concurrent mount points (for root and subdirectories), virtual filesystem
//! hierarchies with on-demand hydration and dehydration, bidirectional folder synchronization,
//! and standard FreeDesktop/GNOME file synchronization status icons.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod command;
mod config;
mod download;
mod error;
mod event;
mod handle;
pub mod mount;
pub mod sync;
pub mod vfs;
mod worker;

pub use config::OrchestratorConfig;
pub use download::DownloadQueueCoordinator;
pub use error::{OrchestratorError, OrchestratorResult};
pub use event::OrchestratorEvent;
pub use handle::{MountsHandle, OrchestratorHandle, VfsHandle};
pub use mount::{MountDescriptor, MountId, MountMode, MountState, MountStatus, SyncFileStatus};
pub use sync::FolderSyncRunner;
pub use vfs::VfsManager;
pub use worker::NextcloudOrchestrator;
