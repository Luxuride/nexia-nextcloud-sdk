use std::any::type_name;

use sdk_core::{
    ApiPath, AuthProvider, DecodeError, Error, Headers, Request, Response, StatusError, Transport,
};
use serde::de::DeserializeOwned;

use super::path::{require_ocs, with_query};
use crate::envelope::{OcsEnvelope, OcsStatus, VerdictEnvelope};
use crate::error::StatusCodeError;
use crate::query::Query;

/// Header that marks a request as an OCS call; OCS endpoints reject requests
/// without it.
const OCS_REQUEST_HEADER: &str = "OCS-APIRequest";
/// Header that selects a JSON answer; OCS defaults to XML.
const ACCEPT_HEADER: &str = "Accept";
const ACCEPT_JSON: &str = "application/json";
const METHOD: &str = "GET";

/// Executes OCS calls and unwraps their envelope.
///
/// Generic over the [`Transport`] that carries bytes and the [`AuthProvider`]
/// that applies credentials, so a client is built from any transport
/// implementation without virtual dispatch. The transport already knows the
/// instance URL and turns an [`ApiPath`] into an absolute URL, so the client
/// holds no base URL of its own.
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
    pub async fn get<T2: DeserializeOwned>(
        &self,
        path: ApiPath,
        query: Query,
    ) -> Result<T2, Error> {
        let path = with_query(&require_ocs(path)?, &query);

        let mut headers = Headers::new();
        headers.set(ACCEPT_HEADER, ACCEPT_JSON);
        headers.set(OCS_REQUEST_HEADER, "true");
        self.auth.authenticate(&mut headers);

        let response = self
            .transport
            .execute(Request::get(path.clone()).with_headers(headers))
            .await?;

        if !response.status().is_success() {
            return Err(status_error(path, &response));
        }

        // The verdict is read first: a failure body is not the endpoint's
        // payload shape, so decoding it as `data` would report the OCS
        // `statuscode` as a malformed body and lose the reason.
        let verdict: VerdictEnvelope = serde_json::from_slice(response.body())
            .map_err(|cause| DecodeError::new(type_name::<OcsEnvelope<T2>>(), cause))
            .map_err(Error::Decode)?;

        match verdict.ocs.meta.verdict() {
            Some(OcsStatus::Ok) => {}
            Some(OcsStatus::Failure) => return Err(status_error(path, &response)),
            // An envelope that carries no readable verdict is malformed, not a
            // success: guessing here would turn a broken answer into a payload
            // the caller cannot distinguish from a real one.
            None => {
                return Err(Error::Decode(DecodeError::new(
                    type_name::<OcsEnvelope<T2>>(),
                    StatusCodeError::new(verdict.ocs.meta.statuscode()),
                )));
            }
        }

        let envelope: OcsEnvelope<T2> = serde_json::from_slice(response.body())
            .map_err(|cause| DecodeError::new(type_name::<OcsEnvelope<T2>>(), cause))
            .map_err(Error::Decode)?;

        Ok(envelope.into_data())
    }
}

fn status_error(path: ApiPath, response: &Response) -> Error {
    Error::Status(StatusError::new(
        METHOD,
        path,
        response.status(),
        response.body(),
    ))
}
