use crate::error::KeyringError;
use crate::platform::PlatformKeyring;
use crate::secret::SecretString;
use crate::traits::KeyringBackend;

/// A high-level handle for getting, setting, and deleting secrets scoped to a specific
/// `service` and `account`.
#[derive(Clone, Debug)]
pub struct KeyringEntry<B = PlatformKeyring> {
    service: String,
    account: String,
    backend: B,
}

impl KeyringEntry<PlatformKeyring> {
    /// Creates a new entry using the native [`PlatformKeyring`].
    pub fn new(service: impl Into<String>, account: impl Into<String>) -> Self {
        Self::with_backend(service, account, PlatformKeyring::new())
    }
}

impl<B> KeyringEntry<B> {
    /// Creates a new entry backed by a custom [`KeyringBackend`].
    pub fn with_backend(
        service: impl Into<String>,
        account: impl Into<String>,
        backend: B,
    ) -> Self {
        Self {
            service: service.into(),
            account: account.into(),
            backend,
        }
    }

    /// The service or application namespace for this entry.
    pub fn service(&self) -> &str {
        &self.service
    }

    /// The account or username for this entry.
    pub fn account(&self) -> &str {
        &self.account
    }

    /// Reference to the underlying backend.
    pub fn backend(&self) -> &B {
        &self.backend
    }
}

impl<B: KeyringBackend> KeyringEntry<B> {
    /// Retrieves the secret stored for this entry.
    pub async fn get(&self) -> Result<Option<SecretString>, KeyringError> {
        self.backend.get_secret(&self.service, &self.account).await
    }

    /// Stores the secret for this entry.
    pub async fn set(&self, secret: &str) -> Result<(), KeyringError> {
        self.backend
            .set_secret(&self.service, &self.account, secret)
            .await
    }

    /// Deletes the secret stored for this entry.
    pub async fn delete(&self) -> Result<(), KeyringError> {
        self.backend
            .delete_secret(&self.service, &self.account)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::MemoryKeyring;

    #[tokio::test]
    async fn entry_operations() {
        let entry = KeyringEntry::with_backend("cloud-app", "user@server", MemoryKeyring::new());
        assert_eq!(entry.service(), "cloud-app");
        assert_eq!(entry.account(), "user@server");

        assert_eq!(entry.get().await.unwrap(), None);

        entry.set("mypassword").await.unwrap();
        assert_eq!(
            entry.get().await.unwrap().unwrap().expose_secret(),
            "mypassword"
        );

        entry.delete().await.unwrap();
        assert_eq!(entry.get().await.unwrap(), None);
    }
}
