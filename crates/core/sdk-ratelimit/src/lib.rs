//! Bandwidth rate limiting and token-bucket traffic shaping for the Nextcloud Rust SDK.
//!
//! Provides [`BandwidthLimit`], [`TokenBucket`], and [`BandwidthLimiter`] for
//! asynchronously shaping upload and download throughput. The limits can be dynamically
//! reconfigured on active connections without reconnection or client reconstruction.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod bucket;
mod limit;
mod limiter;
mod progress;

#[cfg(test)]
mod tests;

pub use bucket::TokenBucket;
pub use limit::BandwidthLimit;
pub use limiter::BandwidthLimiter;
pub use progress::{ProgressCallback, TransferProgress};
