use crate::error::KeyringError;
use crate::secret::SecretString;
use crate::traits::KeyringBackend;

/// Desktop keyring backend for macOS (Apple Keychain) and Windows (Credential Manager).
#[derive(Clone, Debug, Default)]
pub struct DesktopKeyring;

impl DesktopKeyring {
    /// Creates a new desktop keyring client.
    pub fn new() -> Self {
        Self
    }
}

impl KeyringBackend for DesktopKeyring {
    async fn get_secret(
        &self,
        service: &str,
        account: &str,
    ) -> Result<Option<SecretString>, KeyringError> {
        tracing::debug!(service, account, "Looking up secret in desktop keyring");
        let entry = keyring::Entry::new(service, account)
            .map_err(|err| KeyringError::platform("init", err))?;

        match entry.get_password() {
            Ok(password) => {
                tracing::trace!(service, account, "Desktop keyring lookup hit");
                Ok(Some(SecretString::new(password)))
            }
            Err(keyring::Error::NoEntry) => {
                tracing::trace!(service, account, "Desktop keyring entry not found");
                Ok(None)
            }
            Err(err) => {
                tracing::error!(service, account, error = %err, "Failed getting password from desktop keyring");
                Err(KeyringError::platform("get_password", err))
            }
        }
    }

    async fn set_secret(
        &self,
        service: &str,
        account: &str,
        secret: &str,
    ) -> Result<(), KeyringError> {
        tracing::info!(service, account, "Storing secret in desktop keyring");
        let entry = keyring::Entry::new(service, account)
            .map_err(|err| KeyringError::platform("init", err))?;

        entry
            .set_password(secret)
            .map_err(|err| {
                tracing::error!(service, account, error = %err, "Failed setting password in desktop keyring");
                KeyringError::platform("set_password", err)
            })?;

        Ok(())
    }

    async fn delete_secret(&self, service: &str, account: &str) -> Result<(), KeyringError> {
        tracing::info!(service, account, "Deleting secret from desktop keyring");
        let entry = keyring::Entry::new(service, account)
            .map_err(|err| KeyringError::platform("init", err))?;

        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => {
                tracing::error!(service, account, error = %err, "Failed deleting credential from desktop keyring");
                Err(KeyringError::platform("delete_credential", err))
            }
        }
    }
}
