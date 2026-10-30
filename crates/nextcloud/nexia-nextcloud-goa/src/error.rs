//! Error types for GNOME Online Accounts integration.

use std::error::Error;
use std::fmt;

use nexia_nextcloud_core::UrlError;

/// Errors that can occur when discovering GNOME Online Accounts.
#[derive(Debug)]
pub enum GoaError {
    /// A D-Bus communication error.
    DBus(zbus::Error),
    /// A URL parsing error while determining the server base URL.
    Url(UrlError),
    /// No Nextcloud or ownCloud accounts were configured in GNOME Online Accounts.
    NoAccountsFound,
    /// A required D-Bus property was missing or not of expected type.
    MissingProperty(&'static str),
}

impl fmt::Display for GoaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DBus(err) => write!(f, "D-Bus operation failed: {err}"),
            Self::Url(err) => write!(f, "failed to parse server URL: {err}"),
            Self::NoAccountsFound => {
                f.write_str("no Nextcloud or ownCloud accounts found in GNOME Online Accounts")
            }
            Self::MissingProperty(prop) => {
                write!(f, "missing required D-Bus property `{prop}`")
            }
        }
    }
}

impl Error for GoaError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::DBus(err) => Some(err),
            Self::Url(err) => Some(err),
            Self::NoAccountsFound | Self::MissingProperty(_) => None,
        }
    }
}

impl From<zbus::Error> for GoaError {
    fn from(err: zbus::Error) -> Self {
        Self::DBus(err)
    }
}

impl From<UrlError> for GoaError {
    fn from(err: UrlError) -> Self {
        Self::Url(err)
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use nexia_nextcloud_core::UrlError;

    use super::GoaError;

    #[test]
    fn display_formats_all_variants() {
        assert!(
            GoaError::NoAccountsFound
                .to_string()
                .contains("no Nextcloud")
        );
        assert!(
            GoaError::MissingProperty("Id")
                .to_string()
                .contains("missing required D-Bus property `Id`")
        );
        assert!(
            GoaError::Url(UrlError::MissingScheme)
                .to_string()
                .contains("server URL has no scheme")
        );
    }

    #[test]
    fn source_returns_underlying_error() {
        let url_err = GoaError::Url(UrlError::MissingScheme);
        assert!(url_err.source().is_some());

        let missing = GoaError::MissingProperty("Id");
        assert!(missing.source().is_none());

        let no_acc = GoaError::NoAccountsFound;
        assert!(no_acc.source().is_none());
    }
}
