use std::any::type_name;

use nexia_nextcloud_core::{ApiPath, DecodeError, Error, HttpMethod, Response, StatusError};
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
        tracing::warn!(
            method = ?method,
            path = %path.as_str(),
            status = %response.status(),
            "OCS response received non-success HTTP status"
        );
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
        Some(OcsStatus::Ok) => {
            tracing::trace!(
                method = ?method,
                path = %path.as_str(),
                statuscode = verdict.ocs.meta.statuscode(),
                "OCS response statuscode verified successfully"
            );
        }
        Some(OcsStatus::Failure) => {
            tracing::error!(
                method = ?method,
                path = %path.as_str(),
                statuscode = verdict.ocs.meta.statuscode(),
                message = %verdict.ocs.meta.message(),
                "OCS response envelope reported failure verdict"
            );
            return Err(status_error(method, path, response));
        }
        // An envelope that carries no readable verdict is malformed, not a
        // success: guessing here would turn a broken answer into a payload
        // the caller cannot distinguish from a real one.
        None => {
            tracing::error!(
                method = ?method,
                path = %path.as_str(),
                statuscode = verdict.ocs.meta.statuscode(),
                "OCS response envelope missing valid verdict"
            );
            return Err(Error::Decode(DecodeError::new(
                type_name::<OcsEnvelope<Res>>(),
                StatusCodeError::new(verdict.ocs.meta.statuscode()),
            )));
        }
    }

    match serde_json::from_slice::<OcsEnvelope<Res>>(response.body()) {
        Ok(envelope) => Ok(envelope.into_data()),
        Err(cause) => {
            // Endpoints that report nothing in `data` answer with `[]`, `{}` or
            // no `data` field at all, and a caller that asked for `()` only
            // wants the verdict checked above. That leniency is granted only
            // once the envelope itself is known to be well formed: the raw
            // document must carry an `ocs.meta` object, so a garbled or
            // truncated body is reported instead of becoming an empty success.
            let lenient = envelope_is_well_formed(response.body())
                .then(|| serde_json::from_str::<Res>("null").ok())
                .flatten();
            if let Some(value) = lenient {
                return Ok(value);
            }
            Err(Error::Decode(DecodeError::new(
                type_name::<OcsEnvelope<Res>>(),
                cause,
            )))
        }
    }
}

/// Returns whether `body` parses as an OCS document carrying an `ocs.meta` object.
fn envelope_is_well_formed(body: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .as_ref()
        .and_then(|document| document.get("ocs"))
        .and_then(|ocs| ocs.get("meta"))
        .is_some_and(serde_json::Value::is_object)
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
