use std::error::Error as StdError;
use std::fmt;

use super::{DecodeError, StatusError};
use crate::http::StatusCode;
use crate::url::{ApiPath, UrlError};

/// Errors produced by the SDK.
///
/// Variants name the stage that failed rather than flattening everything to
/// a string: a caller can tell "the server said no" ([`Error::Status`]) from
/// "the response was unreadable" ([`Error::Decode`]) and react differently.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// A URL or path was rejected before a request was made.
    Url(UrlError),
    /// The request never produced a response (DNS, TLS, timeout, ...).
    Transport(Box<dyn StdError + Send + Sync>),
    /// The server answered with a status the endpoint cannot accept.
    Status(StatusError),
    /// The response body did not match the expected shape.
    Decode(DecodeError),
}

impl Error {
    /// Wraps a transport-level failure.
    pub fn transport(source: impl StdError + Send + Sync + 'static) -> Self {
        Self::Transport(Box::new(source))
    }

    /// Wraps a failure that must be described by a message alone.
    pub fn invalid_response(source: impl StdError + Send + Sync + 'static) -> Self {
        Self::Decode(DecodeError::new("response", source))
    }

    /// Whether retrying the same request could plausibly succeed.
    ///
    /// Only server-side `5xx`, `429` and transport failures qualify:
    /// everything else is either the caller's mistake or a decode problem a
    /// retry cannot fix.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Status(status) => {
                status.status().is_server_error() || status.status().as_u16() == 429
            }
            Self::Transport(_) => true,
            Self::Url(_) | Self::Decode(_) => false,
        }
    }

    /// The status code, when the server answered at all.
    pub fn status_code(&self) -> Option<u16> {
        self.as_status().map(|status| status.status().as_u16())
    }

    /// The raw status, when the server answered at all.
    pub fn http_status(&self) -> Option<StatusCode> {
        self.as_status().map(StatusError::status)
    }

    /// Stage that produced the failure.
    pub const fn stage(&self) -> Stage {
        match self {
            Self::Url(_) => Stage::Url,
            Self::Transport(_) => Stage::Transport,
            Self::Status(_) => Stage::Status,
            Self::Decode(_) => Stage::Decode,
        }
    }

    /// The URL problem, when this is [`Error::Url`].
    pub fn as_url(&self) -> Option<&UrlError> {
        match self {
            Self::Url(error) => Some(error),
            _ => None,
        }
    }

    /// The transport cause, when this is [`Error::Transport`].
    pub fn as_transport(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Transport(error) => Some(error.as_ref()),
            _ => None,
        }
    }

    /// The status failure, when this is [`Error::Status`].
    pub fn as_status(&self) -> Option<&StatusError> {
        match self {
            Self::Status(error) => Some(error),
            _ => None,
        }
    }

    /// The decode failure, when this is [`Error::Decode`].
    pub fn as_decode(&self) -> Option<&DecodeError> {
        match self {
            Self::Decode(error) => Some(error),
            _ => None,
        }
    }

    /// Path involved, when one was recorded.
    pub fn path(&self) -> Option<&ApiPath> {
        self.as_status().map(StatusError::path)
    }

    /// Response body retained by a status failure.
    pub fn body(&self) -> Option<&str> {
        self.as_status().map(StatusError::body)
    }

    /// Whether the server rejected the credentials.
    pub fn is_unauthenticated(&self) -> bool {
        matches!(self.status_code(), Some(401))
    }

    /// Whether the credentials are valid but lack permission.
    pub fn is_forbidden(&self) -> bool {
        matches!(self.status_code(), Some(403))
    }

    /// Whether the addressed resource does not exist.
    pub fn is_not_found(&self) -> bool {
        matches!(self.status_code(), Some(404))
    }

    /// Whether the request was rejected as malformed.
    pub fn is_bad_request(&self) -> bool {
        matches!(self.status_code(), Some(400))
    }
}

/// Stage of a request at which an [`Error`] was produced.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stage {
    /// Before any request was sent.
    Url,
    /// While sending or receiving bytes.
    Transport,
    /// The server answered with a refused status.
    Status,
    /// The answer could not be decoded.
    Decode,
}

impl Stage {
    /// Short lower-case label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Url => "url",
            Self::Transport => "transport",
            Self::Status => "status",
            Self::Decode => "decode",
        }
    }
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Url(error) => write!(f, "invalid URL: {error}"),
            Self::Transport(error) => write!(f, "request failed: {error}"),
            Self::Status(error) => write!(f, "{error}"),
            Self::Decode(error) => write!(f, "{error}"),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Url(error) => Some(error),
            Self::Transport(error) => Some(error.as_ref()),
            Self::Status(error) => Some(error),
            Self::Decode(error) => Some(error),
        }
    }
}

impl From<UrlError> for Error {
    fn from(error: UrlError) -> Self {
        Self::Url(error)
    }
}

impl From<StatusError> for Error {
    fn from(error: StatusError) -> Self {
        Self::Status(error)
    }
}

impl From<DecodeError> for Error {
    fn from(error: DecodeError) -> Self {
        Self::Decode(error)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::{Error, Stage};
    use crate::error::{DecodeError, StatusError};
    use crate::http::StatusCode;
    use crate::url::ApiPath;

    fn status_error(code: u16) -> StatusError {
        StatusError::new(
            "GET",
            ApiPath::ocs("cloud/capabilities").unwrap(),
            StatusCode::new(code),
            b"",
        )
    }

    #[test]
    fn transport_failures_are_retryable() {
        let error: std::num::ParseIntError = u32::from_str("x").unwrap_err();
        assert!(Error::transport(error).is_retryable());
    }

    #[test]
    fn server_errors_are_retryable_client_errors_are_not() {
        assert!(Error::Status(status_error(503)).is_retryable());
        assert!(Error::Status(status_error(429)).is_retryable());
        assert!(!Error::Status(status_error(404)).is_retryable());
    }

    #[test]
    fn decode_failures_are_not_retryable() {
        let cause: std::num::ParseIntError = u32::from_str("x").unwrap_err();
        let error = Error::Decode(DecodeError::new("Capabilities", cause));
        assert!(!error.is_retryable());
        assert_eq!(error.stage(), Stage::Decode);
    }

    #[test]
    fn status_predicates_read_the_code() {
        assert!(Error::Status(status_error(401)).is_unauthenticated());
        assert!(Error::Status(status_error(403)).is_forbidden());
        assert!(Error::Status(status_error(404)).is_not_found());
        assert!(!Error::Status(status_error(200)).is_not_found());
    }

    #[test]
    fn display_reports_the_stage() {
        assert!(
            Error::Status(status_error(404))
                .to_string()
                .contains("returned 404")
        );
    }
}
