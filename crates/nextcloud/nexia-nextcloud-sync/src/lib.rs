//! Bidirectional synchronization engine, reconciliation logic, and sync journal for Nextcloud.
//!
//! This crate provides:
//! - Deterministic 3-way reconciliation ([`reconcile`], [`SyncAction`]), plus a local-only
//!   refresh that never plans a deletion ([`reconcile_local_changes`]).
//! - Persistent synchronization journal tracking remote ETags, local mtimes, and sizes ([`SyncJournal`], [`JournalEntry`]).
//! - Recursive local filesystem scanner ([`scan_local_directory`], [`DiscoveredLocalFile`]).
//! - Remote WebDAV item metadata model ([`DiscoveredRemoteFile`]).
#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod action;
pub mod download;
pub mod error;
pub mod journal;
pub mod reconciler;
pub mod scanner;

pub use action::SyncAction;
pub use download::{
    DownloadExecutor, DownloadPriority, DownloadProgress, DownloadQueueEvent,
    DownloadQueueSnapshot, DownloadStatus, DownloadTask, DownloadTaskId, SyncDownloadQueue,
};
pub use error::SyncError;
pub use journal::{JournalEntry, SyncJournal};
pub use reconciler::{reconcile, reconcile_local_changes};
pub use scanner::{DiscoveredLocalFile, DiscoveredRemoteFile, scan_local_directory};
