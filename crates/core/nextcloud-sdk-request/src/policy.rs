use std::time::Duration;

/// How often and how long to wait between attempts.
///
/// Delays grow exponentially from `base`, doubling per attempt, and are capped
/// at `max` so a long outage cannot push the next attempt hours away.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetryPolicy {
    max_attempts: u32,
    base: Duration,
    max: Duration,
}

impl RetryPolicy {
    /// The default: three attempts, 250 ms doubling to at most 4 s.
    pub const fn new() -> Self {
        Self {
            max_attempts: 3,
            base: Duration::from_millis(250),
            max: Duration::from_secs(4),
        }
    }

    /// A policy that never retries.
    pub const fn none() -> Self {
        Self {
            max_attempts: 1,
            base: Duration::ZERO,
            max: Duration::ZERO,
        }
    }

    /// Sets the total number of attempts, first one included.
    #[must_use]
    pub const fn with_max_attempts(mut self, max_attempts: u32) -> Self {
        self.max_attempts = max_attempts;
        self
    }

    /// Sets the delay before the second attempt.
    #[must_use]
    pub const fn with_base_delay(mut self, base: Duration) -> Self {
        self.base = base;
        self
    }

    /// Sets the ceiling for the doubling delay.
    #[must_use]
    pub const fn with_max_delay(mut self, max: Duration) -> Self {
        self.max = max;
        self
    }

    /// Total attempts allowed, first one included.
    pub const fn max_attempts(&self) -> u32 {
        self.max_attempts
    }

    /// Whether an attempt number may still be followed by another.
    pub const fn can_retry_after(&self, attempt: u32) -> bool {
        attempt < self.max_attempts
    }

    /// Delay before attempt `attempt + 1`, given `attempt` has just failed.
    pub fn delay_after(&self, attempt: u32) -> Duration {
        if attempt == 0 || self.base.is_zero() {
            return Duration::ZERO;
        }
        let exponent = attempt.saturating_sub(1).min(16);
        let scaled = self.base.saturating_mul(1u32 << exponent);
        scaled.min(self.max)
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::RetryPolicy;

    #[test]
    fn first_failure_waits_the_base_delay() {
        let policy = RetryPolicy::new().with_base_delay(Duration::from_millis(100));
        assert_eq!(policy.delay_after(1), Duration::from_millis(100));
    }

    #[test]
    fn delay_doubles_per_attempt() {
        let policy = RetryPolicy::new()
            .with_base_delay(Duration::from_millis(100))
            .with_max_delay(Duration::from_secs(10));
        assert_eq!(policy.delay_after(1), Duration::from_millis(100));
        assert_eq!(policy.delay_after(2), Duration::from_millis(200));
        assert_eq!(policy.delay_after(3), Duration::from_millis(400));
    }

    #[test]
    fn delay_is_capped() {
        let policy = RetryPolicy::new()
            .with_base_delay(Duration::from_secs(1))
            .with_max_delay(Duration::from_secs(3));
        assert_eq!(policy.delay_after(9), Duration::from_secs(3));
    }

    #[test]
    fn large_attempt_counts_do_not_overflow() {
        let policy = RetryPolicy::new().with_max_delay(Duration::from_secs(2));
        assert_eq!(policy.delay_after(u32::MAX), Duration::from_secs(2));
    }

    #[test]
    fn none_never_retries() {
        let policy = RetryPolicy::none();
        assert!(!policy.can_retry_after(1));
        assert_eq!(policy.delay_after(1), Duration::ZERO);
    }

    #[test]
    fn can_retry_follows_the_attempt_budget() {
        let policy = RetryPolicy::new().with_max_attempts(3);
        assert!(policy.can_retry_after(1));
        assert!(policy.can_retry_after(2));
        assert!(!policy.can_retry_after(3));
    }
}
