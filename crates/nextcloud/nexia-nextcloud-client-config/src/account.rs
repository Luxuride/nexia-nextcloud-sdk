use nexia_nextcloud_core::ServerUrl;

/// Authentication type configured in the Nextcloud desktop client.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AuthType {
    /// Modern Webflow (OAuth2 / Login Flow v2).
    #[default]
    Webflow,
    /// HTTP basic auth or static credentials.
    Http,
    /// Shibboleth enterprise SSO.
    Shibboleth,
    /// Unrecognized authentication type.
    Unknown,
}

impl AuthType {
    /// Parses an authentication type from string.
    pub fn from_str_lossy(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "webflow" => Self::Webflow,
            "http" => Self::Http,
            "shibboleth" | "shp" => Self::Shibboleth,
            _ => Self::Unknown,
        }
    }
}

/// An account discovered from an official Nextcloud desktop client configuration file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesktopClientAccount {
    /// Zero-based numerical index of the account in the client config file.
    pub index: usize,
    /// Target server URL.
    pub server_url: ServerUrl,
    /// Configured user account name.
    pub username: String,
    /// WebDAV username, if different from `username`.
    pub dav_user: Option<String>,
    /// User display name, if recorded by the client.
    pub display_name: Option<String>,
    /// Authentication mechanism.
    pub auth_type: AuthType,
    /// Nextcloud server version string (e.g. "29.0.0.1").
    pub server_version: Option<String>,
    /// Webflow user attribute.
    pub webflow_user: Option<String>,
}

impl DesktopClientAccount {
    /// Returns the effective user name for WebDAV operations.
    pub fn dav_username(&self) -> &str {
        self.dav_user.as_deref().unwrap_or(&self.username)
    }

    /// Candidate account keys used by the official Nextcloud desktop client
    /// when saving credentials in the platform keychain.
    ///
    /// The official desktop client formats keys as:
    /// - `<url>:<username>`
    /// - `<url>/remote.php/dav/files/<user>/:<username>`
    /// - `<username>@<url>`
    ///
    /// A bare server URL is deliberately not a candidate. It names no user, so on a server that
    /// hosts several accounts it would hand this account the token stored for another one.
    pub fn candidate_keychain_keys(&self) -> Vec<String> {
        let base_url = self.server_url.as_str().trim_end_matches('/');
        let user = &self.username;
        let dav_user = self.dav_username();

        vec![
            format!("{base_url}:{user}"),
            format!("{base_url}/remote.php/dav/files/{dav_user}/:{user}"),
            format!("{user}@{base_url}"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::AuthType;
    use nexia_nextcloud_core::ServerUrl;

    fn account() -> DesktopClientAccount {
        DesktopClientAccount {
            index: 0,
            server_url: ServerUrl::parse("https://cloud.example.com").unwrap(),
            username: "alice".to_string(),
            dav_user: None,
            display_name: None,
            auth_type: AuthType::Webflow,
            server_version: None,
            webflow_user: None,
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
