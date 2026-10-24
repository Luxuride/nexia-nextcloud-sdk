use std::collections::HashSet;

use sdk_keyring::{KeyringBackend, SecretString};

use crate::sso::account::{AccountSource, DiscoveredAccount};
use crate::sso::error::SsoError;

/// Aggregated desktop Single-Sign-On discovery engine.
///
/// Queries GNOME Online Accounts, KDE Plasma KAccounts, and official Nextcloud
/// desktop client profiles to find existing authenticated accounts on the system.
#[derive(Clone, Debug, Default)]
pub struct DesktopSso;

impl DesktopSso {
    /// Creates a new SSO discovery coordinator.
    pub fn new() -> Self {
        Self
    }

    /// Discovers all available Nextcloud accounts across all supported desktop subsystems.
    ///
    /// Results are automatically deduplicated by `(server_url, username)`. Subsystem query
    /// failures (e.g. D-Bus unavailable, no KAccounts database) are ignored so discovery
    /// continues gracefully.
    pub async fn discover_accounts(&self) -> Vec<DiscoveredAccount> {
        let mut results = Vec::new();
        let mut seen = HashSet::new();

        // 1. GNOME Online Accounts (via D-Bus session bus)
        if let Ok(goa) = nextcloud_goa::GnomeOnlineAccounts::connect().await
            && let Ok(accounts) = goa.list_accounts().await
        {
            for a in accounts {
                let key = (
                    a.server_url().as_str().to_string(),
                    a.username().to_string(),
                );
                if seen.insert(key) {
                    results.push(DiscoveredAccount {
                        source: AccountSource::GnomeOnlineAccounts,
                        server_url: a.server_url().clone(),
                        username: a.username().to_string(),
                        display_name: None,
                        dav_user: None,
                    });
                }
            }
        }

        // 2. KDE Plasma KAccounts (Accounts-SSO SQLite database)
        if let Ok(db) = nextcloud_kaccounts::KAccountsDatabase::open_default()
            && let Ok(accounts) = db.list_accounts()
        {
            for a in accounts {
                let key = (a.server_url.as_str().to_string(), a.username.clone());
                if seen.insert(key) {
                    results.push(DiscoveredAccount {
                        source: AccountSource::KdeKAccounts,
                        server_url: a.server_url,
                        username: a.username,
                        display_name: Some(a.name),
                        dav_user: None,
                    });
                }
            }
        }

        // 3. Official Nextcloud Desktop Client configurations
        for path in nextcloud_client_config::discover_config_paths() {
            if path.is_file()
                && let Ok(accounts) = nextcloud_client_config::ClientConfigParser::parse_file(&path)
            {
                for a in accounts {
                    let key = (a.server_url.as_str().to_string(), a.username.clone());
                    if seen.insert(key) {
                        results.push(DiscoveredAccount {
                            source: AccountSource::DesktopClient,
                            server_url: a.server_url,
                            username: a.username,
                            display_name: a.display_name,
                            dav_user: a.dav_user,
                        });
                    }
                }
            }
        }

        results
    }

    /// Attempts to look up stored credentials for a discovered account using the platform keyring.
    pub async fn resolve_credentials<K: KeyringBackend>(
        &self,
        account: &DiscoveredAccount,
        keyring: &K,
    ) -> Result<Option<SecretString>, SsoError> {
        const SERVICES: [&str; 4] = ["Nextcloud", "ownCloud", "KAccounts", "signon"];
        let keys = account.candidate_keychain_keys();

        for service in SERVICES {
            for key in &keys {
                if let Some(secret) = keyring.get_secret(service, key).await? {
                    return Ok(Some(secret));
                }
            }
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use sdk_core::ServerUrl;
    use sdk_keyring::MemoryKeyring;

    use super::*;

    #[tokio::test]
    async fn resolves_credentials_for_discovered_account() {
        let account = DiscoveredAccount {
            source: AccountSource::DesktopClient,
            server_url: ServerUrl::parse("https://cloud.example.com").unwrap(),
            username: "alice".to_string(),
            display_name: Some("Alice Example".to_string()),
            dav_user: None,
        };

        let keyring = MemoryKeyring::new();
        let sso = DesktopSso::new();

        assert!(
            sso.resolve_credentials(&account, &keyring)
                .await
                .unwrap()
                .is_none()
        );

        keyring
            .set_secret(
                "Nextcloud",
                "https://cloud.example.com:alice",
                "my-app-password",
            )
            .await
            .unwrap();

        let resolved = sso.resolve_credentials(&account, &keyring).await.unwrap();
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap().expose_secret(), "my-app-password");
    }
}
