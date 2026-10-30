use std::future::Future;

use crate::error::KeyringError;
use crate::secret::SecretString;

/// Behavior implemented by keyring storage backends across supported platforms.
pub trait KeyringBackend: Send + Sync {
    /// Retrieves a secret for `service` and `account`.
    fn get_secret(
        &self,
        service: &str,
        account: &str,
    ) -> impl Future<Output = Result<Option<SecretString>, KeyringError>> + Send;

    /// Stores a `secret` for `service` and `account`.
    fn set_secret(
        &self,
        service: &str,
        account: &str,
        secret: &str,
    ) -> impl Future<Output = Result<(), KeyringError>> + Send;

    /// Deletes the secret for `service` and `account`.
    fn delete_secret(
        &self,
        service: &str,
        account: &str,
    ) -> impl Future<Output = Result<(), KeyringError>> + Send;
}
