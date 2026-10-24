use std::fmt;

use nextcloud_auth::SecretString;
use nextcloud_sdk_core::ServerUrl;

/// Credentials the login flow produced, plus the identity behind them.
///
/// The login name is what the user typed into the browser and is what
/// credentials are sent with; the WebDAV user id can differ from it (an email
/// address logs in but does not name a file tree), which is why both are kept.
#[derive(Clone, PartialEq, Eq)]
pub struct Account {
    server: ServerUrl,
    login_name: String,
    user_id: String,
    password: SecretString,
}

impl Account {
    /// Builds an account record.
    pub fn new(
        server: ServerUrl,
        login_name: impl Into<String>,
        user_id: impl Into<String>,
        password: SecretString,
    ) -> Self {
        Self {
            server,
            login_name: login_name.into(),
            user_id: user_id.into(),
            password,
        }
    }

    /// Instance this account belongs to.
    pub const fn server(&self) -> &ServerUrl {
        &self.server
    }

    /// Login name the credentials were issued for.
    pub fn login_name(&self) -> &str {
        &self.login_name
    }

    /// User id that names the WebDAV tree and the OCS user record.
    pub fn user_id(&self) -> &str {
        &self.user_id
    }

    /// The app password.
    ///
    /// Reading the plaintext is explicit so no log line or `Debug` output can
    /// pick it up by accident.
    pub const fn password(&self) -> &SecretString {
        &self.password
    }
}

impl fmt::Debug for Account {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Account")
            .field("server", &self.server)
            .field("login_name", &self.login_name)
            .field("user_id", &self.user_id)
            .field("password", &self.password)
            .finish()
    }
}

/// Opaque token identifying one pending login.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PollToken(String);

impl PollToken {
    /// Wraps a token received from the server.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The token as it must be sent back.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PollToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A login flow that has been started but not finished.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoginFlow {
    poll: PollToken,
    login_url: String,
    poll_endpoint: String,
}

impl LoginFlow {
    /// Records what the server returned when the flow was started.
    pub fn new(
        poll: PollToken,
        login_url: impl Into<String>,
        poll_endpoint: impl Into<String>,
    ) -> Self {
        Self {
            poll,
            login_url: login_url.into(),
            poll_endpoint: poll_endpoint.into(),
        }
    }

    /// Token to poll with.
    pub const fn poll_token(&self) -> &PollToken {
        &self.poll
    }

    /// URL the application must open in the user's browser.
    pub fn login_url(&self) -> &str {
        &self.login_url
    }

    /// Endpoint the token is polled against.
    pub fn poll_endpoint(&self) -> &str {
        &self.poll_endpoint
    }
}

/// Where the session currently stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionState {
    /// No session; nothing has been attempted.
    Idle,
    /// The login flow was started and the user has not acted yet.
    AwaitingLogin(LoginFlow),
    /// The user authenticated in the browser and has not granted access yet.
    AwaitingGrant(LoginFlow),
    /// An app password is held and requests may be sent.
    Authenticated(Account),
    /// The server rejected the stored credentials; a new login is required.
    ReauthRequired(Account),
}

impl SessionState {
    /// Whether requests may be sent in this state.
    pub const fn is_authenticated(&self) -> bool {
        matches!(self, Self::Authenticated(_))
    }

    /// The account involved, when the state has reached one.
    pub const fn account(&self) -> Option<&Account> {
        match self {
            Self::Authenticated(account) | Self::ReauthRequired(account) => Some(account),
            Self::Idle | Self::AwaitingLogin(_) | Self::AwaitingGrant(_) => None,
        }
    }

    /// The pending login flow, while one is in progress.
    pub const fn login_flow(&self) -> Option<&LoginFlow> {
        match self {
            Self::AwaitingLogin(flow) | Self::AwaitingGrant(flow) => Some(flow),
            Self::Idle | Self::Authenticated(_) | Self::ReauthRequired(_) => None,
        }
    }

    /// Short name of the state, for logs and UI labels.
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::AwaitingLogin(_) => "awaiting_login",
            Self::AwaitingGrant(_) => "awaiting_grant",
            Self::Authenticated(_) => "authenticated",
            Self::ReauthRequired(_) => "reauth_required",
        }
    }
}

#[cfg(test)]
mod tests {
    use nextcloud_auth::SecretString;
    use nextcloud_sdk_core::ServerUrl;

    use super::{Account, LoginFlow, PollToken, SessionState};

    fn server() -> ServerUrl {
        ServerUrl::parse("https://cloud.example.org").expect("valid url")
    }

    fn account() -> Account {
        Account::new(server(), "alice", "alice", SecretString::new("app-pw"))
    }

    #[test]
    fn account_debug_redacts_the_password() {
        let rendered = format!("{:?}", account());
        assert!(!rendered.contains("app-pw"));
        assert!(rendered.contains("alice"));
    }

    #[test]
    fn only_authenticated_allows_requests() {
        let flow = LoginFlow::new(
            PollToken::new("token"),
            "https://cloud.example.org/login/v2/flow/x",
            "https://cloud.example.org/login/v2/poll",
        );
        assert!(!SessionState::Idle.is_authenticated());
        assert!(!SessionState::AwaitingLogin(flow.clone()).is_authenticated());
        assert!(!SessionState::AwaitingGrant(flow).is_authenticated());
        assert!(SessionState::Authenticated(account()).is_authenticated());
        assert!(!SessionState::ReauthRequired(account()).is_authenticated());
    }

    #[test]
    fn pending_flow_is_reachable_only_while_pending() {
        let flow = LoginFlow::new(
            PollToken::new("token"),
            "https://cloud.example.org/login/v2/flow/x",
            "https://cloud.example.org/login/v2/poll",
        );
        assert_eq!(
            SessionState::AwaitingGrant(flow.clone())
                .login_flow()
                .map(LoginFlow::poll_endpoint),
            Some("https://cloud.example.org/login/v2/poll")
        );
        assert!(
            SessionState::Authenticated(account())
                .login_flow()
                .is_none()
        );
    }
}
