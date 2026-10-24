use std::fmt;

use nextcloud_sdk_core::{AuthProvider, Headers};

use super::BearerToken;

/// Applies an OAuth2 access token as an `Authorization: Bearer` header.
#[derive(Clone)]
pub struct BearerAuth {
    token: BearerToken,
}

impl BearerAuth {
    /// Builds a provider from a token string.
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: BearerToken::new(token),
        }
    }

    /// Builds a provider from an already-wrapped token.
    pub const fn from_token(token: BearerToken) -> Self {
        Self { token }
    }

    /// The token this provider applies.
    pub const fn token(&self) -> &BearerToken {
        &self.token
    }

    /// The header value sent to the server.
    pub fn header_value(&self) -> String {
        format!("Bearer {}", self.token.expose())
    }
}

impl AuthProvider for BearerAuth {
    fn authenticate(&self, headers: &mut Headers) {
        headers.set("Authorization", self.header_value());
    }
}

impl fmt::Debug for BearerAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BearerAuth")
            .field("token", &self.token)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use nextcloud_sdk_core::{AuthProvider, Header, Headers};

    use super::BearerAuth;

    fn bearer() -> BearerAuth {
        BearerAuth::new("access-token")
    }

    #[test]
    fn header_value_prefixes_the_token_with_bearer() {
        assert_eq!(bearer().header_value(), "Bearer access-token");
    }

    #[test]
    fn authenticate_sets_the_authorization_header() {
        let mut headers = Headers::new();
        bearer().authenticate(&mut headers);
        assert_eq!(headers.get("Authorization"), Some("Bearer access-token"));
    }

    #[test]
    fn authenticate_replaces_an_existing_authorization_header() {
        let mut headers = Headers::new().with(Header::new("Authorization", "Basic c3RhbGU="));
        bearer().authenticate(&mut headers);
        assert_eq!(headers.len(), 1);
        assert_eq!(headers.get("Authorization"), Some("Bearer access-token"));
    }

    #[test]
    fn debug_redacts_the_token() {
        let rendered = format!("{:?}", bearer());
        assert!(!rendered.contains("access-token"));
        assert!(rendered.contains("<redacted>"));
    }
}
