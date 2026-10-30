use serde::{Deserialize, Serialize};

use crate::wire::{lossy_f64, lossy_i64};

/// User storage quota metrics and capacity constraints.
///
/// In Nextcloud, when a quota is set to unlimited, negative integer codes
/// (typically `-3` or `-2`) are returned for `free` and `total`.
///
/// Official documentation:
/// <https://docs.nextcloud.com/server/latest/developer_manual/client_apis/OCS/ocs-api-overview.html#user-metadata>
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct Quota {
    /// Space remaining in bytes, or negative code when unlimited.
    #[serde(default, deserialize_with = "lossy_i64")]
    pub free: i64,
    /// Space consumed by user files in bytes.
    #[serde(default, deserialize_with = "lossy_i64")]
    pub used: i64,
    /// Total storage capacity allocated in bytes, or negative code when unlimited.
    #[serde(default, deserialize_with = "lossy_i64")]
    pub total: i64,
    /// Storage usage ratio or percentage (e.g. `0.96` or `96.0`).
    #[serde(default, deserialize_with = "lossy_f64")]
    pub relative: f64,
}

impl Quota {
    /// Creates a quota descriptor with explicit byte measurements.
    pub const fn new(free: i64, used: i64, total: i64, relative: f64) -> Self {
        Self {
            free,
            used,
            total,
            relative,
        }
    }

    /// Available free space in bytes, or a negative status code when unlimited.
    pub const fn free(&self) -> i64 {
        self.free
    }

    /// Bytes consumed by user files.
    pub const fn used(&self) -> i64 {
        self.used
    }

    /// Total storage allocation in bytes, or a negative status code when unlimited.
    pub const fn total(&self) -> i64 {
        self.total
    }

    /// Relative storage utilization metric.
    pub const fn relative(&self) -> f64 {
        self.relative
    }

    /// Whether the user has an unlimited storage allocation.
    pub const fn is_unlimited(&self) -> bool {
        self.total < 0
    }

    /// Available free storage in bytes, or `None` if quota is unlimited.
    pub fn free_bytes(&self) -> Option<u64> {
        if self.free >= 0 {
            Some(self.free as u64)
        } else {
            None
        }
    }

    /// Total allocated storage capacity in bytes, or `None` if quota is unlimited.
    pub fn total_bytes(&self) -> Option<u64> {
        if self.total >= 0 {
            Some(self.total as u64)
        } else {
            None
        }
    }

    /// Bytes currently consumed by user files.
    pub fn used_bytes(&self) -> u64 {
        self.used.max(0) as u64
    }

    /// Percentage of storage utilized (0.0 to 100.0).
    pub fn relative_percentage(&self) -> f64 {
        if self.relative > 0.0 && self.relative <= 1.0 {
            self.relative * 100.0
        } else {
            self.relative
        }
    }
}
