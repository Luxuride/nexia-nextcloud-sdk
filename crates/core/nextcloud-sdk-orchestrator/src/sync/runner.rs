//! High-level two-way folder synchronization cycle runner.

use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

use crate::error::{OrchestratorError, OrchestratorResult};
use nextcloud_sync::{JournalEntry, SyncAction, SyncJournal, reconcile, scan_local_directory};

/// High-level synchronization engine coordinating scans, 3-way reconciliation, and journal updates.
#[derive(Debug)]
pub struct FolderSyncRunner {
    local_dir: PathBuf,
    remote_dir: String,
    journal_path: PathBuf,
}

impl FolderSyncRunner {
    /// Creates a new `FolderSyncRunner` for the specified local and remote directory pair.
    pub fn new(
        local_dir: impl Into<PathBuf>,
        remote_dir: impl Into<String>,
        journal_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            local_dir: local_dir.into(),
            remote_dir: remote_dir.into(),
            journal_path: journal_path.into(),
        }
    }

    /// Access the local directory path.
    #[must_use]
    pub fn local_dir(&self) -> &Path {
        &self.local_dir
    }

    /// Access the remote directory path.
    #[must_use]
    pub fn remote_dir(&self) -> &str {
        &self.remote_dir
    }

    /// Executes a synchronization reconciliation cycle, scanning the local filesystem and updating the journal.
    pub fn run_local_reconciliation(&self) -> OrchestratorResult<Vec<SyncAction>> {
        if !self.local_dir.exists() {
            fs::create_dir_all(&self.local_dir).map_err(|err| OrchestratorError::Io {
                path: self.local_dir.clone(),
                context: "creating local sync root directory".to_string(),
                source: err,
            })?;
        }

        if let Some(parent) = self.journal_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let mut journal = if self.journal_path.exists() {
            SyncJournal::load_from_file(&self.journal_path).unwrap_or_else(|_| SyncJournal::new())
        } else {
            SyncJournal::new()
        };

        let local_entries =
            scan_local_directory(&self.local_dir).map_err(|e| OrchestratorError::Sync {
                operation: "scanning local directory".to_string(),
                source: e,
            })?;

        // Reconcile differences: local vs empty remote against journal
        let actions = reconcile(&local_entries, &[], &journal);

        // Commit newly discovered local files into the journal
        for local_entry in &local_entries {
            let j_entry = JournalEntry::new(
                &local_entry.relative_path,
                local_entry.size,
                local_entry.mtime,
                local_entry.is_directory,
            );
            journal.record(j_entry);
        }

        let _ = journal.save_to_file(&self.journal_path);

        info!(
            local = %self.local_dir.display(),
            remote = %self.remote_dir,
            actions_count = actions.len(),
            "completed folder sync reconciliation cycle"
        );

        Ok(actions)
    }
}
