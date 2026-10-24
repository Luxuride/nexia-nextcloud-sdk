use std::error::Error;
use std::fmt;

/// Errors arising from keyring operations across platform backends.
#[derive(Debug)]
#[non_exhaustive]
pub enum KeyringError {
    /// The target entry was not found in the keyring.
    NotFound {
        /// The service or application namespace.
        service: String,
        /// The account or username.
        account: String,
    },
    /// The underlying platform keyring backend returned an error.
    Platform {
        /// The operation that failed (e.g. "read", "write", "delete").
        operation: &'static str,
        /// The underlying cause.
        source: Box<dyn Error + Send + Sync>,
    },
    /// Keyring data was malformed (e.g. invalid UTF-8).
    InvalidData {
        /// Reason for the data invalidity.
        reason: &'static str,
    },
    /// The keyring is not available or inaccessible on this system.
    Unavailable {
        /// Explanation why the keyring is unavailable.
        reason: String,
    },
}

impl KeyringError {
    /// Builds a [`KeyringError::Platform`] failure preserving the underlying source.
    pub fn platform(
        operation: &'static str,
        source: impl Into<Box<dyn Error + Send + Sync>>,
    ) -> Self {
        Self::Platform {
            operation,
            source: source.into(),
        }
    }
}

impl fmt::Display for KeyringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { service, account } => {
                write!(
                    f,
                    "keyring entry '{account}' under service '{service}' not found"
                )
            }
            Self::Platform { operation, .. } => {
                write!(f, "platform keyring operation '{operation}' failed")
            }
            Self::InvalidData { reason } => {
                write!(f, "keyring secret data invalid: {reason}")
            }
            Self::Unavailable { reason } => {
                write!(f, "keyring unavailable: {reason}")
            }
        }
    }
}

impl Error for KeyringError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Platform { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}
