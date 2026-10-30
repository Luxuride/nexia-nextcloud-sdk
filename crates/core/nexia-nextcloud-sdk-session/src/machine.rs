use super::command::{Command, PollTarget};
use super::error::SessionError;
use super::event::{LoginStart, SessionEvent};
use super::state::{Account, LoginFlow, SessionState};

/// The account lifecycle as a pure transition table.
///
/// [`SessionMachine::handle`] takes one event and returns the next state plus
/// the [`Command`] that has to be carried out; no I/O happens here, so the
/// whole table is exercised by unit tests. The user id is resolved by the
/// application through the OCS user endpoint and supplied when credentials
/// arrive.
#[derive(Clone, Debug)]
pub struct SessionMachine {
    state: SessionState,
}

impl SessionMachine {
    /// A machine with no session.
    pub const fn new() -> Self {
        Self {
            state: SessionState::Idle,
        }
    }

    /// A machine resuming from a known state, e.g. a stored account.
    pub const fn with_state(state: SessionState) -> Self {
        Self { state }
    }

    /// Current state.
    pub const fn state(&self) -> &SessionState {
        &self.state
    }

    /// Whether requests may be sent right now.
    pub const fn is_authenticated(&self) -> bool {
        self.state.is_authenticated()
    }

    /// The account, once one is known.
    pub const fn account(&self) -> Option<&Account> {
        self.state.account()
    }

    /// Applies `event`, returning the command to perform next.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] when the event cannot apply in the current
    /// state, e.g. a poll answer after a logout.
    pub fn handle(&mut self, event: SessionEvent) -> Result<Command, SessionError> {
        let state = std::mem::replace(&mut self.state, SessionState::Idle);
        tracing::debug!(?event, current_state = ?state, "Handling session machine event");
        match self.transition(state.clone(), event.clone()) {
            Ok((next, command)) => {
                tracing::info!(
                    current_state = ?state,
                    next_state = ?next,
                    ?command,
                    "Session state transitioned successfully"
                );
                self.state = next;
                Ok(command)
            }
            Err(error) => {
                tracing::warn!(
                    current_state = ?state,
                    ?event,
                    error = %error,
                    "Invalid session state machine transition"
                );
                self.state = state;
                Err(error)
            }
        }
    }

    fn transition(
        &self,
        state: SessionState,
        event: SessionEvent,
    ) -> Result<(SessionState, Command), SessionError> {
        use SessionEvent as E;

        let current = state.clone();
        match (state, event) {
            (SessionState::Idle, E::StartRequested)
            | (SessionState::ReauthRequired(_), E::StartRequested) => {
                Ok((SessionState::Idle, Command::StartLogin))
            }

            (SessionState::Idle, E::FlowStarted(start)) => Ok(start_flow(start)),

            (SessionState::Idle, E::FlowStartFailed) => Ok((SessionState::Idle, Command::Idle)),

            // A failed or expired poll ends the flow: the token is gone server
            // side, so the only useful next step is to start over.
            (SessionState::AwaitingLogin(_), E::PollFailed)
            | (SessionState::AwaitingGrant(_), E::PollFailed)
            | (SessionState::AwaitingLogin(_), E::Expired)
            | (SessionState::AwaitingGrant(_), E::Expired) => {
                Ok((SessionState::Idle, Command::StopPolling))
            }

            (SessionState::AwaitingLogin(flow), E::PollPending)
            | (SessionState::AwaitingGrant(flow), E::PollPending) => {
                Ok((SessionState::AwaitingGrant(flow), next_poll(&current)))
            }

            (SessionState::AwaitingLogin(_), E::Granted(success))
            | (SessionState::AwaitingGrant(_), E::Granted(success)) => Ok(authenticated(success)),

            (SessionState::Authenticated(account), E::CredentialsRejected) => {
                Ok((SessionState::ReauthRequired(account), Command::StopPolling))
            }

            (SessionState::AwaitingLogin(_), E::LogoutRequested)
            | (SessionState::AwaitingGrant(_), E::LogoutRequested)
            | (SessionState::ReauthRequired(_), E::LogoutRequested)
            | (SessionState::Authenticated(_), E::LogoutRequested) => {
                Ok((SessionState::Idle, Command::StopPolling))
            }

            (state, event) => Err(SessionError::new(&state, &event)),
        }
    }
}

fn start_flow(start: LoginStart) -> (SessionState, Command) {
    let login_url = start.login_url().to_owned();
    let flow = LoginFlow::new(
        start.token().clone(),
        login_url.clone(),
        start.poll_endpoint(),
    );
    (
        SessionState::AwaitingLogin(flow),
        Command::OpenBrowser(login_url),
    )
}

fn next_poll(state: &SessionState) -> Command {
    match state.login_flow() {
        Some(flow) => Command::Poll(PollTarget::new(
            flow.poll_endpoint(),
            flow.poll_token().expose(),
        )),
        None => Command::Idle,
    }
}

fn authenticated(success: super::event::LoginSuccess) -> (SessionState, Command) {
    // The poll answer carries no user id, so the login name stands in for it
    // until the caller resolves the account identity; a file tree must not be
    // derived from `user_id` while it still equals the login name.
    let account = Account::new(
        success.server().clone(),
        success.login_name(),
        success.login_name(),
        success.app_password().clone(),
    );
    (
        SessionState::Authenticated(account.clone()),
        Command::Establish(Box::new(account)),
    )
}

impl Default for SessionMachine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use nexia_nextcloud_auth::SecretString;
    use nexia_nextcloud_core::ServerUrl;

    use super::SessionMachine;
    use crate::command::{Command, PollTarget};
    use crate::event::{LoginStart, LoginSuccess, SessionEvent};
    use crate::state::{Account, PollToken, SessionState};

    fn server() -> ServerUrl {
        ServerUrl::parse("https://cloud.example.org").expect("valid url")
    }

    fn start() -> LoginStart {
        LoginStart::new(
            "https://cloud.example.org/login/v2/flow/x",
            "https://cloud.example.org/login/v2/poll",
            PollToken::new("tok"),
        )
    }

    fn success() -> LoginSuccess {
        LoginSuccess::new(server(), "alice", SecretString::new("app-pw"))
    }

    fn flow() -> LoginMachine {
        let mut machine = SessionMachine::new();
        machine
            .handle(SessionEvent::StartRequested)
            .expect("start is valid from idle");
        machine
            .handle(SessionEvent::FlowStarted(start()))
            .expect("flow start is valid while idle");
        machine
    }

    type LoginMachine = SessionMachine;

    #[test]
    fn start_from_idle_returns_start_login() {
        let mut machine = SessionMachine::new();
        let command = machine.handle(SessionEvent::StartRequested).unwrap();
        assert_eq!(command, Command::StartLogin);
        assert_eq!(machine.state().name(), "idle");
    }

    #[test]
    fn flow_start_opens_the_browser() {
        let machine = flow();
        assert!(matches!(machine.state(), SessionState::AwaitingLogin(_)));
    }

    #[test]
    fn first_poll_moves_to_awaiting_grant_and_repolls() {
        let mut machine = flow();
        let command = machine.handle(SessionEvent::PollPending).unwrap();
        assert!(matches!(machine.state(), SessionState::AwaitingGrant(_)));
        assert_eq!(
            command,
            Command::Poll(PollTarget::new(
                "https://cloud.example.org/login/v2/poll",
                "tok"
            ))
        );
    }

    #[test]
    fn grant_authenticates_and_establishes() {
        let mut machine = flow();
        machine.handle(SessionEvent::PollPending).unwrap();
        let command = machine.handle(SessionEvent::Granted(success())).unwrap();
        assert!(machine.is_authenticated());
        assert!(matches!(command, Command::Establish(_)));
        assert_eq!(machine.account().map(Account::login_name), Some("alice"));
    }

    #[test]
    fn rejected_credentials_require_reauthentication() {
        let mut machine = flow();
        machine.handle(SessionEvent::Granted(success())).unwrap();
        let command = machine.handle(SessionEvent::CredentialsRejected).unwrap();
        assert_eq!(command, Command::StopPolling);
        assert_eq!(machine.state().name(), "reauth_required");
        assert!(!machine.is_authenticated());
    }

    #[test]
    fn logout_returns_to_idle() {
        let mut machine = flow();
        machine.handle(SessionEvent::Granted(success())).unwrap();
        machine.handle(SessionEvent::LogoutRequested).unwrap();
        assert_eq!(machine.state().name(), "idle");
        assert!(machine.account().is_none());
    }

    #[test]
    fn reauth_can_start_a_new_flow() {
        let mut machine = flow();
        machine.handle(SessionEvent::Granted(success())).unwrap();
        machine.handle(SessionEvent::CredentialsRejected).unwrap();
        let command = machine.handle(SessionEvent::StartRequested).unwrap();
        assert_eq!(command, Command::StartLogin);
        assert_eq!(machine.state().name(), "idle");
    }

    #[test]
    fn stale_poll_after_logout_is_rejected() {
        let mut machine = flow();
        machine.handle(SessionEvent::LogoutRequested).unwrap();
        let error = machine
            .handle(SessionEvent::PollPending)
            .expect_err("stale poll must be rejected");
        assert_eq!(error.state(), "idle");
        assert_eq!(error.event(), "poll_pending");
    }

    #[test]
    fn a_rejected_event_leaves_the_state_untouched() {
        let mut machine = flow();
        let before = machine.state().clone();
        assert!(machine.handle(SessionEvent::StartRequested).is_err());
        assert_eq!(machine.state().name(), before.name());
    }

    #[test]
    fn expiry_ends_the_flow() {
        let mut machine = flow();
        let command = machine.handle(SessionEvent::Expired).unwrap();
        assert_eq!(command, Command::StopPolling);
        assert_eq!(machine.state().name(), "idle");
        assert_eq!(
            machine.handle(SessionEvent::StartRequested).unwrap(),
            Command::StartLogin
        );
    }

    #[test]
    fn a_failed_poll_ends_the_flow() {
        let mut machine = flow();
        let command = machine.handle(SessionEvent::PollFailed).unwrap();
        assert_eq!(command, Command::StopPolling);
        assert_eq!(machine.state().name(), "idle");
    }

    #[test]
    fn a_failed_poll_is_rejected_once_idle() {
        let mut machine = flow();
        machine.handle(SessionEvent::PollFailed).unwrap();
        assert!(machine.handle(SessionEvent::PollFailed).is_err());
    }
}
