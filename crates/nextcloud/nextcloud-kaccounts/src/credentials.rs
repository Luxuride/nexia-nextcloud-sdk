#[cfg(feature = "keyring")]
use nextcloud_sdk_keyring::{KeyringBackend, SecretString};

use crate::account::KAccount;
use crate::error::KAccountsError;

/// Looks up stored credentials in the platform keyring for a KDE Plasma KAccount.
#[cfg(feature = "keyring")]
pub async fn find_kaccount_credentials<K: KeyringBackend>(
    account: &KAccount,
    keyring: &K,
) -> Result<Option<SecretString>, KAccountsError> {
    const SERVICES: [&str; 4] = ["KAccounts", "signon", "Nextcloud", "ownCloud"];
    let keys = account.candidate_keychain_keys();

    for service in SERVICES {
        for key in &keys {
            if let Some(secret) = keyring
                .get_secret(service, key)
                .await
                .map_err(|err| KAccountsError::Keyring { source: err })?
            {
                return Ok(Some(secret));
            }
        }
    }

    Ok(None)
}

#[cfg(all(test, feature = "keyring"))]
mod tests {
    use nexia_nextcloud_core::ServerUrl;
    use nextcloud_sdk_keyring::MemoryKeyring;

    use super::*;

    #[tokio::test]
    async fn resolves_kaccount_secret_from_keyring() {
        let account = KAccount {
            id: 42,
            name: "KDE Cloud".to_string(),
            provider: "nextcloud".to_string(),
            server_url: ServerUrl::parse("https://cloud.example.com").unwrap(),
            username: "kdeuser".to_string(),
            enabled: true,
            services: vec![],
        };

        let keyring = MemoryKeyring::new();
        assert!(
            find_kaccount_credentials(&account, &keyring)
                .await
                .unwrap()
                .is_none()
        );

        keyring
            .set_secret("KAccounts", "kaccounts:42", "token-from-kwallet")
            .await
            .unwrap();

        let found = find_kaccount_credentials(&account, &keyring).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().expose_secret(), "token-from-kwallet");
    }
}
