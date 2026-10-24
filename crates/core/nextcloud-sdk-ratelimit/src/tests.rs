use std::num::NonZeroU64;
use std::time::{Duration, Instant};

use crate::bucket::TokenBucket;
use crate::limit::BandwidthLimit;
use crate::limiter::BandwidthLimiter;

#[test]
fn limit_constructors_and_conversions() {
    let unlim = BandwidthLimit::unlimited();
    assert!(unlim.is_unlimited());
    assert_eq!(unlim.as_bytes_per_second(), None);

    let zero = BandwidthLimit::bytes_per_second(0);
    assert!(zero.is_unlimited());

    let kib = BandwidthLimit::from_kib(5);
    assert!(!kib.is_unlimited());
    assert_eq!(kib.as_bytes_per_second(), NonZeroU64::new(5 * 1024));

    let mib = BandwidthLimit::from_mib(2);
    assert_eq!(mib.as_bytes_per_second(), NonZeroU64::new(2 * 1024 * 1024));

    let from_u64: BandwidthLimit = 500.into();
    assert_eq!(from_u64.as_bytes_per_second(), NonZeroU64::new(500));

    let from_opt: BandwidthLimit = Some(1000).into();
    assert_eq!(from_opt.as_bytes_per_second(), NonZeroU64::new(1000));

    let from_none: BandwidthLimit = None.into();
    assert!(from_none.is_unlimited());
}

#[test]
fn limit_display_formatting() {
    assert_eq!(BandwidthLimit::unlimited().to_string(), "unlimited");
    assert_eq!(BandwidthLimit::from_mib(5).to_string(), "5 MiB/s");
    assert_eq!(BandwidthLimit::from_kib(50).to_string(), "50 KiB/s");
    assert_eq!(
        BandwidthLimit::bytes_per_second(1234).to_string(),
        "1234 B/s"
    );
}

#[tokio::test]
async fn unlimited_bucket_incurs_no_delay() {
    let bucket = TokenBucket::unlimited();
    let start = Instant::now();
    bucket.acquire(100_000_000).await;
    assert!(start.elapsed() < Duration::from_millis(50));
    assert!(bucket.try_acquire(100_000_000));
}

#[test]
fn try_acquire_respects_token_availability() {
    let rate = 1000; // 1000 B/s, initial burst 64 KiB
    let bucket = TokenBucket::new(BandwidthLimit::bytes_per_second(rate));

    assert!(bucket.try_acquire(1000));
    assert!(bucket.try_acquire(0));
}

#[tokio::test]
async fn acquire_paces_rate_appropriately() {
    // 500 bytes per second, consume the initial burst
    let bucket = TokenBucket::new(BandwidthLimit::bytes_per_second(500));
    // Drain initial burst
    let _ = bucket.try_acquire(64 * 1024);

    let start = Instant::now();
    // Acquiring 500 bytes should take ~1 second
    bucket.acquire(250).await;
    let elapsed = start.elapsed();
    assert!(
        elapsed >= Duration::from_millis(400),
        "Expected at least ~400ms, got {elapsed:?}"
    );
}

#[tokio::test]
async fn dynamic_rate_adjustment_applies_immediately() {
    let bucket = TokenBucket::new(BandwidthLimit::bytes_per_second(100));
    assert_eq!(bucket.limit(), BandwidthLimit::bytes_per_second(100));

    // Update to unlimited
    bucket.set_limit(BandwidthLimit::unlimited());
    assert!(bucket.limit().is_unlimited());

    let start = Instant::now();
    bucket.acquire(10_000_000).await;
    assert!(start.elapsed() < Duration::from_millis(50));

    // Update back to constrained
    bucket.set_limit(BandwidthLimit::from_mib(10));
    assert_eq!(bucket.limit(), BandwidthLimit::from_mib(10));
}

#[tokio::test]
async fn limiter_clones_share_state() {
    let limiter =
        BandwidthLimiter::new(BandwidthLimit::from_kib(50), BandwidthLimit::from_kib(100));
    let clone = limiter.clone();

    assert_eq!(clone.upload_limit(), BandwidthLimit::from_kib(50));
    assert_eq!(clone.download_limit(), BandwidthLimit::from_kib(100));

    limiter.set_upload_limit(BandwidthLimit::from_mib(1));
    assert_eq!(clone.upload_limit(), BandwidthLimit::from_mib(1));

    clone.set_download_limit(BandwidthLimit::unlimited());
    assert!(limiter.download_limit().is_unlimited());
}

#[test]
fn transfer_progress_calculations() {
    use crate::progress::TransferProgress;

    let p1 = TransferProgress::new(500, Some(1000));
    assert_eq!(p1.transferred_bytes(), 500);
    assert_eq!(p1.total_bytes(), Some(1000));
    assert_eq!(p1.fraction(), Some(0.5));
    assert_eq!(p1.percentage(), Some(50.0));
    assert!(!p1.is_complete());

    let p2 = TransferProgress::new(1000, Some(1000));
    assert!(p2.is_complete());
    assert_eq!(p2.fraction(), Some(1.0));
    assert_eq!(p2.percentage(), Some(100.0));

    let p3 = TransferProgress::new(250, None);
    assert_eq!(p3.fraction(), None);
    assert_eq!(p3.percentage(), None);
    assert!(!p3.is_complete());

    let p4 = TransferProgress::new(0, Some(0));
    assert_eq!(p4.fraction(), Some(1.0));
    assert!(p4.is_complete());
}
