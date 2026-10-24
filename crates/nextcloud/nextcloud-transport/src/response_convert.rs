use sdk_core::{Error, Headers, Response, StatusCode};

/// Converts a `reqwest` response into the SDK's raw response.
///
/// The status is carried through as-is: a `404` or `500` is data for the API
/// layer, not a transport failure. Header names keep the casing the server
/// used; a value that is not visible ASCII cannot be represented by
/// `sdk_core::Header` and is dropped, and a name repeated by the server
/// collapses to its last value because the core model maps a name to one
/// value.
pub(crate) async fn to_response(response: reqwest::Response) -> Result<Response, Error> {
    let status = StatusCode::new(response.status().as_u16());
    let headers = collect_headers(response.headers());
    // `bytes` is the byte-exact body accessor; `text` would transcode a
    // charset and XML payloads must reach the decoders untouched.
    let body = response.bytes().await.map_err(Error::transport)?;
    Ok(Response::new(status, headers, body.as_ref().to_vec()))
}

fn collect_headers(source: &reqwest::header::HeaderMap) -> Headers {
    let mut headers = Headers::new();
    for (name, value) in source {
        if let Ok(value) = value.to_str() {
            headers.set(name.as_str(), value);
        }
    }
    headers
}

#[cfg(test)]
mod tests {
    use reqwest::header::{HeaderMap, HeaderValue};

    use super::collect_headers;

    #[test]
    fn header_names_and_values_survive() {
        let mut source = HeaderMap::new();
        source.insert("content-type", HeaderValue::from_static("application/xml"));
        source.insert("x-request-id", HeaderValue::from_static("abc123"));

        let headers = collect_headers(&source);
        assert_eq!(headers.get("Content-Type"), Some("application/xml"));
        assert_eq!(headers.get("x-request-id"), Some("abc123"));
        assert_eq!(headers.len(), 2);
    }

    #[test]
    fn non_representable_values_are_skipped() {
        let mut source = HeaderMap::new();
        source.insert("x-binary", HeaderValue::from_bytes(b"\xff\xfe").unwrap());
        source.insert("accept", HeaderValue::from_static("application/json"));

        let headers = collect_headers(&source);
        assert_eq!(headers.get("x-binary"), None);
        assert_eq!(headers.get("accept"), Some("application/json"));
    }
}
