use nexia_nextcloud_core::Error;

/// Why a call failed, reduced to the part a retry policy cares about.
///
/// The original [`Error`] stays with the caller; the machine only needs to
/// know whether another attempt is worth making, so credentials and bodies do
/// not travel into the state record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    /// The request never produced a response (DNS, TLS, timeout, ...).
    Transport,
    /// The server answered with a status the endpoint cannot accept.
    Status(u16),
    /// The response body did not match the expected shape.
    Decode,
    /// A path or URL was rejected before any request was made.
    Url,
}

impl Failure {
    /// Reduces an [`Error`] to its retry-relevant shape.
    pub fn from_error(error: &Error) -> Self {
        if error.as_url().is_some() {
            return Self::Url;
        }
        if error.as_transport().is_some() {
            return Self::Transport;
        }
        if error.as_status().is_some() {
            return Self::Status(error.status_code().unwrap_or(0));
        }
        Self::Decode
    }

    /// Whether another attempt could plausibly succeed.
    ///
    /// Mirrors [`Error::is_retryable`]: transport failures, `5xx` and `429`
    /// qualify, everything else is either the caller's mistake or a decode
    /// problem a retry cannot fix.
    pub const fn is_retryable(self) -> bool {
        match self {
            Self::Transport => true,
            Self::Status(code) => code >= 500 || code == 429,
            Self::Decode | Self::Url => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Failure;

    #[test]
    fn transport_and_server_errors_are_retryable() {
        assert!(Failure::Transport.is_retryable());
        assert!(Failure::Status(500).is_retryable());
        assert!(Failure::Status(503).is_retryable());
        assert!(Failure::Status(429).is_retryable());
    }

    #[test]
    fn client_errors_and_decode_problems_are_not() {
        assert!(!Failure::Status(404).is_retryable());
        assert!(!Failure::Status(401).is_retryable());
        assert!(!Failure::Decode.is_retryable());
        assert!(!Failure::Url.is_retryable());
    }
}
