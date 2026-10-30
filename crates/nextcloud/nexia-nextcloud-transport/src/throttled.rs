use std::convert::Infallible;

use futures_util::stream::{self, StreamExt};
use nexia_nextcloud_sdk_ratelimit::{BandwidthLimiter, ProgressCallback, TransferProgress};
use reqwest::Body;

const DEFAULT_CHUNK_SIZE: usize = 32 * 1024; // 32 KiB

/// Prepares a request body, pacing outbound chunks according to the upload limiter
/// and reporting progress if configured.
pub(crate) fn prepare_request_body(
    body: Vec<u8>,
    limiter: BandwidthLimiter,
    progress: Option<ProgressCallback>,
) -> Body {
    let total_bytes = body.len() as u64;

    if limiter.upload_limit().is_unlimited() && progress.is_none() {
        return Body::from(body);
    }

    if let Some(ref cb) = progress {
        cb(TransferProgress::new(0, Some(total_bytes)));
    }

    let mut transferred = 0u64;
    let chunks: Vec<(Vec<u8>, u64)> = body
        .chunks(DEFAULT_CHUNK_SIZE)
        .map(|c| {
            transferred = transferred.saturating_add(c.len() as u64);
            (c.to_vec(), transferred)
        })
        .collect();

    let stream = stream::iter(chunks).then(move |(chunk, current_transferred)| {
        let limiter = limiter.clone();
        let progress = progress.clone();
        let chunk_len = chunk.len();

        async move {
            tracing::trace!(
                chunk_len,
                current_transferred,
                total_bytes,
                "Pacing upload stream chunk"
            );
            limiter.throttle_upload(chunk_len).await;
            if let Some(cb) = progress {
                cb(TransferProgress::new(
                    current_transferred,
                    Some(total_bytes),
                ));
            }
            Ok::<_, Infallible>(chunk)
        }
    });

    Body::wrap_stream(stream)
}
