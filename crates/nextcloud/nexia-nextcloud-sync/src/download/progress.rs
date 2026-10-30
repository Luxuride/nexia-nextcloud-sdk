//! Progress snapshot and speed metrics for a queued download.

use serde::{Deserialize, Serialize};

use super::task_id::DownloadTaskId;

/// Snapshot of byte transfer progress and speed metrics for a download task.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DownloadProgress {
    /// Task identifier associated with this progress snapshot.
    pub task_id: DownloadTaskId,
    /// Remote file path being transferred.
    pub remote_path: String,
    /// Bytes transferred so far.
    pub transferred_bytes: u64,
    /// Total expected transfer size in bytes, if known.
    pub total_bytes: Option<u64>,
    /// Instantaneous or smoothed transfer speed in bytes per second.
    pub bytes_per_second: Option<u64>,
}

impl DownloadProgress {
    /// Creates a new progress snapshot.
    pub fn new(
        task_id: DownloadTaskId,
        remote_path: impl Into<String>,
        transferred_bytes: u64,
        total_bytes: Option<u64>,
        bytes_per_second: Option<u64>,
    ) -> Self {
        Self {
            task_id,
            remote_path: remote_path.into(),
            transferred_bytes,
            total_bytes,
            bytes_per_second,
        }
    }

    /// Progress ratio between `0.0` and `1.0`, or `None` if total bytes are unknown.
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

    /// Whether all expected bytes have been received.
    pub const fn is_complete(&self) -> bool {
        match self.total_bytes {
            Some(total) => self.transferred_bytes >= total,
            None => false,
        }
    }

    /// Formats the transferred bytes in human-friendly binary units (`B`, `KB`, `MB`, `GB`).
    pub fn formatted_transferred(&self) -> String {
        format_binary_bytes(self.transferred_bytes)
    }

    /// Formats the expected total size, or `"Unknown size"` if not reported.
    pub fn formatted_total(&self) -> String {
        match self.total_bytes {
            Some(total) => format_binary_bytes(total),
            None => "Unknown size".to_string(),
        }
    }

    /// Formats the current transfer rate (e.g. `"2.4 MB/s"`), or `None` if unknown.
    pub fn formatted_speed(&self) -> Option<String> {
        self.bytes_per_second
            .map(|speed| format!("{}/s", format_binary_bytes(speed)))
    }

    /// Estimated time to completion in seconds based on current speed and remaining bytes.
    pub fn eta_seconds(&self) -> Option<u64> {
        let total = self.total_bytes?;
        let speed = self.bytes_per_second?;
        if speed == 0 || self.transferred_bytes >= total {
            return Some(0);
        }
        let remaining = total - self.transferred_bytes;
        Some(remaining / speed)
    }
}

/// Formats a raw byte count into human-readable binary units (`B`, `KB`, `MB`, `GB`).
pub fn format_binary_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_metrics_and_formatting() {
        let p = DownloadProgress::new(
            DownloadTaskId::new(1),
            "/remote/file.zip",
            50 * 1024 * 1024,
            Some(100 * 1024 * 1024),
            Some(10 * 1024 * 1024),
        );

        assert_eq!(p.fraction(), Some(0.5));
        assert_eq!(p.percentage(), Some(50.0));
        assert!(!p.is_complete());
        assert_eq!(p.formatted_transferred(), "50.0 MB");
        assert_eq!(p.formatted_total(), "100.0 MB");
        assert_eq!(p.formatted_speed(), Some("10.0 MB/s".to_string()));
        assert_eq!(p.eta_seconds(), Some(5));
    }

    #[test]
    fn zero_total_completes() {
        let p = DownloadProgress::new(
            DownloadTaskId::new(2),
            "/remote/empty.txt",
            0,
            Some(0),
            None,
        );
        assert_eq!(p.fraction(), Some(1.0));
        assert!(p.is_complete());
    }
}
