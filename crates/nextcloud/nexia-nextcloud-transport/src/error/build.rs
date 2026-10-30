use std::error::Error as StdError;
use std::fmt;

/// Why a [`TransportBuilder`](crate::TransportBuilder) could not produce a
/// client.
///
/// The variants exist so a caller can tell a rejected configuration from a
/// `reqwest` failure without parsing a message, and both keep their cause:
/// nothing here flattens an underlying error to a string.
#[derive(Debug)]
#[non_exhaustive]
pub enum TransportBuildError {
    /// No Nextcloud base URL was supplied.
    MissingServerUrl,
    /// A timeout was set to zero, which would abort every request instantly.
    ZeroTimeout(&'static str),
    /// The user agent contained characters a header value cannot carry.
    InvalidUserAgent {
        /// Rejected user agent, kept for diagnostics.
        value: String,
        /// Cause reported by the header parser.
        source: reqwest::header::InvalidHeaderValue,
    },
    /// `reqwest` refused to assemble the client.
    Client(reqwest::Error),
}

impl TransportBuildError {
    /// Builds [`TransportBuildError::ZeroTimeout`].
    pub(crate) const fn zero_timeout(field: &'static str) -> Self {
        Self::ZeroTimeout(field)
    }

    /// Builds [`TransportBuildError::InvalidUserAgent`].
    pub(crate) fn invalid_user_agent(
        value: String,
        source: reqwest::header::InvalidHeaderValue,
    ) -> Self {
        Self::InvalidUserAgent { value, source }
    }
}

impl fmt::Display for TransportBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingServerUrl => {
                f.write_str("transport build failed: no server URL configured")
            }
            Self::ZeroTimeout(field) => write!(f, "transport build failed: {field} must be > 0"),
            Self::InvalidUserAgent { .. } => f.write_str(
                "transport build failed: user agent contains characters not allowed in a header",
            ),
            Self::Client(error) => write!(f, "transport build failed: {error}"),
        }
    }
}

impl StdError for TransportBuildError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::MissingServerUrl | Self::ZeroTimeout(_) => None,
            Self::InvalidUserAgent { source, .. } => Some(source),
            Self::Client(error) => Some(error),
        }
    }
}

impl From<reqwest::Error> for TransportBuildError {
    fn from(error: reqwest::Error) -> Self {
        Self::Client(error)
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error as _;

    use super::TransportBuildError;

    #[test]
    fn zero_timeout_names_the_field_and_has_no_source() {
        let error = TransportBuildError::zero_timeout("request timeout");
        assert!(error.to_string().contains("request timeout"));
        assert!(error.source().is_none());
    }

    #[test]
    fn missing_server_url_is_reported() {
        assert!(
            TransportBuildError::MissingServerUrl
                .to_string()
                .contains("no server URL")
        );
    }
}
