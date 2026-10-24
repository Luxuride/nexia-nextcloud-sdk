use std::error::Error;
use std::fmt;
use std::path::PathBuf;

/// Failures arising from desktop client configuration discovery or parsing.
#[derive(Debug)]
#[non_exhaustive]
pub enum ClientConfigError {
    /// A configuration file could not be read from disk.
    Io {
        /// The file path that failed to be read.
        path: PathBuf,
        /// The underlying I/O error.
        source: std::io::Error,
    },
    /// The configuration file syntax was malformed.
    Parse {
        /// Detail explaining why parsing failed.
        reason: String,
    },
    /// Server URL in the configuration was invalid.
    InvalidUrl {
        /// Raw URL string found in config.
        raw: String,
        /// Parse error.
        source: nextcloud_sdk_core::UrlError,
    },
    /// Credential lookup in system keyring failed.
    #[cfg(feature = "keyring")]
    Keyring {
        /// The keyring cause.
        source: nextcloud_sdk_keyring::KeyringError,
    },
}

impl ClientConfigError {
    /// Builds an I/O error.
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    /// Builds a parse error.
    pub fn parse(reason: impl Into<String>) -> Self {
        Self::Parse {
            reason: reason.into(),
        }
    }

    /// Builds an invalid URL error.
    pub fn invalid_url(raw: impl Into<String>, source: nextcloud_sdk_core::UrlError) -> Self {
        Self::InvalidUrl {
            raw: raw.into(),
            source,
        }
    }
}

impl fmt::Display for ClientConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(
                    f,
                    "failed to read client configuration at {}: {source}",
                    path.display()
                )
            }
            Self::Parse { reason } => {
                write!(f, "failed to parse client configuration: {reason}")
            }
            Self::InvalidUrl { raw, source } => {
                write!(
                    f,
                    "invalid server URL '{raw}' in client configuration: {source}"
                )
            }
            #[cfg(feature = "keyring")]
            Self::Keyring { source } => {
                write!(f, "keyring credential lookup failed: {source}")
            }
        }
    }
}

impl Error for ClientConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::InvalidUrl { source, .. } => Some(source),
            #[cfg(feature = "keyring")]
            Self::Keyring { source } => Some(source),
            _ => None,
        }
    }
}
