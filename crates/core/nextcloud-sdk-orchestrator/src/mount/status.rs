//! Status types and icon mappings for mounts and synchronized files.

use serde::{Deserialize, Serialize};

/// High-level operational status of an orchestrator mount point.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MountStatus {
    /// Mount is idle and up-to-date.
    #[default]
    Idle,

    /// Mount is actively performing a synchronization or hydration cycle.
    Syncing,

    /// Mount synchronization is paused.
    Paused,

    /// Mount encountered an error during operation.
    Error(String),

    /// Mount has been unmounted or disabled.
    Unmounted,
}

/// Dynamic runtime state and statistics for an active mount point.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MountState {
    /// Current operational status.
    pub status: MountStatus,

    /// UNIX timestamp in seconds of the last successful synchronization cycle.
    pub last_sync_timestamp: Option<u64>,

    /// Total number of items tracked by this mount.
    pub total_items: u64,

    /// Number of items currently synchronized and available offline.
    pub items_synced: u64,

    /// Optional error message if the mount is in an error state.
    pub error_message: Option<String>,
}

/// Synchronization status of an individual file or directory within a mount.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SyncFileStatus {
    /// File content is present locally and fully synchronized with the Nextcloud server.
    Synced,

    /// File exists on the server as metadata only (0-byte placeholder locally).
    OnlineOnly,

    /// File is currently being uploaded or downloaded.
    Syncing,

    /// File is currently hydrating or downloading from Nextcloud.
    SyncingDown,

    /// File is currently uploading local changes to Nextcloud.
    SyncingUp,

    /// A conflict between local and remote versions was detected.
    Conflict,

    /// File is ignored by synchronization rules (e.g., hidden or temporary file).
    Ignored,

    /// A transfer or verification error occurred for this item.
    Error(String),
}

impl SyncFileStatus {
    /// Returns the standard FreeDesktop / GNOME / Adwaita icon name corresponding to this sync status.
    #[must_use]
    pub fn icon_name(&self) -> &'static str {
        match self {
            Self::Synced => "emblem-ok-symbolic",
            Self::OnlineOnly => "cloud-symbolic",
            Self::Syncing | Self::SyncingDown | Self::SyncingUp => "emblem-synchronizing-symbolic",
            Self::Conflict => "dialog-warning-symbolic",
            Self::Ignored => "action-unavailable-symbolic",
            Self::Error(_) => "dialog-error-symbolic",
        }
    }

    /// Returns a secondary emblem icon name suitable for overlays in file managers or grid views.
    #[must_use]
    pub fn emblem_name(&self) -> &'static str {
        match self {
            Self::Synced => "emblem-default-symbolic",
            Self::OnlineOnly => "folder-remote-symbolic",
            Self::Syncing => "view-refresh-symbolic",
            Self::SyncingDown => "go-down-symbolic",
            Self::SyncingUp => "go-up-symbolic",
            Self::Conflict => "emblem-important-symbolic",
            Self::Ignored => "emblem-readonly-symbolic",
            Self::Error(_) => "error-symbolic",
        }
    }

    /// Returns a human-friendly description of the synchronization state.
    #[must_use]
    pub fn description(&self) -> &'static str {
        match self {
            Self::Synced => "Available offline (fully synchronized)",
            Self::OnlineOnly => "Available online only (metadata placeholder)",
            Self::Syncing => "Synchronizing with server…",
            Self::SyncingDown => "Downloading from server…",
            Self::SyncingUp => "Uploading to server…",
            Self::Conflict => "Conflicting modifications detected",
            Self::Ignored => "Ignored by sync rules",
            Self::Error(_) => "Synchronization error",
        }
    }

    /// Returns `true` if the file content is readily accessible on the local disk without network.
    #[must_use]
    pub fn is_locally_available(&self) -> bool {
        matches!(self, Self::Synced)
    }

    /// Returns `true` if this item requires server communication to read its contents.
    #[must_use]
    pub fn is_online_only(&self) -> bool {
        matches!(self, Self::OnlineOnly)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_file_status_icons_and_availability() {
        let synced = SyncFileStatus::Synced;
        assert_eq!(synced.icon_name(), "emblem-ok-symbolic");
        assert_eq!(synced.emblem_name(), "emblem-default-symbolic");
        assert!(synced.is_locally_available());
        assert!(!synced.is_online_only());

        let online = SyncFileStatus::OnlineOnly;
        assert_eq!(online.icon_name(), "cloud-symbolic");
        assert_eq!(online.emblem_name(), "folder-remote-symbolic");
        assert!(!online.is_locally_available());
        assert!(online.is_online_only());

        let syncing = SyncFileStatus::Syncing;
        assert_eq!(syncing.icon_name(), "emblem-synchronizing-symbolic");
        assert_eq!(syncing.emblem_name(), "view-refresh-symbolic");

        let conflict = SyncFileStatus::Conflict;
        assert_eq!(conflict.icon_name(), "dialog-warning-symbolic");
        assert_eq!(conflict.emblem_name(), "emblem-important-symbolic");

        let ignored = SyncFileStatus::Ignored;
        assert_eq!(ignored.icon_name(), "action-unavailable-symbolic");

        let err = SyncFileStatus::Error("network failure".to_string());
        assert_eq!(err.icon_name(), "dialog-error-symbolic");
    }

    #[test]
    fn test_mount_status_default() {
        assert_eq!(MountStatus::default(), MountStatus::Idle);
    }
}
