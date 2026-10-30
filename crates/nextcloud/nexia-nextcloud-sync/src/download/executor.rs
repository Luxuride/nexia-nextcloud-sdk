//! Download executor abstraction for driving queued transfers.

use std::future::Future;
use std::path::Path;

use crate::error::SyncError;

/// Trait implemented by any component capable of executing a remote file download.
pub trait DownloadExecutor: Send + Sync + 'static {
    /// Executes the download of `remote_path` and writes it to `local_destination`.
    ///
    /// The implementation must report progress updates (transferred bytes, total bytes)
    /// through the `progress_cb` callback.
    fn execute_download(
        &self,
        remote_path: &str,
        local_destination: &Path,
        progress_cb: Box<dyn Fn(u64, Option<u64>) + Send + Sync>,
    ) -> impl Future<Output = Result<u64, SyncError>> + Send;
}

#[cfg(test)]
pub(crate) struct MockDownloadExecutor {
    pub(crate) total_bytes: u64,
}

#[cfg(test)]
impl DownloadExecutor for MockDownloadExecutor {
    async fn execute_download(
        &self,
        _remote_path: &str,
        _local_destination: &Path,
        progress_cb: Box<dyn Fn(u64, Option<u64>) + Send + Sync>,
    ) -> Result<u64, SyncError> {
        let half = self.total_bytes / 2;
        progress_cb(half, Some(self.total_bytes));
        progress_cb(self.total_bytes, Some(self.total_bytes));
        Ok(self.total_bytes)
    }
}
