use serde::{Deserialize, Serialize};

use super::ServerStatus;
use crate::wire::lossy_opt_u64;

/// Core capabilities exposed by the server.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct CoreCapabilities {
    /// Recommended poll interval for client sync in seconds.
    #[serde(
        default,
        alias = "pollinterval",
        alias = "poll_interval",
        deserialize_with = "lossy_opt_u64"
    )]
    pub poll_interval: Option<u64>,
    /// Relative or absolute path to the WebDAV endpoint root (e.g. `"remote.php/webdav"`).
    #[serde(default, alias = "webdav-root", alias = "webdav_root")]
    pub webdav_root: Option<String>,
    /// Server installation and runtime status.
    #[serde(default)]
    pub status: Option<ServerStatus>,
}

impl CoreCapabilities {
    /// Recommended background polling interval in seconds.
    pub const fn poll_interval(&self) -> Option<u64> {
        self.poll_interval
    }

    /// Root WebDAV path advertised by the instance.
    pub fn webdav_root(&self) -> Option<&str> {
        self.webdav_root.as_deref()
    }

    /// Server operational status.
    pub fn status(&self) -> Option<&ServerStatus> {
        self.status.as_ref()
    }
}
