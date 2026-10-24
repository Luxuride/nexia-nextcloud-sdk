use std::fmt;

use crate::SecretString;

/// An OAuth2 access token issued by a Nextcloud instance.
///
/// The token is kept behind [`SecretString`] so no `Debug` or log line can
/// reveal it; request the plaintext through [`BearerToken::expose`].
#[derive(Clone)]
pub struct BearerToken {
    secret: SecretString,
}

impl BearerToken {
    /// Wraps a token string.
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            secret: SecretString::new(token),
        }
    }

    /// Borrows the token plaintext.
    pub fn expose(&self) -> &str {
        self.secret.expose()
    }
}

impl fmt::Debug for BearerToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("BearerToken").field(&self.secret).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::BearerToken;

    #[test]
    fn debug_redacts_the_token() {
        let token = BearerToken::new("ghp-s3cr3t-access-token");
        let rendered = format!("{token:?}");
        assert!(!rendered.contains("ghp-s3cr3t-access-token"));
        assert!(rendered.contains("<redacted>"));
    }

    #[test]
    fn expose_returns_the_token() {
        assert_eq!(BearerToken::new("abc").expose(), "abc");
    }
}
