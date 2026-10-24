//! Encrypted persistent storage adapters for credentials, session state, and cached metadata.

mod cache_entry;
mod cache_store;
mod session_store;

pub use cache_entry::CachedDavEntry;
pub use cache_store::CacheStore;
pub use session_store::SessionStore;
