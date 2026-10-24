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
        let entry = keyring::Entry::new(service, account)
            .map_err(|err| KeyringError::platform("init", err))?;

        match entry.get_password() {
            Ok(password) => Ok(Some(SecretString::new(password))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(KeyringError::platform("get_password", err)),
        }
    }

    async fn set_secret(
        &self,
        service: &str,
        account: &str,
        secret: &str,
    ) -> Result<(), KeyringError> {
        let entry = keyring::Entry::new(service, account)
            .map_err(|err| KeyringError::platform("init", err))?;

        entry
            .set_password(secret)
            .map_err(|err| KeyringError::platform("set_password", err))?;

        Ok(())
    }

    async fn delete_secret(&self, service: &str, account: &str) -> Result<(), KeyringError> {
        let entry = keyring::Entry::new(service, account)
            .map_err(|err| KeyringError::platform("init", err))?;

        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(KeyringError::platform("delete_credential", err)),
        }
    }
}
