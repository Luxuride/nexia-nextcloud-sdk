use std::fmt;

use sdk_session::{LoginStart, SessionState};
use tokio::sync::oneshot;

use crate::error::ClientError;

/// Result of querying cached server capabilities (`(user_id, body)`).
pub type CachedCapabilities = Option<(String, Vec<u8>)>;

/// Tasks dispatched from [`Client`](crate::Client) to the background [`WorkerActor`](crate::WorkerActor).
pub enum WorkerTask {
    /// Initiate Nextcloud Login Flow v2.
    StartLogin {
        /// Channel to return the flow start data or failure.
        reply: oneshot::Sender<Result<LoginStart, ClientError>>,
    },
    /// Poll the pending Login Flow v2 target.
    PollLogin {
        /// Channel to return the updated session state or failure.
        reply: oneshot::Sender<Result<SessionState, ClientError>>,
    },
    /// Log out, invalidating in-memory state and wiping stored credentials/cache.
    Logout {
        /// Channel to return completion status.
        reply: oneshot::Sender<Result<(), ClientError>>,
    },
    /// Store raw capabilities JSON in cache.
    StoreCapabilities {
        /// User ID the capabilities were retrieved for.
        user_id: String,
        /// Serialized JSON payload.
        body: Vec<u8>,
        /// Channel to return completion status.
        reply: oneshot::Sender<Result<(), ClientError>>,
    },
    /// Load cached capabilities JSON.
    LoadCapabilities {
        /// Channel to return optional (user_id, body) payload.
        reply: oneshot::Sender<Result<CachedCapabilities, ClientError>>,
    },
    /// Clear all cached data.
    ClearCache {
        /// Channel to return completion status.
        reply: oneshot::Sender<Result<(), ClientError>>,
    },
    /// Query current session state directly.
    GetSessionState {
        /// Channel to return the current state.
        reply: oneshot::Sender<SessionState>,
    },
}

impl fmt::Debug for WorkerTask {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StartLogin { .. } => f.write_str("WorkerTask::StartLogin"),
            Self::PollLogin { .. } => f.write_str("WorkerTask::PollLogin"),
            Self::Logout { .. } => f.write_str("WorkerTask::Logout"),
            Self::StoreCapabilities { user_id, .. } => f
                .debug_struct("WorkerTask::StoreCapabilities")
                .field("user_id", user_id)
                .finish(),
            Self::LoadCapabilities { .. } => f.write_str("WorkerTask::LoadCapabilities"),
            Self::ClearCache { .. } => f.write_str("WorkerTask::ClearCache"),
            Self::GetSessionState { .. } => f.write_str("WorkerTask::GetSessionState"),
        }
    }
}
