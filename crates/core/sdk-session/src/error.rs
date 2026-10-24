use std::error::Error;
use std::fmt;

use crate::event::SessionEvent;
use crate::state::SessionState;

/// An event arrived that the machine cannot use in its current state.
///
/// This is a programming error on the caller's side (feeding a poll answer
/// into a session that was logged out, say), not a server failure: reporting
/// it keeps a stale answer from silently overwriting a newer state.
#[derive(Debug)]
pub struct SessionError {
    state: &'static str,
    event: &'static str,
}

impl SessionError {
    /// Records the rejected pairing.
    pub fn new(state: &SessionState, event: &SessionEvent) -> Self {
        Self {
            state: state.name(),
            event: event.name(),
        }
    }

    /// State the event was rejected in.
    pub const fn state(&self) -> &'static str {
        self.state
    }

    /// Event that was rejected.
    pub const fn event(&self) -> &'static str {
        self.event
    }
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "event `{}` is not valid while the session is `{}`",
            self.event, self.state
        )
    }
}

impl Error for SessionError {}

#[cfg(test)]
mod tests {
    use super::SessionError;
    use crate::event::SessionEvent;
    use crate::state::SessionState;

    #[test]
    fn display_names_state_and_event() {
        let error = SessionError::new(&SessionState::Idle, &SessionEvent::PollPending);
        let rendered = error.to_string();
        assert!(rendered.contains("poll_pending"));
        assert!(rendered.contains("idle"));
    }
}
