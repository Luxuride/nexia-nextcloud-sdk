# nextcloud-sdk-ratelimit

> Dynamic bandwidth rate limiting, token-bucket traffic shaping, and transfer progress tracking for the Nextcloud Rust SDK.

Part of the [`nexia-nextcloud-sdk`](https://github.com/Luxuride/nextcloud-rust-sdk) workspace.

## Installation

```toml
[dependencies]
nextcloud-sdk-ratelimit = "0.1.2"
```

## Features

- `BandwidthLimit`: Flexible rate representation (`from_kib`, `from_mib`, `bytes_per_second`, `unlimited`).
- `TokenBucket`: Thread-safe, non-blocking token bucket algorithm for async pacing.
- `BandwidthLimiter`: High-level traffic shaper coordinating independent upload/download buckets.
- `TransferProgress`: Real-time transfer metrics (transferred bytes, total bytes, fraction, percentage).

## Example

```rust,no_run
use nextcloud_sdk_ratelimit::{BandwidthLimit, BandwidthLimiter};

let limiter = BandwidthLimiter::new(
    BandwidthLimit::from_mib(2),    // 2 MiB/s upload cap
    BandwidthLimit::from_mib(10),   // 10 MiB/s download cap
);

// Dynamically adjust on running transfers
limiter.set_download_limit(BandwidthLimit::from_kib(500));
```

## License

GNU General Public License v3.0 or later ([`GPL-3.0-or-later`](https://www.gnu.org/licenses/gpl-3.0.html)).
