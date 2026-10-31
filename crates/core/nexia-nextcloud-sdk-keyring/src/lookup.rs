//! Ordered credential lookup across several service and account name conventions.

use crate::error::KeyringError;
use crate::secret::SecretString;
use crate::traits::KeyringBackend;

/// Returns the first secret stored for any of `services` × `keys`, in iteration order.
///
/// Which service and account names a desktop environment uses differs between KAccounts,
/// Accounts-SSO and the official clients, so a lookup has to try all of them. Keeping the loops
/// here keeps the ordering — and with it the precedence between conventions — identical in every
/// place that resolves credentials, instead of copying the loop into each caller.
///
/// # Errors
/// Returns the first [`KeyringError`] a backend reports; an unavailable keyring is not the same
/// answer as an absent secret.
pub async fn find_first_secret<K: KeyringBackend>(
    keyring: &K,
    services: &[&str],
    keys: &[String],
) -> Result<Option<SecretString>, KeyringError> {
    for service in services {
        for key in keys {
            if let Some(secret) = keyring.get_secret(service, key).await? {
                return Ok(Some(secret));
            }
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use crate::memory::MemoryKeyring;

    use super::*;

    #[tokio::test]
    async fn a_missing_secret_is_not_an_error() {
        let keyring = MemoryKeyring::new();
        let keys = vec!["a".to_string()];

        assert!(
            find_first_secret(&keyring, &["Nextcloud"], &keys)
                .await
                .expect("memory keyring succeeds")
                .is_none()
        );
    }

    #[tokio::test]
    async fn services_are_tried_in_order_before_account_names() {
        let keyring = MemoryKeyring::new();
        keyring
            .set_secret("ownCloud", "second", "from-owncloud")
            .await
            .expect("memory keyring stores");
        keyring
            .set_secret("Nextcloud", "first", "from-nextcloud")
            .await
            .expect("memory keyring stores");

        let keys = vec!["first".to_string(), "second".to_string()];
        let found = find_first_secret(&keyring, &["Nextcloud", "ownCloud"], &keys)
            .await
            .expect("memory keyring succeeds")
            .expect("a secret is found");

        assert_eq!(found.expose_secret(), "from-nextcloud");
    }
}
