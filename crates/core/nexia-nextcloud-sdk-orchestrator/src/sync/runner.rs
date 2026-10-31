//! High-level two-way folder synchronization cycle runner.

use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

use crate::error::{OrchestratorError, OrchestratorResult};
use nexia_nextcloud_core::{AuthProvider, Transport};
use nexia_nextcloud_dav::{DavClient, DavPath, Depth, PropfindBody};
use nexia_nextcloud_sync::{
    DiscoveredRemoteFile, JournalEntry, SyncAction, SyncError, SyncJournal, reconcile,
    reconcile_local_changes, scan_local_directory,
};

/// Helper to construct a validated [`DavPath`] from user name and relative path.
pub fn make_dav_path(user: &str, relative: &str) -> OrchestratorResult<DavPath> {
    let clean = relative.trim_matches('/');
    if clean.is_empty() {
        DavPath::root(user).map_err(|err| OrchestratorError::InvalidPath {
            path: relative.to_string(),
            reason: err.to_string(),
        })
    } else {
        DavPath::new(user, clean).map_err(|err| OrchestratorError::InvalidPath {
            path: relative.to_string(),
            reason: err.to_string(),
        })
    }
}

fn percent_decode(s: &str) -> String {
    let mut bytes = Vec::with_capacity(s.len());
    let mut chars = s.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let h1 = chars.next();
            let h2 = chars.next();
            if let (Some(h1), Some(h2)) = (h1, h2) {
                let hex_str = [h1, h2];
                if let Ok(hex) = std::str::from_utf8(&hex_str)
                    && let Ok(val) = u8::from_str_radix(hex, 16)
                {
                    bytes.push(val);
                    continue;
                }
            }
        }
        bytes.push(b);
    }
    String::from_utf8(bytes).unwrap_or_else(|_| s.to_string())
}

/// Detailed summary of actions performed during a folder synchronization cycle.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FolderSyncOutcome {
    /// Total actions identified during reconciliation.
    pub total_actions: usize,
    /// Number of files downloaded (or sparse placeholders created in VFS mode).
    pub downloaded: usize,
    /// Number of files uploaded to Nextcloud.
    pub uploaded: usize,
    /// Number of local files or directories deleted.
    pub deleted_local: usize,
    /// Number of remote files or directories deleted.
    pub deleted_remote: usize,
    /// Number of directories created locally.
    pub created_local_dirs: usize,
    /// Number of directories created remotely.
    pub created_remote_dirs: usize,
    /// Number of conflicting files resolved.
    pub conflicts: usize,
}

/// High-level synchronization engine coordinating scans, 3-way reconciliation, and journal updates.
#[derive(Debug)]
pub struct FolderSyncRunner {
    local_dir: PathBuf,
    remote_dir: String,
    journal_path: PathBuf,
}

/// Loads the sync journal, keeping a corrupt file from silently discarding the baseline.
///
/// The journal is what distinguishes "the user deleted this" from "this was never synced", so an
/// unreadable journal is reported loudly. The cycle still proceeds from an empty baseline, which is
/// the conservative choice: everything is re-examined instead of assumed deleted.
fn load_journal(path: &Path) -> SyncJournal {
    if !path.exists() {
        return SyncJournal::new();
    }

    SyncJournal::load_from_file(path).unwrap_or_else(|error| {
        tracing::warn!(
            path = %path.display(),
            error = %error,
            "sync journal could not be read; continuing from an empty baseline"
        );
        SyncJournal::new()
    })
}

/// Saves the journal, reporting a failure instead of dropping it.
///
/// These calls sit next to file operations that already happened, so the caller cannot undo them;
/// what it can do is stop reporting a clean cycle when the baseline was not recorded.
fn save_journal(path: &Path, journal: &SyncJournal) -> Result<(), SyncError> {
    journal.save_to_file(path).map_err(|source| {
        tracing::error!(path = %path.display(), error = %source, "sync journal could not be saved");
        source
    })
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

    /// Access the journal file path.
    #[must_use]
    pub fn journal_path(&self) -> &Path {
        &self.journal_path
    }

    /// Executes a synchronization reconciliation cycle, scanning the local filesystem and updating the journal.
    ///
    /// The remote side is not queried here, so the returned plan only states what the local scan would
    /// have to upload (see [`reconcile_local_changes`]); it never plans a deletion.
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

        let mut journal = load_journal(&self.journal_path);

        let local_entries =
            scan_local_directory(&self.local_dir).map_err(|e| OrchestratorError::Sync {
                operation: "scanning local directory".to_string(),
                source: e,
            })?;

        let actions = reconcile_local_changes(&local_entries, &journal);

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

        // This call exists to update the journal, so a failed save is a failed cycle: reporting it
        // keeps the next run from comparing the tree against a baseline that never recorded this one.
        journal
            .save_to_file(&self.journal_path)
            .map_err(|source| OrchestratorError::Sync {
                operation: format!("saving sync journal '{}'", self.journal_path.display()),
                source,
            })?;

        info!(
            local = %self.local_dir.display(),
            remote = %self.remote_dir,
            actions_count = actions.len(),
            "completed folder sync reconciliation cycle"
        );

        Ok(actions)
    }

    /// Recursively queries the remote Nextcloud instance via WebDAV PROPFIND to discover all files and collections.
    pub async fn discover_remote<T: Transport, A: AuthProvider>(
        dav_client: &DavClient<'_, T, A>,
        user: &str,
        remote_root: &str,
    ) -> OrchestratorResult<Vec<DiscoveredRemoteFile>> {
        let dav_prefix = format!("/remote.php/dav/files/{user}");
        let mut collected = Vec::new();
        let mut dir_queue = vec![remote_root.to_string()];
        let mut visited = std::collections::HashSet::new();
        let body = PropfindBody::all();

        while let Some(current_dir) = dir_queue.pop() {
            let clean_current = if current_dir.is_empty() || current_dir == "/" {
                "/".to_string()
            } else {
                format!("/{}", current_dir.trim_matches('/'))
            };

            if !visited.insert(clean_current.clone()) {
                continue;
            }

            let dav_path = make_dav_path(user, &clean_current)?;
            let resources = match dav_client.propfind(&dav_path, Depth::One, &body).await {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(dir = %clean_current, error = %e, "WebDAV propfind failed during discovery");
                    return Err(OrchestratorError::Api {
                        operation: format!("propfind at '{clean_current}'"),
                        source: e,
                    });
                }
            };

            for res in resources {
                let decoded_href = percent_decode(&res.href);
                let path_after_prefix = if let Some(idx) = decoded_href.find(&dav_prefix) {
                    &decoded_href[idx + dav_prefix.len()..]
                } else if let Some(p) = decoded_href.strip_prefix(&dav_prefix) {
                    p
                } else if let Some(idx) = decoded_href.find("/remote.php/dav/files/") {
                    let after_files = &decoded_href[idx + "/remote.php/dav/files/".len()..];
                    if let Some(slash) = after_files.find('/') {
                        &after_files[slash..]
                    } else {
                        ""
                    }
                } else if let Some(idx) = decoded_href.find("/remote.php/webdav") {
                    &decoded_href[idx + "/remote.php/webdav".len()..]
                } else {
                    continue;
                };

                let clean_path = format!("/{}", path_after_prefix.trim_matches('/'));

                // Skip the queried collection directory itself
                if clean_path == clean_current || (clean_path == "/" && clean_current == "/") {
                    continue;
                }

                // Calculate relative path within the mount root
                let clean_mount_root = if remote_root.is_empty() || remote_root == "/" {
                    "/".to_string()
                } else {
                    format!("/{}", remote_root.trim_matches('/'))
                };

                let relative_path = if clean_mount_root == "/" {
                    clean_path.clone()
                } else if let Some(rel) = clean_path.strip_prefix(&clean_mount_root) {
                    format!("/{}", rel.trim_matches('/'))
                } else {
                    continue;
                };

                let is_col =
                    res.is_collection || res.href.ends_with('/') || relative_path.ends_with('/');
                let size = res.content_length.unwrap_or(0);
                let etag = res.etag.clone();
                let file_id = res
                    .file_id
                    .as_deref()
                    .and_then(|id| id.trim().parse::<u64>().ok());

                collected.push(DiscoveredRemoteFile {
                    relative_path: relative_path.clone(),
                    href: res.href.clone(),
                    size,
                    etag,
                    is_directory: is_col,
                    file_id,
                });

                if is_col && !visited.contains(&clean_path) {
                    dir_queue.push(clean_path);
                }
            }
        }

        Ok(collected)
    }

    /// Reconciles local and remote directory state against the synchronization journal, enforcing deletion safeguards.
    pub fn plan_reconciliation(
        &self,
        remote_entries: &[DiscoveredRemoteFile],
        is_vfs: bool,
    ) -> OrchestratorResult<(Vec<SyncAction>, SyncJournal)> {
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

        let journal = load_journal(&self.journal_path);

        let local_entries =
            scan_local_directory(&self.local_dir).map_err(|e| OrchestratorError::Sync {
                operation: "scanning local directory".to_string(),
                source: e,
            })?;

        let mut actions = reconcile(&local_entries, remote_entries, &journal);

        // Safeguards
        let local_deletes_count = actions
            .iter()
            .filter(|a| matches!(a, SyncAction::DeleteLocal { .. }))
            .count();

        if local_deletes_count > 0 {
            // Safeguard 1: Remote discovery returned 0 files while local files exist
            if remote_entries.is_empty() && !local_entries.is_empty() {
                return Err(OrchestratorError::Sync {
                    operation: "reconciliation safeguard".to_string(),
                    source: nexia_nextcloud_sync::SyncError::Reconciliation(
                        "Safety safeguard: remote listing returned 0 files; refusing to delete local files".into(),
                    ),
                });
            }

            // Safeguard 2: Bulk deletion exceeds 50% threshold when local file count > 5
            if local_entries.len() > 5 && local_deletes_count > local_entries.len() / 2 {
                return Err(OrchestratorError::Sync {
                    operation: "reconciliation safeguard".to_string(),
                    source: nexia_nextcloud_sync::SyncError::Reconciliation(
                        "Safety safeguard: bulk deletion of more than 50% of files requires manual review".into(),
                    ),
                });
            }

            // Safeguard 3: In VirtualVfs mode, do not delete local files or placeholders automatically
            if is_vfs {
                actions.retain(|a| !matches!(a, SyncAction::DeleteLocal { .. }));
                for action in &mut actions {
                    if let SyncAction::DeleteRemote {
                        relative_path,
                        is_directory,
                    } = action
                    {
                        if *is_directory {
                            *action = SyncAction::CreateLocalDirectory {
                                relative_path: relative_path.clone(),
                            };
                        } else if let Some(rem) = remote_entries
                            .iter()
                            .find(|r| &r.relative_path == relative_path)
                        {
                            *action = SyncAction::Download {
                                relative_path: relative_path.clone(),
                                size: rem.size,
                                etag: rem.etag.clone(),
                            };
                        }
                    }
                }
            }
        }

        // In Two-Way Sync (not VFS), convert any InSync placeholders (0 bytes locally but > 0 bytes remotely) to Download actions
        if !is_vfs {
            for action in &mut actions {
                if let SyncAction::InSync { relative_path } = action
                    && let Some(rem) = remote_entries
                        .iter()
                        .find(|r| &r.relative_path == relative_path)
                    && !rem.is_directory
                {
                    let local_p = self.local_dir.join(relative_path.trim_start_matches('/'));
                    let local_len = fs::metadata(&local_p).map(|m| m.len()).unwrap_or(0);
                    if rem.size > 0 && local_len == 0 {
                        *action = SyncAction::Download {
                            relative_path: relative_path.clone(),
                            size: rem.size,
                            etag: rem.etag.clone(),
                        };
                    }
                }
            }
        }

        Ok((actions, journal))
    }

    /// Creates or updates a local sparse placeholder file with virtual size and records it in the journal.
    pub fn apply_placeholder(
        &self,
        relative_path: &str,
        size: u64,
        etag: Option<String>,
        journal: &mut SyncJournal,
    ) -> OrchestratorResult<PathBuf> {
        let dest = self.local_dir.join(relative_path.trim_start_matches('/'));
        if dest.is_dir() {
            return Err(OrchestratorError::Io {
                path: dest.clone(),
                context: "cannot create virtual placeholder: target path is a directory"
                    .to_string(),
                source: std::io::Error::new(
                    std::io::ErrorKind::IsADirectory,
                    "target path is a directory",
                ),
            });
        }
        if let Some(parent) = dest.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let file = if dest.exists() {
            fs::OpenOptions::new().write(true).open(&dest)
        } else {
            fs::File::create(&dest)
        }
        .map_err(|err| OrchestratorError::Io {
            path: dest.clone(),
            context: "creating virtual placeholder".to_string(),
            source: err,
        })?;

        if size > 0
            && let Err(error) = file.set_len(size)
        {
            tracing::warn!(
                path = %dest.display(),
                size,
                %error,
                "could not set the placeholder length; the file stays allocated"
            );
        }

        let mtime = fs::metadata(&dest)
            .and_then(|m| m.modified())
            .unwrap_or_else(|_| std::time::SystemTime::now());

        let mut entry = JournalEntry::new(relative_path, size, mtime, false);
        if let Some(e) = etag {
            entry = entry.with_remote_etag(e);
        }
        journal.record(entry);

        let mtime_sec = mtime
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        nexia_nextcloud_preview::install_freedesktop_fail_marker(&dest, mtime_sec);

        Ok(dest)
    }

    /// Executes a full synchronization cycle against a remote Nextcloud instance.
    pub async fn execute_sync_cycle<T, A, F, P>(
        &self,
        dav_client: &DavClient<'_, T, A>,
        user: &str,
        is_vfs: bool,
        is_cancelled: F,
        on_progress: P,
    ) -> OrchestratorResult<FolderSyncOutcome>
    where
        T: Transport,
        A: AuthProvider,
        F: Fn() -> bool + Send + Sync,
        P: Fn(usize, usize, &str) + Send + Sync,
    {
        if is_cancelled() {
            return Ok(FolderSyncOutcome::default());
        }

        let remote_files = Self::discover_remote(dav_client, user, &self.remote_dir).await?;
        let (actions, mut journal) = self.plan_reconciliation(&remote_files, is_vfs)?;

        let total_actions = actions.len();
        let mut outcome = FolderSyncOutcome {
            total_actions,
            ..Default::default()
        };

        let (dirs, file_actions): (Vec<_>, Vec<_>) = actions.into_iter().partition(|a| {
            matches!(
                a,
                SyncAction::CreateLocalDirectory { .. } | SyncAction::CreateRemoteDirectory { .. }
            )
        });

        let mut processed = 0;
        for action in dirs.into_iter().chain(file_actions) {
            if is_cancelled() {
                break;
            }

            let rel_path = action.relative_path();

            match &action {
                SyncAction::CreateLocalDirectory { relative_path } => {
                    let dest = self.local_dir.join(relative_path.trim_start_matches('/'));
                    if dest.is_file() {
                        let len = fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
                        if len == 0 {
                            let _ = fs::remove_file(&dest);
                        } else {
                            let conflict_path = dest.with_extension(format!(
                                "file-conflict-{}.part",
                                std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .map(|d| d.as_secs())
                                    .unwrap_or(0)
                            ));
                            let _ = fs::rename(&dest, &conflict_path);
                        }
                    }
                    if let Err(e) = fs::create_dir_all(&dest) {
                        tracing::error!(path = %dest.display(), error = %e, "failed to create local sync directory");
                    } else {
                        let rem_etag = remote_files
                            .iter()
                            .find(|r| &r.relative_path == relative_path)
                            .and_then(|r| r.etag.clone());
                        let mut entry =
                            JournalEntry::new(relative_path, 0, std::time::SystemTime::now(), true);
                        if let Some(e) = rem_etag {
                            entry = entry.with_remote_etag(e);
                        }
                        journal.record(entry);
                        outcome.created_local_dirs += 1;
                    }
                }
                SyncAction::CreateRemoteDirectory { relative_path } => {
                    let full_remote = if self.remote_dir == "/" {
                        relative_path.clone()
                    } else {
                        format!(
                            "{}/{}",
                            self.remote_dir.trim_end_matches('/'),
                            relative_path.trim_start_matches('/')
                        )
                    };
                    if let Ok(dav_path) = make_dav_path(user, &full_remote) {
                        if let Err(e) = dav_client.mkcol(&dav_path).await {
                            tracing::error!(path = %full_remote, error = %e, "failed to create remote directory");
                        } else {
                            journal.record(JournalEntry::new(
                                relative_path,
                                0,
                                std::time::SystemTime::now(),
                                true,
                            ));
                            outcome.created_remote_dirs += 1;
                        }
                    }
                }
                SyncAction::Download {
                    relative_path,
                    size,
                    etag,
                } => {
                    let dest = self.local_dir.join(relative_path.trim_start_matches('/'));
                    let is_rem_dir = remote_files
                        .iter()
                        .find(|r| &r.relative_path == relative_path)
                        .is_some_and(|r| r.is_directory);

                    if is_rem_dir {
                        if dest.is_file() {
                            let _ = fs::remove_file(&dest);
                        }
                        let _ = fs::create_dir_all(&dest);
                        let mut entry =
                            JournalEntry::new(relative_path, 0, std::time::SystemTime::now(), true);
                        if let Some(e) = etag.clone() {
                            entry = entry.with_remote_etag(e);
                        }
                        journal.record(entry);
                        outcome.created_local_dirs += 1;
                        processed += 1;
                        on_progress(processed, total_actions, rel_path);
                        continue;
                    }

                    let full_remote = if self.remote_dir == "/" {
                        relative_path.clone()
                    } else {
                        format!(
                            "{}/{}",
                            self.remote_dir.trim_end_matches('/'),
                            relative_path.trim_start_matches('/')
                        )
                    };

                    if is_vfs {
                        let _ = self.apply_placeholder(
                            relative_path,
                            *size,
                            etag.clone(),
                            &mut journal,
                        );
                        outcome.downloaded += 1;
                    } else if let Ok(dav_path) = make_dav_path(user, &full_remote) {
                        match dav_client.get(&dav_path).await {
                            Ok(bytes) => {
                                if let Some(parent) = dest.parent() {
                                    let _ = fs::create_dir_all(parent);
                                }
                                if let Err(e) = fs::write(&dest, &bytes) {
                                    tracing::error!(dest = %dest.display(), error = %e, "failed to write downloaded sync file");
                                } else {
                                    let mtime = fs::metadata(&dest)
                                        .and_then(|m| m.modified())
                                        .unwrap_or_else(|_| std::time::SystemTime::now());
                                    let mut entry = JournalEntry::new(
                                        relative_path,
                                        bytes.len() as u64,
                                        mtime,
                                        false,
                                    );
                                    if let Some(e) = etag {
                                        entry = entry.with_remote_etag(e.clone());
                                    }
                                    journal.record(entry);
                                    outcome.downloaded += 1;
                                }
                            }
                            Err(e) => {
                                tracing::error!(path = %full_remote, error = %e, "failed to download sync file");
                            }
                        }
                    }
                }
                SyncAction::Upload {
                    relative_path,
                    local_path,
                    size,
                } => {
                    let full_remote = if self.remote_dir == "/" {
                        relative_path.clone()
                    } else {
                        format!(
                            "{}/{}",
                            self.remote_dir.trim_end_matches('/'),
                            relative_path.trim_start_matches('/')
                        )
                    };

                    if let Ok(dav_path) = make_dav_path(user, &full_remote) {
                        match fs::read(local_path) {
                            Ok(bytes) => match dav_client.put(&dav_path, bytes, None).await {
                                Ok(()) => {
                                    let mtime = fs::metadata(local_path)
                                        .and_then(|m| m.modified())
                                        .unwrap_or_else(|_| std::time::SystemTime::now());
                                    journal.record(JournalEntry::new(
                                        relative_path,
                                        *size,
                                        mtime,
                                        false,
                                    ));
                                    outcome.uploaded += 1;
                                }
                                Err(e) => {
                                    tracing::error!(path = %full_remote, error = %e, "failed to upload sync file");
                                }
                            },
                            Err(e) => {
                                tracing::error!(path = %local_path.display(), error = %e, "failed to read local file for upload");
                            }
                        }
                    }
                }
                SyncAction::DeleteLocal {
                    relative_path,
                    local_path,
                    is_directory,
                } => {
                    if *is_directory {
                        let _ = fs::remove_dir_all(local_path);
                    } else if local_path.exists() {
                        let _ = fs::remove_file(local_path);
                    }
                    journal.remove(relative_path);
                    outcome.deleted_local += 1;
                }
                SyncAction::DeleteRemote { relative_path, .. } => {
                    let full_remote = if self.remote_dir == "/" {
                        relative_path.clone()
                    } else {
                        format!(
                            "{}/{}",
                            self.remote_dir.trim_end_matches('/'),
                            relative_path.trim_start_matches('/')
                        )
                    };
                    if let Ok(dav_path) = make_dav_path(user, &full_remote) {
                        if let Err(e) = dav_client.delete(&dav_path).await {
                            tracing::error!(path = %full_remote, error = %e, "failed to delete remote file");
                        } else {
                            journal.remove(relative_path);
                            outcome.deleted_remote += 1;
                        }
                    }
                }
                SyncAction::Conflict { relative_path, .. } => {
                    let orig = self.local_dir.join(relative_path.trim_start_matches('/'));
                    let rem = remote_files
                        .iter()
                        .find(|r| &r.relative_path == relative_path);
                    let is_rem_dir = rem.map(|r| r.is_directory).unwrap_or(false);

                    if is_rem_dir {
                        if orig.is_file() {
                            let len = fs::metadata(&orig).map(|m| m.len()).unwrap_or(0);
                            if len == 0 {
                                let _ = fs::remove_file(&orig);
                            } else {
                                let conflict_path = orig.with_extension(format!(
                                    "conflict-{}.part",
                                    std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .map(|d| d.as_secs())
                                        .unwrap_or(0)
                                ));
                                let _ = fs::rename(&orig, &conflict_path);
                            }
                        }
                        if let Err(e) = fs::create_dir_all(&orig) {
                            tracing::error!(path = %orig.display(), error = %e, "failed to create local sync directory during conflict resolution");
                        } else {
                            let mut entry = JournalEntry::new(
                                relative_path,
                                0,
                                std::time::SystemTime::now(),
                                true,
                            );
                            if let Some(e) = rem.and_then(|r| r.etag.clone()) {
                                entry = entry.with_remote_etag(e);
                            }
                            journal.record(entry);
                            outcome.created_local_dirs += 1;
                        }
                        processed += 1;
                        on_progress(processed, total_actions, rel_path);
                        continue;
                    }

                    if orig.exists() {
                        let conflict_path = orig.with_extension(format!(
                            "conflict-{}.part",
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_secs())
                                .unwrap_or(0)
                        ));
                        let _ = fs::rename(&orig, &conflict_path);
                    }
                    let rem_size = rem.map(|r| r.size).unwrap_or(0);
                    let rem_etag = rem.and_then(|r| r.etag.clone());

                    let _ = self.apply_placeholder(relative_path, rem_size, rem_etag, &mut journal);
                    outcome.conflicts += 1;
                }
                SyncAction::InSync { relative_path } => {
                    if let Some(rem) = remote_files
                        .iter()
                        .find(|r| &r.relative_path == relative_path)
                        && rem.is_directory
                    {
                        let mut entry =
                            JournalEntry::new(relative_path, 0, std::time::SystemTime::now(), true);
                        if let Some(ref e) = rem.etag {
                            entry = entry.with_remote_etag(e.clone());
                        }
                        journal.record(entry);
                    }
                }
            }

            processed += 1;
            on_progress(processed, total_actions, rel_path);
        }

        save_journal(&self.journal_path, &journal).map_err(|source| OrchestratorError::Sync {
            operation: format!("saving sync journal '{}'", self.journal_path.display()),
            source,
        })?;
        Ok(outcome)
    }

    /// Downloads and fully hydrates a remote file payload to a local target destination.
    pub async fn hydrate_file_from_dav<T: Transport, A: AuthProvider>(
        dav_client: &DavClient<'_, T, A>,
        user: &str,
        remote_path: &str,
        dest_path: &Path,
        journal_path: Option<&Path>,
    ) -> OrchestratorResult<u64> {
        if dest_path.is_dir() {
            return Err(OrchestratorError::Io {
                path: dest_path.to_path_buf(),
                context: "cannot hydrate a directory as a file".to_string(),
                source: std::io::Error::new(
                    std::io::ErrorKind::IsADirectory,
                    "target path is a directory",
                ),
            });
        }

        let dav_path = make_dav_path(user, remote_path)?;
        let bytes = dav_client
            .get(&dav_path)
            .await
            .map_err(|e| OrchestratorError::Api {
                operation: format!("downloading '{remote_path}' for hydration"),
                source: e,
            })?;

        if let Some(parent) = dest_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        fs::write(dest_path, &bytes).map_err(|err| OrchestratorError::Io {
            path: dest_path.to_path_buf(),
            context: "writing hydrated file".to_string(),
            source: err,
        })?;

        if let Some(jpath) = journal_path {
            let mut journal = load_journal(jpath);
            let mtime = fs::metadata(dest_path)
                .and_then(|m| m.modified())
                .unwrap_or_else(|_| std::time::SystemTime::now());
            journal.record(JournalEntry::new(
                remote_path,
                bytes.len() as u64,
                mtime,
                false,
            ));
            let _ = save_journal(jpath, &journal);
        }

        nexia_nextcloud_preview::remove_freedesktop_fail_marker(dest_path);
        Ok(bytes.len() as u64)
    }

    /// Dehydrates a local file back into a sparse placeholder with 0 disk allocation.
    pub fn dehydrate_file(
        dest_path: &Path,
        remote_path: &str,
        journal_path: Option<&Path>,
    ) -> OrchestratorResult<()> {
        if dest_path.is_dir() {
            return Err(OrchestratorError::Io {
                path: dest_path.to_path_buf(),
                context: "cannot dehydrate a directory as a file".to_string(),
                source: std::io::Error::new(
                    std::io::ErrorKind::IsADirectory,
                    "target path is a directory",
                ),
            });
        }

        let (size, etag) = if let Some(jpath) = journal_path
            && let Ok(journal) = SyncJournal::load_from_file(jpath)
            && let Some(entry) = journal.get(remote_path)
        {
            (entry.local_size, entry.remote_etag.clone())
        } else {
            let len = fs::metadata(dest_path).map(|m| m.len()).unwrap_or(0);
            (len, None)
        };

        let file = fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(dest_path)
            .map_err(|err| OrchestratorError::Io {
                path: dest_path.to_path_buf(),
                context: "truncating file for dehydration".to_string(),
                source: err,
            })?;

        if size > 0
            && let Err(error) = file.set_len(size)
        {
            tracing::warn!(
                path = %dest_path.display(),
                size,
                %error,
                "could not set the placeholder length; the file stays allocated"
            );
        }

        if let Some(jpath) = journal_path {
            let mut journal = load_journal(jpath);
            let mtime = fs::metadata(dest_path)
                .and_then(|m| m.modified())
                .unwrap_or_else(|_| std::time::SystemTime::now());
            let mut entry = JournalEntry::new(remote_path, size, mtime, false);
            if let Some(e) = etag {
                entry = entry.with_remote_etag(e);
            }
            journal.record(entry);
            let _ = save_journal(jpath, &journal);
        }

        let mtime_sec = fs::metadata(dest_path)
            .and_then(|m| m.modified())
            .unwrap_or_else(|_| std::time::SystemTime::now())
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        nexia_nextcloud_preview::install_freedesktop_fail_marker(dest_path, mtime_sec);

        Ok(())
    }

    /// Creates a collection directory on the remote Nextcloud instance via WebDAV MKCOL.
    pub async fn create_remote_directory<T: Transport, A: AuthProvider>(
        dav_client: &DavClient<'_, T, A>,
        user: &str,
        remote_path: &str,
    ) -> OrchestratorResult<()> {
        let dav_path = make_dav_path(user, remote_path)?;
        dav_client
            .mkcol(&dav_path)
            .await
            .map_err(|e| OrchestratorError::Api {
                operation: format!("creating remote directory '{remote_path}'"),
                source: e,
            })
    }

    /// Deletes a file or collection on the remote Nextcloud instance via WebDAV DELETE.
    pub async fn delete_remote_resource<T: Transport, A: AuthProvider>(
        dav_client: &DavClient<'_, T, A>,
        user: &str,
        remote_path: &str,
    ) -> OrchestratorResult<()> {
        let dav_path = make_dav_path(user, remote_path)?;
        dav_client
            .delete(&dav_path)
            .await
            .map_err(|e| OrchestratorError::Api {
                operation: format!("deleting remote resource '{remote_path}'"),
                source: e,
            })
    }

    /// Moves or renames a file or collection on the remote Nextcloud instance via WebDAV MOVE.
    pub async fn move_remote_resource<T: Transport, A: AuthProvider>(
        dav_client: &DavClient<'_, T, A>,
        user: &str,
        source_path: &str,
        target_path: &str,
    ) -> OrchestratorResult<()> {
        let source_dav = make_dav_path(user, source_path)?;
        let target_dav = make_dav_path(user, target_path)?;
        dav_client
            .move_(&source_dav, &target_dav, false)
            .await
            .map_err(|e| OrchestratorError::Api {
                operation: format!(
                    "moving remote resource from '{source_path}' to '{target_path}'"
                ),
                source: e,
            })
    }

    /// Uploads arbitrary byte content to a remote Nextcloud file path via WebDAV PUT.
    pub async fn upload_file_to_dav<T: Transport, A: AuthProvider>(
        dav_client: &DavClient<'_, T, A>,
        user: &str,
        remote_path: &str,
        bytes: Vec<u8>,
    ) -> OrchestratorResult<()> {
        let dav_path = make_dav_path(user, remote_path)?;
        dav_client
            .put(&dav_path, bytes, None)
            .await
            .map_err(|e| OrchestratorError::Api {
                operation: format!("uploading to remote path '{remote_path}'"),
                source: e,
            })
    }
}
