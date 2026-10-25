//! Priority scheduling levels for queued downloads.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Priority tier determining queue dispatch ordering.
///
/// Higher priority items are scheduled before lower priority ones.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub enum DownloadPriority {
    /// Background transfers (e.g. bulk pre-fetching).
    Low = 0,
    /// Standard scheduled synchronization transfers.
    #[default]
    Normal = 10,
    /// Priority files (e.g. active workspace folder).
    High = 20,
    /// Explicit on-demand user click or immediate file open request.
    Urgent = 30,
}

impl DownloadPriority {
    /// Returns `true` if this task is marked with [`DownloadPriority::Urgent`].
    pub const fn is_urgent(self) -> bool {
        matches!(self, Self::Urgent)
    }

    /// User-facing display label for the priority tier.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Normal => "Normal",
            Self::High => "High",
            Self::Urgent => "Urgent",
        }
    }
}

impl fmt::Display for DownloadPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordering_and_defaults() {
        assert_eq!(DownloadPriority::default(), DownloadPriority::Normal);
        assert!(DownloadPriority::Urgent > DownloadPriority::High);
        assert!(DownloadPriority::High > DownloadPriority::Normal);
        assert!(DownloadPriority::Normal > DownloadPriority::Low);
        assert!(DownloadPriority::Urgent.is_urgent());
        assert!(!DownloadPriority::Normal.is_urgent());
        assert_eq!(DownloadPriority::High.to_string(), "High");
    }
}
