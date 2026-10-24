use std::time::Duration;

use super::failure::Failure;
use super::policy::RetryPolicy;
use super::state::RequestState;

/// What the caller should do next for one API call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Send the request (or send it again).
    Send {
        /// 1-based attempt number to send.
        attempt: u32,
    },
    /// Wait `delay`, then send attempt `attempt`.
    Wait {
        /// Delay to observe before the next attempt.
        delay: Duration,
        /// Attempt number the wait leads to.
        attempt: u32,
    },
    /// Deliver the response to the caller.
    Finish,
    /// Report the failure to the caller; no further attempt will be made.
    GiveUp,
}

/// The lifecycle of a single API call, as a pure transition table.
///
/// The machine decides *whether* and *when* to retry; it does not sleep and
/// does not open a connection. A caller loop sends the request, reports the
/// outcome, and honours the returned [`Effect`]:
///
/// ```text
/// let mut machine = RequestMachine::new(RetryPolicy::new());
/// loop {
///     match machine.start() { /* Effect::Send { attempt } */ }
///     // ... send, then:
///     match machine.failed(Failure::Transport) {
///         Effect::Wait { delay, .. } => sleep(delay).await,
///         Effect::GiveUp => break,
///         _ => {}
///     }
/// }
/// ```
#[derive(Clone, Debug)]
pub struct RequestMachine {
    policy: RetryPolicy,
    state: RequestState,
}

impl RequestMachine {
    /// A machine that has not sent anything yet.
    pub const fn new(policy: RetryPolicy) -> Self {
        Self {
            policy,
            state: RequestState::Idle,
        }
    }

    /// Current state.
    pub const fn state(&self) -> RequestState {
        self.state
    }

    /// The retry policy in force.
    pub const fn policy(&self) -> &RetryPolicy {
        &self.policy
    }

    /// Begins the call, yielding the first attempt to send.
    ///
    /// Calling `start` twice without an outcome keeps the current attempt:
    /// a caller that restarts its loop must not reset the budget.
    pub fn start(&mut self) -> Effect {
        match self.state {
            RequestState::Idle => {
                tracing::debug!(attempt = 1, "Starting first request attempt");
                self.state = RequestState::InFlight { attempt: 1 };
                Effect::Send { attempt: 1 }
            }
            RequestState::InFlight { attempt } | RequestState::RetryScheduled { attempt, .. } => {
                tracing::trace!(attempt, "Request already in-flight or retry scheduled");
                Effect::Send { attempt }
            }
            RequestState::Succeeded { .. } | RequestState::Failed { .. } => Effect::Finish,
            RequestState::Cancelled { .. } => Effect::GiveUp,
        }
    }

    /// Reports that the in-flight attempt produced a response.
    pub fn succeeded(&mut self) -> Effect {
        let attempt = self.state.attempt().max(1);
        tracing::info!(attempt, "Request succeeded");
        self.state = RequestState::Succeeded { attempt };
        Effect::Finish
    }

    /// Reports that the in-flight attempt failed.
    ///
    /// Returns [`Effect::Wait`] when the policy allows another attempt, or
    /// [`Effect::GiveUp`] when it does not.
    pub fn failed(&mut self, failure: Failure) -> Effect {
        let attempt = self.state.attempt().max(1);
        if failure.is_retryable() && self.policy.can_retry_after(attempt) {
            let delay = self.policy.delay_after(attempt);
            tracing::warn!(
                ?failure,
                attempt,
                delay_ms = delay.as_millis(),
                "Request attempt failed; scheduling retry"
            );
            self.state = RequestState::RetryScheduled {
                attempt,
                failure,
                delay,
            };
            Effect::Wait {
                delay,
                attempt: attempt + 1,
            }
        } else {
            tracing::error!(
                ?failure,
                attempt,
                "Request attempt failed; retry policy exhausted or non-retryable failure"
            );
            self.state = RequestState::Failed { attempt, failure };
            Effect::GiveUp
        }
    }

    /// Marks the wait as over and moves on to the next attempt.
    pub fn retry_now(&mut self) -> Effect {
        match self.state {
            RequestState::RetryScheduled { attempt, .. } => {
                let next = attempt + 1;
                tracing::debug!(attempt = next, "Proceeding with next retry attempt");
                self.state = RequestState::InFlight { attempt: next };
                Effect::Send { attempt: next }
            }
            RequestState::InFlight { attempt } => Effect::Send { attempt },
            _ => Effect::Finish,
        }
    }

    /// Abandons the call.
    pub fn cancel(&mut self) -> Effect {
        if self.state.is_finished() {
            return Effect::Finish;
        }
        tracing::info!(attempt = self.state.attempt(), "Cancelling request");
        self.state = RequestState::Cancelled {
            attempt: self.state.attempt(),
        };
        Effect::GiveUp
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{Effect, RequestMachine};
    use crate::failure::Failure;
    use crate::policy::RetryPolicy;
    use crate::state::RequestState;

    fn machine() -> RequestMachine {
        RequestMachine::new(
            RetryPolicy::new()
                .with_max_attempts(3)
                .with_base_delay(Duration::from_millis(10)),
        )
    }

    #[test]
    fn start_sends_the_first_attempt() {
        let mut machine = machine();
        assert_eq!(machine.start(), Effect::Send { attempt: 1 });
        assert_eq!(machine.state(), RequestState::InFlight { attempt: 1 });
    }

    #[test]
    fn success_finishes_immediately() {
        let mut machine = machine();
        machine.start();
        assert_eq!(machine.succeeded(), Effect::Finish);
        assert!(machine.state().is_success());
    }

    #[test]
    fn retryable_failure_waits_then_resends() {
        let mut machine = machine();
        machine.start();
        let effect = machine.failed(Failure::Transport);
        assert_eq!(
            effect,
            Effect::Wait {
                delay: Duration::from_millis(10),
                attempt: 2
            }
        );
        assert_eq!(machine.retry_now(), Effect::Send { attempt: 2 });
    }

    #[test]
    fn non_retryable_failure_gives_up_at_once() {
        let mut machine = machine();
        machine.start();
        assert_eq!(machine.failed(Failure::Status(404)), Effect::GiveUp);
        assert!(!machine.state().is_active());
    }

    #[test]
    fn budget_exhaustion_gives_up() {
        let mut machine = machine();
        machine.start();
        machine.failed(Failure::Transport);
        machine.retry_now();
        machine.failed(Failure::Transport);
        machine.retry_now();
        assert_eq!(machine.failed(Failure::Transport), Effect::GiveUp);
        assert_eq!(
            machine.state(),
            RequestState::Failed {
                attempt: 3,
                failure: Failure::Transport
            }
        );
    }

    #[test]
    fn start_is_idempotent_while_in_flight() {
        let mut machine = machine();
        machine.start();
        assert_eq!(machine.start(), Effect::Send { attempt: 1 });
    }

    #[test]
    fn cancel_finishes_an_active_call() {
        let mut machine = machine();
        machine.start();
        assert_eq!(machine.cancel(), Effect::GiveUp);
        assert!(machine.state().is_finished());
        assert_eq!(machine.state().name(), "cancelled");
    }
}
