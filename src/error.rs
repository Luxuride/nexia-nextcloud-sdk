//! Error types returned by the client and builder.

use std::fmt;

use nextcloud_sdk_core::Error;
use nextcloud_sdk_session::SessionError;
use nextcloud_sdk_storage::StorageError;
use nextcloud_transport::TransportBuildError;

/// Errors that can occur when configuring or operating the client.
#[derive(Debug)]
pub enum ClientError {
    /// An underlying storage error occurred.
    Storage(StorageError),
    /// A state transition was rejected by the session state machine.
    Session(SessionError),
    /// An error occurred in the core HTTP or transport layer.
    Core(Error),
    /// A required builder configuration was missing.
    MissingConfiguration(&'static str),
    /// The background worker actor has terminated.
    WorkerGone,
    /// An operation required an authenticated session, but the client is not authenticated.
    NotAuthenticated,
    /// JSON serialization or deserialization failed.
    Json(serde_json::Error),
    /// Transport initialization failed.
    Transport(TransportBuildError),
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(err) => write!(f, "storage failure: {err}"),
            Self::Session(err) => write!(f, "session state error: {err}"),
            Self::Core(err) => write!(f, "transport error: {err}"),
            Self::MissingConfiguration(field) => {
                write!(f, "missing required client configuration: {field}")
            }
            Self::WorkerGone => write!(f, "background worker actor is unavailable"),
            Self::NotAuthenticated => write!(f, "client is not authenticated"),
            Self::Json(err) => write!(f, "json serialization error: {err}"),
            Self::Transport(err) => write!(f, "transport build error: {err}"),
        }
    }
}

impl std::error::Error for ClientError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Storage(err) => Some(err),
            Self::Session(err) => Some(err),
            Self::Core(err) => Some(err),
            Self::MissingConfiguration(_) | Self::WorkerGone | Self::NotAuthenticated => None,
            Self::Json(err) => Some(err),
            Self::Transport(err) => Some(err),
        }
    }
}

impl From<StorageError> for ClientError {
    fn from(err: StorageError) -> Self {
        Self::Storage(err)
    }
}

impl From<SessionError> for ClientError {
    fn from(err: SessionError) -> Self {
        Self::Session(err)
    }
}

impl From<Error> for ClientError {
    fn from(err: Error) -> Self {
        Self::Core(err)
    }
}

impl From<serde_json::Error> for ClientError {
    fn from(err: serde_json::Error) -> Self {
        Self::Json(err)
    }
}

impl From<TransportBuildError> for ClientError {
    fn from(err: TransportBuildError) -> Self {
        Self::Transport(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_and_source_are_implemented() {
        let err = ClientError::MissingConfiguration("server_url");
        assert_eq!(
            err.to_string(),
            "missing required client configuration: server_url"
        );
        assert!(std::error::Error::source(&err).is_none());

        let json_err: serde_json::Error =
            serde_json::from_str::<String>("not valid json").unwrap_err();
        let client_err = ClientError::from(json_err);
        assert!(std::error::Error::source(&client_err).is_some());
    }
}
