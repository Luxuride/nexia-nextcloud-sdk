//! Theming and server status client errors.

use std::error::Error as StdError;
use std::fmt;

use nexia_nextcloud_core::{DecodeError, Error as SdkError, StatusError, UrlError};

/// Errors encountered when fetching theming metadata, resolving branding, or probing server status.
#[derive(Debug)]
pub enum ThemingError {
    /// Underlying SDK core error (network, transport, status).
    Sdk(SdkError),
    /// URL construction or normalization error.
    Url(UrlError),
    /// JSON serialization or deserialization error.
    Decode(DecodeError),
    /// Server returned an unexpected HTTP status code.
    Status(StatusError),
    /// OCS returned an error code or non-success status.
    Ocs {
        /// OCS status code (e.g. 100 is success, 404 is not found).
        status_code: u32,
        /// Server message.
        message: String,
    },
}

impl fmt::Display for ThemingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sdk(err) => write!(f, "theming SDK error: {err}"),
            Self::Url(err) => write!(f, "theming URL error: {err}"),
            Self::Decode(err) => write!(f, "theming JSON decode error: {err}"),
            Self::Status(err) => write!(f, "theming HTTP status error: {err}"),
            Self::Ocs {
                status_code,
                message,
            } => {
                write!(f, "OCS theming failure (code {status_code}): {message}")
            }
        }
    }
}

impl StdError for ThemingError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Sdk(err) => Some(err),
            Self::Url(err) => Some(err),
            Self::Decode(err) => Some(err),
            Self::Status(err) => Some(err),
            Self::Ocs { .. } => None,
        }
    }
}

impl From<SdkError> for ThemingError {
    fn from(err: SdkError) -> Self {
        Self::Sdk(err)
    }
}

impl From<UrlError> for ThemingError {
    fn from(err: UrlError) -> Self {
        Self::Url(err)
    }
}

impl From<DecodeError> for ThemingError {
    fn from(err: DecodeError) -> Self {
        Self::Decode(err)
    }
}

impl From<StatusError> for ThemingError {
    fn from(err: StatusError) -> Self {
        Self::Status(err)
    }
}
