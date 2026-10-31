use nexia_nextcloud_core::ServerUrl;
use nexia_nextcloud_sdk_keyring::SecretString;

/// The origin or desktop subsystem where a Nextcloud account was discovered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountSource {
    /// GNOME Online Accounts (via D-Bus).
    GnomeOnlineAccounts,
    /// KDE Plasma Online Accounts (KAccounts / Accounts-SSO).
    KdeKAccounts,
    /// Official Nextcloud desktop client profile (`nextcloud.cfg`).
    DesktopClient,
}

/// A unified Nextcloud account discovered across desktop environments and systems.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveredAccount {
    /// The subsystem where this account was found.
    pub source: AccountSource,
    /// Target server URL.
    pub server_url: ServerUrl,
    /// User account name.
    pub username: String,
    /// User display name, if provided by the desktop system.
    pub display_name: Option<String>,
    /// WebDAV user name, if different from `username`.
    pub dav_user: Option<String>,
    /// Secure credential secret, if resolved during discovery.
    pub password: Option<SecretString>,
}

impl DiscoveredAccount {
    /// Creates a new discovered account without credentials.
    pub fn new(source: AccountSource, server_url: ServerUrl, username: impl Into<String>) -> Self {
        Self {
            source,
            server_url,
            username: username.into(),
            display_name: None,
            dav_user: None,
            password: None,
        }
    }

    /// Attaches an already-resolved credential secret to this discovered account.
    #[must_use]
    pub fn with_password(mut self, password: SecretString) -> Self {
        self.password = Some(password);
        self
    }

    /// Returns the WebDAV username or falls back to `username`.
    pub fn dav_username(&self) -> &str {
        self.dav_user.as_deref().unwrap_or(&self.username)
    }

    /// Generates candidate keyring keys across standard desktop services.
    ///
    /// A bare server URL is deliberately not a candidate: it names no user, so on a server that
    /// hosts several accounts it would hand this account the token stored for another one.
    pub fn candidate_keychain_keys(&self) -> Vec<String> {
        let base = self.server_url.as_str().trim_end_matches('/');
        let user = &self.username;
        let dav_user = self.dav_username();

        vec![
            format!("{base}:{user}"),
            format!("{base}/remote.php/dav/files/{dav_user}/:{user}"),
            format!("{user}@{base}"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account() -> DiscoveredAccount {
        DiscoveredAccount {
            source: AccountSource::DesktopClient,
            server_url: ServerUrl::parse("https://cloud.example.com").unwrap(),
            username: "alice".to_string(),
            display_name: None,
            dav_user: None,
            password: None,
        }
    }

    #[test]
    fn every_candidate_names_the_account() {
        let keys = account().candidate_keychain_keys();

        assert_eq!(
            keys,
            vec![
                "https://cloud.example.com:alice".to_string(),
                "https://cloud.example.com/remote.php/dav/files/alice/:alice".to_string(),
                "alice@https://cloud.example.com".to_string(),
            ]
        );
        // A bare server URL would be shared by every account on that server.
        assert!(!keys.contains(&"https://cloud.example.com".to_string()));
    }
}
