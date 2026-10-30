use std::collections::HashSet;

use nexia_nextcloud_sdk_keyring::{KeyringBackend, SecretString};

use crate::sso::account::{AccountSource, DiscoveredAccount};
use crate::sso::error::SsoError;

/// Aggregated desktop Single-Sign-On discovery engine.
///
/// Queries GNOME Online Accounts, KDE Plasma KAccounts, and official Nextcloud
/// desktop client profiles to find existing authenticated accounts on the system.
#[derive(Clone, Debug, Default)]
pub struct DesktopSso;

fn canonical_key(server_url: &nexia_nextcloud_core::ServerUrl, username: &str) -> (String, String) {
    let auth = server_url.authority();
    let host = auth.split(':').next().unwrap_or(auth);
    let resolved = std::net::ToSocketAddrs::to_socket_addrs(&(host, 443))
        .ok()
        .and_then(|mut iter| iter.next())
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|| auth.to_string());
    (resolved, username.to_ascii_lowercase())
}

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
        tracing::info!("Starting desktop SSO account discovery across subsystems");
        let mut results: Vec<DiscoveredAccount> = Vec::new();
        let mut seen = HashSet::new();

        // 1. GNOME Online Accounts (via D-Bus session bus)
        if let Ok(goa) = nexia_nextcloud_goa::GnomeOnlineAccounts::connect().await
            && let Ok(accounts) = goa.list_accounts().await
        {
            for a in accounts {
                let key = canonical_key(a.server_url(), a.username());
                let secret = SecretString::new(a.auth().credentials().password().expose());
                if seen.insert(key.clone()) {
                    results.push(DiscoveredAccount {
                        source: AccountSource::GnomeOnlineAccounts,
                        server_url: a.server_url().clone(),
                        username: a.username().to_string(),
                        display_name: Some(a.presentation_identity().to_string()),
                        dav_user: None,
                        password: Some(secret),
                    });
                } else if let Some(existing) = results
                    .iter_mut()
                    .find(|r| canonical_key(&r.server_url, &r.username) == key)
                    && existing.password.is_none()
                {
                    existing.password = Some(secret);
                }
            }
        }

        // 2. KDE Plasma KAccounts (Accounts-SSO SQLite database)
        if let Ok(db) = nexia_nextcloud_kaccounts::KAccountsDatabase::open_default()
            && let Ok(accounts) = db.list_accounts()
        {
            for a in accounts {
                let key = canonical_key(&a.server_url, &a.username);
                if seen.insert(key) {
                    results.push(DiscoveredAccount {
                        source: AccountSource::KdeKAccounts,
                        server_url: a.server_url,
                        username: a.username,
                        display_name: Some(a.name),
                        dav_user: None,
                        password: None,
                    });
                }
            }
        }

        // 3. Upstream desktop client configurations
        for path in nexia_nextcloud_client_config::discover_config_paths() {
            if path.is_file()
                && let Ok(accounts) =
                    nexia_nextcloud_client_config::ClientConfigParser::parse_file(&path)
            {
                for a in accounts {
                    let key = canonical_key(&a.server_url, &a.username);
                    if seen.insert(key) {
                        results.push(DiscoveredAccount {
                            source: AccountSource::DesktopClient,
                            server_url: a.server_url,
                            username: a.username,
                            display_name: a.display_name,
                            dav_user: a.dav_user,
                            password: None,
                        });
                    }
                }
            }
        }

        tracing::info!(
            total = results.len(),
            "Finished desktop SSO account discovery"
        );
        results
    }

    /// Attempts to look up stored credentials for a discovered account using the platform keyring.
    pub async fn resolve_credentials<K: KeyringBackend>(
        &self,
        account: &DiscoveredAccount,
        keyring: &K,
    ) -> Result<Option<SecretString>, SsoError> {
        if let Some(ref secret) = account.password {
            return Ok(Some(secret.clone()));
        }

        tracing::debug!(
            server = %account.server_url,
            user = %account.username,
            "Attempting to resolve SSO credentials from platform keyring"
        );
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
    use nexia_nextcloud_core::ServerUrl;
    use nexia_nextcloud_sdk_keyring::MemoryKeyring;

    use super::*;

    #[tokio::test]
    async fn resolves_credentials_for_discovered_account() {
        let account = DiscoveredAccount {
            source: AccountSource::DesktopClient,
            server_url: ServerUrl::parse("https://cloud.example.com").unwrap(),
            username: "alice".to_string(),
            display_name: Some("Alice Example".to_string()),
            dav_user: None,
            password: None,
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

    #[tokio::test]
    async fn resolves_prepopulated_password_without_keyring() {
        let account = DiscoveredAccount {
            source: AccountSource::GnomeOnlineAccounts,
            server_url: ServerUrl::parse("https://cloud.example.com").unwrap(),
            username: "bob".to_string(),
            display_name: None,
            dav_user: None,
            password: Some(SecretString::new("pre-resolved-secret")),
        };

        let keyring = MemoryKeyring::new();
        let sso = DesktopSso::new();

        let resolved = sso.resolve_credentials(&account, &keyring).await.unwrap();
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap().expose_secret(), "pre-resolved-secret");
    }
}
