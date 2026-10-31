use std::error::Error;
use std::fmt;
use std::path::PathBuf;

/// Failures encountered while querying KDE Plasma KAccounts.
#[derive(Debug)]
#[non_exhaustive]
pub enum KAccountsError {
    /// The SQLite accounts database could not be opened or read.
    Database {
        /// File path of the database.
        path: PathBuf,
        /// SQLite cause.
        source: rusqlite::Error,
    },
    /// The KAccounts database path could not be resolved or found.
    NotFound {
        /// Candidate search locations.
        searched: Vec<PathBuf>,
    },
    /// URL in account settings is malformed.
    InvalidUrl {
        /// Raw URL.
        raw: String,
        /// Parse error.
        source: nexia_nextcloud_core::UrlError,
    },
    /// The database has none of the table layouts this crate can read.
    UnsupportedSchema {
        /// Tables that were found in the database.
        tables: Vec<String>,
    },
    /// Keyring secret lookup failed.
    #[cfg(feature = "keyring")]
    Keyring {
        /// The keyring cause.
        source: nexia_nextcloud_sdk_keyring::KeyringError,
    },
}

impl KAccountsError {
    /// Builds a database error.
    pub fn database(path: impl Into<PathBuf>, source: rusqlite::Error) -> Self {
        Self::Database {
            path: path.into(),
            source,
        }
    }

    /// Builds an invalid URL error.
    pub fn invalid_url(raw: impl Into<String>, source: nexia_nextcloud_core::UrlError) -> Self {
        Self::InvalidUrl {
            raw: raw.into(),
            source,
        }
    }
}

impl fmt::Display for KAccountsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database { path, source } => {
                write!(
                    f,
                    "failed to query KAccounts database at {}: {source}",
                    path.display()
                )
            }
            Self::NotFound { searched } => {
                write!(
                    f,
                    "KAccounts database accounts.db was not found (searched: {searched:?})"
                )
            }
            Self::InvalidUrl { raw, source } => {
                write!(f, "invalid server URL '{raw}' in KAccounts: {source}")
            }
            Self::UnsupportedSchema { tables } => {
                write!(
                    f,
                    "KAccounts database has no Settings/Services or AccountSettings/AccountServices tables (found: {tables:?})"
                )
            }
            #[cfg(feature = "keyring")]
            Self::Keyring { source } => {
                write!(f, "keyring lookup failed for KAccount: {source}")
            }
        }
    }
}

impl Error for KAccountsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database { source, .. } => Some(source),
            Self::InvalidUrl { source, .. } => Some(source),
            #[cfg(feature = "keyring")]
            Self::Keyring { source } => Some(source),
            _ => None,
        }
    }
}
