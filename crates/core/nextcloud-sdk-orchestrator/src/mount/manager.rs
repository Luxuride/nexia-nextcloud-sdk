//! Mount manager tracking multiple concurrent mount points and file statuses.

use std::collections::HashMap;
use std::path::Path;

use crate::error::{OrchestratorError, OrchestratorResult};
use crate::mount::{MountDescriptor, MountId, MountState, MountStatus, SyncFileStatus};

/// Registry and state tracker managing multiple mount points to local targets for root or subdirectories.
#[derive(Debug, Default)]
pub struct MountManager {
    mounts: HashMap<MountId, (MountDescriptor, MountState)>,
    file_statuses: HashMap<(MountId, String), SyncFileStatus>,
}

impl MountManager {
    /// Creates an empty mount manager.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a new mount point. Fails if a mount with the same ID or local target already exists.
    pub fn add_mount(&mut self, descriptor: MountDescriptor) -> OrchestratorResult<MountId> {
        if self.mounts.contains_key(&descriptor.id) {
            return Err(OrchestratorError::MountAlreadyExists {
                reason: format!("mount ID '{}' already registered", descriptor.id),
            });
        }

        for (existing_desc, _) in self.mounts.values() {
            if existing_desc.local_target == descriptor.local_target {
                return Err(OrchestratorError::MountAlreadyExists {
                    reason: format!(
                        "target path '{}' is already mounted by mount '{}'",
                        descriptor.local_target.display(),
                        existing_desc.id
                    ),
                });
            }
        }

        let id = descriptor.id.clone();
        let state = MountState {
            status: MountStatus::Idle,
            last_sync_timestamp: None,
            total_items: 0,
            items_synced: 0,
            error_message: None,
        };

        self.mounts.insert(id.clone(), (descriptor, state));
        Ok(id)
    }

    /// Removes an existing mount point by its identifier.
    pub fn remove_mount(&mut self, id: &MountId) -> OrchestratorResult<MountDescriptor> {
        let (desc, _) = self
            .mounts
            .remove(id)
            .ok_or_else(|| OrchestratorError::MountNotFound { id: id.clone() })?;

        // Prune associated file statuses
        self.file_statuses.retain(|(m_id, _), _| m_id != id);

        Ok(desc)
    }

    /// Retrieves an immutable reference to the mount descriptor and its state.
    #[must_use]
    pub fn get_mount(&self, id: &MountId) -> Option<(&MountDescriptor, &MountState)> {
        self.mounts.get(id).map(|(desc, state)| (desc, state))
    }

    /// Retrieves a mutable reference to the mount descriptor and state.
    pub fn get_mount_mut(
        &mut self,
        id: &MountId,
    ) -> Option<(&mut MountDescriptor, &mut MountState)> {
        self.mounts.get_mut(id).map(|(desc, state)| (desc, state))
    }

    /// Returns a list of all configured mounts and their current runtime state.
    #[must_use]
    pub fn list_mounts(&self) -> Vec<(MountDescriptor, MountState)> {
        self.mounts.values().cloned().collect()
    }

    /// Finds the mount whose local target directory contains the specified local filesystem path.
    #[must_use]
    pub fn find_mount_for_path<'a>(
        &'a self,
        local_path: &Path,
    ) -> Option<(&'a MountDescriptor, &'a MountState)> {
        self.mounts
            .values()
            .find(|(desc, _)| local_path.starts_with(&desc.local_target))
            .map(|(desc, state)| (desc, state))
    }

    /// Sets the status of an active mount.
    pub fn set_mount_status(
        &mut self,
        id: &MountId,
        status: MountStatus,
    ) -> OrchestratorResult<()> {
        let (_, state) = self
            .mounts
            .get_mut(id)
            .ok_or_else(|| OrchestratorError::MountNotFound { id: id.clone() })?;

        state.status = status;
        Ok(())
    }

    /// Records or updates the synchronization status of a specific file inside a mount.
    pub fn set_file_status(
        &mut self,
        id: MountId,
        relative_path: impl Into<String>,
        status: SyncFileStatus,
    ) {
        let key = (id, normalize_relative_path(&relative_path.into()));
        self.file_statuses.insert(key, status);
    }

    /// Queries the synchronization status of a specific file inside a mount.
    #[must_use]
    pub fn file_status(&self, id: &MountId, relative_path: &str) -> SyncFileStatus {
        let key = (id.clone(), normalize_relative_path(relative_path));
        self.file_statuses
            .get(&key)
            .cloned()
            .unwrap_or(SyncFileStatus::OnlineOnly)
    }

    /// Resolves the file synchronization status for an arbitrary local path across all active mounts.
    #[must_use]
    pub fn status_for_local_path(&self, local_path: &Path) -> Option<SyncFileStatus> {
        let (desc, _) = self.find_mount_for_path(local_path)?;
        let rel_path = desc.relative_path(local_path)?;
        Some(self.file_status(&desc.id, &rel_path.to_string_lossy()))
    }
}

fn normalize_relative_path(path: &str) -> String {
    let trimmed = path.trim();
    let without_leading = trimmed.strip_prefix('/').unwrap_or(trimmed);
    let without_trailing = without_leading.strip_suffix('/').unwrap_or(without_leading);
    without_trailing.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mount::MountMode;
    use std::path::PathBuf;

    #[test]
    fn test_mount_manager_add_and_duplicate_prevention() {
        let mut manager = MountManager::new();
        let target_a = PathBuf::from("/mnt/nextcloud_root");
        let target_b = PathBuf::from("/mnt/nextcloud_photos");

        let desc_a = MountDescriptor::new("acc-1", "/", &target_a, MountMode::VirtualVfs);
        let id_a = desc_a.id.clone();
        assert!(manager.add_mount(desc_a.clone()).is_ok());

        // Duplicate mount ID rejected
        assert!(manager.add_mount(desc_a).is_err());

        // Duplicate local target rejected
        let desc_collision =
            MountDescriptor::new("acc-1", "/Other", &target_a, MountMode::BidirectionalSync);
        assert!(manager.add_mount(desc_collision).is_err());

        // Subdirectory mount accepted
        let desc_b = MountDescriptor::new("acc-1", "/Photos", &target_b, MountMode::VirtualVfs);
        let id_b = desc_b.id.clone();
        assert!(manager.add_mount(desc_b).is_ok());

        assert_eq!(manager.list_mounts().len(), 2);

        // Path resolution
        let file_in_photos = target_b.join("2026/vacation.png");
        let found = manager.find_mount_for_path(&file_in_photos);
        assert!(found.is_some());
        assert_eq!(found.unwrap().0.id, id_b);

        // Removal
        assert!(manager.remove_mount(&id_a).is_ok());
        assert_eq!(manager.list_mounts().len(), 1);
    }

    #[test]
    fn test_file_status_tracking() {
        let mut manager = MountManager::new();
        let target = PathBuf::from("/mnt/cloud");
        let desc = MountDescriptor::new("acc-1", "/", &target, MountMode::VirtualVfs);
        let id = desc.id.clone();
        manager.add_mount(desc).unwrap();

        // Default is OnlineOnly
        assert_eq!(
            manager.file_status(&id, "Notes.md"),
            SyncFileStatus::OnlineOnly
        );

        // Update to Synced
        manager.set_file_status(id.clone(), "Notes.md", SyncFileStatus::Synced);
        assert_eq!(manager.file_status(&id, "Notes.md"), SyncFileStatus::Synced);

        // Lookup by local path
        let local_file = target.join("Notes.md");
        assert_eq!(
            manager.status_for_local_path(&local_file),
            Some(SyncFileStatus::Synced)
        );
    }
}
