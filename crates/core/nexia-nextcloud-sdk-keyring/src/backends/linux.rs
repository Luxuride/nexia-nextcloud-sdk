use std::collections::HashMap;

use oo7::{Keyring, Secret};

use crate::error::KeyringError;
use crate::secret::SecretString;
use crate::traits::KeyringBackend;

const SCHEMA: (&str, &str) = (oo7::XDG_SCHEMA_ATTRIBUTE, "org.freedesktop.Secret.Generic");
const SERVICE_ATTR: &str = "service";
const ACCOUNT_ATTR: &str = "account";

/// Linux keyring backend powered by `oo7`.
///
/// Supports both native desktop D-Bus Secret Service (GNOME Keyring, KWallet)
/// and sandboxed Flatpak containers via the XDG Desktop Portal Secret API.
#[derive(Clone, Debug, Default)]
pub struct LinuxKeyring;

impl LinuxKeyring {
    /// Creates a new Linux keyring client.
    pub fn new() -> Self {
        Self
    }

    async fn keyring(&self) -> Result<Keyring, KeyringError> {
        Keyring::new()
            .await
            .map_err(|err| KeyringError::platform("connect", err))
    }

    fn attributes(service: &str, account: &str) -> HashMap<String, String> {
        HashMap::from([
            (SCHEMA.0.to_owned(), SCHEMA.1.to_owned()),
            (SERVICE_ATTR.to_owned(), service.to_owned()),
            (ACCOUNT_ATTR.to_owned(), account.to_owned()),
        ])
    }
}

impl KeyringBackend for LinuxKeyring {
    async fn get_secret(
        &self,
        service: &str,
        account: &str,
    ) -> Result<Option<SecretString>, KeyringError> {
        tracing::debug!(
            service,
            account,
            "Looking up secret in Linux Secret Service"
        );
        let keyring = self.keyring().await?;
        let attrs = Self::attributes(service, account);
        let items = keyring.search_items(&attrs).await.map_err(|err| {
            tracing::error!(service, account, error = %err, "Failed searching Linux keyring");
            KeyringError::platform("search", err)
        })?;

        let Some(item) = items.into_iter().next() else {
            tracing::trace!(service, account, "No matching item found in Linux keyring");
            return Ok(None);
        };

        let secret = item
            .secret()
            .await
            .map_err(|err| {
                tracing::error!(service, account, error = %err, "Failed reading secret from Linux keyring item");
                KeyringError::platform("read", err)
            })?;
        let text =
            std::str::from_utf8(secret.as_bytes()).map_err(|_| KeyringError::InvalidData {
                reason: "secret payload is not valid UTF-8",
            })?;

        tracing::trace!(
            service,
            account,
            "Successfully retrieved secret from Linux keyring"
        );
        Ok(Some(SecretString::new(text)))
    }

    async fn set_secret(
        &self,
        service: &str,
        account: &str,
        secret: &str,
    ) -> Result<(), KeyringError> {
        tracing::info!(service, account, "Storing secret in Linux Secret Service");
        let keyring = self.keyring().await?;
        let label = format!("{service}:{account}");
        let attrs = Self::attributes(service, account);

        keyring
            .create_item(&label, &attrs, Secret::text(secret), true)
            .await
            .map_err(|err| {
                tracing::error!(service, account, error = %err, "Failed writing secret to Linux keyring");
                KeyringError::platform("write", err)
            })?;

        Ok(())
    }

    async fn delete_secret(&self, service: &str, account: &str) -> Result<(), KeyringError> {
        tracing::info!(
            service,
            account,
            "Deleting secret from Linux Secret Service"
        );
        let keyring = self.keyring().await?;
        let attrs = Self::attributes(service, account);
        let items = keyring
            .search_items(&attrs)
            .await
            .map_err(|err| {
                tracing::error!(service, account, error = %err, "Failed searching Linux keyring for deletion");
                KeyringError::platform("search", err)
            })?;

        for item in items {
            item.delete()
                .await
                .map_err(|err| {
                    tracing::error!(service, account, error = %err, "Failed deleting item from Linux keyring");
                    KeyringError::platform("delete", err)
                })?;
        }

        Ok(())
    }
}
