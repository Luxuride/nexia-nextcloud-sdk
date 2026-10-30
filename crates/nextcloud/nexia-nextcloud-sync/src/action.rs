//! Synchronization action items produced by the reconciler.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Action determined by the reconciler to synchronize an individual file or directory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncAction {
    /// Remote file is new or modified; download to local filesystem.
    Download {
        /// Canonical relative path within the sync root (e.g. `"/Documents/report.pdf"`).
        relative_path: String,
        /// Expected file size in bytes.
        size: u64,
        /// Remote ETag identifier.
        etag: Option<String>,
    },
    /// Local file is new or modified; upload to Nextcloud WebDAV.
    Upload {
        /// Canonical relative path within the sync root.
        relative_path: String,
        /// Full local filesystem path on disk.
        local_path: PathBuf,
        /// Local file size in bytes.
        size: u64,
    },
    /// Remote directory needs to be created on local disk.
    CreateLocalDirectory {
        /// Canonical relative path.
        relative_path: String,
    },
    /// Local directory needs to be created on remote WebDAV.
    CreateRemoteDirectory {
        /// Canonical relative path.
        relative_path: String,
    },
    /// Item was deleted on Nextcloud; delete from local disk.
    DeleteLocal {
        /// Canonical relative path.
        relative_path: String,
        /// Full local filesystem path on disk.
        local_path: PathBuf,
        /// Whether entry is a directory.
        is_directory: bool,
    },
    /// Item was deleted on local disk; delete from remote Nextcloud.
    DeleteRemote {
        /// Canonical relative path.
        relative_path: String,
        /// Whether entry is a directory.
        is_directory: bool,
    },
    /// Both sides were modified concurrently; flagged as a conflict.
    Conflict {
        /// Canonical relative path.
        relative_path: String,
        /// Description of the conflict reason.
        reason: String,
    },
    /// Item is unchanged and up to date on both ends.
    InSync {
        /// Canonical relative path.
        relative_path: String,
    },
}

impl SyncAction {
    /// Returns the canonical relative path for this action.
    pub fn relative_path(&self) -> &str {
        match self {
            Self::Download { relative_path, .. }
            | Self::Upload { relative_path, .. }
            | Self::CreateLocalDirectory { relative_path }
            | Self::CreateRemoteDirectory { relative_path }
            | Self::DeleteLocal { relative_path, .. }
            | Self::DeleteRemote { relative_path, .. }
            | Self::Conflict { relative_path, .. }
            | Self::InSync { relative_path } => relative_path,
        }
    }
}
