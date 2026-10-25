//! Filesystem and WebDAV scanners for discovering local and remote items.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::error::SyncError;

/// An item discovered on the local filesystem during traversal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveredLocalFile {
    /// Relative path within the local sync root (e.g. `"/Documents/report.txt"`).
    pub relative_path: String,
    /// Absolute host path on disk.
    pub absolute_path: PathBuf,
    /// Size in bytes.
    pub size: u64,
    /// Last modification timestamp.
    pub mtime: SystemTime,
    /// Whether this entry is a directory.
    pub is_directory: bool,
}

/// An item discovered on the remote Nextcloud instance via WebDAV.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveredRemoteFile {
    /// Relative path within the remote sync root (e.g. `"/Documents/report.txt"`).
    pub relative_path: String,
    /// Full WebDAV resource href.
    pub href: String,
    /// Remote file size in bytes.
    pub size: u64,
    /// Remote ETag string.
    pub etag: Option<String>,
    /// Whether this entry is a collection/directory.
    pub is_directory: bool,
}

/// Scans a local directory recursively, collecting all files and folders.
pub fn scan_local_directory(root: &Path) -> Result<Vec<DiscoveredLocalFile>, SyncError> {
    let mut collected = Vec::new();
    if !root.exists() {
        return Ok(collected);
    }

    scan_dir_recursive(root, root, &mut collected)?;
    Ok(collected)
}

fn scan_dir_recursive(
    root: &Path,
    current: &Path,
    collected: &mut Vec<DiscoveredLocalFile>,
) -> Result<(), SyncError> {
    let entries = fs::read_dir(current).map_err(|err| SyncError::LocalIo {
        path: current.to_path_buf(),
        context: "reading local directory contents".to_string(),
        source: err,
    })?;

    for entry in entries {
        let entry = entry.map_err(|err| SyncError::LocalIo {
            path: current.to_path_buf(),
            context: "reading local directory entry".to_string(),
            source: err,
        })?;

        let path = entry.path();
        let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");

        // Ignore temporary, hidden sync, or lock files
        if file_name.starts_with('.')
            || file_name.starts_with("~")
            || file_name.ends_with(".part")
            || file_name.ends_with(".crdownload")
        {
            continue;
        }

        let meta = fs::symlink_metadata(&path).map_err(|err| SyncError::LocalIo {
            path: path.clone(),
            context: "reading file metadata".to_string(),
            source: err,
        })?;

        let is_dir = meta.is_dir();
        let rel_path = path.strip_prefix(root).unwrap_or(&path);
        let rel_str = rel_path.to_string_lossy().replace('\\', "/");
        let canonical_rel = format!("/{}", rel_str.trim_matches('/'));

        let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);

        collected.push(DiscoveredLocalFile {
            relative_path: canonical_rel,
            absolute_path: path.clone(),
            size: meta.len(),
            mtime,
            is_directory: is_dir,
        });

        if is_dir {
            scan_dir_recursive(root, &path, collected)?;
        }
    }

    Ok(())
}
