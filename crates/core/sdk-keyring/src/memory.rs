use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::error::KeyringError;
use crate::secret::SecretString;
use crate::traits::KeyringBackend;

/// An in-memory, thread-safe keyring backend primarily intended for testing
/// and environments without a platform credential store.
#[derive(Clone, Debug, Default)]
pub struct MemoryKeyring {
    store: Arc<RwLock<HashMap<(String, String), SecretString>>>,
}

impl MemoryKeyring {
    /// Creates a new empty in-memory keyring.
    pub fn new() -> Self {
        Self {
            store: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Clears all entries stored in this keyring.
    pub fn clear(&self) {
        if let Ok(mut lock) = self.store.write() {
            lock.clear();
        }
    }

    /// Returns the number of secrets currently stored.
    pub fn len(&self) -> usize {
        self.store.read().map(|l| l.len()).unwrap_or(0)
    }

    /// Returns whether the keyring is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl KeyringBackend for MemoryKeyring {
    async fn get_secret(
        &self,
        service: &str,
        account: &str,
    ) -> Result<Option<SecretString>, KeyringError> {
        let lock = self
            .store
            .read()
            .map_err(|e| KeyringError::platform("read", e.to_string()))?;
        Ok(lock
            .get(&(service.to_string(), account.to_string()))
            .cloned())
    }

    async fn set_secret(
        &self,
        service: &str,
        account: &str,
        secret: &str,
    ) -> Result<(), KeyringError> {
        let mut lock = self
            .store
            .write()
            .map_err(|e| KeyringError::platform("write", e.to_string()))?;
        lock.insert(
            (service.to_string(), account.to_string()),
            SecretString::new(secret),
        );
        Ok(())
    }

    async fn delete_secret(&self, service: &str, account: &str) -> Result<(), KeyringError> {
        let mut lock = self
            .store
            .write()
            .map_err(|e| KeyringError::platform("delete", e.to_string()))?;
        lock.remove(&(service.to_string(), account.to_string()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn memory_keyring_roundtrip() {
        let keyring = MemoryKeyring::new();
        assert!(keyring.is_empty());

        assert_eq!(keyring.get_secret("test-svc", "alice").await.unwrap(), None);

        keyring
            .set_secret("test-svc", "alice", "topsecret1")
            .await
            .unwrap();
        assert_eq!(keyring.len(), 1);

        let retrieved = keyring.get_secret("test-svc", "alice").await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().expose_secret(), "topsecret1");

        keyring.delete_secret("test-svc", "alice").await.unwrap();
        assert_eq!(keyring.get_secret("test-svc", "alice").await.unwrap(), None);
        assert!(keyring.is_empty());
    }
}
