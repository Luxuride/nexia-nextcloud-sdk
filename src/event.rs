//! Client event broadcasting for reactive UI and watcher synchronization.

use crate::storage::{CachedTheme, StoredSyncMount};

/// High-level client events emitted during client and mount operations.
#[derive(Clone, Debug, PartialEq)]
pub enum ClientEvent {
    /// A sync mount was stored or configured in the permanent database.
    SyncMountAdded(StoredSyncMount),
    /// A sync mount was removed from the permanent database.
    SyncMountRemoved(String),
    /// A sync mount configuration was updated in the permanent database.
    SyncMountUpdated(StoredSyncMount),
    /// Account visual theme was updated or cached.
    ThemeUpdated(CachedTheme),
}
