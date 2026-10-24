use std::any::type_name;

use nextcloud_sdk_core::{ApiPath, DecodeError, Error, HttpMethod, Response, StatusError};
use serde::de::DeserializeOwned;

use crate::envelope::{OcsEnvelope, OcsStatus, VerdictEnvelope};
use crate::error::StatusCodeError;

/// Decodes an OCS response body, verifying HTTP status and OCS envelope verdict.
///
/// Nextcloud OCS endpoints report verdicts in their `meta` block (statuscode `100`
/// or `200` for success; failure codes like `101` or `997` for errors). When
/// decoding into `()`, successful envelopes are accepted even when the endpoint
/// returns empty data (e.g. `[]`, `{}`) or when caller only cares about verdict success.
pub(crate) fn decode_response<Res: DeserializeOwned>(
    method: HttpMethod,
    path: ApiPath,
    response: &Response,
) -> Result<Res, Error> {
    if !response.status().is_success() {
        return Err(status_error(method, path, response));
    }

    if response.body().is_empty() {
        return serde_json::from_str::<Res>("null")
            .map_err(|cause| DecodeError::new(type_name::<OcsEnvelope<Res>>(), cause))
            .map_err(Error::Decode);
    }

    // The verdict is read first: a failure body is not the endpoint's
    // payload shape, so decoding it as `data` would report the OCS
    // `statuscode` as a malformed body and lose the reason.
    let verdict: VerdictEnvelope = serde_json::from_slice(response.body())
        .map_err(|cause| DecodeError::new(type_name::<OcsEnvelope<Res>>(), cause))
        .map_err(Error::Decode)?;

    match verdict.ocs.meta.verdict() {
        Some(OcsStatus::Ok) => {}
        Some(OcsStatus::Failure) => return Err(status_error(method, path, response)),
        // An envelope that carries no readable verdict is malformed, not a
        // success: guessing here would turn a broken answer into a payload
        // the caller cannot distinguish from a real one.
        None => {
            return Err(Error::Decode(DecodeError::new(
                type_name::<OcsEnvelope<Res>>(),
                StatusCodeError::new(verdict.ocs.meta.statuscode()),
            )));
        }
    }

    match serde_json::from_slice::<OcsEnvelope<Res>>(response.body()) {
        Ok(envelope) => Ok(envelope.into_data()),
        Err(cause) => {
            if let Ok(value) = serde_json::from_str::<Res>("null") {
                Ok(value)
            } else {
                Err(Error::Decode(DecodeError::new(
                    type_name::<OcsEnvelope<Res>>(),
                    cause,
                )))
            }
        }
    }
}

/// Builds an [`Error::Status`] error from an HTTP response, method, and path.
pub(crate) fn status_error(method: HttpMethod, path: ApiPath, response: &Response) -> Error {
    Error::Status(StatusError::new(
        method.as_str(),
        path,
        response.status(),
        response.body(),
    ))
}
