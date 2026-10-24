use std::future::Future;

use sdk_core::{Error, Request, Response, ServerUrl, Transport};

use crate::header_copy::to_header_map;
use crate::method::to_reqwest;
use crate::response_convert::to_response;

/// A [`Transport`] backed by a reusable `reqwest` client.
///
/// The client owns the connection pool, so one instance is meant to live for
/// the lifetime of a client and be shared by `&self`; every call is
/// independent of the previous one.
pub struct ReqwestTransport {
    client: reqwest::Client,
    server: ServerUrl,
}

impl ReqwestTransport {
    /// Wraps an existing client and the server it talks to.
    pub fn new(client: reqwest::Client, server: ServerUrl) -> Self {
        Self { client, server }
    }

    /// The instance this transport targets.
    pub const fn server(&self) -> &ServerUrl {
        &self.server
    }

    /// The underlying client, for callers that need its configuration.
    pub const fn client(&self) -> &reqwest::Client {
        &self.client
    }

    /// Resolves `request` against the server URL and sends it.
    async fn send(&self, request: Request) -> Result<Response, Error> {
        let method = to_reqwest(request.method())?;
        let url = self.server.join(request.path());

        let mut builder = self
            .client
            .request(method, url)
            .headers(to_header_map(request.headers())?);
        if let Some(body) = request.body() {
            builder = builder.body(body.to_vec());
        }

        let response = builder.send().await.map_err(Error::transport)?;
        to_response(response).await
    }
}

impl Transport for ReqwestTransport {
    fn execute(&self, request: Request) -> impl Future<Output = Result<Response, Error>> + Send {
        self.send(request)
    }
}

impl std::fmt::Debug for ReqwestTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The client holds credentials in its default headers, so only the
        // target server is described here.
        f.debug_struct("ReqwestTransport")
            .field("server", &self.server)
            .finish_non_exhaustive()
    }
}
