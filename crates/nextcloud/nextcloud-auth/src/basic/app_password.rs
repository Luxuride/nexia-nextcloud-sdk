use std::fmt;

use crate::SecretString;

/// A Nextcloud user name paired with an app password.
///
/// Nextcloud recommends app passwords over the account password for clients:
/// they can be revoked individually and never expose the primary login.
#[derive(Clone)]
pub struct AppPassword {
    user: String,
    password: SecretString,
}

impl AppPassword {
    /// Builds credentials from a user name and its app password.
    pub fn new(user: impl Into<String>, password: SecretString) -> Self {
        Self {
            user: user.into(),
            password,
        }
    }

    /// The user name these credentials belong to.
    pub fn user(&self) -> &str {
        &self.user
    }

    /// The app password.
    ///
    /// Reading the plaintext is explicit so no accidental `Debug` or log line
    /// can pick it up.
    pub fn password(&self) -> &SecretString {
        &self.password
    }
}

impl fmt::Debug for AppPassword {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppPassword")
            .field("user", &self.user)
            .field("password", &self.password)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::AppPassword;
    use crate::SecretString;

    #[test]
    fn debug_redacts_the_password() {
        let credentials = AppPassword::new("alice", SecretString::new("s3cr3t-app-password"));
        let rendered = format!("{credentials:?}");
        assert!(!rendered.contains("s3cr3t-app-password"));
        assert!(rendered.contains("<redacted>"));
        assert!(rendered.contains("alice"));
    }

    #[test]
    fn exposes_user_and_password() {
        let credentials = AppPassword::new("alice", SecretString::new("pw"));
        assert_eq!(credentials.user(), "alice");
        assert_eq!(credentials.password().expose(), "pw");
    }
}
