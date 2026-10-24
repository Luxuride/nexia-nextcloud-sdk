use std::future::Future;
use std::sync::{Arc, RwLock};

use nextcloud_sdk_core::{Error, Request, Response, ServerUrl, Transport};
use nextcloud_sdk_ratelimit::{BandwidthLimit, BandwidthLimiter, ProgressCallback};

use crate::header_copy::to_header_map;
use crate::method::to_reqwest;
use crate::response_convert::to_response;
use crate::throttled::prepare_request_body;

/// A [`Transport`] backed by a reusable `reqwest` client, with integrated bandwidth rate limiting
/// and transfer progress reporting.
///
/// The client owns the connection pool, so one instance is meant to live for
/// the lifetime of a client and be shared by `&self`; every call is
/// independent of the previous one.
#[derive(Clone)]
pub struct ReqwestTransport {
    client: reqwest::Client,
    server: ServerUrl,
    limiter: BandwidthLimiter,
    download_progress: Arc<RwLock<Option<ProgressCallback>>>,
    upload_progress: Arc<RwLock<Option<ProgressCallback>>>,
}

impl ReqwestTransport {
    /// Wraps an existing client and the server it talks to with unconstrained throughput.
    pub fn new(client: reqwest::Client, server: ServerUrl) -> Self {
        Self::with_limiter(client, server, BandwidthLimiter::unlimited())
    }

    /// Wraps an existing client, the server it talks to, and an explicit bandwidth limiter.
    pub fn with_limiter(
        client: reqwest::Client,
        server: ServerUrl,
        limiter: BandwidthLimiter,
    ) -> Self {
        Self {
            client,
            server,
            limiter,
            download_progress: Arc::new(RwLock::new(None)),
            upload_progress: Arc::new(RwLock::new(None)),
        }
    }

    /// Wraps an existing client, server, limiter, and optional global transfer progress callbacks.
    pub fn with_limiter_and_progress(
        client: reqwest::Client,
        server: ServerUrl,
        limiter: BandwidthLimiter,
        download_progress: Option<ProgressCallback>,
        upload_progress: Option<ProgressCallback>,
    ) -> Self {
        Self {
            client,
            server,
            limiter,
            download_progress: Arc::new(RwLock::new(download_progress)),
            upload_progress: Arc::new(RwLock::new(upload_progress)),
        }
    }

    /// The instance this transport targets.
    pub const fn server(&self) -> &ServerUrl {
        &self.server
    }

    /// The underlying client, for callers that need its configuration.
    pub const fn client(&self) -> &reqwest::Client {
        &self.client
    }

    /// The active bandwidth rate limiter.
    pub const fn limiter(&self) -> &BandwidthLimiter {
        &self.limiter
    }

    /// The currently active upload bandwidth limit.
    pub fn upload_limit(&self) -> BandwidthLimit {
        self.limiter.upload_limit()
    }

    /// The currently active download bandwidth limit.
    pub fn download_limit(&self) -> BandwidthLimit {
        self.limiter.download_limit()
    }

    /// Dynamically updates the upload bandwidth limit.
    pub fn set_upload_limit(&self, limit: impl Into<BandwidthLimit>) {
        self.limiter.set_upload_limit(limit.into());
    }

    /// Dynamically updates the download bandwidth limit.
    pub fn set_download_limit(&self, limit: impl Into<BandwidthLimit>) {
        self.limiter.set_download_limit(limit.into());
    }

    /// The currently active default download progress callback, if any.
    pub fn download_progress(&self) -> Option<ProgressCallback> {
        self.download_progress
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Dynamically sets or clears the default download progress callback.
    pub fn set_download_progress(&self, callback: Option<ProgressCallback>) {
        let mut guard = self
            .download_progress
            .write()
            .unwrap_or_else(|e| e.into_inner());
        *guard = callback;
    }

    /// The currently active default upload progress callback, if any.
    pub fn upload_progress(&self) -> Option<ProgressCallback> {
        self.upload_progress
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Dynamically sets or clears the default upload progress callback.
    pub fn set_upload_progress(&self, callback: Option<ProgressCallback>) {
        let mut guard = self
            .upload_progress
            .write()
            .unwrap_or_else(|e| e.into_inner());
        *guard = callback;
    }

    /// Resolves `request` against the server URL, sends it, and receives the response.
    pub async fn send_with_progress(
        &self,
        request: Request,
        download_progress: Option<ProgressCallback>,
        upload_progress: Option<ProgressCallback>,
    ) -> Result<Response, Error> {
        let method = to_reqwest(request.method())?;
        let url = self.server.join(request.path());

        let mut builder = self
            .client
            .request(method, url)
            .headers(to_header_map(request.headers())?);

        if let Some(body) = request.body() {
            let req_body =
                prepare_request_body(body.to_vec(), self.limiter.clone(), upload_progress);
            builder = builder.body(req_body);
        }

        let response = builder.send().await.map_err(Error::transport)?;
        to_response(response, &self.limiter, download_progress.as_ref()).await
    }

    /// Executes `request` with a one-off download progress callback.
    pub fn execute_with_progress(
        &self,
        request: Request,
        progress: ProgressCallback,
    ) -> impl Future<Output = Result<Response, Error>> + Send {
        let ul = self.upload_progress();
        self.send_with_progress(request, Some(progress), ul)
    }
}

impl Transport for ReqwestTransport {
    fn execute(&self, request: Request) -> impl Future<Output = Result<Response, Error>> + Send {
        let dl = self.download_progress();
        let ul = self.upload_progress();
        self.send_with_progress(request, dl, ul)
    }
}

impl std::fmt::Debug for ReqwestTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReqwestTransport")
            .field("server", &self.server)
            .field("limiter", &self.limiter)
            .finish_non_exhaustive()
    }
}
