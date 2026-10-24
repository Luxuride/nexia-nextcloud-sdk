use std::collections::HashMap;

use oo7::{Keyring, Secret};

use crate::error::StorageError;
use crate::key::StoreKey;
use crate::key_store::KeyStore;

/// The `xdg:schema` attribute identifying SDK keychain entries.
const SCHEMA: (&str, &str) = (
    oo7::XDG_SCHEMA_ATTRIBUTE,
    "dev.luxuride.NextcloudSdk.StoreKey",
);
const APPLICATION: &str = "application";
const ACCOUNT: &str = "account";

/// Keeps the database key in the platform keychain through `oo7`.
///
/// One entry is created per application id and account label, so several
/// installs (or several Nextcloud accounts with separate databases) do not
/// fight over a single secret.
#[derive(Clone, Debug)]
pub struct KeychainKeyStore {
    application: String,
    account: String,
}

impl KeychainKeyStore {
    /// Builds a store addressed by `application` and `account`.
    ///
    /// `application` is usually the enclosing app's bundle/reverse-DNS id,
    /// `account` a stable label for the database set.
    pub fn new(application: impl Into<String>, account: impl Into<String>) -> Self {
        Self {
            application: application.into(),
            account: account.into(),
        }
    }

    /// Label written to the keychain entry.
    pub fn label(&self) -> String {
        format!("{}/{}", self.application, self.account)
    }

    fn attributes(&self) -> HashMap<String, String> {
        HashMap::from([
            (SCHEMA.0.to_owned(), SCHEMA.1.to_owned()),
            (APPLICATION.to_owned(), self.application.clone()),
            (ACCOUNT.to_owned(), self.account.clone()),
        ])
    }

    async fn keyring(&self) -> Result<Keyring, StorageError> {
        Keyring::new().await.map_err(StorageError::keychain)
    }

    async fn entry(&self) -> Result<Option<oo7::Item>, StorageError> {
        let keyring = self.keyring().await?;
        let items = keyring
            .search_items(&self.attributes())
            .await
            .map_err(StorageError::keychain)?;
        Ok(items.into_iter().next())
    }
}

impl KeyStore for KeychainKeyStore {
    async fn load_or_create(&self) -> Result<StoreKey, StorageError> {
        if let Some(key) = self.load().await? {
            return Ok(key);
        }
        let key = StoreKey::generate()?;
        self.store(key.clone()).await?;
        Ok(key)
    }

    async fn load(&self) -> Result<Option<StoreKey>, StorageError> {
        let Some(item) = self.entry().await? else {
            return Ok(None);
        };
        let secret = item.secret().await.map_err(StorageError::keychain)?;
        let text = std::str::from_utf8(secret.as_bytes())
            .map_err(|_| StorageError::MalformedKey {
                entry: self.label(),
            })?
            .to_owned();
        StoreKey::from_hex(text.trim(), self.label()).map(Some)
    }

    async fn store(&self, key: StoreKey) -> Result<(), StorageError> {
        let keyring = self.keyring().await?;
        keyring
            .create_item(
                &self.label(),
                &self.attributes(),
                Secret::text(key.to_hex()),
                true,
            )
            .await
            .map_err(StorageError::keychain)
    }

    async fn delete(&self) -> Result<(), StorageError> {
        let keyring = self.keyring().await?;
        keyring
            .delete(&self.attributes())
            .await
            .map_err(StorageError::keychain)
    }
}

#[cfg(test)]
mod tests {
    use super::KeychainKeyStore;

    #[test]
    fn label_combines_application_and_account() {
        let store = KeychainKeyStore::new("dev.example.app", "default");
        assert_eq!(store.label(), "dev.example.app/default");
    }

    #[test]
    fn attributes_are_stable_and_name_the_schema() {
        let store = KeychainKeyStore::new("dev.example.app", "default");
        let attributes = store.attributes();
        assert_eq!(
            attributes.get("xdg:schema").map(String::as_str),
            Some("dev.luxuride.NextcloudSdk.StoreKey")
        );
        assert_eq!(
            attributes.get("account").map(String::as_str),
            Some("default")
        );
    }

    #[test]
    fn debug_prints_only_the_address() {
        let store = KeychainKeyStore::new("dev.example.app", "default");
        let rendered = format!("{store:?}");
        assert!(rendered.contains("dev.example.app"));
        assert!(!rendered.contains("hex"));
    }
}
