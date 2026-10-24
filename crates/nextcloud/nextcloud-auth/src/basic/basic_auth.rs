use std::fmt;

use base64::Engine as _;
use base64::prelude::BASE64_STANDARD;
use sdk_core::{AuthProvider, Headers};

use super::AppPassword;
use crate::SecretString;

/// Applies [`AppPassword`] credentials as an `Authorization: Basic` header.
///
/// The value is `base64(user:password)`; the colon separator is fixed by
/// RFC 7617, so neither field may contain one — callers use the URL-encoded
/// Nextcloud user name, which never does.
#[derive(Clone)]
pub struct BasicAuth {
    credentials: AppPassword,
}

impl BasicAuth {
    /// Builds a provider from a user name and its app password.
    pub fn new(user: impl Into<String>, password: SecretString) -> Self {
        Self {
            credentials: AppPassword::new(user, password),
        }
    }

    /// Builds a provider from already-assembled credentials.
    pub const fn from_app_password(credentials: AppPassword) -> Self {
        Self { credentials }
    }

    /// The credentials this provider applies.
    pub const fn credentials(&self) -> &AppPassword {
        &self.credentials
    }

    /// The header value sent to the server.
    pub fn header_value(&self) -> String {
        let encoded = BASE64_STANDARD.encode(self.encoded_pair());
        format!("Basic {encoded}")
    }

    fn encoded_pair(&self) -> Vec<u8> {
        let mut pair = Vec::with_capacity(self.credentials.user().len() + 1);
        pair.extend_from_slice(self.credentials.user().as_bytes());
        pair.push(b':');
        pair.extend_from_slice(self.credentials.password().expose().as_bytes());
        pair
    }
}

impl AuthProvider for BasicAuth {
    fn authenticate(&self, headers: &mut Headers) {
        headers.set("Authorization", self.header_value());
    }
}

impl fmt::Debug for BasicAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BasicAuth")
            .field("credentials", &self.credentials)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use base64::Engine as _;
    use base64::prelude::BASE64_STANDARD;
    use sdk_core::{AuthProvider, Headers};

    use super::BasicAuth;
    use crate::SecretString;

    fn basic_auth() -> BasicAuth {
        BasicAuth::new("alice", SecretString::new("app-password"))
    }

    #[test]
    fn header_value_matches_independent_encoding() {
        let provider = BasicAuth::new("alice", SecretString::new("app-password"));
        let expected = BASE64_STANDARD.encode(b"alice:app-password");
        assert_eq!(provider.header_value(), format!("Basic {expected}"));
    }

    #[test]
    fn authenticate_sets_the_authorization_header() {
        let mut headers = Headers::new();
        basic_auth().authenticate(&mut headers);
        assert_eq!(
            headers.get("Authorization"),
            Some("Basic YWxpY2U6YXBwLXBhc3N3b3Jk")
        );
    }

    #[test]
    fn authenticate_replaces_an_existing_authorization_header() {
        let mut headers = Headers::new().with(sdk_core::Header::new("authorization", "stale"));
        basic_auth().authenticate(&mut headers);
        assert_eq!(headers.len(), 1);
        assert_eq!(
            headers.get("Authorization"),
            Some("Basic YWxpY2U6YXBwLXBhc3N3b3Jk")
        );
    }

    #[test]
    fn debug_redacts_both_the_password_and_the_encoded_pair() {
        let rendered = format!("{:?}", basic_auth());
        assert!(!rendered.contains("app-password"));
        assert!(!rendered.contains("YWxpY2U6YXBwLXBhc3N3b3Jk"));
        assert!(rendered.contains("<redacted>"));
    }
}
