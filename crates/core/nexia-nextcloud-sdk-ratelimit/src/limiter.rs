use std::sync::Arc;

use crate::bucket::TokenBucket;
use crate::limit::BandwidthLimit;

/// High-level traffic shaper coordinating upload and download token buckets.
///
/// Cheaply cloneable via internal [`Arc`] pointers. Dynamically reconfigurable
/// at runtime, allowing speed limits to be updated on-the-fly while active transfers
/// are taking place.
#[derive(Clone, Debug)]
pub struct BandwidthLimiter {
    upload: Arc<TokenBucket>,
    download: Arc<TokenBucket>,
}

impl Default for BandwidthLimiter {
    fn default() -> Self {
        Self::unlimited()
    }
}

impl BandwidthLimiter {
    /// Creates a limiter with explicit upload and download constraints.
    pub fn new(upload: BandwidthLimit, download: BandwidthLimit) -> Self {
        Self {
            upload: Arc::new(TokenBucket::new(upload)),
            download: Arc::new(TokenBucket::new(download)),
        }
    }

    /// Creates an unconstrained limiter with no upload or download throttling.
    pub fn unlimited() -> Self {
        Self::new(BandwidthLimit::Unlimited, BandwidthLimit::Unlimited)
    }

    /// The currently active upload bandwidth limit.
    pub fn upload_limit(&self) -> BandwidthLimit {
        self.upload.limit()
    }

    /// The currently active download bandwidth limit.
    pub fn download_limit(&self) -> BandwidthLimit {
        self.download.limit()
    }

    /// Dynamically updates the upload bandwidth limit.
    pub fn set_upload_limit(&self, limit: BandwidthLimit) {
        tracing::info!(?limit, "Setting upload bandwidth limit");
        self.upload.set_limit(limit);
    }

    /// Dynamically updates the download bandwidth limit.
    pub fn set_download_limit(&self, limit: BandwidthLimit) {
        tracing::info!(?limit, "Setting download bandwidth limit");
        self.download.set_limit(limit);
    }

    /// Throttles an outbound upload transfer of `bytes` according to the upload limit.
    pub async fn throttle_upload(&self, bytes: usize) {
        tracing::trace!(bytes, "Throttling upload chunk");
        self.upload.acquire(bytes).await;
    }

    /// Throttles an inbound download transfer of `bytes` according to the download limit.
    pub async fn throttle_download(&self, bytes: usize) {
        tracing::trace!(bytes, "Throttling download chunk");
        self.download.acquire(bytes).await;
    }

    /// Reference to the underlying upload token bucket.
    pub const fn upload_bucket(&self) -> &Arc<TokenBucket> {
        &self.upload
    }

    /// Reference to the underlying download token bucket.
    pub const fn download_bucket(&self) -> &Arc<TokenBucket> {
        &self.download
    }
}
