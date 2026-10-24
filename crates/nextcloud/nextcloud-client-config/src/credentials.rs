#[cfg(feature = "keyring")]
use sdk_keyring::{KeyringBackend, SecretString};

use crate::account::DesktopClientAccount;
use crate::error::ClientConfigError;

/// Looks up stored credentials in the platform keyring for a desktop client account.
///
/// Tries standard desktop client service names (`"Nextcloud"`, `"ownCloud"`) and
/// candidate account key formats.
#[cfg(feature = "keyring")]
pub async fn find_account_credentials<K: KeyringBackend>(
    account: &DesktopClientAccount,
    keyring: &K,
) -> Result<Option<SecretString>, ClientConfigError> {
    const SERVICES: [&str; 2] = ["Nextcloud", "ownCloud"];
    let keys = account.candidate_keychain_keys();

    for service in SERVICES {
        for key in &keys {
            if let Some(secret) = keyring
                .get_secret(service, key)
                .await
                .map_err(|err| ClientConfigError::Keyring { source: err })?
            {
                return Ok(Some(secret));
            }
        }
    }

    Ok(None)
}

#[cfg(all(test, feature = "keyring"))]
mod tests {
    use sdk_core::ServerUrl;
    use sdk_keyring::MemoryKeyring;

    use super::*;
    use crate::account::AuthType;

    #[tokio::test]
    async fn resolves_credentials_from_keyring() {
        let account = DesktopClientAccount {
            index: 0,
            server_url: ServerUrl::parse("https://cloud.example.com").unwrap(),
            username: "admin".to_string(),
            dav_user: None,
            display_name: None,
            auth_type: AuthType::Webflow,
            server_version: None,
            webflow_user: None,
        };

        let keyring = MemoryKeyring::new();
        assert!(
            find_account_credentials(&account, &keyring)
                .await
                .unwrap()
                .is_none()
        );

        keyring
            .set_secret(
                "Nextcloud",
                "https://cloud.example.com:admin",
                "app-token-xyz",
            )
            .await
            .unwrap();

        let found = find_account_credentials(&account, &keyring).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().expose_secret(), "app-token-xyz");
    }
}
