//! Mount descriptor defining the configuration of a Nextcloud mount point.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::mount::{MountId, MountMode};

/// Specification of a configured mount point connecting a Nextcloud remote hierarchy to a local directory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MountDescriptor {
    /// Unique identifier for this mount configuration.
    pub id: MountId,

    /// Identifier of the associated Nextcloud account.
    pub account_id: String,

    /// Remote Nextcloud directory path (e.g., `"/"` for root, or `"/Photos"`, `"/Documents"` for subdirectories).
    pub remote_path: String,

    /// Destination directory on the local filesystem where the mount is attached.
    pub local_target: PathBuf,

    /// Mount synchronization mode.
    pub mode: MountMode,

    /// Optional user-visible label for this mount.
    pub display_name: Option<String>,

    /// Whether this mount point is active and participating in automatic synchronization.
    pub enabled: bool,
}

impl MountDescriptor {
    /// Creates a new mount descriptor connecting a remote path to a local target directory.
    #[must_use]
    pub fn new(
        account_id: impl Into<String>,
        remote_path: impl Into<String>,
        local_target: impl Into<PathBuf>,
        mode: MountMode,
    ) -> Self {
        let remote_str = remote_path.into();
        let cleaned_remote = if remote_str.is_empty() || !remote_str.starts_with('/') {
            format!("/{remote_str}")
        } else {
            remote_str
        };

        Self {
            id: MountId::new(),
            account_id: account_id.into(),
            remote_path: cleaned_remote,
            local_target: local_target.into(),
            mode,
            display_name: None,
            enabled: true,
        }
    }

    /// Assigns a custom display name to this mount descriptor.
    #[must_use]
    pub fn with_display_name(mut self, name: impl Into<String>) -> Self {
        self.display_name = Some(name.into());
        self
    }

    /// Sets the custom mount identifier.
    #[must_use]
    pub fn with_id(mut self, id: MountId) -> Self {
        self.id = id;
        self
    }

    /// Returns `true` if this mount attaches to the Nextcloud root directory (`"/"`).
    #[must_use]
    pub fn is_root(&self) -> bool {
        self.remote_path == "/"
    }

    /// Resolves an absolute local path inside this mount's local target to a relative path within the mount.
    #[must_use]
    pub fn relative_path(&self, absolute_path: &Path) -> Option<PathBuf> {
        absolute_path
            .strip_prefix(&self.local_target)
            .ok()
            .map(Path::to_path_buf)
    }
}
