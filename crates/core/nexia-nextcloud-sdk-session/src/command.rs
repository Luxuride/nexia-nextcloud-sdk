use super::state::Account;

/// Work the application has to perform on behalf of the machine.
///
/// The machine never touches the network: it returns a command and waits for
/// the matching event. Retry timing and HTTP stay with the caller, which keeps
/// the transition table pure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// `POST /index.php/login/v2` to start a flow.
    StartLogin,
    /// Open this URL in the user's browser.
    OpenBrowser(String),
    /// `POST` the target's token to the target's endpoint.
    Poll(PollTarget),
    /// The flow is over; stop polling.
    StopPolling,
    /// Build a client for this account and use it.
    Establish(Box<Account>),
    /// Nothing to do until the next event arrives.
    Idle,
}

/// Endpoint and token of one poll attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PollTarget {
    endpoint: String,
    token: String,
}

impl PollTarget {
    /// Poll `endpoint` with `token`.
    pub fn new(endpoint: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            token: token.into(),
        }
    }

    /// Endpoint to post to.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Token to post.
    pub fn token(&self) -> &str {
        &self.token
    }
}

#[cfg(test)]
mod tests {
    use nexia_nextcloud_auth::SecretString;
    use nexia_nextcloud_core::ServerUrl;

    use super::{Account, Command, PollTarget};

    #[test]
    fn poll_target_exposes_what_it_was_built_with() {
        let target = PollTarget::new("https://example.org/login/v2/poll", "tok");
        assert_eq!(target.endpoint(), "https://example.org/login/v2/poll");
        assert_eq!(target.token(), "tok");
    }

    #[test]
    fn establish_debug_does_not_leak_the_password() {
        let account = Account::new(
            ServerUrl::parse("https://cloud.example.org").expect("valid url"),
            "alice",
            "alice",
            SecretString::new("super-secret"),
        );
        let rendered = format!("{:?}", Command::Establish(Box::new(account)));
        assert!(!rendered.contains("super-secret"));
    }
}
