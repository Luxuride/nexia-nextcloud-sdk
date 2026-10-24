//! Nextcloud account discovered via GNOME Online Accounts.

use std::fmt;

use nextcloud_auth::BasicAuth;
use sdk_core::ServerUrl;

/// A Nextcloud or ownCloud account discovered via GNOME Online Accounts.
///
/// This type holds account metadata along with assembled [`BasicAuth`] credentials
/// retrieved from the desktop session's secure credential store.
#[derive(Clone)]
pub struct GoaAccount {
    id: String,
    username: String,
    server_url: ServerUrl,
    auth: BasicAuth,
    presentation_identity: String,
}

impl GoaAccount {
    /// Creates a new [`GoaAccount`].
    pub fn new(
        id: impl Into<String>,
        username: impl Into<String>,
        server_url: ServerUrl,
        auth: BasicAuth,
        presentation_identity: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            username: username.into(),
            server_url,
            auth,
            presentation_identity: presentation_identity.into(),
        }
    }

    /// The GNOME Online Accounts internal identifier (e.g. `account_1234`).
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The user name associated with the account.
    pub fn username(&self) -> &str {
        &self.username
    }

    /// The base URL of the Nextcloud instance.
    pub fn server_url(&self) -> &ServerUrl {
        &self.server_url
    }

    /// The basic authentication credentials for API requests.
    pub fn auth(&self) -> &BasicAuth {
        &self.auth
    }

    /// Presentation identity suitable for displaying in a user interface.
    pub fn presentation_identity(&self) -> &str {
        &self.presentation_identity
    }
}

impl fmt::Debug for GoaAccount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GoaAccount")
            .field("id", &self.id)
            .field("username", &self.username)
            .field("server_url", &self.server_url)
            .field("auth", &"[REDACTED]")
            .field("presentation_identity", &self.presentation_identity)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use nextcloud_auth::{AppPassword, BasicAuth, SecretString};
    use sdk_core::ServerUrl;

    use super::GoaAccount;

    fn sample_account() -> GoaAccount {
        let secret = SecretString::new("super-secret-password".to_owned());
        let creds = AppPassword::new("alice", secret);
        let auth = BasicAuth::from_app_password(creds);
        let url = ServerUrl::parse("https://cloud.example.com").unwrap();

        GoaAccount::new(
            "account_1234",
            "alice",
            url,
            auth,
            "alice@cloud.example.com",
        )
    }

    #[test]
    fn getters_return_expected_values() {
        let account = sample_account();

        assert_eq!(account.id(), "account_1234");
        assert_eq!(account.username(), "alice");
        assert_eq!(account.server_url().as_str(), "https://cloud.example.com");
        assert_eq!(account.presentation_identity(), "alice@cloud.example.com");
        assert_eq!(account.auth().credentials().user(), "alice");
    }

    #[test]
    fn debug_redacts_auth_and_secrets() {
        let account = sample_account();
        let debug_repr = format!("{account:?}");

        assert!(debug_repr.contains("account_1234"));
        assert!(debug_repr.contains("alice"));
        assert!(debug_repr.contains("https://cloud.example.com"));
        assert!(debug_repr.contains("alice@cloud.example.com"));
        assert!(debug_repr.contains("[REDACTED]"));

        // Ensure password plaintext or basic auth base64 never appears
        assert!(!debug_repr.contains("super-secret-password"));
        assert!(!debug_repr.contains("Basic "));
    }
}
