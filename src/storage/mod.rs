//! Encrypted persistent storage adapters for credentials, session state, and cached metadata.

mod cache_entry;
mod cache_store;
mod config_store;
mod session_store;

pub use cache_entry::CachedDavEntry;
pub use cache_store::CacheStore;
pub use config_store::{CachedTheme, ConfigStore, StoredSyncMount};
pub use session_store::SessionStore;

use nexia_nextcloud_sdk_storage::StorageError;

/// Wraps a `rusqlite` failure as a query error naming `operation`.
///
/// Kept in one place so every store reports the failing statement the same way
/// and so the duplicate definition cannot drift between them.
pub(crate) fn query_err(operation: &'static str, source: rusqlite::Error) -> StorageError {
    StorageError::Query {
        operation,
        source: Box::new(source),
    }
}

/// Current UNIX time in whole seconds.
///
/// A clock before the epoch is reported as `0` rather than failing: every
/// caller only uses the value as a freshness stamp.
pub(crate) fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
