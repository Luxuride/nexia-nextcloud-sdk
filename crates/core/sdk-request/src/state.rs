use std::fmt;

use super::failure::Failure;
use super::policy::RetryPolicy;

/// Where one API call currently stands.
///
/// The state carries no response body: it records progress, not payload. The
/// caller keeps the [`Response`](sdk_core::Response) it received, which
/// means an application can log the state machine freely without leaking
/// file contents or OCS error bodies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestState {
    /// Not started yet.
    Idle,
    /// Attempt `attempt` is in flight or about to be.
    InFlight {
        /// 1-based attempt number.
        attempt: u32,
    },
    /// Attempt `attempt` failed with `failure` and the policy allows another.
    RetryScheduled {
        /// Attempt that just failed.
        attempt: u32,
        /// Why it failed.
        failure: Failure,
        /// How long to wait before the next attempt.
        delay: std::time::Duration,
    },
    /// The call succeeded; the response is with the caller.
    Succeeded {
        /// Attempt that produced the answer.
        attempt: u32,
    },
    /// The call failed and will not be attempted again.
    Failed {
        /// Final attempt number.
        attempt: u32,
        /// Why it failed.
        failure: Failure,
    },
    /// The caller gave up before an answer arrived.
    Cancelled {
        /// Attempt that was in flight, if any.
        attempt: u32,
    },
}

impl RequestState {
    /// Whether the call is still making progress.
    pub const fn is_active(self) -> bool {
        matches!(
            self,
            Self::Idle | Self::InFlight { .. } | Self::RetryScheduled { .. }
        )
    }

    /// Whether a response or a final failure has been reached.
    pub const fn is_finished(self) -> bool {
        matches!(
            self,
            Self::Succeeded { .. } | Self::Failed { .. } | Self::Cancelled { .. }
        )
    }

    /// Whether the call ultimately succeeded.
    pub const fn is_success(self) -> bool {
        matches!(self, Self::Succeeded { .. })
    }

    /// The failure that ended or interrupted the call, if any.
    pub const fn failure(self) -> Option<Failure> {
        match self {
            Self::RetryScheduled { failure, .. } | Self::Failed { failure, .. } => Some(failure),
            Self::Idle
            | Self::InFlight { .. }
            | Self::Succeeded { .. }
            | Self::Cancelled { .. } => None,
        }
    }

    /// Attempt number involved, when the state has one.
    pub const fn attempt(self) -> u32 {
        match self {
            Self::Idle => 0,
            Self::InFlight { attempt }
            | Self::RetryScheduled { attempt, .. }
            | Self::Succeeded { attempt }
            | Self::Failed { attempt, .. }
            | Self::Cancelled { attempt } => attempt,
        }
    }

    /// Short name for logs and UI labels.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::InFlight { .. } => "in_flight",
            Self::RetryScheduled { .. } => "retry_scheduled",
            Self::Succeeded { .. } => "succeeded",
            Self::Failed { .. } => "failed",
            Self::Cancelled { .. } => "cancelled",
        }
    }

    /// Fraction of the attempt budget used, for a progress bar.
    ///
    /// Returns a value in `0.0..=1.0`; a call that succeeded on its first
    /// attempt reports `1.0`.
    pub fn progress(self, policy: &RetryPolicy) -> f32 {
        let budget = policy.max_attempts().max(1);
        match self {
            Self::Idle => 0.0,
            state if state.is_finished() => 1.0,
            Self::InFlight { attempt } | Self::RetryScheduled { attempt, .. } => {
                let used = attempt.min(budget) as f32;
                (used / budget as f32).clamp(0.0, 1.0)
            }
            Self::Succeeded { .. } | Self::Failed { .. } | Self::Cancelled { .. } => 1.0,
        }
    }
}

impl fmt::Display for RequestState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RetryScheduled {
                attempt,
                failure,
                delay,
            } => write!(
                f,
                "attempt {attempt} failed ({failure:?}); retrying in {delay:?}"
            ),
            Self::Failed { attempt, failure } => {
                write!(f, "attempt {attempt} failed ({failure:?}); giving up")
            }
            other => f.write_str(other.name()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::RequestState;
    use crate::failure::Failure;
    use crate::policy::RetryPolicy;

    #[test]
    fn retry_and_failure_states_carry_the_cause() {
        let retry = RequestState::RetryScheduled {
            attempt: 1,
            failure: Failure::Transport,
            delay: Duration::from_millis(250),
        };
        assert_eq!(retry.failure(), Some(Failure::Transport));
        assert!(retry.is_active());
        assert!(!retry.is_finished());

        let failed = RequestState::Failed {
            attempt: 3,
            failure: Failure::Status(404),
        };
        assert_eq!(failed.failure(), Some(Failure::Status(404)));
        assert!(failed.is_finished());
        assert!(!failed.is_active());
    }

    #[test]
    fn progress_grows_with_the_attempt_budget() {
        let policy = RetryPolicy::new().with_max_attempts(4);
        assert_eq!(RequestState::Idle.progress(&policy), 0.0);
        assert_eq!(RequestState::InFlight { attempt: 2 }.progress(&policy), 0.5);
        assert_eq!(
            RequestState::Succeeded { attempt: 2 }.progress(&policy),
            1.0
        );
    }

    #[test]
    fn display_names_the_attempt_and_cause() {
        let failed = RequestState::Failed {
            attempt: 2,
            failure: Failure::Status(500),
        };
        let rendered = failed.to_string();
        assert!(rendered.contains("attempt 2"));
        assert!(rendered.contains("Status(500)"));
    }
}
