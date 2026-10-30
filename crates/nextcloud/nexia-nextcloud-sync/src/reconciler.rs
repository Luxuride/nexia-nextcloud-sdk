//! Bidirectional reconciliation logic between local files, remote files, and journal entries.

use std::collections::{BTreeSet, HashMap};

use crate::action::SyncAction;
use crate::journal::SyncJournal;
use crate::scanner::{DiscoveredLocalFile, DiscoveredRemoteFile};

/// Reconciles local files against remote files using the previous sync journal state.
///
/// Implements deterministic 3-way merge logic:
/// - Both unchanged -> [`SyncAction::InSync`]
/// - Local modified, Remote unchanged -> [`SyncAction::Upload`]
/// - Remote modified, Local unchanged -> [`SyncAction::Download`]
/// - Both modified concurrently -> [`SyncAction::Conflict`]
/// - Local deleted, Remote unchanged -> [`SyncAction::DeleteRemote`]
/// - Remote deleted, Local unchanged -> [`SyncAction::DeleteLocal`]
/// - New local item -> [`SyncAction::Upload`] or [`SyncAction::CreateRemoteDirectory`]
/// - New remote item -> [`SyncAction::Download`] or [`SyncAction::CreateLocalDirectory`]
pub fn reconcile(
    local_files: &[DiscoveredLocalFile],
    remote_files: &[DiscoveredRemoteFile],
    journal: &SyncJournal,
) -> Vec<SyncAction> {
    let mut actions = Vec::new();

    let local_map: HashMap<&str, &DiscoveredLocalFile> = local_files
        .iter()
        .map(|f| (f.relative_path.as_str(), f))
        .collect();

    let remote_map: HashMap<&str, &DiscoveredRemoteFile> = remote_files
        .iter()
        .map(|f| (f.relative_path.as_str(), f))
        .collect();

    // Sorted so the action list is identical across runs (and so a parent
    // directory is always ordered before the entries inside it).
    let mut all_paths: BTreeSet<&str> = BTreeSet::new();
    all_paths.extend(local_map.keys().copied());
    all_paths.extend(remote_map.keys().copied());

    for path in &all_paths {
        let local = local_map.get(path);
        let remote = remote_map.get(path);
        let journal_entry = journal.get(path);

        match (local, remote, journal_entry) {
            // Case 1: Present in all three (steady-state check)
            (Some(loc), Some(rem), Some(j)) => {
                if loc.is_directory != rem.is_directory {
                    actions.push(SyncAction::Conflict {
                        relative_path: (*path).to_string(),
                        reason: format!(
                            "Item type mismatch: local is_directory={}, remote is_directory={}",
                            loc.is_directory, rem.is_directory
                        ),
                    });
                } else if loc.is_directory && rem.is_directory {
                    // Both are directories.
                    // Directory ETag changes remotely reflect child file additions or modifications,
                    // which are reconciled individually. Directories never download file content.
                    actions.push(SyncAction::InSync {
                        relative_path: (*path).to_string(),
                    });
                } else {
                    let local_modified = loc.mtime > j.local_mtime || loc.size != j.local_size;
                    let remote_modified = rem.etag != j.remote_etag;

                    if local_modified && remote_modified {
                        actions.push(SyncAction::Conflict {
                            relative_path: (*path).to_string(),
                            reason:
                                "Both local file and remote Nextcloud file were modified concurrently"
                                    .to_string(),
                        });
                    } else if local_modified {
                        actions.push(SyncAction::Upload {
                            relative_path: (*path).to_string(),
                            local_path: loc.absolute_path.clone(),
                            size: loc.size,
                        });
                    } else if remote_modified {
                        actions.push(SyncAction::Download {
                            relative_path: (*path).to_string(),
                            size: rem.size,
                            etag: rem.etag.clone(),
                        });
                    } else {
                        actions.push(SyncAction::InSync {
                            relative_path: (*path).to_string(),
                        });
                    }
                }
            }

            // Case 2: Present in both, but not tracked in journal (initial sync)
            (Some(loc), Some(rem), None) => {
                if (loc.is_directory && rem.is_directory)
                    || (!loc.is_directory && !rem.is_directory && loc.size == rem.size)
                {
                    actions.push(SyncAction::InSync {
                        relative_path: (*path).to_string(),
                    });
                } else {
                    actions.push(SyncAction::Conflict {
                        relative_path: (*path).to_string(),
                        reason: "Item exists on both sides with differing attributes without journal baseline"
                            .to_string(),
                    });
                }
            }

            // Case 3: Present locally, absent remotely
            (Some(loc), None, Some(_j)) => {
                actions.push(SyncAction::DeleteLocal {
                    relative_path: (*path).to_string(),
                    local_path: loc.absolute_path.clone(),
                    is_directory: loc.is_directory,
                });
            }
            (Some(loc), None, None) => {
                if loc.is_directory {
                    actions.push(SyncAction::CreateRemoteDirectory {
                        relative_path: (*path).to_string(),
                    });
                } else {
                    actions.push(SyncAction::Upload {
                        relative_path: (*path).to_string(),
                        local_path: loc.absolute_path.clone(),
                        size: loc.size,
                    });
                }
            }

            // Case 4: Absent locally, present remotely
            (None, Some(rem), Some(_j)) => {
                actions.push(SyncAction::DeleteRemote {
                    relative_path: (*path).to_string(),
                    is_directory: rem.is_directory,
                });
            }
            (None, Some(rem), None) => {
                if rem.is_directory {
                    actions.push(SyncAction::CreateLocalDirectory {
                        relative_path: (*path).to_string(),
                    });
                } else {
                    actions.push(SyncAction::Download {
                        relative_path: (*path).to_string(),
                        size: rem.size,
                        etag: rem.etag.clone(),
                    });
                }
            }

            // Case 5: Neither side present
            (None, None, _) => {}
        }
    }

    actions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::JournalEntry;
    use std::path::PathBuf;
    use std::time::SystemTime;

    #[test]
    fn reconcile_all_state_transitions() {
        let t0 = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(100);
        let t1 = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(200);

        let mut journal = SyncJournal::new();
        journal.record(JournalEntry {
            relative_path: "/steady.txt".to_string(),
            remote_etag: Some("\"etag0\"".to_string()),
            local_size: 100,
            local_mtime: t0,
            is_directory: false,
        });

        // Test InSync
        let local_files = vec![DiscoveredLocalFile {
            relative_path: "/steady.txt".to_string(),
            absolute_path: PathBuf::from("/home/user/Nextcloud/steady.txt"),
            size: 100,
            mtime: t0,
            is_directory: false,
        }];
        let remote_files = vec![DiscoveredRemoteFile {
            relative_path: "/steady.txt".to_string(),
            href: "/remote.php/dav/files/user/steady.txt".to_string(),
            size: 100,
            etag: Some("\"etag0\"".to_string()),
            is_directory: false,
            file_id: None,
        }];

        let actions = reconcile(&local_files, &remote_files, &journal);
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            SyncAction::InSync {
                relative_path: "/steady.txt".to_string()
            }
        );

        // Test Local modified -> Upload
        let local_mod = vec![DiscoveredLocalFile {
            relative_path: "/steady.txt".to_string(),
            absolute_path: PathBuf::from("/home/user/Nextcloud/steady.txt"),
            size: 150,
            mtime: t1,
            is_directory: false,
        }];
        let actions = reconcile(&local_mod, &remote_files, &journal);
        assert!(matches!(actions[0], SyncAction::Upload { .. }));

        // Test Remote modified -> Download
        let remote_mod = vec![DiscoveredRemoteFile {
            relative_path: "/steady.txt".to_string(),
            href: "/remote.php/dav/files/user/steady.txt".to_string(),
            size: 200,
            etag: Some("\"etag1\"".to_string()),
            is_directory: false,
            file_id: None,
        }];
        let actions = reconcile(&local_files, &remote_mod, &journal);
        assert!(matches!(actions[0], SyncAction::Download { .. }));

        // Test Concurrent modifications -> Conflict
        let actions = reconcile(&local_mod, &remote_mod, &journal);
        assert!(matches!(actions[0], SyncAction::Conflict { .. }));
    }

    #[test]
    fn reconcile_directory_etag_change_never_emits_download() {
        let t0 = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(100);
        let mut journal = SyncJournal::new();
        journal.record(JournalEntry {
            relative_path: "/Camera".to_string(),
            remote_etag: Some("\"etag-old\"".to_string()),
            local_size: 0,
            local_mtime: t0,
            is_directory: true,
        });

        let local_files = vec![DiscoveredLocalFile {
            relative_path: "/Camera".to_string(),
            absolute_path: PathBuf::from("/home/user/Nextcloud/Camera"),
            size: 4096,
            mtime: t0,
            is_directory: true,
        }];

        let remote_files = vec![DiscoveredRemoteFile {
            relative_path: "/Camera".to_string(),
            href: "/remote.php/dav/files/user/Camera/".to_string(),
            size: 1048576,
            etag: Some("\"etag-new-from-remote\"".to_string()),
            is_directory: true,
            file_id: None,
        }];

        let actions = reconcile(&local_files, &remote_files, &journal);
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            SyncAction::InSync {
                relative_path: "/Camera".to_string()
            }
        );
    }

    #[test]
    fn reconcile_type_mismatch_produces_conflict() {
        let t0 = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(100);
        let mut journal = SyncJournal::new();
        journal.record(JournalEntry {
            relative_path: "/Camera".to_string(),
            remote_etag: Some("\"etag\"".to_string()),
            local_size: 0,
            local_mtime: t0,
            is_directory: false,
        });

        // Local is a file, but remote is a directory
        let local_files = vec![DiscoveredLocalFile {
            relative_path: "/Camera".to_string(),
            absolute_path: PathBuf::from("/home/user/Nextcloud/Camera"),
            size: 0,
            mtime: t0,
            is_directory: false,
        }];

        let remote_files = vec![DiscoveredRemoteFile {
            relative_path: "/Camera".to_string(),
            href: "/remote.php/dav/files/user/Camera/".to_string(),
            size: 0,
            etag: Some("\"etag\"".to_string()),
            is_directory: true,
            file_id: None,
        }];

        let actions = reconcile(&local_files, &remote_files, &journal);
        assert_eq!(actions.len(), 1);
        assert!(matches!(actions[0], SyncAction::Conflict { .. }));
    }
}
