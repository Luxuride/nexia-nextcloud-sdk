//! Strongly typed identifier for Nextcloud mount points.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique identifier for an orchestrator mount point.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MountId(String);

impl MountId {
    /// Creates a new random mount identifier.
    #[must_use]
    pub fn new() -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let random: u32 = (timestamp ^ (timestamp >> 16)) as u32;
        Self(format!("mount-{timestamp:x}-{random:x}"))
    }

    /// Creates a mount identifier from an existing string.
    #[must_use]
    pub fn from_string(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns a string slice reference to the underlying identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for MountId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for MountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for MountId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for MountId {
    fn from(s: String) -> Self {
        Self(s)
    }
}
