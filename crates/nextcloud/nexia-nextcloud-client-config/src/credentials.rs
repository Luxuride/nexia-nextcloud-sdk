#[cfg(feature = "keyring")]
use nexia_nextcloud_sdk_keyring::{KeyringBackend, SecretString, find_first_secret};

use crate::account::DesktopClientAccount;
use crate::error::ClientConfigError;

/// Looks up stored credentials in the platform keyring for a desktop client account.
///
/// Tries the standard desktop client service names (`"Nextcloud"`, `"ownCloud"`) against every
/// account name format the client has used.
#[cfg(feature = "keyring")]
pub async fn find_account_credentials<K: KeyringBackend>(
    account: &DesktopClientAccount,
    keyring: &K,
) -> Result<Option<SecretString>, ClientConfigError> {
    const SERVICES: [&str; 2] = ["Nextcloud", "ownCloud"];
    let keys = account.candidate_keychain_keys();

    find_first_secret(keyring, &SERVICES, &keys)
        .await
        .map_err(|source| ClientConfigError::Keyring { source })
}

#[cfg(all(test, feature = "keyring"))]
mod tests {
    use nexia_nextcloud_core::ServerUrl;
    use nexia_nextcloud_sdk_keyring::MemoryKeyring;

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
