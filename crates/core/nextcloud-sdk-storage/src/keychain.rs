use nextcloud_sdk_keyring::{KeyringBackend, KeyringEntry, PlatformKeyring};

use crate::error::StorageError;
use crate::key::StoreKey;
use crate::key_store::KeyStore;

/// Keeps the database key in the platform keychain through [`nextcloud_sdk_keyring`].
///
/// One entry is created per application id and account label, so several
/// installs (or several Nextcloud accounts with separate databases) do not
/// fight over a single secret.
#[derive(Clone, Debug)]
pub struct KeychainKeyStore<B = PlatformKeyring> {
    entry: KeyringEntry<B>,
}

impl KeychainKeyStore<PlatformKeyring> {
    /// Builds a store addressed by `application` and `account`.
    ///
    /// `application` is usually the enclosing app's bundle/reverse-DNS id,
    /// `account` a stable label for the database set.
    pub fn new(application: impl Into<String>, account: impl Into<String>) -> Self {
        Self {
            entry: KeyringEntry::new(application, account),
        }
    }
}

impl<B> KeychainKeyStore<B> {
    /// Builds a store addressed by `application` and `account` with a custom backend.
    pub fn with_backend(
        application: impl Into<String>,
        account: impl Into<String>,
        backend: B,
    ) -> Self {
        Self {
            entry: KeyringEntry::with_backend(application, account, backend),
        }
    }

    /// Label written to the keychain entry.
    pub fn label(&self) -> String {
        format!("{}/{}", self.entry.service(), self.entry.account())
    }
}

impl<B: KeyringBackend> KeyStore for KeychainKeyStore<B> {
    async fn load_or_create(&self) -> Result<StoreKey, StorageError> {
        if let Some(key) = self.load().await? {
            return Ok(key);
        }
        let key = StoreKey::generate()?;
        self.store(key.clone()).await?;
        Ok(key)
    }

    async fn load(&self) -> Result<Option<StoreKey>, StorageError> {
        let secret = self.entry.get().await.map_err(StorageError::keychain)?;
        let Some(secret) = secret else {
            return Ok(None);
        };
        StoreKey::from_hex(secret.expose_secret().trim(), self.label()).map(Some)
    }

    async fn store(&self, key: StoreKey) -> Result<(), StorageError> {
        self.entry
            .set(&key.to_hex())
            .await
            .map_err(StorageError::keychain)
    }

    async fn delete(&self) -> Result<(), StorageError> {
        self.entry.delete().await.map_err(StorageError::keychain)
    }
}

#[cfg(test)]
mod tests {
    use nextcloud_sdk_keyring::MemoryKeyring;

    use super::KeychainKeyStore;
    use crate::key::StoreKey;
    use crate::key_store::KeyStore;

    #[test]
    fn label_combines_application_and_account() {
        let store = KeychainKeyStore::new("dev.example.app", "default");
        assert_eq!(store.label(), "dev.example.app/default");
    }

    #[test]
    fn debug_prints_only_the_address() {
        let store = KeychainKeyStore::new("dev.example.app", "default");
        let rendered = format!("{store:?}");
        assert!(rendered.contains("dev.example.app"));
        assert!(!rendered.contains("hex"));
    }

    #[tokio::test]
    async fn memory_backed_keystore_lifecycle() {
        let store =
            KeychainKeyStore::with_backend("dev.example.app", "account1", MemoryKeyring::new());

        assert!(store.load().await.unwrap().is_none());

        let key = store.load_or_create().await.unwrap();
        let loaded = store.load().await.unwrap().unwrap();
        assert_eq!(key.to_hex(), loaded.to_hex());

        store.delete().await.unwrap();
        assert!(store.load().await.unwrap().is_none());

        // Explicit store
        let custom_key = StoreKey::generate().unwrap();
        store.store(custom_key.clone()).await.unwrap();
        assert_eq!(
            store.load().await.unwrap().unwrap().to_hex(),
            custom_key.to_hex()
        );
    }
}
