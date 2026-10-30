use std::fmt;
use std::sync::Arc;

use nexia_nextcloud_core::ServerUrl;
use nexia_nextcloud_sdk_ratelimit::{
    BandwidthLimit, BandwidthLimiter, ProgressCallback, TransferProgress,
};

use crate::error::TransportBuildError;
use crate::reqwest_transport::ReqwestTransport;

use super::timeouts::TimeoutConfig;

/// User agent sent when the caller does not supply one.
pub const DEFAULT_USER_AGENT: &str = concat!("nexia-nextcloud-sdk/", env!("CARGO_PKG_VERSION"));

/// Configures a [`ReqwestTransport`].
///
/// Everything shared by all requests lives here, so a single client is built
/// once and shared by reference with whichever API crates need it. The server
/// URL is the only mandatory piece; timeouts and user agent fall back to
/// bounded defaults.
#[derive(Clone)]
pub struct TransportBuilder {
    server: Option<ServerUrl>,
    timeouts: TimeoutConfig,
    user_agent: Option<String>,
    limiter: Option<BandwidthLimiter>,
    download_progress: Option<ProgressCallback>,
    upload_progress: Option<ProgressCallback>,
}

impl TransportBuilder {
    /// A builder with default timeouts and no user agent override.
    pub const fn new() -> Self {
        Self {
            server: None,
            timeouts: TimeoutConfig::new(),
            user_agent: None,
            limiter: None,
            download_progress: None,
            upload_progress: None,
        }
    }

    /// A builder for the instance at `server`.
    pub const fn with_server(server: ServerUrl) -> Self {
        Self {
            server: Some(server),
            timeouts: TimeoutConfig::new(),
            user_agent: None,
            limiter: None,
            download_progress: None,
            upload_progress: None,
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

    /// Sets the shared bandwidth limiter for upload and download traffic shaping.
    #[must_use]
    pub fn bandwidth_limiter(mut self, limiter: BandwidthLimiter) -> Self {
        self.limiter = Some(limiter);
        self
    }

    /// Sets the initial upload bandwidth limit.
    #[must_use]
    pub fn upload_limit(mut self, limit: impl Into<BandwidthLimit>) -> Self {
        let limiter = self.limiter.unwrap_or_default();
        limiter.set_upload_limit(limit.into());
        self.limiter = Some(limiter);
        self
    }

    /// Sets the initial download bandwidth limit.
    #[must_use]
    pub fn download_limit(mut self, limit: impl Into<BandwidthLimit>) -> Self {
        let limiter = self.limiter.unwrap_or_default();
        limiter.set_download_limit(limit.into());
        self.limiter = Some(limiter);
        self
    }

    /// Sets the default download progress callback.
    #[must_use]
    pub fn download_progress(
        mut self,
        callback: impl Fn(TransferProgress) + Send + Sync + 'static,
    ) -> Self {
        self.download_progress = Some(Arc::new(callback));
        self
    }

    /// Sets the default upload progress callback.
    #[must_use]
    pub fn upload_progress(
        mut self,
        callback: impl Fn(TransferProgress) + Send + Sync + 'static,
    ) -> Self {
        self.upload_progress = Some(Arc::new(callback));
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
        tracing::debug!(server = %server, "Building ReqwestTransport");
        self.timeouts.validate()?;

        let builder = self
            .timeouts
            .apply(reqwest::Client::builder())
            .tcp_nodelay(true)
            .http2_adaptive_window(true)
            .http2_keep_alive_interval(Some(std::time::Duration::from_secs(30)))
            .http2_keep_alive_timeout(std::time::Duration::from_secs(10))
            .http2_keep_alive_while_idle(true)
            .pool_max_idle_per_host(32);
        let builder = match self.user_agent {
            Some(user_agent) => {
                reqwest::header::HeaderValue::from_str(&user_agent).map_err(|source| {
                    TransportBuildError::invalid_user_agent(user_agent.clone(), source)
                })?;
                builder.user_agent(user_agent)
            }
            None => builder.user_agent(DEFAULT_USER_AGENT),
        };

        let client = builder.build()?;
        let limiter = self.limiter.unwrap_or_default();
        Ok(ReqwestTransport::with_limiter_and_progress(
            client,
            server,
            limiter,
            self.download_progress,
            self.upload_progress,
        ))
    }
}

impl Default for TransportBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for TransportBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TransportBuilder")
            .field("server", &self.server)
            .field("timeouts", &self.timeouts)
            .field("user_agent", &self.user_agent)
            .field("limiter", &self.limiter)
            .finish_non_exhaustive()
    }
}
