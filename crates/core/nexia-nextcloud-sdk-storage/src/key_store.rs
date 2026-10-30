use std::future::Future;

use crate::{StorageError, StoreKey};

/// Where the database key lives between runs.
///
/// Implementations must keep the key out of the database file itself; the
/// platform keychain ([`KeychainKeyStore`](crate::KeychainKeyStore)) is the
/// default, but an application may supply its own (an OS credential manager,
/// an HSM, a test double) by implementing this trait.
pub trait KeyStore: Send + Sync {
    /// Returns the stored key, creating and persisting one when absent.
    ///
    /// The returned future is `Send` so callers can hold it across `.await`
    /// in spawned tasks.
    fn load_or_create(&self) -> impl Future<Output = Result<StoreKey, StorageError>> + Send;

    /// Returns the stored key, or `Ok(None)` when nothing is stored yet.
    fn load(&self) -> impl Future<Output = Result<Option<StoreKey>, StorageError>> + Send;

    /// Persists `key`, replacing any previous value.
    fn store(&self, key: StoreKey) -> impl Future<Output = Result<(), StorageError>> + Send;

    /// Forgets the stored key.
    ///
    /// Called when the databases are deleted: a key without a database and a
    /// database without a key are both useless, but only the former is
    /// harmless to leave behind.
    fn delete(&self) -> impl Future<Output = Result<(), StorageError>> + Send;
}
