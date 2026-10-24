use std::sync::Arc;

/// Snapshot of transfer progress for upload or download operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransferProgress {
    transferred_bytes: u64,
    total_bytes: Option<u64>,
}

impl TransferProgress {
    /// Creates a new progress snapshot.
    pub const fn new(transferred_bytes: u64, total_bytes: Option<u64>) -> Self {
        Self {
            transferred_bytes,
            total_bytes,
        }
    }

    /// Total bytes transferred up to this snapshot.
    pub const fn transferred_bytes(&self) -> u64 {
        self.transferred_bytes
    }

    /// Total expected transfer size in bytes, if known from `Content-Length` or metadata.
    pub const fn total_bytes(&self) -> Option<u64> {
        self.total_bytes
    }

    /// Progress ratio between `0.0` and `1.0`, or `None` if the total transfer size is unknown.
    pub fn fraction(&self) -> Option<f64> {
        self.total_bytes.map(|total| {
            if total == 0 {
                1.0
            } else {
                (self.transferred_bytes as f64 / total as f64).clamp(0.0, 1.0)
            }
        })
    }

    /// Progress expressed as a percentage between `0.0` and `100.0`, or `None` if unknown.
    pub fn percentage(&self) -> Option<f64> {
        self.fraction().map(|f| f * 100.0)
    }

    /// Whether all expected bytes have been transferred. Always `false` if total size is unknown.
    pub const fn is_complete(&self) -> bool {
        match self.total_bytes {
            Some(total) => self.transferred_bytes >= total,
            None => false,
        }
    }
}

/// Thread-safe callback invoked with incremental [`TransferProgress`] updates.
pub type ProgressCallback = Arc<dyn Fn(TransferProgress) + Send + Sync>;
