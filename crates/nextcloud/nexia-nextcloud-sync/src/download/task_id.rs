//! Unique task identifier for queued sync download items.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Strongly-typed identifier for a queued file download operation.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct DownloadTaskId(u64);

impl DownloadTaskId {
    /// Constructs a download task identifier from a numeric value.
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// Returns the underlying numeric identifier value.
    pub const fn raw(self) -> u64 {
        self.0
    }
}

impl From<u64> for DownloadTaskId {
    fn from(val: u64) -> Self {
        Self::new(val)
    }
}

impl fmt::Display for DownloadTaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "task-{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_and_equality() {
        let id1 = DownloadTaskId::new(42);
        let id2 = DownloadTaskId::from(42);
        assert_eq!(id1, id2);
        assert_eq!(format!("{id1}"), "task-42");
        assert_eq!(id1.raw(), 42);
    }
}
