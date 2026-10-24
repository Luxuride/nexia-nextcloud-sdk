use std::time::Duration;

use crate::error::TransportBuildError;

/// How long a request may wait before it is abandoned.
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// How long establishing a connection may take.
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Timeouts applied to every request the client sends.
///
/// Both halves are optional: `None` leaves the decision to `reqwest`, which
/// then waits indefinitely. The default is deliberately bounded, so a
/// Nextcloud instance that stops answering cannot hang a caller forever.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeoutConfig {
    request: Option<Duration>,
    connect: Option<Duration>,
}

impl TimeoutConfig {
    /// Bounded defaults ([`DEFAULT_REQUEST_TIMEOUT`], [`DEFAULT_CONNECT_TIMEOUT`]).
    pub const fn new() -> Self {
        Self {
            request: Some(DEFAULT_REQUEST_TIMEOUT),
            connect: Some(DEFAULT_CONNECT_TIMEOUT),
        }
    }

    /// No timeouts at all, i.e. `reqwest`'s own default behaviour.
    pub const fn none() -> Self {
        Self {
            request: None,
            connect: None,
        }
    }

    /// Replaces the whole-request timeout.
    #[must_use]
    pub const fn with_request(mut self, timeout: Duration) -> Self {
        self.request = Some(timeout);
        self
    }

    /// Replaces the connection timeout.
    #[must_use]
    pub const fn with_connect(mut self, timeout: Duration) -> Self {
        self.connect = Some(timeout);
        self
    }

    /// Whole-request timeout, when one is set.
    pub const fn request(&self) -> Option<Duration> {
        self.request
    }

    /// Connection timeout, when one is set.
    pub const fn connect(&self) -> Option<Duration> {
        self.connect
    }

    /// Rejects timeouts that would abort every request immediately.
    pub(crate) fn validate(&self) -> Result<(), TransportBuildError> {
        if self.request.is_some_and(|timeout| timeout.is_zero()) {
            return Err(TransportBuildError::zero_timeout("request timeout"));
        }
        if self.connect.is_some_and(|timeout| timeout.is_zero()) {
            return Err(TransportBuildError::zero_timeout("connect timeout"));
        }
        Ok(())
    }

    /// Applies both timeouts to `builder`.
    pub(crate) fn apply(self, mut builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
        if let Some(timeout) = self.connect {
            builder = builder.connect_timeout(timeout);
        }
        if let Some(timeout) = self.request {
            builder = builder.timeout(timeout);
        }
        builder
    }
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{DEFAULT_CONNECT_TIMEOUT, DEFAULT_REQUEST_TIMEOUT, TimeoutConfig};

    #[test]
    fn defaults_are_bounded() {
        let timeouts = TimeoutConfig::default();
        assert_eq!(timeouts.request(), Some(DEFAULT_REQUEST_TIMEOUT));
        assert_eq!(timeouts.connect(), Some(DEFAULT_CONNECT_TIMEOUT));
    }

    #[test]
    fn none_clears_both_halves() {
        let timeouts = TimeoutConfig::none();
        assert_eq!(timeouts.request(), None);
        assert_eq!(timeouts.connect(), None);
        assert!(timeouts.validate().is_ok());
    }

    #[test]
    fn zero_request_timeout_is_rejected() {
        let error = TimeoutConfig::new()
            .with_request(Duration::ZERO)
            .validate()
            .expect_err("zero timeout must be rejected");
        assert!(error.to_string().contains("request timeout"));
    }

    #[test]
    fn zero_connect_timeout_is_rejected() {
        let error = TimeoutConfig::new()
            .with_connect(Duration::ZERO)
            .validate()
            .expect_err("zero timeout must be rejected");
        assert!(error.to_string().contains("connect timeout"));
    }
}
