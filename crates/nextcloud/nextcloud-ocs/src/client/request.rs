use std::any::type_name;

use nexia_nextcloud_core::{
    ApiPath, AuthProvider, DecodeError, Error, Headers, HttpMethod, Request,
};
use serde::Serialize;

/// Header that marks a request as an OCS call; OCS endpoints reject requests
/// without it.
pub(crate) const OCS_REQUEST_HEADER: &str = "OCS-APIRequest";
/// Header that selects a JSON answer; OCS defaults to XML.
pub(crate) const ACCEPT_HEADER: &str = "Accept";
pub(crate) const ACCEPT_JSON: &str = "application/json";
/// Header that identifies the request body payload type.
pub(crate) const CONTENT_TYPE_HEADER: &str = "Content-Type";
pub(crate) const CONTENT_TYPE_JSON: &str = "application/json";

/// Assembles an OCS request with headers, credentials, and an optional body.
pub(crate) fn build_request<Req: Serialize>(
    method: HttpMethod,
    path: ApiPath,
    body: Option<&Req>,
    auth: &impl AuthProvider,
) -> Result<Request, Error> {
    let mut headers = Headers::new();
    headers.set(ACCEPT_HEADER, ACCEPT_JSON);
    headers.set(OCS_REQUEST_HEADER, "true");

    let body_bytes = match body {
        Some(payload) => {
            headers.set(CONTENT_TYPE_HEADER, CONTENT_TYPE_JSON);
            let bytes = serde_json::to_vec(payload)
                .map_err(|cause| DecodeError::new(type_name::<Req>(), cause))
                .map_err(Error::Decode)?;
            Some(bytes)
        }
        None => None,
    };

    auth.authenticate(&mut headers);

    let mut request = Request::new(method, path).with_headers(headers);
    if let Some(bytes) = body_bytes {
        request = request.body_bytes(bytes);
    }
    Ok(request)
}
