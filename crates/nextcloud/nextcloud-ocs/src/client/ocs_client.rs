use nextcloud_sdk_core::{ApiPath, AuthProvider, Error, HttpMethod, Transport};
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::path::{require_ocs, with_query};
use super::request::build_request;
use super::response::decode_response;
use crate::query::Query;

/// Executes OCS calls and unwraps their envelope.
///
/// Generic over the [`Transport`] that carries bytes and the [`AuthProvider`]
/// that applies credentials, so a client is built from any transport
/// implementation without virtual dispatch. The transport already knows the
/// instance URL and turns an [`ApiPath`] into an absolute URL, so the client
/// holds no base URL of its own.
///
/// All OCS calls follow the Nextcloud Open Collaboration Services (OCS)
/// specification (<https://docs.nextcloud.com/server/latest/developer_manual/client_apis/OCS/>):
/// - Every request sets `OCS-APIRequest: true` and `Accept: application/json`.
/// - Mutation requests (`POST`, `PUT`) serialize request bodies as JSON and set
///   `Content-Type: application/json`.
/// - Responses are wrapped in an [`OcsEnvelope`](crate::OcsEnvelope) carrying
///   a `meta.statuscode` verdict (`100` or `200` for success; failure codes such
///   as `101`, `102`, `997`, `998`, or `999` map to [`Error::Status`]).
/// - Calling endpoints with unit response `()` succeeds on valid envelopes even
///   when the response payload is empty (e.g. `[]` or `{}`), or when the caller
///   only cares about verdict success.
#[derive(Clone, Debug)]
pub struct OcsClient<T, A> {
    transport: T,
    auth: A,
}

impl<T: Transport, A: AuthProvider> OcsClient<T, A> {
    /// Pairs a transport with the credentials applied to every call.
    pub const fn new(transport: T, auth: A) -> Self {
        Self { transport, auth }
    }

    /// The transport this client sends through.
    pub const fn transport(&self) -> &T {
        &self.transport
    }

    /// The credential provider applied to every call.
    pub const fn auth(&self) -> &A {
        &self.auth
    }

    /// Performs a `GET` against an OCS path and returns the envelope payload.
    ///
    /// The call sets `Accept: application/json` and `OCS-APIRequest: true` and
    /// applies credentials. Three cases fail: a path outside `/ocs/v2.php`, a
    /// response status or an OCS verdict the caller cannot accept, and a body
    /// that is not an envelope of the requested type. A failure keeps the raw
    /// body, so the OCS code (`997` app not installed, `998` not allowed,
    /// `999` invalid) and its message stay reachable through [`Error::body`].
    ///
    /// See Nextcloud OCS API documentation:
    /// <https://docs.nextcloud.com/server/latest/developer_manual/client_apis/OCS/>
    pub async fn get<Res: DeserializeOwned>(
        &self,
        path: ApiPath,
        query: Query,
    ) -> Result<Res, Error> {
        let path = with_query(&require_ocs(path)?, &query);
        tracing::debug!(path = %path.as_str(), "Executing OCS GET request");
        let request = build_request(HttpMethod::Get, path.clone(), None::<&()>, &self.auth)?;
        let response = self.transport.execute(request).await?;
        decode_response(HttpMethod::Get, path, &response)
    }

    /// Performs a `POST` against an OCS path with a serialised request body.
    ///
    /// The call sets `Accept: application/json`, `Content-Type: application/json`,
    /// and `OCS-APIRequest: true`, and applies credentials.
    ///
    /// When `Res` is `()`, calls succeed if the server returns an OCS envelope
    /// reporting success (`statuscode` 100 or 200), ignoring empty or present payload data.
    ///
    /// See Nextcloud OCS API documentation:
    /// <https://docs.nextcloud.com/server/latest/developer_manual/client_apis/OCS/>
    pub async fn post<Req: Serialize, Res: DeserializeOwned>(
        &self,
        path: ApiPath,
        body: &Req,
    ) -> Result<Res, Error> {
        self.post_with_query(path, Query::new(), body).await
    }

    /// Performs a `POST` against an OCS path with query parameters and a
    /// serialised request body.
    ///
    /// The call sets `Accept: application/json`, `Content-Type: application/json`,
    /// and `OCS-APIRequest: true`, and applies credentials.
    ///
    /// When `Res` is `()`, calls succeed if the server returns an OCS envelope
    /// reporting success (`statuscode` 100 or 200), ignoring empty or present payload data.
    ///
    /// See Nextcloud OCS API documentation:
    /// <https://docs.nextcloud.com/server/latest/developer_manual/client_apis/OCS/>
    pub async fn post_with_query<Req: Serialize, Res: DeserializeOwned>(
        &self,
        path: ApiPath,
        query: Query,
        body: &Req,
    ) -> Result<Res, Error> {
        let path = with_query(&require_ocs(path)?, &query);
        tracing::debug!(path = %path.as_str(), "Executing OCS POST request");
        let request = build_request(HttpMethod::Post, path.clone(), Some(body), &self.auth)?;
        let response = self.transport.execute(request).await?;
        decode_response(HttpMethod::Post, path, &response)
    }

    /// Performs a `PUT` against an OCS path with a serialised request body.
    ///
    /// The call sets `Accept: application/json`, `Content-Type: application/json`,
    /// and `OCS-APIRequest: true`, and applies credentials.
    ///
    /// When `Res` is `()`, calls succeed if the server returns an OCS envelope
    /// reporting success (`statuscode` 100 or 200), ignoring empty or present payload data.
    ///
    /// See Nextcloud OCS API documentation:
    /// <https://docs.nextcloud.com/server/latest/developer_manual/client_apis/OCS/>
    pub async fn put<Req: Serialize, Res: DeserializeOwned>(
        &self,
        path: ApiPath,
        body: &Req,
    ) -> Result<Res, Error> {
        self.put_with_query(path, Query::new(), body).await
    }

    /// Performs a `PUT` against an OCS path with query parameters and a
    /// serialised request body.
    ///
    /// The call sets `Accept: application/json`, `Content-Type: application/json`,
    /// and `OCS-APIRequest: true`, and applies credentials.
    ///
    /// When `Res` is `()`, calls succeed if the server returns an OCS envelope
    /// reporting success (`statuscode` 100 or 200), ignoring empty or present payload data.
    ///
    /// See Nextcloud OCS API documentation:
    /// <https://docs.nextcloud.com/server/latest/developer_manual/client_apis/OCS/>
    pub async fn put_with_query<Req: Serialize, Res: DeserializeOwned>(
        &self,
        path: ApiPath,
        query: Query,
        body: &Req,
    ) -> Result<Res, Error> {
        let path = with_query(&require_ocs(path)?, &query);
        tracing::debug!(path = %path.as_str(), "Executing OCS PUT request");
        let request = build_request(HttpMethod::Put, path.clone(), Some(body), &self.auth)?;
        let response = self.transport.execute(request).await?;
        decode_response(HttpMethod::Put, path, &response)
    }

    /// Performs a `DELETE` against an OCS path.
    ///
    /// The call sets `Accept: application/json` and `OCS-APIRequest: true`
    /// and applies credentials.
    ///
    /// When `Res` is `()`, calls succeed if the server returns an OCS envelope
    /// reporting success (`statuscode` 100 or 200), ignoring empty or present payload data.
    ///
    /// See Nextcloud OCS API documentation:
    /// <https://docs.nextcloud.com/server/latest/developer_manual/client_apis/OCS/>
    pub async fn delete<Res: DeserializeOwned>(&self, path: ApiPath) -> Result<Res, Error> {
        self.delete_with_query(path, Query::new()).await
    }

    /// Performs a `DELETE` against an OCS path with query parameters.
    ///
    /// The call sets `Accept: application/json` and `OCS-APIRequest: true`
    /// and applies credentials.
    ///
    /// When `Res` is `()`, calls succeed if the server returns an OCS envelope
    /// reporting success (`statuscode` 100 or 200), ignoring empty or present payload data.
    ///
    /// See Nextcloud OCS API documentation:
    /// <https://docs.nextcloud.com/server/latest/developer_manual/client_apis/OCS/>
    pub async fn delete_with_query<Res: DeserializeOwned>(
        &self,
        path: ApiPath,
        query: Query,
    ) -> Result<Res, Error> {
        let path = with_query(&require_ocs(path)?, &query);
        tracing::debug!(path = %path.as_str(), "Executing OCS DELETE request");
        let request = build_request(HttpMethod::Delete, path.clone(), None::<&()>, &self.auth)?;
        let response = self.transport.execute(request).await?;
        decode_response(HttpMethod::Delete, path, &response)
    }
}
