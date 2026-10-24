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
        let keyring = self.keyring().await?;
        let attrs = Self::attributes(service, account);
        let items = keyring
            .search_items(&attrs)
            .await
            .map_err(|err| KeyringError::platform("search", err))?;

        let Some(item) = items.into_iter().next() else {
            return Ok(None);
        };

        let secret = item
            .secret()
            .await
            .map_err(|err| KeyringError::platform("read", err))?;
        let text =
            std::str::from_utf8(secret.as_bytes()).map_err(|_| KeyringError::InvalidData {
                reason: "secret payload is not valid UTF-8",
            })?;

        Ok(Some(SecretString::new(text)))
    }

    async fn set_secret(
        &self,
        service: &str,
        account: &str,
        secret: &str,
    ) -> Result<(), KeyringError> {
        let keyring = self.keyring().await?;
        let label = format!("{service}:{account}");
        let attrs = Self::attributes(service, account);

        keyring
            .create_item(&label, &attrs, Secret::text(secret), true)
            .await
            .map_err(|err| KeyringError::platform("write", err))?;

        Ok(())
    }

    async fn delete_secret(&self, service: &str, account: &str) -> Result<(), KeyringError> {
        let keyring = self.keyring().await?;
        let attrs = Self::attributes(service, account);
        let items = keyring
            .search_items(&attrs)
            .await
            .map_err(|err| KeyringError::platform("search", err))?;

        for item in items {
            item.delete()
                .await
                .map_err(|err| KeyringError::platform("delete", err))?;
        }

        Ok(())
    }
}
