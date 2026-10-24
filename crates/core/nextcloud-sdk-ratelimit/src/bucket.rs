use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::limit::BandwidthLimit;

#[derive(Debug)]
struct BucketState {
    limit: BandwidthLimit,
    available_tokens: f64,
    last_replenished: Instant,
}

impl BucketState {
    fn new(limit: BandwidthLimit) -> Self {
        let initial_tokens = match limit {
            BandwidthLimit::Unlimited => 0.0,
            BandwidthLimit::BytesPerSecond(rate) => (rate.get() as f64).max(64.0 * 1024.0),
        };

        Self {
            limit,
            available_tokens: initial_tokens,
            last_replenished: Instant::now(),
        }
    }

    fn replenish(&mut self, now: Instant) {
        if let BandwidthLimit::BytesPerSecond(rate) = self.limit {
            let rate_f64 = rate.get() as f64;
            let max_burst = rate_f64.max(64.0 * 1024.0);

            if now > self.last_replenished {
                let elapsed = now.duration_since(self.last_replenished).as_secs_f64();
                self.available_tokens = (self.available_tokens + elapsed * rate_f64).min(max_burst);
                self.last_replenished = now;
            }
        }
    }
}

/// Token bucket algorithm implementation for bandwidth pacing.
///
/// Thread-safe and asynchronous. Allows dynamic rate adjustments on running transfers
/// without interrupting execution or causing lock contention.
#[derive(Debug)]
pub struct TokenBucket {
    state: Mutex<BucketState>,
}

impl Default for TokenBucket {
    fn default() -> Self {
        Self::unlimited()
    }
}

impl TokenBucket {
    /// Creates a new token bucket with the given bandwidth limit constraint.
    pub fn new(limit: BandwidthLimit) -> Self {
        Self {
            state: Mutex::new(BucketState::new(limit)),
        }
    }

    /// Creates an unconstrained token bucket that incurs no transfer delays.
    pub fn unlimited() -> Self {
        Self::new(BandwidthLimit::Unlimited)
    }

    /// Returns the currently active bandwidth limit.
    pub fn limit(&self) -> BandwidthLimit {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.limit
    }

    /// Dynamically adjusts the active bandwidth limit.
    ///
    /// The update applies immediately to subsequent and pending token acquisitions.
    pub fn set_limit(&self, limit: BandwidthLimit) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        state.replenish(now);

        match limit {
            BandwidthLimit::Unlimited => {
                state.available_tokens = 0.0;
            }
            BandwidthLimit::BytesPerSecond(rate) => {
                let rate_f64 = rate.get() as f64;
                let max_burst = rate_f64.max(64.0 * 1024.0);
                state.available_tokens = state.available_tokens.clamp(-max_burst, max_burst);
            }
        }

        state.limit = limit;
        state.last_replenished = now;
    }

    /// Attempts to immediately acquire `bytes` tokens without awaiting.
    ///
    /// Returns `true` if sufficient tokens were available and deducted, or `false` otherwise.
    pub fn try_acquire(&self, bytes: usize) -> bool {
        if bytes == 0 {
            return true;
        }

        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        state.replenish(now);

        match state.limit {
            BandwidthLimit::Unlimited => true,
            BandwidthLimit::BytesPerSecond(_) => {
                if state.available_tokens >= bytes as f64 {
                    state.available_tokens -= bytes as f64;
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Asynchronously acquires `bytes` tokens, pacing execution via [`tokio::time::sleep`]
    /// when the requested byte count exceeds available bucket capacity.
    pub async fn acquire(&self, bytes: usize) {
        if bytes == 0 {
            return;
        }

        let delay = {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            let now = Instant::now();
            state.replenish(now);

            match state.limit {
                BandwidthLimit::Unlimited => Duration::ZERO,
                BandwidthLimit::BytesPerSecond(rate) => {
                    let rate_f64 = rate.get() as f64;
                    state.available_tokens -= bytes as f64;
                    if state.available_tokens < 0.0 {
                        let deficit = -state.available_tokens;
                        let delay_secs = deficit / rate_f64;
                        Duration::from_secs_f64(delay_secs)
                    } else {
                        Duration::ZERO
                    }
                }
            }
        };

        if delay > Duration::ZERO {
            tracing::trace!(
                delay_ms = delay.as_millis(),
                bytes,
                "TokenBucket pacing sleep"
            );
            tokio::time::sleep(delay).await;
        }
    }
}
