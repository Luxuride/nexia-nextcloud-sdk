use futures_util::StreamExt;
use nexia_nextcloud_core::{Error, Headers, Response, StatusCode};
use nextcloud_sdk_ratelimit::{BandwidthLimiter, ProgressCallback, TransferProgress};

/// Converts a `reqwest` response into the SDK's raw response, applying download rate limiting
/// and progress reporting if configured.
pub(crate) async fn to_response(
    response: reqwest::Response,
    limiter: &BandwidthLimiter,
    progress: Option<&ProgressCallback>,
) -> Result<Response, Error> {
    let status = StatusCode::new(response.status().as_u16());
    let headers = collect_headers(response.headers());

    let total_bytes: Option<u64> = response
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|val| val.to_str().ok())
        .and_then(|s| s.parse().ok());

    let body = if limiter.download_limit().is_unlimited() && progress.is_none() {
        let bytes = response.bytes().await.map_err(Error::transport)?;
        bytes.as_ref().to_vec()
    } else {
        let mut body = Vec::with_capacity(total_bytes.unwrap_or(0) as usize);
        let mut transferred_bytes: u64 = 0;
        let mut stream = response.bytes_stream();

        if let Some(cb) = progress {
            cb(TransferProgress::new(0, total_bytes));
        }

        while let Some(chunk_res) = stream.next().await {
            let chunk = chunk_res.map_err(Error::transport)?;
            let chunk_len = chunk.len();
            tracing::trace!(chunk_len, transferred_bytes, "Pacing download stream chunk");
            limiter.throttle_download(chunk_len).await;
            transferred_bytes = transferred_bytes.saturating_add(chunk_len as u64);
            body.extend_from_slice(&chunk);

            if let Some(cb) = progress {
                cb(TransferProgress::new(transferred_bytes, total_bytes));
            }
        }
        body
    };

    Ok(Response::new(status, headers, body))
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
