use std::error::Error;
use std::fmt;

/// Failures encountered during desktop account discovery or credential resolution.
#[derive(Debug)]
#[non_exhaustive]
pub enum SsoError {
    /// GNOME Online Accounts error.
    Goa(nextcloud_goa::GoaError),
    /// KDE Plasma KAccounts error.
    KAccounts(nextcloud_kaccounts::KAccountsError),
    /// Nextcloud desktop client config error.
    ClientConfig(nextcloud_client_config::ClientConfigError),
    /// Keyring error.
    Keyring(sdk_keyring::KeyringError),
}

impl fmt::Display for SsoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Goa(err) => write!(f, "GNOME Online Accounts query failed: {err}"),
            Self::KAccounts(err) => write!(f, "KDE KAccounts query failed: {err}"),
            Self::ClientConfig(err) => write!(f, "desktop client config query failed: {err}"),
            Self::Keyring(err) => write!(f, "keyring credential resolution failed: {err}"),
        }
    }
}

impl Error for SsoError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Goa(err) => Some(err),
            Self::KAccounts(err) => Some(err),
            Self::ClientConfig(err) => Some(err),
            Self::Keyring(err) => Some(err),
        }
    }
}

impl From<nextcloud_goa::GoaError> for SsoError {
    fn from(err: nextcloud_goa::GoaError) -> Self {
        Self::Goa(err)
    }
}

impl From<nextcloud_kaccounts::KAccountsError> for SsoError {
    fn from(err: nextcloud_kaccounts::KAccountsError) -> Self {
        Self::KAccounts(err)
    }
}

impl From<nextcloud_client_config::ClientConfigError> for SsoError {
    fn from(err: nextcloud_client_config::ClientConfigError) -> Self {
        Self::ClientConfig(err)
    }
}

impl From<sdk_keyring::KeyringError> for SsoError {
    fn from(err: sdk_keyring::KeyringError) -> Self {
        Self::Keyring(err)
    }
}
