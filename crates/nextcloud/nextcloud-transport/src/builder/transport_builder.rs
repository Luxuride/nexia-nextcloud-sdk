use sdk_core::ServerUrl;

use crate::error::TransportBuildError;
use crate::reqwest_transport::ReqwestTransport;

use super::timeouts::TimeoutConfig;

/// User agent sent when the caller does not supply one.
pub const DEFAULT_USER_AGENT: &str = concat!("nextcloud-rust-sdk/", env!("CARGO_PKG_VERSION"));

/// Configures a [`ReqwestTransport`].
///
/// Everything shared by all requests lives here, so a single client is built
/// once and shared by reference with whichever API crates need it. The server
/// URL is the only mandatory piece; timeouts and user agent fall back to
/// bounded defaults.
#[derive(Clone, Debug)]
pub struct TransportBuilder {
    server: Option<ServerUrl>,
    timeouts: TimeoutConfig,
    user_agent: Option<String>,
}

impl TransportBuilder {
    /// A builder with default timeouts and no user agent override.
    pub const fn new() -> Self {
        Self {
            server: None,
            timeouts: TimeoutConfig::new(),
            user_agent: None,
        }
    }

    /// A builder for the instance at `server`.
    pub const fn with_server(server: ServerUrl) -> Self {
        Self {
            server: Some(server),
            timeouts: TimeoutConfig::new(),
            user_agent: None,
        }
    }

    /// Sets the Nextcloud base URL.
    #[must_use]
    pub fn server(mut self, server: ServerUrl) -> Self {
        self.server = Some(server);
        self
    }

    /// Replaces both timeouts at once.
    #[must_use]
    pub const fn timeouts(mut self, timeouts: TimeoutConfig) -> Self {
        self.timeouts = timeouts;
        self
    }

    /// Sets the `User-Agent` header sent with every request.
    #[must_use]
    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    /// The configured server URL, once one has been set.
    pub const fn server_url(&self) -> Option<&ServerUrl> {
        self.server.as_ref()
    }

    /// The configured timeouts.
    pub const fn configured_timeouts(&self) -> TimeoutConfig {
        self.timeouts
    }

    /// Builds the client.
    ///
    /// # Errors
    ///
    /// Returns [`TransportBuildError::MissingServerUrl`] when no server was
    /// configured, a zero-timeout variant for an unusable timeout, and the
    /// remaining variants when `reqwest` rejects the configuration.
    pub fn build(self) -> Result<ReqwestTransport, TransportBuildError> {
        let server = self.server.ok_or(TransportBuildError::MissingServerUrl)?;
        self.timeouts.validate()?;

        let builder = self.timeouts.apply(reqwest::Client::builder());
        let builder = match self.user_agent {
            Some(user_agent) => {
                // Reject here so a bad value is a typed build error rather
                // than a deferred error on the first request.
                reqwest::header::HeaderValue::from_str(&user_agent).map_err(|source| {
                    TransportBuildError::invalid_user_agent(user_agent.clone(), source)
                })?;
                builder.user_agent(user_agent)
            }
            None => builder.user_agent(DEFAULT_USER_AGENT),
        };

        let client = builder.build()?;
        Ok(ReqwestTransport::new(client, server))
    }
}

impl Default for TransportBuilder {
    fn default() -> Self {
        Self::new()
    }
}
