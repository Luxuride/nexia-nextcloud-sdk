use nexia_nextcloud_auth::SecretString;
use nexia_nextcloud_core::ServerUrl;

use crate::state::PollToken;

/// What the server returned from `POST /index.php/login/v2`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoginStart {
    login_url: String,
    poll_endpoint: String,
    token: PollToken,
}

impl LoginStart {
    /// Records the three values the start response carries.
    pub fn new(
        login_url: impl Into<String>,
        poll_endpoint: impl Into<String>,
        token: PollToken,
    ) -> Self {
        Self {
            login_url: login_url.into(),
            poll_endpoint: poll_endpoint.into(),
            token,
        }
    }

    /// URL to open in the user's browser.
    pub fn login_url(&self) -> &str {
        &self.login_url
    }

    /// Endpoint the token must be polled against.
    pub fn poll_endpoint(&self) -> &str {
        &self.poll_endpoint
    }

    /// Token identifying this flow.
    pub const fn token(&self) -> &PollToken {
        &self.token
    }
}

/// What the server returned once the user granted access.
#[derive(Clone, PartialEq, Eq)]
pub struct LoginSuccess {
    server: ServerUrl,
    login_name: String,
    app_password: SecretString,
}

impl LoginSuccess {
    /// Builds a success record from the poll answer.
    pub fn new(
        server: ServerUrl,
        login_name: impl Into<String>,
        app_password: SecretString,
    ) -> Self {
        Self {
            server,
            login_name: login_name.into(),
            app_password,
        }
    }

    /// Instance the credentials belong to, as the server reported it.
    pub const fn server(&self) -> &ServerUrl {
        &self.server
    }

    /// Login name the credentials were issued for.
    pub fn login_name(&self) -> &str {
        &self.login_name
    }

    /// The app password.
    pub const fn app_password(&self) -> &SecretString {
        &self.app_password
    }
}

impl std::fmt::Debug for LoginSuccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoginSuccess")
            .field("server", &self.server)
            .field("login_name", &self.login_name)
            .field("app_password", &self.app_password)
            .finish()
    }
}

/// Everything that can move the session machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionEvent {
    /// The user asked to log in.
    StartRequested,
    /// The start call answered with a login URL and a poll token.
    FlowStarted(LoginStart),
    /// The start call failed; the flow never began.
    FlowStartFailed,
    /// A poll answered `404`, so the user has not granted access yet.
    PollPending,
    /// A poll answered `200` with credentials.
    Granted(LoginSuccess),
    /// A poll failed for a reason other than "not yet".
    PollFailed,
    /// The poll token expired before the user finished (20 minutes).
    Expired,
    /// The server rejected the stored credentials.
    CredentialsRejected,
    /// The user asked to forget the account.
    LogoutRequested,
}

impl SessionEvent {
    /// Stable name used in diagnostics and [`SessionError`](crate::SessionError).
    pub const fn name(&self) -> &'static str {
        match self {
            Self::StartRequested => "start_requested",
            Self::FlowStarted(_) => "flow_started",
            Self::FlowStartFailed => "flow_start_failed",
            Self::PollPending => "poll_pending",
            Self::Granted(_) => "granted",
            Self::PollFailed => "poll_failed",
            Self::Expired => "expired",
            Self::CredentialsRejected => "credentials_rejected",
            Self::LogoutRequested => "logout_requested",
        }
    }
}

#[cfg(test)]
mod tests {
    use nexia_nextcloud_auth::SecretString;
    use nexia_nextcloud_core::ServerUrl;

    use super::{LoginStart, LoginSuccess, SessionEvent};
    use crate::state::PollToken;

    #[test]
    fn login_success_debug_redacts_the_password() {
        let success = LoginSuccess::new(
            ServerUrl::parse("https://cloud.example.org").expect("valid url"),
            "alice",
            SecretString::new("app-password-value"),
        );
        let rendered = format!("{success:?}");
        assert!(!rendered.contains("app-password-value"));
        assert!(rendered.contains("alice"));
    }

    #[test]
    fn login_start_exposes_the_poll_target() {
        let start = LoginStart::new(
            "https://example.org/login/v2/flow/x",
            "https://example.org/login/v2/poll",
            PollToken::new("tok"),
        );
        assert_eq!(start.poll_endpoint(), "https://example.org/login/v2/poll");
        assert_eq!(start.token().expose(), "tok");
    }

    #[test]
    fn every_event_has_a_stable_name() {
        assert_eq!(SessionEvent::PollPending.name(), "poll_pending");
        assert_eq!(SessionEvent::LogoutRequested.name(), "logout_requested");
    }
}
